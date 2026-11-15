#![cfg(test)]

use super::*;
use crate::tests::window_of_tabs;
use eframe::egui::epaint::Shape;
use std::{fs, path::PathBuf, time::Instant};

struct Directory(PathBuf);

impl Directory {
    fn new(label: &str) -> Self {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!(
            "explorer-interaction-{label}-{}-{}",
            std::process::id(),
            u64::from_ne_bytes(nonce)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Worktrees {
    _temporary: Directory,
    main: PathBuf,
    branch: PathBuf,
}

impl Worktrees {
    fn new() -> Self {
        let temporary = Directory::new("worktrees");
        let main = temporary.0.join("main");
        let branch = temporary.0.join("branch");
        fs::create_dir_all(main.join("src")).unwrap();
        fs::write(main.join("src/a.md"), "main worktree").unwrap();
        for index in 0..32 {
            fs::write(main.join(format!("entry-{index:02}.md")), "entry").unwrap();
        }
        let git = |directory: &std::path::Path, args: &[&str]| {
            let result = std::process::Command::new("git")
                .arg("-C")
                .arg(directory)
                .args(args)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        };
        fs::create_dir_all(&main).unwrap();
        git(&main, &["init", "-q"]);
        git(&main, &["add", "."]);
        git(
            &main,
            &[
                "-c",
                "user.name=Explorer Test",
                "-c",
                "user.email=explorer@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "base",
            ],
        );
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature",
                branch.to_str().unwrap(),
                "HEAD",
            ],
        );
        fs::write(branch.join("src/worktree-only.md"), "branch worktree").unwrap();
        git(&branch, &["add", "src/worktree-only.md"]);
        git(
            &branch,
            &[
                "-c",
                "user.name=Explorer Test",
                "-c",
                "user.email=explorer@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "worktree file",
            ],
        );
        Self {
            _temporary: temporary,
            main,
            branch,
        }
    }
}

fn painted_frame(
    window: &mut crate::Window,
    ctx: &egui::Context,
    area: egui::Rect,
    events: Vec<egui::Event>,
) -> (Vec<(String, egui::Pos2)>, Vec<egui::epaint::ClippedShape>) {
    fn texts(shape: &Shape, found: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            Shape::Text(text) => found.push((
                text.galley.job.text.clone(),
                text.pos + text.galley.rect.center().to_vec2(),
            )),
            Shape::Vec(shapes) => {
                for shape in shapes {
                    texts(shape, found);
                }
            }
            _ => {}
        }
    }
    let mut native = eframe::Frame::_new_kittest();
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(area),
            time: Some(ctx.cumulative_frame_nr() as f64 * 0.01),
            events,
            ..Default::default()
        },
        |ui| {
            eframe::App::logic(window, ui.ctx(), &mut native);
            eframe::App::ui(window, ui, &mut native);
            if ui
                .ctx()
                .data_mut(|data| data.remove_temp::<bool>(egui::Id::new("explorer-test-discard")))
                .unwrap_or(false)
            {
                ui.ctx()
                    .request_discard("exercise native repeated-pass handoff");
            }
        },
    );
    let mut found = Vec::new();
    for shape in &output.shapes {
        texts(&shape.shape, &mut found);
    }
    let shapes = output.shapes.clone();
    output.drop_without_applying_deltas();
    (found, shapes)
}

fn frame(
    window: &mut crate::Window,
    ctx: &egui::Context,
    area: egui::Rect,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Pos2)> {
    painted_frame(window, ctx, area, events).0
}

fn click_at(window: &mut crate::Window, ctx: &egui::Context, area: egui::Rect, pos: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        window,
        ctx,
        area,
        vec![egui::Event::PointerMoved(pos), button(true)],
    );
    frame(window, ctx, area, vec![button(false)]);
}

fn click_refresh(window: &mut crate::Window, ctx: &egui::Context, area: egui::Rect) {
    frame(window, ctx, area, vec![]);
    let dock = window.sidebar.last_dock.unwrap();
    click_at(
        window,
        ctx,
        area,
        egui::pos2(dock.right() - 22.0, dock.top() + 19.0),
    );
}

