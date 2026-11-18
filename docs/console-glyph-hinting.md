# Does hinting reach console text?

> Historical baseline: the results below describe unmodified epaint0.36.1. Ivy now applies the
> targeted fallback and raw-coverage policy described in the production update below.

Yes for the bundled Geist Mono in the tested configurations. Every sampled glyph changes when
hinting is disabled. Ubuntu Sans Mono 1.100 produces identical bitmaps with hinting on and off,
but **this is not an initialization fallback or a swallowed error**: the actual epaint hinted
branch succeeds, with hinting enabled. Its sampled glyph outlines have no hint instructions.

These results describe epaint 0.36.1 / skrifa 0.44.0 on Linux x86_64, Rust 1.97.1, measured on
2026-11-18. They do not explain all of the visual difference from Tilix: compositor filtering,
font version, gamma, subpixel rendering and hinting target can differ. No production setting changes.

## What was measured

The headless `hinting` example installs one exact face, with no fallback font, in fresh epaint
`Fonts` caches for default, forced-on and forced-off conditions. `Fonts::new` installs definitions
immediately; unlike `Context::set_fonts`, no extra egui pass is needed. Default must equal forced-on,
and a fresh forced-on run must repeat exactly; either mismatch stops the diagnostic.

Each `Iil1mw0O+-={}` glyph is laid out separately at 14.0 and 14.7 points, at 1 and 2 pixels per
point (14, 28, 14.7 and 29.4 ppem). Samples retain glyph ID, atlas UV bounds, dimensions, bearings,
baseline position, advance and every atlas alpha byte. Comparison uses the union of the bounds
on a shared baseline, with transparent padding. It never aligns tight crops independently or resamples.
Atlas alpha includes epaint's default color transfer; it is not raw rasterizer coverage.

The same matrix also measures a fixed-column console row, a normally shaped ASCII run, and the
optional ligature registration (a second copy of the face, bins disabled) placed on fixed columns.
The diagnostic mirrors the placement in `painter/ligatures.rs::on_grid`; it does not instantiate
the GUI or call the private painter. The column step floors the measured `M` width to device pixels before multiplying columns,
as `CellMetrics::measure` does. The fixed-column row uses individual layouts, as ordinary
console cells do. The ordinary shaped run retains epaint's subpixel positions. Actual bin choices
are visible in the instrumented trace. No ligature-font substitution or ligature-specific glyph
claim is made for these two fonts.

All variations use axis defaults and the same default color transfer. The target is
`Smooth { mode: Normal, symmetric_rendering: true, preserve_linear_metrics: true }`, epaint's
conversion of `SmoothHinting::default()` (`light=false`). The preserved horizontal metrics limit
horizontal fitting by design; they do not disable vertical hinting.

## Results

Aggregate **isolated-cell** results, summed across all 13 glyphs:

| Font | Points | Scale | Changed glyphs | Different pixels | Sum of absolute alpha differences |
|---|---:|---:|---:|---:|---:|
| Geist Mono | 14.0 | 1 | 13/13 | 413 | 13,722 |
| Geist Mono | 14.0 | 2 | 13/13 | 754 | 25,790 |
| Geist Mono | 14.7 | 1 | 13/13 | 529 | 19,661 |
| Geist Mono | 14.7 | 2 | 13/13 | 1,161 | 52,512 |
| Ubuntu Sans Mono | 14.0 | 1 | 0/13 | 0 | 0 |
| Ubuntu Sans Mono | 14.0 | 2 | 0/13 | 0 | 0 |
| Ubuntu Sans Mono | 14.7 | 1 | 0/13 | 0 | 0 |
| Ubuntu Sans Mono | 14.7 | 2 | 0/13 | 0 | 0 |

Fixed-column row and ligature-registration aggregates equal the isolated-cell columns above.
The normally shaped Geist run has different-pixel/absolute-difference totals of 398/13,521,
771/26,048, 528/19,226 and 1,149/50,325 respectively. Ubuntu remains 0/0 on every path.
The distinction therefore survives representative positioning and both registration settings.

Font identities (face index 0):

- Bundled `resources/fonts/GeistMono-Regular.ttf`: family **Geist Mono**, version
  `1.900; ttfautohint (v1.8.4.16-eb64)`, no variable axes.
  SHA-256 `686914333e77a620c85b35481064788deb33fc25f69901bdbf296c1bbc26a7fe`.
