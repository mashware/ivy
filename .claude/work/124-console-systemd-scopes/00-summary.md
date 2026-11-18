# 124 — Separate console systemd scopes
Size: M. Phase: plan. PR #1 of 1 pending.
Branch: 124-console-systemd-scopes from main.
Study complete; no production changes or runtime validation.
Choose gated helper before exec plus optional user-manager scope.
Linux startup asynchronous; absent manager runs original once unscoped.
OOM notice requires scope Result=oom-kill; unknown signals stay generic.
Startup deadline 1 second; final observation 500 ms from root exit.
Independent challenge resolved; no high findings.
One PR, no schema changes or ticket dependencies.
Next: owner approves study, then build opens 03-design.md and 04-mr-plan.md.
Full approval summary: done-summary.md.
