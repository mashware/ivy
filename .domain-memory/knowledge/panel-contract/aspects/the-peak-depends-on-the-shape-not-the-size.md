---
id: asp_the-peak-depends-on-the-shape-not-the-size_h8y96s
slug: the-peak-depends-on-the-shape-not-the-size
name: >-
  The peak when interpreting JSON depends on the file's shape, not on its size:
  with the same 256 KiB the factor goes from 3× to 95×
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-12T14:07:12.510Z'
updated_at: '2026-08-12T14:07:12.510Z'
last_verified: '2026-08-12T14:07:12.510Z'
file_paths:
  - crates/panel-contract/src/panel.rs
  - crates/panel-contract/examples/read.rs
  - docs/04-contrato-panel.md
symbols:
  - MAX_BYTES
  - read
  - interpret
content_hashes:
  crates/panel-contract/src/panel.rs: 'sha256:2f88d49d982871a2367457566cc72b098caee9f6217d84e0ed9db26e29c6b7b7'
  crates/panel-contract/examples/read.rs: 'sha256:a8b364eb1f469d850d060adb5c8953582075b21cbb4022073e517dbf9726db8a'
  docs/04-contrato-panel.md: 'sha256:5af2cb170fb82f7c94d1f1a491cd2349f599e2bde0740449655e5fcc44a2e75f'
tags:
  - measurement
  - memory
  - frame
  - ceiling
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**Measured in `--release`, with the same 262 KiB in four different shapes.** What gets reserved is **one node per value**, so the worst case is the one that packs the most values into the same bytes:

| Shape | Lines | Peak | Factor | Interpreting |
|---|---|---|---|---|
| One single giant line | 1 | 769 KiB | **3.0×** | 0.1 ms |
| Lines like those of a real panel | 1 588 | 1.78 MiB | **7.1×** | 2.8 ms |
| Minimal objects of the contract | 20 163 | 16.27 MiB | 65.1× | 17.1 ms |
| Objects of some arbitrary key | 32 766 | **23.78 MiB** | **95.1×** | **22.3 ms** |

**A single number describes nothing here, and writing a single one already went wrong once.** The ceiling's comment said «256 KiB reach some 10 MiB of peak, some forty times»: the realistic case is **5.6 times cheaper** and the worst one **2.4 times more expensive**. Neither the figure nor the factor corresponded to any real shape.

**The normal case does not come anywhere close.** The nine `panel.json` files in the repo are read in some **10 µs each** —of which 2.8 µs are system calls— with peaks of 5-11 KiB. Re-reading the four of four panes on every frame would cost less than a quarter of 1 %.

> **The constraint for whoever reads from the painting loop is still the same —do not re-read on every frame— but its reason was the wrong one.** It is not that reading costs a frame: it is that a **pathological** file costs a frame and a half. Stated as «reading is expensive» it invites building an invalidation cache against a problem that real files do not have, while the real problem —laying out and painting 32 766 lines— is left untouched.

**Four panes do not multiply the peak by four**: the intermediates of each read die before the next one starts, so it is one peak plus three residents — 32 MiB in the worst case, not 95. And the memory comes back on releasing the panel.

**Discarded with its number:** deriving with `serde` instead of the generic value. In the real case it is **2.5 µs** of difference, 0.06 % of a frame with four panes. And `untagged` fixes nothing —it accumulates each element in its internal buffer before deciding the variant, almost the same as building the generic value—; only a hand-written visitor wins, and it is **strict**, which is exactly what per-field tolerance does not admit.

**When it would stop holding:** if the JSON interpreter's version changes, if the ceiling is touched, or if it is measured on another machine. `cargo run --release --example read -p panel-contract` returns that machine's numbers.

## Where it lives

The ceiling and its figures, in `crates/panel-contract/src/panel.rs` (`MAX_BYTES`). The bench that re-runs them, in `crates/panel-contract/examples/read.rs`. The warning for whoever publishes, in `docs/04-contrato-panel.md`.
