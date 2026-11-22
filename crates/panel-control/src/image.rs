//! What an image an agent opens is held to, in one place for the bridge that checks it on the
//! agent's side and the window that reads it (D120): the bridge's check is a courtesy that answers
//! at once, and the window's read is the enforcement, so the two must not drift.

use std::ffi::OsStr;
use std::path::{Component, Path, Prefix};

/// The largest image file the drawer reads.
pub const MAX_BYTES: u64 = 32 * 1024 * 1024;

/// Why a path is not one the window reads on an agent's word, as the agent is told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elsewhere {
    /// A share: reading it would authenticate to that host.
    AnotherMachine,
    /// A device or object-namespace path. Its text cannot say whether it stays on this machine —
    /// `\\?\GLOBALROOT\Device\HarddiskVolume3\…` does, `\\?\GLOBALROOT\Device\Mup\…` does not — so
    /// the agent is told the form it can give instead of being told where the file is.
    NotADrivePath,
}

impl Elsewhere {
    /// What follows the path in the sentence the agent reads.
    pub fn says(self) -> &'static str {
        match self {
            Elsewhere::AnotherMachine => "is not on this machine",
            Elsewhere::NotADrivePath => "is not a drive path; give it as C:\\…",
        }
    }
}

/// Whether `path` names a file on this machine. On Windows an absolute path can name another
/// host's share or a device, and reading it would authenticate to that host on an agent's word; no
/// other platform has a prefix. **An allowlist, not a list of what leaves** (D120): a refusal list
/// missed `\\?\GLOBALROOT\Device\Mup\…`, which reaches the SMB redirector, and `\\?\unc\…`, which
/// std parses case-sensitively as a plain verbatim prefix while Windows does not.
pub fn is_local(path: &Path) -> bool {
    elsewhere(path).is_none()
}

/// Why `path` is refused by its text alone, or `None` when it is on this machine (see `is_local`).
pub fn elsewhere(path: &Path) -> Option<Elsewhere> {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => prefix_elsewhere(prefix.kind()),
        _ => None,
    }
}

/// A drive letter, in either spelling, or a volume named by its GUID — which is how `canonicalize`
/// names a volume with no letter — is local. A mapped drive letter is the user's own mapping, not a
/// host the agent chose, so it passes (D120). Everything else under `\\?\` and `\\.\` is the NT
/// object namespace, of which only the `UNC` spellings are known to name a share.
fn prefix_elsewhere(prefix: Prefix<'_>) -> Option<Elsewhere> {
    match prefix {
        Prefix::Disk(_) | Prefix::VerbatimDisk(_) => None,
        Prefix::Verbatim(component) if is_volume(component) => None,
        Prefix::UNC(..) | Prefix::VerbatimUNC(..) => Some(Elsewhere::AnotherMachine),
        Prefix::Verbatim(component) | Prefix::DeviceNS(component)
            if component.eq_ignore_ascii_case("UNC") =>
        {
            Some(Elsewhere::AnotherMachine)
        }
        _ => Some(Elsewhere::NotADrivePath),
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
            assert_eq!(prefix_elsewhere(local), None, "{local:?}");
        }
        for share in [
            Prefix::UNC(name("host"), name("share")),
            Prefix::VerbatimUNC(name("host"), name("share")),
            Prefix::Verbatim(name("unc")),
            Prefix::DeviceNS(name("UNC")),
        ] {
            assert_eq!(
                prefix_elsewhere(share),
                Some(Elsewhere::AnotherMachine),
                "{share:?}"
            );
        }
        for device in [
            Prefix::Verbatim(name("GLOBALROOT")),
            Prefix::Verbatim(name("Volume{")),
            Prefix::Verbatim(name("Volume{}")),
            Prefix::Verbatim(name("Volumes")),
            // Longer than `Volume{}` and opened like it, so only the closing brace refuses it.
            Prefix::Verbatim(name("Volume{0c1a2b3c-0000")),
            // The `€` spans bytes 5..8: slicing the prefix there would panic instead of refusing.
            Prefix::Verbatim(name("Volum€{abc}")),
            Prefix::DeviceNS(name("PhysicalDrive0")),
            Prefix::DeviceNS(name("C:")),
        ] {
            assert_eq!(
                prefix_elsewhere(device),
                Some(Elsewhere::NotADrivePath),
                "{device:?}"
            );
        }
    }

    /// The same forms as Windows parses them, the two that a refusal list let through included.
    #[cfg(windows)]
    #[test]
    fn a_share_or_a_device_is_not_local_on_windows() {
        for (remote, why) in [
            (r"\\host\share\a.png", Elsewhere::AnotherMachine),
            (r"\\?\UNC\host\share\a.png", Elsewhere::AnotherMachine),
            (r"\\?\unc\host\share\a.png", Elsewhere::AnotherMachine),
            (r"\\.\UNC\host\share\a.png", Elsewhere::AnotherMachine),
            (
                r"\\?\GLOBALROOT\Device\Mup\host\share\a.png",
                Elsewhere::NotADrivePath,
            ),
            (
                r"\\?\GLOBALROOT\Device\HarddiskVolume3\a.png",
                Elsewhere::NotADrivePath,
            ),
            (r"\\.\C:\a.png", Elsewhere::NotADrivePath),
            (r"\\.\PhysicalDrive0", Elsewhere::NotADrivePath),
        ] {
            assert_eq!(elsewhere(Path::new(remote)), Some(why), "{remote}");
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
