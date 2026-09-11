---
id: asp_user-facing-text-is-ascii-comments-are-not_snjgr6
slug: user-facing-text-is-ascii-comments-are-not
name: >-
  The Spanish detector for user-facing messages is «non-ASCII», so a correct
  English sentence with an em dash fails it — and only once somebody lists that
  file
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-11T20:22:32.737Z'
updated_at: '2026-09-11T20:22:32.737Z'
last_verified: '2026-09-11T20:22:32.737Z'
file_paths:
  - crates/app/src/settings.rs
  - crates/app/src/keyboard.rs
symbols:
  - no_layout_or_watcher_message_was_left_in_spanish
  - DRAWN_RULE
  - 'HeldPaste::sentence'
content_hashes:
  crates/app/src/settings.rs: 'sha256:b4bcf5132ae7c60dc9cccb4fedae1c93d77362d46527808e07d10c98d53a1040'
  crates/app/src/keyboard.rs: 'sha256:674ef9ba2cadfedfe82afb9a68b85b927a7365571191ca99b6863ddc634c1af3'
tags:
  - d27
  - guards
  - messages
feature_id: feat_migration-to-english_xnx8mc
---

## What it does

**The guard that keeps user-facing messages out of Spanish detects Spanish as «non-ASCII».** `no_layout_or_watcher_message_was_left_in_spanish` (`crates/app/src/settings.rs`) walks sentence-shaped string literals in a named set of modules and rejects any character that is neither ASCII nor the measurement report's drawn rule, on the stated grounds that *«the angle quotes, the dashes and the accents are how this repository writes Spanish»*.

That is a deliberate over-approximation and it works. The consequence to know **before writing any message**:

> **`—` is forbidden in a string literal a person will read, while the same character is the house style in every comment around it.** Two registers in one file, enforced for one of them.

And it is enforced **only for the files somebody remembered to list**. The held-paste notice in `keyboard.rs` carried an em dash from the milestone that wrote it, and nothing had ever checked, because `keyboard.rs` was not in that walk's list. Adding it — which a later module split forced — turned a silent violation into a red suite, in a delivery that had nothing to do with it. See [[the-sixth-list-lives-in-production-code]].

**The cheaper-looking alternative is worse**: exempting the em dash (`DRAWN_RULE` is the precedent for an exception) weakens one of the three signals the detector runs on, for every walked file at once, to save two characters in one sentence. It was put to the user with both options on the table and rejected.

## Where it lives

`crates/app/src/settings.rs`, inside `#[cfg(test)] mod tests` — `DRAWN_RULE`, the `SOURCES` array, and the assertion that names the offending literal. The message that failed is `HeldPaste::sentence` in `crates/app/src/keyboard.rs`.
