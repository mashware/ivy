use super::*;

/// A fresh folder of its own under the system's temporary one, removed when dropped.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("opener-{}", u64::from_ne_bytes(nonce)));
        std::fs::create_dir(&path).unwrap();
        Self(canonical(&path).unwrap())
    }

    fn file(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, "x").unwrap();
        path
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn calls(desktop: Desktop) -> Vec<Call> {
    let Desktop::Recorded(calls) = desktop else {
        unreachable!("the tests' desktop records")
    };
    calls
}

#[test]
fn documents_may_launch_and_anything_that_could_run_may_not() {
    for allowed in [
        "/r/notes.txt",
        "/r/data.JSON",
        "/r/photo.png",
        "/r/report.pdf",
        "/r/main.rs",
        "/r/archive.tar.txt",
    ] {
        assert!(may_launch(Path::new(allowed)), "{allowed} was refused");
    }
    for refused in [
        "/r/run.sh",
        "/r/tool.py",
        "/r/app.js",
        "/r/page.html",
        "/r/image.svg",
        "/r/page.xml",
        "/r/launcher.desktop",
        "/r/setup.exe",
        "/r/script.ps1",
        "/r/Makefile",
        "/r/LICENSE",
        "/r/.txt",
        "/r/notes.txt.sh",
        // A direction override paints this as «xexe.txt»; a line break would run on paste.
        "/r/x\u{202e}txt.exe",
        "/r/report\u{202e}.txt",
        "/r/two\nlines.txt",
    ] {
        assert!(!may_launch(Path::new(refused)), "{refused:?} may launch");
    }
}

#[test]
fn a_name_windows_would_change_is_told_apart_everywhere_and_refused_there() {
    assert!(altered_by_windows("notes.txt."));
    assert!(altered_by_windows("notes.txt "));
    assert!(!altered_by_windows("notes.txt"));
    #[cfg(windows)]
    assert!(!may_launch(Path::new(r"C:\r\notes.txt.")));
}

#[test]
fn the_verbatim_prefix_comes_off_for_a_drive_and_for_a_share() {
    assert_eq!(
        without_verbatim_prefix(r"\\?\C:\r\a.txt").as_deref(),
        Some(r"C:\r\a.txt")
    );
    assert_eq!(
        without_verbatim_prefix(r"\\?\UNC\host\share\a.txt").as_deref(),
        Some(r"\\host\share\a.txt")
    );
    assert_eq!(without_verbatim_prefix(r"C:\r\a.txt"), None);
}

#[test]
fn a_failure_is_said_with_the_path_escaped() {
    let line = could_not("open", Path::new("/r/a\nb.txt"), "no handler");
    assert_eq!(line, "ivy: could not open \"/r/a\\nb.txt\": no handler");
    assert!(!line.contains('\n'));
}

#[test]
fn open_launches_an_allowed_document_and_reveals_anything_else() {
    let folder = Folder::new();
    let notes = folder.file("notes.txt");
    let script = folder.file("run.sh");
    let bare = folder.file("Makefile");
    let mut desktop = Desktop::default();
    desktop.open(&notes);
    desktop.open(&script);
    desktop.open(&bare);
    desktop.reveal(&notes);
    assert_eq!(
        calls(desktop),
        [
            Call::Open(notes.clone()),
            Call::Reveal(script),
            Call::Reveal(bare),
            Call::Reveal(notes),
        ]
    );
}

crate::windows_unix_omission!(
    a_link_is_judged_by_where_it_leads,
    "unix-only: creates symlinks to distinguish a script link from a regular document"
);

/// A link is judged by the file it leads to: a `.txt` name pointing at a script is revealed, and
/// one pointing at a document opens that document.
#[cfg(unix)]
#[test]
fn a_link_is_judged_by_where_it_leads() {
    let folder = Folder::new();
    let script = folder.file("run.sh");
    let readme = folder.file("readme.txt");
    let disguised = folder.0.join("notes.txt");
    let honest = folder.0.join("about.txt");
    std::os::unix::fs::symlink(&script, &disguised).unwrap();
    std::os::unix::fs::symlink(&readme, &honest).unwrap();
    let mut desktop = Desktop::default();
    desktop.open(&disguised);
    desktop.open(&honest);
    assert_eq!(calls(desktop), [Call::Reveal(script), Call::Open(readme)]);
}

