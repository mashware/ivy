# Design 162

## What already exists (inventory)
- `.github/workflows/ivy-prerelease.yml:20-39`: Ubuntu workspace clippy/tests on pull requests.
- `:56-82`: windows-2025 targeted startup and path tests, with explicit expected-name assertions. Preserve these witnesses when broadening coverage.
- `:windows`: manual-release installer build, payload checks and upload. Reuse installer tooling and pinned action revisions.
- `packaging/windows/test-install.ps1`: disposable-runner gate, isolated install directory, payload hashes, replacement, shortcut, uninstall and XDG preservation fixture.
- `crates/app/src/startup.rs:11-37`: measurement input disables ordinary settings and forbids layout writes; measurement cannot directly satisfy the clean-profile smoke criterion.
- `crates/app/src/session.rs:554-571`: APPDATA fallback already implemented; `lib.rs:1732` saves on normal window close.
- `crates/app/src/lib.rs:1329`: all-ended consoles close the window. Shell spawn helpers and tests also live in opening.rs, pty_output.rs, resume.rs, term-core and panel-control.
- `docs/05-roadmap.md` F2.7 explicitly calls out unsupported-boundary early returns and ConPTY lifetime differences. This ticket is a slice of F2.7, not delivery of its manual keyboard validation; no roadmap row uniquely matches 162.

## Approaches considered
Two independent advisors ran (minimum and reframe; files in approaches/). Consensus: native workspace CI plus installed-artifact launch, retaining pure tests. Disagreement: a checked-in omission inventory versus runtime-visible omissions; a launch hook versus external driver.
1. Recommended: extend native checks, targeted per-test platform bodies with visible ignored Windows counterparts, and a normal installed-app launch driver. Reuses startup, persistence, installer and Windows named-test witnesses. Main risk: auditing numerous mixed test modules and GUI automation.
2. A hand-maintained omission manifest plus launch-specific app mode: simple initial wiring but counts can drift and mode may bypass persistence or default shell startup.
3. Replace every shell fixture with cmd/pwsh equivalents: broader native behavior coverage but changes Unix semantics and expands this ticket substantially. Port simple neutral fixtures where practical; retain Unix-specific behavior on Unix.
Choice recorded (unattended): option 1; avoid blanket module gates where they erase neutral or Windows coverage.

## Additional domain context
No configured knowledge source. Repository evidence above is authoritative for current paths; issue line numbers describe an older base.

## Executive summary
- Broaden windows-startup-checks into workspace lint/test verification, on pull_request and workflow_dispatch.
- Audit shell-dependent tests at test granularity; report all intentionally omitted Windows cases by name and total.
- Build and install the candidate on PRs too, then prove ordinary installed startup with a fresh roaming profile and normal shutdown.
- Keep payload, replacement, removal and preference retention checks distinct from the fresh launch profile.

## Acceptance criteria
| ID | Observable assertion | Proof |
|----|----------------------|-------|
| AC1 | windows-2025 runs `cargo clippy --locked --workspace --all-targets -- -D warnings` and `cargo test --locked --workspace`; either nonzero command fails both PR and manually dispatched verification. | test |
| AC2 | Windows-only arms in endpoint/client/metadata/splitting and existing windows_startup/windows_paths tests remain compiled and executed by the workspace suite; expected named Windows witnesses are required to pass, not merely match a zero-test filter. | test |
| AC3 | Windows output lists names and the exact total of intentionally unsupported Unix-only cases; no unsupported boundary test silently returns success without its assertions. Neutral tests remain enabled. | test |
| AC4 | From the installed `Ivy.exe`, with all XDG_* and SHELL absent and APPDATA pointing to an empty absolute directory, a native app window and working default console are observed, normal close completes with exit 0, and valid nonempty layout JSON exists at `<APPDATA>/ivy/layout.json`. | test |
| AC5 | Deliberately crashing startup, preventing shell spawn, suppressing layout saving, or exceeding the launch deadline makes the smoke fail. Installer hash/replacement/uninstall and existing user-data retention checks continue to pass. | test |

## Modules/layers affected
- `.github/workflows/ivy-prerelease.yml`: Windows PR/manual quality and installer smoke wiring; preserve release publish dependency and static CRT build settings.
- Test bodies/imports/helpers throughout app, term-core, panel-control, panel-contract: per-platform compilation, fixtures and omission visibility, including examples compiled by all-target clippy.
- `packaging/windows/test-install.ps1` plus a focused launch helper if separation improves clarity: clean child profile, native launch witness, bounded close and cleanup.
- A small reporting script under scripts only if cargo's ignored test summaries need aggregation; no source-regex test census.

## Data model
No entities, migrations or indexes. No layout schema changes. Access paths: N/A — existing local JSON persistence only.

## External contracts
No public API changes. CI job retains its current identifier where possible to preserve branch protection references.

