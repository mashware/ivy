# Design 69 — Explain remote-control compatibility without regressing panel routing

## Approaches considered
The minimum and reframe advisors independently agree that removing the safeguard is not supported by evidence. Neither established an alternative joint-support path; their emphasis differs (explain current limitation versus defer topology changes). Reports: approaches/minimum.md and approaches/reframe.md. Models from FLOW.codex.md: workers=gpt-6-luna; 2 advisors, followed by one design challenger.

| Rank | Approach | Affected modules | Main risk / why it could be a bad idea |
|---|---|---|---|
| 1 (Recommended) | Preserve launch policy; document observed incompatibility, separate choices and required proofs | README, Connect manual/setup prompt, existing guide tests, decisions | Does not enable same-live-thread mobile access; must describe it as unresolved, never claim impossibility for untested versions |
| 2 | Investigate invocation-scoped MCP config forwarding to a shared daemon | launcher, shell dispatch, recovery, MCP and native tests | Generic protocol config exists, but CLI forwarding, reattachment and persistence remain unproved; unsafe to ship from schema evidence |
| 3 | Separate per-console remote-control servers or proxy | new lifecycle, auth and routing integration | No demonstrated phone association or recovery story; complexity exceeds evidence |
| Rejected | Simply remove --no-daemon or persist daemon_auto_start=true | launcher/user config | Reintroduces reported identity failure or changes personal config, against the ticket |

Provisional recommendation: rank 1, subject to the unresolved high challenge about the fallback premise. No production code is written in this study. No new user setting or mode is introduced.

## Decisions clarified while choosing the approach
No open product decision: the ticket explicitly permits fallback. Runtime compatibility remains an evidence gap, not a question that can be answered by preference. Do not turn the phrase “no verified joint path” into “Codex cannot support this.”

## Additional domain context
See 02-research.md for local help/schema evidence, exact version boundaries and official sources. Owner-reported active-writer conflict is failure evidence only. Container has 0.163.0; owner reportedly used 0.167.0. No phone or live Ivy GUI verification here.

## What already exists (inventory)
- ivy-codex classify/supports/main: capability-checked independent launch, argument/environment inheritance, remote/admin bypass.
- shell_integration plus resources/agent-integrations/codex/shell: transient integration, aliases/functions precedence.
- README Codex section; chrome/connection.rs manual and copied setup_prompt: currently advise independent mode without mobile caveat.
- chrome/connection/tests.rs: existing prompt/guide regression assertions to update proportionally.
- docs/02-decisions.md Codex clarification: rationale for avoiding persistent identity/global daemon restarts.
- docs/03-architecture.md and recovery README: current launch and ownership contracts remain accurate under selected approach.

## Executive summary
Tell users why a connected remote host can still fail to open a live independent conversation. Keep ordinary local startup isolated, show the supported independent-panel choice, and describe explicit remote/shared launch as a separate topology with unverified Ivy routing. Do not promote it as a working combined solution. Give a precise verification procedure and disclose what has not been tested.

## Acceptance criteria
| ID | Observable assertion | Proof |
|---|---|---|
| AC1 | README and Connect manual plus copied setup prompt explain that ordinary supported start/resume/fork inject --no-daemon; remote-control connected status does not prove that the active conversation can load messages or accept input. | Review text and existing prompt tests |
| AC2 | Documentation explains PANEL_ID/PANEL_DIR forwarding and retains the existing safeguard; two real consoles on ordinary start and exact resume must update only their own panels before any native isolation claim. | Existing launcher/shell regressions; manual two-console check remains unverified in this run |
| AC3 | Any claim of joint support requires a real phone to load history and send input to the still-live conversation without an active-writer conflict, for the checked versions. Without that evidence, README and Connect explicitly state the reported topology is not verified for joint use and explain the limitation/choices. | Text review for fallback; real-phone verification required for runtime claim, unavailable here |
| AC4 | Existing aliases/functions, explicit remote/admin argv, auth and user Codex config retain their behavior; no config/home replacement, saved pane values or daemon lifecycle changes are introduced. | Unchanged launch code; relevant existing launcher/shell checks |
| AC5 | Guidance distinguishes the host’s personal features.daemon_auto_start=false from Ivy’s argument; enabling the feature alone is not presented as overriding --no-daemon. No automatic setting change is proposed. | Text review and preserved-config regression |
| AC6 | README, manual guide and copied setup prompt agree on observed version/OS limits, supported choices, and the difference between discovery, usable tool calls, correct-console observation and mobile access. | Existing prompt tests plus editorial cross-check |

