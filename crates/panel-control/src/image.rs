//! What an image an agent opens is held to, in one place for the bridge that checks it on the
//! agent's side and the window that reads it (D120): the bridge's check is a courtesy that answers
//! at once, and the window's read is the enforcement, so the two must not drift.

use std::ffi::OsStr;
use std::path::{Component, Path, Prefix};

/// The largest image file the drawer reads.
pub const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Whether `path` names a file on this machine. On Windows an absolute path can name another
/// host's share or a device, and reading it would authenticate to that host on an agent's word; no
/// other platform has a prefix. **An allowlist, not a list of what leaves** (D120): a refusal list
/// missed `\\?\GLOBALROOT\Device\Mup\…`, which reaches the SMB redirector, and `\\?\unc\…`, which
/// std parses case-sensitively as a plain verbatim prefix while Windows does not.
pub fn is_local(path: &Path) -> bool {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => prefix_is_local(prefix.kind()),
        _ => true,
    }
}

/// A drive letter, in either spelling, or a volume named by its GUID — which is how `canonicalize`
/// names a volume with no letter. A mapped drive letter is the user's own mapping, not a host the
/// agent chose, so it passes (D120). Everything else under `\\?\` is the NT object namespace.
fn prefix_is_local(prefix: Prefix<'_>) -> bool {
    match prefix {
        Prefix::Disk(_) | Prefix::VerbatimDisk(_) => true,
        Prefix::Verbatim(component) => is_volume(component),
        _ => false,
    }
}

/// `Volume{…}`, matched without regard to case as Windows matches it.
fn is_volume(component: &OsStr) -> bool {
    let Some(name) = component.to_str() else {
        return false;
    };
    name.len() > "Volume{}".len()
        && name.is_char_boundary("Volume{".len())
        && name[.."Volume{".len()].eq_ignore_ascii_case("Volume{")
        && name.ends_with('}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_of_this_machine_is_local() {
        let here = std::env::temp_dir().join("shot.png");
        assert!(is_local(&here));
    }

    /// The decision over each prefix, on every platform: `Prefix` is built here by hand, so the
    /// allowlist is exercised where Windows' path parser is not.
    #[test]
    fn only_a_drive_or_a_volume_prefix_is_local() {
        let name = OsStr::new;
        for local in [
            Prefix::Disk(b'C'),
            Prefix::VerbatimDisk(b'Z'),
            Prefix::Verbatim(name("Volume{0c1a2b3c-0000-0000-0000-100000000000}")),
            Prefix::Verbatim(name("volume{0c1a2b3c-0000-0000-0000-100000000000}")),
        ] {
            assert!(prefix_is_local(local), "{local:?}");
        }
        for remote in [
            Prefix::Verbatim(name("GLOBALROOT")),
            Prefix::Verbatim(name("unc")),
            Prefix::Verbatim(name("Volume{")),
            Prefix::Verbatim(name("Volume{}")),
            Prefix::Verbatim(name("Volumes")),
            // Longer than `Volume{}` and opened like it, so only the closing brace refuses it.
            Prefix::Verbatim(name("Volume{0c1a2b3c-0000")),
            // The `€` spans bytes 5..8: slicing the prefix there would panic instead of refusing.
            Prefix::Verbatim(name("Volum€{abc}")),
            Prefix::UNC(name("host"), name("share")),
            Prefix::VerbatimUNC(name("host"), name("share")),
            Prefix::DeviceNS(name("PhysicalDrive0")),
        ] {
            assert!(!prefix_is_local(remote), "{remote:?}");
        }
    }

    /// The same forms as Windows parses them, the two that a refusal list let through included.
    #[cfg(windows)]
    #[test]
    fn a_share_or_a_device_is_not_local_on_windows() {
        for remote in [
            r"\\host\share\a.png",
            r"\\?\UNC\host\share\a.png",
            r"\\?\unc\host\share\a.png",
            r"\\?\GLOBALROOT\Device\Mup\host\share\a.png",
            r"\\.\PhysicalDrive0",
        ] {
            assert!(!is_local(Path::new(remote)), "{remote}");
        }
        for local in [
            r"C:\Users\me\a.png",
            r"\\?\C:\Users\me\a.png",
            r"\\?\Volume{0c1a2b3c-0000-0000-0000-100000000000}\a.png",
        ] {
            assert!(is_local(Path::new(local)), "{local}");
        }
    }
}
