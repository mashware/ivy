# Validation 164

Revision: 4ed9487a44b152a773109cf83a7a55b3c0417b47. Native execution is assigned to Windows CI by the owner comment of 2026-11-20; a Linux cross-check is never native proof. This is a draft handoff.

## Criteria coverage
| Criterion | Proof / asserted tests | Evidence and location | Status |
|---|---|---|---|
| AC1 bind, first-instance duplicate refusal, pane isolation | binding_is_what_binds_says; first_instance_refuses_a_duplicate_and_pipe_dacl_is_exact; pane_ids_bind_distinct_pipe_names; connect-before-accept case | windows-2025 full contract/control job | not-verified-unattended: native CI pending |
| AC2 protected exact current-user/SYSTEM DACL, distinct-user deny, remote reject | Actual pipe DACL semantic tuple inspection; production PIPE_REJECT_REMOTE_CLIENTS; second-user and network behavior need the named owner trials | Native test job + F1 windows-manual-proof.md | not-verified-unattended: CI and human behavior proof pending; no AccessCheck substitute claimed |
| AC3 private directory at birth, inherited owner/DACL/type/reparse refusal | fresh_panel_directory_has_the_required_owner_and_protected_acl; null_and_broad_dacls_are_rejected; otherwise_exact_but_unprotected_directory_dacl_is_rejected; inherited_reparse_point_is_rejected_even_with_terminal_dot_or_separator; inherited_file_and_missing_path_are_rejected | Native contract tests; GNU/MSVC cross-checks only assert compilation here | not-verified-unattended: native CI pending; foreign-owner fixture not exercised here |
| AC4 nonzero expected PID, match before bytes, malformed fail closed | portable strict PID parser; windows_client_rejects_wrong_server_before_sending_request_bytes; windows_endpoint_connect_uses_inherited_server_pid; windows_mcp_instructions_require_expected_pid_and_panel_set_round_trips | Portable Linux test + native integration job | parser proven-by-test; native behavior not-verified-unattended |
| AC5 bounded stop/cancel/drain/join, release reliability, no worker callbacks | Pending accept/read/write cancellation tests; held_snapshot_returns_busy_and_drop_joins_idle_worker; full_inbox_keeps_close_pending_until_delivery_completes; late split completion; registration barrier; panic/spawn failure; connect/drop race; revoked admission regression | Native endpoint/lifecycle suites, readiness witnesses and exact release state; bound derives from production constants | not-verified-unattended: native CI pending |
| AC6 MCP initialization/set and intended-pane painting | Native MCP child round trip; app native wake test; concrete committed-document/plan tests; real desktop observation F2 | windows-2025 + owner two-pane resting/minimized observation | not-verified-unattended: CI + human painting proof pending |
| AC7 Unix regressions, formatting/lint/docs, no Windows session authorization implied | Linux full workspace; packaging and session recovery; existing macOS CI continues; Windows app/source test cross-check | Local evidence logs + existing macOS/native CI | Local results below; macOS/native runs not-verified-unattended |

Owner-authorized proof locations are explicit; none of these native statuses is silently called proven. Remaining owner steps are published in full in the draft body.

## Test coverage
- Added 31 native unit tests: directory 5, endpoint 7, concrete services 6, lifecycle 8, app 5.
- Added 3 native integration tests: explicit wrong PID before bytes, inherited public connector authentication, native MCP handshake/set.
- Portable strict expected-PID parsing is tested. Existing Unix fixtures and their assertions remain active on Unix; Windows callback-based constructors are intentionally unavailable, so their app fixture consumers are Unix-only. Windows executes dedicated concrete-service tests.

## Suite results
`cargo test --locked --workspace`: passed, 2691 passed, 0 failed, 16 ignored across 34 result groups (unit, integration and doc tests). Formatting, Linux workspace Clippy, documentation, GNU app test cross-check and MSVC contract/control test cross-check passed. Documentation emits private-link warnings; Windows check emits platform dead-code/unused warnings; neither is a failed command. Cross-compilation is not runtime evidence.

Initial validation found and repaired, without exemptions:
- Deferred delivery What was overwritten before it reached the frame; restored queued request.
- Split arrival test used a 200ms scheduling assumption; explicit arrival barrier preserves its assertions (own commit 22d9919).
- Unix guidance changed a tested identity sentence; retained platform-specific wording.
- Shell-source guard lacked three new module entries; added exact include_str rows.
- Glossary external names lacked PANEL_SERVER_PID; added beside PANEL_ID/PANEL_DIR.
- App module/type ceilings: moved native bridge and document policy to owning free functions, moved wake test to a recognized end-of-file test module. PanelState's measured ratchet drops 356→353. No ceiling raised or assertion removed.
- Wait declarations: six native waits have an explicit reason; the equality is measured 21, and platform test-only modules use the existing scanner-supported attribute form without changing their Rust condition.
- Windows legacy callback fixture gates: GNU compilation and independent review caught and fixed transitive/misplaced test cfgs.

## Performance
No bench_cmd configured; no timings or improvement claimed. No data queries, schemas, jobs or external API loops were added. Workers clone bounded snapshots, try_lock instead of waiting for UI-owned data, have at most 8 connections and one pending release token per connection; no arbitrary worker callback is accepted. Cancellation and full-inbox bounds are asserted in native tests awaiting CI, not inferred as measured latency here.

## Edge cases / open proof
- [x] New operations and platform-only tests compile for Windows GNU; contract/control compile for MSVC.
- [x] No schema or dependency-on-another-ticket gate; #161 is on base.
- [ ] F1 — owner observes distinct-user denial and remote rejection on a network-capable Windows host. Steps in draft body.
- [ ] F2 — owner observes actual intended-pane painting/resting/minimized behavior with two panes. Steps in draft body.
Native execution unavailable on this Linux host; no Windows desktop, alternate-user token or authenticated SMB host can be driven here. This limitation is the owner's approved draft proof location, not a request for an executor.

## Regressions
Full Linux command passed on the final reviewed SHA; no unresolved local regression remains. CI must pass native Windows and existing macOS jobs before readiness. Human F1/F2 remain pending and are not reported as passing.

## Final commands and evidence

`cargo test --locked --workspace`: passed, 2691 passed, 0 failed, 16 ignored across 34 result groups (unit, integration and doc tests).

- `cargo fmt --all --check`: exit 0.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0.
- `cargo doc-check --locked`: exit 0 (private-link warnings).
- `cargo check --locked -p app --tests --target x86_64-pc-windows-gnu`: exit 0 (platform warnings).
- `cargo check --locked -p panel-contract -p panel-control --tests --target x86_64-pc-windows-msvc`: exit 0.
- `python3 -m unittest scripts/test_release.py scripts/test_macos.py scripts/test_windows.py -v`: 23 passed.
- Session-recovery Node test suite (`integrations/session-recovery/tests/*.test.js`): 19 passed; captured TAP output is in evidence.
- Focused source guard repair check: 32 passed; full workspace independently reran these guards green.

Evidence: evidence/linux-workspace-final.txt, evidence/linux-quality-final.txt, evidence/windows-cross-checks.txt, evidence/packaging-suite.txt, evidence/session-recovery-js.txt. No visual before/after exists on this Linux host; owner captures it on the Windows revision above.
