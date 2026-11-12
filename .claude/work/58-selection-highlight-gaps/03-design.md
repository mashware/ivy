# Design #58

## Approaches considered
not run at this size (S).

## Additional domain context
- `epaint-rounds-each-glyph-origin-on-its-own` (D34): epaint snaps each glyph origin and each filled rectangle to device pixels; the cell step is floored to whole pixels so per-cell glyphs land on a regular grid. The tessellator's own comment (`tessellator.rs:2022`): «the contents of the galley are already snapped to pixel coordinates» — it only rounds the galley's position.
- `a-guard-written-from-the-same-wrong-model-as-the-code`: assert the reason (glyphs drawn at their texel size, on the cell grid), not a shape a wrong fix also satisfies.

## Root cause (measured)

**The optional ligature path squeezes every shaped run horizontally, so its glyphs are resampled and land off the pixel grid.** `painter/ligatures.rs::on_grid` multiplies every glyph position and every mesh vertex x by `cell width / font advance`. Because D34 floors the cell to whole pixels, that ratio is always below 1:

| ppp | advance (pt) | cell (pt) | ratio | quads drawn at a size ≠ their texels | quads starting off a pixel |
|---|---|---|---|---|---|
| 1.0 | 7.84 | 7.00 | 0.893 | 51/51 | 49/51 |
| 1.25 | 7.84 | 7.20 | 0.918 | 51/51 | 47/51 |
| 1.5 | 7.84 | 7.33 | 0.935 | 51/51 | 49/51 |
| 2.0 | 7.84 | 7.50 | 0.957 | 51/51 | 51/51 |

Measured with a throwaway test (reverted) laying out `gh issue create --repo mashware/agent-runner --body-file` with the reporter's own settings — `font = Ubuntu Sans Mono`, `size = 14`, **`font.ligatures = on`** (read from `~/.config/ivy/config`). Every glyph quad is narrower than its raster and starts on a fraction of a pixel, so the linear-filtered atlas smears each glyph and the spacing between glyphs varies by up to a pixel: the «narrow vertical gaps between characters».

**Why it looks tied to selection and intermittent.** Runs are split by style, and `same_style` includes `selected`. Selecting text re-cuts the runs: a one-cell run, the cursor cell, or any non-ASCII cell is painted per cell at full size on the grid, while its neighbours in a run of two or more are squeezed. The selection highlight (swapped colours) makes the smeared columns of dark-on-light glyphs stand out. Whether a given line shows it depends on where runs break — selection edges, colours, cursor — hence «intermittent».

**Ruled out as the main cause.** Seams between adjacent highlight rectangles from f32 rounding of each edge on its own: emulated over 4 000 origins × 220 columns — zero at ppp 1.0, 1.25 and 2.0; 1–2 origins in 4 000 at 1.5/1.75. Real but rare and not the reporter's scale; recorded as a note, not in scope.

## What already exists (inventory)
- `painter/ligatures.rs` — `paint_row` (runs, eligibility, `same_style`), `layout` (refuses runs whose width differs from `len × advance`), `on_grid` (the squeeze). Its test `real_ligature_ink_and_fixed_grid_survive_fractional_sizes_scales_and_long_runs` **asserts the squeeze** (`b.pos.x == a.pos.x * ratio`): it pins the defect and must be rewritten.
- `painter/ligatures/tests.rs` — integration tests over the fixture `JetBrainsMono-Regular.ttf`.
- `painter.rs::paint_with_ligatures` / `paint_character` — per-cell path: one glyph per cell at `metrics.cell(...).left_top()`, unscaled; the reference for where a glyph belongs.
- epaint `Glyph { pos, advance_width, uv_rect, first_vertex }` — enough to move each glyph's quad without rasterising again.

