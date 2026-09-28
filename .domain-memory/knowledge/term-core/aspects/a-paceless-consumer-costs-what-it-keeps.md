---
id: asp_a-paceless-consumer-costs-what-it-keeps_km0yd6
slug: a-paceless-consumer-costs-what-it-keeps
name: >-
  A consumer that drains Pty::spawn's channel without pace unbrakes the program,
  not the memory: memory grows only with what it keeps, and no API shape can
  bound that
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T19:58:40.973Z'
updated_at: '2026-09-28T19:58:40.973Z'
last_verified: '2026-09-28T19:58:40.973Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/examples/leak.rs
  - crates/app/src/pty_output.rs
symbols:
  - 'Pty::spawn'
  - READS_IN_FLIGHT
  - QUEUE_CEILING
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:1d66d00a4b2be7d5c1cf8cfebe5f6be9914e5dbd1068827d713ac5e746f0bf4e'
  crates/app/examples/leak.rs: 'sha256:56b12192bd25ad7b2ab6eecc87d1b6cc15c52b38c8c0e402bb311eae8c91e048'
  crates/app/src/pty_output.rs: 'sha256:77b6a2457cccfa8410bf60bd99ae66dd83dd904dcde34f299f6a7118c8012fd8'
tags:
  - backpressure
  - measurement
  - memory
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two properties that a bare `for` over the output channel is easy to confuse.** It disables the brake on the *program* — the channel never fills, so the program inside runs flat out, like `> /dev/null`. It does **not** disable the bound on *memory*: the channel stays at `sync_channel(READS_IN_FLIGHT)` however it is drained, and resident memory grows only with what the consumer itself keeps.

**Measured 2026-09-28** (release, Linux 7.0.0-34, `cat /dev/urandom`):

| consumer | result |
|---|---|
| keeps nothing (`paceless-count`) | 1 767.8 MiB drained in 30 s, resident growth ≤ 952 KiB |
| keeps every chunk (`paceless-keep`) | reaches the 512 MiB cap in 8.8 s — growth equals what was kept |

**How it was misread.** Issue #26 (August 2026) measured 31 MB in 501 ms from a consumer that stored everything and read it as a property of the push-only `Receiver`. It proposed a type that only hands out up to a maximum, or a pull `read(max)`. Neither can bound anything: `loop { kept.extend(read(MAX)) }` grows exactly the same. **A memory bound can only live where the bytes are kept**, which is why `PtyOutput` carries `QUEUE_CEILING`. D8 already said it right («the bytes pile up all the same, only a little further along»). #26 closed with a doc fix and the bench pair, not a redesign.

**Two traps met while proving it.**
1. **This load never exercises the channel's ceiling.** `cat /dev/urandom` (~57 MiB/s) delivers slower than a bare `for` drains, so the channel sits at 0–1 chunks. The flat figure shows the consumer's storage is the variable. It is **not** evidence the channel is bounded — `term-core`'s bounded-channel test and the `sync_channel` are.
2. **Calling the old doc sentences false would have planted a false comment.** «leaves the ceiling without effect» (`Pty::spawn`) and «nullifies this brake» (`READS_IN_FLIGHT`) are true about the brake on the program. Only «hands the problem back to where it was» was imprecise. The investigation's first draft called them false; the challenger caught it.

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn` doc and `READS_IN_FLIGHT`. `crates/app/examples/leak.rs` — scenarios `paceless-keep` / `paceless-count` (`cargo run --release --example leak -p app -- <scenario>`). `crates/app/src/pty_output.rs` — `QUEUE_CEILING`, the storing consumer's own bound.