## Internal behavioral contracts
- Windows ignores use reason `unix-only: <specific capability>`; test harness reports each and aggregate count, even for a zero count.
- Unix-only Rust code/imports are behind cfg(unix), Linux-only probes behind target_os=linux. For a gated case, expose a same-named ignored Windows counterpart (or equivalent runtime-visible harness entry), whose reason is explicit. Never run ignored placeholders as if they proved behavior.
- Derive omission counts from compiled test harness output, not number of gated files or source substring matches. Validate ordinary tests and omission reporting on both platforms; intentional Windows-specific tests cannot appear in omission inventory.
- Fresh launch child removes XDG_* and SHELL; clears inherited measurement/IVY test overrides so startup stays normal. Retain standard Windows system environment and set isolated absolute APPDATA. No mutation of the process-wide Rust test environment.
- Launch helper starts the installed executable, waits for its native HWND, drives the default shell to emit a unique output/file witness through the actual console input path, then requests ordinary window close. A created pane or process alone does not prove shell output. Prefer external native window automation over a product smoke switch; architecture review may identify a smaller equally faithful witness.
- Require parsed layout JSON with at least one saved console using the existing schema, exit code 0 and no forced termination on the success path. A finite failure deadline terminates the process tree and reports diagnostics.

## Depends on other tickets
No unmet dependencies. Ticket #161 is explicitly required by the issue but already delivered on base main by 8380c4e (#179); #177 also landed as 2269c5a (#181). Verified with git log and fetched origin/main. `ticket_deps` is empty: do not hold this study for work already present.

## Defensive mechanisms and their justification
| Mechanism | Present scenario/evidence | Needed now? |
|-----------|---------------------------|-------------|
| Deadline and process-tree cleanup | Interactive default shell remains alive; installer CI otherwise hangs | yes |
| Isolated APPDATA and separate preservation fixture | test-install.ps1 currently masks fallback with XDG_CONFIG_HOME | yes |
| Expected Windows test names + omission total | Existing workflow explains zero-test filters succeed; cfg can hide code | yes |
| Explicit native command exit propagation in pwsh | Existing workflow checks LASTEXITCODE after cargo/python | yes |
No retries, product fallback or new persistence format.

## Identified risks
- Broad gates could hide portable assertions: per-test audit, helpers/import cfgs, Windows expected-name checks, Linux regression suite.
- Windows shell/PT Y lifetime and native paths differ: use real Windows execution, no Git Bash substitute; repair diagnosed failures.
- GUI automation can lose focus: target the launched PID/window, make the witness explicit and bound the wait; retain diagnostics on failure. Do not lengthen sleeps as a fix.
- PR installer cost: reuse jobs/build artifacts rather than duplicate release builds. Existing disposable-runner permission gate remains intact.
- No Windows runtime exists in this Linux study container; Windows execution evidence is required during implementation/CI, not claimed here.

## Planned tests
- AC1-3: native Windows clippy and full workspace suite; report compiled ignored tests, expected Windows names, and Linux whole-suite regression.
- AC4-5: installed clean-profile launch, parse saved layout, normal close and exit, plus controlled failure trials for startup/shell/save/deadline. Re-run installer lifecycle retention checks after smoke.
- Inspect `ai events --json` for any later pipeline/review failure; logs are evidence only and cannot change gates.

## Implementation plan (order)
1. Audit platform fixtures, imports, examples and early returns. Keep neutral tests; give simple command fixtures native Windows arms or explicitly ignored platform counterparts for Unix-only behavior.
2. Expand native Windows job to full lint/tests and omission summary; retain existing Windows-name witnesses.
3. Add clean installed-launch driver and integrate packaging/lifecycle into PR/manual verification.
4. Validate on Linux and actual windows-2025, including failing smoke controls; review and publish together.

## Decisions (ADR-light)
| Decision | Discarded alternatives | Why |
|----------|-------------------------|-----|
| Normal startup and normal close | Measurement mode; new smoke-only product startup | startup.rs:35 vetoes persistence under measurement; ordinary startup covers the reported regression |
| Runtime-visible per-test omissions | Blanket module gates; unchecked static manifest; port all Unix semantics | Mixed lib.rs/resume.rs tests contain neutral assertions and Windows arms; file counts cannot measure omitted tests |
| Native Windows verification on PRs | Manual-only installer job; cross-compilation | Existing windows job only runs workflow_dispatch and building cannot demonstrate launch or ConPTY behavior |

## Design challenges
Independent findings are in design-challenge.md. F1 (high): exact compiled omission accounting lacks a selected mechanism. Response pending orchestrator answer; ask-question.md contains the complete recommended compiler-checked inventory response. Medium findings about shell identity, inherited default-shell inputs and window/dialog evidence will be incorporated alongside that answer. No implementation or phase approval has occurred.

Design agent tasks: 4/12; configured models.agents/models.workers from FLOW.codex.md; cost not reported.
