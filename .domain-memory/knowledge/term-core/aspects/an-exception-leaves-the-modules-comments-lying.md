---
id: asp_an-exception-leaves-the-modules-comments-lying_wzktmy
slug: an-exception-leaves-the-modules-comments-lying
name: >-
  A module that gains ONE named exception leaves its comments lying in cascade:
  eleven in a single diff, and the header first of all
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T12:42:25.647Z'
updated_at: '2026-09-06T12:42:25.647Z'
last_verified: '2026-09-06T12:42:25.647Z'
file_paths:
  - crates/app/src/fonts.rs
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
symbols:
  - Discoverer
  - use_the_family
  - ensure_the_collection
  - hand_over_the_faces
  - apply_the_settings
content_hashes:
  crates/app/src/fonts.rs: 'sha256:e64e2947f2bc20575da8a4a16877b532af56a559637f28262753778f7df63c98'
  crates/app/src/lib.rs: 'sha256:b7ed6e7ac1a1934a329301543ff11d9378fcf086b1e9d2d646abcbee1e6507d6'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
tags:
  - comments
  - guards
  - review
  - fonts
feature_id: feat_term-core_nx0g1r
---

## What it does

F2.10's fourth delivery gave `fonts.rs` one exception: the machine's fonts are enumerated at start-up **when the settings file names a family**. One condition, one call site. It left **eleven comments across the module saying the opposite of what the code now does**, and the compiler, the suite and all four file-walking guards were green throughout.

The ones that mattered, in the order a reader meets them:

- **The module header** — *«The terminal does not enumerate the machine's fonts when opening the window»*. It is the first thing anybody reads and the sentence **D24 is summarised by**. It became conditional and said nothing about it.
- **The field's docblock** — *«as long as only characters the factory fonts cover appear, this is never built»*, sitting directly under the new field that builds it.
- **The builder's body comment** — *«this line never runs»*. It now runs at start-up.
- Three more that described the fallback order (*«only the icon ones go in front»*, *«behind the factory ones»*, *«where the factory ones rule»*) from before there was a face that goes in front of everything.

### Why the cascade, and where to look for it

**An exception invalidates every comment that states the rule in the absolute** — and a module that argues its decisions, as this one does, states its rules in the absolute **many times over**: in the header, in the field, in the builder, in each reader. The blast radius is not the diff; it is *every place the old rule was worth explaining*.

So the sweep is: grep the module for the rule's own words (here: «never», «does not enumerate», «behind the factory ones», «only the icon ones»), not the changed lines.

### The guard that does not exist

`glossary.rs` catches a comment citing an identifier the tree no longer has — a **rename**. Nothing catches a comment whose sentence stopped being true while every name in it still exists. That gap is why this was found by a reviewer given one job —*do the comments still tell the truth about the code beside them?*— and by nothing else. It is the same shape as [[what-breaks-without-anything-failing]]: green suite, green guards, false file.

## Where it lives

`crates/app/src/fonts.rs`, mostly: module header, the `collection` field, `ensure_the_collection`, `family_of`, `SystemFace::for_icons`, `hand_over_the_faces`. Also `lib.rs` (`apply_the_settings`, which stopped applying everything) and `measure.rs` (a docblock counting the fields of a struct that grew).