fn click_text(window: &mut crate::Window, ctx: &egui::Context, area: egui::Rect, text: &str) {
    let painted = frame(window, ctx, area, vec![]);
    let dock = window.sidebar.last_dock.expect("Files dock is visible");
    let pos = painted
        .iter()
        .find(|(label, pos)| label == text && dock.contains(*pos))
        .unwrap_or_else(|| panic!("{text:?} not painted; visible text: {painted:?}"))
        .1;
    click_at(window, ctx, area, pos);
}

fn click_tab(window: &mut crate::Window, ctx: &egui::Context, area: egui::Rect, index: usize) {
    let metrics =
        crate::painter::CellMetrics::measure(ctx, egui::FontId::monospace(window.font_size));
    let cards = crate::tab_bar::geometry(window, area, &metrics)
        .cards
        .unwrap();
    let count = window.tabs.len();
    let width = ((cards.width() - 72.0) / count as f32).min(190.0);
    let center = egui::pos2(
        cards.left() + 10.0 + width * index as f32 + (width - 3.0) / 2.0,
        cards.center().y,
    );
    click_at(window, ctx, area, center);
}

fn type_text(window: &mut crate::Window, ctx: &egui::Context, area: egui::Rect, text: &str) {
    let events = text
        .chars()
        .map(|character| egui::Event::Text(character.to_string()))
        .collect();
    frame(window, ctx, area, events);
}

