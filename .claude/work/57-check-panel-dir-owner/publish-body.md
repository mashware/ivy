Closes #57

## Purpose
A surviving agent or tmux session can retain a closed console's identity. In a shared temporary directory, another local user could recreate that directory and receive panel documents on a planted socket. Inherited clients now refuse that path before connecting.

## What changes for the user / system
- Genuine consoles continue receiving panel and session updates.
- The MCP bridge and session helper refuse directories belonging to another effective uid, with group/other permission bits, or naming a final symlink.
- Refusals name the directory and distinguish a missing directory from an insecure one.
- The direct OpenCode plugin checks again on every one-second reconnect attempt; repairing a directory permits the next attempt.

## What is NOT included
- Peer authentication and eliminating the check/connect pathname race or hardening ancestors.
- Checks in arbitrary scripts that write panel files.
- Windows channel support or changes to session identity semantics.

## Validation
- `cargo test --locked --workspace`: 2,101 passed, 0 failed, 7 existing ignored.
- `cargo fmt --all --check`, workspace Clippy with `-D warnings`, and `cargo doc-check --locked`: passed. Doc-check reports existing private-item-link warnings.
- OpenCode simulated and real-socket fixtures: 2 tests passed in each mode; session-recovery under Node 24: 19 passed; packaging-script tests: 23 passed.
- Independent security, correctness and contract reviews: no unresolved findings. Validation added the genuine inherited bridge positive control and registered its file in the repository script audit; the full suite passed after that registration.

## Evidence
```text
Unsafe modes and final symlink spellings: bridge/helper exit with a named refusal;
nonblocking listener accept reports WouldBlock (no connection).
Wrong uid: expected-uid injection refuses in Rust and JS fixtures without chown.
Missing directory: named "directory is gone" diagnostic, no privacy mislabel.
Plugin: no connection on metadata refusal; fresh lstat every 1000 ms retry;
repair permits connection; disposal cancels retries.
Genuine console: bridge, helper and real-socket plugin exchanges succeed.
```

## Steps to test it
1. Run `cargo test --locked -p panel-control`. The subprocess refusal fixture places listeners behind unsafe directories and proves no connection was accepted; existing genuine-console tests verify successful exchanges.
2. Run `node --test resources/agent-integrations/opencode/plugin.test.mjs` and repeat with `IVY_REAL_SOCKET=1`. The controlled fixture proves fresh checks, no connection on refusal, repair/retry and disposal.
3. Run `npm test --prefix integrations/session-recovery` under Node 24 or newer.

<details>
<summary>Technical details for reviewers</summary>

- Shared Rust metadata guard at the two inherited connection doors; explicit `Producer::of` and `endpoint::connect` retain their caller-managed behavior.
- Unix predicate: `lstat`, real directory, owner = effective uid, `mode & 0o077 == 0`. Windows checker is a no-op behind the existing unsupported-channel gates.
- Metadata lookup removes only terminal slash/dot components, preserving interior `..` and the original path in diagnostics.
- OpenCode mirrors the predicate before every socket connection. Existing protocol envelopes and handshake behavior remain unchanged.
- D7 and the architecture security table describe the invariant and its limits.
- No schema changes, migrations, events or dependencies on other tickets.

</details>

Prepared by an unattended run; nobody confirmed the implementation brief during this run. The approved study supplied its scope.
