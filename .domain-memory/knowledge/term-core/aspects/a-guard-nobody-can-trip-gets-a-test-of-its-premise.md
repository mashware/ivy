---
id: asp_a-guard-nobody-can-trip-gets-a-test-of-its-premise_7apjzx
slug: a-guard-nobody-can-trip-gets-a-test-of-its-premise
name: >-
  A guard or clamp that cannot be reached: what deserves a test is the
  relationship that keeps it inert, not the guard
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-09-09T21:54:52.007Z'
updated_at: '2026-09-09T21:54:52.007Z'
last_verified: '2026-09-09T21:54:52.007Z'
file_paths:
  - crates/app/src/painter.rs
  - crates/term-core/src/screen.rs
symbols:
  - paint_cursor
  - 'View::cell'
  - 'View::new'
  - 'Screen::view'
content_hashes:
  crates/app/src/painter.rs: 'sha256:8ffdf3a61b18c3a0a9c7c55f78d2b7591595b8c60b3b4259875a66477f3a697d'
  crates/term-core/src/screen.rs: 'sha256:2d233a7c9bd5ffb14bfd15808bdb8e8ff57cfdc1cf537357b62a5e41f30f9902'
tags:
  - testing
  - guards
  - D30
feature_id: feat_term-core_nx0g1r
---

## What it does

**Two of them turned up in one delivery, and the answer is the same both times.**

**1. `paint_cursor`'s third guard.** It leaves through three guards before any ink is chosen: no cursor in the view, a shape of `Hidden`, and **no cell where the cursor stands**. The third cannot be reached from any `Screen`, and a test written for it panics inside the dependency instead of exercising it. Three independent facts close every door: `Screen::view` range-checks the cursor's row and clamps its column; a grid of zero columns — the only shape that would make a clamped column fall outside — is refused by the engine itself (`assert!(columns >= 1)`, `alacritty_terminal-0.26.0/src/grid/row.rs:38`), so constructing one panics before the test starts; and `View::new` is `pub(crate)` to `term-core`, so `crates/app` cannot fabricate one either. **The column clamp is inert too**: deleting it leaves the suite green, because alacritty keeps its own cursor inside the grid across resizes and `CSI 999G`. The only mutation that reaches the guard is turning the **row** check into an unconditional `Some(...)`, which a scrolled-up screen then trips.

**2. A clamp on a deadline.** A blink's flip was clamped to its fifteen-second stop — and `15_000` is exactly twenty-five halves of `600`, so the last flip lands **on** the stop and the clamp can never bind. The assertion written to cover it stayed green with the clamp deleted, and the comment defending it had the arithmetic wrong («at 14.9 s the flip would fall at 15.3»; it falls at 15.0).

> **Pin the RELATIONSHIP that keeps the guard inert, not the guard.** «Whenever the view reports a cursor, there is a cell where it stands.» «The stop is a whole number of half-periods.» Each fails the day somebody moves a constant or simplifies a clamp — which is exactly when the guard starts mattering — and each says where to look. Keep the guard: it costs a line and keeps the function correct for inputs the current constants forbid. What is not acceptable is prose claiming it does something.

Both are recorded in the repository's debt log rather than deleted, because **a guard nobody can trip reads, to the next person, as a case that happens**.

## Where it lives

`crates/app/src/painter.rs` — `a_reported_cursor_always_stands_on_a_cell_and_that_is_why_the_third_guard_is_inert` and the sibling test on the two blink constants. Both entries are in `docs/DEBT.md`.
