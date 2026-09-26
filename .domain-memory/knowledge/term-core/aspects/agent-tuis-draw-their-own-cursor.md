---
id: asp_agent-tuis-draw-their-own-cursor_5198fj
slug: agent-tuis-draw-their-own-cursor
name: >-
  Agent TUIs hide the terminal's cursor and draw their own as an inverted space:
  'where the cursor is' misses it, and 'a space is empty' paints over it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T18:09:22.229Z'
updated_at: '2026-09-26T18:09:22.229Z'
last_verified: '2026-09-26T18:09:22.229Z'
file_paths:
  - crates/app/src/painter/emoji.rs
symbols:
  - spills
content_hashes: {}
tags:
  - cursor
  - agent-tui
  - painting
feature_id: feat_term-core_nx0g1r
---

## What it does

Ink-based agent TUIs (Claude Code, opencode) send `CSI ?25l` and draw their own cursor as an **inverted-video space**. Any painting rule that decides from the terminal's cursor misses the one a person sees, and any rule treating a space as empty paints over it. When #283 let a selector emoji spill into a blank neighbour, the first version spilled over that fake cursor; the fix is to count a neighbour as blank only when it is a space **painted like the emoji's own cell** — same resolved background, same inversion, same selection. term-core already resolves inverse video into `bg`, so background equality alone rejects the fake cursor; matching inversion (not forbidding it) keeps the spill inside an inverted status bar, and matching selection keeps an emoji from changing size while a selection is dragged over it.

The converse trap: **reading whether the cursor is hidden** makes a rule flip on every redraw, because full-screen programs hide and show the cursor around each one. Read the cursor's position only. The declared price: a program that parks its hidden cursor right after an emoji keeps it in one cell. Found by the silent-failure and built-in reviews of #283; each clause is bound by a test with a mutation that went red.

## Where it lives

crates/app/src/painter/emoji.rs `spills` and its tests in crates/app/src/painter.rs
