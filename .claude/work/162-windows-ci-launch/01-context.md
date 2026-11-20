# Context 162

## Ticket
- Windows CI must run workspace clippy and the whole workspace test suite on windows-2025.
- Launch the installed Ivy.exe with XDG_* and SHELL absent, exercise a real console, save layout under APPDATA, and exit 0.
- Unix-only tests must be explicitly gated and their omitted count reported.
- Depends on #161. No other repositories are in scope.

## Acceptance criteria (provisional)
- A startup crash fails CI.
- Windows-specific test arms compile and run.
- Unix-only omissions are counted in job output.

## Decided in the ticket thread
Empty thread, supplied by ai ticket --json.

## Relevant domain knowledge
No configured knowledge search. Repository inventory follows in the design.

## Repo state at start
- Branch: main; clean.
- Last commit: 0429d9ea71534b88703625be6e90563207562a7b.
- Effective configuration: FLOW.codex.md only; FLOW.md absent; FLOW_CONTEXT=unattended but FLOW.unattended.md absent. Inactive FLOW.claude.md and FLOW.opencode.md listed, not applied.
- Autonomy: unattended; orchestrator: ai; worktree: off (all from FLOW.codex.md).
- Shared flow rules version 0.118.0; installed plugin manifest absent, so version comparison unavailable.

## Decisions clarified at start
No open product questions. kind null routes as feat. why=study stops before implementation.

## Estimated size: M
Recommended: CI, installer lifecycle, test gating across multiple crates, and a real GUI/PTY launch cross several modules. An initial shell/platform search matches 71 Rust files; this is an inventory lead, not a skip count.
