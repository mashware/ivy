# Implementation environment preflight

Ticket 164 is blocked at implementation preflight: provide a native Windows execution environment (with a second-user/remote pipe test facility and a desktop for the intended-pane GUI check), then relaunch this approved work with why=start.

The approved study requires native Windows proof for protected pipe/directory ACLs, other-user and remote connection denial, overlapped cancellation and joined worker shutdown, native UI wake, and actual intended-pane painting (AC1–AC6). 04-mr-plan.md explicitly states that Linux checks or Windows cross-compilation cannot establish these criteria and that native proof is required before publication. This run has no native Windows execution tool or configured Windows host. The only supplied GUI route, /opt/ivy-gui/README.md, runs a private Linux display.

Observed evidence:
- uname -a: Linux, x86_64.
- /usr/local/cargo/bin/rustc -vV: rustc 1.97.1; host x86_64-unknown-linux-gnu.
- /usr/local/cargo/bin/rustup target list --installed: x86_64-unknown-linux-gnu only.
- command -v wine, powershell and pwsh: unavailable.
- Rust exists under /usr/local/cargo/bin, outside PATH; using its absolute path resolves that local tooling issue.
- .github/workflows/ivy-prerelease.yml defines a windows-2025 pull-request job, but this run cannot launch or inspect a pre-publication native Windows execution through the ai interface. The unattended contract reserves forge publication to the orchestrator.

Run state:
- why=start; authoritative ticket 164; kind defaults to feat.
- Configuration listing found FLOW.claude.md, FLOW.codex.md and FLOW.opencode.md. Effective configuration is FLOW.codex.md; FLOW_CONTEXT=unattended selects no additional file because FLOW.unattended.md is absent. Shared rules and plugin both version 0.118.0.
- Created 164-windows-private-control-pipe from main with --no-track; HEAD and origin/main are both 137fd348165866dcd3382608a811d0c594119f42. No base catch-up was needed.
- Recorded the business brief, copied design contracts and five-step implementation checklist in 05-implementation.md. No production code was changed; no tests, commits or PR publication were claimed. Last completed phase remains plan; PR #1 is in_progress at build preflight.
- No schema changes; no pending question; no CI failure was supplied in this start run.

The next action belongs to the runner operator: make native Windows execution and the required security/UI test facilities available to the implementation run and relaunch ticket 164. No approval gate was answered or relaxed.
