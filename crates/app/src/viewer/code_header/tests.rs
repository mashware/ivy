use super::super::{Kind, State, drawer, view};
use super::*;
use crate::opener::Call;
use std::time::{Duration, Instant};

#[test]
fn a_relative_path_is_counted_from_the_root_and_only_inside_it() {
    let root = Some(Path::new("/r"));
    assert_eq!(
        relative(Path::new("/r/src/a.ts"), root),
        Some(Path::new("src/a.ts"))
    );
    assert_eq!(relative(Path::new("/elsewhere/a.ts"), root), None);
    // A sibling whose name starts like the root is not inside it.
    assert_eq!(relative(Path::new("/r2/a.ts"), root), None);
    assert_eq!(relative(Path::new("/r"), root), None);
    assert_eq!(relative(Path::new("/r/src/a.ts"), None), None);
}

/// A tree of real files under the system's temporary folder, removed when dropped: what the
/// header shows is where links lead, so it needs files that exist.
struct Tree(PathBuf);

impl Tree {
    fn new(files: &[&str]) -> Self {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let base = std::env::temp_dir().join(format!("header-{}", u64::from_ne_bytes(nonce)));
        for file in files {
            let path = base.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "x".repeat(400)).unwrap();
        }
        std::fs::create_dir_all(&base).unwrap();
        Self(opener::canonical(&base).unwrap())
    }

    fn at(&self, path: &str) -> PathBuf {
        self.0
            .join(Path::new(path).components().collect::<PathBuf>())
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The File tab showing `file` as code, with `root` as the Files folder it was opened from.
fn opened(file: &Path, root: Option<&Path>) -> State {
    let mut state = State::default();
    state.open(Kind::Code);
    state.path = file.to_string_lossy().into_owned();
    state.code.header.root = root.map(Path::to_owned);
    state
}

/// What one frame painted: each text with where it sits, and what it copied.
struct Painted {
    texts: Vec<(String, egui::Rect)>,
    copied: Vec<String>,
}

impl Painted {
    fn at(&self, text: &str) -> egui::Pos2 {
        self.texts
            .iter()
            .find(|(t, _)| t == text)
            .unwrap_or_else(|| panic!("{text:?} not painted: {:?}", self.names()))
            .1
            .center()
    }

    /// Where `text` is painted inside `area`, which tells the drawer's from Files' own.
    fn at_in(&self, text: &str, area: egui::Rect) -> egui::Pos2 {
        self.texts
            .iter()
            .map(|(t, rect)| (t, rect.center()))
            .find(|(t, at)| *t == text && area.contains(*at))
            .unwrap_or_else(|| panic!("{text:?} not painted in {area:?}: {:?}", self.names()))
            .1
    }

    fn has(&self, text: &str) -> bool {
        self.texts.iter().any(|(t, _)| t == text)
    }

    fn names(&self) -> Vec<&str> {
        self.texts.iter().map(|(t, _)| t.as_str()).collect()
    }
}

struct Drawer {
    ctx: egui::Context,
    time: f64,
}

impl Drawer {
    fn new() -> Self {
        let ctx = crate::ui::icons::installed_for_tests(1.0);
        ctx.global_style_mut(|style| {
            style.animation_time = 0.0;
            style.interaction.tooltip_delay = 0.0;
            style.interaction.show_tooltips_only_when_still = false;
        });
        Self { ctx, time: 0.0 }
    }

    fn frame(&mut self, state: &mut State, events: Vec<egui::Event>) -> Painted {
        self.time += 0.05;
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0));
        let output = self.ctx.run_ui(
            egui::RawInput {
                time: Some(self.time),
                screen_rect: Some(rect),
                events,
                ..Default::default()
            },
            |ui| {
                view::show(
                    state,
                    ui,
                    rect,
                    crate::theme::Theme::default(),
                    false,
                    false,
                    (
                        &drawer::Tabs {
                            changes: None,
                            document: Some("file".into()),
                            image: None,
                            active: Some(crate::sidebar::Tool::Document),
                            side_by_side: false,
                        },
                        &drawer::Title {
                            project: None,
                            folder: "the source console".into(),
                            unavailable: false,
                        },
                        crate::viewer::view::Body::Document,
                    ),
                );
            },
        );
        let mut texts = Vec::new();
        for clipped in &output.shapes {
            if let egui::Shape::Text(text) = &clipped.shape {
                texts.push((
                    text.galley.text().to_owned(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                ));
            }
            if let Some((glyph, _, rect)) = crate::ui::icons::painted_icon(&clipped.shape) {
                texts.push((glyph, rect));
            }
        }
        let copied = output
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
        Painted { texts, copied }
    }

    /// Frames until the header knows where the file lives.
    fn settled(&mut self, state: &mut State) -> Painted {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let painted = self.frame(state, vec![]);
            if state.code.header.resolved.is_some() {
                return self.frame(state, vec![]);
            }
            assert!(Instant::now() < deadline, "the header never resolved");
            std::thread::sleep(Duration::from_millis(5));
            drop(painted);
        }
    }

    fn click(&mut self, state: &mut State, at: egui::Pos2) -> Painted {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut copied = Vec::new();
        for events in [
            vec![egui::Event::PointerMoved(at)],
            vec![button(true)],
            vec![button(false)],
        ] {
            copied.extend(self.frame(state, events).copied);
        }
        let mut painted = self.frame(state, vec![]);
        painted.copied.splice(0..0, copied);
        painted
    }

    fn hover(&mut self, state: &mut State, at: egui::Pos2) -> Painted {
        self.frame(state, vec![egui::Event::PointerMoved(at)]);
        self.frame(state, vec![egui::Event::PointerMoved(at)])
    }

    /// Open the menu and click `item` in it; what that frame and the next ones painted.
    fn menu_item(&mut self, state: &mut State, item: &str) -> Painted {
        // A fresh context paints nothing on its first pass.
        self.frame(state, vec![]);
        let painted = self.frame(state, vec![]);
        let more = painted.at(egui_phosphor::regular::CARET_DOWN);
        let menu = self.click(state, more);
        self.click(state, menu.at(item))
    }

    /// Open the menu and rest the pointer on `item`; what that frame painted, its tooltip included.
    fn menu_hover(&mut self, state: &mut State, item: &str) -> Painted {
        // A fresh context paints nothing on its first pass.
        self.frame(state, vec![]);
        let painted = self.frame(state, vec![]);
        let more = painted.at(egui_phosphor::regular::CARET_DOWN);
        let menu = self.click(state, more);
        self.hover(state, menu.at(item))
    }
}

