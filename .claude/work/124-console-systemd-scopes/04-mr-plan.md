# Delivery plan 124

## Summary
- Number of PRs: 1.
- One coherent change contains Linux isolation, compatibility fallback, reliable exit evidence and the user notice. The async startup work is required to keep slow manager calls off the UI, not a separate feature.
- Advisor: `plan-advisor.md`. Its conditional early-mitigation split was evaluated and merged below.
- Size M remains appropriate: an external integration across the terminal core and application, with no database changes. Estimates are advisory thermometers, never targets or ceilings.

## Execution order (waves)
Wave 1: #1 (start after study approval). No later waves or PR dependencies.

| n | Wave | depends_on | Title | Lines estimate | Files estimate |
|---|------|------------|-------|----------------|----------------|
| 1 | 1 | [] | Isolate Linux consoles in systemd scopes and explain OOM exits | ~1200 | ~18 |

## PR #1: Isolate Linux consoles in systemd scopes and explain OOM exits

### Includes
- Linux-only user-manager backend, unique transient scope names, optional OOMPolicy and retained final-result observation.
- Fresh internal Ivy launcher, private gate IPC and in-place exec preserving the owned PTY child; fallback executes the original command once.
- Asynchronous Linux launch completion, stable pending console identity/order, cancellation and original launch-error handling.
- Normal/restored/fallback/takeover call sites; no extra installed helper binary.
- Cached termination state, memory-pressure notice, wake/drain/dismiss and automatic-close behavior; ordinary exits and unknown signals remain distinct.
- Tests covering AC1–AC6, Linux dependency/lockfile changes, relevant source inventories and focused behavior documentation.

### Why it is its own PR
This is the single unsplit deliverable for ticket 124. It repairs the reported containment failure and supplies the requested explanation together.

### Standalone-mergeable
Yes. No persisted state or other-ticket/PR prerequisite. Optional integration degrades to existing unscoped execution. macOS and Windows keep direct launching. The public term-core surface remains bespoke and old entry points remain available.

### Unlocks
Ticket completion; no subsequent slice.

### Risk if it stays alone on main
The intended final behavior is complete. Older managers may isolate processes without confirming oomd attribution; those cases remain unknown and documented. Policy at a common ancestor or a global kernel OOM still lies outside leaf-scope isolation guarantees.

### Estimate
M; ~1200 added/modified lines in ~18 files including tests and inventories. The asynchronous application boundary and actual Linux tests may move this estimate; implement coherently and record the actual delta rather than cut tests or documentation to fit it.

## Dependencies between PRs
None. Other-ticket dependencies: none. No release/deploy dependency.

## Merge test
Evaluated the candidate pair “scope launch/backend” → “OOM outcome and notice” (the latter is its only dependent). A backend-only first PR does mitigate a real failure and is asymmetric with a UI-only first PR, as the advisor observed. But the only loss from merging them is the opportunity for an earlier partial release, which this run has no established requirement to make. The orchestrator may stack parts on an umbrella, so separate parts do not guarantee that early default-branch release. Scope lifetime observation and the notice also share one exit contract. Merge the pair into #1 and avoid an extra pipeline/merge wave. Same-wave pairs in the final one-PR plan: not applicable.

## Plan risks
- No online migrations, deployed-client transition, new domain events or runtime feature flags.
- New Linux D-Bus dependency must be target-specific, lockfile-pinned and compatible with Rust 1.97.1 and packaging license generation.
- Async launch must preserve existing app behavior on errors and cancellation, and existing module ceilings.
- Runtime host verification needs a build environment with Rust and a disposable Linux systemd user session. A private dbus-run-session alone cannot prove scope placement or oomd behavior.
- Kernel-OOM simulation is useful but is not proof of userspace systemd-oomd; validate both mechanisms separately without stressing the host session.

## Implementation checklist (for build)
- [ ] Gate/manager backend and injected failure tests (AC1, AC2).
- [ ] Ivy helper dispatch and asynchronous app integration, including cancel/fallback/takeover (AC2, AC5).
- [ ] Scope outcome/termination state and existing console notice/auto-close integration (AC3, AC4).
- [ ] Real Linux helper and cgroup integration, ordinary regression suite and platform checks (AC1–AC6).

## Decision: one or several PRs?
One. Plan accepted as an unattended study draft; implementation remains behind the orchestrator's study approval. No code is written by this run.
