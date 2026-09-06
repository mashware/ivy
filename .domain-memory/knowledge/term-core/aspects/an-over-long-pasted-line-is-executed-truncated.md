---
id: asp_an-over-long-pasted-line-is-executed-truncated_36017m
slug: an-over-long-pasted-line-is-executed-truncated
name: >-
  An over-long pasted line is not lost: a DIFFERENT command is executed, and the
  screen shows the one that was not
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T19:30:45.543Z'
updated_at: '2026-09-06T19:30:45.543Z'
last_verified: '2026-09-06T19:30:45.543Z'
file_paths:
  - crates/app/src/keyboard.rs
  - docs/02-decisiones.md
symbols:
  - paste_or_hold
  - HeldPaste
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:c427780eccb982faa34da9e6de95c43c774cdcbfc1df64cab0596327ca7cb199'
  docs/02-decisiones.md: 'sha256:96121e6d8650618d166ab688841fe0b2e590d7e6b16a0f4491c8735d188870d7'
tags:
  - paste
  - security
  - termios
  - clipboard
feature_id: feat_term-core_nx0g1r
---

## What it does

**The line discipline closes an over-long line at its buffer limit instead of refusing it.** So the program receives something **syntactically complete** and runs it. Measured with the shell's stdout going to a **file**, where the echo cannot forge it: a 4 200-character `echo …_TAIL_OK` produced 4 090 characters and no tail, while the terminal echoed all 4 200 followed by a normal prompt. **No error, no mark, and the visible line is the one that did not run.**

> The first version of this probe counted the bytes coming back on the master and reported that everything arrived. It was counting **the echo**, not the program. Same trap as [[the-echo-forges-the-evidence-a-test-reads]], one layer down — and it nearly published a wrong number.

### What looking at the far side rules out, and it is most of the options

- **Splitting at the boundary is worse than the bug.** In a shell the tail then **runs as a command of its own**. And it is a *pure win* for `cat`. `ICANON` is on for both, so **no mode query separates them**: a terminal cannot make splitting safe by asking the kernel.
- **Warning after sending arrives after the incident.** Nothing reaches the program until the line closes, and once it closes the shell has already run the wrong command.
- **A cut inside a quote** leaves the shell waiting at its continuation prompt with the rest discarded; the user's next Enter completes a command whose head they cannot see.
- **The cut is aimable.** Padding places the boundary on any byte, so a visible target ending in a harmless suffix can be cut back to a real one — demonstrated on a scratch file. It is the clipboard-hijack class through a vector **no filter can see**, because there is nothing in the text to filter. But it needs the clipboard *and* the exact boundary *and* the platform: **the argument is the everyday case**, a long command whose trailing flag falls off with nobody's malice involved.

### What nobody outside has done, checked rather than assumed

**None of WezTerm, Kitty or Ghostty chunks a paste**, none warns by size by default, and **none has an issue of this shape in its tracker** — searched for the constant, the name and four ways of saying «the paste got cut». The project's rule is to look up what was solved outside; looking it up said there is nothing to copy. **That is worth as much as an answer would have been**: it means whatever ships is a position and has to be argued, not cited.

All three *do* read the terminal's mode off the master — WezTerm and Kitty for a password heuristic, Ghostty for the same — and **none consults it on the paste path**.

### No test in the tree could have caught it

Every assertion about pasting was about what the paste **produces** — the exact bytes handed to the pseudoterminal — and every one was correct. The failure is on the far side of the kernel, and until this bug **there was no test anywhere in the project that read back what a program actually received**.

## Where it lives

`crates/app/src/keyboard.rs` — `paste_or_hold` holds such a paste instead of writing it, and `HeldPaste` carries the text. **D31** carries the decision, its seven discarded alternatives and what it leaves open.
