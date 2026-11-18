# 124 — Separate console systemd scopes
Size: M. Phase: plan; build pre-flight blocked. PR #1 of 1 pending.
Branch: 124-console-systemd-scopes from main (161c80f).
Approved study preserved; no implementation or production changes.
Blocker: cargo, rustc and systemctl absent; PID 1 is docker-init.
Next action: relaunch on a Rust 1.97.1 worker with a disposable systemd user session.
Resume: build pre-flight, then the approved one-PR plan.
Evidence: blocked-reason.md. Full approved study: done-summary.md.
