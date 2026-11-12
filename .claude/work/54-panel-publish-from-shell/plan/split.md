# Split #54 — two PRs, two waves

| n | wave | depends_on | includes | standalone-mergeable | unlocks | risk if alone on main | lines_est / files_est |
|---|------|------------|----------|----------------------|---------|-----------------------|-----------------------|
| 1 | 1 | — | **macOS process attestation.** term-core: hoist the ancestry walk and `arguments.rs` into `foreground.rs`, `lifetime_ended(&OwnedFd)`, neutral `native::birth` in tests, new `foreground/macos.rs` (`snapshot`, `executable`, `group`, `kqueue` lifetime). panel-control: macOS branch of `Peer` (`LOCAL_PEERTOKEN`, `pidversion == p_idversion`, `p_uniqueid` as birth). D84 amendment for the macOS backend and its PID-reuse checks | **yes.** Both are public library APIs that today answer `Unsupported` off Linux (`foreground.rs:50-81`, `peer.rs:46,82`), so the backend replaces a live branch: no dead code, no new caller, no wire change. Linux unchanged, proved by the existing suite. `cargo check --target aarch64-apple-darwin`, then `cargo test -p term-core foreground` and `-p panel-control session` on the owner's Mac | #2 working on macOS (AC9) | Very low. No app code calls it on macOS yet (session recovery stays Linux-only, F4). The PR must not claim it works on macOS until the Mac run (D53, D54) | ~700 / 6 |
| 2 | 2 | 1 | **The command, end to end.** wire `panel_publish`; `ClientRoot`; `Services.attest`; the per-pane hold cell written only by the window; peer capture gated on «session handler or attester»; `ivy-session publish`; `verify::root` identity-only mode, plus the `programs.rs` Mach-O and darwin Codex targets with the `verify`/`programs` gates widened; the `Attester` in `opening.rs`; `What::Overlay` with the apply-time re-check and confession; `· hook` on the row; `did.overlaid`/`overlays_refused`; the new decision plus the D43/D47/D50/D80 amendments, `04-panel-contract.md` and the flow line in `08-configuration.md`. Tests AC1–AC8 and the four mutations | **yes.** It closes #54. Outside Ivy the helper exits 0 silently (AC4). Existing verbs are untouched, and the new tool is additive | Closes #54 | None. This is the finished ticket | ~1 500 / 16 |

**Criteria.** #1 stands on the **owner's decision** (12 Nov 2026: the macOS attestation goes in its own PR, before the command). The split is also asymmetric (criterion 4): with #1 first, main never carries a command that refuses on every Mac. #2 claims **criterion 2**: it closes #54.

**Dependency check.**
- On Linux, #2 needs nothing from #1. Its dependency is the owner's order, plus AC9.
- `programs.rs` and the `verify` gates move to #2 on purpose. `ai_sessions` is a private app module with no macOS caller until the `Attester` exists, so widening its gates in #1 would leave dead code on macOS.
- Clippy `-D warnings` runs only on Linux (`ivy-prerelease.yml:31`), so a macOS warning would pass CI unseen.

## Merge test

**Merged:**
- **The term-core hoist into #1.** It is scaffolding for the macOS backend, and the Linux path does not need it.
- **The panel-control half and the app half of #2 into one PR.** Alone, panel-control would ship a wire tool and a helper with no attester (`Services.attest: None`): flow's line would get a refusal on every tool call, which is a visibly half-built product. The panel-control half is scaffolding for the app half.
- **The docs into #2, and the D84 macOS amendment into #1.** The rule is that a decision goes in the PR of the code it governs.
- **`programs.rs` and the `verify` gates into #2.** #2 is the first PR that calls them on macOS, as explained above.

**Kept apart:**
- **#1 → #2,** even though #2 is #1's only dependent. Merging them would lose the owner's decision: the attestation is proved on the Mac before the command lands on top of it. That decision stands, whatever the merge test says.

No wave holds two PRs, so there are no same-wave pairs to test.

**Inside #2 (not a split):**
- Watch the 600-line and 300-line-`impl` guard (`shape.rs`) on `driving.rs` and `listener.rs`. Any module carved out to satisfy it stays in this PR.
- Not measured by any bench: F1, the frame cost of a hook publication.
