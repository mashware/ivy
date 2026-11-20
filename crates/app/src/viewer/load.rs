use eframe::egui;
use std::cell::Cell;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;
#[cfg(test)]
use std::time::Instant;

const MARKDOWN_LIMIT: usize = 1024 * 1024;
const CODE_LIMIT: usize = 1024 * 1024;
/// How far into a file a NUL byte is looked for before it is called binary.
const BINARY_SNIFF: usize = 8 * 1024;
const DIFF_LIMIT: usize = 4 * 1024 * 1024;
const STDERR_LIMIT: usize = 64 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Markdown,
    /// Any other file of the Document tab, shown as code.
    Code,
    Unstaged,
    Staged,
    Local,
    AgainstBase,
}

impl Kind {
    /// The Document tab's kinds, as against the Changes tab's comparisons.
    pub(super) fn is_document(self) -> bool {
        matches!(self, Self::Markdown | Self::Code)
    }

    /// How the Document tab shows `path`: formatted when it is Markdown, as code otherwise.
    pub(super) fn of_document(path: &Path) -> Self {
        let markdown = path.extension().is_some_and(|ext| {
            ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown")
        });
        if markdown { Self::Markdown } else { Self::Code }
    }
}

pub(super) struct Loaded {
    pub(super) text: String,
    pub(super) markdown: Option<super::markdown::MarkdownDocument>,
    pub(super) code: Option<super::code::CodeDocument>,
    pub(super) diff: Option<super::diff::Document>,
}

pub(super) struct Job {
    receiver: Receiver<Result<Loaded, String>>,
    finished: Cell<bool>,
}

impl Job {
    #[cfg(test)]
    pub(super) fn start(path: PathBuf, kind: Kind, ctx: egui::Context) -> Result<Self, String> {
        Self::start_with_base(path, kind, "main".into(), ctx)
    }

    pub(super) fn start_with_base(
        path: PathBuf,
        kind: Kind,
        base: String,
        ctx: egui::Context,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("viewer-loader".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(|| match kind {
                    Kind::Markdown => read_markdown(&path).and_then(|text| {
                        let markdown = Some(super::markdown::MarkdownDocument::parse(&text)?);
                        Ok(Loaded {
                            text,
                            markdown,
                            code: None,
                            diff: None,
                        })
                    }),
                    Kind::Code => read_text(&path).map(|text| Loaded {
                        code: Some(super::code::CodeDocument::parse(&path, &text)),
                        text,
                        markdown: None,
                        diff: None,
                    }),
                    Kind::Unstaged | Kind::Staged | Kind::Local | Kind::AgainstBase => {
                        load_diff(&path, kind, &base).map(|(text, diff)| Loaded {
                            text,
                            markdown: None,
                            code: None,
                            diff: Some(diff),
                        })
                    }
                })
                .unwrap_or_else(|_| Err("Viewer loader failed unexpectedly".into()));
                let _ = sender.send(result);
                ctx.request_repaint();
            })
            .map_err(|e| format!("Cannot start viewer loader: {e}"))?;
        Ok(Self {
            receiver,
            finished: Cell::new(false),
        })
    }

    pub(super) fn poll(&self) -> Option<Result<Loaded, String>> {
        if self.finished.get() {
            return None;
        }
        let result = match self.receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("Viewer loader stopped unexpectedly".into()),
        };
        self.finished.set(true);
        Some(result)
    }
}

fn read_markdown(path: &Path) -> Result<String, String> {
    let file = open_regular(path)?;
    let bytes =
        read_bounded(file, MARKDOWN_LIMIT).map_err(|e| format!("Cannot read Markdown: {e}"))?;
    String::from_utf8(bytes).map_err(|_| "Markdown is not UTF-8".into())
}

