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

/// Whether `path`'s text names a file on this machine. On Windows an absolute path can name
/// another host's share or a device, and reading it would authenticate to that host on an agent's
/// word; paths on other platforms have no prefix. The text is all it judges: a link on the way is
/// [`no_link_on_the_way`]'s. **An allowlist, not a list of what leaves** (D120): a refusal list
/// missed `\\?\GLOBALROOT\Device\Mup\…`, which reaches the SMB redirector, and `\\?\unc\…`, which
/// std parses case-sensitively as a plain verbatim prefix while Windows does not.
pub fn is_local(path: &Path) -> bool {
    elsewhere(path).is_none()
}

/// Why `path` is refused by its text alone, or `None` when its text names this machine (see
/// `is_local`).
pub fn elsewhere(path: &Path) -> Option<Elsewhere> {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => prefix_elsewhere(prefix.kind()),
        _ => None,
    }
}

/// A drive letter, in either spelling, or a volume named by its GUID — which is how `canonicalize`
/// names a volume with no letter — is local. A mapped drive letter is the user's own mapping, not a
/// host the agent chose, so it passes (D120). Everything else under `\\?\` and `\\.\` is the NT
/// object namespace: its `UNC` spellings are answered as a share, and the rest as not a drive
/// path, because some of it reaches a share (`\\?\GLOBALROOT\Device\Mup\…`) and some does not.
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

