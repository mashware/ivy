# #54 — publish a panel from a shell command (panel.publish_cmd)

- What: `ivy-session publish` lets flow's hook repaint a pane its agent holds, so an answered question stops showing «waiting for you» and plan ticks show without the agent republishing.
- Size: **L** · 2 PRs · branch `54-panel-publish-from-shell` (worktree) · study for handoff (`ai-study/#54`), built elsewhere.
- Plan: #1 macOS process attestation (wave 1, ~700 lines) → #2 the command end to end (wave 2, ~1 500 lines, closes #54). `04-mr-plan.md`.
- Stands: D84 attestation (identity only, `check_mode` off) bound to the holder's client root; no token; PANEL_ID never the proof.
- Stands: hook replaces lines/attention/phase; `header` stays; holder's `actions` stay only while the hook still says `wait` (owner).
- Stands: any descendant of the agent may repaint (owner); reply once queued, window re-checks at apply; hold recorded once, by the window.
- Stands: macOS done properly (owner), unverified until the owner's Mac run; session recovery stays Linux-only (F4).
- Contracts: wire `panel_publish`, CLI `ivy-session publish` — literal in `03-design.md` "External contracts".
- Follow-ups: F1 bench gap, F4 recovery on macOS, F5 client path matching.
- Next: handoff of the study — open `03-design.md`, `04-mr-plan.md`.
