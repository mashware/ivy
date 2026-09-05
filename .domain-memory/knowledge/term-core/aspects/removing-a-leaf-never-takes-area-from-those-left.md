---
id: asp_removing-a-leaf-never-takes-area-from-those-left_fy36sq
slug: removing-a-leaf-never-takes-area-from-those-left
name: >-
  Removing a leaf almost never takes area from the ones left: absorbing costs
  them a divider — and the geometric floor is checked all the same against the
  tree as it is NOW
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T20:07:02.388Z'
updated_at: '2026-08-18T13:58:47.482Z'
last_verified: '2026-08-18T13:58:47.482Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - Node
  - remove
  - collapse
  - absorb
  - place
  - can_split
  - move_pane
content_hashes:
  crates/app/src/lib.rs: 'sha256:d7688143658df12a548b8eb3bd4e0e7fc0ce9966b7a5e14a6e7d7a8e8835ba9d'
tags:
  - layout-tree
  - invariants
  - geometry
feature_id: feat_term-core_nx0g1r
---

## What it does

**Removing a leaf from the layout tree almost never takes area from the ones left.** The «almost»
was measured in F2.3 and corrects what this record claimed before. The three operations that come
into play, and none of them is obvious on its own:

- **`remove`** takes a child out of its branch's list **without touching the weights of the
  others**, so each sibling's share goes from `wᵢ/Σ` to `wᵢ/(Σ−w)` — strictly **larger**.
- **`collapse`** replaces the branch that is left with a single child by that child, and that child
  takes the **whole** area of the branch.
- **`absorb`** lifts the grandchildren up to siblings by scaling their weights (`w · suyo / Σ`),
  and that keeps **the tiling** but **not the exact geometry**.

> **The exception, measured and not reasoned out: `absorb` costs the WHOLE branch a divider, the
> ones that do not take part included.** Lifting the children puts one more divider into the outer
> branch, and the dividers are deducted from the area **before** tiling: the usable part goes down
> by one thickness for every extra child that is lifted. With an area of 1 200 points, a console
> with nothing to do with the gesture that took half of it **goes from 599 to 598**. Pinned by
> `absorbing_preserves_the_geometry_of_the_consoles_that_do_not_move`, which is why it asserts
> **proportions and not points** — see [[a-geometric-invariant-is-asserted-in-proportions]].

> **What this record said before was the code's own doc, and it was false too**: «giving each one
> `w · suyo / Σ` moves nothing on screen». Corrected in the code in F2.3 (commit `20872e6`). It is a
> textbook case of [[what-breaks-is-what-the-new-operation-makes-reachable]]: the claim had been
> written for two milestones and nobody had made `move_pane` go down that path.

**And the conclusion of the record STILL STANDS, but by another route.** Checking the geometric
floor against the tree as it is now is coherent and sufficient: the intuitive wording —«validate
against the tree left after removing»— **is incoherent**, because you cannot validate against a tree
that does not exist yet. And what the floor protects is **the branch that is split**, not the rest:
taking in a console raises the children of the destination branch by one, exactly as splitting it
does, which is just what `can_split` exists to check. A console with nothing to do with it losing
one point does not violate that, because **the minimum of 20 columns is a condition of the operation
and not a permanent invariant** — shrinking the window already breaks it, and the floor that
`place` applies on every frame is the width of **one cell**, not `MINIMUM_CONSOLE`.

## Where it lives

`crates/app/src/lib.rs`: `Node::remove`, `Node::collapse`, `Node::absorb`, `Node::move_pane`,
`place` and `can_split`. Came out of the F2.3 design (issue 76), where the first wording said
«validate against the tree left after removing» and meant nothing; the divider exception came out
while writing the test of the second PR of that same milestone, by measuring — not by reading.
