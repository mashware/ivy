# Ivy session recovery adapters

These adapters report an exact conversation ID to the Ivy pane that launched the client. They do not read prompts, responses, transcripts or session databases, submit prompts, change authentication, or restore arbitrary command-line options.

Use Ivy's **Sessions** startup mode and the `ivy-session` executable installed beside Ivy. Ivy supplies its verified absolute location in the ephemeral `IVY_SESSION_HELPER` environment variable alongside `PANEL_DIR` and `PANEL_ID`. Adapters require that location and never search the client's PATH for a helper. Linux native foreground-process verification must accept the client before a report becomes eligible for recovery. Installing an adapter is not proof of a verified connection; use Ivy's actual connection status. Unsupported hosts, nested/background clients and unsupported launch topologies remain unverified.

## Explicit setup

The examples below are additions to existing client configuration. Preserve existing keys, plugins and hooks. Ivy does not install or rewrite them automatically. Replace `/ABS/ivy/session-recovery` with the absolute directory containing this README. The app supplies the helper locator when launching its panes; it is not saved conversation metadata or a command to configure manually.

### Claude Code 2.1.319 and 2.1.321

This directory is an official Claude Mods plugin. For a temporary load, start the official CLI with:

```sh
claude --plugin-dir /ABS/ivy/session-recovery
```

For loading on future exact-ID restarts, explicitly add its directory to the official `CLAUDE_CODE_PLUGIN_DIRS` setting/environment, or install the plugin through a marketplace you manage. For example, merge this into the user's Claude settings, preserving the existing `env` object:

```json
{
  "env": {
    "CLAUDE_CODE_PLUGIN_DIRS": "/ABS/ivy/session-recovery"
  }
}
```

If that variable already lists directories, append to that list using the platform's documented separator. The adapter currently enables only the verified API versions 2.1.319 and 2.1.321. Client policy/trust can prevent a mod from loading; no bypass is used. Ivy's fixed restore command does not replay a temporary `--plugin-dir` flag.

The mod requires an official attached `terminal` surface before registration, then reads the official live `$.session.id()` after probing for a close challenge. A single serialized cycle runs at most once per second while idle, and publishes only a changed ID or a new close challenge. It withholds cwd; Ivy obtains independently validated native directory metadata. Existing **StatusLine/HUD configuration is untouched**. Outside an Ivy pane, or without a valid absolute helper locator, it creates no timer or helper process.

### OpenCode 1.18.31

Append the TUI entry to the `plugin` array in the user's `tui.json`; this is not a server plugin in `opencode.json`:

```json
{
  "plugin": ["/ABS/ivy/session-recovery/opencode/index.js"]
}
```

The official loader provides the host's `solid-js` runtime; no separate Solid installation or bundled copy is needed. The plugin uses `api.route.current`, root session metadata and the declared local directory. Home/custom routes, selected child sessions and workspace sessions withdraw the eligible ID. Route changes request a fast serialized sample; idle probes run once per second.

Only the normal local TUI and its fixed `--session ID` form are accepted. Attach, headless/server, listening-network, mini, prompt-bearing, experimental workspace and unknown CLI modes are declined. A matching local-looking cwd or loopback server URL is not accepted as proof of local execution. This conservative version-pinned check is in addition to Ivy's kernel verification.

### Codex 0.167.0 — last reported conversation

Merge the `SessionStart` entry from [codex/hooks.json](codex/hooks.json) into the official Codex hooks file, preserving existing entries, and approve the hook through Codex's ordinary trust review. Its static command is:

```text
if [ -n "$PANEL_DIR" ] && [ -n "$PANEL_ID" ] && [ -n "$IVY_SESSION_HELPER" ]; then
  case "$IVY_SESSION_HELPER" in
    /*) case "$IVY_SESSION_HELPER" in
      *[[:cntrl:]]*) ;;
      *) "$IVY_SESSION_HELPER" hook codex >/dev/null || : ;;
    esac ;;
  esac
fi
```

Use a direct local Codex invocation with `--no-daemon`. Capture also accepts up to four bounded `-c` / `--config` pairs immediately after that flag, before an optional `resume ID`; values remain opaque and are never saved or restored. Ivy restores it with `codex resume --no-daemon ID` and the client's existing authentication.

