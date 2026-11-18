# Context 107

## Ticket
- Compare Claude Code's mascot in Ivy and Tilix with the same dark-daltonized theme.
- The ticket reports screenshot RGB (245,154,137), before and after #103.
- Determine whether the colour is emitted by the application or introduced by terminal rendering.
- No change to the palette or renderer is authorized by an unproven colour discrepancy.

## Acceptance criteria (provisional)
- Record comparable screenshots and interior mascot RGB samples from both terminals, with the same application version and theme.
- Explain equality or differences with terminal output and colour settings as evidence.

## Decided in the ticket thread
Empty thread (orchestrator supplied comments: []).

## Relevant domain knowledge
No configured knowledge search. Local colour code inspected; findings recorded in 03-design.md.

## Repo state at start
- Branch: main; clean.
- Last commit: 074f0e5 — Split any console from its own header (#108).
- Effective configuration: FLOW.codex.md only. FLOW_CONTEXT=unattended; FLOW.unattended.md absent; FLOW.md absent. Inactive FLOW.claude.md and FLOW.opencode.md not read.
- No AGENTS.md found.

## Decisions clarified at start
No open product questions: the ticket defines the comparison. Captures require a graphical host with Tilix.

## Estimated size: XS
Verification-only work using existing terminal and colour paths, with no planned production code changes. The estimate describes scope, not a line budget.
