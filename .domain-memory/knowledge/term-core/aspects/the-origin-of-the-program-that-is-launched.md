---
id: asp_the-origin-of-the-program-that-is-launched_jmepbw
slug: the-origin-of-the-program-that-is-launched
name: The origin of the program that is launched decides whether term-core is sensitive surface
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-07T19:11:28.224Z'
updated_at: '2026-08-09T18:26:56.608Z'
last_verified: '2026-08-09T18:26:56.608Z'
file_paths:
  - crates/term-core/src/lib.rs
  - crates/app/src/lib.rs
  - docs/05-roadmap.md
symbols:
  - 'Pty::spawn'
  - run
content_hashes:
  crates/term-core/src/lib.rs: 'sha256:dbd22bb07beff290b27a27223f089e5bd44d914c5530d09bbd739011e72d03c9'
  crates/app/src/lib.rs: 'sha256:ea51aded92eaf7e48a86b358573c4b5598a7487cdab504d8a5b6302c4a30d2b3'
  docs/05-roadmap.md: 'sha256:27e4b1b7ac4e1d6e3504f73415a2a8dbd76e8521dd81c1f0c4ab6af4a51042de'
tags:
  - security
  - child-process
  - f1.5
feature_id: feat_term-core_nx0g1r
---

## What it does

`Pty::spawn` **is still not sensitive surface**, but since F1.3 the reason is no longer «the program
is fixed in code» — because it is not.

**What changed (F1.3, the loop's PR).** `app` launches the shell `$SHELL` names, with `/bin/sh` as a
fallback if that variable is missing or the binary cannot be launched. With `/bin/sh` fixed, the
screen you have to look at in order to judge egui is a shell with no prompt and no history — it does
not represent real use, and the whole milestone exists in order to look at that screen.

**Why that opens no surface.** The variable comes out of the session environment of whoever opens
the application, not out of the app's configuration nor out of a `panel.json` of foreign origin.
Whoever controls that environment already controls the session: there is no elevation, there is
nothing an attacker reaches that they did not reach before. And `portable-pty` already copies the
parent's whole environment to the child, so the child sees that session with `$SHELL` or without it;
the only thing the crate sets by hand is `TERM` and `COLORTERM`, no credential.

**The condition that invalidates this assessment has not changed: F1.5.** That milestone injects
`PANEL_ID`/`PANEL_DIR` and opens the door to the program coming out of the user's configuration. As
soon as the program or its arguments come from configuration, from a `panel.json` —which can be of
foreign origin: a cloned repo, a container— or from any other input **external to the process**,
`spawn` becomes sensitive surface and has to be reviewed as such.

**The border, said once and for all, because it is the one you have to know how to apply:**
inheriting the environment of your own session opens no surface; receiving the program from a file
or a configuration somebody else could have written does. The other two rules about the child
process ask the same thing —what comes in and where it comes from—: that a foreign `panel.json` is
untrusted input, and that the app never defines `ANTHROPIC_API_KEY` when launching a panel (D4).

## Where it lives

`crates/term-core/src/lib.rs` — `Pty::spawn`, which sets `TERM`/`COLORTERM` and inherits the rest of
the environment. `crates/app/src/lib.rs` — `Window::new`, which reads `$SHELL` and falls back to
`/bin/sh`. The invalidation condition is written down in milestone F1.5 of `docs/05-roadmap.md`.
