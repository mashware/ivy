//! Locks the complete portable-pty package and its exact local patch, not runtime behavior.
use std::{
    fs,
    path::{Path, PathBuf},
};
const FILES: &[(&str, usize, u64)] = &[
    (".cargo_vcs_info.json", 97, 0x88fbbc849651df08),
    ("Cargo.lock", 18976, 0x00ff6cd422d6dc6b),
    ("Cargo.toml", 2264, 0xf9dad8af8f565204),
    ("Cargo.toml.orig", 936, 0xa7fc7ef25edccec6),
    ("IVY-PATCH.md", 823, 0xa9876ddf8626a0fc),
    ("LICENSE.md", 1068, 0x028bb86a5aba23b7),
    ("UPSTREAM-SHA256", 1407, 0x07bbe12cabb5cfa9),
    ("examples/bash.rs", 2492, 0xdf4d96f44f65fa17),
    ("examples/narrow.rs", 3305, 0xd402e3a9ec1cb11b),
    ("examples/whoami.rs", 3587, 0xaa2da9a28dc8306d),
    ("examples/whoami_async.rs", 2529, 0x181e74bcfd65cfc1),
    ("src/cmdbuilder.rs", 30143, 0x583a665eed4567f4),
    ("src/lib.rs", 13378, 0x5676f8a9cf1ce2d9),
    ("src/serial.rs", 9219, 0xd6a84d39505bec40),
    ("src/unix.rs", 12978, 0xb18ecd58b66e4eeb),
    ("src/win/conpty.rs", 2984, 0x83e7070889418e49),
    ("src/win/mod.rs", 4398, 0x22d43d029191e19b),
    ("src/win/procthreadattr.rs", 2183, 0x3428a9f4fec7a41e),
    ("src/win/psuedocon.rs", 6034, 0xab2af26c3e504a15),
];
fn fingerprint(bytes: &[u8]) -> (usize, u64) {
    let bytes = String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace("\r\n", "\n");
    (
        bytes.len(),
        bytes.bytes().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
        }),
    )
}
fn walk(base: &Path, dir: &Path, files: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(base, &path, files);
        } else {
            files.push(
                path.strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}
#[test]
fn portable_pty_patch_and_complete_package_are_pinned() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(manifest.contains("portable-pty = { path = \"vendor/portable-pty\" }"));
    let base = root.join("vendor/portable-pty");
    let mut present = Vec::new();
    walk(&base, &base, &mut present);
    present.sort();
    assert_eq!(
        present,
        FILES.iter().map(|(p, _, _)| *p).collect::<Vec<_>>()
    );
    for (path, length, hash) in FILES {
        let data = fs::read(base.join(path)).unwrap();
        assert_eq!(
            fingerprint(&data),
            (*length, *hash),
            "vendor file changed: {path}"
        );
    }
}
