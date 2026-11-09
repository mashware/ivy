# Codex title selection

Codex 0.167.0 publishes native lifecycle independently of its terminal title. To retain conversation/project names without duplicate activity, merge this key into the existing `[tui]` table of the effective Codex `config.toml`, normally `~/.codex/config.toml`:

```toml
[tui]
terminal_title = ["thread-name", "project-name"]
```

If the key already contains customized items, preserve their order and remove `activity` (including its `spinner` alias). Optional `run-state`/`status` also adds lifecycle text. Do not create a duplicate table or overwrite unrelated keys. Restart Codex and resume the desired conversation. Ivy previews this setting and never edits the file.

Pinned contracts: [schema](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/core/config.schema.json), [title renderer](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/tui/src/chatwidget/status_surfaces.rs), [native status](https://github.com/openai/codex/blob/rust-v0.167.0/codex-rs/tui/src/terminal_program_status.rs). Real-client title and permission/question behavior must still be verified.
