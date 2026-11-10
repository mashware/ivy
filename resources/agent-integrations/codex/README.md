# Codex title selection

Codex 0.167.0 publishes native lifecycle independently of its terminal title. To retain conversation/project names without duplicate activity, merge this key into the existing `[tui]` table of the effective Codex `config.toml`, normally `~/.codex/config.toml`:

```toml
[tui]
terminal_title = ["thread-name", "project-name"]
```

If the key already contains customized items, preserve their order and remove `activity` (including its `spinner` alias). Optional `run-state`/`status` also adds lifecycle text. Do not create a duplicate table or overwrite unrelated keys. Restart Codex and resume the desired conversation. Ivy previews this setting and never edits the file.

Pinned contracts: [schema](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/core/config.schema.json), [title renderer](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/tui/src/chatwidget/status_surfaces.rs), [native status](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/tui/src/terminal_program_status.rs). On Linux on 2026-11-09, the actual official 0.167.0 TUI emitted native permission and Plan-mode question waits, withdrew them after answering, and cleared on normal quit. This standalone pseudoterminal check does not establish compatibility with other versions/platforms or native Ivy title presentation.

## Console-local startup

In a newly opened ordinary Ivy Bash, Zsh or Fish console, type `codex` normally. Ivy checks the client and starts supported local interactive sessions independently without changing shell or Codex files. Codex 0.167.0 advertises the required `--no-daemon` option. Existing aliases or functions take precedence with a diagnostic; explicit `command codex`, absolute paths, custom shell arguments and unsupported shells bypass integration. Remote and administrative commands pass through. The [connection guide](../../../README.md#codex-forward-identity-and-check-the-launch-mode) explains the required MCP registration and version-checked fallback.
