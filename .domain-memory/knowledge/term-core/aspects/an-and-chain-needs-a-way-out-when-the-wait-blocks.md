---
id: asp_an-and-chain-needs-a-way-out-when-the-wait-blocks_72aq0w
slug: an-and-chain-needs-a-way-out-when-the-wait-blocks
name: >-
  Swapping `;` for `&&` after `stty` hangs any caller that waits for the marker
  with a blocking read — the chain needs a failure marker too
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-28T21:23:09.599Z'
updated_at: '2026-09-28T21:23:09.599Z'
last_verified: '2026-09-28T21:23:09.599Z'
file_paths:
  - crates/term-core/examples/termios_probe.rs
  - crates/app/tests/scripts.rs
symbols: []
content_hashes:
  crates/term-core/examples/termios_probe.rs: 'sha256:aee8349c466f66c378fc5bda69acf49e2642173f780670c36b31af6590a15d49'
  crates/app/tests/scripts.rs: 'sha256:f9ea30e1c87ca5f0c0d7efca283359b0fdabc552d0e10125daca9a1d986a9b82'
tags:
  - stty
  - scripts
  - pty
feature_id: feat_term-core_nx0g1r
---

## What it does

**The `&&` fix of [[the-echo-forges-the-evidence-a-test-reads]] trades a lie for a hang wherever the caller waits with a blocking read.** Found in the review of #149, September 2026, on `term-core/examples/termios_probe.rs`.

After the swap to `stty raw -echo && printf ready; exec cat`, a failed `stty` means nothing is ever printed. `exec cat` then never writes, and the probe's `reader.read` blocks for ever. Its 5 s deadline is checked only **between** reads, so it never gets a turn. Before the swap the probe reported wrong flags. After it, the probe hangs and prints nothing.

**The shape that works:** `stty … && printf ready || printf failed; exec cat`, with the wait loop leaving on either word. The scripts guard (`no_script_can_print_its_marker_when_the_mode_command_failed`) accepts it, because the ready marker is still reached only through `&&`.

> Wherever a test or example waits for a marker with a read that has no timeout of its own, give the failure branch a marker of its own. The deadline in the loop condition only helps if the read returns.

## Where it lives

`probe` and `main` in `crates/term-core/examples/termios_probe.rs`. The rule that judges the chain is in `crates/app/tests/scripts.rs`.