- Fixture `crates/app/tests/fixtures/hinting/UbuntuSansMono.ttf`: family **Ubuntu Sans Mono**,
  version **1.100**, `wght` axis 100–700, default 400. Normalized default coordinates are zero.
  SHA-256 `b27217ddbae92f98c59b31ec122e0b38ab3b3efca6e1edd77cb4300773913d87`.
  [Official upstream revision](https://github.com/canonical/Ubuntu-Sans-Mono-fonts/tree/c57353c1772eb8aaab9c539e3d42c971a03a5fcd),
  unmodified `fonts/variable/UbuntuSansMono[wght].ttf`; Ubuntu Font Licence retained beside the fixture.

## Did epaint silently fall back?

No such failure was observed in this matrix. Direct skrifa replay creates the instance at unscaled
size/default location/default target, reconfigures it at real ppem/default axes/**epaint's converted
target**, and draws hinted (non-pedantic) and unhinted outlines. Every creation, reconfiguration and
draw succeeds; `is_enabled()` is true after reconfiguration for both faces. This replay alone cannot
prove which epaint branch ran.

To resolve Ubuntu's equal output, an isolated epaint copy was instrumented at the actual creation,
reconfiguration, hinted draw and unhinted draw sites. It preserves the original `.ok()` semantics,
adding observations only. The full matrix using this copy is byte-identical to **both** unmodified
runs. The trace records 720 successful creations, 1,008 successful reconfigurations, 1,296 successful
hinted draws (all enabled) and 432 successful unhinted control draws, with no errors. Counts include
epaint's replacement-character setup and repeated fresh contexts; they are not unique glyph counts.
The trace includes font and sample boundaries, glyph IDs and subpixel bins.

Both faces report `prefer_interpreter=true`. Under skrifa's default `Engine::AutoFallback`, this
selects the TrueType interpreter, not the automatic hinter. Geist's sampled outlines all report
`has_hinting=true`; Ubuntu's all report `false`. Ubuntu has a nonempty seven-byte `prep` table,
which suffices to prefer the interpreter despite those glyphs lacking instructions. This source
rule plus the observed successful hinted draws explains why “hinting enabled” does not imply
changed bitmaps here. It is not evidence that variable fonts generally fail. Other fonts, weights,
characters, platforms and target settings remain unmeasured.

## Renderer path and failure semantics

- Ivy's ordinary console glyphs reach `Painter::text` in `crates/app/src/painter.rs`; the optional
  ASCII path uses `layout_no_wrap` in `painter/ligatures.rs`. Both use epaint's outline allocation.
  Geometric box/block drawing and color emoji are separate paths.
- `fonts/bundled.rs::base_definitions` puts Geist Mono before egui's fallback. System-face selection
  in `fonts/selection.rs` installs exact bytes/index without overriding hinting; its optional
  ligature copy overrides only subpixel binning.
- [epaint 0.36.1 source](https://docs.rs/crate/epaint/0.36.1/source/src/text/font.rs), lines 402–415:
  failed instance creation becomes `None`, selecting unhinted rendering. Lines 243–260:
  reconfiguration or hinted draw failure returns `None` from allocation; it does **not** take the
  unhinted branch there. This experiment observed neither failure. It does not characterize later
  replacement-glyph behavior under an injected failure.
- [skrifa 0.44.0 source](https://docs.rs/crate/skrifa/0.44.0/source/src/outline/hint.rs),
  `Engine::AutoFallback` / `resolve_auto_fallback` and `HintingInstance::is_enabled`, distinguishes
  engine selection, successful initialization and dynamic enablement.

## Reproduce and inspect evidence

From the repository root, with the pinned Rust toolchain and `sha256sum` installed:

```sh
cargo test -p app --example hinting
cargo run -p app --example hinting -- crates/app/tests/fixtures/hinting/UbuntuSansMono.ttf /tmp/hinting-a.json
cargo run -p app --example hinting -- crates/app/tests/fixtures/hinting/UbuntuSansMono.ttf /tmp/hinting-b.json
cmp /tmp/hinting-a.json /tmp/hinting-b.json
```

Invalid/missing files, missing characters, empty allocations, replay errors and repeatability
mismatches fail explicitly. Errors stop the run with an error message rather than emitting an
apparently equal empty sample. The example emits sample boundaries on stderr and JSON to the given
file. Its inline tests prove the comparator detects changed alpha, size and bearings, and that an
invalid face or unmapped character is an error.

Full results: [measurements.json.gz](evidence/hinting/measurements.json.gz) (decompress to JSON),
[epaint trace](evidence/hinting/epaint-trace.log.gz),
[verification](evidence/hinting/verification.txt),
[observation patch](evidence/hinting/epaint-observation.patch).
The patch SHA-256 is `024c87ec8e1dcf9d3c1c1fa8c1eb6e23064e2f9b9ba9ae79f292770266e0b83d`.

To reproduce instrumentation, copy the registry's **epaint-0.36.1** package to
`/tmp/ivy-hinting-epaint`, apply the observation patch there with `patch -p1`, then run:

```sh
cp Cargo.lock /tmp/ivy-hinting-Cargo.lock
cargo --config 'patch.crates-io.epaint.path="/tmp/ivy-hinting-epaint"' run -p app --example hinting -- crates/app/tests/fixtures/hinting/UbuntuSansMono.ttf /tmp/hinting-instrumented.json 2>/tmp/hinting-trace.log
cp /tmp/ivy-hinting-Cargo.lock Cargo.lock
cmp /tmp/hinting-a.json /tmp/hinting-instrumented.json
```

The temporary dependency and Cargo patch are not part of the repository configuration.


## Production update: instructionless fonts and raw coverage

The accepted native comparison used Ubuntu Sans Mono1.006 at11 typographic points on Wayland,
against the normal Tilix profile. The owner accepted the combined automatic-hinting/raw-coverage
preview after rejecting the original and automatic-hinting-only previews. This is visual acceptance
of that setup, not a claim of pixel parity with every desktop renderer or platform.

Ivy sets `FontColorTransferFunction::Off` for both context themes once when installing its fonts.
This preserves raster coverage instead of applying egui's default edge-weight boost. The policy
survives theme switching and font-definition rebuilds without rewriting styles every frame.

The local epaint patch caches a per-face decision: when the font prefers the interpreter but none
of its glyphs contains native hint instructions, use automatic hinting. Fonts with native glyph
instructions, CFF and unknown hint-information formats keep the original selection. The decision
is reused when size, scale, variation location or target reconfigures the hinter. Disabling hinting
still bypasses it. Glyph placement, target, cell flooring and default geometry remain unchanged.

`font_rasterization` checks actual fixture atlas masks: Ubuntu must change when hinting is enabled;
Geist keeps its exact original mask fingerprints at scales1 and2. A startup regression compares
actual glyph coverage to independent raw and boosted contexts in both themes. These tests use
repository fixtures and do not depend on an installed font.

The vendored sources retain their licenses and a bounded patch description in
`vendor/epaint/IVY-PATCH.md`. Raster initialization changes, so historical frame results are not a
measurement of this revision; no new performance improvement is claimed.
