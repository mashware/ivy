# portable-pty 0.9.0 local patch

Published crates.io package, including its MIT license and published metadata. `UPSTREAM-SHA256` records the original files before modification; `.cargo-ok` is Cargo extraction state, not package content. The crate archive checksum is recorded in the root Cargo.lock history and here: `b4a596a2b3d2752d94f51fac2d4a96737b8705dddd311a32b9af47211f08671e`.

Only `src/cmdbuilder.rs`, `src/win/psuedocon.rs` and `Cargo.toml` differ: Windows opt-in required cwd passes the selected string to CreateProcessW without home fallback; malformed required state errors; legacy and Unix paths stay unchanged. The manifest adds serde_json solely for compatibility tests. The application source guard fingerprints every file and locks the exact patched bytes, tolerating only checkout CRLF normalization.
