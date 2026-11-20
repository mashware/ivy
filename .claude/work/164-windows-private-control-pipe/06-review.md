# Code review 164

## Summary
- Reviewed: `137fd348...3b4fbbc` first frozen full branch; authentication/revocation fix rechecked at `d251b2d`; test-fixture/guard delta reviewed at `a430c91` and `d2b7c59`. Final ownership/size-guard refactor independently reviewed at `c74c76d3f01c82c35de8d6312070fe68aaa2072c`. Final reviewed tree after the equivalent-cfg/wait-declaration audit: `4ed9487a44b152a773109cf83a7a55b3c0417b47`. No untracked source files. Full branch context retained because fixes touch prior findings.
- Review tier: proportional L, authentication/public API sensitive surface, maximum effort broad review plus blinded contract and idiom checks. Built-in review tool unavailable: independent general-purpose reviewer used as an explicit substitute; configured extra panel/review_skill and role reinforcements are empty.
- Review path: one frozen full bundle, then a final fix bundle and bounded revision diffs. `agents.exec_cmd` empty, so agentic path. Plugin manifest unavailable; CLI pinned to CORE 0.118.0 and works.
- Config read: FLOW.codex.md — FLOW_CONTEXT=unattended: FLOW.unattended.md not found
- Agent models: models.agents/workers = gpt-6-luna from FLOW.codex.md. No saved role definitions.
- Cost: 9/12 subagents launched, 12 reviewer turns including follow-ups; proportional, broad effort max. Build's three delegated writers are separate from this review budget.
- Measured cost: not reported.
- Completeness rounds: 2. Second audit: no uncovered files. First audit abbreviated two paths incorrectly; targeted review covered the actual chrome/channel files and agent-signal tests.
- Remaining blockers: 0. High findings: 0.

## Measured tier
```
Changed lines: 4090 over 41 file(s), `137fd3481658`..`d251b2d1391d` (base `main`)
Config read: FLOW.codex.md — FLOW_CONTEXT=unattended: FLOW.unattended.md not found
review_depth: proportional
Diff size: L   ·   meta.json.size: L   ·   Effective size: L
Tier: proportional → built-in at `xhigh`, panel: yes
With the sensitive-surface bump: built-in at `max`, panel always
Budget: budget_max 12 · fanout_max 4
Within 10% under a threshold: no
quality.sensitive_paths matched: none (the key is empty)

Left to you, because a command cannot see it: whether the generic categories apply
(auth, secrets, payments, personal data, a public contract, a schema change), and
whether a matched path is a control-flow change or observability only — a log level,
a metric, a message text gets the review its size already earned. Decide, then record
which of the two it was. Any deviation from the tier above goes in 06-review.md with
its reason.

```
Final guard/ownership deltas add four files to the branch total and was reviewed separately; it does not lower the L/security tier.

## Findings and disposition
1. **Fixed, general + blinded contract:** public Windows endpoint::connect bypassed server PID verification. It now reads inherited expected PID and authenticates before returning a stream; subprocess regression proves the public route refuses a wrong inherited PID before bytes. Native CI requires its test name.
2. **Fixed, general:** release admission ignored revocation. Per-slot closure races admission CAS, revocation is checked around admission and reservation publication, prior admitted tokens retain their pending count until UI completion. Native regression covers rejected new admissions and retained prior work; follow-up reviewer found no blocker.
3. **Fixed, app reinforcement:** callback-only app fixtures supplied no HWND or inherited app identity on Windows. The Unix callback fixture and exactly its dependent tests are explicitly Unix; dedicated native concrete-service tests remain compiled and required. GNU test cross-check exposed a misplaced local-method gate and two transitive callers; corrected, independently rechecked.
4. **Fixed, parent static analysis:** app's platform wiring crossed Clippy's argument-count threshold; narrow boundary annotations preserve the direct wiring.
5. **Fixed, full-suite source guard:** scripts::the_list_still_names_every_module required three new panel-control module rows. Added real include_str entries; guard assertions remain intact.

## Rejected reports
- Other Windows forwarding lists were alleged missing without a named path. Repository search finds Codex's env_vars as the only explicit Windows allowlist; opening/resume append PID separately, no-pane and daemon remove it. Final blinded contract check matches all four grouped contracts.
- WM_PAINT was alleged unsuitable as a native wake. Pinned winit 0.30.13 Windows event_loop.rs:1276–1297 turns the message into RedrawRequested and handles DefWindowProc lifecycle. Callback replacement conflicts with the bounded concrete-worker requirement; WM_APP requires an absent owner hook. Actual resting-pane painting remains human proof, never inferred from source.

