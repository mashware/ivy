# Inventory #57 — reusable pieces (paths relative to worktree .worktrees/57-check-panel-dir-owner/)

Short answer: two Rust clients connect with an inherited identity: the MCP bridge (`Producer::of_this_pane`) and the session helper (`session::exchange`). One TS plugin opens `control.sock` itself. No client checks owner or mode anywhere today.

## 1. Clients that read PANEL_ID/PANEL_DIR and connect
- crates/panel-control/src/client.rs:30 — `whole_identity`: id non-empty text, dir absolute
- crates/panel-control/src/client.rs:66-108 — `of_this_pane`: whole_identity, then BINDS, then `Self::of`; any failure is "could not be reached at <dir>" (NotConnected)
- crates/panel-control/src/client.rs:113 — `Producer::of(dir,id)` → `endpoint::connect`, no checks (tests use it)
- crates/panel-control/src/endpoint.rs:124 — `name_of`: POSIX `dir/control.sock`, Windows `ivy-<id>` pipe
- crates/panel-control/src/endpoint.rs:143,183 — `BINDS` (false on Windows); `connect` = `Stream::connect(name_of)`
- crates/panel-control/src/bin/panel-mcp.rs:41,228 — handshake reads ENV_NAMES, uses whole_identity only (never opens dir)
- crates/panel-control/src/bin/panel-mcp.rs:673 — tool calls connect via `Producer::of_this_pane`
- crates/panel-control/src/session/helper.rs:70-91 — `exchange`: own rule (non-empty only, no absolute check), `name_of` + `ConnectOptions` 2 s timeout
- crates/panel-control/src/bin/ivy-session.rs:7 — pre-check that both vars are non-empty, then `session::exchange`
- crates/panel-control/src/bin/ivy-codex.rs — no socket connection found (it spawns codex only)
- resources/agent-integrations/opencode/ivy-opencode.tsx:101-108 — `PANEL_DIR`+`PANEL_ID` non-empty, not win32, `createConnection(join(dir,"control.sock"))`, retries every 1 s (:74)
- integrations/session-recovery/opencode/index.js:10, hooks/register.js:5, codex/hooks.json:8 — check only that the variables are non-empty and run the `ivy-session` helper (they inherit the helper's check)

## 2. How the directory is created
- crates/panel-contract/src/lib.rs:87 — `PanelDir::create`: `base_dir()/ivy-<8hex>`, one path component
- crates/panel-contract/src/lib.rs:171 — `create_dir`: non-recursive; fails if the directory exists; `0o700` on unix only (no mode on Windows); owner = the creating process
- crates/panel-contract/src/lib.rs:213,256 — base dir: absolute `XDG_RUNTIME_DIR`, else absolute `temp_dir()` (macOS `$TMPDIR`), else `/tmp` on Unix; Windows temporary as given
- crates/panel-contract/src/lib.rs:65 — `ENV_NAMES = ["PANEL_ID","PANEL_DIR"]`
- crates/panel-contract/src/lib.rs:151 — Drop/`remove` deletes `control.sock`, panel.json, then the directory

## 3. Existing owner/mode checks and dependencies
- No client-side uid/owner check exists. Nothing calls `getuid` or `geteuid`
- crates/panel-control/src/session/peer.rs:52-80 — server-side `SO_PEERCRED`/`SO_PEERPIDFD` via `libc` (Linux only, pid only)
- crates/app/src/panel/driving.rs:18,526 — `MetadataExt` dev/ino (app crate, not reusable as-is)
- crates/app/src/settings/store.rs:17, startup/migration.rs:19 — `symlink_metadata` checks in app
- Dependencies: panel-control = panel-contract, serde_json, serde, interprocess 2.4, `libc 0.2` on unix only (Cargo.toml:42-43). panel-contract = getrandom 0.3, serde_json only (no libc)

## 4. Tests with an inherited identity
- crates/panel-control/tests/panel_mcp_handshake.rs:61,131 — handshake/tool refusals; `nowhere()` uses nonexistent dirs (:50, "never opened by a handshake")
- panel_mcp_handshake.rs:168-176 — asserts "could not be reached" + dir name
- crates/panel-control/tests/session_helper.rs:14 (real PanelDir env), :81 (absent identity)
- crates/panel-control/src/client.rs:390 — `an_identity_is_whole_only_with_...absolute`
- crates/panel-control/src/endpoint.rs:211-231 — asserts the endpoint's directory mode is 0o700
- crates/panel-contract/src/lib.rs:416 — `the_directory_cannot_be_read_by_anyone_else` (mode 0o700)
- resources/agent-integrations/opencode/plugin.test.mjs:11,29 — `mkdtemp` directory (0700, own uid) as PANEL_DIR
- crates/panel-control/tests/ivy_codex_launch.rs:22-23 — relative PANEL_DIR, but ivy-codex never connects

## 5. Docs and decisions
- docs/02-decisions.md:684 — D7 identity; :711 base dir (D88); :712 single component; :714 explicit `0700`
- docs/02-decisions.md:6427 — premise: PANEL_DIR is created by the app, so its writers have the user's uid
- docs/02-decisions.md:9327-9353 — D88 whole identity; refusal names the directory
- docs/03-architecture.md:66 — endpoint inside the `0700` directory; :125-132 — Security table (PANEL_ID is a name, not a credential; the `0700` does the work); D43/D44 named there
- crates/panel-control/src/endpoint.rs:147-155 — Windows is not bound until F2.7 (the 0700 boundary does not exist there)
