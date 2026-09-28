//! What more than one test binary of this crate needs, written **once**.
//!
//! The guards of this suite grew one delivery at a time, and each copied the nearest piece it needed
//! instead of sharing it: the accent folding was byte-identical in two files, and of three lists of
//! the crate's files only one was checked against the tree. A copy is a second place to keep up to
//! date, and the first time the two disagree nothing says which one is wrong.
//!
//! Every binary that declares `mod common;` compiles the whole module and calls part of it, so an item
//! one binary does not call is dead code in that binary. That is what the `allow` is for, and nothing
//! else.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::Path;

/// Lowercase, and Spanish accents folded onto ASCII.
///
/// **Both sides get folded**, the text judged and the list it is judged against, so an entry written
/// without its accent finds the word written with it. That is why every word list of these guards is
/// ASCII on purpose.
///
/// **It lowercases first, so only the lowercase accents need an arm.** Six uppercase arms with no
/// caller are six arms no test can reach.
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Every Rust file below each of `dirs`, as a path relative to `root` with `/` separators.
///
/// **This discovers names only.** The guards take the text they judge from `include_str!`, so a
/// declared file that is renamed or deleted is a compile error; the walk closes the opposite hole, a
/// file that exists on disk and that no list declares.
pub fn rust_sources(root: &Path, dirs: &[&str]) -> BTreeSet<String> {
    fn walk(dir: &Path, root: &Path, found: &mut BTreeSet<String>) {
        let entries = std::fs::read_dir(dir)
            .unwrap_or_else(|failure| panic!("cannot read {}: {failure}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|failure| {
                panic!("cannot read an entry in {}: {failure}", dir.display())
            });
            let path = entry.path();
            let file_type = entry.file_type().unwrap_or_else(|failure| {
                panic!("cannot read the file type of {}: {failure}", path.display())
            });
            if file_type.is_dir() {
                walk(&path, root, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let relative = path
                    .strip_prefix(root)
                    .expect("a file found below the root has the root as its prefix")
                    .to_string_lossy()
                    .replace('\\', "/");
                assert!(
                    found.insert(relative.clone()),
                    "the source walk found {relative} twice"
                );
            }
        }
    }

    let mut found = BTreeSet::new();
    for dir in dirs {
        walk(&root.join(dir), root, &mut found);
    }
    found
}

/// Whether a declared list of files is the set of files on disk, with the difference as the error.
///
/// Both directions are reported, because a list can be wrong both ways: a file on disk with no row
/// has no guard, and a row with no file is a guard reading nothing. A `Result` and not an `Option`
/// so that a call whose answer nobody reads is a warning rather than a guard that never fails.
pub fn list_against_disk(
    list: &str,
    declared: &[&str],
    on_disk: &BTreeSet<String>,
) -> Result<(), String> {
    let set: BTreeSet<String> = declared.iter().map(|name| (*name).to_owned()).collect();
    if set.len() != declared.len() {
        return Err(format!(
            "{list} has {} rows but only {} different paths: a duplicate hides a file that is absent",
            declared.len(),
            set.len()
        ));
    }
    let undeclared: Vec<_> = on_disk.difference(&set).collect();
    let absent: Vec<_> = set.difference(on_disk).collect();
    if undeclared.is_empty() && absent.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{list} is not the set of files on disk. Files with no row: {undeclared:?}; rows with no \
         file: {absent:?}"
    ))
}
