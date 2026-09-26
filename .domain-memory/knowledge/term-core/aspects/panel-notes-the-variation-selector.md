---
id: asp_panel-notes-the-variation-selector_fk3adg
slug: panel-notes-the-variation-selector
name: >-
  The panel notes every character for font discovery, the emoji selector
  included: a ☁️ in a panel makes the window search the machine
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T16:08:28.515Z'
updated_at: '2026-09-26T16:08:28.515Z'
last_verified: '2026-09-26T16:08:28.515Z'
file_paths:
  - crates/app/src/panel.rs
  - crates/app/src/fonts.rs
  - scripts/measure-frame.sh
symbols:
  - note_text
  - resolve
content_hashes:
  crates/app/src/panel.rs: 'sha256:373164c3893c7dff0102bc6de92cfe2daee837779bc865f38050c99a516b9560'
  crates/app/src/fonts.rs: 'sha256:6437b6d69ce3d04e8175d68c813e4c4caef651a8e3dbcad219ba51a77ce6fe91'
  scripts/measure-frame.sh: 'sha256:465f0d9927c9fbebf177895a7c675b43daa5fd1baa5c2ab6e50b91c84020a588'
tags:
  - font-discovery
  - measurement
  - batch-z
feature_id: feat_term-core_nx0g1r
---

## What it does

The status panel calls `discoverer.note_text(&row.text)`, which notes **every** character — a standalone `U+FE0F` included. The console notes one base character per cell and never sees a selector. No face egui 0.36 ships covers `U+FE0F` (its NotoEmoji-Regular does cover 🔥 ✅ 📦 ☁), so **a panel line with `☁️` sends the window off to search the machine for a font**.

In a measurement batch that is not `P` the guard «only P may search» invalidates the run — which is why batch `Z` puts 🔥 ✅ 📦 🚀 in its panels and no emoji with a selector. **Batch `Q` passing that same guard is not evidence**: its emoji go through the console's per-cell `note(cell.c)`, a different path.

Found by a skeptic that ran `fc-query` over `epaint_default_fonts-0.36.1` and read `fonts.rs` `resolve()`. Outside measurement it is a one-off search per session, not a defect anyone meets.

## Where it lives

crates/app/src/panel.rs `paint` (note_text); scripts/measure-frame.sh batch Z (`e1`..`e4`)
