---
id: asp_a-second-outcome-with-the-same-field-blinds-old-tests_dzpg7v
slug: a-second-outcome-with-the-same-field-blinds-old-tests
name: >-
  A test disguise: when a second outcome leaves the same field in the same
  state, every test asserting on that field stops telling the two apart
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T21:25:18.524Z'
updated_at: '2026-09-25T21:25:18.524Z'
last_verified: '2026-09-25T21:25:18.524Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/painter/notices.rs
symbols:
  - the_notice_box
  - the_question_box_with_no_button
  - hold_a_paste_on
content_hashes:
  crates/app/src/lib.rs: 'sha256:def487034867d993c3bc3caa084d5e0a5ef05e2cb9e7f6d55ea2195164e51e51'
  crates/app/src/painter/notices.rs: 'sha256:ad4f8857c2c5e495f094b96f491b54045f30df3e6b45d3a63c7b09a88ea7cee3'
tags:
  - test-disguise
  - D30
  - held-paste
  - review-finding
feature_id: feat_term-core_nx0g1r
---

## What it does

**The shape.** The held-paste click tests pressed the centre of the notice over a `sleep 30` console and asserted only `held.is_none()`. That was a sound assertion while a click could only discard. #199 added a «Send it anyway» button that **also** clears `held` — it sends and lets the paste go — so a press landing on the wrong target satisfied the same assertion, and a sleeping program receives nothing either way.

**It became live, not theoretical.** The helper `the_notice_box` computed the **unshortened** question box, while production shortens the question to make room for the button; the old box's centre could fall on the button. Every click test would then have sent instead of discarded and stayed green. Found by the built-in code review.

**Fix that shipped.** The helper now asks `the_boxes(..).question` — what the painter and the mouse pass really use — and the unshortened box is kept test-only as `the_question_box_with_no_button`, for the tests about the question's own ceiling and floor. A new test proves a click on the question sends nothing by clocking the absence with an **arrival**: a raw `cat` writes to a file, a second paste is held and sent by the button, and the file must hold only the second one.

**Rule.** When a change adds an outcome that leaves an existing field in the same state as an existing outcome, re-read every test that asserts on that field: assert on the effect that differs (bytes a program writes to a file), or prove where the press landed. See [[a-revocation-test-must-drive-the-door-not-the-setter]] and [[the-echo-forges-the-evidence-a-test-reads]].

## Where it lives

crates/app/src/lib.rs tests (the_notice_box, a_click_on_the_question_discards_the_paste_and_the_program_receives_none_of_it); crates/app/src/painter/notices.rs (the_question_box_with_no_button)
