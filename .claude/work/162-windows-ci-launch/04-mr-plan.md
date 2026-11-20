# Delivery plan 162

## Summary
- Number of PRs: 1; size M.
- One complete Windows verification slice. Platform audit, native suite and installed launch jointly prove the ticket; splitting creates another review/CI wave without a required deployment boundary.
- Plan proposal: plan-proposal.md, independently prepared. Accepted as flow mechanics (unattended); study approval remains external.

## Execution order (waves)
Wave 1: #1. No further wave.

## PRs
### #1: Verify the Windows workspace and installed application launch
- Wave: 1; depends_on: [] (no unmet ticket dependency).
- Includes: per-test platform audit, native fixture/compile fixes, runtime-visible omission reporting, complete Windows clippy/tests, PR/manual installer launch smoke, preservation of installer lifecycle checks.
- Standalone-mergeable: yes; one coherent change, no temporary dead code or migration. Existing Linux verification remains required.
- Why its own PR: single remaining delivery, exempt from split qualification.
- Unlocks: Windows regressions fail before publishing an installer.
- Risk if alone: remaining F2.7 manual keyboard/platform validation is still outside this ticket, not hidden by a completed automated check.
- Estimated size: M; lines_est: 600; files_est: 25 (soft upper end of advisor's 350–600 lines / 15–25 files). Platform inventory can increase this; never compress code or coverage to fit.

## Dependencies between PRs
None. #161 already merged at 8380c4e; base/main and origin/main are 0429d9ea71534b88703625be6e90563207562a7b after fetch.

## Merge test
Evaluated pair A (test portability + workspace CI) / B (installed launch). Merging loses no deployment separation or independent enabling work; retain as #1. Evaluated platform gates alone / workspace enforcement: gates alone reduce test coverage without proving Windows execution, so merge. Evaluated installed driver / installer CI wiring: an unexecuted driver delivers no regression gate, so merge. No pair survives as a separate wave.

## Checklist for implementation
- [ ] Inventory each shell/platform test, Windows branch, example target, and unsupported early return; keep neutral coverage.
- [ ] Implement platform fixtures/gates, compiler-checked per-test omission inventory and pinned libtest reporter; require exact qualified-name membership on Windows and real Linux counterparts; print reasons and exact total including zero.
- [ ] Prove reporter fixtures reject missing, extra and duplicate entries; report unrelated ignores separately.
- [ ] Run Windows full workspace clippy/tests and existing Windows witness-name checks on PR/manual runs.
- [ ] Install and launch the candidate with clean profile; observe PID-targeted native window, resolver-bound actual shell witness with inherited PATH/ComSpec, normal shutdown, valid saved layout, exit 0.
- [ ] Prove smoke controls fail for crash, missing shell, missing layout and deadline; preserve replacement/removal/preferences.
- [ ] Validate Linux whole suite and actual Windows execution, review, publish.

## Validation contract
Open 03-design.md acceptance criteria and contracts before implementation. Windows outcomes are unverified in this study. If CI fails, inspect ai events --json and diagnose/repair the mechanism, including fragile tests outside the original diff. CI logs never authorize bypassing gates. No code, tests or CI jobs ran during this study.

## Cost
Planning: 1/12 agent tasks; monetary cost not reported by harness. Design: two approach advisors, one architecture advisor and one independent challenge (4/12 tasks, three reused agent contexts); configured workers/agents model from FLOW.codex.md.

## Final acceptance
One PR retained after applying the design-challenge answer. Size M remains appropriate for cross-crate platform fixtures, compiler-checked omission reporting and installed GUI verification. Plan accepted (recorded unattended); no additional planning fan-out: the existing independent plan-proposal.md remains applicable, with the approved inventory mechanism included. No schema, cross-repository or new ticket dependencies.
