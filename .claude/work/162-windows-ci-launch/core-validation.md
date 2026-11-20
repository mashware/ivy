# Core Windows test portability validation

Validation covers `term-core`, `panel-control`, and `panel-contract` after the portability changes. No native Windows runner was available; Windows validation used the installed `x86_64-pc-windows-msvc` target.

Commands and results:

- `PATH=/usr/local/cargo/bin:$PATH cargo fmt -p term-core -p panel-control -p panel-contract` — exit 0.
- `PATH=/usr/local/cargo/bin:$PATH cargo test --locked -p term-core -p panel-control -p panel-contract --quiet` — exit 0. Unit/integration suites passed: panel-contract 130; panel-control 112 + 2 + 29 + 2 + 2 + 8 + 4 + 7; term-core 236 passed, 3 ignored, plus integration suites 4 and 1. Doc tests passed.
- `PATH=/usr/local/cargo/bin:$PATH cargo clippy --locked -p term-core -p panel-control -p panel-contract --all-targets -- -D warnings` — exit 0.
- `PATH=/usr/local/cargo/bin:$PATH cargo check --locked -p term-core -p panel-control -p panel-contract --all-targets --target x86_64-pc-windows-msvc` — exit 0.
- `PATH=/usr/local/cargo/bin:$PATH cargo clippy --locked -p term-core -p panel-control -p panel-contract --all-targets --target x86_64-pc-windows-msvc -- -D warnings` — exit 0.
- Cargo test listing audit compared every entry in `core-omissions.json` with the Linux package test list — 79 inventory entries, zero missing test names.

Windows execution remains unverified locally. The target compilation validates Windows cfg arms and test harness compilation; the CI test runner provides runtime verification.

ConPTY EOF evidence: F2.7 in `docs/05-roadmap.md:349` records that the pseudoconsole owns the pipe write end and `drop(pair.slave)` does not close it, so EOF does not arrive while the console lives. In portable-pty 0.9.0, `src/win/conpty.rs:14-24` passes `stdout.write` to `PsuedoCon::new`; `:26-42` keeps the master and slave on an `Arc`-shared inner; and `:46-50` stores the `PsuedoCon` in that inner. Thus dropping the child process or slave does not close the output writer while the master remains alive. The two tests that assert reader EOF with a live master are listed as Windows omissions. Portable output/environment tests instead collect only through their asserted marker on Windows, while retaining their full EOF drain on Unix.

## Final socket-audit correction
At 5e5c5dd, 24 additional Unix socket-dependent tests received Windows ignored twins and helper/import gates. The core inventory has 103 entries; the global compiler-backed Linux inventory has 379. Final combined MSVC all-target Clippy for term-core, panel-control, panel-contract and host-load passed (windows-subset-validated.log). Full Linux workspace validation passed (tests-final-reviewed.log); earlier 79-entry counts describe the preceding audit, not the final inventory.
