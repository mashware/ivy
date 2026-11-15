---
id: asp_bundled-fonts-are-the-base-every-rebuild-starts-from_zqxy6f
slug: bundled-fonts-are-the-base-every-rebuild-starts-from
name: >-
  Bundled fonts must be the base every font-definition rebuild starts from,
  never the «chosen» face: queue_definitions rebuilds from scratch on every
  write
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-15T19:14:38.044Z'
updated_at: '2026-11-15T19:14:38.044Z'
last_verified: '2026-11-15T19:14:38.044Z'
file_paths:
  - crates/app/src/fonts/bundled.rs
  - crates/app/src/fonts/selection.rs
  - crates/app/src/fonts/startup.rs
symbols:
  - base_definitions
  - queue_definitions
  - choose_the_font
content_hashes:
  crates/app/src/fonts/selection.rs: 'sha256:bfd930fa3c364289a7d2fc4acd4cf1d20a2794300495600c79c6bedc4f564949'
  crates/app/src/fonts/startup.rs: 'sha256:1a62da42bc6cb8775829f3db8f476bb56da3cabca0566b16268bca38285bf367'
tags:
  - fonts
  - egui
  - D95
  - D24
  - D9
feature_id: feat_term-core_nx0g1r
---

## What it does

`fonts::selection::queue_definitions` is the only writer of `set_fonts` after start-up, and it rebuilds the whole `FontDefinitions` from scratch every time: a configured family, a fallback-discovery round, a Settings preview, clearing the font. Anything registered once outside that base is dropped by the next rebuild. So the bundled Geist / Geist Mono (D95) live in `fonts::bundled::base_definitions()`, which `queue_definitions` starts from instead of `FontDefinitions::default()`, and `choose_the_font` installs that base first on every start-up, whatever the configured family turns out to be.

Registering them as the `chosen` face instead would have failed three ways: clearing `font` in Settings reverts to Hack; the first CJK discovery round drops them; and a chosen face is what arms the ligature family.

Side effects that bit tests: every test whose «factory» baseline was egui's Hack must now start from the bundled base (`fonts::install_bundled` in a test context), and Geist's taller line pushed the second row of Settings' ANSI swatches below an 850-pt viewport. Geist Mono lacks `⊘`, `∎`, `✓` and braille; `⊘` falls to Hack, 0.03 pt wider than a Geist Mono cell at 14 (advance 1233/2048 vs 600/1000).

Evidence: `fonts::bundled::tests::a_start_up_with_no_font_paints_with_the_bundled_faces`, `tests::a_font_the_machine_does_not_have_leaves_geist_mono_in_place` (mutation «skip install at start-up» killed); fc-query of GeistMono-Regular.ttf charset; panel symbol tests measuring 8.4287 vs 8.4.

## Where it lives

crates/app/src/fonts/bundled.rs (base_definitions, install); crates/app/src/fonts/selection.rs (queue_definitions); crates/app/src/fonts/startup.rs (choose_the_font)
