# 164 — Windows private control channel
Size M provisional; phase design; branch 164-windows-private-control-pipe.
Study only: no code, commit or PR; base 137fd348165866dcd3382608a811d0c594119f42.
Native secure overlapped Windows pipe recommended; wire unchanged; Unix preserved.
#161 already on main at 8380c4e; ticket_deps is empty.
Pending design_challenge F1: bounded zero-worker Drop cannot rely on cancellation of I/O alone.
Owner decides whether to expand cancellable handler/release lifecycle or revise shutdown criterion.
Public platform wrappers and separate Windows PID launch entry resolve F2/F3.
Remote denial needs actual native proof; ACL tuple checks specified.
04-mr-plan.md is a draft one-PR plan (~1600 lines / 25 files), not registered or approved.
Next: apply answer to 03-design.md, revise lifecycle and size, then close design and plan.
Open 03-design.md, challenge.md and ask-question.md in full when resuming.
