//! The row above a file in the code view: where it lives, and what can be done with it.
use std::path::Path;

/// `path` as seen from `root`, the Files folder it was opened from; nothing when it lies outside.
///
/// Counted on the path as Files listed it, not on where its links lead: a file reached through a
/// linked folder inside the tree is still where the person saw it.
pub(super) fn relative<'a>(path: &'a Path, root: Option<&Path>) -> Option<&'a Path> {
    path.strip_prefix(root?)
        .ok()
        .filter(|rest| !rest.as_os_str().is_empty())
}

#[cfg(test)]
mod tests;
