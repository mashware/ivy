# Context #58

## Ticket
- Selected terminal text sometimes shows narrow vertical gaps between characters and a fragmented selection highlight.
- Reported while viewing Codex output in Ivy; most visible on selected command lines with `--repo`, `--body-file`, `git.zabri.io issue create`.
- Intermittent; trigger not isolated; steps not independently reproduced.
- Unknown: Ivy version, font, display scale, selection gesture, whether copied text is altered.
- Root cause unconfirmed. Labelled `bug`; started through the feature flow with `--study` (studied here, built elsewhere).

## Acceptance criteria (provisional — promoted to first-class criteria in the design phase)
- Selecting text preserves terminal cell spacing; glyphs stay readable.
- No unexpected gaps or fragmentation in the selection highlight.

## Decided in the ticket thread
empty thread

## Relevant domain knowledge
- D34 / epaint rounds each glyph origin separately (`round_text_to_pixels`); a fractional cell step opens a 1 px column every few cells. Fixed by flooring the cell to whole device pixels — but the card is marked drifted against `painter.rs`, so check the fix still holds on every path (selection, ligatures).
- Filled rectangles are rounded by epaint too (`round_rects_to_pixels`), so adjacent backgrounds tile; a seam in the highlight therefore points at something other than rect rounding (per-run painting, a different step, a layer).
- A guard written from the same model as the code catches nothing: assert the reason (cell never wider than the advance), not the shape.
- Hypothesis worth checking first (not a finding): optional font ligatures (#335) — `--` is a ligature in many coding fonts, and a selection boundary or colour change splitting a shaped run could misplace glyphs.

## Repo state at start
- Branch: 58-selection-highlight-gaps (worktree `.worktrees/58-selection-highlight-gaps`, from origin/main)
- Last commit: 9e30f7a 51 Agents in an Ivy console are asked to publish their panel [minor] (#55)

## Decisions clarified at start
no open questions — the unknowns (font, scale, trigger) are investigation material for design, not product decisions.

- Linked branch (`gh issue develop`) skipped: the study is handed off and the PR train is not known yet (FLOW note for feat:start).
- Roadmap: not linked — no item names #58.

## Estimated size: S
Probably a painter-side fix in one or two files (selection/run painting). Root cause unknown, so design may reclassify.
