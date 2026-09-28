---
id: asp_panel-id-is-a-name-not-a-credential_5tn5br
slug: panel-id-is-a-name-not-a-credential
name: >-
  `PANEL_ID` is a name, not a credential: every descendant inherits it and any
  sibling reads it, so an API «scoped by PANEL_ID» is scoped by nothing
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:21:27.942Z'
updated_at: '2026-09-11T20:21:27.942Z'
last_verified: '2026-09-11T20:21:27.942Z'
file_paths:
  - crates/panel-contract/src/lib.rs
  - crates/app/src/lib.rs
  - docs/03-architecture.md
symbols:
  - 'PanelDir::env'
  - 'PanelDir::create'
  - ENV_NAMES
content_hashes:
  crates/panel-contract/src/lib.rs: 'sha256:89f2403cb9e6b68c083cc6b3305065b05da811214801cb502ad15341310a9e5a'
  crates/app/src/lib.rs: 'sha256:171c1421de9c4a6d0646eb0c98664b64626350b32415c7e52c7c788d0838ed09'
  docs/03-architecture.md: 'sha256:ab0843c9bbfa404b3b248fecd83bcbca5d8e6b922ae18cc6abae877bd4cd11d8'
tags:
  - security
  - phase-3
  - mcp
  - scoping
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**`PANEL_ID` cannot authenticate anything.** `PanelDir::env()` puts it in the child's **environment** (`crates/panel-contract/src/lib.rs:91`), so every descendant of that pane inherits it — the shell, the agent, every tool the agent spawns, anything the user runs by hand there. On Linux `/proc/<pid>/environ` is readable by the same uid, so **any process of this user can enumerate every pane's id without being in any pane at all**. And it is eight hex digits drawn once at launch, sized for *collision avoidance* and not for unguessability — its own docblock says 32 bits because «a collision leaves a split without an identity».

> **A value that every descendant inherits and any sibling can read is a name. Authenticating with it authenticates nothing.**

The architecture document's security rule — *«MCP is scoped by `PANEL_ID`: an agent reaches its own pane and no other»* — is therefore a statement about *which* pane, never a check that can be enforced from the message.

**The shape that does work, decided in the design of 226:** the pane a call acts on is **the endpoint the connection managed to open**, one per pane inside its own `0700` directory, and the id **does not exist in the wire format at all**. Not «carry it and check it» — absent. A field that exists is a field somebody starts trusting, and the check that validates it is the kind a later refactor drops without a test noticing. It rests on `PanelDir::create()` being non-recursive, whose docblock already says creation *is* the uniqueness check.

Two consequences worth knowing before designing anything here:

- **The scoping tests then need no environment at all** — they connect to two endpoints and assert each reaches one pane — which retires the `set_var` hazard rather than working around it. See [[set-var-is-undefined-in-this-suite]].
- **The same reasoning kills a peer-uid check** (`SO_PEERCRED`) as a mechanism with no scenario: reaching the socket requires the `0700` to have already failed, at which point whoever broke it bypasses the check too.

## Where it lives

`crates/panel-contract/src/lib.rs` — `ENV_NAMES` (`:45`), `PanelDir::create` (`:67`), `PanelDir::id` (`:75`), `PanelDir::env` (`:91`). The security rule it refutes is in `docs/03-architecture.md`, channel 2 and the Security table.
