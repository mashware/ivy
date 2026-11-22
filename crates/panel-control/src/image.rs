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

/// How many links a path may pass through before it is refused: the bound Linux puts on the
/// symbolic links of one path.
#[cfg(any(windows, test))]
const MAX_LINKS: usize = 40;

/// Refuses `path` when a symbolic link or a junction on its way — an ancestor or the file itself —
/// leads off this machine. On Windows a link under `C:\` can point at `\\host\share`, and opening
/// through it authenticates to that host exactly as the prefix `is_local` refuses would (D120).
/// Each link's target is read without following it, so the walk itself reaches no host.
///
/// Elsewhere it answers `Ok`: a link to a mount is to a mount the user made, as a mapped drive
/// letter is on Windows.
#[cfg(windows)]
pub fn links_stay_on_this_machine(path: &Path) -> Result<(), String> {
    walk_links(path, &mut look_on_disk, &is_local)
}

/// See the Windows arm: no other platform names a host in a path.
#[cfg(not(windows))]
pub fn links_stay_on_this_machine(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// What one step of the walk found at a path, without following it if it is a link.
#[cfg(any(windows, test))]
enum Step {
    /// Not a link.
    Plain,
    /// A link, and where it points.
    Link(std::path::PathBuf),
    /// A link whose target could not be read.
    Unreadable(std::io::Error),
    /// Nothing could be asked of it: the open that follows says why, and nothing past it is
    /// looked at, since looking would go through it.
    Unknown,
}

/// `std` calls both a symbolic link and a junction (a mount point) a symlink on Windows, and reads
/// both with `read_link`, which takes the target from the reparse data (D108).
#[cfg(windows)]
fn look_on_disk(path: &Path) -> Step {
    match std::fs::symlink_metadata(path) {
        Ok(found) if found.file_type().is_symlink() => match std::fs::read_link(path) {
            Ok(target) => Step::Link(target),
            Err(why) => Step::Unreadable(why),
        },
        Ok(_) => Step::Plain,
        Err(_) => Step::Unknown,
    }
}

/// The walk over `path`, with how a step is looked at and what counts as local handed in, so it is
/// tested where no path has a prefix.
#[cfg(any(windows, test))]
fn walk_links(
    path: &Path,
    look: &mut impl FnMut(&Path) -> Step,
    local: &impl Fn(&Path) -> bool,
) -> Result<(), String> {
    let mut links = 0;
    walk_from(path, path, look, local, &mut links).map(|_| ())
}

/// Walks `through` from its root, and each link's target before going past that link: whatever
/// is looked at past a link is reached through it. Answers whether the walk got to the end.
#[cfg(any(windows, test))]
fn walk_from(
    asked: &Path,
    through: &Path,
    look: &mut impl FnMut(&Path) -> Step,
    local: &impl Fn(&Path) -> bool,
    links: &mut usize,
) -> Result<bool, String> {
    let mut walked = std::path::PathBuf::new();
    for component in through.components() {
        walked.push(component);
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            continue;
        }
        let target = match look(&walked) {
            Step::Plain => continue,
            Step::Unknown => return Ok(false),
            Step::Unreadable(why) => {
                return Err(format!(
                    "{} passes through a link whose target cannot be read: {why}",
                    asked.display()
                ));
            }
            Step::Link(target) => target,
        };
        *links += 1;
        if *links > MAX_LINKS {
            return Err(format!(
                "{} passes through more than {MAX_LINKS} links",
                asked.display()
            ));
        }
        // A relative target is relative to the folder the link is in.
        let target = match walked.parent() {
            Some(folder) if target.is_relative() => folder.join(target),
            _ => target,
        };
        if !local(&target) {
            return Err(format!(
                "{} leads to {}, which is not on this machine",
                asked.display(),
                target.display()
            ));
        }
        if !walk_from(asked, &target, look, local, links)? {
            return Ok(false);
        }
    }
    Ok(true)
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

    /// A disk made of a table: the links in it, the paths nothing can be asked of, and every
    /// other path a plain entry. It writes down each path it is asked about, in order.
    struct Disk {
        links: Vec<(&'static str, &'static str)>,
        unknown: Vec<&'static str>,
        unreadable: Vec<&'static str>,
        looked: Vec<std::path::PathBuf>,
    }

    impl Disk {
        fn of(links: &[(&'static str, &'static str)]) -> Self {
            Disk {
                links: links.to_vec(),
                unknown: Vec::new(),
                unreadable: Vec::new(),
                looked: Vec::new(),
            }
        }

        /// Walks `path` over this disk, where only what is under `/share` is another machine:
        /// the walk's own logic, with the prefix test that only Windows can parse handed in.
        fn walk(&mut self, path: &str) -> Result<(), String> {
            let mut look = |at: &Path| {
                self.looked.push(at.to_path_buf());
                if self.unknown.iter().any(|p| Path::new(p) == at) {
                    return Step::Unknown;
                }
                if self.unreadable.iter().any(|p| Path::new(p) == at) {
                    return Step::Unreadable(std::io::Error::other("no reparse data"));
                }
                match self.links.iter().find(|(link, _)| Path::new(link) == at) {
                    Some((_, target)) => Step::Link(target.into()),
                    None => Step::Plain,
                }
            };
            walk_links(Path::new(path), &mut look, &|p: &Path| {
                !p.starts_with("/share")
            })
        }

        fn looked_at(&self, path: &str) -> bool {
            self.looked.iter().any(|p| p == Path::new(path))
        }
    }

    #[test]
    fn a_path_with_no_link_is_walked_to_its_end() {
        let mut disk = Disk::of(&[]);
        assert_eq!(disk.walk("/home/me/a.png"), Ok(()));
        assert_eq!(
            disk.looked,
            ["/home", "/home/me", "/home/me/a.png"].map(std::path::PathBuf::from)
        );
    }

    /// **The file, or any folder on its way, that links to another machine refuses the path**, and
    /// nothing past that link is looked at: looking would go through it.
    #[test]
    fn a_link_to_another_machine_refuses_the_path_wherever_it_is() {
        let mut disk = Disk::of(&[("/home/me/a.png", "/share/a.png")]);
        assert_eq!(
            disk.walk("/home/me/a.png"),
            Err("/home/me/a.png leads to /share/a.png, which is not on this machine".to_owned())
        );
        let mut disk = Disk::of(&[("/home/me", "/share")]);
        assert_eq!(
            disk.walk("/home/me/a.png"),
            Err("/home/me/a.png leads to /share, which is not on this machine".to_owned())
        );
        assert!(!disk.looked_at("/home/me/a.png"));
    }

    /// A relative target is read from the link's own folder: `b.png` beside the link, which is
    /// itself a link to another machine. Read on its own, `b.png` names nothing in the table.
    #[test]
    fn a_relative_target_is_read_from_the_links_folder() {
        let mut disk = Disk::of(&[
            ("/home/me/a.png", "b.png"),
            ("/home/me/b.png", "/share/b.png"),
        ]);
        assert_eq!(
            disk.walk("/home/me/a.png"),
            Err("/home/me/a.png leads to /share/b.png, which is not on this machine".to_owned())
        );
    }

    /// **A local target is walked too, before anything past the link**: a link to `/data` where
    /// `/data` itself links to another machine is refused, and the file under it is never asked
    /// for. A local chain is followed to its end.
    #[test]
    fn a_local_target_is_walked_before_the_path_goes_past_it() {
        let mut disk = Disk::of(&[("/home/me", "/data"), ("/data", "/share")]);
        assert_eq!(
            disk.walk("/home/me/a.png"),
            Err("/home/me/a.png leads to /share, which is not on this machine".to_owned())
        );
        assert!(!disk.looked_at("/home/me/a.png"));
        let mut disk = Disk::of(&[("/home/me", "/data/me")]);
        assert_eq!(disk.walk("/home/me/a.png"), Ok(()));
        assert!(disk.looked_at("/data/me"));
        assert!(disk.looked_at("/home/me/a.png"));
    }

    #[test]
    fn a_loop_of_links_is_refused_once_it_passes_the_bound() {
        let mut disk = Disk::of(&[("/home/a", "/home/b"), ("/home/b", "/home/a")]);
        assert_eq!(
            disk.walk("/home/a/x.png"),
            Err(format!(
                "/home/a/x.png passes through more than {MAX_LINKS} links"
            ))
        );
    }

    /// A link whose target cannot be read refuses the path: where it leads is unknown. A path
    /// nothing can be asked of ends the walk without a refusal, because the open that follows
    /// fails and says why — and nothing past it is looked at.
    #[test]
    fn an_unreadable_link_refuses_and_an_unknown_path_ends_the_walk() {
        let mut disk = Disk::of(&[]);
        disk.unreadable.push("/home/me");
        assert_eq!(
            disk.walk("/home/me/a.png"),
            Err(
                "/home/me/a.png passes through a link whose target cannot be read: \
                 no reparse data"
                    .to_owned()
            )
        );
        let mut disk = Disk::of(&[("/home/gone/a.png", "/share/a.png")]);
        disk.unknown.push("/home/gone");
        assert_eq!(disk.walk("/home/gone/a.png"), Ok(()));
        assert!(!disk.looked_at("/home/gone/a.png"));
    }

    /// **On a real Windows disk**, a symbolic link to a share — as a file, and as a folder on the
    /// way — is refused by reading it, never by following it. Creating a symbolic link needs a
    /// privilege (or Developer Mode) that a test runner may lack; then the test says so and checks
    /// nothing. A junction cannot point at a share, so none is made here.
    #[cfg(windows)]
    #[test]
    fn a_symbolic_link_to_a_share_is_refused_on_windows() {
        const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;
        let folder = std::env::temp_dir().join(format!("ivy-image-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("a.png");
        let through = folder.join("share");
        let made = std::os::windows::fs::symlink_file(r"\\host\share\a.png", &file)
            .and_then(|()| std::os::windows::fs::symlink_dir(r"\\host\share", &through));
        if let Err(why) = &made
            && why.raw_os_error() == Some(ERROR_PRIVILEGE_NOT_HELD)
        {
            let _ = std::fs::remove_dir_all(&folder);
            eprintln!("skipped: this account may not create symbolic links ({why})");
            return;
        }
        made.unwrap();
        for asked in [file.clone(), through.join("a.png")] {
            let said = links_stay_on_this_machine(&asked).unwrap_err();
            let start = format!("{} leads to ", asked.display());
            assert!(said.starts_with(&start), "{said}");
            assert!(said.ends_with(", which is not on this machine"), "{said}");
            let target = &said[start.len()..said.len() - ", which is not on this machine".len()];
            assert!(!is_local(Path::new(target)), "{target}");
        }
        let _ = std::fs::remove_dir_all(&folder);
    }
}
