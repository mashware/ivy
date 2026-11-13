# 57 — Refuse unsafe inherited console directories
Size M; phase ship; branch 57-check-panel-dir-owner; 1/1 PR published; merge pending.
The single PR is recorded as published. This start run has no further work; waiting for merge.
Rust bridge/helper and direct OpenCode plugin validate directory owner, mode and type.
Symlink slash/dot forms refused; original path and gone/error diagnostics retained.
All six acceptance criteria proven by tests; wire/handshake contracts verified.
Workspace: 2101 passed, 0 failed, 7 existing ignored. Clippy, fmt and doc-check passed.
Plugin simulated/real socket: 2 each; Node24 recovery: 19; Python packaging: 23.
Initial suite exposed missing script-audit row for new test; fixed and full rerun passed.
Review: no findings; cost 4/12 review subagent runs across initial/delta review.
No migrations, dependencies, cross-repo work or outstanding questions.
Accepted limits: pathname race, ancestor hardening, arbitrary file producers; Linux runtime checks only.
