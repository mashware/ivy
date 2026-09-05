---
id: asp_collapsing-a-branch-can-bring-back-tmux-nesting_s1bts4
slug: collapsing-a-branch-can-bring-back-tmux-nesting
name: >-
  Collapsing the branch left with one child is not enough: it can nest two
  branches of the same orientation and bring back the divider that moves a block
  of two consoles
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-17T12:01:46.999Z'
updated_at: '2026-08-17T12:01:46.999Z'
last_verified: '2026-08-17T12:01:46.999Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - 'Node::collapse'
  - 'Node::absorb'
  - 'Node::split'
  - 'Node::close'
content_hashes:
  crates/app/src/lib.rs: 'sha256:69e903da2ae25795351e9b8a05001efaad7c4703b59964f4e430a94a6176c041'
tags:
  - pane-tree
  - F2.2
  - design
  - measured
feature_id: feat_term-core_nx0g1r
---

## What it does

**The whole reason the n-ary tree exists is that each divider moves only its two neighbours** — with a
binary tree, three consoles in a row get nested and the outer divider moves **a block of two**, which is
the `tmux` behavior its users complain about most. The design achieves it with one rule: **splitting in
the same orientation as the branch that already contains the leaf inserts a sibling**, not a new branch.

**And closing undoes it through the back door.** The closing rule says that a branch left with **a single
child** is replaced by that child. But that child **can be a branch of the same orientation as its new
mother**, and then you are left with exactly the nesting that splitting is forbidden to create.

**You get there in three gestures, all of them from the keyboard:** split below, split beside, close the
top one. The branch of rows is left with a single child —the branch of columns— and is replaced by it
inside the root branch of columns.

> **Measured, not reasoned:** dragging the outer divider, a console that that divider **ought not to
> touch** goes from **335.6 to 504.0 points** — it grows by 50 %. Verified with a temporary test before
> deciding anything.

**The fix is flattening, and the weights are scaled.** On collapsing, if the surviving child tiles in the
same orientation as the outer branch, **its children rise to become siblings** of the ones above, with
`group's weight · its own / the sum of theirs` and **in the place the group occupied**. That way nothing
moves on screen: neither the block with respect to its aunts, nor the children among themselves. Copying
the weights as they are would move them, and in a direction that depends on how many siblings there were.

> **The general lesson: an invariant that one operation maintains by construction is not maintained by
> another.** «Splitting cannot produce this shape» was true and was taken as sufficient; the shape is
> produced by **closing**, so it falls to closing to undo it. When two operations share a structure, each
> one has to answer for the **whole** invariant, not only for what it itself creates.

**And it is not seen on screen, which is what makes it dangerous.** A branch with one child gives it its
whole area and the tiling puts no divider on it: the layout **is painted the same** with the nesting and
without it. The only thing that changes is what happens **afterwards** — on dragging and on splitting.
That is why the test has to assert over the **effect** (drag and look at who moves) and not over the
tree's shape, which is where the defect is noted down.

Acceptance criterion AC18 of F2.2, written into the design **during** the PR that found it.

## Where it lives

`crates/app/src/lib.rs` — `Node::collapse` (the branch with one child is replaced by it),
`Node::absorb` (the group's children rise with the weight scaled), called from `Node::remove` after the
descent; and `Node::split`, which is the operation that maintains the invariant from the other side.
F2.2, PR #74.
