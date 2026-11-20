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
        Self(std::fs::canonicalize(base).unwrap())
    }

    fn at(&self, path: &str) -> PathBuf {
        self.0.join(path)
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
                            markdown: true,
                            side_by_side: false,
                        },
                        &drawer::Title {
                            project: None,
                            folder: "the source console".into(),
                            unavailable: false,
                        },
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
        let painted = self.frame(state, vec![]);
        let more = painted.at(egui_phosphor::regular::CARET_DOWN);
        let menu = self.click(state, more);
        self.click(state, menu.at(item))
    }
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
    assert_eq!(relative.copied, ["src/a.ts"]);
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
        // And the refused item says why.
        let more = painted.at(egui_phosphor::regular::CARET_DOWN);
        let menu = drawer.click(&mut state, more);
        let hovered = drawer.hover(&mut state, menu.at("Copy absolute path"));
        assert!(hovered.has(UNSAFE), "{name:?}: {:?}", hovered.names());
    }
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

/// One pass of the whole window: the texts and icons it painted, and what it copied.
fn window_pass(
    window: &mut crate::Window,
    ctx: &egui::Context,
    time: &mut f64,
    events: Vec<egui::Event>,
) -> Painted {
    *time += 0.05;
    let mut frame = eframe::Frame::_new_kittest();
    let input = egui::RawInput {
        time: Some(*time),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 700.0),
        )),
        events,
        ..Default::default()
    };
    eframe::App::logic(window, ctx, &mut frame);
    let output = ctx.run_ui(input, |ui| eframe::App::ui(window, ui, &mut frame));
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

/// Run window passes until `done` holds, or fail saying `what` never happened.
fn until(
    window: &mut crate::Window,
    ctx: &egui::Context,
    time: &mut f64,
    what: &str,
    mut done: impl FnMut(&crate::Window, &Painted) -> bool,
) -> Painted {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let painted = window_pass(window, ctx, time, vec![]);
        if done(window, &painted) {
            return painted;
        }
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn window_click(
    window: &mut crate::Window,
    ctx: &egui::Context,
    time: &mut f64,
    at: egui::Pos2,
) -> Vec<String> {
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
        copied.extend(window_pass(window, ctx, time, events).copied);
    }
    copied
}

/// In the whole window: a folder of the path shows that folder in Files, and the menu, which
/// floats over the drawer, keeps its clicks — the item acts and no console is focused by it.
#[cfg(target_os = "linux")]
#[test]
fn a_folder_of_the_path_shows_in_files_and_the_menu_keeps_its_clicks() {
    let tree = Tree::new(&["r/src/a.ts"]);
    let root = tree.at("r");
    let script = format!("cd '{}' && exec sleep 30", root.display());
    let mut window = crate::tests::window_of_tabs([[script.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    let pane = window.active().panes()[0].name;
    let ctx = crate::ui::icons::installed_for_tests(1.0);
    ctx.global_style_mut(|style| style.animation_time = 0.0);
    let mut time = 0.0;
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    until(
        &mut window,
        &ctx,
        &mut time,
        "Files reaching the console's folder",
        |w, _| crate::explorer::root_of(w, pane).as_ref() == Some(&root),
    );
    assert!(crate::viewer::open_document_target(
        &mut window,
        pane,
        tree.at("r/src/a.ts"),
        None
    ));
    let painted = until(&mut window, &ctx, &mut time, "the header", |w, p| {
        w.sidebar.last_dock.is_some_and(|dock| {
            p.texts
                .iter()
                .any(|(t, r)| t == "›" && dock.contains(r.center()))
        })
    });
    drop(painted);
    // The drawer at its shortest, where the menu has the least room under the header.
    window.sidebar.set_height(0.0);
    window_pass(&mut window, &ctx, &mut time, vec![]);
    let painted = window_pass(&mut window, &ctx, &mut time, vec![]);
    let dock = window.sidebar.last_dock.expect("the drawer is placed");
    assert!(dock.height() <= 170.0, "the drawer stayed tall: {dock:?}");

    // A menu item over the drawer: it copies, and the focus stays where it was.
    let focused = window.active().focus;
    let more = painted.at_in(egui_phosphor::regular::CARET_DOWN, dock);
    window_click(&mut window, &ctx, &mut time, more);
    let menu = window_pass(&mut window, &ctx, &mut time, vec![]);
    for item in [
        "Open in File Manager",
        "Copy absolute path",
        "Copy relative path",
        "Wrap lines",
    ] {
        let at = menu.at(item);
        assert!(
            dock.contains(at),
            "{item} at {at:?} is outside the drawer {dock:?}"
        );
    }
    let item = menu.at("Copy relative path");
    let copied = window_click(&mut window, &ctx, &mut time, item);
    assert_eq!(copied, ["src/a.ts"]);
    assert_eq!(window.active().focus, focused);

    // With Files closed, a folder of the path opens Files on it and the file stays in the drawer.
    crate::sidebar::close_files(&mut window);
    window_pass(&mut window, &ctx, &mut time, vec![]);
    let painted = window_pass(&mut window, &ctx, &mut time, vec![]);
    assert!(!crate::sidebar::files_visible(&window));
    let dock = window.sidebar.last_dock.expect("the drawer is placed");
    window_click(&mut window, &ctx, &mut time, painted.at_in("src", dock));
    let painted = until(
        &mut window,
        &ctx,
        &mut time,
        "Files showing the folder",
        |w, _| crate::explorer::root_of(w, pane).as_ref() == Some(&root.join("src")),
    );
    assert!(crate::sidebar::files_visible(&window));
    let dock = window
        .sidebar
        .last_dock
        .expect("the drawer is still placed");
    painted.at_in("a.ts", dock);
}
