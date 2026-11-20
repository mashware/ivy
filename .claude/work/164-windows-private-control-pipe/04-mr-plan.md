# Delivery plan 164

## Summary
- Number of PRs: 1.
- Wave 1: #1 (after study approval).
- One atomic security boundary: usable pipe creation, ACL checks, server authentication, discovery and lifetime cannot be enabled separately safely.
- Plan advisor and independent security-exposure audit: approaches/planning-advice.md. Accepted as delivery mechanics (unattended).

## Execution order (waves)
Wave 1: #1. No later wave or stacking needed.

## PRs
### #1: Enable the private Windows control channel
- **Wave**: 1 — **depends on**: nothing; #161 is already on main at 8380c4e.
- **Includes**: Windows private directory creation and inherited ACL/owner validation; PANEL_SERVER_PID contract and forwarding; platform transport facade; native overlapped pipe bind/connect/read/write; client server-PID checks; first-instance name ownership and remote rejection; stop/cancel/drain and tracked Windows workers; BINDS and MCP instructions; native Windows test job and existing Unix regressions; intended-pane rendering evidence.
- **Why its own PR**: the sole coherent delivery of ticket 164; no enabling-only slice earns another merge.
- **Standalone-mergeable**: yes, native Windows capability arrives with its complete security and lifetime tests, while Unix delegates to its current transport.
- **Unlocks**: MCP panels and shared channel operations on Windows.
- **Risk if it stays alone**: native resource/cancellation bugs or incomplete PID forwarding; all must be covered by Windows tests before publication.
- **Estimated size**: M (substantial native platform implementation).
- **lines_est**: ~1600 added+modified lines, including tests.
- **files_est**: ~25; estimate includes transport imports/test helpers, environment forwarders and CI.

## Dependencies between PRs
None. No outstanding ticket dependency. Verified `git merge-base --is-ancestor 8380c4e origin/main` succeeds on fetched base 137fd348165866dcd3382608a811d0c594119f42.

## Merge test
Not applicable to a one-PR final plan. Candidate cuts evaluated:
- Native bind vs ACL/PID: combine; raw bind without its checks is unsafe, disabled scaffolding alone serves no user.
- Directory/environment vs endpoint: combine; the new client checks must accept the directory and expected PID supplied by the app.
- Transport vs discovery: combine; advertising before binding gives unusable instructions, binding without discovery leaves agents without tools.
- Endpoint vs lifecycle/tests: combine; bounded teardown and native security proof are acceptance requirements of the endpoint itself.
No exposure window between merges because there is one merge.

## Plan risks
- No migrations, new wire events or temporary feature flags.
- Existing Unix environment clients retain their two-variable behavior; Windows forwarding must carry PANEL_SERVER_PID everywhere.
- A Linux check or Windows cross-compilation cannot prove DACLs or overlapped completion; native Windows tests are mandatory.
- GUI painting requires native observation/evidence. If unavailable, mark manual proof honestly under validation; do not claim a screenshot or success.
- Fixed-size environment arrays, session peer helpers and tests may make the file estimate grow. Record growth; do not compress code to meet it.

## Decision: one or several PRs?
One. Splitting provides scaffolding without usable independent value and separates defenses from the endpoint they protect. No extra runtime, daemon, unrelated Windows porting or native session-authorization work.
