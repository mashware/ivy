---
id: asp_the-sibling-mould-brings-its-decisions-and-its-gaps_4vvrc2
slug: the-sibling-mould-brings-its-decisions-and-its-gaps
name: >-
  Copying the sibling piece imports its decisions AND ITS GAPS: six out of
  fourteen findings of one review, with the knowledge already filed and not
  applied
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-19T15:30:17.829Z'
updated_at: '2026-08-19T15:30:17.829Z'
last_verified: '2026-08-19T15:30:17.829Z'
file_paths:
  - crates/app/src/lib.rs
symbols:
  - drag_its_panel_divider
  - drag_the_divider
content_hashes:
  crates/app/src/lib.rs: 'sha256:67dfdfc9bad7c0d618f018577eecd1de410625cc5cf9347f593360fff275852e'
tags:
  - method
  - review
  - borrowed-code
feature_id: feat_term-core_nx0g1r
---

## What it does

**Six of the fourteen findings of a single review came out of having copied the sibling piece. And the aggravating part: two of them were ALREADY FILED in this store.**

In F2.4 the panel border was written taking as its mould the dragging of the tree's dividers, which had been working for months. From that mould came **the four good guards** —not asking for keyboard focus, keeping the cursor on release too, bailing out if there is nothing to share out, not accumulating the delta— and **also two absences**:

- **Not filtering the mouse button.** In the mould it is a pre-existing defect nobody had noticed; on the new path it would have been a new defect. **It was written down** in [[a-drag-that-lasts-several-frames-in-egui]], point 3, with its example of the X11 paste.
- **A guard that protects against nothing.** The one for the value with no number is necessary in the mould because its state is a fraction; on the new path, which stores integers, it is a mechanism with no scenario — see [[an-integer-state-needs-no-nan-guard-but-does-need-a-ceiling]].

> **What fails is not being unaware of the rule.** The project's rule already says that borrowed code carries its reason and that one has to name what makes it applicable to your case. It fails because **a mould that works reads as validated**, and the absence of a guard in it reads as «not needed here» instead of as «nobody has looked at it». Copying is not inheriting decisions: it is inheriting the gaps as well, and the gaps are invisible because there is nothing written where there should have been.

**And having the record saved is not enough either.** The point about the button had been in the store for months and it was committed again, because whoever writes the new path looks at **the sibling piece**, not at the library's list of traps. The countermeasure is procedural and it is concrete: **every new drag control is gone over against the four points of that record, one by one**, before it is taken as done.

**What did work to catch it:** an **independent** reviewer who went to the library's source to check the semantics instead of reasoning about the names. Neither of the two absences is visible by reading the diff, because the diff **looks like** good code — it is good code, copied to a place where one of its decisions no longer applies and another is missing.

**And the corollary for the pre-existing defect that turns up along the way:** it is declared and ticketed separately, never dragged into the milestone that discovered it nor used to bless it. «**It is not filtered there either**» is not an argument — it is the confirmation that there are two defects instead of one.

## Where it lives

The case lives in `Pane::drag_its_panel_divider` and in `Window::drag_the_divider` (`crates/app/src/lib.rs`): the first is the copy, the second the mould. The mould's pre-existing defect —which still does not filter the button— is declared in the F2.4 review report and has a ticket of its own.
