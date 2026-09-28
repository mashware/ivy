---
id: asp_a-closing-pane-answers-one-read-and-leaks-its-acceptor_7v8c2b
slug: a-closing-pane-answers-one-read-and-leaks-its-acceptor
name: >-
  A closing pane answers one admitted read, then ends every control thread
  without trusting its removed endpoint
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-26T22:38:22.067Z'
updated_at: '2026-09-28T09:17:11.480Z'
last_verified: '2026-09-28T09:17:11.480Z'
file_paths:
  - crates/app/src/closing.rs
  - crates/app/src/lib.rs
  - crates/app/src/watcher.rs
  - crates/panel-control/src/endpoint.rs
  - crates/panel-control/src/listener.rs
symbols:
  - 'Channel::drop'
  - 'Acceptor::accept'
  - 'Watcher::stop_before_retirement'
  - 'Window::rearm_the_watcher'
  - 'Window::close_a_pane_called'
content_hashes:
  crates/app/src/closing.rs: 'sha256:384c4cc57704fbf896d1d07bfd454e3ef386bb77298b73f2dbc115ba061aa7f7'
  crates/app/src/lib.rs: 'sha256:4099eb8634bd823c63a28cb328bb1c5673382a4638bf6dc6ed63a7c16164941f'
  crates/app/src/watcher.rs: 'sha256:79a2ca4c68153883ba50a63646a542c181452c90a940ce7fa2ea1e2c872b3b40'
  crates/panel-control/src/endpoint.rs: 'sha256:6187cb590badb23444de6d06a37a183030987c0e583f8a55cacb89d7412b150b'
  crates/panel-control/src/listener.rs: 'sha256:d82ef415e840d3e0fdfca273196496e43fbd4255dae6c79a8313e87d0f521947'
tags:
  - panel-control
  - lifecycle
  - shutdown
  - watcher
  - regression
feature_id: feat_panel-contract_8y2gc0
---

## What it does

A closing connection's rule is tied to its read loop, not to an admission instant. One eligible `panel_get` may still be answered after the stop flag is observed, and the thread exits after the first socket read begun with that flag already raised; later traffic fails with the socket's ordinary close semantics.

The acceptor itself must end independently of the public `control.sock` pathname. Pane destruction removes `PanelDir` first, so a pathname self-connect from `Channel::drop` becomes unreachable and leaks one acceptor thread and listening descriptor per close. Keeping the path alive is still unsafe because that connect is synchronous and deadline-free on the interface thread.

The channel therefore owns a private stream-pair stopper and the acceptor polls it beside the public listener. Dropping the stopper wakes the acceptor after the public endpoint and directory are already gone, with no client connect.

Intentional pane retirement is distinct from genuine watcher failure. The current watcher generation is made inactive before its asynchronous backend is dropped and before watched directories disappear, then survivors are rearmed and flagged for reread. A queued event from the retired backend is ignored, while unexpected directory loss for a live pane still activates the global reread fallback.

Regression coverage must execute production-shaped destruction, place the destructor behind an external deadline, retain the acceptor witness after endpoint removal, and deterministically record whether any watched directory was already missing when its generation retired. A retirement counter alone cannot prove ordering because rearm also stops the old generation.

## Where it lives

The control-channel shutdown lives in crates/panel-control/src/endpoint.rs and listener.rs. Ordered watcher retirement and all pane, tab, remote-session and whole-window close paths live in crates/app/src/closing.rs, lib.rs and watcher.rs.
