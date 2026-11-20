---
id: asp_a-dock-launched-mac-app-gives-consoles-no-locale_xcdcmd
slug: a-dock-launched-mac-app-gives-consoles-no-locale
name: >-
  A Mac app opened from the Dock gives its consoles no locale, and zsh then
  miscounts every UTF-8 prompt glyph (D105)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-19T15:47:37.557Z'
updated_at: '2026-11-19T15:47:37.557Z'
last_verified: '2026-11-19T15:47:37.557Z'
file_paths:
  - crates/term-core/src/locale.rs
  - crates/term-core/src/lib.rs
symbols:
  - console_lang
  - FIXED_ENV
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:7ee8c68e3c4920c6671a448d0eebf260bbbb8bf6d4e66405f81fcab7e0108a65'
tags:
  - macos
  - locale
  - zsh
feature_id: feat_term-core_nx0g1r
---

## What it does

**An app opened from the macOS Dock or Finder inherits launchd's environment, which has no `LANG`.**
Measured inside an Ivy console on 19 November 2026 (#117): `LANG=""`, every `LC_*` = `C`.
Ivy then set nothing, so every console ran in the C locale.

**What it breaks.** zsh 5.9 in C counts each byte of an undecodable UTF-8 glyph as a column
(`LC_ALL=C zsh -f`, `x=❯; print ${(m)#x}` gives `3`). Starship's `❯` made the prompt two columns
wider than painted, and typing `cd ` left an unerasable `cdcd`. `LANG=en_US.UTF-8` removed it.
On Linux the same overcount stayed harmless because ZLE moved relative to the cursor; what makes
it visible on macOS was never identified, and the reporter's test also made zsh a login shell.

**How the reference terminals answer it** (source read 2026-11-19), all from `NSLocale`:
- WezTerm, kitty, Ghostty: only when `LANG` is unset or empty; `en_US.UTF-8` behind it.
- Alacritty: `LC_ALL` when `setlocale` fails or yields `C`; `LC_CTYPE=UTF-8` behind it.
- iTerm2 and Terminal.app: a default-on setting; iTerm2 overrides an inherited value.
- None does it on Linux or Windows.

Ivy follows WezTerm (D105): macOS only, nothing inherited, full `LANG`, validated with
`newlocale` (not `setlocale`, which changes the process's own locale).

**Still open:** on macOS 14+ `NSLocale.currentLocale` may report the app's language (English, the
only one Ivy ships) rather than the system's, turning `es_ES.UTF-8` into `en_US.UTF-8`.

## Where it lives

crates/term-core/src/locale.rs; docs/02-decisions.md D105
