---
id: asp_five-ways-a-queued-shape-is-never-drawn_r3b9cp
slug: five-ways-a-queued-shape-is-never-drawn
name: >-
  A shape can reach egui's queue with the right colour, size, width and stroke
  kind and still be invisible — five ways, and a test that asks two of the
  questions passes over a blank screen
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:54:33.573Z'
updated_at: '2026-09-09T21:54:33.573Z'
last_verified: '2026-09-09T21:54:33.573Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/painter.rs
  - crates/app/src/theme.rs
symbols:
  - the_only_rectangle_stroked_in
  - paint_the_question_over_it
content_hashes:
  crates/app/src/lib.rs: 'sha256:e8bd846a3440e53be756ff25a7fc65376922d1ef1119dff8d912594f58492213'
  crates/app/src/painter.rs: 'sha256:8ffdf3a61b18c3a0a9c7c55f78d2b7591595b8c60b3b4259875a66477f3a697d'
  crates/app/src/theme.rs: 'sha256:07d9aafc484c6e632bda93a5e2e14e89cad94bb61522797aeae43a6dcf50d0a5'
tags:
  - egui
  - epaint
  - testing
  - D30
feature_id: feat_term-core_nx0g1r
---

## What it does

**Found by pushing an adversarial pass at the ASSERTIONS after they were written, not at the code.**

1. **The clip.** Putting a shape on the queue does **not** drop one whose clip is empty. `with_clip_rect(Rect::NOTHING)` leaves the `ClippedShape` complete and the tessellator is what skips it. Handing a mark to a neighbour's painter is the same defect wearing the wrong console. Only the cursor's own lookup read `clipped.clip_rect` before this.
2. **The queue order.** A notice's box is `rect_filled` then `rect_stroke` over the same rectangle with the same corner radius. **Swap the two calls** and the fill covers a one-point `Inside` border entirely: two shapes on the queue, both correct, no border on screen. Nothing in this crate asserted the order two shapes are queued in.
3. **The alpha, through the search key.** A mark looked up *by* `theme.focus()` is invisible if that palette entry is given an alpha of 1 — the search and the subject move together and every assertion holds. This is [[an-expectation-computed-from-what-you-mutate-pins-nothing]] arriving on a different mark: there the trap was two palette entries holding the same grey, here it is that the expectation and the search key are the same value. Fixed by writing the colour out.
4. **The width, from above.** A floor of `>= 1.0` calls a fifty-point band correct. And a **relative** ceiling does not close it either — fifty is a fifth of a three-hundred-point console, so any fraction loose enough to be principled is loose enough to pass. The exact width is what binds.
5. **The variant the test reads.** A painting walked by a dozen tests can still carry **no assertion at all**, because a test that filters the shape queue by one variant is blind to every other. Filtering by `Shape::Text` is the ordinary way to assert what a console says, and it makes every rectangle on the same path invisible for free. «Is there a test on this path?» is the wrong question; **«what does that test read off the path?»** is the right one. Measured: a `rect_stroke` could be **deleted outright** with the whole suite green.

> **«Is the shape queued, and is it right?» is five questions, and a test that asks two of them passes over a blank screen.**

Point 5 cost a false claim in a design document, a delivery plan, a work summary and a public issue comment before review caught it: the first diagnosis was «nothing walks this path», and the two readings lead to two different repairs — *build a scenario* versus *assert the thing nobody looked at*.

## Where it lives

`crates/app/src/lib.rs` — `the_only_rectangle_stroked_in` is the shared lookup that pins colour, corner, thickness, stroke kind, clip and uniqueness at once, so every caller inherits all six.
