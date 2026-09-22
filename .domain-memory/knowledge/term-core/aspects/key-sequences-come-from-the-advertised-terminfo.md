---
id: asp_key-sequences-come-from-the-advertised-terminfo_rszra6
slug: key-sequences-come-from-the-advertised-terminfo
name: >-
  The key sequences are the ones the advertised TERM's terminfo describes — and
  the arrows still do not match it
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T20:37:54.655Z'
updated_at: '2026-09-22T20:37:54.655Z'
last_verified: '2026-09-22T20:37:54.655Z'
file_paths:
  - crates/app/src/keyboard.rs
  - crates/term-core/src/lib.rs
  - crates/term-core/src/screen.rs
symbols:
  - key_bytes
  - navigation_key
  - FIXED_ENV
  - application_cursor_keys
content_hashes:
  crates/app/src/keyboard.rs: 'sha256:1299f986e390ceb4006f58cd67d224042e8b13933ba600534b586add3822f91a'
  crates/term-core/src/lib.rs: 'sha256:5af3288f30591cf1465e35c369b5fd72b46a34799967c3c9dc421abb80bb2aaa'
  crates/term-core/src/screen.rs: 'sha256:ff9255e8e8839fc015b7b993841522344befc6b34ae898ee1d2edfab06f5d63b'
tags:
  - keyboard
  - terminfo
  - DECCKM
feature_id: feat_term-core_nx0g1r
---

## What it does

**Every program is launched with `TERM=xterm-256color` (`FIXED_ENV`), so what a curses program expects from a key is what that terminfo entry says — not what memory says.** Check a translation with `infocmp -1 xterm-256color`.

What the entry describes, read on 22 Sep 2026:
- `smkx=\E[?1h\E=` switches on application cursor keys (DECCKM). Under it: `khome=\EOH`, `kend=\EOF`, and the arrows `kcuu1..kcub1=\EOA..\EOD` (SS3).
- The tilde keys do not change with the mode: `kich1=\E[2~`, `kdch1=\E[3~`, `kpp=\E[5~`, `knp=\E[6~`.
- Modified forms are xterm's parameter: `kHOM=\E[1;2H`, `kDC=\E[3;2~`, `kPRV=\E[5;2~`, `kLFT=\E[1;2D` — `m = 1 + Shift + 2·Alt + 4·Ctrl`, and with a parameter the CSI form is used in either mode.

**Since D52 the six navigation keys match it; the arrows do not** (#261): `key_bytes` sends `\E[A` in every mode and drops Ctrl. So a curses program that called `keypad()` is told Up is `\EOA` and receives `\E[A`. What works today works because programs also bind the CSI form, not because the terminal matches what it advertises. The arrows also feed the wheel-as-arrows path of the alternate screen, which is why they were left out of D52.

The mode is exposed as `Screen::application_cursor_keys()` (a `bool`, over `TermMode::APP_CURSOR`) and read once per keyboard pass in `send_keys`, next to `bracketed_paste()`.

## Where it lives

crates/app/src/keyboard.rs (key_bytes, navigation_key, send_keys); crates/term-core/src/lib.rs (FIXED_ENV); crates/term-core/src/screen.rs (application_cursor_keys). Evidence: `infocmp -1 xterm-256color` on the dev machine, 22 Sep 2026; arrows at keyboard.rs key_bytes on origin/main eaaf701.
