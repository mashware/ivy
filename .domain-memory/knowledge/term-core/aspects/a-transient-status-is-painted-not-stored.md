---
id: asp_a-transient-status-is-painted-not-stored_sr7tqv
slug: a-transient-status-is-painted-not-stored
name: >-
  In the image drawer a status that lasts only while something is in flight is
  composed when painting, never written into the shared notice
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-22T21:05:10.008Z'
updated_at: '2026-11-22T21:05:10.008Z'
last_verified: '2026-11-22T21:05:10.008Z'
file_paths:
  - crates/app/src/viewer/image/paste.rs
  - crates/app/src/viewer/image.rs
  - crates/app/src/viewer/view.rs
symbols:
  - shown
  - Pasting
content_hashes: {}
tags:
  - drawer
  - notice
  - design
feature_id: feat_term-core_nx0g1r
---

## What it does

**A console's image `Slot.notice` has many writers**: Send, the unsent-marks refusal, and every
paste answer.

**The mechanism that failed.** #199 first wrote «A paste is already on its way.» into that slot
and kept the covered text aside to restore when the paste ended. In each of three review rounds
(7, 8 and 9) it failed a new way:
- a notice lost when another writer replaced the whole string;
- a notice brought back stale (Send's «Still finishing» corrected only in the visible copy);
- answers piled up;
- the cover stored inside itself on a third press.
Every patch added a layer to the same fault.

**What converged.** Never write it. Compose it at paint time (`paste::shown`, beside the image and
in the empty Image tab) from the console it was asked again for (`asked_again`) and the live
`busy()` state. What is stored underneath is never touched, so nothing needs restoring or
correcting. The flag is cleared when the next read starts. On the first PR the change removed more
code than it added (+99 −167); the rebuild over one image per console (#198) kept it.

**Rule:** a status that lasts only while something is in flight is derived when painting, never
stored in a slot other writers share. A sign it is needed: a «covered» or «previous» copy of a
field appears.

**Test disguise:** assert on painted text (`texts` over the drawer's rect), not on `notice`, which
by construction never holds the status.

Evidence: mutations in #199; `a_paste_asked_again_is_painted_only_in_its_consoles_tab_while_on_its_way`.

## Where it lives

crates/app/src/viewer/image/paste.rs (`shown`, `Pasting.asked_again`); crates/app/src/viewer/image.rs (`Viewer::show`); crates/app/src/viewer/view.rs (the empty Image tab)
