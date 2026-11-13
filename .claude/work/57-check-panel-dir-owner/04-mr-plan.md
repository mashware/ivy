# Delivery plan 57

## Summary
- Number of PRs: 1.
- One complete trust-boundary fix spans Rust and JS; splitting would leave a known inherited consumer unprotected and add a delivery wave without independent value.

## Execution order (waves)
Wave 1: #1 — start after study approval. No dependency or later wave.

## PRs
### #1: Reject unsafe inherited panel directories
- **Wave**: 1 — **depends on**: nothing; can start after study approval.
- **Includes**: shared Unix private-directory guard; bridge/helper call sites and diagnostic preservation; direct OpenCode check on every connect; caller-level no-connect and positive-control tests; symlink terminal slash/dot handling and regression tests; D7, architecture security table and endpoint explanation updates.
- **Why its own PR**: sole unsplit deliverable, closing #57; no scaffolding-only slice.
- **Standalone-mergeable**: yes; existing PanelDir, frames and response shapes remain compatible; complete invariant supplied in both languages.
- **Unlocks**: complete fix for #57, no dependent slice.
- **Risk if alone on main**: complete implementation; existing installed client/plugin versions still lack the guard until upgraded.
- **Estimated size**: M.
- **lines_est**: ~350 (soft range 250–450 added/modified; not a target).
- **files_est**: ~11 (soft range 9–13, not a limit).

### Plan #1
- [x] Implement checker and permission/type/uid/spelling/error matrix.
- [x] Guard both Rust inherited entry points; preserve binary errors; prove no connection and genuine-client success.
- [x] Guard direct JS plugin on every attempt; verify no-connect/recheck/repair/disposal.
- [x] Update D7, architecture table and endpoint explanation.
- [x] Review contracts and run focused/full validation checks named in the design.

## Dependencies between PRs
None. No ticket dependencies. Existing base supplies PanelDir, inherited identity logic and all target consumers.

## Merge test
- Candidate Rust/JS pair: merged into #1; separation leaves one language exposed and adds partial-security rollout without independent ticket value. A combined review loses only smaller language-specific diffs.
- Candidate docs/code pair: merged into #1; separation leaves inaccurate security guidance or describes an unshipped guard. No independent delivery value.

## Plan risks
- Online migrations: none.
- Compatibility: protocol unchanged; effective-uid mismatch (including sudo -E) now refuses as intended.
- Events and flags: none introduced.
- Release/handoff note: upgrade MCP, ivy-session and the direct OpenCode asset together where possible; old installed consumers retain the vulnerability. Coordinate through normal release/install paths, not a new installer feature.
- Rollback: revert complete PR; this restores the prior security exposure. Peer-uid authentication and the check/connect race remain explicitly outside this fix's guarantee.

## Decision: one PR
One atomic PR, accepted for study routing (unattended). No production code or tests changed in this run. Study approval is still required before build. Planning and security proposals both returned; no blockers. Two plan agents used, within default budget 12; model gpt-6-luna from FLOW.codex.md, general-purpose role fallback.
