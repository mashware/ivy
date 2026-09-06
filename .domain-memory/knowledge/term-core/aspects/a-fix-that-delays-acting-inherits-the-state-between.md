---
id: asp_a-fix-that-delays-acting-inherits-the-state-between_nwmxda
slug: a-fix-that-delays-acting-inherits-the-state-between
name: >-
  A fix that puts a delay between deciding and acting inherits every piece of
  state that can change in between — and it reintroduced its own bug twice
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T19:31:13.401Z'
updated_at: '2026-09-06T19:31:13.401Z'
last_verified: '2026-09-06T19:31:13.401Z'
file_paths:
  - crates/app/src/keyboard.rs
symbols:
  - HeldPaste
  - paste_or_hold
  - start_the_keys
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:c427780eccb982faa34da9e6de95c43c774cdcbfc1df64cab0596327ca7cb199'
tags:
  - design
  - review
  - state
  - paste
feature_id: feat_term-core_nx0g1r
---

## What it does

**The fix for the truncated paste holds the bytes and asks. That delay — frames, or minutes — is where it reintroduced the very bug it was built to stop. Twice. Both caught in review, neither by reading the diff.**

**One: a second over-long paste replaced the first, and the first went without a word.** The call site read `paste_or_hold(…).or(held.take())`, and `.or` evaluates its argument, so the standing question was taken out and dropped. Silent loss, one layer up, in the feature built to stop silent loss. Fixed by never replacing a standing question and **counting** the ones turned away into it, so nothing goes unsaid.

**Two, and it is the sharper one: the paste was composed in one mode and sent in another.** A paste is composed against the program's paste mode — wrapped or not, a break as `\n` or as `\r`. The answer comes many frames later and **the mode moves underneath**: `readline` turns the wrapping off while a command runs and on again at its prompt (see [[who-is-in-canonical-mode-and-when]]). Bytes composed unwrapped carry `\r` between their lines; delivered to a prompt that has since turned the wrapping on, **the line editor executes each line as it arrives**. Fixed by keeping the *text* and composing it **when it is sent**.

> The first version stored the composed bytes with a docblock arguing for it in so many words: *«so that saying yes sends exactly what would have been sent, and not a second composition that could differ from the first»*. **That reasoning is backwards** — a second composition differing from the first is exactly what is wanted when the world moved. The docblock now says so, because the next reader will have the same intuition.

### The rule to carry forward

**Enumerate what the delay can outlive.** For anything held to be asked about: the mode the payload was prepared for, the question already standing, the pane that asked, the focus that would answer, the program that would receive it. Each one is a piece of state read at decision time and used at action time, and each is a defect until it is either re-read or declared.

### A third one, declared rather than fixed

A short paste arriving while a question stands goes through, so the held bytes land **after** it if the answer is yes — the reverse of the order they were pasted in. That is the price of not freezing the console over a question, and it is written where the code is rather than left for somebody to meet.

## Where it lives

`crates/app/src/keyboard.rs` — `HeldPaste` keeps the text and not the bytes, and its docblock carries the autopsy. **D31** records the decision; the postmortem of the ticket is where the pattern was named.
