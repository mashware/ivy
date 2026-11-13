# Reframe: document the supported boundary

**Approach.** Keep Ivy’s version-checked `--no-daemon` injection for local interactive, resume and fork commands; document remote-control compatibility as unresolved. Explain the choices: use independent local sessions with console routing, or try remote control only through a Codex-documented launch mode compatible with one active thread writer, without Ivy promising multi-console isolation. `connected` does not prove a live conversation can be resumed. Revisit integration only after Codex documents per-thread MCP environment/configuration and phone plus two-console checks pass.

**Modules.** `crates/panel-control/src/bin/ivy-codex.rs` classifies commands and injects the flag. `crates/app/src/shell_integration.rs` and Bash/Zsh/Fish resources preserve aliases/functions and invoke the helper. Update `README.md` and `crates/app/src/chrome/connection.rs` (including its prompt) consistently. Keep user Codex config and `PANEL_ID`/`PANEL_DIR` forwarding unchanged; shared MCP process identity is not established by forwarding variable names alone.

**Main risk.** The phone workflow remains unavailable until Codex documents a compatible path. Evidence does not establish single-console support or parity between local 0.163.0 and owner-host 0.167.0; keep both unknown.

**Why dropping the safeguard is a bad idea.** A shared daemon can own the thread the remote server resumes (the observed writer conflict) and lose console panel identity. Removing the flag risks cross-console leakage without proving phone access works.
