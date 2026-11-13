# 69 — Codex mobile remote control and panel routing
Study only; size S provisional for guidance proposal, phase design.
Branch 69-codex-mobile-panel-routing from main afef5c7; no tracked changes.
Original runtime compatibility criteria remain open; guidance fallback is not approved.
Checked installed 0.163.0 and isolated exact 0.167.0 help/schema; no live daemon/MCP/phone test.
Generic per-thread configuration remains a candidate, not proved supported or impossible.
Independent review found high gaps: fallback condition unproved; version/runtime claims need restraint.
Pending design_challenge question asks retain investigation or explicitly choose limited guidance scope.
README, Connect visible/manual and copied prompt need consistent advice if guidance is chosen.
Relevant existing tests are in crates/app/src/tests/integrated_chrome.rs, not connection/tests.rs.
No schema/API changes, dependencies or external-repo work.
Resume design from pending; open 03-design.md, 02-research.md and design-challenge.md.
Never begin build while why=study.
