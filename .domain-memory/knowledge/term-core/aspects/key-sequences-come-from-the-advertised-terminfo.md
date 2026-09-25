---
id: asp_key-sequences-come-from-the-advertised-terminfo_rszra6
slug: key-sequences-come-from-the-advertised-terminfo
name: >-
  The key sequences are the ones the advertised TERM's terminfo describes — and
  the reference terminals disagree about the wheel
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T20:37:54.655Z'
updated_at: '2026-09-25T00:00:00.000Z'
last_verified: '2026-09-25T00:00:00.000Z'
file_paths:
  - crates/app/src/keyboard.rs
  - crates/app/src/mouse.rs
  - crates/term-core/src/lib.rs
  - crates/term-core/src/screen.rs
symbols:
  - navigation_key
  - wheel_arrow
  - CursorKeys::of
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
  - wheel
feature_id: feat_term-core_nx0g1r
---

## What it does

**Every program is launched with `TERM=xterm-256color` (`FIXED_ENV`), so what a curses program expects from a key is what that terminfo entry says — not what memory says.** Check a translation with `infocmp -1 xterm-256color`.

What the entry describes, read on 22 and 25 Sep 2026:
- `smkx=\E[?1h\E=` switches on application cursor keys (DECCKM) — curses sends it from `keypad()`. Under it: `khome=\EOH`, `kend=\EOF`, and the arrows `kcuu1..kcub1=\EOA..\EOD` (SS3).
- The tilde keys do not change with the mode: `kich1=\E[2~`, `kdch1=\E[3~`, `kpp=\E[5~`, `knp=\E[6~`.
- Modified forms are xterm's parameter: `kHOM=\E[1;2H`, `kDC=\E[3;2~`, `kPRV=\E[5;2~`, `kLFT=\E[1;2D` — `m = 1 + Shift + 2·Alt + 4·Ctrl`, and with a parameter the CSI form is used in either mode.

**Since D52 — point 5 closed the arrows (#261) — all ten keys match it**: one table, `navigation_key`, gives the bare key by mode and any modifier as `CSI n;m X`. The mode is read once per pass as `CursorKeys::of(&screen)`, over `Screen::application_cursor_keys()` (`TermMode::APP_CURSOR`).

**The wheel on the alternate screen sends the bare arrow in the screen's mode**, through `keyboard::wheel_arrow`, which shares the `UP`/`DOWN` constants with the keyboard's table so the two cannot disagree. `less` and `vim` switch DECCKM on as they start, so a wheel with plain `CSI A` sends them the other shape on every notch.

**Do not cite a reference terminal for the wheel without reading its source — they disagree.** WezTerm's alternate scroll presses the arrow through its key encoder (`term/src/terminalstate/mouse.rs`, `self.key_down(KeyCode::UpArrow, …)`), so it honours DECCKM. **Alacritty does not**: `alacritty/src/input/mod.rs` always pushes `ESC O A` / `ESC O B`, whatever `APP_CURSOR` says (read 25 Sep 2026). A first draft of D52 point 5 cited Alacritty as doing what this terminal does, and a reviewer caught it.

**What a person sees change in a shell**: readline inserts the tail of a modified arrow it has no binding for (`;2A` for `Shift+Up`). Debian/Ubuntu's `/etc/inputrc` binds only `\e[1;5C` / `\e[1;5D` (word motion). These are the same bytes xterm, WezTerm and Alacritty send — the remedy is a binding, not a plainer key.

## Where it lives

crates/app/src/keyboard.rs (navigation_key, UP/DOWN, wheel_arrow, CursorKeys::of, send_keys); crates/app/src/mouse.rs (send_wheel); crates/term-core/src/lib.rs (FIXED_ENV); crates/term-core/src/screen.rs (application_cursor_keys). Evidence: `infocmp -1 xterm-256color` on the dev machine; the Alacritty and WezTerm sources on their default branches, 25 Sep 2026; tests `the_arrows_follow_the_cursor_key_mode_the_program_chose` and `under_application_cursor_keys_the_wheel_sends_the_arrows_the_program_asked_for`.
