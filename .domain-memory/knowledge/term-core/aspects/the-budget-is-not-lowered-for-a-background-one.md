---
id: asp_the-budget-is-not-lowered-for-a-background-one_ryegvz
slug: the-budget-is-not-lowered-for-a-background-one
name: >-
  «That a background console keeps running» and «that it consumes less» are
  incompatible by construction: cutting its budget trips the brake on it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:25:49.343Z'
updated_at: '2026-08-15T21:25:49.343Z'
last_verified: '2026-08-15T21:25:49.343Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - BUDGET
  - 'Pane::pump'
  - pump_them_all
content_hashes:
  crates/app/src/lib.rs: 'sha256:b3ca4328c99c0b94b314ea9a6c6d1ab4f6afe0dad22ef3415700f289bc9af411'
tags:
  - budget
  - tabs
  - F2.1
  - measured
feature_id: feat_term-core_nx0g1r
---

## What it does

**The pumping budget is per pane, not the window's**, and that decides what can be done with a
console that is out of sight: `PUMP_BUDGET` is asked for in full in every `Pane::pump`, and
`pump_them_all` calls it once per pane. With N panes the real ceiling per frame is **N×128 KiB**.

> **The consequence is a design contradiction, not a tuning knob:** «that a background console keeps
> running» and «that a background console consumes less» **are incompatible by construction**.
> Lowering its budget means no longer emptying its pseudoterminal at the rate it produces, and the
> margin the kernel gives away before blocking the program is **20 480 bytes**
> ([[the-pseudoterminal-measured]]), **six times less than the budget itself**. That is, cutting it
> **trips the brake of D8**: the program blocks on writing. It is not that the painting goes slower
> — **it is that the work stops**.

**What does share things out, and does not need building, is the frame itself:** every pane swallows
its 128 KiB and the frame gets longer, so the total throughput shares itself out on its own without
any of them stopping. The price is paid in frames per second of **the whole** window.

**With numbers** (arithmetic over the 4.35-4.51 ms measured for two consoles in the D3 annex; ~2.18
ms per console at a full budget):

| Tabs dumping | Frame | 120 Hz | 60 Hz |
|---|---|---|---|
| 1 (measured) | 7.01 ms | 84 % | 42 % |
| 2 | ~11.3 ms | **136 %** | 68 % |
| 4 | ~20.1 ms | 241 % | **121 %** |

**Two tabs dumping at once already go outside the 120 Hz slot**, and four go outside the 60 Hz one
as well. It is the explicit price of the milestone's promise, accepted and to be published — not a
defect. **Still tabs —the design case, agents waiting— cost nothing**: one empty `try_recv` per pane.

**And the alternative lever is not there either:** a budget by time instead of by bytes is turned
down in writing, because then the cost of parsing is by definition whatever it is set to and **the
instrument gives back what one put in oneself**.

## Where it lives

`PUMP_BUDGET`, `Pane::pump` and `Window::pump_them_all` in `crates/app/src/lib.rs`
