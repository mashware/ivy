# Delivery plan #56

Accepted in `auto` (2026-11-12). Proposal and its reasoning: `plan-proposal.md`.

## Summary
- Number of PRs: 2
- Split justification: five fixes for behaviour wrong today (items 2–6) must not wait behind the new outside label (item 1), which is the largest review and needs a proof on a real Mac. Neither needs the other.

## Execution order (waves)
- **Wave 1** (start now, in parallel): #1, #2

Suggested order inside the wave: #1 first (smaller; #2 then rebases a few lines in `driving.rs` and D88).

## PRs

### #1: The hold ends on done, and one rule per identity reader
- **Wave**: 1 — **depends on**: nothing — can start immediately.
- **Includes**:
  - Item 3: `Done(C)` hold state in `crates/app/src/panel/driving.rs` — `done` (publish or patch) gives the panel back to the file; the holder re-claims with its next publish and can still retire; `patch` only on the holder's own document; the held-only warning and the backed-claim rule become «held and not done».
  - Item 2: `endpoint::socket_present` gates `instructions_for` in `panel-mcp`.
  - Item 4: the session helper (`session/helper.rs`, `bin/ivy-session.rs`) uses `client::whole_identity`.
  - Item 5: `panel_contract::base_dir` made public and used for the shell-integration rc dir.
  - Item 6: the Connect guide asks for the manual publishing prompt only where server instructions did not arrive; prompt and README carry the subagent rule.
  - D43 amended; D88 amended (stat, helper, rc dir).
  - Tests for AC5–AC12.
- **Why it is its own PR**: criterion 1 — repairs things broken today (a file producer refused until the bridge exits, a closed console promised a panel, a helper with its own identity rule, a relative rc dir, a misleading guide prompt); bundling would hold them behind the macOS port.
- **Standalone-mergeable**: yes — each item is a self-contained fix, no new contract, no dead code.
- **Unlocks**: nothing required by #2.
- **Risk if it stays alone on main**: none.
- **Estimated size**: M.
- **`lines_est`**: ~550 lines (added + modified, tests included).
- **`files_est`**: ~14 files.

### #2: Label a publication made from outside the console (Linux and macOS)
- **Wave**: 1 — **depends on**: nothing — can start immediately.
- **Includes**:
  - term-core: `ForegroundScope::descends(pid, birth)`; `foreground/macos.rs` with its own scope fields (`proc_pidinfo` `PROC_PIDTBSDINFO`).
  - panel-control: `session/peer` split into `peer/linux.rs` and `peer/macos.rs` (`LOCAL_PEERPID`, never via `is_alive`); peer captured for every connection; `Services.origin`; verdict once on the connection thread; `What::Publish.outside`.
  - app: the label's scope cell (shape of `resume::Route`, in `control/channel.rs`/`resume.rs`, never `lib.rs`); `PanelState.published_by`; the label in `warning()`.
  - D7 amended; D88 amended (label).
  - Tests for AC1–AC4 on Linux; AC1/AC2 manual on a real Mac.
- **Why it is its own PR**: the remaining entry if nothing had been split (the feature #1 must not wait on).
- **Standalone-mergeable**: yes — every new piece has its caller in the PR; other targets answer «cannot tell», which paints no label.
- **Unlocks**: —
- **Risk if it stays alone on main**: none — nothing is gated on the label.
- **Estimated size**: M.
- **`lines_est`**: ~900 lines.
- **`files_est`**: ~16 files.
- **Shape guard (D28)**: check `crates/panel-control/src/listener.rs`'s production line count before building; over 600 → the verdict goes into a module of its own inside this PR.

## Dependencies between PRs
- None. Both touch `driving.rs` (`Holder` in #1, `published_by` in #2) and different paragraphs of D88: a textual rebase, not a dependency.

## Merge test
- #1 + #2 (same wave): kept apart — merging puts the five fixes behind the largest review and a manual step on another machine; keeping apart costs one small rebase.
- Inside #2, lower layers vs app: merged — the lower layers' only consumer is the app; alone they are dead code on main.
- Inside #2, Linux vs macOS: merged — one user decision; Linux first would leave the Mac DMG silently on «cannot tell» (a half-built feature, not a safety gain). Revisit only if the Mac proof becomes the bottleneck.
- Inside #1, items 2–6: merged — none closes a ticket of its own and none depends on another; splitting only adds review rounds.

## Plan risks
- Online migrations: none.
- Compatibility with deployed clients: the `initialize` reply drops `instructions` also when the socket is absent — clients already handle its absence.
- New domain events: none.
- Introduced feature flags: none.

## Decision: one or several PRs?
Two: fixes to today's behaviour ship without waiting for the new feature and its real-Mac proof; everything else merged per the merge test.
