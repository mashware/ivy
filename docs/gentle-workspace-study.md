# Feasibility study — A native agent workspace in Ivy

## Product judgment

A user reviewing a diff should be able to answer a helper without finding its terminal, losing the selected hunk or guessing which project asked. Ivy already owns the review surface. Gentle Shell already runs the agent and exposes an interactive host protocol. Connecting those two pieces is feasible on documentary evidence. Reply delivery across a disconnect, child-question identity and permission prompts still need a real-process trial. This study covers public interfaces and Ivy main at `bd89e654`. No runtime or signed-in account test was performed.

## Evidence and choices

| Component | Evidence | Proposed use in Ivy |
|---|---|---|
| Gentle AI | Configures existing agents through their native integrations. Engram supplies persistent memory, ODD guides work, and review support varies by agent. [Supported agents](https://github.com/Gentleman-Programming/gentle-ai/blob/main/docs/agents.md) | Let users keep their chosen configuration. Show setup status or open the upstream setup flow only on request. Leave memory, workflow transitions and review decisions with their producer. |
| Gentle Shell | Runs its own Pi workspace with helpers, tasks and permission controls. Interactive terminal sessions can emit OSC 7501 activity records. These include a blocked helper's identity and status. [Activity contract](https://github.com/Gentleman-Programming/gentle-shell/blob/main/docs/gentle-shell.md#osc-7501-activity-record-contract) | For users who launch the TUI, investigate optional read-only attention. The record signals a question but cannot carry a complete interactive exchange. |
| Gentle's desktop client | Starts one `gentle-shell --mode rpc` child per session and sets `GENTLE_SHELL_INTERACTIVE_HOST=1`. Its native UI handles questions and helper activity. Desktop capabilities vary with Shell version, and automatic permission mode may refuse RPC input. [Native host and limitations](https://github.com/Gentleman-Programming/gentle-shell-gpui#how-it-works) | Start with this documented host path for rich sessions. Confirm protocol behavior on a pinned release. Ivy owns rendering and each child owns agent execution. The reference client demonstrates feasibility without requiring a second desktop or a UI framework migration. |
| Gentle Tower | Its local dashboard can show tasks, send messages and stop helpers. Some permission changes require a terminal confirmation. [Tower controls](https://github.com/Gentleman-Programming/gentle-shell#gentle-tower--every-session-as-a-floor-in-your-browser) | Useful reference for cross-session UX. Embedding the dashboard would add another session selector and would leave Ivy's existing review context disconnected. |
| Ivy | Has pane-scoped producer identity, a project board, native documents and review feedback targeted to a console. [Architecture](03-architecture.md) · [Panel contract](04-panel-contract.md) · [Product decisions](02-decisions.md) | Keep ordinary PTY consoles. Introduce an opt-in native Gentle session with a private child channel and project association. Adapt the existing review delivery path for an RPC destination. |
| Claude subscriptions | Anthropic permits a user to sign in through the unmodified official Claude Code binary in a host app. Third-party applications may not intermediate subscription credentials. Gentle documents usage reporting from a Claude bridge. [Anthropic authentication rules](https://code.claude.com/docs/en/legal-and-compliance) · [Gentle Shell reference](https://github.com/Gentleman-Programming/gentle-shell/blob/main/docs/gentle-shell.md) | Support the official client in an Ivy console. Selecting Gentle AI settings is independent of who authenticates. Bridge usage counters alone establish neither entitlement nor billing. Treat bridge-backed subscription parity as unverified. |

## Proposed interaction and ownership

When a helper asks a question, Ivy shows the project, session, helper, question text and available answers. The reviewer opens the request with a click or keyboard, answers it, then returns to the same diff position. Only the owning runtime can resolve the request. A progress badge can disappear after a disconnect even if work is unfinished, so disconnect is a distinct visible state.

Bind pending replies to the child connection, session generation and upstream request identity. Disable submission once in flight. Distinguish local submission, upstream confirmation and observed continuation. If the child exits between send and acknowledgement, show delivery as unknown and reconcile using upstream evidence if available. Replaying input after reconnect can answer a different question. An ordinary message, an answer and a permission approval need distinct controls.

Review a file against the originating worktree and revision. Preview outgoing feedback and retain its original target when focus changes. Route a native Gentle message through its session channel and a console review through Ivy's existing explicit delivery path. Bound streamed output and batch visual updates so background activity cannot stall keyboard input. Measure the native view with one active and one noisy background session before making performance claims.

## Scope and remaining experiments

| User need | First delivery or decision |
|---|---|
| Conversation, tasks, helpers, questions and cancellation | Core native workflow. Verify lifecycle events, supported input types, helper targeting and failure handling against an installed version. |
| Plans, images, file changes and review | Reuse Ivy viewers. Prove artifact provenance and comment routing with two worktrees of the same repository. |
| Profiles, model routing, permissions, Engram, skills and ODD | Runtime-owned behavior. Identify which settings have supported host controls. Surface those incrementally and keep upstream setup reachable. |
| Usage and costs | Label source, freshness and unavailable data. Keep provider allowances separate from session token estimates. |
| Setup and portability | Verify executable discovery from desktop launches separately from interactive shells. Run native acceptance per OS before declaring support. |
| Full screen parity | Inventory remaining Gentle-only screens against user journeys after the core loop works. Design canvas, voice, music and visual themes are outside this epic's acceptance criteria. |

The first experiment uses two independent project sessions and a separate active review. Capture sanitized protocol events for a real child question, duplicate click, cancellation, runtime exit and late reply. Record exact release and OS. If the available interface cannot safely address a helper question, ship only declared attention with a jump to its owning interaction and keep the native reply milestone open. Revisit the integration with an upstream contract or a scoped extension before proceeding.

[Active roadmap](05-roadmap.md) · [Tracking epic #280](https://github.com/mashware/ivy/issues/280)
