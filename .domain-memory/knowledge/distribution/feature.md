---
id: feat_distribution_kb0x1w
slug: distribution
name: Distribution — native packages do not establish platform capability
type: feature
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-09T08:41:30.582Z'
updated_at: '2026-11-09T08:41:30.582Z'
last_verified: '2026-11-09T08:41:30.582Z'
file_paths:
  - crates/panel-control/src/endpoint.rs
  - docs/05-roadmap.md
  - docs/10-installation.md
symbols: []
content_hashes:
  crates/panel-control/src/endpoint.rs: 'sha256:6187cb590badb23444de6d06a37a183030987c0e583f8a55cacb89d7412b150b'
  docs/05-roadmap.md: 'sha256:d3c4ecec6d4a405d293f81daa5f2708229750d23f0bd6275a5eb60029a5096c5'
tags:
  - distribution
  - portability
  - windows
  - mcp
---

## What it does

Ivy's first native installers must distinguish packaged executables from supported platform
capabilities. Including panel-mcp in a Windows installer does not enable MCP: the control
acceptor explicitly returns Unsupported until F2.7. Packaging therefore documents that bridge
as unavailable on Windows and leaves native portability completion separate. An installer build
is not implementation or validation of its transport.

## Where it lives

crates/panel-control/src/endpoint.rs Windows acceptor()/Acceptor::wait() arms;
docs/05-roadmap.md F2.7; docs/10-installation.md. Design343 preserves this boundary; Linux
delivery does not claim completion of F2.7.
