Implementation cannot begin with verifiable results in this environment. Relaunch ticket 124 on a Linux worker with the pinned Rust 1.97.1 toolchain (cargo, rustc, rustfmt and clippy), workspace build dependencies, and access to a disposable systemd user session for controlled scope/OOM integration tests.

Observed at build pre-flight on 2026-11-18:
- cargo --version: /bin/sh: cargo: not found (exit 127).
- rustc --version: /bin/sh: rustc: not found (exit 127).
- systemctl --user is-system-running: /bin/sh: systemctl: not found (exit 127).
- /proc/1/comm: docker-init; /run/user and /run/systemd absent.
- No cargo found in /usr/bin, ~/.cargo/bin or /opt/rust/bin.

The approved design requires actual helper/PTY/cgroup tests, systemd-oomd attribution and sibling survival. A private D-Bus session alone cannot establish these. No production code changed and no checks are claimed to pass. Work remains at phase plan, on branch 124-console-systemd-scopes, ready to enter build on relaunch.

Configuration: FLOW.codex.md only. FLOW_CONTEXT=unattended selects missing FLOW.unattended.md. Core version 0.117.0; installed plugin manifest absent, so version comparison unavailable.
