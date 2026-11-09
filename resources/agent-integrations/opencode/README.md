# OpenCode TUI integration

This asset targets the official OpenCode **1.18.31 TUI plugin** API. It does not use the directory-wide server plugin surface. Ivy embeds the asset and installs it only after the user chooses **Install / repair OpenCode plugin** in **Connect agents → Activity and attention**.

Add the copied absolute file path to the existing `plugin` array in OpenCode's `tui.json`, preserving its other entries. Restart OpenCode in the intended Ivy console. Do not hard-code `PANEL_ID` or `PANEL_DIR`: the console supplies them. The plugin is inactive without those variables or on Windows, whose endpoint is not yet supported by this adapter.

The visible TUI route scopes activity to its session. Only requests shown for that root or its direct children contribute input attention. Local SDK pending lists reconcile the TUI's event-maintained getters, including cancellation. Pending hydration never means no request. Failed or incomplete refresh marks the preceding declaration unconfirmed; it does not answer it. Refreshes and socket writes are serialized and bounded. No prompts, answers or command arguments leave the plugin.

The plugin uses `solid-js` from the host's supported plugin environment and Node-compatible socket/path APIs. No provider API or credentials are added.

## Verification

Run the executable adapter fixture:

```sh
node --test resources/agent-integrations/opencode/plugin.test.mjs
```

It executes the actual asset with a simulated SDK/reactive runtime and a temporary Unix socket, including request cancellation, delayed navigation, reconnect, failed refresh and a 100-event storm. This proves local adapter behavior, not real-client permission/question interoperability. The support status and required live checks are documented in `docs/08-configuration.md`.

First-party contract sources:

- https://github.com/anomalyco/opencode/blob/v1.18.31/packages/opencode/specs/tui-plugins.md
- https://github.com/anomalyco/opencode/blob/v1.18.31/packages/tui/src/plugin/adapters.tsx
- https://github.com/anomalyco/opencode/blob/v1.18.31/packages/tui/src/context/sync.tsx
- https://github.com/anomalyco/opencode/blob/v1.18.31/packages/tui/src/routes/session/index.tsx
