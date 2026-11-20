use super::*;

/// A fresh folder of its own under the system's temporary one, removed when dropped.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("opener-{}", u64::from_ne_bytes(nonce)));
        std::fs::create_dir(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
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

#[cfg(target_os = "linux")]
mod linux {
    use super::super::linux::{fallback, opener};
    use std::cell::RefCell;
    use std::path::Path;

    #[test]
    fn the_opener_gets_the_path_as_its_one_argument() {
        let path = Path::new("/r/a b; $(rm -rf x).txt");
        let command = opener(path);
        assert_eq!(command.get_program(), "xdg-open");
        assert_eq!(command.get_args().collect::<Vec<_>>(), [path.as_os_str()]);
    }

    /// The file manager is asked for the file itself; when it does not answer, its folder opens —
    /// never the file.
    #[test]
    fn a_reveal_nobody_answers_opens_the_folder_and_never_the_file() {
        let path = Path::new("/r/src/my file.txt");
        let asked = RefCell::new(Vec::new());
        let answered = fallback(path, |uri| {
            asked.borrow_mut().push(uri.to_owned());
            Ok(())
        })
        .unwrap();
        assert!(answered.is_none());
        let unanswered = fallback(path, |uri| {
            asked.borrow_mut().push(uri.to_owned());
            Err("no service".into())
        })
        .unwrap()
        .expect("a folder to open instead");
        assert_eq!(unanswered.get_program(), "xdg-open");
        assert_eq!(
            unanswered.get_args().collect::<Vec<_>>(),
            [Path::new("/r/src").as_os_str()]
        );
        assert_eq!(
            asked.into_inner(),
            ["file:///r/src/my%20file.txt", "file:///r/src/my%20file.txt"]
        );
    }
}
