# OpenCode TUI integration

This asset targets the official OpenCode **1.18.31 TUI plugin** API. It does not use the directory-wide server plugin surface. Ivy embeds the asset and installs it only after the user chooses **Install / repair OpenCode plugin** in **Connect agents → Activity and attention**.

Add the copied absolute file path to the existing `plugin` array in OpenCode's `tui.json`, preserving its other entries. Restart OpenCode in the intended Ivy console. Do not hard-code pane variables: the console supplies them. The plugin is inactive without its complete inherited identity.

On Windows, update Ivy and use **Install / repair OpenCode plugin** before restarting OpenCode. The adapter uses the bundled `ivy-session.exe` at the absolute `IVY_SESSION_HELPER` location supplied by Ivy, with `PANEL_ID`, `PANEL_DIR` and `PANEL_SERVER_PID`. It never searches PATH, invokes a shell, or installs a separate dependency. Missing or invalid helper configuration leaves the adapter inactive. Linux and macOS retain the direct Unix socket transport.

The visible TUI route scopes activity to its session. Only requests shown for that root or its direct children contribute input attention. Local SDK pending lists reconcile the TUI's event-maintained getters, including cancellation. Pending hydration never means no request. Failed or incomplete refresh marks the preceding declaration unconfirmed; it does not answer it. Refreshes and socket writes are serialized and bounded. No prompts, answers or command arguments leave the plugin.

The plugin uses `solid-js` from the host's supported plugin environment and Node-compatible socket/path APIs. No provider API or credentials are added.

## Verification

Run the executable adapter fixture:

```sh
node --test resources/agent-integrations/opencode/plugin.test.mjs
node --test resources/agent-integrations/opencode/windows-helper.test.mjs
IVY_REAL_SOCKET=1 node --test resources/agent-integrations/opencode/plugin.test.mjs
```

The first command uses a simulated SDK/reactive runtime and transport; the third substitutes a real temporary Unix socket. They execute the actual asset, including request cancellation, delayed navigation, failed refresh, a 100-event storm, fresh reconnect after an outage, silence after a permanent wire refusal, and at least nine consecutive transient delivery failures on the same connection. Use a Node runtime with native TypeScript support. A permanent wire refusal stops automatic reconnect; transient queue-full/timeouts retry a fresh snapshot on the same connection. Enable the integration and restart OpenCode after a permanent refusal. These fixtures prove local adapter behavior, not real-client permission/question interoperability.

Additional live checks on Linux on 2026-11-09 used the actual 1.18.31 TUI and this asset inside Ivy: a permission approved once, questions answered and canceled, background-tab attention withdrawal, two independent question waits on one server/directory, and null withdrawal on normal quit. Canceling one pane's question preserved the other pane's wait. Additional failure/ordering cases remain fixture evidence. Version/platform scope is documented in `docs/08-configuration.md`.

The Windows helper fixtures execute the actual asset with a controlled child transport. The native witness runs the actual plugin, actual helper and two real panel channels:

```powershell
cargo build --locked -p panel-control --bin ivy-session
$fixture = (Resolve-Path resources/agent-integrations/opencode/native-helper.test.mjs).Path
$helper = (Resolve-Path target/debug/ivy-session.exe).Path
cargo run --locked -p panel-control --example opencode-signals -- $fixture $helper
```

Its passing marker requires busy, simultaneous waits, clearing one pane while the other still waits, and explicit withdrawal from both panes. Windows CI runs this independently from the Rust suite. Native fixture results do not establish installed OpenCode desktop interoperability: permission/question approval, denial, reply, cancellation, background tabs, normal quit and absence of a stray console window still need an installed-client trial. No Windows live-client claim is made here. Conversation recovery and its reporters remain deferred under #174.

First-party contract sources:

- https://github.com/anomalyco/opencode/blob/v1.18.31/packages/opencode/specs/tui-plugins.md
- https://github.com/anomalyco/opencode/blob/v1.18.31/packages/tui/src/plugin/adapters.tsx
- https://github.com/anomalyco/opencode/blob/v1.18.31/packages/tui/src/context/sync.tsx
- https://github.com/anomalyco/opencode/blob/v1.18.31/packages/tui/src/routes/session/index.tsx
