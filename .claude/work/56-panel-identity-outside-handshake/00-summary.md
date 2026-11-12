# #56 — panel identity outside the MCP handshake

- What: a borrowed console identity, a dead endpoint promised a panel, the hold outliving `done`, three readers with their own identity rule.
- Size: M · study only (`handoff: true`, built elsewhere via `ai-study/#56`) · branch `56-panel-identity-outside-handshake`.
- Decisions: outside caller is **labelled, never refused** (user); label on **Linux and macOS** (user), Windows unchanged.
- `done` releases the hold **to the file only**: the holder re-claims with its next publish and can still retire; others stay refused.
- Bridge withholds handshake instructions when `PANEL_DIR/control.sock` is absent (stat, `socket_present`); `sun_path` check cut.
- Helper → `whole_identity`; rc dir → public `base_dir`; Connect guide asks for the manual prompt only where server instructions did not arrive, + subagent rule.
- Decisions to amend: D7, D43, D88 (D50 untouched).
- Contracts: MCP `initialize` `instructions` absent also without the socket; app-row literal `published from a process outside this console` (03-design.md).
- Criteria: AC1–AC12 in `03-design.md`; AC1/AC2 need a real Mac.
- Depends on: none (#57, #54 either order).
- Plan: 2 PRs, one wave, no dependency — #1 hold-on-done + items 2,4,5,6 (AC5–AC12, ~550 lines); #2 outside label Linux+macOS (AC1–AC4, ~900 lines). `04-mr-plan.md`.
- Next: hand off the study (`ai-study/#56`).
