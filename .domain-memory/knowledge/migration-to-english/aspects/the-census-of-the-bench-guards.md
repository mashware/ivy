---
id: asp_the-census-of-the-bench-guards_k0qaeh
slug: the-census-of-the-bench-guards
name: >-
  The real census of the bench's guards before translating it: 8 tied, 1 tied
  halfway, 3 untied and 1 tied FALSELY by a negation — and the `compare` mode
  fails in the direction nobody expects
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-03T16:11:59.443Z'
updated_at: '2026-09-03T16:11:59.443Z'
last_verified: '2026-09-03T16:11:59.443Z'
file_paths:
  - scripts/measure-frame.sh
  - crates/app/src/measure.rs
  - crates/app/src/lib.rs
symbols:
  - CONSOLE_NAME
  - GESTURE_VARIABLE
content_hashes:
  scripts/measure-frame.sh: 'sha256:ff632681ce0123f34a58a161ee3726107de47669051545e12a59b4968f47bd42'
  crates/app/src/measure.rs: 'sha256:b1880ad9f4993a2551dc9c33cff803f5bfafa3400490943a915b284ad1935861'
  crates/app/src/lib.rs: 'sha256:a97016b4445d86d4aa0f43246bdc3ef5a6690e0adb6e8b801db6ee9be35fcb5a'
tags:
  - d27
  - f2.12
  - guards
  - measurement-bench
  - mutation
feature_id: feat_migration-to-english_xnx8mc
---

## What it does

**How many of the bench's guards were really tied down by a test, counted literal by literal in August 2026, before translating it.** The coupling between the script and the report the application writes lived **only in a comment**, and the impression that «hardly any of them is tied» was false: it came out almost the other way round.

**28 calls to invalidate hung off 12 literals of the report** — and the literal is the unit a rename breaks, not the guard:

| State | How many | Which ones |
|---|---|---|
| Tied by a test that asserts the literal | **8** | ventana · configuración · división · arrastres · recolocar · restauradas · barra · el tamaño |
| Tied **halfway** | 1 | `pestañas:` — the test uses `contains` where the `sed` **anchors with `$`**: a suffix leaves it green and the `sed` empty |
| **Untied** | 3 | escribir la disposición … intentos · fichas marcadas a la vez como mucho · caracteres buscados |
| **Tied falsely** | 1 | glifos rasterizados |

> **The «tied falsely» one is the one that teaches something.** Two assertions asserted **a negation**: `!report.contains("0 glifos rasterizados · 0 sin fuente")`. Renaming the producer's literal leaves them **green by vacuity** — the negation of something that no longer exists always holds. It is the «asserting about the field instead of about the effect» disguise wearing another face, and it was alive in the tree with the suite green.

### Two more traps from the same coupling

1. **The console names go into the comparison.** `CONSOLE_NAME` ends up inside the pane's line, and the `sed` that extracts the geometry **keeps the rest of the line** with its `(izquierda)`. Translating them in one commit and the `sed` in another makes **two runs of the same day come out «not comparable»**.
2. **There are two vocabularies for the gesture and the guards use the one from the environment.** The script passes one through an environment variable; the report publishes another one on screen. **Seven guards compare the environment one**: translating the report's breaks none, translating the environment's breaks seven.

### And the `compare` mode fails in the direction nobody expects

The function that extracts the geometry pulls three literals out with `sed`, and before that there is a `grep -q` of the header. Translating the header **whole** makes it abort with a message that blames the report («it is from before the header published it»): noisy but safe. Translating it **halfway** leaves the extraction returning the same emptiness for both reports: **a green light to average any two runs whatsoever**.

> **And «halfway» is exactly what a file-by-file migration produces all by itself.** That is why delivery #2 of the milestone tied the net **before** translating anything.

### How it was closed

Verified in September 2026: `measure.rs:2722 every_pattern_the_script_looks_for_in_the_report_still_extracts_something` exists, which reads the script with `include_str!`, **re-extracts every `grep`/`sed` pattern and re-applies it** to reference reports built by calling the real `header()` — not hand-written strings — and which also fails if some line of the script that reads the report contributed no pattern. The negation about the glyphs is no longer in `lib.rs`. **The census is historical; the lesson —a test ties a coupling down only if it DERIVES the pattern, and a negation ties nothing— is not.**

## Where it lives

`scripts/measure-frame.sh` — the calls to invalidate and the `grep`/`sed` patterns that read the report; the `compare` mode and its extraction of the geometry; and the gesture vocabulary it exports through the environment.

`crates/app/src/measure.rs` — the producer of the report, `CONSOLE_NAME`, and at `:2722` the guard that **derives** the patterns instead of copying them.

`docs/02-decisions.md` — **AC8** of milestone F2.12, which is this finding turned into a criterion: no guard of the bench is left dead, and the four that nothing covered are tested by breaking them on purpose.