/// A file for the code view, or the one sentence that says why it cannot be shown as text.
fn read_text(path: &Path) -> Result<String, String> {
    let file = open_regular(path)?;
    let bytes = read_bounded(file, CODE_LIMIT).map_err(|e| match e.kind() {
        ErrorKind::InvalidData => "Larger than 1 MiB".to_owned(),
        _ => format!("Cannot read file: {e}"),
    })?;
    if bytes[..bytes.len().min(BINARY_SNIFF)].contains(&0) {
        return Err("Binary file".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "Not UTF-8 text".to_owned())?;
    Ok(text
        .strip_prefix('\u{feff}')
        .map(str::to_owned)
        .unwrap_or(text))
}

fn open_regular(path: &Path) -> Result<std::fs::File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|e| format!("Cannot open file: {e}"))?;
    let metadata = file
        .metadata()
        .map_err(|e| format!("Cannot inspect file: {e}"))?;
    if metadata.is_dir() {
        // Files lists a link to a folder as a link and does not follow it (#88).
        return Err("This is a folder, not a file".into());
    }
    if !metadata.is_file() {
        return Err("The selected path is not a regular file".into());
    }
    Ok(file)
}

fn read_bounded(reader: impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "content exceeds the size limit",
        ));
    }
    Ok(bytes)
}

fn git_command(path: &Path) -> Command {
    let mut command = Command::new("git");
    // Agent sessions can inherit Git routing that would override the selected worktree.
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    command
        .args([
            "--no-pager",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.quotePath=true",
            "-C",
        ])
        .arg(path)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

#[cfg(test)]
fn git_diff(path: &Path, kind: Kind) -> Result<String, String> {
    git_diff_base(path, kind, "main")
}

fn git_diff_base(path: &Path, kind: Kind, base: &str) -> Result<String, String> {
    let mut inspect = git_command(path);
    inspect.args(["rev-parse", "--is-inside-work-tree"]);
    if run_git(inspect, GIT_TIMEOUT)?.trim() != "true" {
        return Err("This repository has no working tree".into());
    }
    let revision = match kind {
        Kind::Local => {
            let mut verify = git_command(path);
            verify.args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]);
            match run_git(verify, GIT_TIMEOUT) {
                Ok(revision) => Some(revision.trim().to_owned()),
                Err(_) => {
                    let mut empty = git_command(path);
                    empty.args(["hash-object", "-t", "tree", "--stdin"]);
                    Some(run_git(empty, GIT_TIMEOUT)?.trim().to_owned())
                }
            }
        }
        Kind::AgainstBase => {
            let mut verify = git_command(path);
            verify
                .args(["rev-parse", "--verify", "--end-of-options"])
                .arg(format!("{base}^{{commit}}"));
            Some(run_git(verify, GIT_TIMEOUT)?.trim().to_owned())
        }
        _ => None,
    };
    let mut command = git_command(path);
    command.args(["diff", "--no-ext-diff", "--no-textconv", "--no-color"]);
    if kind == Kind::Staged {
        command.arg("--cached");
    }
    if let Some(revision) = revision {
        command.arg(revision);
    }
    command.arg("--");
    run_git(command, GIT_TIMEOUT)
}

fn load_diff(
    path: &Path,
    kind: Kind,
    base: &str,
) -> Result<(String, super::diff::Document), String> {
    let text = git_diff_base(path, kind, base)?;
    let mut doc = super::diff::Document::parse(&text)?;
    if kind == Kind::Local {
        add_untracked(path, &mut doc, text.len())?;
    }
    Ok((text, doc))
}

