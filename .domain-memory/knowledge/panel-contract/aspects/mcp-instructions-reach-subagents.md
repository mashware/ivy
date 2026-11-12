---
id: asp_mcp-instructions-reach-subagents_rh1ek8
slug: mcp-instructions-reach-subagents
name: >-
  MCP server instructions reach subagents too, and a subagent shares the
  connection that holds the pane: handshake text must tell it to leave the panel
  alone
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-12T21:36:57.151Z'
updated_at: '2026-11-12T21:36:57.151Z'
last_verified: '2026-11-12T21:36:57.151Z'
file_paths:
  - crates/panel-control/src/bin/panel-mcp.rs
  - crates/panel-control/src/client.rs
symbols:
  - INSTRUCTIONS
  - instructions_for
  - whole_identity
content_hashes:
  crates/panel-control/src/bin/panel-mcp.rs: 'sha256:186c07fc431860f1f23e9b813c8e6f852de810a669684d8b978b74e7c5701d96'
  crates/panel-control/src/client.rs: 'sha256:a0c6ddca7c500fedc69ee329ca85d32f839cd5224bf15ffa70adaf64ac7e48b5'
tags:
  - mcp
  - panel
  - D88
  - D43
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**The `instructions` an MCP server returns from `initialize` are not only for the top-level agent.** A harness like Claude Code also places them in its subagents' prompts, and a subagent's tool calls travel through the parent's MCP server process — for `panel-mcp`, the same connection that holds the pane (D43). So a text that tells «every agent» to publish makes each subagent replace the parent's whole document and its `attention`: one reviewer subagent publishing `done` erases the question the parent is waiting on. A worker process the parent starts in the same console brings a bridge of its own and is refused at every stop, or holds the pane against its parent if it publishes first.

**Rule (D88):** any instruction text sent over the handshake tells an agent working for another agent in the same console to leave the panel alone; an agent started by another in a console of its own has its own identity and keeps its panel.

**Second lesson from the same delivery:** every reader of `PANEL_ID`/`PANEL_DIR` must use the one rule, `client::whole_identity` (identifier non-empty text, directory absolute). A looser copy in the tool path made the handshake say «no console» for an empty identifier while the first tool call connected with it. The session helper still has its own rule (deferred).

Evidence: during #51's review the xhigh code-review subagent reported Bellwether's near-identical handshake text present in its own subagent prompt; the two-rule split was read in `Producer::of_this_pane` (presence only) against the new handshake gate and closed in 8a4a544 (mutant «empty id accepted» killed).

## Where it lives

crates/panel-control/src/bin/panel-mcp.rs (INSTRUCTIONS, instructions_for); crates/panel-control/src/client.rs (whole_identity, Producer::of_this_pane); docs/02-decisions.md D88
