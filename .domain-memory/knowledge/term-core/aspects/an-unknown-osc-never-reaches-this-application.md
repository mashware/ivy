---
id: asp_an-unknown-osc-never-reaches-this-application_60sx7n
slug: an-unknown-osc-never-reaches-this-application
name: >-
  An unrecognised OSC never reaches this application: the `Handler` is the
  library's own `Term`, so «just use OSC» means vendoring the parser or scanning
  the hot path twice
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:22:05.512Z'
updated_at: '2026-09-11T20:22:05.512Z'
last_verified: '2026-09-11T20:22:05.512Z'
file_paths:
  - crates/term-core/src/screen.rs
  - docs/05-roadmap.md
symbols:
  - 'Screen::advance'
  - Processor
  - VoidListener
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:dacdeb0a045bcab12705289ccfc29b3de7cda7b9e72f97da9f6c7819e1d955d2'
  docs/05-roadmap.md: 'sha256:2df23979f7e61d6abc400d13bbf58be20254a8d687b1f634e35d2fa34db78cbd'
tags:
  - osc
  - vt
  - phase-3
  - transport
feature_id: feat_term-core_nx0g1r
---

## What it does

**OSC as an inbound channel is not a small change here, because an unrecognised OSC is dealt with inside `alacritty_terminal` and never surfaces.** `Screen` holds a `Term<VoidListener>` and a `Processor`, and everything goes through `self.parser.advance(&mut self.term, bytes)` (`crates/term-core/src/screen.rs:4, 56-57, 213-214, 235-236`). The `Handler` is the library's own `Term`; this project implements none. There is no hook to hang a private sequence on.

The two ways out, both expensive, and worth knowing **before** anyone proposes it:

- **Vendor `alacritty_terminal`.** The repo's only vendored patch (`egui-winit`, D33) changes **one `return`**, and it still required `tests/vendor_patch.rs` with content hashes to keep it honest. A `Handler` implementation is a far larger surface to carry across upgrades.
- **A second byte scanner in front of `advance`** — on the hot path D8 caps at 1 MiB, which is the path every measured throughput number in this project describes.

> This is why the roadmap places OSC **after F2.7**, and why a phase-3 design that replaced the MCP server with OSC sequences was ranked weakest of four by two of three critics. **The portability argument (ConPTY, macOS) is the one everybody repeats; the one that actually decides it is that the parser is somebody else's and the door is not open.**

The OSC proposal had a genuine merit that is worth keeping separately: scoping over the PTY cannot be forged, because the message arrives on *that* pane's descriptor rather than carrying an identifier the sender chose. The same property is obtainable from a per-pane endpoint — see [[panel-id-is-a-name-not-a-credential]] — without touching the parser.

## Where it lives

`crates/term-core/src/screen.rs` — the `VoidListener` import at `:4`, the fields at `:56-57`, construction at `:213-214`, and `advance` at `:235-236`. The roadmap's placement of OSC is in `docs/05-roadmap.md`, Part I, channel table.
