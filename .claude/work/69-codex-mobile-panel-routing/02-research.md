# Research evidence — 13 November 2026

## Scope and limits
Read-only investigation of Ivy main afef5c75e0f94cc9156501df613bafbaa7986217 and the available Codex 0.163.0. The ticket/README report 0.167.0 on the owner host; that executable and an Android device are unavailable here. No real Ivy GUI, daemon lifecycle command, phone pairing, model turn, authentication read, or user configuration change was performed. No native two-console or mobile success is claimed.

## Confirmed local mechanism
- `crates/panel-control/src/bin/ivy-codex.rs`: `classify` recognizes start/resume/fork as interactive; `supports` checks help; `main` prepends `--no-daemon`. Explicit `--remote` and administrative `remote-control` pass through. `--enable/--disable` are consumed as option/value pairs; changing a feature flag does not defeat the separate injected launch argument.
- `resources/agent-integrations/codex/shell/{bash,zsh,fish}` and `crates/app/src/shell_integration.rs`: wrapper is local to the console; existing aliases/functions win. No launcher config writer exists.
- README.md and `crates/app/src/chrome/connection.rs` explain env forwarding and local isolation but omit the active-writer/mobile limitation. The copied `setup_prompt` repeats that omission, so changing only visible manual text is insufficient.
- `integrations/session-recovery/README.md` and native Codex probe constrain recovery to direct independent sessions. Changing launch topology would need renewed recovery verification.
- `docs/02-decisions.md` Codex setup clarification (around 8730) explains why a global daemon restart, saved pane identity and MCP pane selectors were rejected. `docs/03-architecture.md` lines 139–155 ties routing to the connection endpoint; identity names are not credentials.
- The ticket supplies the active-writer log. It is reported failure evidence, not a local reproduction or an upstream diagnosis. Inference: a separate live writer is incompatible with a second server trying to resume that same thread in this topology; a connected host does not establish conversation access.

## Read-only local capability checks
See codex-version.txt, codex-help.txt, codex-resume-help.txt and codex-remote-control-help.txt. All commands exited 0. Local help advertises independent execution and remote endpoint selection. `codex features list` reports daemon_auto_start stable/true here; this says nothing about the owner host configuration. The owner reports false. Ivy writes neither value, and the launcher argument controls its own independent launch separately.
`codex app-server generate-json-schema --experimental --out <work>/protocol-0.163.0` exited 0. ThreadStartParams, ThreadResumeParams and ThreadForkParams expose a generic `config` object. ThreadStartParams also has execution `environments`, not a documented invoking-console MCP environment field. This keeps per-thread config overrides a candidate; schemas alone prove neither the CLI forwards them as needed nor isolation/rebinding on resume or mobile reattachment. Do not label such an override unsupported or verified from this evidence.

## Official sources actually opened
- [CLI reference](https://learn.chatgpt.com/docs/cli/reference): distinguishes independent interactive execution, explicit server attachment and remote-control lifecycle. Its current content is not a version-pinned guarantee for 0.167.0.
- [MCP configuration](https://learn.chatgpt.com/docs/extend/mcp?surface=cli): stdio environment forwarding reads the configured environment source; it does not establish per-console propagation through a shared daemon.
- [Configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference): MCP env and env_vars entries exist. No daemon_auto_start entry found in this page; local feature listing is the evidence for that flag.
- [App Server](https://learn.chatgpt.com/docs/app-server): thread-scoped protocol and MCP tool calls exist, but this is not a verified ordinary CLI startup solution.
- [Remote connections](https://learn.chatgpt.com/docs/remote-connections): mobile setup guidance concerns supported desktop hosts; it does not establish the ticket’s Linux CLI topology.
Current official pages and local 0.163.0 differ in age/surface; absence of a documented solution is not proof of impossibility in 0.167.0 or newer.

## Alternatives evaluated
1. Remove --no-daemon: rejected. It abandons the existing environment safeguard with no two-console proof.
2. Enable daemon_auto_start: insufficient by itself while the argument is injected; persistent edits violate the acceptance criterion.
3. Per-invocation `-c mcp_servers.<existing-name>.env=...`: plausible research direction, not an approved implementation. Must check exact target-version CLI/daemon propagation, existing-server-name resolution, config persistence, resume/fork rebinding and remote reconnection. Do not save actual PANEL values. No claim that it works or fails.
4. Per-console remote-control server/home or proxy: no verified phone host selection and attachment; risks auth/config divergence, extra lifecycle management and recovery regressions. Not justified by this evidence.
5. Document the current limitation and choices: supported by the ticket fallback. Keep launcher policy. Explain local independent panels versus explicitly chosen shared/remote topology whose Ivy panel routing remains unverified; never present the latter as satisfying both requirements.

## Host verification protocol for any later joint-support claim
Record exact Codex/Ivy/mobile versions, OS, launch argv and daemon flag state without dumping secrets. In two real Ivy consoles A and B, ordinary start then exact resume in each must publish different markers only in its originating panel; alternate updates while both stay live. On a real phone open A while A remains active, load prior messages, send input, observe response on both clients and no active-writer failure; repeat for B and resume. Test reassociation to a fresh console, explicit remote launches and existing aliases/functions. Compare relevant config bytes before/after. Connected status alone fails this check. If no host/device is available, mark these checks unverified.

## Additional exact-version capability evidence
After challenge, fetched the official npm package @openai/codex@0.167.0-linux-x64 into /tmp/ivy-69-codex-0167 without replacing the installed client or changing configuration. Executed only --version, --help, resume --help, remote-control --help and app-server generate-json-schema --experimental. All exited 0; target-version.txt confirms codex-cli 0.167.0. target-help.txt / target-resume-help.txt / target-remote-help.txt and target-schema-summary.json preserve results. InitializeParams exposes clientInfo/capabilities; start/resume/fork expose generic config. There is still no live daemon, MCP override propagation, two-console or phone test. This resolves the older-executable evidence gap for capability checks only, not the runtime question. No claim that 0.167.0 cannot support both is justified. The actual owner host is still unavailable.
