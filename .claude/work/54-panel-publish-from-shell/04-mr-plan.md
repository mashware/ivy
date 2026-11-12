# Delivery plan #54

Accepted in auto mode, 12 Nov 2026. The detail is in `plan/split.md` and `plan/security-windows.md`.

## Summary
- Number of PRs: 2
- Why split: the owner decided the macOS attestation lands first, in its own PR, proved on a real Mac. The rest is one deliverable: a wire tool with no attester behind it would refuse every tool call, so it would ship half-built.

## Execution order (waves)
- **Wave 1** (start now): #1
- **Wave 2** (after #1 is merged): #2

## PRs

### #1: macOS process attestation
- **Wave:** 1. **Depends on:** nothing. It can start immediately.
- **Includes:**
  - **term-core:**
    - Hoist the ancestry walk and `arguments.rs` into `foreground.rs`.
    - Add `lifetime_ended(&OwnedFd)`. It fails closed: a `poll` or `kevent` error counts as ended, as `foreground/linux.rs:190` and `peer.rs:43-44` do today.
    - Use a neutral `native::birth` in the tests.
    - Add `foreground/macos.rs`: `snapshot` (`proc_pidinfo` with uniqueid), `getsid`, `proc_pidpath`, `tcgetpgrp`, and a `kqueue` lifetime. Run the snapshot before and after.
    - On every other OS, the platform functions stay `Err(Unsupported)` and never return defaults.
  - **panel-control:** a macOS branch of `Peer`. It reads `LOCAL_PEERTOKEN`, requires `pidversion == p_idversion`, and uses `p_uniqueid` as `birth`.
  - **docs:** amend D84 with the macOS backend and its checks against PID reuse.
- **Why it is its own PR:** the owner's decision of 12 Nov 2026. The halves are also asymmetric: with #1 first, main never carries a command that refuses on every Mac.
- **Standalone-mergeable:** yes. These public APIs answer `Unsupported` off Linux today, and the backend replaces that branch. There is no new caller, no wire change, and nothing in the app calls it on macOS yet. Linux behaviour is unchanged, as the existing suite proves.
- **Proof:**
  - `cargo check --target aarch64-apple-darwin`.
  - On the owner's Mac: `cargo test -p term-core foreground` and `cargo test -p panel-control session`.
  - Until those pass on the Mac, the PR says macOS is unverified (D53, D54).
- **Unlocks:** #2 working on macOS (AC9).
- **Risk if it stays alone on main:** very low. The code is unused on macOS until #2.
- **Estimated size:** M · **`lines_est`:** ~700 · **`files_est`:** ~6.

### #2: hook publishes over its holder's panel
- **Wave:** 2. **Depends on:** #1.
- **Includes:**
  - The wire tool `panel_publish` and `ClientRoot`.
  - `Services.attest`, and the per-pane hold cell, written only by the window.
  - Peer capture, gated on «session handler or attester».
  - `ivy-session publish`.
  - An identity-only mode for `verify::root` (`check_mode` off).
  - In `programs.rs`, Mach-O recognition and the darwin Codex targets. Only `verify` and `programs` are widened to macOS.
  - The `Attester` in `opening.rs`.
  - `What::Overlay`, with the re-check at apply time and the confession on the row.
  - `· hook` on the identity row.
  - `did.overlaid` and `did.overlays_refused`.
  - The new decision, plus the amendments to D43, D47, D50 and D80.
  - `docs/04-panel-contract.md`, and the flow line in `docs/08-configuration.md`.
  - Tests for AC1–AC8, and the four mutations.
- **Guards from the security pass** (`plan/security-windows.md`):
  - **Attest the publisher first, then read the hold cell.** An unattested publisher gets the exit-1 refusal, never the exit-0 "nobody holds".
  - **The helper keys its exit 0 on a distinct reply code**, `"code":"unheld"`, never on the error text.
  - **Exit 0 comes only from a missing identity or from a platform where no endpoint binds.** A failed connect with a whole identity is exit 2.
  - **The cell writer and the re-check at apply time land together** (both are in this PR). The Overlay arm never takes a hold.
  - **The session callback (`worker`, `protocol`, the `callback` arm) stays Linux-only.** A test pins that the callback answers `unsupported` on macOS, so session registration and relaunch do not turn on there (F4).
- **Why it is its own PR:** it closes #54.
- **Standalone-mergeable:** yes. The tool is additive, the existing verbs are untouched, and outside Ivy the helper exits 0 silently (AC4).
- **Unlocks:** nothing further; the ticket is done.
- **Risk if it stays alone on main:** none; this is the finished ticket. Watch the module ceilings in `shape.rs` on `driving.rs` and `listener.rs`: any module carved out to satisfy them stays in this PR.
- **Estimated size:** L · **`lines_est`:** ~1 500 · **`files_est`:** ~16.

## Dependencies between PRs
- #2 depends on #1 for macOS (AC9), and on the owner's decision about order. On Linux, #2 needs nothing from #1.

## Merge test
- **Hoisting the term-core walk:** merged into #1. It is scaffolding for the macOS backend.
- **The panel-control and app halves of the command:** merged into #2. Alone, panel-control would ship a tool with no attester, which is a half-built product, and would open a window where an Overlay arm runs with no root re-check.
- **The docs:** each decision goes into the PR of the code it governs. The D84 amendment is in #1; the rest is in #2.
- **`programs.rs` and the `verify` gates:** in #2, the first PR that calls them on macOS. Widening them in #1 would leave dead code.
- **#1 → #2 kept apart.** Merging them would lose the owner's decision that the attestation is proved on a Mac before the command lands on top of it.

## Plan risks
- **Online migrations:** none.
- **Compatibility with deployed clients:**
  - The new wire tool is additive.
  - A `panel-mcp` from an older Ivy is unaffected.
  - flow outside Ivy is unchanged.
- **New domain events:** none.
- **Feature flags:** none. The command is only used when a person writes the flow line.
- **CI:** clippy `-D warnings` runs on Linux only (`ivy-prerelease.yml:31`), so a macOS warning would pass unseen. The owner's Mac check covers it.

## Decision: one or several PRs?
Two:
- **#1** is the owner's own-PR macOS attestation.
- **#2** is the indivisible command: tool, attester, overlay and docs.

A third split along panel-control and app was discarded: it would leave main with a tool that refuses everything.