/// Refuses an agent's `path` when an entry on its way — a folder from the root down, or the file
/// itself — is a link (D120). On Windows a symbolic link under `C:\` can lead to `\\host\share`,
/// and opening through it authenticates to that host exactly as the prefixes `is_local` refuses
/// would. Where a link leads is never read: reading a target and checking it is what review found
/// three ways around (D120). Entries are asked from the root down and the walk stops at the first
/// link, so nothing is asked through one. A path with a `..` in it is refused before anything is
/// asked: Windows collapses `..` by the text, without asking the disk, so in
/// `C:\repo\gone\..\shots\a.png` the open reaches `shots` — a link, maybe — through an entry the
/// walk stopped at because it is not there. Without a `..`, an entry that is not there stops the
/// walk without a refusal, since the open that follows has to pass through that same entry and
/// fails; any other error refuses, because the walk can no longer tell what is there.
///
/// `is_link` answers for one entry without following it: [`refused_link`] in the bridge and the
/// window, and a stand-in in the tests, so the walk and both callers are pinned where no entry is
/// one.
pub fn no_link_on_the_way(
    path: &Path,
    mut is_link: impl FnMut(&Path) -> std::io::Result<bool>,
) -> Result<(), String> {
    if path.components().any(|c| c == Component::ParentDir) {
        return Err(format!(
            "{} has a «..» in it; give the path without one",
            path.display()
        ));
    }
    let mut walked = std::path::PathBuf::new();
    for component in path.components() {
        walked.push(component);
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        match is_link(&walked) {
            Ok(false) => {}
            Ok(true) => {
                return Err(format!(
                    "{} passes through a link, and an agent's image is not opened through one",
                    path.display()
                ));
            }
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(why) => {
                return Err(format!(
                    "cannot tell whether {} passes through a link: {why}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

/// Whether the entry `path` names is a link, asked of the entry itself: `symlink_metadata` opens
/// the reparse point rather than what it names. std's `is_symlink` is the name-surrogate rule — a
/// symbolic link, a junction, or any other reparse tag that names another path — so those are what
/// is refused. A reparse point that is no name (a cloud placeholder, deduplication, compression)
/// holds the file's own data behind its filter and passes, as D108 lets it pass: refusing it would
/// refuse every image under a OneDrive-synced folder (D120).
#[cfg(windows)]
pub fn refused_link(path: &Path) -> std::io::Result<bool> {
    Ok(std::fs::symlink_metadata(path)?.file_type().is_symlink())
}

/// No link is refused here, and nothing is asked of the disk: a link leads to what the user made
/// reachable, as a mapped drive letter does on Windows. That is an accepted cost, not a claim that
/// no path can name a host here — macOS's `/net` and an autofs `-hosts` map can (D120).
#[cfg(not(windows))]
pub fn refused_link(_path: &Path) -> std::io::Result<bool> {
    Ok(false)
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

    /// What one entry of the table answers when it is asked.
    type Answer = fn() -> std::io::Result<bool>;

    /// Walks `path` over a disk made of a table — what each entry answers, every other entry not
    /// a link — and writes down each entry it was asked about, in order.
    fn walk(
        path: &str,
        answers: &[(&str, Answer)],
    ) -> (Result<(), String>, Vec<std::path::PathBuf>) {
        let mut asked = Vec::new();
        let said = no_link_on_the_way(Path::new(path), |at: &Path| {
            asked.push(at.to_path_buf());
            answers
                .iter()
                .find(|(entry, _)| Path::new(entry) == at)
                .map_or(Ok(false), |(_, answer)| answer())
        });
        (said, asked)
    }

    fn link() -> std::io::Result<bool> {
        Ok(true)
    }

    fn gone() -> std::io::Result<bool> {
        Err(std::io::ErrorKind::NotFound.into())
    }

    fn denied() -> std::io::Result<bool> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "access is denied",
        ))
    }

    fn paths(entries: &[&str]) -> Vec<std::path::PathBuf> {
        entries.iter().map(std::path::PathBuf::from).collect()
    }

    /// Every entry is asked, from the root down, and the file last.
    #[test]
    fn a_path_with_no_link_is_asked_entry_by_entry_from_the_root() {
        let (said, asked) = walk("/home/me/a.png", &[]);
        assert_eq!(said, Ok(()));
        assert_eq!(asked, paths(&["/home", "/home/me", "/home/me/a.png"]));
    }

    /// **A link anywhere refuses the path, the file included**, and nothing past a link is asked:
    /// asking would go through it.
    #[test]
    fn a_link_anywhere_on_the_way_refuses_the_path() {
        let refused = "/home/me/a.png passes through a link, and an agent's image is not opened \
                       through one";
        for (at, asked_up_to) in [
            ("/home", &["/home"][..]),
            ("/home/me", &["/home", "/home/me"][..]),
            (
                "/home/me/a.png",
                &["/home", "/home/me", "/home/me/a.png"][..],
            ),
        ] {
            let (said, asked) = walk("/home/me/a.png", &[(at, link)]);
            assert_eq!(said, Err(refused.to_owned()), "{at}");
            assert_eq!(asked, paths(asked_up_to), "{at}");
        }
    }

    /// **An entry the disk will not answer for refuses the path** with what it said: the walk
    /// cannot tell whether a link is there, and a folder it may not look into may still be passed
    /// through by the open.
    #[test]
    fn an_entry_that_cannot_be_asked_refuses_the_path() {
        let (said, asked) = walk("/home/me/a.png", &[("/home/me", denied)]);
        assert_eq!(
            said,
            Err(
                "cannot tell whether /home/me/a.png passes through a link: access is denied"
                    .to_owned()
            )
        );
        assert_eq!(asked, paths(&["/home", "/home/me"]));
    }

    /// **A `..` anywhere refuses the path before any entry is asked**: Windows collapses it by the
    /// text, so a missing entry before it would end the walk while the open reaches the link after
    /// it. Here `/home/gone` is not there and `/home/me` is a link.
    #[test]
    fn a_path_with_a_parent_step_is_refused_before_anything_is_asked() {
        for path in ["/home/gone/../me/a.png", "/home/me/a.png/..", "/../home/me/a.png"] {
            let (said, asked) = walk(path, &[("/home/gone", gone), ("/home/me", link)]);
            assert_eq!(
                said,
                Err(format!("{path} has a «..» in it; give the path without one")),
                "{path}"
            );
            assert_eq!(asked, paths(&[]), "{path}");
        }
    }

    /// Without a `..`, an entry that is not there leaves the path to the open, which has to pass
    /// through it and fails, and nothing under it is asked.
    #[test]
    fn an_entry_that_is_not_there_is_left_to_the_open() {
        let (said, asked) = walk("/home/gone/a.png", &[("/home/gone", gone)]);
        assert_eq!(said, Ok(()));
        assert_eq!(asked, paths(&["/home", "/home/gone"]));
        let (said, _) = walk("/home/me/a.png", &[("/home/me/a.png", gone)]);
        assert_eq!(said, Ok(()));
    }

    /// **On a real Windows disk** a junction — which needs no privilege to make — and a symbolic
    /// link are each refused as the file's folder, and the plain folder beside them is not. A
    /// symbolic link needs a privilege (or Developer Mode) a runner may lack: then that half says
    /// so and checks nothing, and the junction half still runs.
    #[cfg(windows)]
    #[test]
    fn a_junction_or_a_symbolic_link_is_refused_on_windows() {
        const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;
        let folder = std::env::temp_dir().join(format!("ivy-image-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let plain = folder.join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        std::fs::write(plain.join("a.png"), b"x").unwrap();
        assert_eq!(
            no_link_on_the_way(&plain.join("a.png"), refused_link),
            Ok(())
        );
        let junction = folder.join("junction");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&plain)
            .output()
            .unwrap();
        assert!(made.status.success(), "{made:?}");
        let mut links = vec![junction];
        let symbolic = folder.join("symbolic");
        match std::os::windows::fs::symlink_dir(&plain, &symbolic) {
            Ok(()) => links.push(symbolic),
            Err(why) if why.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD) => {
                eprintln!("symbolic link skipped: this account may not create one ({why})");
            }
            Err(why) => panic!("{why}"),
        }
        for link in links {
            let file = link.join("a.png");
            assert_eq!(
                no_link_on_the_way(&file, refused_link),
                Err(format!(
                    "{} passes through a link, and an agent's image is not opened through one",
                    file.display()
                ))
            );
        }
        let _ = std::fs::remove_dir_all(&folder);
    }
}