/// What `act` handed the desktop, and the line it would print.
fn acted(path: &Path, open: bool, answer: Result<(), String>) -> (Vec<Call>, Result<(), String>) {
    let mut calls = Vec::new();
    let line = act(path, open, |call| {
        calls.push(call.clone());
        answer
    });
    (calls, line)
}

/// The click's whole path, as the real desktop runs it: an allowed document reaches the launcher,
/// anything else reaches it as a reveal, and Open in File Manager never as an open.
#[test]
fn a_click_hands_the_desktop_what_it_decided() {
    let folder = Folder::new();
    let notes = folder.file("notes.txt");
    let script = folder.file("run.sh");
    assert_eq!(
        acted(&notes, true, Ok(())),
        (vec![Call::Open(notes.clone())], Ok(()))
    );
    assert_eq!(acted(&script, true, Ok(())).0, [Call::Reveal(script)]);
    assert_eq!(acted(&notes, false, Ok(())).0, [Call::Reveal(notes)]);
}

/// A failure names what was attempted: an Open that became a reveal could not show.
#[test]
fn a_failure_says_what_the_click_actually_tried() {
    let folder = Folder::new();
    let notes = folder.file("notes.txt");
    let script = folder.file("run.sh");
    let failed = |path: &Path| acted(path, true, Err("no handler".into())).1.unwrap_err();
    assert_eq!(failed(&notes), could_not("open", &notes, "no handler"));
    assert_eq!(failed(&script), could_not("show", &script, "no handler"));
    let missing = folder.0.join("gone.txt");
    let (calls, line) = acted(&missing, true, Ok(()));
    assert!(calls.is_empty());
    assert!(line.unwrap_err().starts_with("ivy: could not open"));
}

/// Only a regular file opens: a folder named like a document is shown instead.
#[test]
fn only_a_regular_file_is_launched() {
    let folder = Folder::new();
    let named = folder.0.join("notes.txt");
    std::fs::create_dir(&named).unwrap();
    let document = folder.file("real.txt");
    assert_eq!(acted(&named, true, Ok(())).0, [Call::Reveal(named)]);
    assert_eq!(acted(&document, true, Ok(())).0, [Call::Open(document)]);
}

crate::windows_unix_omission!(
    a_file_swapped_for_a_link_after_the_judgement_is_not_launched,
    "unix-only: requires Unix symlink or permission semantics"
);

/// Right before the launch, the judged file is read again: once swapped for a link, it is not
/// handed over.
#[cfg(unix)]
#[test]
fn a_file_swapped_for_a_link_after_the_judgement_is_not_launched() {
    let folder = Folder::new();
    let notes = folder.file("notes.txt");
    let script = folder.file("run.sh");
    assert_eq!(still_a_file(&notes), Ok(()));
    std::fs::remove_file(&notes).unwrap();
    std::os::unix::fs::symlink(&script, &notes).unwrap();
    assert!(still_a_file(&notes).is_err());
    // A folder in its place is refused too; a regular file again passes.
    std::fs::remove_file(&notes).unwrap();
    std::fs::create_dir(&notes).unwrap();
    assert!(still_a_file(&notes).is_err());
    assert_eq!(still_a_file(&script), Ok(()));
}

#[cfg(target_os = "linux")]
mod linux {
    use super::super::linux::{Launch, command_for, opener, wait};
    use super::Folder;
    use crate::opener::Call;
    use std::cell::RefCell;
    use std::path::Path;

    #[test]
    fn the_opener_gets_the_path_as_its_one_argument() {
        let path = Path::new("/r/a b; $(rm -rf x).txt");
        let command = opener(path);
        assert_eq!(command.get_program(), "xdg-open");
        assert_eq!(command.get_args().collect::<Vec<_>>(), [path.as_os_str()]);
    }

    fn args(command: &std::process::Command) -> Vec<&std::ffi::OsStr> {
        command.get_args().collect()
    }

