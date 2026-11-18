# Implementation 107 — Mascot colour comparison

## Brief
recorded (unattended)

After this work:
- The owner can assess whether the mascot colour matches between Ivy and Tilix under the same theme.
- Paired captures and interior pixel samples establish the observed result.
- Foreground escape sequences and active attributes explain the colour source.

This work does NOT include:
- A speculative palette or renderer adjustment.
- A replacement comparison using a different terminal or theme.
- A claim of equality without paired measurements.

## Contracts to respect — none changed in design
The design declares no changed external contracts.

plan: skipped — single comparison, no MR/PR plan.

## Execution
- [ ] AC1: Record matching application/theme and terminal/display settings.
- [ ] AC2: Capture both terminals and sample three interior pixels per mascot.
- [ ] AC3: Identify emitted foreground and active attributes; explain the comparison.

## Decisions made during implementation
- Approved study retained; no production change justified without measurements.
- Headless GUI helper discovered at `/usr/local/bin/ivy-gui`; unset DISPLAY alone is not evidence that GUI execution is unavailable.

## Environment verification
- `ivy-gui /opt/ivy-gui/smoke.sh /work/repo/.claude/work/107-compare-mascot-colour/environment-evidence` exited 0: window activation, mouse click, keyboard input and PNG capture passed. Capture: `environment-evidence/terminal.png`; this is a GUI smoke capture, not mascot comparison evidence.
- `claude --version` exited 0: 2.1.310 (Claude Code).
- `CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup /usr/local/cargo/bin/cargo --version` exited 0: cargo 1.97.1. The toolchain exists outside PATH; no missing-toolchain blocker remains.
- Tilix has no executable in PATH or under `/usr/bin`, `/usr/local/bin`, `/opt`; there is no cached Tilix package in `/var/cache/apt/archives`. The runner is unprivileged (uid 1000), and `sudo` is absent.
- Cached `/opt/cargo-target/{debug,release}/app` binaries exist but predate the checkout. They were not used as evidence for this commit.
- `git fetch origin main` succeeded; HEAD and origin/main both resolve to `074f0e52df83b17c207a99d45f3d2359ce55ba1a`. No fast-forward was needed.

## Result
Blocked before AC1: the approved reference terminal, Tilix, is unavailable. AC1–AC3 remain unverified; no mascot screenshot, RGB measurement or emitted-sequence result was fabricated. The virtual display is usable, correcting the study's assumption that an unset DISPLAY prevents graphical execution. No production files changed; no repository tests run because there is no code change and unit tests cannot prove the paired rendering comparison.

## Next action
Provide Tilix and its runtime dependencies in the runner, then relaunch ticket 107 with `why=start` to resume the unchecked criteria on the same virtual display. Alternatively perform the approved procedure on a host with both terminals and provide the paired lossless captures, configuration and relevant PTY output. Match dark-daltonized in the same Claude Code version. Do not substitute xterm results for the approved Ivy/Tilix comparison.
