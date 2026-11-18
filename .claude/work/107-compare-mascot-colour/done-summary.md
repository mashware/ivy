<!-- flow-study -->
## What and why
Compare Claude Code's mascot colour in Ivy and Tilix with the same dark-daltonized theme. This study prepares the comparison; it does not establish that the colours match or differ.

## Acceptance criteria
- AC1: Record matching Claude version/theme plus terminal versions, palettes, bold policy, child environment, font, scale and display settings.
- AC2: Save lossless paired screenshots and at least three interior RGB samples with coordinates per terminal; report exact equality or differences.
- AC3: Identify the emitted foreground sequence and active attributes, distinguish explicit RGB from indexed/bold/dim colour, and explain the measured result.

## Design in brief
Use the graphical comparison procedure in 03-design.md, including explicit RGB, bright-red and bold-red control swatches. Ivy copies explicit RGB channels through its core and theme, with dim handled separately. The ticket's reported (245,154,137), hex #f59a89, also exactly matches Ivy Night's bright-red entry. That is a diagnostic clue, not proof of Claude's output.

Run the comparison before changing colours. Equal results support closing the question for that setup. Different results require distinguishing palette configuration from an actual rendering defect before proposing a fix. No other ticket dependencies.

## MR/PR plan
None at this stage: collect comparison evidence and report the result first. A demonstrated defect would need a concrete fix and regression coverage; no speculative production edits are planned.

## Schema changes
none

## Risks
The comparison remains unverified: Tilix is unavailable and DISPLAY/WAYLAND_DISPLAY are unset in this runner. A person or subsequent execution on a graphical host must perform AC1–AC3. Antialiasing, opacity, display profiles and differing palettes can confound screenshot samples; record and control these settings. No tests run and no code changed in this study.
<!-- end flow-study -->