## Executive summary
- Keep shaping (it is what substitutes the ligature glyphs) but **stop scaling** its output.
- Place each shaped glyph by **translation only**: its quad keeps its texel size and moves so its pen position lands on the origin of the cell it came from, a whole device pixel.
- **Rasterise the ligature face without subpixel binning** (`FontTweak::subpixel_binning = Some(false)` on the face the ligature family registers). epaint 0.36.1 bins by default (`text/mod.rs:62`) and puts the pen's fraction of `k × advance` into the raster (`font.rs:622`), so a translated quad would land on a whole pixel with its ink still up to ¾ px off. With binning off, every glyph is rasterised at phase 0 — as the per-cell path already is, since its galleys start on a whole pixel.
- With both, an unsubstituted glyph is drawn exactly like the per-cell path — same position, same atlas texels — so a selection edge that re-cuts a run changes nothing.
- Rewrite the test that pins the squeeze into one that pins the reason.

## Acceptance criteria

| ID | Given / When / Then | Proof |
|----|---------------------|-------|
| AC1 | Given ligatures on and a shaped run, when it is painted at ppp 1.0, 1.25, 1.5 and 2.0 with sizes 13, 14 and 17.3, then every glyph quad's width in device pixels equals its texel width (no glyph is scaled) | test |
| AC2 | Given a shaped run of N cells, then every glyph's pen x equals its source cell's x (`origin + column × cell width`), and the quad's left edge is a whole device pixel | test |
| AC3 | Given text the font does not substitute (`--repo mashware/agent-runner`), then the shaped path and the per-cell path put every glyph quad at identical device-pixel positions **and with identical `uv` (the same atlas texels)** — quad geometry alone is satisfied by a glyph rasterised at another subpixel phase | test |
| AC4 | Given a run, when a selection starts or ends inside it, then no glyph outside the selection edge changes position, size **or `uv`** versus the unselected painting | test |
| AC5 | Ligature substitution still produces different ink from the separate glyphs (`!=`, `=>`, `==`, `===` on the fixture) — existing assertion kept | test |
| AC6 | With the reporter's settings (Ubuntu Sans Mono, size 14, `font.ligatures = on`), selecting Codex-style command output shows no gaps or fragmented highlight, compared with `font.ligatures = off` | manual |

