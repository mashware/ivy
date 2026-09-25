---
id: asp_a-scanner-guard-misreads-raw-byte-strings_wegp43
slug: a-scanner-guard-misreads-raw-byte-strings
name: >-
  A hand-written Rust scanner in a guard misreads `br#"…"#`, and the fixture
  that proves the fix binds only with an ODD number of inner quotes
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-25T20:11:50.444Z'
updated_at: '2026-09-25T20:11:50.444Z'
last_verified: '2026-09-25T20:11:50.444Z'
file_paths:
  - crates/app/tests/waits.rs
  - crates/app/tests/glossary.rs
symbols:
  - opens_a_raw_string
  - layers
  - judge
content_hashes:
  crates/app/tests/glossary.rs: 'sha256:b14923f243f488348770036e11496b093be8b8678bdfef85228c15874702c1ca'
tags:
  - testing
  - guards
  - mutation
  - scanner
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two traps in the scanners the source-walking guards hand-write. Only mutation reveals them.** Found on #190 (the waits guard, D57), September 2026.

**1. The byte and C prefixes.** A rule that an `r` opens a raw string only when it is "not the tail of a word" looks safe: it stops `ident"` being read as a raw string, a case Rust syntax never produces. It also rejects `br#"…"#` and `cr"…"`, because the `b` or `c` before the `r` is a word character. This repository writes JSON fixtures as `br#"{\"v\":1,\"lines\":[…]}"#` in `app/src/lib.rs`'s tests. Read as an ordinary string, every inner quote flips the scanner between code and text. A guard that counts braces to find where a test module ends can then end it in the wrong place, and a wait after it can land inside a "string" and go unseen. The fix recognises the prefix: the `r` starts a token, or follows a `b`/`c` that does.

**2. The fixture has to be able to fail.** The first fixture was `br#"{"v":1}"#; rx.recv();`. It stayed green with the prefix handling removed, because two inner quotes close the misread string again before the code after it. `br#"a"b"#; rx.recv();`, with one inner quote, goes red under the same mutation. This is the same disguise as picking a number where the defect and the fix give the same result: here the number is the count of quotes.

**How it was found.** The built-in `code-review` found it. The author's own eight mutations had all come back red, but none touched the prefix, so they could not see it. A mutation tests only what its author thought to mutate.

**Also from that delivery.** A mutation script whose search pattern no longer matches the edited code applies nothing, and it reports the mutation as survived. Check that the replacement happened (assert the pattern is present) before reading a green as a gap.

## Where it lives

`crates/app/tests/waits.rs` — `layers` (the two-layer scanner) and `opens_a_raw_string`, with the fixture `a_raw_byte_string_full_of_quotes_does_not_hide_the_code_after_it`. `crates/app/tests/glossary.rs`'s `identifiers` has the same raw-string branch without the word check.
