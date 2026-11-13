# Ticket 69 — target-host evidence needed

The study has not selected a compatible runtime implementation. These checks qualify a candidate; they are not a supported setup recipe. Keep the current Ivy safeguard until isolation is established. Do not publish credentials, pairing codes, authentication files, or full private conversation history.

## First resolve the ordinary launch contract
Record Ivy, Codex, desktop OS and mobile client versions. On the target version, identify a supported ordinary CLI start/resume path that both uses the shared app-server and conveys per-console MCP identity. Record actual launch arguments, server ownership, and MCP process parent/identity. Preserve the existing MCP server name and user config. Saved probes for 0.167.0 show that a single identity override switched MCP ownership to the TUI; an explicit remote launch sent config:{}.

## Acceptance observations
| Check | Required observation |
|---|---|
| Ordinary start A/B | Start two real Ivy consoles; alternating distinct panel markers affect only the originating console while both conversations remain live. Record thread IDs and ownership without private history. |
| Exact saved-thread resume A/B | Create real saved history, exit/reopen each selected thread, and repeat alternating markers. An empty pre-turn thread is insufficient. |
| Fresh-console reassociation | Resume one saved thread in a fresh console; updates reach its new panel and leave the other active console unchanged. |
| Mobile on live start | While A remains active on desktop, load its prior messages on a real phone and send a harmless input; observe the same response on both clients and no active-writer error. Repeat for B. |
| Mobile after resume | Repeat history/input checks while each resumed desktop conversation remains live. |
| Existing setup | Existing shell aliases/functions retain precedence, explicit remote arguments pass through, and relevant user config bytes remain unchanged. |

For failures, include the precise action and a redacted error excerpt. A connected host, synthetic MCP identity or direct protocol response alone does not satisfy live mobile access or two-console routing. Provide host access or these measured traces and relaunch as study; do not approve an incomplete design as an implementation plan.