## Modules/layers affected
- `crates/app/src/painter/ligatures.rs` — replace `on_grid`'s scaling with per-glyph translation onto the cell grid.
- `crates/app/src/painter/ligatures/tests.rs` (or the inline test) — the new assertions.
- `docs/02-decisions.md` — amend the decision that introduced optional ligatures (#335) with how a shaped run is placed and the discarded scaling.

## Data model
- New / modified entities: none.
- Migrations: none.
- Indexes: none.

### Access paths
N/A — no data access.

## External contracts
none.

## Internal behavioral contracts
- **Placement rule**: for a shaped run starting at column `s`, a glyph whose pen position is `p` (in the galley, points) belongs to cell `k = round(p / advance)` and is translated by `(s + k) × cell_width − p`; vertex x never changes by any factor other than this translation. `uv` never changes.
- **Invariant**: for every glyph quad, `width_px == uv_width_texels` and `left_px` is an integer.
- **Edge**: a run the font lays out wider than `len × advance` (± a pixel) is still refused and falls back to per-cell painting (existing behaviour kept).
- **Invariant**: the ligature face is rasterised at subpixel phase 0 only; an unsubstituted glyph in a shaped run uses the same atlas entry (`uv`) as the per-cell path.
- **Edge**: a substituted glyph spanning several cells (true merged ligature) is placed at the cell of its pen position and keeps its own design width (`n × advance`), so it overhangs the run's last cell by `n × (advance − cell)` — about 0.84 px per cell for Ubuntu Sans Mono 14 at ppp 1; `calt` connectors overlap their neighbours by `advance − cell`. Same-colour ink, nothing clipped, the same overlap per-cell glyphs already have under D34. Named in AC6's manual check.
- **Mapping**: shaping keeps one glyph per char through zero-width continuation glyphs (`text_layout.rs:226–229`), which the tessellator skips; so the glyph index equals the column offset within an ASCII run as well as `round(p / advance)`.

## Depends on other tickets
none.

Not dependencies (either order works): none.

## Defensive mechanisms and their justification

| Mechanism | Real scenario that justifies it (with evidence) | Needed now? |
|-----------|--------------------------------------------------|-------------|
| Keep `layout`'s refusal of runs whose width ≠ `len × advance` | Already there; a font that kerns or changes advance inside a run would otherwise misplace cells | kept, not new |

No new mechanism is introduced.

## Implementation plan (order)
1. Rewrite the existing squeeze assertion in the inline test into AC1/AC2 (red against today's code).
2. Add AC3 and AC4 tests over a real mesh (Ubuntu Sans Mono is not a fixture; use the JetBrains fixture plus a non-substituted string).
3. Register the ligature face with `FontTweak { subpixel_binning: Some(false), .. }` where `fonts/selection.rs::queue_definitions` adds `terminal-ligature-face`. If that `FontData` entry is shared with the per-cell family, the tweak reaches it too, which is harmless (per-cell galleys already start on a whole pixel, so they bin to 0).
4. Replace `on_grid`: clone the galley as today, then for each glyph translate its four vertices (via `first_vertex`) and its `pos.x` onto the cell grid; recompute `row.size.x`, `mesh_bounds` and `galley.rect` from the translated geometry.
5. Mutate: put the scale back → AC1 red; drop the snap to the cell → AC2/AC3 red; turn binning back on → AC3/AC4 red on `uv`.
6. Amend the ligatures decision in `docs/02-decisions.md`.
7. Manual check AC6 with a temporary `XDG_CONFIG_HOME` carrying the reporter's three font lines.

## Planned tests
- Unit: AC1, AC2 (ligatures.rs inline), AC5 kept.
- Integration: AC3, AC4 (ligatures/tests.rs, painting a real row).
- Functional: AC6 manual.

## Decisions (ADR-light)
| Decision | Discarded alternative | Why |
|----------|-----------------------|-----|
| Translate each shaped glyph onto its cell | Scale the galley to the cell step (today) | Scaling resamples every glyph and leaves 47–51 of 51 quads off a pixel at every scale measured |
| Translate each shaped glyph onto its cell | Skip shaping when advance ≠ cell width | D34 floors the cell, so advance ≠ cell for almost every font and size: it would switch ligatures off in practice |
| Translate each shaped glyph onto its cell | Shape, then repaint each glyph per cell via `painter.text` | Loses the substituted glyph ids (per-cell text reshapes one char) — the ligature itself disappears |
| Translate each shaped glyph onto its cell | Lay the run out at `size × cell / advance` | Native rasters on the grid, but glyphs come out smaller than per-cell ones, so a selection edge that re-cuts a run would change glyph size |
| Ligature face rasterised without subpixel binning | Keep epaint's default binning | With binning, the pen fraction of `k × advance` is baked into the raster (bins ¾, ¾, ½ for glyphs 1–3 at ppp 1), so ink stays up to ¾ px off the grid after translation |

## Design challenges

Challenger: one `general-purpose` agent (opus), full report in `challenge.md`. Budget 1/12.

| Angle | Finding | Type | Severity | Response |
|-------|---------|------|----------|----------|
| Assumption | Translation keeps each glyph's subpixel phase from the old pen (epaint bins by default), so ink stays up to ¾ px off the grid | missing | high | Incorporated: ligature face registered with `subpixel_binning: Some(false)` (summary, plan step 3, ADR row). Verified: `text/mod.rs:62`, `font.rs:428/622` |
| Guard model | AC3/AC4 compared quad geometry only, which a binned glyph also satisfies | missing | high | Incorporated: AC3/AC4 now assert identical `uv`; mutation step adds «binning back on → red» |
| Simplification | Turn binning off on the ligature face | missing | medium | Adopted (same as the first row) |
| Edge | Multi-cell ligature / `calt` overhang of `advance − cell` per cell not named | missing | low | Named in the Edge contract and in AC6 |
| Idiom | Missing alternative: lay the run out at a smaller size | idiom | low | Added as an ADR row |
| Cell mapping, root cause, frame cost, need | No findings (mapping holds; root cause plausible; no undeclared frame work — translation replaces the multiply inside the existing `LigatureCost.spent` span) | — | — | — |

Design recorded as accepted (`autonomy.mode: auto`).