fn add_untracked(
    path: &Path,
    doc: &mut super::diff::Document,
    mut total: usize,
) -> Result<(), String> {
    let mut root_command = git_command(path);
    root_command.args(["rev-parse", "--show-toplevel"]);
    let root_text = run_git(root_command, GIT_TIMEOUT)?;
    let root = PathBuf::from(root_text.trim_end_matches('\n'))
        .canonicalize()
        .map_err(|e| format!("Cannot inspect repository: {e}"))?;
    let mut list = git_command(&root);
    list.args(["ls-files", "--others", "--exclude-standard", "-z", "--"]);
    let names = run_git(list, GIT_TIMEOUT)?;
    for name in names.split('\0').filter(|s| !s.is_empty()) {
        let relative = PathBuf::from(name);
        let mut file = super::diff::make_file(relative.clone());
        let mut current = root.clone();
        let mut unsafe_path = false;
        for part in relative.components() {
            if !matches!(part, std::path::Component::Normal(_)) {
                unsafe_path = true;
                break;
            }
            current.push(part);
            if std::fs::symlink_metadata(&current)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(true)
            {
                unsafe_path = true;
                break;
            }
        }
        if unsafe_path {
            file.notice = Some("Symlink or unavailable file was not read".into());
            doc.files.push(file);
            continue;
        }
        let bytes = match open_untracked(&root, &relative).and_then(|f| {
            if !f.metadata()?.is_file() {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "not a regular file",
                ));
            }
            read_bounded(f, MARKDOWN_LIMIT)
        }) {
            Ok(bytes) => bytes,
            Err(e) => {
                file.notice = Some(format!("Cannot read file: {e}"));
                doc.files.push(file);
                continue;
            }
        };
        total = total
            .checked_add(bytes.len())
            .ok_or("Git changes exceed the size limit")?;
        if total > DIFF_LIMIT {
            return Err("Git changes exceed the size limit".into());
        }
        if bytes.contains(&0) {
            file.notice = Some("Binary file".into());
        } else if let Ok(contents) = String::from_utf8(bytes) {
            let lines = contents
                .lines()
                .enumerate()
                .map(|(i, text)| super::diff::DiffLine {
                    old: None,
                    new: u32::try_from(i + 1).ok(),
                    text: text.into(),
                    kind: super::diff::Change::Added,
                })
                .collect::<Vec<_>>();
            let pairs = (0..lines.len())
                .map(|i| super::diff::Pair {
                    old: None,
                    new: Some(i),
                })
                .collect();
            file.hunks.push(super::diff::Hunk {
                header: "New file".into(),
                lines,
                pairs,
            });
        } else {
            file.notice = Some("Binary or non-UTF-8 file".into());
        }
        doc.files.push(file);
    }
    Ok(())
}

#[cfg(unix)]
fn open_untracked(root: &Path, path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let mut dir = std::fs::File::open(root)?;
    let mut parts = path.components().peekable();
    while let Some(part) = parts.next() {
        let std::path::Component::Normal(name) = part else {
            return Err(std::io::Error::new(ErrorKind::InvalidInput, "invalid path"));
        };
        let name = std::ffi::CString::new(name.as_bytes())
            .map_err(|_| std::io::Error::new(ErrorKind::InvalidInput, "invalid path"))?;
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | libc::O_CLOEXEC
            | if parts.peek().is_some() {
                libc::O_DIRECTORY
            } else {
                0
            };
        // Each parent is opened relative to its already verified directory descriptor.
        let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        dir = unsafe { std::fs::File::from_raw_fd(fd) };
    }
    Ok(dir)
}
/// The walk in `add_untracked` looked at the path before this open, not at it, so the last
/// component is checked again here (D108). Intermediate folders are not held: the race D108 leaves
/// open.
#[cfg(windows)]
fn open_untracked(root: &Path, path: &Path) -> std::io::Result<std::fs::File> {
    crate::windows_fs::open_no_follow(&root.join(path))
}
#[cfg(not(any(unix, windows)))]
fn open_untracked(root: &Path, path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(root.join(path))
}

