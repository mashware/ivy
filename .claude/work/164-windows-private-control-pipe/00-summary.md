# Ticket 164 — Windows private control channel
Size L; phase plan complete; one planned PR #1, pending study approval.
F1 answered: retain bounded shutdown and zero workers at Drop return.
Windows: native secure overlapped pipe; explicit protected user+SYSTEM ACL; remote rejection; first-instance ownership.
Client authenticates PANEL_SERVER_PID; identity arrays remain two slots; PID is separate on Windows.
Lifecycle: callback-free Windows service API, bounded committed snapshots, nonblocking native UI wake, reliable release slots.
Unix behavior and Windows unsupported native session/overlay authorization remain unchanged.
Literal contracts and AC1–AC7: 03-design.md, including Lifecycle revision implementing the answered F1.
Independent lifecycle/security challenges: no remaining high blocker; planning recommends one atomic delivery.
Delivery: 04-mr-plan.md; advisory ~2400 lines/~32 files, never a target.
#161 already on origin/main at 8380c4e; ticket_deps=[]; no schema changes.
No production code or tests executed; native ACL, remote denial, UI wake, shutdown and painting proof remain required.
Study stops before code; done-summary.md is the reviewable study for approval.
Next build opens 03-design.md and PR #1 in 04-mr-plan.md in full.
