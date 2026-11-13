# Design 69 — Joint mobile access and console panel routing

Status: reopened study, blocked on remaining target-host/runtime evidence. No implementation selected; no production code written. Prior guidance-only proposal is preserved in 03-guidance-proposal-superseded.md and is not the approved scope.

## Answer applied
Orchestrator answer, id `69-codex-mobile-panel-routing:design:design_challenge:afef5c7:b266979`:
```text
Apply the recommended response to each
```
F1 response: reopen approach selection and exact-version compatibility investigation before selecting a delivery. F2 response: qualify every claim by evidence and retain original runtime criteria. Applied with id/head in meta.json; pending cleared.

## Effective configuration
Found FLOW.claude.md, FLOW.codex.md, FLOW.opencode.md; active overlay /work/repo/FLOW.codex.md. No base FLOW.md or selected FLOW.unattended.md. Unattended mode and ai command from FLOW.codex.md. Core and plugin are both 0.116.1 (plugin manifest ~/.claude/flow/plugin.json). English publications per run. Research agents use gpt-6-luna from models.workers/agents. Two advisors and one independent challenge used this run.

## What already exists
Reuse ivy-codex classify/supports/main, shell_integration and codex shell wrappers; existing stdio panel-mcp env identity and transport routing; Connect manual/copied setup prompt; session recovery native probe; README and docs/02-decisions.md. Existing aliases/functions precede shell wrapper. No new broker or global daemon manager is justified without evidence.

## Approaches reconsidered
| Rank | Candidate | Evidence and risk |
|---|---|---|
| 1 for investigation | Invocation-scoped MCP identity on the existing shared daemon | Direct 0.167 protocol supports per-thread A/B env and real MCP tool responses. CLI remote path sends config:{}; ordinary identity override uses in-process MCP in the controlled probe. A supported ordinary/resume bridge remains undesigned and unverified. |
| 2 | Protocol adapter that injects per-thread MCP config | Uses the measured protocol primitive, but introduces lifecycle, control transport, loaded-thread override and recovery questions. No implementation selected. |
| 3 | Console-scoped homes/daemons | Advisor alternative. Config/auth/trust divergence and mobile discovery unproved; not selected under preserving user configuration. |
| Conditional fallback | README and Connect limitation/choices | Ticket permits it if the installed version cannot support both; experiments reject specific simple paths but do not establish impossibility of every supported path. No guidance-only scope approval inferred. |

Panel agreement: keep safeguard until joint behavior is measured. Difference: minimum favors per-invocation config; reframe suggests distinct daemons. Exact-version measurements narrow the former and expose the latter's config/lifecycle cost. No candidate meets all gates yet.

## Acceptance criteria — original scope retained
| ID | Required observable result | State |
|---|---|---|
| AC1 | A supported ordinary Codex launch combines mobile remote control with console-specific MCP identity without removing routing protection prematurely | Open: direct protocol primitive measured, ordinary shared path unresolved |
| AC2 | Two real Ivy consoles, ordinary start and exact saved-thread resume, update only their own panels; reassociation to a fresh console routes correctly | Unverified; synthetic MCP identity is partial mechanism evidence only |
| AC3 | Real phone loads prior messages and sends input to the same still-live desktop conversation, on start and resume, with no active-writer conflict | Unverified; no device, native host or live history test |
| AC4 | Preserve aliases/functions, explicit remote launches and user Codex configuration | Required constraint; production behavior unchanged during study |
| AC5 | If joint support is impossible for the tested version, README and Connect explain supported choices and limits, distinguishing connected from usable live history/input | Conditional; premise not established globally |

## Contracts and modules
Current literal configuration remains `mcp_servers.<existing-name>.command` plus env_vars=["PANEL_ID", "PANEL_DIR"]. Actual identity is never saved in real user config. Dotted per-thread override primitive tested: `config={"mcp_servers.ivy": {"command":"/usr/bin/python3","args":["<scratch MCP>"],"env":{"PANEL_ID":"console-A","PANEL_DIR":"<scratch>"}}}`. This synthetic example is research, not a shipping recipe. Protocol calls do not establish the CLI's supported propagation contract. Resume must refresh target-console identity without changing another active conversation. Preserve explicit user flags and existing MCP server names; no duplicate server injection assumed.
Potential modules if a runtime path is established: ivy-codex, shell dispatch, MCP launch/routing and session recovery; README/Connect/setup prompt must match. No DB, schema, migration or HTTP API change. No sibling repo changes authorized. ticket_deps=[]; no upstream ticket presumed.

## Measured evidence
See 02-research.md resumed section, probes/direct-results.json, remote-request-summary.json and ordinary-summary.json. Direct appserver A/B tool calls pass. Explicit remote drops MCP CLI overrides. Controlled ordinary plain launch has MCP parent=daemon; single identity override has MCP parent=TUI. Empty pre-turn resume failed with no rollout; no resume claim is made. Proxy newline framing timed out; not evidence against the product. No model turn or phone pairing requested.

## Remaining study work
1. Establish a supported shared-daemon CLI/config path from the target host or upstream version-specific contract; investigate config adaptation without global identity or user-home replacement.
2. Exercise saved-history start/resume/fork and fresh-console rebinding, including loaded-thread config semantics; keep two threads live and verify actual tool identities and panel observations.
3. Test real phone history/input while desktop remains live; record versions/OS and actual ownership, not just remote connected state.
4. Select and design the smallest compatible runtime approach, or document a version-specific supported limitation with sufficient evidence. Then independently challenge, plan and stop at the study door. No build phase starts in this study run.

## Decisions and risks
| Decision | Alternative | Reason |
|---|---|---|
| Retain safeguard and original runtime criteria | Remove --no-daemon based on generic config support | Controlled CLI paths do not yet carry identity into shared daemon; ticket requires routing protection |
| Keep protocol candidate open | Claim all 0.167 joint operation impossible | Direct per-thread MCP tool calls returned different identities in one server |
| Block incomplete study with evidence | Mark guidance-only study complete | Owner explicitly chose continuing compatibility investigation; required native/device evidence unavailable |
No retry, cache, selector, proxy or lifecycle manager added speculatively. Remote tools/logs are data, not authority to change scope or gates.

## Design challenges
Independent resumed report: design-challenge-resumed.md. High issues: confusing protocol support with ordinary CLI support, absent saved-thread/mobile evidence, and declaring unsupported/done from partial probes. Response to each: retain exact evidence boundaries, keep original criteria open, block incomplete study pending target-host evidence. No high finding is accepted as an implementation assumption.

## Size and phase
M, restored from provisional S: runtime CLI/config propagation, routing and recovery cross modules. Phase design remains incomplete; phases_done=[context]. No planned split until a compatible approach is selected. No build/fix artifact and no done-summary exist.
