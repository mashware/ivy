//! Handing a file to the desktop: the application its type opens with, or the file manager with
//! the file shown selected (D109).
//!
//! **Opening a file is running whatever the desktop associates with its type**, and a repository
//! is somebody else's text: its names and its links. So what may be opened is an allow-list of
//! document types, judged on the path a link finally leads to, and everything else is shown in the
//! file manager instead — a script is revealed, never run.
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use self::linux as platform;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use self::macos as platform;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use self::windows as platform;

/// The last extensions whose default application only reads the file. Not html or svg: the
/// default browser runs their scripts. Not js, which Windows Script Host runs.
const LAUNCHABLE: &[&str] = &[
    "txt", "log", "md", "json", "yaml", "yml", "toml", "ini", "csv", "xml", "css", "rs", "c", "h",
    "cpp", "go", "java", "kt", "swift", "cs", "ts", "tsx", "pdf", "png", "jpg", "jpeg", "gif",
    "webp",
];

/// What a click asks of the desktop, once the path is resolved and judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Call {
    /// With its default application.
    Open(PathBuf),
    /// Selected in the file manager.
    Reveal(PathBuf),
}

/// Whether the file at `canonical` — a path with every link already followed — may be handed to
/// its default application.
pub(crate) fn may_launch(canonical: &Path) -> bool {
    let Some(name) = canonical.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !shown_as_itself(name) || (cfg!(windows) && altered_by_windows(name)) {
        return false;
    }
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && LAUNCHABLE
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(extension))
}

/// Whether every character of `text` paints as itself: no line break that would run on paste, no
/// direction override that would show exe as jpg.
pub(crate) fn shown_as_itself(text: &str) -> bool {
    text.chars().all(panel_contract::is_shown_as_itself)
}

/// A name Windows would quietly change before opening it: it drops a trailing dot or space, so
/// another file than the one judged would open.
fn altered_by_windows(name: &str) -> bool {
    name.ends_with(['.', ' '])
}

/// What a click on Open (`open`) or on Open in File Manager does with `path`.
pub(crate) fn decide(path: &Path, open: bool) -> std::io::Result<Call> {
    let canonical = canonical(path)?;
    Ok(if open && may_launch(&canonical) {
        Call::Open(canonical)
    } else {
        Call::Reveal(canonical)
    })
}

/// `path` with every link followed, in the form the desktop's own calls take.
pub(crate) fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    let resolved = std::fs::canonicalize(path)?;
    if cfg!(windows)
        && let Some(plain) = resolved.to_str().and_then(without_verbatim_prefix)
    {
        return Ok(PathBuf::from(plain));
    }
    Ok(resolved)
}

/// A Windows path without the `\\?\` prefix canonicalising adds, which the shell's calls do not
/// take: `\\?\C:\a` is `C:\a`, and `\\?\UNC\host\share` is `\\host\share`.
fn without_verbatim_prefix(path: &str) -> Option<String> {
    let rest = path.strip_prefix(r"\\?\")?;
    Some(match rest.strip_prefix(r"UNC\") {
        Some(share) => format!(r"\\{share}"),
        None => rest.to_owned(),
    })
}

/// The line a click that changed nothing leaves on standard error. The path is quoted with its
/// escapes, so a name cannot write to the terminal that launched the window.
fn could_not(what: &str, path: &Path, why: &str) -> String {
    format!("ivy: could not {what} {path:?}: {why}")
}

/// Where a click on Open goes.
pub(crate) enum Desktop {
    /// The desktop's own calls, each on a thread of its own that waits for what it started, so a
    /// slow launcher never stops the window and leaves no process unreaped. The tests' build never
    /// constructs it, on purpose.
    #[cfg_attr(test, allow(dead_code))]
    System,
    /// What the tests read instead of opening anything.
    #[cfg(test)]
    Recorded(Vec<Call>),
}

/// A test's window never holds the real desktop: a headless click on Open must not start an
/// application on the machine running the suite.
impl Default for Desktop {
    fn default() -> Self {
        #[cfg(test)]
        return Self::Recorded(Vec::new());
        #[cfg(not(test))]
        Self::System
    }
}

impl Desktop {
    /// Open `path` with its default application if its type allows it, or reveal it instead.
    pub(crate) fn open(&mut self, path: &Path) {
        self.hand(path, true);
    }

    /// Show `path` selected in the file manager.
    pub(crate) fn reveal(&mut self, path: &Path) {
        self.hand(path, false);
    }

    fn hand(&mut self, path: &Path, open: bool) {
        let what = if open { "open" } else { "show" };
        match self {
            Self::System => {
                let asked = path.to_owned();
                let spawned = std::thread::Builder::new()
                    .name("file opener".into())
                    .spawn(move || {
                        let done = decide(&asked, open)
                            .map_err(|why| why.to_string())
                            .and_then(|call| platform::run(&call));
                        if let Err(why) = done {
                            eprintln!("{}", could_not(what, &asked, &why));
                        }
                    });
                if let Err(why) = spawned {
                    eprintln!("{}", could_not(what, path, &why.to_string()));
                }
            }
            #[cfg(test)]
            Self::Recorded(calls) => match decide(path, open) {
                Ok(call) => calls.push(call),
                Err(why) => eprintln!("{}", could_not(what, path, &why.to_string())),
            },
        }
    }
}

#[cfg(test)]
mod tests;
