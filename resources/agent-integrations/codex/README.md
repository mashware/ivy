# Codex title selection

Codex 0.167.0 publishes native lifecycle independently of its terminal title. To retain conversation/project names without duplicate activity, merge this key into the existing `[tui]` table of the effective Codex `config.toml`, normally `~/.codex/config.toml`:

```toml
[tui]
terminal_title = ["thread-name", "project-name"]
```

If the key already contains customized items, preserve their order and remove `activity` (including its `spinner` alias). Optional `run-state`/`status` also adds lifecycle text. Do not create a duplicate table or overwrite unrelated keys. Restart Codex and resume the desired conversation. Ivy previews this setting and never edits the file.

Pinned contracts: [schema](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/core/config.schema.json), [title renderer](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/tui/src/chatwidget/status_surfaces.rs), [native status](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/tui/src/terminal_program_status.rs). On Linux on 2026-11-09, the actual official 0.167.0 TUI emitted native permission and Plan-mode question waits, withdrew them after answering, and cleared on normal quit. This standalone pseudoterminal check does not establish compatibility with other versions/platforms or native Ivy title presentation.

## Console-local startup

In a newly opened ordinary Ivy Bash, Zsh or Fish console, type `codex` normally. On Linux and macOS, supported starts, resumes and forks use the shared daemon while retaining each console's panel. Eligibility depends on advertised client capabilities and the live protocol, not CLI or server version numbers. Ivy does not edit shell or Codex configuration, and it preserves the daemon's remote-control preference. To enable remote control, run `codex remote-control start`; a connected host alone does not prove that a phone can load the live history or send input. Phone history, input and MCP panel publication were verified with CLI/server 0.167.2, including after saved-thread resume; verify them on your own device.

Ivy never adds `--no-daemon` automatically. Missing capabilities or a failed shared connection stop the launch with a diagnostic. Other platforms and advanced options preserve the original command with a panel-routing diagnostic. Native macOS two-console panel isolation and real phone history/input must be verified separately; Linux fixture results do not establish that interoperability. Explicit `--no-daemon`, remote and administrative commands retain their behavior. Existing aliases or functions take precedence with a diagnostic; explicit `command codex`, absolute paths, custom shell arguments and unsupported shells bypass integration. If a session from before this integration is still active and cannot be safely attached, leave its old desktop session before resuming. The [connection guide](../../../README.md#codex-forward-identity-and-check-the-launch-mode) explains MCP registration and verification.