/// Whether a painted text starts with `start`.
fn says(painted: &Painted, start: &str) -> bool {
    painted.texts.iter().any(|(t, _)| t.starts_with(start))
}

fn recorded(state: &State) -> &[Call] {
    match &state.code.header.desktop {
        opener::Desktop::Recorded(calls) => calls,
        opener::Desktop::System => unreachable!("the tests' desktop records"),
    }
}

/// The header reads from the Files folder down to the file, the file with its type's icon.
#[test]
fn the_path_runs_from_the_files_folder_to_the_file() {
    let tree = Tree::new(&["r/src/a.ts"]);
    let mut state = opened(&tree.at("r/src/a.ts"), Some(&tree.at("r")));
    let painted = Drawer::new().settled(&mut state);
    let order: Vec<f32> = ["r", "src", egui_phosphor::regular::FILE_TS, "a.ts"]
        .iter()
        .map(|text| painted.at(text).x)
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    assert_eq!(painted.texts.iter().filter(|(t, _)| t == "›").count(), 2);
}

/// Copy absolute path and Copy relative path put the literal paths on the clipboard.
#[test]
fn the_copy_items_copy_the_literal_paths() {
    let tree = Tree::new(&["r/src/a.ts"]);
    let file = tree.at("r/src/a.ts");
    let mut state = opened(&file, Some(&tree.at("r")));
    let mut drawer = Drawer::new();
    drawer.settled(&mut state);
    let absolute = drawer.menu_item(&mut state, "Copy absolute path");
    assert_eq!(absolute.copied, [file.to_str().unwrap()]);
    let relative = drawer.menu_item(&mut state, "Copy relative path");
    assert_eq!(
        relative.copied,
        [Path::new("src").join("a.ts").to_str().unwrap()]
    );
}

