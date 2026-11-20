# Implementation 162

## Brief PR #1
recorded (unattended)
- Windows users receive an installer checked for ordinary startup before release.
- Contributors get Windows workspace lint and test failures on pull requests.
- Unsupported Unix tests are reported explicitly with their reasons and exact count.

**This PR does NOT include:**
- Porting every Unix shell integration fixture to Windows.
- The remaining manual Windows keyboard validation in F2.7.
- Changes to the layout format or public APIs.

## Contracts to respect (copied verbatim from design)
No public API changes. CI job retains its current identifier where possible to preserve branch protection references.

Internal behavioral contracts remain in 03-design.md and are supplied whole to workers.

## Plan PR #1
- [x] Audit and gate individual Unix fixtures, retaining neutral tests and Windows arms; build and run whole Linux suite.
- [x] Add compiled-list omission inventory and reporter with negative fixtures; wire full Windows workspace lint/tests.
- [x] Add PID-bound installed startup, shell input witness, persistence and normal-close verification with failure controls.
- [x] Review and validate the combined change; native Windows execution is pending CI.

Point of no return: none. Out of plan: manual keyboard acceptance and broader shell porting.

## Steps
- 05f305c · Explicit PTY bridge omissions, portable queue checks retained · 1 file, +73 −0; 38 Linux tests passed.
- 7c99367 · Compiler-listed omission reporter, fixtures and native CI wiring · 3 files, +286 −7; Python fixtures passed.
- 4ac4b54 · Launch cleanup race and absent ComSpec handling · 1 file, +5 −2; portable launch contract fixtures passed.
- 14bb0a2 · Reject listing count mismatch · 2 files, +8 −1; 9 Python fixtures passed.
- d92cbc1 · Installed launch driver and oracle fixtures · 3 files, +260 −0; portable PowerShell syntax/Add-Type/layout fixtures passed. Native Windows execution pending.

## Decisions made during implementation
- Fast-forwarded approved study branch to fetched origin/main (137fd34); no work commits existed.
- Effective config is FLOW.codex.md; FLOW_CONTEXT=unattended has no context file. Other listed overlays are inactive.
- Shared rules version 0.118.0; installed plugin manifest absent, comparison unavailable.

## Launch implementation evidence
- `/tmp/ivy-pwsh/pwsh -NoProfile -File packaging/windows/test-launch-contract.ps1`: passed PowerShell parsing, C# interop compilation, valid leaf/branch and invalid-layout rejection fixtures.
- Existing installer preservation fixture remains separate from the launch child's isolated APPDATA. Child removes XDG_*, SHELL, MEASURE_*, IVY_* and panel identity overrides while preserving PATH and ComSpec.
- Shell witness is sent as keyboard input to the launched PID's native window; PowerShell witness includes actual child PID, cmd witness is paired with the observed resolver-selected executable.
- Readiness and shutdown use state waits under one deadline; failure captures logs, windows and descendant processes. Normal success requires child cleanup and a saved console.
- MSVC cross-target workspace check was attempted but ring's C build requires absent Microsoft `lib.exe`; this Linux environment cannot provide native Windows runtime proof.

## Size thermometer
- The platform audit exceeds the indicative 25-file estimate plus two-file threshold. Continue and record overrun (unattended hot-cut option 2): per-test omissions across mixed modules are required by the approved acceptance criteria and splitting would leave coverage reduced without enforcement. No coverage removed to meet an estimate.

- `host-load` audited: no Unix shell or platform-specific test fixture; `cargo clippy --locked -p host-load --all-targets --target x86_64-pc-windows-msvc -- -D warnings` passed.
- `python3 -m unittest scripts/test_windows.py -v`: 4 passed; existing installer packaging checks preserved.
- `pty_output` retains neutral queue/diagnostic tests; 14 POSIX runtime cases receive explicit ignored Windows counterparts. Native ConPTY output/EOF differs from the raw Unix PTY assumptions in these fixtures.

## Final build audit
- App and core audit complete; 379 explicit omission entries. Fourteen neutral cursor tests restored to Windows execution; neutral restoration/settings fixtures use the platform fallback shell.
- Helper cfgs follow their Unix test callers; no blanket mixed-module gates added.
- Recovery restored the two affected files before edits; AST audit retained all 661 root and 111 integrated functions and existing calls/literals/fixtures.
- New Windows mirror file registered in glossary, prose, scripts and shape manifests after real guards identified missing registrations.
- Windows app cross-compilation unavailable because ring requires the absent Microsoft C archive tool; core and host-load MSVC check/clippy passed. Native runtime evidence remains pending CI.

## Review correction results
- 5e5c5dd adds 24 individual socket-dependent core omissions, keeping pure predicates active. The combined inventory has 379 entries, including 14 pre-existing manual Linux ignores and two separately reported portable manual ignores.
- 6d961f8 makes Linux paste premises assert and makes shell prevention a real child-only environment failure control. cb70fad validates saved tree orientation, weights, nesting and leaf fields.
- 72b725e declares the wholly test-only launch module as such; the existing shape guard then excludes its test lines under its established rule, with no ceiling raised. All four source manifests include the new mirror file.
- 9c6f4e9 and 0e65d3a limit the clipboard boundary witness to Linux and format the gate; independent review confirmed the final hash.
- Whole workspace run at 5e5c5dd passed 2,690 tests, 0 failures, 16 existing ignored cases. The final one-line platform-gate correction does not change Linux compiled behavior; its named Linux test passed separately, and final formatting/lint/inventory checks are refreshed.