// D57: all waiting and pipe draining stays off the window thread; readers have byte ceilings.
fn run_git(command: Command, timeout: Duration) -> Result<String, String> {
    let output = crate::git_process::run(command, timeout, DIFF_LIMIT, STDERR_LIMIT)?;
    if !output.status.success() {
        if output
            .stderr
            .starts_with(b"fatal: not a git repository (or any of the parent directories): .git")
            || output
                .stderr
                .starts_with(b"fatal: not a git repository (or any parent up to mount point ")
        {
            return Err("This folder is not a Git repository".into());
        }
        return Err(format!(
            "Git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| "Git output is not UTF-8".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ivy-viewer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn git(path: &Path, args: &[&str]) {
        let mut command = Command::new("git");
        for (key, _) in std::env::vars_os() {
            if key
                .to_string_lossy()
                .to_ascii_uppercase()
                .starts_with("GIT_")
            {
                command.env_remove(key);
            }
        }
        let status = command
            .arg("-C")
            .arg(path)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn index_path(path: &Path) -> PathBuf {
        let mut command = git_command(path);
        command.args(["rev-parse", "--git-path", "index"]);
        let resolved = PathBuf::from(run_git(command, GIT_TIMEOUT).unwrap().trim());
        if resolved.is_absolute() {
            resolved
        } else {
            path.join(resolved)
        }
    }
    #[test]
    fn non_repository_message_is_clear_without_hiding_other_git_errors() {
        let temp = Temp::new();
        for kind in [Kind::Local, Kind::Staged, Kind::Unstaged, Kind::AgainstBase] {
            let error = load_diff(&temp.0, kind, "main").err().unwrap();
            assert_eq!(error, "This folder is not a Git repository");
        }
        let error = load_diff(&temp.0.join("missing"), Kind::Local, "main")
            .err()
            .unwrap();
        assert!(error.contains("Git failed:"), "{error}");
        assert!(!error.contains("This folder is not a Git repository"));
        git(&temp.0, &["init", "--bare", "-q"]);
        assert_eq!(
            load_diff(&temp.0, Kind::Local, "main").err().unwrap(),
            "This repository has no working tree"
        );
    }

    #[test]
    fn inherited_git_context_cannot_redirect_worktree_changes() {
        const CHILD: &str = "IVY_VIEWER_GIT_CONTEXT_CHILD";
        if let Some(path) = std::env::var_os(CHILD) {
            let (text, doc) = load_diff(Path::new(&path), Kind::Local, "main").unwrap();
            assert!(text.contains("+WORKTREE_ONLY_CHANGE"), "{text}");
            assert!(!text.contains("MAIN_ONLY_CHANGE"), "{text}");
            assert!(doc.files.iter().any(|f| f.path == Path::new("linked-new")));
            assert!(!doc.files.iter().any(|f| f.path == Path::new("main-new")));
            return;
        }
        let temp = Temp::new();
        git(&temp.0, &["init", "-q"]);
        git(&temp.0, &["config", "user.email", "test@example.invalid"]);
        git(&temp.0, &["config", "user.name", "Test"]);
        fs::write(temp.0.join("tracked"), "base\n").unwrap();
        git(&temp.0, &["add", "tracked"]);
        git(
            &temp.0,
            &["-c", "commit.gpgsign=false", "commit", "-qm", "base"],
        );
        // Git names linked-worktree admin directories from the worktree path's basename. Give
        // another worktree that basename first so the target's admin directory is not `linked`.
        let first_linked = temp.0.join("first-parent/linked");
        let linked = temp.0.join("target-parent/linked");
        fs::create_dir_all(first_linked.parent().unwrap()).unwrap();
        fs::create_dir_all(linked.parent().unwrap()).unwrap();
        git(
            &temp.0,
            &[
                "worktree",
                "add",
                "-qb",
                "linked-first",
                first_linked.to_str().unwrap(),
            ],
        );
        git(
            &temp.0,
            &["worktree", "add", "-qb", "linked", linked.to_str().unwrap()],
        );
        fs::write(temp.0.join("tracked"), "MAIN_ONLY_CHANGE\n").unwrap();
        fs::write(linked.join("tracked"), "WORKTREE_ONLY_CHANGE\n").unwrap();
        fs::write(temp.0.join("main-new"), "main untracked\n").unwrap();
        fs::write(linked.join("linked-new"), "linked untracked\n").unwrap();
        fs::write(linked.join("target-index-only"), "target index\n").unwrap();
        git(&linked, &["add", "target-index-only"]);
        let main_index_path = index_path(&temp.0);
        let linked_index_path = index_path(&linked);
        assert_ne!(
            linked_index_path,
            temp.0.join(".git/worktrees/linked/index"),
            "the fixture must exercise a non-default linked-worktree admin name"
        );
        let main_index = fs::read(&main_index_path).unwrap();
        let linked_index = fs::read(&linked_index_path).unwrap();
        // A child process isolates inherited environment without mutating the parallel test runner.
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "viewer::load::tests::inherited_git_context_cannot_redirect_worktree_changes",
                "--nocapture",
            ])
            .env(CHILD, &linked)
            .env("GIT_DIR", temp.0.join(".git"))
            .env("GIT_WORK_TREE", &temp.0)
            .env("GIT_INDEX_FILE", temp.0.join(".git/index"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(main_index, fs::read(&main_index_path).unwrap());
        assert_eq!(linked_index, fs::read(&linked_index_path).unwrap());
    }

    #[test]
    fn staged_and_unstaged_leave_repository_unchanged() {
        let temp = Temp::new();
        git(&temp.0, &["init", "-q"]);
        git(&temp.0, &["config", "user.email", "test@example.invalid"]);
        git(&temp.0, &["config", "user.name", "Test"]);
        let file = temp.0.join("file");
        fs::write(&file, "base\n").unwrap();
        git(&temp.0, &["add", "file"]);
        git(&temp.0, &["commit", "-qm", "base"]);
        fs::write(&file, "staged\n").unwrap();
        git(&temp.0, &["add", "file"]);
        fs::write(&file, "unstaged\n").unwrap();
        let before = fs::read(temp.0.join(".git/index")).unwrap();
        let staged = git_diff(&temp.0, Kind::Staged).unwrap();
        let unstaged = git_diff(&temp.0, Kind::Unstaged).unwrap();
        assert!(staged.contains("+staged") && !staged.contains("+unstaged"));
        assert!(unstaged.contains("+unstaged") && !unstaged.contains("+staged"));
        assert_eq!(before, fs::read(temp.0.join(".git/index")).unwrap());
        assert_eq!(fs::read_to_string(file).unwrap(), "unstaged\n");
    }
    #[test]
    fn local_and_base_include_expected_changes_without_writes() {
        let temp = Temp::new();
        git(&temp.0, &["init", "-q"]);
        git(&temp.0, &["config", "user.email", "test@example.invalid"]);
        git(&temp.0, &["config", "user.name", "Test"]);
        let file = temp.0.join("tracked");
        fs::write(&file, "base\n").unwrap();
        git(&temp.0, &["add", "tracked"]);
        git(&temp.0, &["commit", "-qm", "base"]);
        git(&temp.0, &["branch", "review-base"]);
        fs::write(&file, "changed\n").unwrap();
        fs::write(temp.0.join("new file"), "new\n").unwrap();
        let index = fs::read(temp.0.join(".git/index")).unwrap();
        let (_, local) = load_diff(&temp.0, Kind::Local, "main").unwrap();
        assert!(local.files.iter().any(|f| f.path == Path::new("tracked")));
        let added = local
            .files
            .iter()
            .find(|f| f.path == Path::new("new file"))
            .unwrap();
        assert_eq!(added.hunks[0].lines[0].text, "new");
        let (_, base) = load_diff(&temp.0, Kind::AgainstBase, "review-base").unwrap();
        assert!(base.files.iter().any(|f| f.path == Path::new("tracked")));
        assert!(load_diff(&temp.0, Kind::AgainstBase, "--bad").is_err());
        assert_eq!(index, fs::read(temp.0.join(".git/index")).unwrap());
        assert_eq!(fs::read_to_string(file).unwrap(), "changed\n");
    }
    #[test]
    fn local_before_first_commit_includes_staged_and_untracked_without_writes() {
        let temp = Temp::new();
        git(&temp.0, &["init", "-q"]);
        fs::write(temp.0.join("staged"), "staged content\n").unwrap();
        git(&temp.0, &["add", "staged"]);
        fs::write(temp.0.join("untracked"), "untracked content\n").unwrap();
        let index = fs::read(temp.0.join(".git/index")).unwrap();
        fn objects(path: &Path) -> Vec<PathBuf> {
            fn walk(path: &Path, found: &mut Vec<PathBuf>) {
                for entry in fs::read_dir(path).unwrap() {
                    let path = entry.unwrap().path();
                    if path.is_dir() {
                        walk(&path, found);
                    } else {
                        found.push(path);
                    }
                }
            }
            let mut found = Vec::new();
            walk(path, &mut found);
            found.sort();
            found
        }
        let before = objects(&temp.0.join(".git/objects"));
        let (_, doc) = load_diff(&temp.0, Kind::Local, "main").unwrap();
        for name in ["staged", "untracked"] {
            assert!(
                doc.files.iter().any(|f| f.path == Path::new(name)),
                "missing {name}"
            );
        }
        assert_eq!(index, fs::read(temp.0.join(".git/index")).unwrap());
        assert_eq!(before, objects(&temp.0.join(".git/objects")));
    }
    #[test]
    fn file_errors_and_limits_are_visible() {
        let temp = Temp::new();
        assert!(read_markdown(&temp.0.join("missing")).is_err());
        assert!(read_markdown(&temp.0).is_err());
        let file = temp.0.join("large.md");
        fs::write(&file, vec![b'x'; MARKDOWN_LIMIT + 1]).unwrap();
        assert!(read_markdown(&file).unwrap_err().contains("size limit"));
        assert!(read_bounded(&vec![b'x'; DIFF_LIMIT + 1][..], DIFF_LIMIT).is_err());
        assert!(git_diff(&temp.0.join("missing"), Kind::Unstaged).is_err());
    }
    /// A file the code view cannot show as text says why in one sentence; a text file is read
    /// whole, without a byte-order mark.
    #[test]
    fn code_files_say_why_they_are_not_text() {
        let temp = Temp::new();
        let write = |name: &str, bytes: &[u8]| {
            let path = temp.0.join(name);
            fs::write(&path, bytes).unwrap();
            path
        };
        let binary = write("image.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
        assert_eq!(read_text(&binary).unwrap_err(), "Binary file");
        let latin1 = write("old.txt", b"caf\xe9\n");
        assert_eq!(read_text(&latin1).unwrap_err(), "Not UTF-8 text");
        let large = write("big.log", &vec![b'x'; CODE_LIMIT + 1]);
        assert_eq!(read_text(&large).unwrap_err(), "Larger than 1 MiB");
        let exact = write("fits.log", &vec![b'x'; CODE_LIMIT]);
        assert_eq!(read_text(&exact).unwrap().len(), CODE_LIMIT);
        let bom = write("bom.rs", "\u{feff}fn main() {}\n".as_bytes());
        assert_eq!(read_text(&bom).unwrap(), "fn main() {}\n");
        assert_eq!(
            read_text(&temp.0).unwrap_err(),
            "This is a folder, not a file"
        );
        // A NUL past the sniffed prefix is text that happens to hold one, not a binary.
        let mut late = vec![b'a'; BINARY_SNIFF];
        late.push(0);
        assert!(read_text(&write("late.txt", &late)).is_ok());
    }

    #[test]
    fn markdown_opens_formatted_and_anything_else_as_code() {
        for (name, kind) in [
            ("README.md", Kind::Markdown),
            ("NOTES.MARKDOWN", Kind::Markdown),
            ("main.rs", Kind::Code),
            ("index.php", Kind::Code),
            ("LICENSE", Kind::Code),
        ] {
            assert_eq!(Kind::of_document(Path::new(name)), kind, "{name}");
        }
        assert!(Kind::Code.is_document() && Kind::Markdown.is_document());
        assert!(!Kind::Local.is_document() && !Kind::AgainstBase.is_document());
    }

    #[cfg(unix)]
    #[test]
    fn git_timeout_does_not_wait_for_child() {
        let mut command = Command::new("sh");
        command
            .args(["-c", "exec sleep 10"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let started = Instant::now();
        assert!(
            run_git(command, Duration::from_millis(50))
                .unwrap_err()
                .contains("timed out")
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    // A file symlink, not a junction: a junction names a folder, which fails to open as a file
    // with or without the flag, so it would pass with the fix removed. GitHub's Windows runners
    // run elevated, so creating one is expected to work.
    #[cfg(windows)]
    #[test]
    fn windows_paths_untracked_file_symlink_is_not_followed() {
        let temp = Temp::new();
        fs::write(temp.0.join("outside.txt"), "secret\n").unwrap();
        std::os::windows::fs::symlink_file(temp.0.join("outside.txt"), temp.0.join("link"))
            .expect("creating a file symlink (the runner is expected to be elevated)");
        let refused = open_untracked(&temp.0, Path::new("link")).unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::InvalidInput, "{refused}");
        let mut plain = String::new();
        open_untracked(&temp.0, Path::new("outside.txt"))
            .unwrap()
            .read_to_string(&mut plain)
            .unwrap();
        assert_eq!(plain, "secret\n");
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_untracked_listing_runs_git_on_the_verbatim_root() {
        let temp = Temp::new();
        git(&temp.0, &["init", "-q"]);
        fs::write(temp.0.join("note.txt"), "hello\n").unwrap();
        let (_, doc) = load_diff(&temp.0, Kind::Local, "main").unwrap();
        let note = doc
            .files
            .iter()
            .find(|f| f.path == Path::new("note.txt"))
            .expect("the untracked file is listed");
        assert!(note.notice.is_none(), "{:?}", note.notice);
        assert_eq!(note.hunks[0].lines[0].text, "hello");
    }
}