/// Outside the folder it was opened from, Copy relative path is off and says why; Copy absolute
/// path still copies.
#[test]
fn outside_the_files_folder_there_is_no_relative_path_and_it_says_why() {
    let tree = Tree::new(&["r/src/a.ts", "other/b.ts"]);
    let mut state = opened(&tree.at("r/src/a.ts"), Some(&tree.at("other")));
    let mut drawer = Drawer::new();
    let painted = drawer.settled(&mut state);
    // Outside the root the whole path stands for itself, with no folder to click.
    assert!(
        !painted.has("src") && !painted.has("›"),
        "{:?}",
        painted.names()
    );
    let more = painted.at(egui_phosphor::regular::CARET_DOWN);
    let menu = drawer.click(&mut state, more);
    let item = menu.at("Copy relative path");
    let hovered = drawer.hover(&mut state, item);
    assert!(
        hovered
            .texts
            .iter()
            .any(|(t, _)| t.starts_with("Outside the Files folder it was opened from")),
        "{:?}",
        hovered.names()
    );
    let clicked = drawer.click(&mut state, item);
    assert!(clicked.copied.is_empty(), "{:?}", clicked.copied);
    let absolute = drawer.menu_item(&mut state, "Copy absolute path");
    assert_eq!(absolute.copied, [tree.at("r/src/a.ts").to_str().unwrap()]);
}

crate::windows_unix_omission!(
    a_name_that_does_not_show_as_itself_is_never_copied_or_launched,
    "unix-only: constructs a non-UTF-8 path with Unix OsStr encoding"
);

/// A name with a line break or a direction override is never copied, and Open shows it in the
/// file manager instead of launching it, though its last extension is a document's.
#[cfg(unix)]
#[test]
fn a_name_that_does_not_show_as_itself_is_never_copied_or_launched() {
    for name in ["r/two\nlines.txt", "r/x\u{202e}gpj.txt"] {
        let tree = Tree::new(&[name]);
        let file = tree.at(name);
        let mut state = opened(&file, Some(&tree.at("r")));
        let mut drawer = Drawer::new();
        let painted = drawer.settled(&mut state);
        for item in ["Copy absolute path", "Copy relative path"] {
            let clicked = drawer.menu_item(&mut state, item);
            assert!(
                clicked.copied.is_empty(),
                "{name:?} {item}: {:?}",
                clicked.copied
            );
        }
        let painted = drawer.click(&mut state, painted.at("Open"));
        assert_eq!(recorded(&state), [Call::Reveal(file)], "{name:?}");
        drop(painted);
        // And each refused item says why.
        for item in ["Copy absolute path", "Copy relative path"] {
            let hovered = Drawer::new().menu_hover(&mut state, item);
            assert!(
                hovered.has(UNSAFE),
                "{name:?} {item}: {:?}",
                hovered.names()
            );
        }
    }
}

crate::windows_unix_omission!(
    a_folder_that_does_not_show_as_itself_is_never_copied,
    "unix-only: constructs a non-UTF-8 path with Unix OsStr encoding"
);

/// The path is judged whole: a line break in a folder's name refuses both Copy items, and a
/// plain path beside it copies.
#[cfg(unix)]
#[test]
fn a_folder_that_does_not_show_as_itself_is_never_copied() {
    let tree = Tree::new(&["r/two\nlines/a.txt", "r/plain/a.txt"]);
    for (name, copies) in [("r/two\nlines/a.txt", false), ("r/plain/a.txt", true)] {
        let file = tree.at(name);
        let mut state = opened(&file, Some(&tree.at("r")));
        let mut drawer = Drawer::new();
        drawer.settled(&mut state);
        for item in ["Copy absolute path", "Copy relative path"] {
            let clicked = Drawer::new().menu_item(&mut state, item);
            assert_eq!(clicked.copied.is_empty(), !copies, "{name:?} {item}");
            if !copies {
                let hovered = Drawer::new().menu_hover(&mut state, item);
                assert!(
                    hovered.has(UNSAFE),
                    "{name:?} {item}: {:?}",
                    hovered.names()
                );
            }
        }
    }
}