Original ticket intent is preserved: joint runtime behavior is the preferred outcome, but the expressly permitted limitation path is the deliverable here. AC2/AC3 runtime checks remain unverified, never converted into passed checks by publishing documentation. Future publication must prominently disclose this.

## Modules/layers affected
1. README.md, existing Codex section: add mobile limitation, choices and test steps.
2. crates/app/src/chrome/connection.rs: matching visible manual guidance and copied setup_prompt, preserving copy-only behavior.
3. crates/app/src/tests/integrated_chrome.rs: adapt meaningful existing guidance checks for new caveat/false-success prevention; avoid exact whole-prose snapshots.
4. docs/02-decisions.md: append a dated clarification to existing Codex setup rationale; distinguish report, tested capabilities and unverified compatibility.
No launch-policy, bridge, shell integration, session recovery or architecture behavior changes. Reuse existing guide, no new UI component.

## Data model
No entities, migrations or indexes.
### Access paths
N/A — no data access.

## CQRS / Commands and Queries
N/A.

## API / HTTP
No new API or transport. No upstream changes.

## External contracts
Existing stdio configuration remains literal:
```toml
[mcp_servers.ivy]
command = "/absolute/path/to/panel-mcp"
env_vars = ["PANEL_ID", "PANEL_DIR"]
```
`ivy` is an example: update the existing server name rather than duplicate or overwrite it. Never write actual per-console values into persistent config. Consumers are existing Codex installations. Documentation only; no new contract.
Explicit bypasses remain `command codex`, the verified external absolute client path, and existing explicit `--remote <ADDR>` forms. These select another launch path; they do not guarantee per-console Ivy panels or solve the competing-writer error for an already live independent thread. Do not recommend opening a second writer or restarting/stopping a daemon as a repair.

## Internal behavioral contracts
- Ordinary supported local startup continues to prepend --no-daemon only when advertised, including resume/fork; argv otherwise inherited unchanged.
- Opening/copying the Connect guide executes no program and writes no client settings.
- A host connected result alone never warrants a successful live-phone claim.
- Keep the “distinct statuses in two consoles” verification and add actual history/input checks for mobile; do not use tool discovery as a proxy for either.

## Depends on other tickets
None. ticket_deps=[]. No upstream implementation is presumed; if a later build elects a behavior change, reopen this design and verify it first.

## Identified risks
- Compatibility uncertainty: mitigated by version-qualified evidence and explicit lack of host/mobile proof, without declaring all Codex versions incapable.
- Existing documentation may send the assistant toward independent mode when a person wants mobile access: mitigated by updating both setup_prompt and manual copy.
- An explicit bypass may again lose identity: mitigated by the same caveat adjacent to choices, not a promise that it fixes joint operation.
- Personal daemon setting is independent: handled explicitly in guidance; leave config unchanged.
- Native testing unavailable: record pending two-console/phone checks in validation and publication. No successful connection claim.
No deferred risk is silently accepted.

## Defensive mechanisms and their justification
No new guards, retries, caches, locks or flags. Retain the existing independent launch guard because D73’s observed shared-daemon environment loss and the ticket both require preserving console routing until proved otherwise.