    /// An open runs `xdg-open` on the file and never asks the file manager.
    #[test]
    fn an_open_runs_the_file_and_asks_no_bus() {
        let folder = Folder::new();
        let file = folder.file("notes.txt");
        let asked = RefCell::new(0);
        let launch = command_for(&Call::Open(file.clone()), |_| {
            *asked.borrow_mut() += 1;
            Ok(())
        })
        .unwrap();
        let Launch::Run(command) = launch else {
            panic!("an open runs a command")
        };
        assert_eq!(command.get_program(), "xdg-open");
        assert_eq!(args(&command), [file.as_os_str()]);
        assert_eq!(asked.into_inner(), 0);
    }

    /// The file manager is asked for the file itself; when it fails, its folder opens — never the
    /// file — and the reason travels with it.
    #[test]
    fn a_reveal_nobody_answers_opens_the_folder_and_never_the_file() {
        let folder = Folder::new();
        std::fs::create_dir(folder.0.join("src")).unwrap();
        let path = folder.file("src/my file.txt");
        let uri = url::Url::from_file_path(&path).unwrap().to_string();
        let asked = RefCell::new(Vec::new());
        let reveal = Call::Reveal(path.clone());
        let answered = command_for(&reveal, |uri| {
            asked.borrow_mut().push(uri.to_owned());
            Ok(())
        })
        .unwrap();
        assert!(matches!(answered, Launch::Done));
        let unanswered = command_for(&reveal, |uri| {
            asked.borrow_mut().push(uri.to_owned());
            Err("no service".into())
        })
        .unwrap();
        let Launch::Folder { command, why } = unanswered else {
            panic!("a folder to open instead")
        };
        assert_eq!(command.get_program(), "xdg-open");
        assert_eq!(args(&command), [folder.0.join("src").as_os_str()]);
        assert_eq!(why, "no service");
        assert_eq!(asked.into_inner(), [uri.clone(), uri]);
    }

    /// The folder a failed reveal falls back to is read again: a link in its place is not opened.
    #[test]
    fn a_folder_swapped_for_a_link_is_not_opened() {
        let folder = Folder::new();
        let page = folder.file("page.html");
        let swapped = folder.0.join("src");
        std::os::unix::fs::symlink(&page, &swapped).unwrap();
        let reveal = Call::Reveal(swapped.join("a.txt"));
        let refused = command_for(&reveal, |_| Err("no service".into()));
        assert!(refused.is_err());
        std::fs::remove_file(&swapped).unwrap();
        std::fs::create_dir(&swapped).unwrap();
        assert!(matches!(
            command_for(&reveal, |_| Err("no service".into())),
            Ok(Launch::Folder { .. })
        ));
    }

    /// The launcher is waited for, and how it ended is what the click reports.
    #[test]
    fn a_launcher_that_exits_unhappy_says_so() {
        assert_eq!(wait(std::process::Command::new("true")), Ok(()));
        let failed = wait(std::process::Command::new("false")).unwrap_err();
        assert!(
            failed.contains("\"false\"") && failed.contains("exit status: 1"),
            "{failed}"
        );
    }
}

#[cfg(all(test, windows))]
mod linux {
    crate::windows_unix_omission!(
        the_opener_gets_the_path_as_its_one_argument,
        "unix-only: invokes the Linux xdg-open resolver"
    );
    crate::windows_unix_omission!(
        an_open_runs_the_file_and_asks_no_bus,
        "unix-only: exercises Linux desktop portal fallback for xdg-open"
    );
    crate::windows_unix_omission!(
        a_reveal_nobody_answers_opens_the_folder_and_never_the_file,
        "unix-only: exercises Linux desktop portal fallback and xdg-open"
    );
    crate::windows_unix_omission!(
        a_folder_swapped_for_a_link_is_not_opened,
        "unix-only: uses Linux symlink semantics in the xdg-open resolver"
    );
    crate::windows_unix_omission!(
        a_launcher_that_exits_unhappy_says_so,
        "unix-only: invokes Linux xdg-open and inspects its process exit"
    );
}
