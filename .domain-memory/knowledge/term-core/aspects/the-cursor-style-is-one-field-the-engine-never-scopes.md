---
id: asp_the-cursor-style-is-one-field-the-engine-never-scopes_s2x89e
slug: the-cursor-style-is-one-field-the-engine-never-scopes
name: >-
  The cursor's style is ONE field on the terminal: it is the seam for changing
  what this terminal does by default, and nothing scopes it — not the alternate
  screen, not the program that set it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:53:25.607Z'
updated_at: '2026-09-09T21:53:25.607Z'
last_verified: '2026-09-09T21:53:25.607Z'
file_paths:
  - crates/term-core/src/screen.rs
symbols:
  - 'Screen::new'
  - cursor_style
  - default_cursor_style
  - swap_alt
  - reset_state
content_hashes:
  crates/term-core/src/screen.rs: 'sha256:2d233a7c9bd5ffb14bfd15808bdb8e8ff57cfdc1cf537357b62a5e41f30f9902'
tags:
  - alacritty
  - vte
  - cursor
  - D37
feature_id: feat_term-core_nx0g1r
---

## What it does

## The seam: change the engine's default, do not add a rule beside it

`alacritty_terminal` 0.26's `Term::cursor_style()` (`src/term/mod.rs:942`) is `self.cursor_style.unwrap_or(self.config.default_cursor_style)`, and **every door a program has lands back on that field**: DECSCUSR's `0` is `set_cursor_style(None)` (`vte-0.15.0/src/ansi.rs:1714`), `reset_state` (`:1840`, what `ESC c` and `tput reset` run) sets `cursor_style = None`, and DEC mode 12 and `set_cursor_shape` both `get_or_insert(default)`.

So setting `Config { default_cursor_style: CursorStyle { shape: Block, blinking: true }, .. }` in `Screen::new` makes a shell blink, leaves **every sequence with the meaning its standard gives it**, and makes the change reversible by the program in both directions **without this crate implementing any of it**. Checked and clean: `Screen::new` is the only place a `Config` is built in this workspace, DECSTR is not implemented, and vi mode is never entered here.

**Two consequences before flipping it**: a shape picked through OSC 50 `CursorShape=` sets the shape **alone** and therefore inherits whatever the default says about blinking; and `CSI ?12$p` (report the mode) **mutates** `cursor_style` to `Some(default)` while answering.

## And nothing scopes that field, so what a program asks for outlives it

Verified by writing the test rather than assumed: `swap_alt` (`term/mod.rs:714`) swaps the grid, the keyboard mode stacks and the selection and **never touches `cursor_style`**; what `CSI ?1049` saves and restores is the DEC cursor — position and attributes — which is a different thing.

So a full-screen program that asks for a steady beam inside the alternate screen **leaves the shell it came back to with a steady beam**, blink bit included, until a reset. xterm and kitty behave the same way, so it is the standard's shape and not a bug — **but it becomes user-visible the moment a terminal has a non-trivial default**: leaving `vim` can leave a shell's cursor still, and «my cursor stopped blinking and I did not touch anything» has no obvious cause. Anything armed off that bit changes state on programs **exiting**, not only on programs starting.

## Where it lives

`crates/term-core/src/screen.rs` — the `Config` in `Screen::new` carries the default, and `Screen::cursor_blinks` is the O(1) reader the painter uses. The decision is **D37**; `docs/08-configuration.md` tells whoever uses the terminal that a program's request outlives it.
