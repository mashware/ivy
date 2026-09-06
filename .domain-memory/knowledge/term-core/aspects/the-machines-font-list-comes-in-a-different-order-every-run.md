---
id: asp_the-machines-font-list-comes-in-a-different-order-every-run_wxpswa
slug: the-machines-font-list-comes-in-a-different-order-every-run
name: >-
  `fontique`'s family list comes in a different order on every run, so «the
  first installed family» makes an intermittent test — production already knew
  it and the test did not
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T12:42:57.505Z'
updated_at: '2026-09-06T12:42:57.505Z'
last_verified: '2026-09-06T12:42:57.505Z'
file_paths:
  - crates/app/src/fonts.rs
symbols:
  - an_installed_family
  - look_for
  - use_the_family
content_hashes:
  crates/app/src/fonts.rs: 'sha256:e64e2947f2bc20575da8a4a16877b532af56a559637f28262753778f7df63c98'
tags:
  - tests
  - flaky
  - fonts
  - fontique
feature_id: feat_term-core_nx0g1r
---

## What it does

`Collection::family_names()` is backed by a hash map seeded **per process**, so the order it yields is different on every run of the binary. A test that takes `family_names().next()` as «a family this machine has» therefore tests **a different family every time it runs**.

Two failures came out of that, both in the same afternoon:

1. A guard asserting that a family is found however it is capitalized **bailed out green** whenever the drawn name happened to be all lowercase — it covered nothing on an unknown fraction of runs.
2. Another one **panicked outright** when the drawn family's files could not be opened, which is a fact about the machine and not about the code under test. It passed, then failed, then passed again with no change in between.

**Production already knew.** `Discoverer::look_for` sorts the names before walking them, and its comment says why: *«the order in which the system delivers them is not stable: two bench runs gave different families for the same character until it was sorted»*. The knowledge was in the module and the test written next to it did not use it.

### What the test does instead

- **Sort before choosing**, so the subject is the same on every run.
- Pick by the property the test needs (here: a name that has a capital in it), not by position.
- Treat *«the machine lists this family and cannot open it»* as **not concluding** — return, do not fail — while keeping the one answer that would be the code's fault («reported as missing») as a failure.

### The general shape

An intermittent test is worse than a missing one: it gets re-run until it is green and then believed. When a test's subject comes from the environment, **the environment has to be sorted, filtered or pinned first** — and if the production path next door already does it, that is the strongest sign the test must too.

## Where it lives

`crates/app/src/fonts.rs`: the test helper `an_installed_family` and the four guards that use it; the production precedent is in `Discoverer::look_for`.
