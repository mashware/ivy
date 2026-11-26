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

## Windows route storage and shared-panel limitation

Windows can store private route records, but Ivy's shared-panel relay is not supported there.
Ordinary interactive launches explain that Ivy cannot yet verify the owning console for a shared
conversation, then run the original Codex command with its arguments and exit status unchanged.
Ivy does not add `--no-daemon`, start a substitute daemon or edit your Codex settings.

The official Codex daemon and proxy do document Windows support through an AF_UNIX path, not a
named pipe ([daemon documentation](https://github.com/openai/codex/blob/rust-v0.167.2/codex-rs/app-server-daemon/README.md)).
That upstream capability is separate from Ivy's pane routing: the route currently lacks the
expected Ivy server PID required by the Windows authenticated connection. Native console
interruption, daemon/proxy attachment and two-console/phone interoperability must also be verified
before enabling the relay. Storage tests do not prove that a phone and a local panel share one
conversation. This limitation does not disable independently configured direct MCP connections.

Route storage requires current-user-owned directories and files with a protected user/SYSTEM-only
DACL. Insecure existing storage is refused, not repaired automatically. Runtime route files and
their directory cannot be reparse-backed (including cloud/filter-backed storage); choose an ordinary
local Codex home. The route JSON stays at version 1. Experimental session recovery is outside this
storage support; no session-transfer or restore behavior is added.

See [native route verification](../../../docs/windows-codex-route-checks.md) for CI commands and
the privileged wrong-owner check.
