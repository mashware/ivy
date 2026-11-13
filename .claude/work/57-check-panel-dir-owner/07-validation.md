# Validation 57

## Criteria coverage
| Criterion | Proof type | Test / confirmation | Status |
|---|---|---|---|
| AC1 genuine consoles | test | inherited_directory::bridge_connects_to_a_genuine_private_console; session_helper::helper_reports_only_allowed_metadata_with_its_actual_kernel_identity; plugin real-socket route bootstrap/exchange | proven-by-test |
| AC2 group/other bits refused before connect | test | endpoint::tests::inherited_directory_requires_private_mode_type_and_effective_owner; inherited_directory::unsafe_inherited_directories_never_reach_a_listening_socket; plugin private directory fixture | proven-by-test |
| AC3 wrong effective uid | test | Rust private_to_uid injected expected uid; JS metadata fixture injected uid | proven-by-test |
| AC4 final symlink spellings | test | Rust unit and listener subprocess fixtures, JS real lstat fixture; alias, alias/, alias/., repeated slash/dot | proven-by-test |
| AC5 missing path | test | Rust unit and both subprocess clients assert original path + directory is gone, no privacy label; JS no-connect fixture | proven-by-test |
| AC6 retry/recheck/repair/disposal | test | plugin controlled timers assert 1000 ms, fresh lstat count, unchanged socket count on refusal, repair enables connection, close removes timer | proven-by-test |

## Test coverage
- Added Rust unit predicate matrix; added Rust inherited clients refusal fixture and genuine inherited bridge success.
- Added JS controlled retry/metadata fixture, including real lstat symlink spellings and genuine directory positive control.
- Existing helper success and direct plugin real-socket tests retained and executed.

## Suite results
- cargo test --locked -p panel-control: passed before final positive test; final inherited_directory focused run 2/2 passed.
- node --test resources/agent-integrations/opencode/plugin.test.mjs: 2 passed.
- IVY_REAL_SOCKET=1 node --test resources/agent-integrations/opencode/plugin.test.mjs: 2 passed.
- npm test --prefix integrations/session-recovery under Node 24: 19 passed. Default Node 22 rejects the existing --test-isolation=none flag; no repository change required.
- python3 -m unittest scripts/test_release.py scripts/test_macos.py scripts/test_windows.py -v: 23 passed.
- cargo doc-check --locked: passed; 23 private-item-link warnings in untouched existing documentation.
- cargo fmt --all --check: passed.
- Initial full workspace run: source-audit registry omitted new inherited_directory.rs; fixed by registering the file, assertions retained. Focused scripts suite: 5 passed. Final whole-suite rerun: cargo test --locked --workspace passed, 2101 passed, 0 failed, 7 existing ignored across 30 result groups.
- Final workspace Clippy: passed.

## Performance
Independent reader: no actionable findings. One lstat per attempt, existing one-second timer, no accumulated retries. No queries touched. No bench_cmd configured; no timing claim.

## Edge cases verified
- [x] Every group/other mode bit; wrong uid without root/chown; ordinary file; metadata ENOTDIR error.
- [x] Missing paths retain gone diagnosis; no fallback to connect.
- [x] Final symlink terminal slash/dot variants rejected; genuine trailing variants accepted.
- [x] Whole supported identity still receives handshake instructions; its first tool request refuses an insecure path.
- [x] Retry repair and disposal, no cached frames while refused.

## Regressions and limits
Protocol shape and absent-identity behavior preserved. Linux host execution only; native macOS/Windows execution unavailable here. Check/connect race, ancestor hardening and arbitrary external file producers remain accepted scope limits, as approved in study.

Validated revision: ca90853218a3862cdc2c5fadf8e8ab68f69e260a (same production code as reviewed ec654cd; test-only delta independently reviewed).
