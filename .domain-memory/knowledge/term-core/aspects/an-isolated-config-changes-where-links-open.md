---
id: asp_an-isolated-config-changes-where-links-open_wvgt2s
slug: an-isolated-config-changes-where-links-open
name: >-
  A manual test with a throwaway XDG_CONFIG_HOME changes which browser opens and
  makes it a second instance — it looks exactly like a product bug
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-22T19:51:41.694Z'
updated_at: '2026-09-22T19:51:41.694Z'
last_verified: '2026-09-22T19:51:41.694Z'
file_paths:
  - crates/app/src/links.rs
symbols:
  - Desktop
content_hashes: {}
tags:
  - links
  - manual-testing
  - xdg
  - browser
feature_id: feat_term-core_nx0g1r
---

## What it does

Launching the terminal for a hand test with a throwaway `XDG_CONFIG_HOME` is the usual way to keep the person's `layout.json` from being restored and overwritten. **It also moves two things the desktop keeps there**, and on #257 both were reported by the user as defects:

- **The browser choice** lives in `$XDG_CONFIG_HOME/mimeapps.list`: without it `xdg-settings get default-web-browser` falls back to the system default (Firefox on Ubuntu, where the person had chosen Brave).
- **Chromium-family profiles** live in `$XDG_CONFIG_HOME/BraveSoftware` (google-chrome and chromium likewise): the launched browser finds no profile, starts a second instance with an empty one, and opens a new window instead of a tab in the one already open.

Both disappeared after copying `~/.config/mimeapps.list` into the isolated directory and symlinking `~/.config/BraveSoftware` into it. **Check before blaming the opener**: `env XDG_CONFIG_HOME=<dir> xdg-settings get default-web-browser`, and a dry run of the same `webbrowser` version under `strace -f -e trace=execve` shows exactly which binary it would launch.

## Where it lives

Any manual run of `target/release/app` with `XDG_CONFIG_HOME` overridden; the opener is `crates/app/src/links.rs` (`Desktop::System`).
