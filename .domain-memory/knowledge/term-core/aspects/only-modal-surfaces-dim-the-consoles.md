---
id: asp_only-modal-surfaces-dim-the-consoles_rn72xn
slug: only-modal-surfaces-dim-the-consoles
name: >-
  Only modal surfaces may dim the consoles; the drawer and a title-bar press are
  not modal, and a dim test must read opacity
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-17T21:02:09.062Z'
updated_at: '2026-11-17T21:02:09.062Z'
last_verified: '2026-11-17T21:02:09.062Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/chrome.rs
  - crates/app/src/header/menu.rs
  - crates/app/src/sidebar/tests.rs
symbols:
  - block_pointer
  - 'chrome::is_modal'
  - 'tab_bar::partition'
content_hashes:
  crates/app/src/lib.rs: 'sha256:950e08f0c11dc4e836d4603065c1d76130a1517bdf2b90055eb160d1372ff098'
  crates/app/src/chrome.rs: 'sha256:80095ca2af73b1ff2409caa0e650499a1af203cbb32bf8e9037dbf5032f85f54'
  crates/app/src/header/menu.rs: 'sha256:e870b08e3af10add9a12102ae1adbaf43f59f9be8701c57effe00758ea759e15'
  crates/app/src/sidebar/tests.rs: 'sha256:e37c66d388d4d93824a126d2262084e7ff6c73e675c8b015f87392fe17ac6973'
tags:
  - dimming
  - modal
  - egui
  - tests
feature_id: feat_term-core_nx0g1r
---

## What it does

`tab_bar::partition`'s `dim_consoles` reaches `header::menu::block_pointer`, which calls
`ui.disable()`. That multiplies the painter's opacity by `disabled_alpha` (0.5) and keeps it only
when the flag is set.

The flag once included `viewer_owned` and `chrome_owned`:

- With the bottom drawer holding the keys, every console was shaded.
- While a title-bar button was held, every console painted at half opacity. The owner reported
  this as a blink on every button (#93 PR 6).

Since #93 PR 6 the flag is `pane_owned || chrome::is_modal` (a console's rename or menu, the title
bar's menu, a dialog). Nothing that flips with input, and no non-modal tool such as Files or the
drawer, may drive it.

Test disguise: a dimmed pass does not change a `TextShape`'s colour, only its `opacity_factor`,
and the header text is painted outside the dimmed painter. A test comparing header text colours
stays green under the mutation. Compare the consoles' `Rect` fills, and text colour multiplied by
`opacity_factor`. `neither_the_drawer_nor_a_title_bar_press_dims_the_consoles` does both and kills
both mutations. This supersedes [[dimming-on-a-per-frame-claim-blinks]], which still listed a
modal viewer as a steady state that may dim.

## Where it lives

crates/app/src/lib.rs (tab_bar::partition call), crates/app/src/chrome.rs (is_modal), crates/app/src/header/menu.rs (block_pointer), crates/app/src/sidebar/tests.rs
