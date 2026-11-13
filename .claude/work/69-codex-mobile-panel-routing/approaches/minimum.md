# Minimum safe approach

Keep the console-local `--no-daemon` launch policy and document the current incompatibility between it and opening that same live conversation through Codex remote control. Update the README and Connect guide to state that a remote-control server reporting `connected` does not establish that an active local conversation can load; under the observed active-writer conflict, phone access to that conversation is unsupported. Give users the supported choices: use the local Ivy session with per-console routing, or use remote control only for a launch mode/version whose joint behavior has been demonstrated. Do not prescribe daemon mode or claim phone support until Codex provides a compatible way to retain per-console MCP identity and avoid competing writers.

**Affected modules:** `crates/panel-control/src/bin/ivy-codex.rs` (retain current injection and explicit remote bypass); `README.md`; Connect copy in `crates/app/src/chrome/connection.rs`. `crates/app/src/shell_integration.rs` needs no behavior change; its aliases/functions precedence and console-local identity setup remain intact.

**Risks:** The limitation may frustrate users and must be revisited when a Codex version offers a verified compatible mode. Documentation can become stale, so state the tested version/evidence and avoid promising a workaround.

**Why not remove `--no-daemon`:** ordinary consoles could again share a daemon session whose MCP bridge lacks the correct `PANEL_ID`/`PANEL_DIR`, reintroducing cross-console publication. The current evidence does not establish that daemon mode fixes phone access while preserving isolation.
