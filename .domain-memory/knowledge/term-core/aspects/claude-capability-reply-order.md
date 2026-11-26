---
id: asp_claude-capability-reply-order_ehytv1
slug: claude-capability-reply-order
name: >-
  Claude ends native capability negotiation on its second device-attributes
  reply
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-26T14:25:24.608Z'
updated_at: '2026-11-26T14:25:24.608Z'
last_verified: '2026-11-26T14:25:24.608Z'
file_paths:
  - crates/term-core/src/screen.rs
  - crates/term-core/src/screen/metadata.rs
  - crates/term-core/src/screen/title.rs
  - crates/app/src/pty_output.rs
  - crates/app/src/tests/agent_signals/native_transport.rs
  - crates/app/src/launch/tests.rs
symbols: []
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:032b70298a589df6ed03785adb07d1001fc61eb81e81ec864e91d1af0e7bddb4'
  crates/term-core/src/screen/metadata.rs: 'sha256:a238d7ddb72f703fbf7cc187ac08e4d4b8f93ddb9f97a2ef0929ad53418ba9ac'
  crates/term-core/src/screen/title.rs: 'sha256:928becf4e7b0c72b9ed816621dc785d45335763b2d5931d2fbd967a517190a9d'
  crates/app/src/pty_output.rs: 'sha256:c3b9c97dca89b8d144b1611f5fb570ea3a1630f644c15af0534a20e7a5b1c717'
  crates/app/src/tests/agent_signals/native_transport.rs: 'sha256:819deb785e74ef964dd02d3bb30846b8f363577ecc9b39739b9c53f967ed5691'
  crates/app/src/launch/tests.rs: 'sha256:05aae4cc968607ed724c809d98b8a60ec60c62ec1a3f612781e4bf16fc168ae9'
tags:
  - claude
  - pty
  - osc
  - terminal-replies
feature_id: feat_term-core_nx0g1r
---

## What it does

Claude Code 2.1.332 on Linux asks for its OSC 7501 capability and then sends a second primary device-attributes query. The probes can arrive together. After the normal first device-attributes reply, Claude sends a native declaration if the capability reply precedes the second device-attributes reply (3/3). Reversing them in the same write or delaying the capability reply by 20 ms produced no declaration (0/6). A lone capability probe can pass while the real startup fails.

Collect the fixed reply at its query position inside the ordered terminal batch, including across PTY read splits. Give that single response a separate fixed allowance so it cannot make a valid 16 KiB ordinary reply batch fail. Identity and activity still originate in explicit client declarations. See D80.

## Where it lives

The terminal screen assembles replies and the pane pump hands them to the existing ordered writer. The PTY regression sends the complete observed startup sequence. A second fixture covers the buffered startup path.
