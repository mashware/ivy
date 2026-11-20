---
id: asp_a-translocated-mac-launch-has-a-per-launch-path_rhprwp
slug: a-translocated-mac-launch-has-a-per-launch-path
name: >-
  A quarantined Mac app opened where it was downloaded runs from a per-launch
  copy: never offer a path derived from current_exe for a client to save (D106)
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T14:07:41.252Z'
updated_at: '2026-11-20T14:07:41.252Z'
last_verified: '2026-11-20T14:07:41.252Z'
file_paths:
  - crates/app/src/chrome/connection.rs
  - crates/app/src/chrome/connection/translocation.rs
symbols: []
content_hashes:
  crates/app/src/chrome/connection.rs: 'sha256:9cc99fa4927345d5fbe398c2517a294fe7ad8b77e5c5c6137255611a108f5a96'
tags:
  - macos
  - gatekeeper
  - distribution
  - mcp
feature_id: feat_distribution_kb0x1w
---

## What it does

**On macOS, Ivy's own executable path can exist only for the current launch.** A quarantined
app opened from where it was downloaded (the mounted DMG, `~/Downloads`) without Finder moving
it runs under Gatekeeper App Translocation, from a randomized read-only copy at
`/private/var/folders/…/AppTranslocation/<uuid>/d/Ivy.app`. Rust's `current_exe` returns that
path, and `is_file()` on a sibling binary is true while Ivy runs, so a check «the file exists»
cannot tell it from an install.

**What broke (#145).** The connection guide offered `current_exe()`'s sibling `panel-mcp` as
the bridge path, Codex saved it in `~/.codex/config.toml`, and after Ivy quit the server was
`failed (0 tools)`. An app moved into Applications with `mv` in a shell stays quarantined and
is translocated too.

**Rule.** A path offered for a client to persist (MCP bridge, hooks, adapters) is never taken
from `current_exe` unchecked. Paths used only by this launch's own consoles are fine.
Detection used: an `AppTranslocation` path component; the Security framework's query would
need a native binding for the same answer. Removing quarantine is ruled out as an install step
(`docs/10-installation.md`); the remedy shown is dragging the app to Applications in Finder.

**Still open:** an app run from a mounted DMG *without* quarantine offers a `/Volumes/…` path
that vanishes on eject.

## Where it lives

crates/app/src/chrome/connection.rs (Bridge::beside), crates/app/src/chrome/connection/translocation.rs, docs/02-decisions.md D106