The reviewed static shell guard invokes only the quoted app-supplied absolute path. It discards the native protocol reply because Codex uses a different strict hook-output schema, and returns success even when optional recovery is disabled or unavailable. This observational hook cannot block a model turn; actual registration is still checked in Ivy. Missing pane variables, a missing/relative locator or control characters cause no execution. Hook stdin remains the official JSON; no saved command, payload interpolation or `eval` is used.

**Codex recovery uses its last accepted hook report.** In 0.167.0, SessionStart can wait until a model turn. An idle selection, clear, resume or fork can therefore leave the reported ID different from the displayed conversation. Ivy does not modify Codex, infer a selection from its screen, or substitute `--last`/`--continue`. The helper forwards only the allowlisted ID, cwd, source and event, ignoring transcript/content fields. Hook-end notifications do not clear a live root.

## Reporter and close behavior

Claude/OpenCode obtain an app-issued generation bound to the native root's lifetime. Their increasing sequence orders live samples, not helper process births. Every cycle probes before reading. A stale generation causes a new attach and a fresh read; an old sample is never replayed under the replacement generation. Helper EOF is not root-process death.

On clean Ivy exit, active Claude/OpenCode reporters must acknowledge its new challenge after a live read within the shared 1250 ms close bound. A missing acknowledgment removes AI identity from the final snapshot while retaining a usable directory. A crash uses the last successfully saved snapshot. Codex retains its explicitly limited last-report behavior.

## Validation and remaining proof

Run the adapter tests with Node.js (verified with 24.14.1):

```sh
node --test --test-isolation=none tests/*.test.js
claude plugin validate . --strict --json
```

The fixtures exercise serialized delivery, unchanged-state suppression, fresh challenge reads,
stale-generation replacement, disposal, ID/input bounds, literal absolute helper argv/stdin,
hostile PATH entries, quiet/nonfatal Codex shell execution and conservative local-route eligibility.

Installed OpenCode 1.18.31 loaded the actual TUI adapter and host Solid runtime, reported two own
sessions and home in sequence, acknowledged clean close, and reopened the same exact ID and cwd
in a fresh process without a model prompt. A separate local-server attach probe was declined.

Claude 2.1.319 passed live identity and close-acknowledgment probes. Claude 2.1.321 passed a
separately authorized one-message own conversation, assistant OK, close acknowledgment and fresh
exact-ID/cwd reopening without another message. A final no-input reopen used the actual production
plugin directory and also acknowledged close. Its public generated Mods declarations matched the
required APIs. Normal subscription authentication was used with provider/key overrides removed;
the installed native artifact was pinned through a temporary PATH and the documented auto-updater
was disabled only in that child. Personal settings, authentication and HUD were not rewritten.

A separate zero-message Claude fixture did not have a resumable record. The client's explicit
“No conversation found with session ID” result matched its versioned loader rejecting logs without
stored messages. This does not mean that every visually empty or cleared conversation is absent.
Ivy keeps the failed target and explains the failure rather than silently choosing another session.

Codex 0.167.0 passed its separately authorized one-message native hook capture, assistant OK and
exact-ID/cwd reopening in a fresh process. The reopened foreground ID was checked with the official
local `/status` command, without another model turn. Its test-only hook trust was supplied through
supported per-run configuration and confirmed by a metadata-only local `hooks/list` query; no
thread or model request was made by that query. Personal configuration stayed unchanged. The first
combined test exposed a local-status input-timing issue in its provisional loading composer; the
separate resume-only confirmation retained the positive ID/directory criteria and the used ledger. Full Claude switch/fork/compaction and native Settings/guide
mouse interaction checks also remain unverified; successful installed resume probes do not replace
them.

Official references: [Claude Mods APIs](https://code.claude.com/docs/en/plugins/mods/reference), [between-turn timers](https://code.claude.com/docs/en/plugins/mods/api#run-work-in-the-background), [explicit plugin loading](https://code.claude.com/docs/en/plugins/mods/create), [OpenCode 1.18.31 TUI plugin contract](https://github.com/anomalyco/opencode/blob/v1.18.31/packages/opencode/specs/tui-plugins.md), [Codex hooks](https://learn.chatgpt.com/docs/hooks).