/// Copy relative path says why it is off: while the file is being found, when it was not opened
/// from a Files folder, and when the file or that folder could not be found.
#[test]
fn copy_relative_path_says_why_it_is_off() {
    let tree = Tree::new(&["r/src/a.ts"]);
    let file = tree.at("r/src/a.ts");
    let reason = |state: &mut State| Drawer::new().menu_hover(state, "Copy relative path");

    // Still being found: the answer is held back, then its sender goes away.
    let mut state = opened(&file, Some(&tree.at("r")));
    Drawer::new().settled(&mut state);
    let (sender, answer) = mpsc::channel();
    state.code.header.resolved = None;
    state.code.header.answer = Some(answer);
    assert!(reason(&mut state).has(FINDING));
    drop(sender);
    let stopped = reason(&mut state);
    assert!(
        says(&stopped, "Could not find this file: the search stopped"),
        "{:?}",
        stopped.names()
    );

    let mut state = opened(&file, None);
    Drawer::new().settled(&mut state);
    assert!(reason(&mut state).has(NO_FOLDER));

    let mut state = opened(&tree.at("r/src/gone.ts"), Some(&tree.at("r")));
    Drawer::new().settled(&mut state);
    let gone = reason(&mut state);
    assert!(
        says(&gone, "Could not find this file: "),
        "{:?}",
        gone.names()
    );
    assert!(!says(&gone, "Outside the Files folder"));

    let mut state = opened(&file, Some(&tree.at("gone")));
    Drawer::new().settled(&mut state);
    let lost = reason(&mut state);
    assert!(
        says(
            &lost,
            "Could not find the Files folder it was opened from: "
        ),
        "{:?}",
        lost.names()
    );

    // And with both found it copies, so the reasons above are not what any state says.
    let mut state = opened(&file, Some(&tree.at("r")));
    Drawer::new().settled(&mut state);
    let copied = Drawer::new().menu_item(&mut state, "Copy relative path");
    assert_eq!(
        copied.copied,
        [Path::new("src").join("a.ts").to_str().unwrap()]
    );
}

/// Open hands a document to its application and shows anything else in the file manager, and its
/// tooltip says which before the click; Open in File Manager always shows.
#[test]
fn open_launches_a_document_reveals_a_script_and_says_which() {
    let tree = Tree::new(&["r/notes.txt", "r/run.sh"]);
    for (name, says, call) in [
        ("r/notes.txt", OPENS, Call::Open as fn(PathBuf) -> Call),
        ("r/run.sh", REVEALS, Call::Reveal),
    ] {
        let file = tree.at(name);
        let mut state = opened(&file, Some(&tree.at("r")));
        let mut drawer = Drawer::new();
        let painted = drawer.settled(&mut state);
        let open = painted.at("Open");
        assert!(drawer.hover(&mut state, open).has(says), "{name}");
        drawer.click(&mut state, open);
        drawer.menu_item(&mut state, "Open in File Manager");
        assert_eq!(
            recorded(&state),
            [call(file.clone()), Call::Reveal(file)],
            "{name}"
        );
    }
    // A document's name on a link to a script: the tooltip and the click follow the script.
    #[cfg(unix)]
    {
        let disguised = tree.at("r/notes-link.txt");
        std::os::unix::fs::symlink(tree.at("r/run.sh"), &disguised).unwrap();
        let mut state = opened(&disguised, Some(&tree.at("r")));
        let mut drawer = Drawer::new();
        let open = drawer.settled(&mut state).at("Open");
        assert!(drawer.hover(&mut state, open).has(REVEALS));
        drawer.click(&mut state, open);
        assert_eq!(recorded(&state), [Call::Reveal(tree.at("r/run.sh"))]);
    }
}

/// Wrap lines in the menu is off until ticked, and ticked it folds the file's long line.
#[test]
fn wrap_lines_in_the_menu_folds_the_long_lines() {
    let tree = Tree::new(&["r/long.txt"]);
    let file = tree.at("r/long.txt");
    let text = format!("{}\n", "word ".repeat(80));
    let mut state = opened(&file, Some(&tree.at("r")));
    state.loaded = Some(super::super::Loaded {
        code: Some(super::super::code::CodeDocument::parse(&file, &text)),
        text,
        markdown: None,
        diff: None,
    });
    let mut drawer = Drawer::new();
    let line = |painted: &Painted| {
        painted
            .texts
            .iter()
            .find(|(t, _)| t.starts_with("word word"))
            .expect("the long line is painted")
            .1
    };
    let before = line(&drawer.settled(&mut state));
    drawer.menu_item(&mut state, "Wrap lines");
    let after = line(&drawer.frame(&mut state, vec![]));
    assert!(before.right() > 900.0, "{before:?}");
    assert!(after.right() <= 900.0 && after.height() > 2.0 * before.height());
}

/// A file that cannot be shown as text still has its header: it can be opened and copied.
#[test]
fn a_binary_file_keeps_its_header() {
    let tree = Tree::new(&["r/photo.png"]);
    let file = tree.at("r/photo.png");
    let mut state = opened(&file, Some(&tree.at("r")));
    state.message = "Binary file".into();
    let mut drawer = Drawer::new();
    let painted = drawer.settled(&mut state);
    assert!(painted.has("Binary file") && painted.has("photo.png"));
    drawer.click(&mut state, painted.at("Open"));
    assert_eq!(recorded(&state), [Call::Open(file)]);
}

mod window;
