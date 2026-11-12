# #58 — selected text shows gaps and a fragmented highlight

- Work: with `font.ligatures = on` (the reporter's config), shaped text runs are squeezed to the cell step and drawn off the pixel grid, so glyphs smear and spacing jumps where a selection re-cuts a run. Study only: built elsewhere (handoff).
- Size: S, one PR. Branch 58-selection-highlight-gaps, worktree `.worktrees/58-selection-highlight-gaps`.
- Fix (03-design.md): place each shaped glyph on its cell by translation only (no scaling), and register the ligature face with `subpixel_binning: Some(false)` so ink lands on whole pixels too.
- Acceptance: AC1–AC5 tests (quads at texel size, on the cell grid, same `uv` as the per-cell path, selection edges move nothing, ligature ink kept); AC6 manual with the reporter's font settings.
- The existing ligature test asserts the squeeze and must be rewritten.
- Out of scope, noted: rare 1 px seams between highlight rectangles at ppp 1.5/1.75 (f32 edge rounding).
- Depends on: none. Contracts: none.
- Next: handoff of the study.
