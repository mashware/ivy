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

fn frame(
    window: &mut crate::Window,
    ctx: &egui::Context,
    area: egui::Rect,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Pos2)> {
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
    eframe::App::logic(window, ctx, &mut native);
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(area),
            events,
            ..Default::default()
        },
        |ui| eframe::App::ui(window, ui, &mut native),
    );
    let mut found = Vec::new();
    for shape in &output.shapes {
        texts(&shape.shape, &mut found);
    }
    output.drop_without_applying_deltas();
    found
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

fn click_text(window: &mut crate::Window, ctx: &egui::Context, area: egui::Rect, text: &str) {
    let painted = frame(window, ctx, area, vec![]);
    let pos = painted
        .iter()
        .find(|(label, _)| label == text)
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
        if visible.iter().any(|(text, _)| text.contains(expected)) {
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
fn files_menu_pins_and_follows_across_tabs_without_leaking_input_or_retiling() {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let ctx = egui::Context::default();
    let worktrees = Worktrees::new();
    let first = &worktrees.main;
    let second = &worktrees.branch;
    let marker = second.join("outside-click.marker");
    fs::write(second.join("second.md"), "second").unwrap();
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
    frame(&mut window, &ctx, area, vec![]);

    // The integrated titlebar menu exposes Files as a user-reachable entry.
    click_at(&mut window, &ctx, area, egui::pos2(1075.0, 25.0));
    assert!(
        frame(&mut window, &ctx, area, vec![])
            .iter()
            .any(|(text, _)| text == "Files"),
        "the titlebar menu should show a Files entry"
    );
    click_text(&mut window, &ctx, area, "Files");
    frame(&mut window, &ctx, area, vec![]);
    assert!(window.explorer.visible);
    wait_for_text(&mut window, &ctx, area, "entry-00.md");
    assert_eq!(
        window.explorer.navigation[&ids[0]].root.as_deref(),
        Some(first.as_path())
    );

    // Expand A/src, select its Markdown child, then render with enough rows to retain 120 points.
    let a_child = first.join("src/a.md");
    window
        .explorer
        .navigation
        .get_mut(&ids[0])
        .unwrap()
        .expanded
        .insert(first.join("src"));
    frame(&mut window, &ctx, area, vec![]);
    wait_for_text(&mut window, &ctx, area, "a.md");
    click_text(&mut window, &ctx, area, "a.md");
    window.explorer.navigation.get_mut(&ids[0]).unwrap().scroll = 120.0;
    frame(&mut window, &ctx, area, vec![]);
    assert_eq!(
        window.explorer.navigation[&ids[0]].selected,
        Some(a_child.clone())
    );
    assert_eq!(window.explorer.navigation[&ids[0]].scroll, 120.0);

    click_text(&mut window, &ctx, area, "Pin this console");
    assert_eq!(window.explorer.pinned, Some(ids[0]));
    // A tab card changes active focus after explorer ownership is captured in the same frame.
    let metrics =
        crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    click_tab(&mut window, &ctx, area, 1);
    assert_eq!(
        window.active, 1,
        "explorer did not allow the real tab-card click"
    );
    assert!(
        frame(&mut window, &ctx, area, vec![])
            .iter()
            .any(|(text, _)| text.contains("Pinned:") && text.contains(&format!("#{}", ids[0]))),
        "pinned A should remain the displayed target after switching tabs"
    );
    assert_eq!(window.explorer.target(Some(ids[2])), Some(ids[0]));
    assert_eq!(
        window.explorer.navigation[&ids[0]].root,
        Some(first.clone())
    );

    // Follow B, then switch away and back: A's rendered navigation survives while B stays fresh.
    click_text(&mut window, &ctx, area, "Follow active pane");
    wait_for_text(&mut window, &ctx, area, "branch");
    let b_before = (
        window.explorer.navigation[&ids[2]].root.clone(),
        window.explorer.navigation[&ids[2]].expanded.clone(),
        window.explorer.navigation[&ids[2]].selected.clone(),
        window.explorer.navigation[&ids[2]].scroll,
    );
    click_tab(&mut window, &ctx, area, 0);
    assert_eq!(window.active, 0);
    wait_for_text(&mut window, &ctx, area, "main");
    assert_eq!(window.explorer.target(Some(ids[0])), Some(ids[0]));
    let a_restored = &window.explorer.navigation[&ids[0]];
    assert_eq!(a_restored.root, Some(first.clone()));
    assert!(a_restored.expanded.contains(&first.join("src")));
    assert_eq!(a_restored.selected, Some(a_child.clone()));
    assert_eq!(a_restored.scroll, 120.0);
    click_tab(&mut window, &ctx, area, 1);
    assert_eq!(window.active, 1);
    wait_for_text(&mut window, &ctx, area, "branch");
    let b_after = (
        window.explorer.navigation[&ids[2]].root.clone(),
        window.explorer.navigation[&ids[2]].expanded.clone(),
        window.explorer.navigation[&ids[2]].selected.clone(),
        window.explorer.navigation[&ids[2]].scroll,
    );
    assert_eq!(b_after, b_before, "B's navigation changed while browsing A");

    // Pin A again while it is active, then return to B to exercise its outside-click delivery.
    click_tab(&mut window, &ctx, area, 0);
    wait_for_text(&mut window, &ctx, area, "main");
    click_text(&mut window, &ctx, area, "Pin this console");
    click_tab(&mut window, &ctx, area, 1);
    assert_eq!(window.explorer.pinned, Some(ids[0]));
    assert_eq!(window.active, 1);

    // Explorer keys and pointer gestures are consumed before the terminal dispatcher.
    // Reclaim keyboard ownership after the tab click transferred it to the active console.
    click_text(&mut window, &ctx, area, "entry-01.md");
    let before_keystrokes = window.pane_called(ids[2]).unwrap().keystrokes.len();
    type_text(&mut window, &ctx, area, "SHOULD_NOT_REACH_PTY");
    frame(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    frame(
        &mut window,
        &ctx,
        area,
        vec![
            egui::Event::PointerMoved(egui::pos2(120.0, 350.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, -2.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
            egui::Event::PointerButton {
                pos: egui::pos2(120.0, 350.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert_eq!(
        window.pane_called(ids[2]).unwrap().keystrokes.len(),
        before_keystrokes
    );

    // A click on the console transfers focus to B while preserving the pinned A tree.
    let content = crate::tab_bar::geometry(&window, area, &metrics).consoles;
    let b_rect = crate::pane_view::view_tiling(&window, content, &metrics).parts[0];
    click_at(&mut window, &ctx, area, b_rect.center());
    frame(&mut window, &ctx, area, vec![]);
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
    assert_eq!(window.explorer.target(Some(ids[2])), Some(ids[0]));

    // Follow resolves to B immediately, and the same frame paints B's root.
    click_text(&mut window, &ctx, area, "Follow active pane");
    assert_eq!(window.explorer.pinned, None);
    let painted = frame(&mut window, &ctx, area, vec![]);
    assert!(
        painted
            .iter()
            .any(|(text, _)| !text.contains("Pinned:") && text.contains("Following:"))
    );
    wait_for_text(&mut window, &ctx, area, "branch");
    assert_eq!(window.explorer.target(Some(ids[2])), Some(ids[2]));
    assert_eq!(
        window.explorer.navigation[&ids[2]].root,
        Some(second.clone())
    );

    // Hide and reopen retain each pane's tree and pin state. The panel remains an overlay.
    click_text(&mut window, &ctx, area, "Pin this console");
    assert_eq!(window.explorer.pinned, Some(ids[2]));
    window
        .explorer
        .navigation
        .get_mut(&ids[0])
        .unwrap()
        .selected = Some(a_child.clone());
    window.explorer.navigation.get_mut(&ids[0]).unwrap().scroll = 120.0;
    click_text(&mut window, &ctx, area, "Hide");
    assert!(!window.explorer.visible);
    toggle(&mut window, &ctx);
    frame(&mut window, &ctx, area, vec![]);
    assert!(window.explorer.visible);
    assert_eq!(window.explorer.navigation[&ids[0]].selected, Some(a_child));
    assert_eq!(window.explorer.navigation[&ids[0]].scroll, 120.0);
    assert_eq!(window.explorer.pinned, Some(ids[2]));
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
    assert!(!window.explorer.visible, "Escape should close the explorer");
    assert_eq!(
        window.pane_called(ids[2]).unwrap().keystrokes,
        before_escape,
        "Escape leaked to the active PTY"
    );
    assert_eq!(
        window
            .all_panes()
            .map(|pane| (pane.name, pane.measured))
            .collect::<Vec<_>>(),
        before,
        "the overlay must not change console geometry"
    );
}

#[test]
fn markdown_selection_opens_from_the_pinned_pane_only_after_enter() {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let ctx = egui::Context::default();
    let first = Directory::new("markdown-handoff-a");
    let second = Directory::new("markdown-handoff-b");
    fs::write(first.0.join("handoff.md"), "PINNED_SOURCE_CONTENT").unwrap();
    fs::write(second.0.join("handoff.md"), "ACTIVE_SOURCE_CONTENT").unwrap();
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
    toggle(&mut window, &ctx);
    frame(&mut window, &ctx, area, vec![]);
    assert_eq!(window.explorer.target(Some(ids[0])), Some(ids[0]));
    click_text(&mut window, &ctx, area, "handoff.md");
    assert!(window.explorer.visible);
    assert!(
        !crate::viewer::is_open(&window),
        "single click opened the viewer"
    );
    assert_eq!(
        window.explorer.navigation[&ids[0]].selected,
        Some(first.0.join("handoff.md"))
    );

    click_text(&mut window, &ctx, area, "Pin this console");
    click_tab(&mut window, &ctx, area, 1);
    assert_eq!(window.active, 1);
    assert_eq!(window.explorer.target(Some(ids[2])), Some(ids[0]));
    click_text(&mut window, &ctx, area, "handoff.md");
    assert_eq!(
        window.active, 1,
        "selecting the pinned tree changed active tab"
    );
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
    assert!(
        crate::viewer::is_open(&window),
        "Enter did not open the selected file"
    );
    assert!(
        !window.explorer.visible,
        "the explorer stayed over the viewer"
    );
    wait_for_text(&mut window, &ctx, area, "PINNED_SOURCE_CONTENT");
    assert!(
        frame(&mut window, &ctx, area, vec![])
            .iter()
            .any(|(text, _)| text
                .contains(&first.0.join("handoff.md").to_string_lossy().to_string())),
        "viewer did not open the Markdown file from the pinned pane"
    );
}

#[test]
fn remote_source_suspends_saved_navigation_and_console_directory_resets_it_explicitly() {
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
    toggle(&mut window, &ctx);
    frame(&mut window, &ctx, area, vec![]);

    window
        .explorer
        .navigation
        .get_mut(&pane)
        .unwrap()
        .enter(original.0.join("docs"));
    wait_for_text(&mut window, &ctx, area, "original.md");
    let saved_root = original.0.join("docs");

    publish_directory(
        &mut window,
        pane,
        &format!("file://localhost{}", current.0.display()),
    );
    wait_for_source(&mut window, &ctx, area, pane, &current.0);
    wait_for_text(&mut window, &ctx, area, "Console directory changed");
    assert_eq!(
        window.explorer.navigation[&pane].root,
        Some(saved_root.clone())
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
        Some(saved_root.clone())
    );

    publish_directory(
        &mut window,
        pane,
        &format!("file://localhost{}", current.0.display()),
    );
    wait_for_source(&mut window, &ctx, area, pane, &current.0);
    wait_for_text(&mut window, &ctx, area, "Console directory changed");
    assert_eq!(window.explorer.navigation[&pane].root, Some(saved_root));
    click_text(&mut window, &ctx, area, "Use console directory");
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
    let selected = directory.0.join("selected.md");
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
    toggle(&mut window, &ctx);
    frame(&mut window, &ctx, area, vec![]);
    wait_for_text(&mut window, &ctx, area, "selected.md");
    click_text(&mut window, &ctx, area, "selected.md");
    assert_eq!(
        window.explorer.navigation[&pane].selected,
        Some(selected.clone())
    );
    fs::remove_file(selected).unwrap();
    click_text(&mut window, &ctx, area, "Refresh");
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
    click_text(&mut window, &ctx, area, "Refresh");
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let painted = frame(&mut window, &ctx, area, vec![]);
        if painted
            .iter()
            .any(|(text, _)| text.contains("Directory unavailable."))
        {
            assert!(painted.iter().any(|(text, _)| text == "Refresh"));
            assert!(
                painted
                    .iter()
                    .any(|(text, _)| text == "Use console directory")
            );
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
