---
id: asp_copy-completion-must-follow-console-input_hrkwgg
slug: copy-completion-must-follow-console-input
name: >-
  Async Send tests must drive real frame input and preserve both sides of
  deferred Enter ordering
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-26T09:03:33.574Z'
updated_at: '2026-11-26T09:03:33.574Z'
last_verified: '2026-11-26T09:03:33.574Z'
file_paths:
  - crates/app/src/viewer.rs
  - crates/app/src/viewer/delivery.rs
  - crates/app/src/viewer/image/send.rs
  - crates/app/src/viewer/image.rs
symbols:
  - keys
  - before_keys
  - press_enter
  - poll
  - typing_in_the_copy_completion_frame_is_asked_about_before_the_line
  - an_enter_already_owed_precedes_text_typed_in_the_next_frame
content_hashes:
  crates/app/src/viewer.rs: 'sha256:ed73d9c8bb9ee623c4df7013907d7362801affd8d7ffe96ef11f3ae48f8a2d16'
  crates/app/src/viewer/delivery.rs: 'sha256:adbc759ffd870c52823262ffbbd06dbeeaf00fa823727a1415e084e7443293e7'
  crates/app/src/viewer/image/send.rs: 'sha256:c26b4f2a49c81e831275a9bd53fa7d2711ace431beb6cf893891e0fac5a5a18e'
  crates/app/src/viewer/image.rs: 'sha256:6876a565a4d988ec94fc0582d9ee15e7291b67c4785d0d1280515352a95958a3'
tags:
  - testing
  - egui
  - input-order
  - async-send
feature_id: feat_term-core_nx0g1r
---

## What it does

A test that writes through resume::write before polling an asynchronous Send cannot prove the real
frame ordering. The current frame's Text or Paste may not have reached the console dispatcher when
copy completion reads the previous draft record. Establish the late draft with an actual egui event
in the completion frame, after returning keyboard ownership through an ordinary console click.

Keep an already-owed Enter ahead of new console input, then decide unanswered copy completion after
console dispatch. Moving all delivery after input creates the inverse race: new typing is joined
before the Enter owed for an already-written line. Keep a second receiver test for line, Enter,
then next-frame text in that exact order.

In issue196, the real completion-frame Text test failed before the repair while the owed-Enter
control passed. Both passed after splitting delivery around console input. Independently moving
completion before dispatch or moving the owed Enter after dispatch made the corresponding test fail.

## Where it lives

crates/app/src/viewer.rs: keys orders the two delivery stages around console dispatch.
crates/app/src/viewer/delivery.rs: before_keys retains review insertion and the already-owed Enter.
crates/app/src/viewer/image/send.rs: press_enter and poll own their separate state transitions.
crates/app/src/viewer/image.rs: typing_in_the_copy_completion_frame_is_asked_about_before_the_line
and an_enter_already_owed_precedes_text_typed_in_the_next_frame assert raw receiver byte order.
Evidence: #196 review blocker and runtime mutations570/580; all source mutations restored.
