# Validation 162

## Summary
Corrected candidate: `0e65d3a37a81561bbc8715ed96271d7cdf59a4a9`.
Local checks pass. Native Windows execution is **not verified unattended**: this runner is Linux, with no native Windows desktop or installer environment. No native success is inferred from compilation or script inspection.

## Suite results and provenance
- `cargo test --locked --workspace` at `5e5c5dd9081bcce94370507fa85b61e4a05231df`: exit 0; 2,690 passed, 0 failed, 16 existing ignored cases across 33 suite summaries. Full output: `tests-final-reviewed.log`.
- The only subsequent semantic delta is a test attribute from `cfg(unix)` to `cfg(target_os = "linux")`; Linux compilation selects the same test body. Its named test passed at `9c6f4e9` (1 passed, 0 failed); final `0e65d3a` differs only in indentation. Output: `boundary-final.log`. No whole-suite result is attributed to a revision it did not execute.
- Final `cargo fmt --all --check`: exit 0 (`fmt-published.log`).
- Final `cargo clippy --locked --workspace --all-targets -- -D warnings`: exit 0 (`clippy-published.log`).
- Final compiler-backed `python3 scripts/windows-test-report.py --platform linux`: exit 0; 379 inventory entries, 28 compiled harnesses, two separately reported existing manual latency ignores (`omissions-published.log`). The 14 pre-existing Linux manual ignores retain their exact original reasons.
- `python3 -m unittest scripts/test_release.py scripts/test_macos.py scripts/test_windows.py scripts/test_windows_test_report.py -v`: 38 passed (`python-validated.log`).
- `pwsh -NoProfile -File packaging/windows/test-launch-contract.ps1`: exit 0; parser, C# interop compilation, saved-tree rejection/acceptance, clean child environment and actual shell-prevention environment fixtures (`launch-contract-reviewed.log`). This is portable contract coverage, not Win32 execution.
- `cargo doc-check --locked`: exit 0; 21 existing private-link warnings, no broken-link failure (`docs-validated.log`).
- `cargo clippy --locked -p term-core -p panel-control -p panel-contract -p host-load --all-targets --target x86_64-pc-windows-msvc -- -D warnings`: exit 0 (`windows-subset-validated.log`). The full Windows workspace cross-check was attempted; ring needs an unavailable Microsoft C archive tool (`lib.exe`), recorded in `windows-check.log`. No whole-app Windows compilation claim.

All Cargo commands used `/usr/local/cargo/bin` on PATH. Test commands were auto-discovered from the Rust workspace and existing CI because quality.test/static_analysis are unset. No red-base or flaky-test exception was used, and no failure was dismissed as outside this change.

## Criteria coverage
| Criterion | Status | Evidence and required next verification |
|---|---|---|
| AC1: full native Windows lint/tests fail verification on nonzero exit | not-verified-unattended | CI runs both exact workspace commands on PR and workflow dispatch, with explicit exit checks. Require `windows-startup-checks` green on the draft. |
| AC2: Windows arms and named startup/path witnesses execute | not-verified-unattended | Core/host-load MSVC lint passed. Workflow checks all three startup and six path witness names for `ok`; require the full native app suite and these witnesses in CI. |
| AC3: exact visible omissions, no unsupported silent successes, neutral tests retained | not-verified-unattended | Fifteen reporter fixtures and Linux compiler validation cover missing/extra/duplicate names, counts, reasons, baseline ignores and real counterparts; Linux paste witnesses assert their premise, and portable cursor/restoration tests remain enabled. Native Windows must confirm exactly 379 declared ignored counterparts plus two existing manual ignores. |
| AC4: clean installed startup, actual default console input, normal exit 0, valid saved layout | not-verified-unattended | Portable contract fixtures verify environment cleaning and saved-tree oracle. Native installer smoke must observe the installed PID-owned window, actual shell PID/input witness, normal close, descendant cleanup and nonempty saved layout. |
| AC5: crash/prevent-shell/save/deadline controls reject and installer lifecycle survives | not-verified-unattended | Shell-prevention environment fixture passes; negative controls are wired before the normal run, with expected failure-stage checks. Native CI must execute all four controls and payload/replacement/uninstall/data-retention checks. |

There is no missing test implementation classified as unproven. Every remaining criterion has an executable native CI check, but this environment cannot execute it. These statuses permit only a draft publication under the unattended validation rule.

## Performance and data access
Performance review found repeated process snapshots at 50 ms; the driver now uses 250 ms polling and deadline checks around discovery/shutdown queries. Cleanup and output collection remain bounded. No database queries/schema changes; no unresolved query verdicts. No bench_cmd is configured; no timing or memory improvement is claimed.

## Edge cases and regressions
- [x] Missing/extra/duplicate compiler-listed omissions, mismatched ignored reasons/counts and unrelated manual ignores rejected by reporter fixtures.
- [x] Linux held-paste tests cannot pass by returning before assertions; the corrected clipboard test passes and its Linux gate preserves macOS behavior.
- [x] Invalid saved leaves, absent orientation/weights, zero weights, same-direction nesting and isolated shell-control environment checked by portable PowerShell fixtures.
- [x] New mirror source registered in glossary/prose/scripts/shape manifests. All repository guards pass in the whole suite; the launch file is explicitly test-only, without raising a production ceiling.
- [ ] F1: native Windows desktop, ConPTY, installed launch controls and lifecycle — require both Windows CI jobs green and inspect `windows-launch-evidence` before merge. No native Windows runtime is available here.

## Next step
Hand the draft to the orchestrator. F1 and AC1–AC5 remain explicit merge prerequisites in the publication body.
