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
symbols:
  - shown_notice
  - shown_empty
  - opened
  - Pasting
content_hashes: {}
tags:
  - drawer
  - notice
  - design
feature_id: feat_term-core_nx0g1r
---

## What it does

**`Viewer.notice` and the empty viewer's text have many writers**: Send, the unsent-marks refusal,
and every paste answer.

**The mechanism that failed.** #199 first wrote «A paste is already on its way.» into that slot
and kept the covered text aside to restore when the paste ended. In each of three review rounds
(7, 8 and 9) it failed a new way:
- a notice lost when another writer replaced the whole string;
- a notice brought back stale (Send's «Still finishing» corrected only in the visible copy);
- answers piled up;
- the cover stored inside itself on a third press.
Every patch added a layer to the same fault.

**What converged.** Never write it. Compose it at paint time (`paste::shown_notice` /
`shown_empty`) from a flag (`asked_again`) and the live `busy()` state. What is stored underneath
is never touched, so nothing needs restoring or correcting. The flag is cleared when the next read
starts and when another document opens (`paste::opened`). The change removed more code than it
added (+99 −167).

**Rule:** a status that lasts only while something is in flight is derived when painting, never
stored in a slot other writers share. A sign it is needed: a «covered» or «previous» copy of a
field appears.

**Test disguise:** assert on painted text (`text_rect`), not on `notice`, which by construction
never holds the status.

Evidence: mutations Z1–Z6 and W1–W3 in #199; `on_its_way_is_painted_only_while_that_paste_is`,
`on_its_way_does_not_follow_into_another_picture`.

## Where it lives

crates/app/src/viewer/image/paste.rs (`shown_notice`, `shown_empty`, `opened`, `Pasting.asked_again`); crates/app/src/viewer/image.rs (`Viewer::show`, `Viewer::start`)
