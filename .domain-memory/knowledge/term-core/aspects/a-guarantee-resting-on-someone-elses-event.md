---
id: asp_a-guarantee-resting-on-someone-elses-event_em8jpf
slug: a-guarantee-resting-on-someone-elses-event
name: >-
  A guarantee resting on an event a dependency emits for its own reasons: the
  signal that was convenient in one milestone became the sole carrier of a
  promise in the next
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-08T09:13:08.032Z'
updated_at: '2026-09-08T09:13:08.032Z'
last_verified: '2026-09-08T09:13:08.032Z'
file_paths:
  - crates/app/src/keyboard.rs
  - docs/02-decisions.md
symbols:
  - bytes
  - 'Event::Paste'
  - 'Event::Copy'
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:345522591ba5b695d69c974a854c72542d5148a73ae1aa0cc9f95cda935ce0c8'
  docs/02-decisions.md: 'sha256:a882fc9bdfa1ef397ba991d62b8e391eecae0d6cda42533de02466b23b0596a3'
tags:
  - design
  - keyboard
  - D11
  - D33
feature_id: feat_term-core_nx0g1r
---

## What it does

**The failure shape behind #203, stated so it is recognised the next time rather than rediscovered.** Traced with `git log -L` over the one line, oldest first:

- **F1.3, `d7e5862` (#17), 9 August 2026** — the line is *born* as `Event::Paste(_) => Some(Cow::Borrowed(&[22]))`, with the comment «the clipboard's text is discarded on purpose». Real pasting did not exist yet: the paste event was used **only as a way of noticing that the chord had been pressed**. Perfectly reasonable at the time.
- **F2.6, `3830b26` (#51), 13 August 2026** — pasting arrives, and with it D11's promise: «Ctrl+V is worth in a terminal what the control byte is worth». The dependence on `Event::Paste` was **inherited, never chosen**. A mechanism that was a convenient hint in one milestone became the only carrier of a guarantee in the next.

**So there is no suspect commit and no regression**, which is why `git blame` finds nothing to point at. The defect is as old as the keyboard module; what F2.6 changed is that the project started promising something the mechanism cannot deliver — the event is emitted at the dependency's discretion, for **its** purposes, and it withholds it exactly when the clipboard has no text.

> **The criterion this leaves.** When a promise starts to rest on an event a dependency emits, ask **what makes that dependency emit it** — not what it is called. `Event::Paste` is named after the *gesture* and is emitted for the *payload*: no payload, no event, and the gesture disappears with it.

**Where else the same shape is available here:** `Event::Copy` — `Ctrl+Shift+C` also arrives already turned into a request rather than a key, and its own test disguise already bit once (D26's annex: the test seeded a key the desktop never lets through, and copying interrupted the program for a whole milestone with the suite green). Checked while fixing #203: cut and copy are pushed **unconditionally** upstream, so they carry no equivalent silent drop — but nobody had checked, and any future chord the desktop resolves before the application sees it has the same exposure. D26's shortcut table cannot claim what never arrives, so a `key.` setting for such a chord would be a promise with no mechanism behind it.

## Two habits this cost, both worth more than the bug

**A decision that declares a limit unfixable carries the evidence of the attempt.** D11 said *«it cannot be fixed from here»* and the knowledge card repeated it verbatim; the eventual fix came from reading four routes in the dependency's source, and **none of that reading had been done**. An unfixability claim names what was read or measured, or it is written as «not attempted».

**A limit accepted because it is rare states the population it was judged against.** «An empty clipboard is rare» was true and beside the point: the real condition was «holds no text», and a clipboard holding an image is an ordinary state in a terminal built to host agents. **The frequency argument was evaluated against the wrong population**, and that is what kept a real defect filed as a limitation for 26 days — written down twice, in D11 and in the store, and found in the end by using the product.

## Where it lives

`keyboard::bytes` in `crates/app/src/keyboard.rs`; D11 and D33 in `docs/02-decisions.md`. The mechanism it broke on is in [[ctrl-v-is-discarded-when-the-clipboard-holds-no-text]].
