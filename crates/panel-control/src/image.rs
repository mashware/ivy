//! What an image an agent opens is held to, in one place for the bridge that checks it on the
//! agent's side and the window that reads it (D120): the bridge's check is a courtesy that answers
//! at once, and the window's read is the enforcement, so the two must not drift.

use std::path::{Component, Path, Prefix};

/// The largest image file the drawer reads.
pub const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Whether `path` names a file on this machine. On Windows an absolute path can name another
/// host's share or a device (`\\host\share\a.png`, `\\?\UNC\…`, `\\.\…`), and reading it would
/// authenticate to that host on an agent's word; no other platform has such a prefix.
pub fn is_local(path: &Path) -> bool {
    !matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..) | Prefix::DeviceNS(..))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_of_this_machine_is_local() {
        let here = std::env::temp_dir().join("shot.png");
        assert!(is_local(&here));
    }

    /// The three prefixes that leave the machine, and the drive forms that do not.
    #[cfg(windows)]
    #[test]
    fn a_share_or_a_device_is_not_local_on_windows() {
        for remote in [
            r"\\host\share\a.png",
            r"\\?\UNC\host\share\a.png",
            r"\\.\PhysicalDrive0",
        ] {
            assert!(!is_local(Path::new(remote)), "{remote}");
        }
        for local in [r"C:\Users\me\a.png", r"\\?\C:\Users\me\a.png"] {
            assert!(is_local(Path::new(local)), "{local}");
        }
    }
}
