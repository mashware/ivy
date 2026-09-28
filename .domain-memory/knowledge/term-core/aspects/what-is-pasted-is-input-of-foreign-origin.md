---
id: asp_what-is-pasted-is-input-of-foreign-origin_9zg973
slug: what-is-pasted-is-input-of-foreign-origin
name: >-
  What is pasted is untrusted input: the paste marker does not protect from the
  kernel, and filtering only its closing sequence can be slipped past
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-13T16:41:41.826Z'
updated_at: '2026-08-13T16:41:41.826Z'
last_verified: '2026-08-13T16:41:41.826Z'
file_paths:
  - crates/app/src/keyboard.rs
  - crates/app/src/lib.rs
  - crates/term-core/src/screen.rs
  - docs/02-decisions.md
symbols:
  - paste
  - 'Screen::bracketed_paste'
  - 'Pane::send_keys'
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:fac665b7c9d92b5de4f79218612a98ba1d6c97b26242a606fd4149487b51b79c'
  crates/app/src/lib.rs: 'sha256:a05a4309b46dd644b5b6a2389c026f7fef94d9e1a7cb1fcde40b75eb39607291'
  crates/term-core/src/screen.rs: 'sha256:858f97327f67c34b3921b2beb9f07f3b3ebe6c26c2d6a915d535bb6e1e7a04a0'
  docs/02-decisions.md: 'sha256:da19a4989a8d929cc92409af0e0c4267a1e2e9dbb2b06e341954025e551b9e5e'
tags:
  - clipboard
  - security
  - terminal
  - D11
feature_id: feat_term-core_nx0g1r
---

## What it does

**A clipboard is untrusted input, and the paste wrapping protects from far less than it seems.**

**One: it can be closed from inside, and filtering only its closing sequence is not enough.** A text that brings the closing sequence closes it on its own and whatever comes behind it goes in as if typed, **executing without Enter**. And removing it with a search-and-delete **can be slipped past**: `str::replace` walks the original string and does not rescan what it leaves behind, so with `ESC[2` + the closing sequence + `01~` the pieces on either side **stick together and form a new closing sequence**. Verified by running it, not reasoned.

**Two, and it is worse: the wrapping does not protect from the kernel at all.** Control characters are processed by **the line discipline**, which sees the bytes *before* the program does. A pasted `0x03` interrupts the foreground process, a `0x1a` suspends the shell and leaves the pane looking dead, a `0x1c` causes a core dump and a `0x04` closes the input. Marking the paste changes none of that.

> **That is why `xterm` and `VTE` have been filtering controls out of the paste for years, and that is why the rule is not «remove that sequence» but «remove the controls».** An invariant —there is no control in the body except tab and line breaks— is checkable at a glance; a list of forbidden sequences always has one more hole.

**The cost, accepted and written down:** a text with colors loses its escapes and the rest is left as ordinary text. It is what `xterm` does by default. From the terminal **there is no way of telling the control the user wanted from the one that was slipped in on them**.

**And a detail that is only visible on screen: the line break comes out differently depending on whether there is a marker.** With no marker the Enter key has to be **imitated** —carriage return— because the program does not tell what is pasted from what is typed. **With a marker it goes through as it is**: putting a return in there is putting a control inside a text, and it inserts it literally — a box comes out in the middle of the command and the shell prints it. It is the rule Alacritty uses, and here it was discovered **by pasting two lines and looking**, because the tests asserted what the code did and both agreed on the wrong thing.

## Where it lives

`keyboard::paste` in `crates/app/src/keyboard.rs`, decided in **D11** of `docs/02-decisions.md`. The mode is queried with `Screen::bracketed_paste()`, which returns `bool` so as not to let the engine's type show through.
