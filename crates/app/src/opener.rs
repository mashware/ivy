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

/// The last extensions whose default application only reads the file. Not html, svg or xml: the
/// default browser runs their scripts, and XML in the XHTML namespace is a page. Not js, which
/// Windows Script Host runs.
const LAUNCHABLE: &[&str] = &[
    "txt", "log", "md", "json", "yaml", "yml", "toml", "ini", "csv", "css", "rs", "c", "h", "cpp",
    "go", "java", "kt", "swift", "cs", "ts", "tsx", "pdf", "png", "jpg", "jpeg", "gif", "webp",
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

/// Whether Open hands the file at `canonical` to its application: a regular file whose name may
/// launch. A folder, a pipe or a device named like a document is shown instead.
pub(crate) fn launches(canonical: &Path) -> bool {
    may_launch(canonical) && std::fs::metadata(canonical).is_ok_and(|found| found.is_file())
}

/// What a click on Open (`open`) or on Open in File Manager does with `path`.
pub(crate) fn decide(path: &Path, open: bool) -> std::io::Result<Call> {
    let canonical = canonical(path)?;
    Ok(if open && launches(&canonical) {
        Call::Open(canonical)
    } else {
        Call::Reveal(canonical)
    })
}

/// Whether `path` is still a regular file and not a link, read right before a launcher gets it.
/// What `decide` judged can be swapped for a link between the judgement and the launch, and a
/// launcher follows links; this narrows that window to the instant before the spawn, it does not
/// close it.
fn still_a_file(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(found) if found.file_type().is_file() => Ok(()),
        Ok(_) => Err("it is no longer a plain file".to_owned()),
        Err(why) => Err(why.to_string()),
    }
}

/// One click, whole: decide what it asks, check the file again for an open, and hand the call to
/// `run`; the line to print when it changed nothing. The real desktop runs this on the click's
/// thread with the platform's calls, and the tests run it with a recording `run`.
fn act(
    path: &Path,
    open: bool,
    run: impl FnOnce(&Call) -> Result<(), String>,
) -> Result<(), String> {
    let call = match decide(path, open) {
        Ok(call) => call,
        Err(why) => {
            let what = if open { "open" } else { "show" };
            return Err(could_not(what, path, &why.to_string()));
        }
    };
    // An Open that became a reveal failed at showing, not at opening.
    let (what, checked) = match &call {
        Call::Open(file) => ("open", still_a_file(file)),
        Call::Reveal(_) => ("show", Ok(())),
    };
    checked
        .and_then(|()| run(&call))
        .map_err(|why| could_not(what, path, &why))
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
        match self {
            Self::System => {
                let asked = path.to_owned();
                let spawned = std::thread::Builder::new()
                    .name("file opener".into())
                    .spawn(move || {
                        if let Err(line) = act(&asked, open, platform::run) {
                            eprintln!("{line}");
                        }
                    });
                if let Err(why) = spawned {
                    let what = if open { "open" } else { "show" };
                    eprintln!("{}", could_not(what, path, &why.to_string()));
                }
            }
            #[cfg(test)]
            Self::Recorded(calls) => {
                let recorded = act(path, open, |call| {
                    calls.push(call.clone());
                    Ok(())
                });
                if let Err(line) = recorded {
                    eprintln!("{line}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
