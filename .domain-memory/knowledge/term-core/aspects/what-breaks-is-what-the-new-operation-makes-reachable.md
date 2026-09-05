---
id: asp_what-breaks-is-what-the-new-operation-makes-reachable_d1y48j
slug: what-breaks-is-what-the-new-operation-makes-reachable
name: >-
  What breaks when an operation is added is not its code: it is the defects that
  were already there and that it makes reachable — 16 of 17 blockers in two PRs
type: aspect
status: active
superseded_by: null
confidence: 80
created_at: '2026-08-16T23:13:05.089Z'
updated_at: '2026-08-17T12:00:32.902Z'
last_verified: '2026-08-17T12:00:32.902Z'
file_paths:
  - crates/app/src/lib.rs
  - crates/app/src/measure.rs
symbols:
  - 'Node::split'
  - 'Node::collapse'
  - 'Tab::move_in'
  - 'Window::publish_the_report'
  - 'Window::close_the_focused'
  - 'measure::side'
  - 'Window::some_closed_by_hand'
content_hashes:
  crates/app/src/lib.rs: 'sha256:69e903da2ae25795351e9b8a05001efaad7c4703b59964f4e430a94a6176c041'
  crates/app/src/measure.rs: 'sha256:627412ac283ad6138a2b514940f63e45eb84d875a0405dd62991852b0442a69c'
tags:
  - review
  - risk
  - pane-tree
  - F2.2
feature_id: feat_term-core_nx0g1r
---

## What it does

**The pattern, with its count, and it is already two PRs in a row.** The PR that taught how to **split** a pane closed **fourteen blocking findings** and **thirteen were in code that PR does not write**. The one that taught how to **close** closed **three** and **all three** were outside its code. Thirteen plus three is **sixteen of seventeen**; the title of this record said «15 of 17» for a day, and the PR that closes the milestone corrected it when it went to cite it in a versioned decision — a record's arithmetic is also checked before publishing it. None of them was a new defect: they were states that until then **could not be reached from the application**, and the new operation put them one gesture away.

**Of splitting, the three that teach the most:**

1. **The report panicked with three panes.** The title of each span table indexed an array of two sides —«left», «right»— by position. With two fixed panes it was correct by construction; with three, it is a panic **inside the report that gets published on exit**, that is, losing the whole measured session at once. The same defect had already been fixed in the header a PR earlier, and it stayed alive in the title because nobody had been able to get to three.

2. **Dragging one divider moved the sibling subtree's.** The search for the divider by its number used the «the weight changed» boolean as if it said «I have found it». Finding it and not moving it —the frame in which the pointer stays still— was read as «it is not here», the search went on down the next branch, and since the count had not advanced, the sibling's matched the same number. **Unreachable while the layout was one branch of two leaves**: two sibling branches were needed, that is, two splits.

3. **A premise of the design turned out to be false.** The design discarded giving each divider a stable identity with this argument: «the position only changes on splitting or closing, and neither of the two can happen in the middle of a drag — it is one gesture at a time». **There are two hands**: with the mouse held down on a divider and the split chord with the other, the dividers get renumbered and the in-flight drag moves a different one, because egui's control identifier is **positional**.

**Of closing, the two the pattern predicts and nobody saw by reading the change:**

4. **The window closed on its own taking away what was being read.** The rule «the window closes when all the programs have died» has an exception from F2.1: if the user has closed something **by hand**, they are in charge. That flag was raised by closing a tab, and closing a **pane** did not raise it. With an already finished console next to the agent's, closing the agent's left «everything dead» and **the window disappeared with the content that was being looked at**. The asymmetry was in plain sight in the new code: closing the **only** console of a tab delegates to closing the tab, which does raise it — that is, **the same gesture protected or not depending on how many siblings the console had**.

5. **A text of the report started lying without anyone touching it.** When the side hint does not apply, the report said «no side, which **with more than two** no longer describes anything». While a tab always had two consoles or more —it was born with two and splitting only added— it was true. **Closing makes the one-console tab reachable**, and there that line claims the opposite of what happens. Same place, same mechanism and same originating PR as case 1: **the report is where this pattern strikes most often**, because it is full of sentences that describe the geometry of back then.

> **The rule: when adding an operation, the question is not «what can fail in what I write?» but «which states become reachable for the first time, and who was taking them for impossible?».** All the code that said «this cannot happen» —by the type, by the fixed number, by the single gesture— has to be reread with the new operation in front. And not only the code: **the sentences**. A message explaining why something does not apply carries inside it an assumption about how many there were.

**And how they are found, because they do not come out by reading the diff:** of splitting, two of three came out by **mutating** (cutting the recursive descent left twelve tests green) and the third from an external reviewer. Of closing, the two were brought by **the only reviewer of four who delivered a report**, and neither had been seen by whoever wrote the change going over their own code. Reading the change is not enough: the defect is in the file the change **does not touch**.

## Where it lives

`crates/app/src/lib.rs` — `Node::split` (the descent), `Tab::move_in` (the divider search), `Window::publish_the_report` (the title of each table and the focus line), `Window::close_the_focused` and the `some_closed_by_hand` flag; `crates/app/src/measure.rs` — `side()`, the rule of when the side describes anything. Lived through in F2.2, PRs #73 (splitting) and #74 (closing).
