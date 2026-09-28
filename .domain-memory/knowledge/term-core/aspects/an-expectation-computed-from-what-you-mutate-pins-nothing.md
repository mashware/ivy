---
id: asp_an-expectation-computed-from-what-you-mutate-pins-nothing_mrkd01
slug: an-expectation-computed-from-what-you-mutate-pins-nothing
name: >-
  Three new ways for a painting test to pass without pinning anything: the
  expectation computed with what you mutate, the maximum of two, and the color
  that exists but is not seen
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-24T15:24:02.373Z'
updated_at: '2026-08-24T15:24:02.373Z'
last_verified: '2026-08-24T15:24:02.373Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/theme.rs
  - crates/app/src/panel.rs
symbols:
  - 'Theme::attention_color'
  - 'Tab::attention'
  - 'Window::paint_the_tab_bar'
  - AttentionState
content_hashes:
  crates/app/src/lib.rs: 'sha256:b6bf814641dc6477356840fc727cffda19e824d7d5e80d702fefcf5161b30cf9'
  crates/app/src/theme.rs: 'sha256:802410da838730679f28ad3ee77a9b373271839c5af59055f591299f621c3f0c'
  crates/app/src/panel.rs: 'sha256:d9844bb19996c434945fd41689580cbaa88ac00d1919dea2597ba2c5346c7319'
tags:
  - tests
  - mutation
  - painting
  - egui
feature_id: feat_term-core_nx0g1r
---

## What it does

**Three new disguises, all three caught by a reviewer and none visible by reading the diff.** They came
out of the same milestone (F2.8, August 2026) and none of them is the already filed disguise of
[[a-test-that-pins-the-helper-does-not-pin-the-path]]: in all three, the test **does go in through the
real path** —the painting— and still pins nothing.

### 1. The expectation computed with the very function being mutated

The painting test compared each symbol's color against `tema.attention_color(clase, sostenido)` — **the
very function whose table was meant to be pinned**. Crossing the amber of `wait` with the red of
`block`, or painting `wait` with the grey of normal text, left **all 525 tests green**: the change is
applied to both sides of the equality and cancels out.

> **An expectation that is computed by calling what is being tested measures that the code is consistent
> with itself, not that it is correct.** The concrete value —`t.paleta[3]`— has to be written in the test
> at least once.

It is the same root as the project's disguise nº 1 (asserting over the field where a value is noted down
instead of over its effect), with the twist that **here the assertion IS over the effect**: over the
color that reaches the shape queue. What fails is the other side of the comparison.

### 2. With two elements, position wins and not the rule

Twice in the same milestone, with two different mechanisms:

- **The tie-break.** «At equal class the claim someone is holding wins» was being tested with the live
  one **always in the second pane**. `Iterator::max` returns **the last** of the tied ones, so the live
  one was winning **by order**. Removing the tie-break from the `Ord`, the tab bar's nine tests stayed
  green.
- **The accumulation.** «The card shows what **its** panes claim most» was being tested with tabs of
  **two**. A pair does not tell «all of them» apart from «the first two»: keeping only the first two left
  **eleven** tests green.

> **With two elements, half a dozen ways of accumulating badly give the same result.** Three elements and
> **all** their orderings; with two, at least both orders.

### 3. A color can exist in the shape queue and be invisible on screen

The active tab's symbol was being painted with its correct color **on top of the focus fill**: measured
contrast **1.01:1** for the red of the block and **1.02:1** for the dimmed yellow — the same color, as
far as reading it goes. The test asserted the text's color and passed with the defect in place; the
number next to it was already being inverted for this same reason, written down in its comment.

> **In a painting test, asserting a shape's color is not asserting that it is seen: it is seen or not
> according to what is UNDERNEATH, and that is another shape in the queue.** Where a color falls onto a
> fill that the application itself chooses, the test has to look at the pair.

### 4. Two palette entries hold the SAME value, so writing the literal pins neither

**Disguise 1's own fix has a hole, and this is it.** «The concrete value has to be written in the test
at least once» assumes the value identifies the entry. In this theme it does not:
`TermColor::Cursor` and `TermColor::Foreground` are **both** the project's grey — asserted side by
side, three lines apart, in `the_three_default_colors_are_the_project_ones` (`crates/app/src/theme.rs`).
So a cursor test that writes `Color32::from_rgb(0xA7,0xA7,0xA7)` stays green when the two entries are
swapped, and green when the painter reads the wrong one: it is disguise 2 of **D30** — a number where
the defect and the fix agree — arriving through the palette instead of through the assertion.

> **Before pinning a colour by its literal, check whether another entry of the palette holds the same
> one.** Where one does, the test needs a theme built for it, whose entry under test differs from its
> twin, and the mutation list has to include swapping the two entries — the only mutation that catches
> it.

**And building that theme is only half the fix, which is the half that is easy to stop at.** The
delivery that told the focused console's cursor apart (#209, September 2026) built exactly such a
theme — cursor magenta, text grey — and then read the colour back with
`theme.color(TermColor::Cursor)`, **the same call the painter makes**, to find the shape in the queue.
Both sides of the equality came from the accessor again. Measured: with
`TermColor::Cursor => self.foreground`, **all seven of its new tests stayed green**; only an unrelated
settings test fell. Written out as a `const` on the assertion's side, the same mutation falls seven of
eight.

> **The rule, sharper than «write the value once»: write it on the ASSERTION's side, not the
> fixture's.** A fixture and an assertion that both call the same accessor are one side of an equality
> wearing two names. The tell is a test that constructs its input through the same API it verifies its
> output with — here `Theme::from_colors(…cursor: X)` and `theme.color(TermColor::Cursor)`, five lines
> apart, both reading as «the cursor's colour».

## Where it lives

The first three were discovered while reviewing PR #99 and #100 of agent-terminal; the fourth in
PR #212 (September 2026), by a test auditor that mutated the palette accessor — something the
delivery's own mutation rounds never thought to touch, which is [[a-mutation-only-tests-what-its-author-imagined]]
once more. The first was caught by a
test auditor that mutated for real; the second, twice, by a test auditor and a coverage auditor; the
third, by a code reviewer who **computed the contrast** instead of reading the color.

The fix for each is in `crates/app/src/lib.rs` (`mod tests`) and in `crates/app/src/theme.rs`:
`each_attention_claim_carries_the_color_the_decision_assigned_it` nails each class to its palette entry;
`between_two_equal_claims_the_card_shows_the_one_someone_holds` walks both orders and
`with_three_consoles_the_card_shows_the_claim_that_rules` the six orderings of three; and
`the_active_cards_symbol_is_painted_over_the_background_and_not_over_the_highlight` looks at what is underneath.
The fourth's is in `crates/app/src/painter.rs` and `crates/app/src/lib.rs`: `A_CURSOR_OF_ITS_OWN`, a
written-out constant used both to build the theme and to find the shape, with
`theme_whose_cursor_is_its_own` / `window_whose_cursor_is_its_own` around it.

Related: [[a-test-that-pins-the-helper-does-not-pin-the-path]] (nine times, the neighboring disguise),
[[a-drag-test-needs-two-passes-at-least]] (the scenario that cannot occur) and
[[seeding-can-be-pinned-timing-not-always]] (when a test cannot sustain a measured span).