## Implementation plan (order)
1. Update README with observed active-writer symptom, launch-mode explanation, separate choices and version-qualified verification procedure; cite official docs already opened in 02-research.md.
2. Update both visible Connect manual and setup_prompt with matching limitation, personal flag distinction and refusal to infer mobile success from connected status.
3. Adapt existing guide checks to protect the meaningful caveat and run relevant existing launcher, MCP and shell regressions. Do not add a tautological test solely to mirror prose.
4. Append dated rationale to existing decision; editorial cross-check all surfaces. Validate text path without claiming native checks. Publish one coherent PR after approved study; keep launch behavior unchanged.

## Planned tests
- AC1/AC3/AC5/AC6: existing connection prompt tests and editorial review of README/manual/copied prompt.
- AC2/AC4: existing `cargo test --locked -p panel-control --test ivy_codex_launch`; existing panel-mcp identity/handshake regressions and app shell_integration tests as supported by dependencies/toolchain.
- AC2 manual: two real consoles, distinguishable statuses, ordinary start then exact resume, alternate updates while both live. Record versions/argv; reassociation to fresh console too.
- AC3 manual: real phone opens each still-live conversation, shows prior messages, accepts prompt and produces matching desktop/phone reply, with no active-writer conflict. Test start and resume, recording exact host/mobile versions. Do not mark verified from a daemon connected event.
- AC4: verify aliases/functions and explicit remote paths retain precedence/argv; compare user config bytes without printing secrets. No real device or GUI is available in study, so no suite/runtime success is claimed.

## Decisions (ADR-light)
| Decision | Discarded alternatives | Concrete reason |
|---|---|---|
| Use the permitted documentation fallback for this delivery | remove guard; per-thread config override; per-console remote servers | Guard removal loses known routing protection; generic schema is insufficient to prove CLI/mobile rebinding; no exact-target host/device available |
| Preserve personal daemon_auto_start | enable globally; rewrite CODEX_HOME | Separate injected argument still selects independent mode, and ticket requires config preservation |
| Keep one coherent delivery | separate README and Connect PRs | Either alone would leave contradictory setup advice to users |

## Design challenges
Independent report: design-challenge.md. Architecture report: architecture.md. The challenger found two unresolved high issues: selecting the ticket’s conditional fallback before establishing its premise, and overstating version/OS evidence. Exact 0.167.0 help/schema checks now supplement older local evidence but do not prove runtime compatibility or impossibility. No high response has been taken on the owner’s behalf. The proposal is provisional; do not build or treat the documentation fallback as approved.

| Angle | Finding | Type | Severity | Response |
|---|---|---|---|---|
| Fit/need | No evidence yet establishes that the reported target version cannot support both; prose is not phone acceptance proof | missing | high | Pending orchestrator question: recommended reopen design and keep compatibility investigation as required before selecting fallback |
| Assumption | Version/OS scope must distinguish checked capabilities from owner-reported runtime failure | missing | high | Pending orchestrator question: recommended constrain all statements to their actual evidence and keep native checks open |
| Simplification | Avoid duplicate wording-only tests | unnecessary | low | Reuse integrated_chrome test and editorial checklist; corrected test pointer |
| Operation | Remote bypass could lose panel identity | idiom | medium | Keep routing caveat next to every remote choice |
| ADR | Invocation-scoped config is an open candidate, not a disproved alternative | missing | medium | Keep candidate open in research; cannot select a final runtime approach without testing propagation and rebinding |


## Size and handoff
Reclassify M → S (recorded unattended) after investigation: one contained guidance change, no new launch or domain logic. No delivery split needed; design → build door. Design remains pending the challenge response; why=study must never advance into code.

## Answer applied — 2026-11-13
Orchestrator answer for 69-codex-mobile-panel-routing:design:design_challenge:afef5c7:b266979:
```text
Apply the recommended response to each
```
Response F1: Reopen approach selection and exact-version investigation; guidance-only delivery is not selected.
Response F2: Constrain all compatibility claims to their evidence. Two-console start/resume and real-phone checks remain mandatory and open.
Reclassified S → M because runtime launcher/config propagation and session recovery cross modules. Previous guidance plan above is superseded pending investigation.
