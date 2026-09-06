---
id: asp_the-echo-forges-the-evidence-a-test-reads_nfhnsj
slug: the-echo-forges-the-evidence-a-test-reads
name: >-
  The line discipline forges, character for character, the evidence a terminal
  test reads — and no control character can tell it from the program
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-06T17:06:57.340Z'
updated_at: '2026-09-06T17:06:57.340Z'
last_verified: '2026-09-06T17:06:57.340Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/tests/scripts.rs
  - docs/02-decisiones.md
symbols:
  - confirm_the_console_is_in_the_mode_it_asked_for
  - wait_for_this_console_to_say
  - a_pane_that_asks_for_the_mouse
content_hashes:
  crates/app/src/lib.rs: 'sha256:06780a0eaae7013bb6602574e7d50386ede8e464b226f9458287d5071106f017'
  docs/02-decisiones.md: 'sha256:96121e6d8650618d166ab688841fe0b2e590d7e6b16a0f4491c8735d188870d7'
tags:
  - testing
  - termios
  - false-green
  - mouse
  - portability
feature_id: feat_term-core_nx0g1r
---

## What it does

**A terminal test that asserts on what appears on screen is asserting on something the kernel can write.** Fifteen tests of the mouse path launched `stty -echo -icanon; printf '\033[?100Xh'; echo ready; cat -v` and waited for the `cat -v` inside to echo back the report they had sent. With `stty` missing the `;` let the ready marker through anyway, and **fourteen of the fifteen stayed green** — measured: 744 passed, 1 failed.

**They stayed green because the terminal wrote the answer itself.** With `ICANON` on the line discipline delivers nothing to the program without a newline, and a mouse report has none: `cat -v` received **nothing**. With `ECHO` on, the line discipline echoed the same bytes, and `ECHOCTL` renders an escape `^[` — character for character what `cat -v` would have printed. Confirmed by the pair that differs in one variable: with `stty -echo` alone (canonical still on) the same tests go **red**, on «the program did not receive «^[[<0;4;3M»; on screen there is: "ready"».

> This is the project's first recorded disguise —asserting on where a value is recorded instead of on the effect it has— in its sharpest form: **the effect was real and its author was somebody else**.

### Chaining with `&&` is necessary and not sufficient

Replace `stty` with `true` — exits zero, changes nothing — and the chain sees nothing wrong. Measured: **15 of 15 green** without a probe of the mode, **15 of 15 red** with one. A command's exit status is not a mode that took.

### No control character can be the probe, and `0x7f` is the trap

The probe has to be a byte whose rendering **only the program** can produce. Every control character fails by construction: `ECHOCTL` and `cat -v` both use caret notation, so `0x1b` comes back `^[` from either.

`0x7f` looks like the exception, because canonical mode eats it as the erase key — and it shipped on that reasoning. **It is wrong: the erase key is configuration, not a property.** Measured over three pseudoterminals, one per mode:

| mode | probe | came back |
|---|---|---|
| raw (`-echo -icanon`) | `0x7f` | `^?` |
| canonical, erase at its default | `0x7f` | *(nothing)* |
| canonical + echo, **`stty erase ^H`** | `0x7f` | **`^?`** |
| canonical + echo, erase moved | `0x80` | `\x80` raw |
| raw | `0x80` | **`M-^@`** |

With the erase key moved, the `0x7f` probe passed over a canonical terminal: **all fifteen green, fully forged**. A byte **above** `0x7f` has no twin — `cat -v` writes it in meta notation and the line discipline echoes the raw byte.

**And a tab is not the answer either**, which is worth recording because it was proposed as the remedy: `ECHOCTL` exempts TAB, but so does `cat -v` — `printf 'a\tb' | cat -v` gives back a literal tab, not `^I`. Waiting for `^I` would have left all fifteen tests permanently red. Measured before it was applied.

### Why it matters beyond these tests

The guarantee stopped depending on anybody's configuration, and that is the point: **F2.7 takes this terminal to macOS and Windows**, where the default erase key, the `stty` operands and `cat -v` are all up for renegotiation. A green that rests on a default is a green that will lie exactly where nobody can check it.

## Where it lives

`crates/app/src/lib.rs` — `confirm_the_console_is_in_the_mode_it_asked_for` writes the probe byte and waits for the meta notation, then wipes the grid **through the parser** (`\x1b[H\x1b[2J`) rather than by replacing the `Screen`, which would throw away the mouse modes the tests assert on. `wait_for_this_console_to_say` exists because the other waiter clears. **D30** carries the decision and the six discarded alternatives; the tests are the ones **D23** rests on.
