---
id: asp_who-is-in-canonical-mode-and-when_08w6x3
slug: who-is-in-canonical-mode-and-when
name: >-
  A modern shell is canonical for most of the time it is on screen — and
  sampling only at the prompt gives a correct measurement and a false conclusion
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T19:30:19.640Z'
updated_at: '2026-09-06T19:30:19.640Z'
last_verified: '2026-09-06T19:30:19.640Z'
file_paths:
  - crates/app/src/keyboard.rs
  - crates/term-core/src/lib.rs
symbols:
  - paste_or_hold
  - the_longest_line_a_reader_keeps
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:c427780eccb982faa34da9e6de95c43c774cdcbfc1df64cab0596327ca7cb199'
  crates/term-core/src/lib.rs: 'sha256:21899fbd18b28515f01ca0c0c025ca2b260981459a259167fbc21b961f6070c7'
tags:
  - termios
  - measurement
  - paste
  - blast-radius
feature_id: feat_term-core_nx0g1r
---

## What it does

**`bash` and `zsh` are in raw mode at their prompt and canonical the moment they launch a foreground command.** The line editor holds the terminal while it is editing and **hands it back** when it runs something. Measured over a real pseudoterminal, sampling both moments:

| program | at the prompt | while a foreground command runs |
|---|---|---|
| `bash -i` | ICANON off | **ICANON on** |
| `zsh -i` | ICANON off | **ICANON on** |
| `sh -i` | ICANON on | ICANON on |
| `cat` | ICANON on | — |

> ### The trap, and it cost this investigation a wrong conclusion
>
> Sampling **only at the prompt** is a correct measurement. The conclusion drawn from it — *«the long-paste truncation does not reach `bash` or `zsh`»* — was **false**, and a challenger overturned it by sampling the other moment and reproducing the bug end to end: with a `sleep` in the foreground, `bash` executed the truncated half of a paste and the rest vanished.
>
> **Anything that reasons about the terminal's mode has to say which moment it sampled.** A table of modes with no moment attached is not evidence.

**So the blast radius of anything gated on canonical mode is not `sh` and `cat`: it is everybody who types or pastes while something is running** — type-ahead, an entirely ordinary act.

### What this means for any fix that reads the mode

The mode can be read — `portable-pty` exposes `get_termios()` on the master, 254 ns, proven end to end — and **it still cannot be a gate**:

- **It flips at every command boundary**, so it is not a rare window; it is the normal rhythm of a shell.
- **D29** put an unbounded queue between reading it and the bytes reaching the line discipline, so the gap is however long the writer thread stays blocked.
- And the two ways of being wrong are **not symmetric**: reading «canonical» when the program went raw refuses something that would have worked — visible and recoverable. Reading «raw» when it went canonical **sends the long line anyway**, with a check in front of it saying it was checked.

**A reading is worth something as «raw, therefore no limit» and worth nothing as «canonical, therefore act».** That asymmetry is why **D31**'s gate keys off the *paste mode* the program declared rather than off a termios reading: a program that turns the bracketed-paste markers on is a line editor in raw mode by construction, and `readline` turns them off before running a command — the same switch, observed from the side that does not race.

## Where it lives

Probed with Python's `pty` module; the Rust equivalent is `crates/term-core/examples/termios_probe.rs`, which is `#[cfg(unix)]` because Windows has no such question at all. The gate that uses the conclusion is `paste_or_hold` in `crates/app/src/keyboard.rs`; **D31** carries the decision.
