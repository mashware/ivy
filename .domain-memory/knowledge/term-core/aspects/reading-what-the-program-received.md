---
id: asp_reading-what-the-program-received_je1s12
slug: reading-what-the-program-received
name: >-
  Reading what a console's program received: `term_core::received::Received`,
  and the one mode it cannot see
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T18:17:21.333Z'
updated_at: '2026-09-28T18:17:21.333Z'
last_verified: '2026-09-28T18:17:21.333Z'
file_paths:
  - crates/term-core/src/received.rs
  - crates/term-core/Cargo.toml
  - crates/app/Cargo.toml
symbols:
  - Received
  - 'Received::script'
  - 'Received::wait_for'
  - 'Received::so_far'
  - 'Received::wait_until_ready'
content_hashes:
  crates/term-core/Cargo.toml: 'sha256:110404ff37f3f44613df3eb289ae88784c482b7240091d3854ded5daab38d08b'
  crates/app/Cargo.toml: 'sha256:61879c2d8770b1d505704076b748bc1e3d0783f675e8ee80f14cb312431c8ee1'
tags:
  - testing
  - echo
  - line-discipline
  - test-disguise
feature_id: feat_term-core_nx0g1r
---

## What it does

**Any test that needs to know what the program on the far side of the pty got uses `term_core::received::Received`** (#167) instead of reading the screen or counting master bytes — both of which the echo can satisfy. The program becomes `<prelude> && touch ready && exec cat > received`; the test compares the file byte for byte. From `app` it is reached through a dev-dependency on term-core with `features = ["testing"]`.

**Why no snapshot of the mode (`stty -a`) was needed — and its limit.** A prelude that fails never marks ready (`&&`). One that exits 0 without taking is caught by the comparison only if the mode governs INPUT (canonical lines, ICRNL, signals): those change the bytes the program reads. A mode that governs ONLY the echo (`-echo`) changes nothing in the file — harmless for this probe, since the echo never reaches the file, but it means **this probe cannot prove an echo setting took**. Found by the built-in review of #167, after the module doc had claimed the opposite.

**A prelude may not carry its own `;` or newline** — refused by an assert: `stty raw; stty -echo && touch ready` chains ready to the last command only, which is #158's defect from inside, and `crates/app/tests/scripts.rs` cannot see it (its markers are `echo`/`printf`, and this script is built at runtime).

**Evidence**: mutating the empty prelude to `stty -icanon min 1` turned the echo test red with the helper holding "abc" before the line end; making `script` ignore the prelude turned both raw-mode app tests red (0 bytes: cooked mode holds a paste with no line end); dropping the separator assert turned its `should_panic` test red.

**Absence is clocked by an arrival**: write a line without its end, see the echo on the master, assert the file is empty, then write the end and wait for exactly the line — never a timer.

## Where it lives

crates/term-core/src/received.rs (module + tests); callers in crates/app/src/lib.rs tests (send-held button, discard click, panel button)