## Double-blind contract verification
Both passes received only literal contracts and shape hunks. Final pass matches pipe/wire, separate PID launch/clearing, authenticated entry routes, stable two-entry arrays, and no new HTTP/DB/wire verb.

## Over-engineering (fit + YAGNI)
Protected ACLs, first-instance ownership, PID checks, bounded connection/inbox/snapshots, private stop events, completion draining, release slots and worker ownership address the approved present scenarios. Fault hooks are private/channel-local and production passes None. No new runtime, retry service, cache or detach fallback. No unjustified defensive mechanism found.

## Data-access duel
No database/search/key-value queries or schema changes, including fixes.

## Adversarial verification
Not run: fewer than four ambiguous findings. The two concrete defects were fixed; two speculative reports were checked against repository/pinned dependency source. No finding left unverified due to budget.

## Areas not covered after two rounds / skipped for budget
None. Runtime Windows, macOS and GUI/network execution are validation locations, not claims of code-review proof.

## Quality gates
Format and Linux Clippy pass. MSVC contract/control tests cross-compile; GNU app tests cross-compile. Final Linux workspace run is recorded in 07-validation.md. Native execution is assigned to Windows CI by the owner's proof-location clarification.

## Next step
Validate and hand off a draft; owner proof remains explicit.

## Final guard repair and independent recheck
Source guards identified a legitimate new environment name and app module/type growth. Added PANEL_SERVER_PID beside existing external environment names. Moved HWND wiring and its wake test to existing control/channel.rs; document installation/publication and commit policy are free functions in document.rs; close draining is a free control function. PanelState's measured ratchet decreases 356→353; no ceiling raised, no guarded production hidden. Independent maximum-effort reader examined all nine delta files and callers at c74c76d and found no issue. No production edits after that pass. New guard files are part of this explicit coverage addition after the completeness sweep.

## Final measured tier
```
Changed lines: 4161 over 45 file(s), `137fd3481658`..`c74c76d3f01c` (base `main`)
Config read: FLOW.codex.md — FLOW_CONTEXT=unattended: FLOW.unattended.md not found
review_depth: proportional
Diff size: L   ·   meta.json.size: L   ·   Effective size: L
Tier: proportional → built-in at `xhigh`, panel: yes
With the sensitive-surface bump: built-in at `max`, panel always
Budget: budget_max 12 · fanout_max 4
Within 10% under a threshold: no
quality.sensitive_paths matched: none (the key is empty)

Left to you, because a command cannot see it: whether the generic categories apply
(auth, secrets, payments, personal data, a public contract, a schema change), and
whether a matched path is a control-flow change or observability only — a log level,
a metric, a message text gets the review its size already earned. Decide, then record
which of the two it was. Any deviation from the tier above goes in 06-review.md with
its reason.

```

## Final wait-source audit
The complete suite then reached D57's wait audit. Separate cfg(test)/cfg(platform) attributes make test-only modules visible to that conservative scanner with identical Rust meaning. The existing plan-worker recv declaration sits immediately next to its call, below its allow attribute. Six native waits now explain their termination/intentional public blocking behavior; DECLARED_WAITS is measured 21 instead of 15, with the six sites named at the equality. No wait is removed from the scanner or moved behind a different API. Independent review of the exact ten-file 38+/14- delta at 4ed9487a44b152a773109cf83a7a55b3c0417b47 found no executable behavior change or issue. Final reviewed SHA is 4ed9487a44b152a773109cf83a7a55b3c0417b47; fixed_sha is empty.

## Final measured branch
```
Changed lines: 4177 over 46 file(s), `137fd3481658`..`4ed9487a44b1` (base `main`)
Config read: FLOW.codex.md — FLOW_CONTEXT=unattended: FLOW.unattended.md not found
review_depth: proportional
Diff size: L   ·   meta.json.size: L   ·   Effective size: L
Tier: proportional → built-in at `xhigh`, panel: yes
With the sensitive-surface bump: built-in at `max`, panel always
Budget: budget_max 12 · fanout_max 4
Within 10% under a threshold: no
quality.sensitive_paths matched: none (the key is empty)

Left to you, because a command cannot see it: whether the generic categories apply
(auth, secrets, payments, personal data, a public contract, a schema change), and
whether a matched path is a control-flow change or observability only — a log level,
a metric, a message text gets the review its size already earned. Decide, then record
which of the two it was. Any deviation from the tier above goes in 06-review.md with
its reason.

```
