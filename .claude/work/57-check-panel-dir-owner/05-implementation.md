# Implementation 57

## Brief PR #1
recorded (unattended)
- Agents refuse inherited console directories that another user could control.
- Genuine consoles continue receiving panel and session updates.
- Refusals identify the directory and distinguish a closed console.

**This PR does NOT include**:
- Peer-uid authentication or ancestor directory hardening.
- File producers controlled by external scripts.
- Windows channel support or helper identity redesign.

## Contracts to respect (copied verbatim from 03-design.md §"External contracts")
### Refusal text
Existing JSON-RPC tool result shape is unchanged: `result.isError = true`, `result.content[0].type = "text"`; text contains the original pathname.
Existing ivy-session shape is unchanged: `{"ok":false,"error":"helper_error","message":"<detail>"}`, exit 1, stderr prefix `ivy-session: `. Keep its existing `outside_ivy` response for absent identity.
For an existing insecure path, detail contains: `<dir> is not a private directory of this user`.
For a missing path, detail contains: `this console's panel could not be reached at <dir>` and `directory is gone`; never mislabel it as an insecure directory.
Other metadata failures preserve path and system cause and fail closed. No raw metadata failure falls through to connect.
Consumers: MCP tool callers, helper adapters, and subprocess test fixtures; no sibling-repo contract.
### Direct OpenCode retry
`setTimeout(connect, 1000)` after a metadata refusal; `createConnection(...)` is called only after success. A refused directory must not trigger the connected callback or send cached frames. `close()` cancels retry and prevents further attempts.


## Plan PR #1
- [x] 1. Shared Rust guard and metadata/spelling tests — endpoint.rs; endpoint unit tests.
- [x] 2. Rust inherited clients and error propagation — client.rs, helper.rs, ivy-session.rs; subprocess listener tests.
- [x] 3. Direct plugin per-attempt validation — ivy-opencode.tsx, plugin.test.mjs; controlled retry and real socket fixtures.
- [x] 4. Security documentation — D7 and architecture; textual contract audit.
Point of no return: none. Out of plan: peer authentication, external file producers, Windows channels.

## Steps

## Deviations from design
None.

## Steps completed
- 214ba0e · Shared guard and metadata/spelling matrix · 1 file, +100 −5
- 2241ef6 · Rust clients and subprocess no-connect proof · 4 files, +108 −6
- abc7a2b · Direct plugin and retry tests · 2 files, +90 −3
- ec654cd · Security documentation · 2 files, +18 −0

## Contract verification
- MCP refusal retains result.isError and text content; subprocess test checks both.
- ivy-session retains helper_error JSON, exit 1 and stderr prefix; underlying detail survives.
- Missing directory names the original path and says directory is gone; no privacy mislabel.
- Direct plugin checks before createConnection and schedules setTimeout(connect, 1000); close cancels retry.

## Relevant commands executed
- cargo test --locked -p panel-control: passed, including new unit and subprocess tests.
- node --test resources/agent-integrations/opencode/plugin.test.mjs: 2 passed.
- IVY_REAL_SOCKET=1 node --test resources/agent-integrations/opencode/plugin.test.mjs: 2 passed.
- npm test --prefix integrations/session-recovery: Node 22 rejects existing --test-isolation option; same command under Node 24 passed.
- Initial clippy duplicated-branch finding corrected before review.

## Decisions made during implementation
- Preserve errors for both helper metadata and publish CLI entry points, which share exchange_line.
- No external premise remains unsettled; metadata traversal behavior is exercised in both languages.

## Validation coverage completion
- bd8493f · Genuine inherited bridge success with returned document asserted · 1 test file, +41 −1.
- Focused inherited_directory run: 2 passed. No production changes.

### Red suite at validate (unattended), round 1: 1 red
- [x] the_list_still_names_every_module — MODULES has no row for new inherited_directory.rs. Add the file to the existing script-audit registry, preserving the complete coverage assertion. Fix the registry, never skip or loosen the guard.

- ca90853 · Register new Rust fixture in script-audit source list · 1 test file, +4 −0.
- Final cargo test --locked --workspace: 2101 passed, 0 failed, 7 existing ignored.
- Final Clippy and formatting: passed.
