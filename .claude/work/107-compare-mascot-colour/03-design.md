# Study 107 — Compare mascot colour

## Statement of the change
Produce an evidence-based comparison of Claude Code's mascot in Ivy and Tilix under the same dark-daltonized theme. Record measurements and explain the colour source before deciding whether any renderer change is needed. No production change is currently justified.

## What already exists
- `crates/term-core/src/lib.rs:446`: child environment fixes TERM=xterm-256color and COLORTERM=truecolor.
- `crates/term-core/src/view.rs:659`: `color_from` copies Color::Spec channels directly into TermColor::Rgb, retaining indexed colours separately.
- `crates/app/src/theme.rs:390`: explicit RGB resolves through Color32::from_rgb; indexed colour resolves through the terminal palette.
- `crates/app/src/theme.rs:417`: bold and dim attributes are applied separately. Dim can change explicit RGB; truecolor support alone does not prove screenshot equality.
- `crates/app/src/theme.rs:167`: Ivy Night's bright-red entry is 0xf59a89 = (245,154,137), exactly the ticket's reported sample. This coincidence suggests checking indexed/bold-red output; it does not prove which sequence Claude emits.
- `crates/app/src/theme.rs:939–985`: existing tests cover exact colour preservation and bold's effect on indexed versus explicit RGB.
- `crates/app/src/painter.rs:458–462`: the painter calls with_attributes with the cell colours, bold and dim flags, accounting for inverse video.
- `docs/02-decisions.md`, D35 amendment for #103: box-drawing geometry is a separate concern. The ticket reports the same colour before and after that fix.

## Acceptance criteria
| ID | Observable result | Proof |
|---|---|---|
| AC1 | Same Claude Code version and dark-daltonized theme run in Ivy and Tilix; record terminal versions, Ivy theme, Tilix palette/bold policy, TERM/COLORTERM, font/size, scale, background and display settings. | manual |
| AC2 | Lossless screenshots contain at least three interior mascot samples per terminal with coordinates and exact RGB values, taken on the same display with matched opacity and no selection/hover overlay. State whether the corresponding RGB triplets match exactly; retain raw results. | manual |
| AC3 | Identify the mascot's foreground sequence and active attributes from a local PTY capture or equivalent inspection. Distinguish explicit RGB, indexed palette and bold/dim cases, and explain any measured difference using that evidence. | manual |

## Comparison procedure
1. Use a graphical Linux host with both Ivy and Tilix. Keep the application version, account/theme selection, display profile, scale and font consistent. Record actual environment in each child, not only the parent shell. Verify dark-daltonized inside Claude Code in both terminals.
2. Capture the mascot in both terminals with terminal backgrounds opaque. Save original PNGs and terminal configuration metadata. Do not resize or recompress before sampling. Take multiple fully covered interior pixels; antialiased edges blend foreground with background and are unsuitable as the only samples. If glyphs have no solid interior, report that limitation and add a solid swatch using the same foreground sequence.
3. Run these diagnostic swatches in both shells (reset attributes before each):
   - `printf '\033[0;38;2;245;154;137m████\033[0m\n'` — explicit RGB control.
   - `printf '\033[0;91m████\033[0m\n'` — bright-red palette entry.
   - `printf '\033[0;1;31m████\033[0m\n'` — bold normal red.
   These distinguish direct colour from palette/bold policy; swatches supplement the actual mascot comparison.
4. Capture a minimal local PTY output around the mascot, using an available PTY recorder, and inspect the foreground/background SGR commands plus bold, dim and reverse flags active at its cells. Confirm the capture has not changed terminal detection. Retain only the relevant sanitized escape sequence evidence, not unrelated session content.
5. Record a table: terminal/version, foreground sequence, attributes, sample coordinates, sample RGB, matching control swatch, screenshot filename. The reported Ivy RGB is an input from the ticket, not a measurement made in this study.
6. Equal mascot samples with comparable inputs: report that no discrepancy was observed in the tested setup and propose closing the question. Different indexed samples: compare the selected palettes and bold policy first; a configurable palette difference alone does not justify rewriting truecolor handling. Different explicit RGB samples: trace attributes, core channels, resolved paint colour, then renderer/display output. Fix only a demonstrated defect and add a regression at its actual boundary in a subsequent implementation step.

## Modules/layers affected
Evidence/report only initially. Existing parser, theme and painter are inspection points; no edits planned without a demonstrated defect.

## Data model / schema changes
None. No data access, migrations or indexes.

## External contracts
None changed. Diagnostic escape sequences above use existing terminal behaviour.

## Internal behavioural contracts
An undimmed explicit RGB input (245,154,137) must resolve to Color32 RGB (245,154,137). Indexed colour 9 intentionally uses the active terminal palette, which may differ between terminals.

## Depends on other tickets
None. #103 is background context, not a prerequisite.

## Planned tests and validation
Manual AC1–AC3 are the decisive proof. Existing exact-colour and bold tests support a future diagnosis but cannot prove framebuffer or Tilix equality. No tests were run for this study; no source code was changed. If a defect requires code, run relevant regression tests and the repository's required checks after designing that fix.

## Environment and current result
`command -v claude` resolves to /usr/local/bin/claude. `command -v tilix` returns no executable. DISPLAY and WAYLAND_DISPLAY are unset. Therefore no paired GUI capture or measured comparison was performed here. The question remains unanswered; neither theme colour nor renderer corruption is proven. Execute AC1–AC3 on a graphical host before making a colour change.

## Decisions
XS verification study: reuse existing paths and tests; do not introduce new tooling or palette adjustments before measurements. No approach panel or implementation plan is needed for the comparison. No defensive mechanisms introduced.
