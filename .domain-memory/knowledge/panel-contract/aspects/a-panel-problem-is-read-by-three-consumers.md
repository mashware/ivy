---
id: asp_a-panel-problem-is-read-by-three-consumers_3espzv
slug: a-panel-problem-is-read-by-three-consumers
name: >-
  A panel's stored `problem` is read by three consumers, so silencing it in the
  painter alone breaks the other two: decide at the probe
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-11-20T15:23:32.468Z'
updated_at: '2026-11-20T15:23:32.468Z'
last_verified: '2026-11-20T15:23:32.468Z'
file_paths:
  - crates/app/src/panel/driving.rs
  - crates/app/src/panel/pane.rs
  - crates/app/src/panel.rs
symbols:
  - 'PanelState::problem'
  - refresh_file
  - warning_about_the_panel
  - attention_with
  - how_long_until_warning
content_hashes:
  crates/app/src/panel/driving.rs: 'sha256:7de3bab3acb76fc21f0ce843e29f0ccdc58aed8049d36f1ed66bf0c00c58769f'
  crates/app/src/panel/pane.rs: 'sha256:ebd77ff80590570ab49e15dd836efdb5de89098b6a3205e1e8e83f0af7d7bbfa'
  crates/app/src/panel.rs: 'sha256:fd50961b4afe8c14316c55407307ebac9981c0bdffd2524dc1f443e313e31d2a'
tags:
  - panel
  - warning
  - staleness
  - channel
feature_id: feat_panel-contract_8y2gc0
---

## What it does

**`PanelState.problem` is not only the warning's input.** Three readers
depend on it: the warning text (`warning_about_the_panel`), whether a claim is
painted as backed (`attention_with`, through «nothing to confess» and the
`done` exception), and whether the window arms a wake-up to say a panel went
stale (`how_long_until_warning` returns `None` whenever a problem is stored).

**Measured on #155, November 2026.** A channel-published panel that reported
`done` made the app look at a `panel.json` that never existed; the probe stored
`Problem::Missing` and the warning said «the panel file is no longer there».
The first fix filtered that one line in the painter. The built-in review
showed the stored problem still disarmed the stale clock and kept a dead
agent's `wait` claim backed for ever. The fix that holds is at the probe:
`refresh_file` records no problem for a missing file when the channel owns the
document.

> **Rule: when a stored state is wrong, change what is stored, not what one
> reader paints.** Before silencing a warning, list every reader of the field
> it comes from.

## Where it lives

crates/app/src/panel/driving.rs (refresh_file); readers in crates/app/src/panel/pane.rs and crates/app/src/panel.rs
