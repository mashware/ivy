# Ticket 164 — private Windows control channel
Size L; one draft PR handed to the orchestrator.
Branch 164-windows-private-control-pipe; reviewed and validated 4ed9487a44b152a773109cf83a7a55b3c0417b47.
Windows private ACLs, expected PID, cancellable transport and joined workers implemented.
Bounded snapshots, native wake and release slots integrated; wire unchanged.
Independent review resolved authentication/revocation and fixture issues; no blockers remain.
Cost: 9/12 subagents launched; 12 reviewer turns; broad effort max.
Linux workspace: 2691 passed, 0 failed, 16 ignored; fmt/clippy/docs pass.
GNU app tests and MSVC contract/control tests cross-compile; packaging 23 and JS 19 pass.
Windows CI must execute the native matrix; no native runtime success claimed here.
F1 distinct-user/remote proof and F2 intended-pane/resting/minimized painting remain human checks.
Exact owner procedure is in the draft body. No schema change or pending question.