fn wait_for_text(
    window: &mut crate::Window,
    ctx: &egui::Context,
    area: egui::Rect,
    expected: &str,
) {
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let visible = frame(window, ctx, area, vec![]);
        if visible.iter().any(|(text, pos)| {
            text.contains(expected)
                && window
                    .sidebar
                    .last_dock
                    .is_some_and(|dock| dock.contains(*pos))
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "missing {expected:?}; visible text: {visible:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn wait_for_source(
    window: &mut crate::Window,
    ctx: &egui::Context,
    area: egui::Rect,
    pane: usize,
    expected: &std::path::Path,
) {
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        frame(window, ctx, area, vec![]);
        if window.projects.directory(pane) == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "pane {pane} did not publish {expected:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn wait_for_unavailable_source(
    window: &mut crate::Window,
    ctx: &egui::Context,
    area: egui::Rect,
    pane: usize,
) {
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        frame(window, ctx, area, vec![]);
        if window.projects.directory(pane).is_none() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "pane {pane} kept an unsupported local source"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn publish_directory(window: &mut crate::Window, pane: usize, uri: &str) {
    window
        .all_panes_mut()
        .find(|candidate| candidate.name == pane)
        .unwrap()
        .screen
        .advance(format!("\x1b]7;{uri}\x07").as_bytes());
}

#[test]
fn files_rail_follows_tabs_and_restores_navigation_without_leaking_input() {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let ctx = egui::Context::default();
    let worktrees = Worktrees::new();
    let first = &worktrees.main;
    let second = &worktrees.branch;
    let marker = second.join("outside-click.marker");
    fs::write(first.join("src/a.txt"), "text").unwrap();
    fs::write(second.join("00-second.txt"), "text").unwrap();
    let first_command = format!("cd '{}' && exec sleep 30", first.display());
    let target_command = format!(
        "cd '{}' && exec cat > '{}'",
        second.display(),
        marker.display()
    );
    let mut window = window_of_tabs([
        [first_command.as_str(), "sleep 30"],
        [target_command.as_str(), "sleep 30"],
    ]);
    window.chrome.enable();
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let ids: Vec<_> = window.all_panes().map(|pane| pane.name).collect();
    wait_for_source(&mut window, &ctx, area, ids[0], first);
    wait_for_source(&mut window, &ctx, area, ids[2], second);
    let before: Vec<_> = window
        .all_panes()
        .map(|pane| (pane.name, pane.measured))
        .collect();
    let metrics =
        crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let rail = crate::tab_bar::geometry(&window, area, &metrics).rail;
    let rail_button = egui::pos2(rail.center().x, rail.top() + 20.0);
    click_at(&mut window, &ctx, area, rail_button);
    assert!(window.explorer.visible);
    click_refresh(&mut window, &ctx, area);
    wait_for_text(&mut window, &ctx, area, "entry-00.md");
    let painted = frame(&mut window, &ctx, area, vec![]);
    let dock = window.sidebar.last_dock.unwrap();
    for removed in ["×", "Pin", "Follow", "Up", "Reset", "Refresh"] {
        assert!(
            !painted
                .iter()
                .any(|(text, pos)| text == removed && dock.contains(*pos)),
            "removed toolbar control {removed} remains"
        );
    }
    // Click the actual folder disclosure control, then select a plain-text child quietly.
    let src = painted
        .iter()
        .find(|(text, pos)| text == "src" && dock.contains(*pos))
        .unwrap()
        .1;
    let arrow = egui::pos2(dock.left() + 17.0, src.y);
    click_at(&mut window, &ctx, area, arrow);
    wait_for_text(&mut window, &ctx, area, "a.txt");
    click_text(&mut window, &ctx, area, "a.txt");
    assert!(!crate::viewer::is_open(&window));
    assert!(window.explorer.message.is_empty());
    let selected = first.join("src/a.txt");
    // Advance simulated time so these pairs are double-clicks, not part of the preceding clicks.
    for _ in 0..70 {
        frame(&mut window, &ctx, area, vec![]);
    }
    click_text(&mut window, &ctx, area, "src");
    click_text(&mut window, &ctx, area, "src");
    assert!(
        !window.explorer.navigation[&ids[0]]
            .expanded
            .contains(&first.join("src"))
    );
    // A rapid double-click on the chevron toggles once per click, never again via row activation.
    for _ in 0..70 {
        frame(&mut window, &ctx, area, vec![]);
    }
    click_at(&mut window, &ctx, area, arrow);
    click_at(&mut window, &ctx, area, arrow);
    assert!(
        !window.explorer.navigation[&ids[0]]
            .expanded
            .contains(&first.join("src"))
    );
    assert_eq!(
        window.explorer.navigation[&ids[0]].root,
        Some(first.clone())
    );
    click_at(&mut window, &ctx, area, arrow);
    wait_for_text(&mut window, &ctx, area, "a.txt");
    click_text(&mut window, &ctx, area, "a.txt");
    window.explorer.navigation.get_mut(&ids[0]).unwrap().scroll = 120.0;
    frame(&mut window, &ctx, area, vec![]);
    // Reload expanded, scrolled contents through the real icon and wait through loading frames.
    fs::write(first.join("entry-10a.txt"), "new").unwrap();
    click_refresh(&mut window, &ctx, area);
    wait_for_text(&mut window, &ctx, area, "entry-10a.txt");
    assert_eq!(window.explorer.navigation[&ids[0]].scroll, 120.0);
    assert!(
        window.explorer.navigation[&ids[0]]
            .expanded
            .contains(&first.join("src"))
    );
    click_tab(&mut window, &ctx, area, 1);
    assert_eq!(window.active, 1);
    click_refresh(&mut window, &ctx, area);
    wait_for_text(&mut window, &ctx, area, "00-second.txt");
    let b_before = window.explorer.navigation[&ids[2]].root.clone();
    click_tab(&mut window, &ctx, area, 0);
    wait_for_text(&mut window, &ctx, area, "main");
    let restored = &window.explorer.navigation[&ids[0]];
    assert_eq!(restored.root, Some(first.clone()));
    assert_eq!(restored.selected, Some(selected.clone()));
    assert!(restored.expanded.contains(&first.join("src")));
    assert_eq!(restored.scroll, 120.0);
    click_tab(&mut window, &ctx, area, 1);
    wait_for_text(&mut window, &ctx, area, "00-second.txt");
    assert_eq!(window.explorer.navigation[&ids[2]].root, b_before);
    click_text(&mut window, &ctx, area, "00-second.txt");
    let before_keys = window.pane_called(ids[2]).unwrap().keystrokes.clone();
    type_text(&mut window, &ctx, area, "SHOULD_NOT_REACH_PTY");
    assert_eq!(window.pane_called(ids[2]).unwrap().keystrokes, before_keys);
    let content = crate::tab_bar::geometry(&window, area, &metrics).consoles;
    let console = crate::pane_view::view_tiling(&window, content, &metrics).parts[0];
    click_at(&mut window, &ctx, area, console.center());
    type_text(&mut window, &ctx, area, "outside-marker\n");
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while !matches!(
        fs::read_to_string(&marker).as_deref(),
        Ok("outside-marker\n")
    ) {
        assert!(
            Instant::now() < deadline,
            "outside click did not route input to B's PTY"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    click_at(&mut window, &ctx, area, rail_button);
    assert!(!window.explorer.visible);
    click_at(&mut window, &ctx, area, rail_button);
    assert!(window.explorer.visible);
    assert_eq!(window.explorer.navigation[&ids[0]].selected, Some(selected));
    assert_eq!(window.explorer.navigation[&ids[0]].scroll, 120.0);
    // Give the explorer keyboard focus before checking Escape ownership.
    click_text(&mut window, &ctx, area, "00-second.txt");
    let before_escape = window.pane_called(ids[2]).unwrap().keystrokes.clone();
    frame(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(!window.explorer.visible);
    assert_eq!(
        window.pane_called(ids[2]).unwrap().keystrokes,
        before_escape
    );
    assert_eq!(
        window
            .all_panes()
            .map(|pane| (pane.name, pane.measured))
            .collect::<Vec<_>>(),
        before
    );
}

#[test]
fn markdown_single_click_opens_the_file_from_the_active_console() {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let ctx = egui::Context::default();
    let first = Directory::new("markdown-handoff-a");
    let second = Directory::new("markdown-handoff-b");
    fs::write(first.0.join("handoff.md"), "FIRST_SOURCE_CONTENT").unwrap();
    fs::write(second.0.join("handoff.md"), "SECOND_SOURCE_CONTENT").unwrap();
    let first_command = format!("cd '{}' && exec sleep 30", first.0.display());
    let second_command = format!("cd '{}' && exec sleep 30", second.0.display());
    let mut window = window_of_tabs([
        [first_command.as_str(), "sleep 30"],
        [second_command.as_str(), "sleep 30"],
    ]);
    window.chrome.enable();
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let ids: Vec<_> = window.all_panes().map(|pane| pane.name).collect();
    wait_for_source(&mut window, &ctx, area, ids[0], &first.0);
    wait_for_source(&mut window, &ctx, area, ids[2], &second.0);
    for (tab, expected) in [(0, "FIRST_SOURCE_CONTENT"), (1, "SECOND_SOURCE_CONTENT")] {
        click_tab(&mut window, &ctx, area, tab);
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
        frame(&mut window, &ctx, area, vec![]);
        click_refresh(&mut window, &ctx, area);
        wait_for_text(&mut window, &ctx, area, "handoff.md");
        click_text(&mut window, &ctx, area, "handoff.md");
        assert!(
            crate::viewer::is_open(&window),
            "single click did not open Markdown"
        );
        assert!(!window.explorer.visible);
        assert_eq!(window.active, tab);
        wait_for_text(&mut window, &ctx, area, expected);
    }
    // Keyboard activation remains available without first clicking the file.
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    frame(&mut window, &ctx, area, vec![]);
    let pane = ids[2];
    window.explorer.navigation.get_mut(&pane).unwrap().selected = Some(second.0.join("handoff.md"));
    frame(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(crate::viewer::is_open(&window));
    wait_for_text(&mut window, &ctx, area, "SECOND_SOURCE_CONTENT");
}

#[test]
fn remote_source_suspends_navigation_and_local_directory_changes_follow_automatically() {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let ctx = egui::Context::default();
    let original = Directory::new("source-original");
    let current = Directory::new("source-current");
    fs::create_dir(original.0.join("docs")).unwrap();
    fs::write(original.0.join("docs/original.md"), "old root").unwrap();
    fs::write(current.0.join("current.md"), "new cwd").unwrap();
    let command = format!("cd '{}' && exec sleep 30", original.0.display());
    let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let pane = window.active().panes()[0].name;
    wait_for_source(&mut window, &ctx, area, pane, &original.0);
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    frame(&mut window, &ctx, area, vec![]);
    click_refresh(&mut window, &ctx, area);

    window
        .explorer
        .navigation
        .get_mut(&pane)
        .unwrap()
        .enter(original.0.join("docs"));
    wait_for_text(&mut window, &ctx, area, "original.md");

    publish_directory(
        &mut window,
        pane,
        &format!("file://localhost{}", current.0.display()),
    );
    wait_for_source(&mut window, &ctx, area, pane, &current.0);
    wait_for_text(&mut window, &ctx, area, "current.md");
    assert_eq!(
        window.explorer.navigation[&pane].root,
        Some(current.0.clone())
    );

    publish_directory(&mut window, pane, "file://remote.invalid/unsupported");
    wait_for_unavailable_source(&mut window, &ctx, area, pane);
    wait_for_text(
        &mut window,
        &ctx,
        area,
        "Local directory unavailable or unsupported.",
    );
    assert_eq!(
        window.explorer.navigation[&pane].root,
        Some(current.0.clone())
    );

    publish_directory(
        &mut window,
        pane,
        &format!("file://localhost{}", current.0.display()),
    );
    wait_for_source(&mut window, &ctx, area, pane, &current.0);
    wait_for_text(&mut window, &ctx, area, "current.md");
    assert!(
        !frame(&mut window, &ctx, area, vec![])
            .iter()
            .any(|(text, _)| text.contains("Console directory changed") || text.starts_with('#'))
    );
    assert_eq!(
        window.explorer.navigation[&pane].root,
        Some(current.0.clone()),
        "explicit reset did not adopt the console's new local directory"
    );
}

#[test]
fn refresh_clears_a_deleted_selection_and_reports_a_deleted_explorer_root() {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let ctx = egui::Context::default();
    let directory = Directory::new("delete-refresh");
    let selected = directory.0.join("selected.txt");
    let child = directory.0.join("child");
    fs::write(&selected, "selected").unwrap();
    fs::create_dir(&child).unwrap();
    fs::write(child.join("child.md"), "child").unwrap();
    let command = format!("cd '{}' && exec sleep 30", directory.0.display());
    let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let pane = window.active().panes()[0].name;
    wait_for_source(&mut window, &ctx, area, pane, &directory.0);
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    frame(&mut window, &ctx, area, vec![]);
    click_refresh(&mut window, &ctx, area);
    wait_for_text(&mut window, &ctx, area, "selected.txt");
    click_text(&mut window, &ctx, area, "selected.txt");
    assert_eq!(
        window.explorer.navigation[&pane].selected,
        Some(selected.clone())
    );
    fs::remove_file(selected).unwrap();
    click_refresh(&mut window, &ctx, area);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while window.explorer.navigation[&pane].selected.is_some() {
        frame(&mut window, &ctx, area, vec![]);
        assert!(
            Instant::now() < deadline,
            "Refresh kept the deleted file selected"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    window
        .explorer
        .navigation
        .get_mut(&pane)
        .unwrap()
        .enter(child.clone());
    wait_for_text(&mut window, &ctx, area, "child.md");
    fs::remove_dir_all(&child).unwrap();
    click_refresh(&mut window, &ctx, area);
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let painted = frame(&mut window, &ctx, area, vec![]);
        if painted
            .iter()
            .any(|(text, _)| text.contains("Directory unavailable."))
        {
            assert!(!painted.iter().any(|(text, _)| text == "Reset"));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "missing deleted-root error: {painted:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(window.explorer.navigation[&pane].root, Some(child));
    assert_eq!(window.projects.directory(pane), Some(directory.0.as_path()));
}

#[test]
fn show_more_reaches_deep_content_without_changing_the_root() {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let ctx = egui::Context::default();
    let directory = Directory::new("large-tree");
    let command = format!("cd '{}' && exec sleep 30", directory.0.display());
    let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let pane = window.active().panes()[0].name;
    wait_for_source(&mut window, &ctx, area, pane, &directory.0);
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    frame(&mut window, &ctx, area, vec![]);
    click_refresh(&mut window, &ctx, area);
    wait_for_text(
        &mut window,
        &ctx,
        area,
        directory.0.file_name().unwrap().to_str().unwrap(),
    );
    let first = directory.0.join("4095");
    let second = first.join("4095");
    let target = second.join("deep.txt");
    for path in [&directory.0, &first, &second] {
        let entries = if path == &second {
            vec![Entry {
                path: target.clone(),
                kind: EntryKind::File,
            }]
        } else {
            (0..4096)
                .map(|index| Entry {
                    path: path.join(format!("{index:04}")),
                    kind: EntryKind::Directory,
                })
                .collect()
        };
        let request = window.explorer.request(pane, path.clone());
        window.explorer.cache.insert(
            request,
            Cached {
                result: Ok(Listing {
                    entries,
                    bytes: 0,
                    ..Default::default()
                }),
                used: 0,
            },
        );
    }
    let nav = window.explorer.navigation.get_mut(&pane).unwrap();
    nav.expanded.extend([first, second]);
    nav.selected = Some(target.clone());
    frame(&mut window, &ctx, area, vec![]);
    click_text(&mut window, &ctx, area, "Show more");
    assert_eq!(
        window.explorer.navigation[&pane].root,
        Some(directory.0.clone())
    );
    assert_eq!(window.explorer.navigation[&pane].selected, Some(target));
    assert_eq!(window.explorer.navigation[&pane].scroll, 0.0);
    window.explorer.navigation.get_mut(&pane).unwrap().scroll = 8192.0 * window.explorer.row_stride;
    wait_for_text(&mut window, &ctx, area, "deep.txt");
    assert!(
        !frame(&mut window, &ctx, area, vec![])
            .iter()
            .any(|(text, _)| text == "Show more")
    );
}

#[test]
#[cfg(unix)]
fn clicking_a_linked_markdown_file_opens_the_existing_viewer() {
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let directory = Directory::new("linked-markdown");
    fs::write(directory.0.join("source.md"), "LINKED_MARKDOWN_CONTENT").unwrap();
    std::os::unix::fs::symlink(directory.0.join("source.md"), directory.0.join("AGENTS.md"))
        .unwrap();
    let command = format!("cd '{}' && exec sleep 30", directory.0.display());
    let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let pane = window.active().panes()[0].name;
    wait_for_source(&mut window, &ctx, area, pane, &directory.0);
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    wait_for_text(&mut window, &ctx, area, "AGENTS.md");
    let painted = frame(&mut window, &ctx, area, vec![]);
    let dock = window.sidebar.last_dock.unwrap();
    let pos = painted
        .iter()
        .find(|(text, pos)| text == "AGENTS.md ↗" && dock.contains(*pos))
        .unwrap()
        .1;
    for pressed in [true, false] {
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("explorer-test-discard"), true));
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    wait_for_text(&mut window, &ctx, area, "LINKED_MARKDOWN_CONTENT");
    assert_eq!(
        crate::sidebar::active(&window),
        Some(crate::sidebar::Tool::Markdown)
    );
}

#[test]
fn typed_name_search_finds_files_and_folders_below_collapsed_directories() {
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let directory = Directory::new("name-search");
    fs::create_dir_all(directory.0.join("src/nested/NeedleFolder")).unwrap();
    fs::write(directory.0.join("src/nested/NeedleFile.rs"), "not searched").unwrap();
    fs::write(
        directory.0.join("src/nested/NeedleFolder/inside.txt"),
        "inside",
    )
    .unwrap();
    fs::write(
        directory.0.join("src/nested/NeedleFolder/NeedleChild.txt"),
        "matching child",
    )
    .unwrap();
    fs::write(directory.0.join("unrelated.rs"), "needle in contents only").unwrap();
    let command = format!("cd '{}' && exec sleep 30", directory.0.display());
    let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let pane = window.active().panes()[0].name;
    wait_for_source(&mut window, &ctx, area, pane, &directory.0);
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    wait_for_text(&mut window, &ctx, area, "src");
    click_text(&mut window, &ctx, area, "Search files and folders");
    let before = window.pane_called(pane).unwrap().keystrokes.clone();
    type_text(&mut window, &ctx, area, "needle");
    wait_for_text(&mut window, &ctx, area, "src/nested/NeedleFile.rs");
    let found = frame(&mut window, &ctx, area, vec![]);
    assert!(
        found
            .iter()
            .any(|(text, _)| text == "src/nested/NeedleFolder")
    );
    assert!(!found.iter().any(|(text, _)| text == "unrelated.rs"));
    assert_eq!(window.pane_called(pane).unwrap().keystrokes, before);
    for _ in 0..70 {
        frame(&mut window, &ctx, area, vec![]);
    }
    click_text(&mut window, &ctx, area, "src/nested/NeedleFolder");
    click_text(&mut window, &ctx, area, "src/nested/NeedleFolder");
    wait_for_text(&mut window, &ctx, area, "inside.txt");
    assert_eq!(
        window.explorer.navigation[&pane].root,
        Some(directory.0.clone())
    );
    assert_eq!(window.explorer.navigation[&pane].query, "needle");
    assert_eq!(
        frame(&mut window, &ctx, area, vec![])
            .iter()
            .filter(|(text, _)| text.ends_with("NeedleChild.txt"))
            .count(),
        1
    );
    frame(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    wait_for_text(&mut window, &ctx, area, "unrelated.rs");
    assert_eq!(
        window.explorer.navigation[&pane].root,
        Some(directory.0.clone())
    );
    assert!(window.explorer.visible);
}

#[test]
fn ctrl_clicking_a_local_path_reveals_its_own_consoles_file_and_scrolls_to_it() {
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let directory = Directory::new("path-reveal");
    fs::create_dir(directory.0.join("folder")).unwrap();
    for index in 0..80 {
        fs::write(
            directory.0.join(format!("folder/entry-{index:02}.rs")),
            "entry",
        )
        .unwrap();
    }
    let command = format!("cd '{}' && exec sleep 30", directory.0.display());
    let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let pane = window.active().panes()[0].name;
    wait_for_source(&mut window, &ctx, area, pane, &directory.0);
    let target = directory.0.join("folder/entry-60.rs");
    let other = window.active().panes()[1].name;
    window.active_mut().focus = other;
    window
        .pane_called(pane)
        .unwrap()
        .screen
        .advance(b"\x1b[?25l\x1b[2J\x1b[Hfolder/entry-60.rs");
    frame(&mut window, &ctx, area, vec![]);
    let metrics =
        crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let console = window
        .pane_called(pane)
        .unwrap()
        .painted_console
        .as_ref()
        .unwrap()
        .area;
    let pos = console.min + egui::vec2(metrics.width() * 3.5, metrics.height() * 0.5);
    for pressed in [true, false] {
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::CTRL,
                },
            ],
        );
    }
    wait_for_text(&mut window, &ctx, area, "entry-60.rs");
    assert_eq!(
        window.explorer.navigation[&pane].root,
        Some(directory.0.join("folder"))
    );
    assert_eq!(window.explorer.navigation[&pane].selected, Some(target));
    assert!(window.explorer.navigation[&pane].scroll > 0.0);
    assert_eq!(
        crate::sidebar::active(&window),
        Some(crate::sidebar::Tool::Files)
    );
    for (text, expected) in [
        ("./folder", "entry-00.rs"),
        ("./missing.rs", "Path unavailable"),
    ] {
        window
            .pane_called(pane)
            .unwrap()
            .screen
            .advance(format!("\x1b[2J\x1b[H{text}").as_bytes());
        frame(&mut window, &ctx, area, vec![]);
        let console = window
            .pane_called(pane)
            .unwrap()
            .painted_console
            .as_ref()
            .unwrap()
            .area;
        let pos = console.min + egui::vec2(metrics.width() * 3.5, metrics.height() * 0.5);
        for pressed in [true, false] {
            frame(
                &mut window,
                &ctx,
                area,
                vec![
                    egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::CTRL,
                    },
                ],
            );
        }
        wait_for_text(&mut window, &ctx, area, expected);
        assert_eq!(
            window.explorer.navigation[&pane].root,
            Some(directory.0.join("folder"))
        );
        assert!(window.explorer.navigation[&pane].selected.is_none());
        assert_eq!(window.explorer.navigation[&pane].scroll, 0.0);
    }
    wait_for_text(&mut window, &ctx, area, "entry-00.rs");
}

#[test]
fn scrolling_files_keeps_console_paint_and_geometry_stable() {
    fn colors(shapes: &[egui::epaint::ClippedShape], area: egui::Rect) -> Vec<egui::Color32> {
        shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Text(text) if area.contains(text.pos) => Some(text),
                _ => None,
            })
            .flat_map(|text| text.galley.rows.iter())
            .flat_map(|row| {
                row.row
                    .visuals
                    .mesh
                    .vertices
                    .iter()
                    .map(|vertex| vertex.color)
            })
            .collect()
    }
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let directory = Directory::new("scroll-opacity");
    for index in 0..100 {
        fs::write(directory.0.join(format!("entry-{index:03}.txt")), "entry").unwrap();
    }
    let command = format!("cd '{}' && exec sleep 30", directory.0.display());
    let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let pane = window.active().panes()[0].name;
    wait_for_source(&mut window, &ctx, area, pane, &directory.0);
    window
        .pane_called(pane)
        .unwrap()
        .screen
        .advance(b"\x1b[?25l\x1b[2J\x1b[HCONSOLE_OPACITY_WITNESS");
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    wait_for_text(&mut window, &ctx, area, "entry-000.txt");
    let baseline = painted_frame(&mut window, &ctx, area, vec![]).1;
    let console = window
        .pane_called(pane)
        .unwrap()
        .painted_console
        .as_ref()
        .unwrap()
        .area;
    let before = colors(&baseline, console);
    assert!(!before.is_empty());
    let grid = window.pane_called(pane).unwrap().measured;
    let dock = window.sidebar.last_dock.unwrap();
    let pos = egui::pos2(dock.right() - 15.0, dock.top() + 110.0);
    let pressed = painted_frame(
        &mut window,
        &ctx,
        area,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    )
    .1;
    assert_eq!(colors(&pressed, console), before);
    let moved = pos + egui::vec2(0.0, 140.0);
    let dragged = painted_frame(
        &mut window,
        &ctx,
        area,
        vec![
            egui::Event::PointerMoved(moved),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -180.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ],
    )
    .1;
    assert_eq!(colors(&dragged, console), before);
    assert_eq!(window.pane_called(pane).unwrap().measured, grid);
    assert!(window.explorer.navigation[&pane].scroll > 0.0);
}

#[test]
fn partial_name_search_notice_is_visible_even_when_the_folder_exceeds_listing_limits() {
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let directory = Directory::new("partial-name-search");
    for index in 0..4097 {
        fs::write(directory.0.join(format!("match-{index:04}.txt")), "entry").unwrap();
    }
    let command = format!("cd '{}' && exec sleep 30", directory.0.display());
    let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, area, vec![]);
    let pane = window.active().panes()[0].name;
    wait_for_source(&mut window, &ctx, area, pane, &directory.0);
    crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
    wait_for_text(&mut window, &ctx, area, "Directory exceeds explorer limit");
    click_text(&mut window, &ctx, area, "Search files and folders");
    type_text(&mut window, &ctx, area, "match");
    wait_for_text(&mut window, &ctx, area, "Search result limit reached");
    wait_for_text(&mut window, &ctx, area, "match-");
}

#[test]
fn hidden_files_reconciliation_is_discounted_from_framework_time() {
    let ctx = egui::Context::default();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    assert!(!window.explorer.visible);
    let before = window.timed_separately;
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        show(&mut window, ui, ui.max_rect());
    });
    output.drop_without_applying_deltas();
    assert!(window.timed_separately > before);
    window.sidebar.close_frame();
    let mut report = Vec::new();
    window.sidebar.publish(&mut report).unwrap();
    assert!(String::from_utf8(report).unwrap().contains("frames=1"));
}
