---
id: asp_set-var-is-undefined-in-this-suite_m3g7dt
slug: set-var-is-undefined-in-this-suite
name: >-
  In this suite `std::env::set_var` is undefined behavior: every console that is
  launched walks the whole environment, and the tests run in parallel
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-15T21:24:59.175Z'
updated_at: '2026-08-15T21:24:59.175Z'
last_verified: '2026-08-15T21:24:59.175Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/term-core/src/lib.rs
symbols:
  - set_var
  - 'Pane::new'
  - CommandBuilder
  - run
content_hashes:
  crates/app/src/lib.rs: 'sha256:b3ca4328c99c0b94b314ea9a6c6d1ab4f6afe0dad22ef3415700f289bc9af411'
  crates/term-core/src/lib.rs: 'sha256:6d8ac25cbaae68a1d230090cc4c349577ba67801bee3f7d5de632814ca3a4289'
tags:
  - tests
  - environment
  - F2.1
feature_id: feat_term-core_nx0g1r
---

## What it does

**The contract of `std::env::set_var` is about the WHOLE environment, not about the key being
touched**: with one write in flight, any concurrent **read** is undefined behavior. The intuitive
justification —«I can touch this variable because no other test reads it»— **is false**, and it was
written in this project before a review knocked it down.

**And this suite reads the environment all over the place and in parallel:**

- `Pane::new` reads `SHELL` — and there are dozens of tests that open panes.
- `portable_pty` walks **`std::env::vars_os()` while building each `CommandBuilder`**, that is,
  **once per console launched** (`cmdbuilder.rs:75`).
- There is no `--test-threads=1`, no nextest configuration and no `serial_test` in the repo: `cargo
  test` spreads the work across threads.

The symptom is not a clean failure: it is **hangs or corrupt child environments, intermittent and in
tests that have nothing to do** with the one that touched the variable.

> **The way to live with an environment variable in a project that launches processes: split the
> parsing from the reading.** A pure function takes the value and is tested in full; a single line
> reads it from the environment and **no test covers it**, said in the code itself. It is the same
> border that was already accepted for `on_exit`: there are places a test cannot get into and the
> honest thing is to write that down, not to fake coverage.

Applied in F2.1 with `MEASURE_TABS`, the door that opens N tabs at startup:
`how_many_tabs(Option<&str>)` is tested and the one that reads the variable, `run()`, is not.
What covers that line is not a test — it is use: whoever measures sees a single tab as soon as the
window opens. The split into a pure function and an uncovered caller **is** the shape this record
argues for, and `crates/app/src/lib.rs` says so in its own words at the function: «the only line
left uncovered is `run`'s».

## Where it lives

`how_many_tabs` / `run` in `crates/app/src/lib.rs`; the third-party
environment reader is in `portable-pty`, `src/cmdbuilder.rs:75`
