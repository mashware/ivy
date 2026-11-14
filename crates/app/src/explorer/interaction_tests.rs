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

#[test]
fn files_menu_pins_and_follows_across_tabs_without_leaking_input_or_retiling() {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let ctx = egui::Context::default();
    let first = Directory::new("first");
    let second = Directory::new("second");
    let marker = second.0.join("outside-click.marker");
    fs::write(first.0.join("first.md"), "first").unwrap();
    fs::write(second.0.join("second.md"), "second").unwrap();
    let first_command = format!("cd '{}' && exec sleep 30", first.0.display());
    let target_command = format!(
        "cd '{}' && exec cat > '{}'",
        second.0.display(),
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
    wait_for_source(&mut window, &ctx, area, ids[0], &first.0);
    wait_for_source(&mut window, &ctx, area, ids[2], &second.0);
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
    wait_for_text(&mut window, &ctx, area, "first.md");
    assert!(window.explorer.navigation[&ids[0]].root.as_deref() == Some(first.0.as_path()));

    click_text(&mut window, &ctx, area, "Pin this console");
    assert_eq!(window.explorer.pinned, Some(ids[0]));
    // A tab card changes active focus after explorer ownership is captured in the same frame.
    let metrics =
        crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let cards = crate::tab_bar::geometry(&window, area, &metrics)
        .cards
        .unwrap();
    let card_width = ((cards.width() - 72.0) / 2.0).min(190.0);
    let second_card = egui::pos2(
        cards.left() + 10.0 + card_width + (card_width - 3.0) / 2.0,
        cards.center().y,
    );
    click_at(&mut window, &ctx, area, second_card);
    assert_eq!(
        window.active, 1,
        "tab card at {second_card:?} within cards {cards:?}; explorer keyboard={}, held={}, visible={}",
        window.explorer.keyboard, window.explorer.held, window.explorer.visible,
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
        Some(first.0.clone())
    );

    // Explorer keys and pointer gestures are consumed before the terminal dispatcher.
    // Reclaim keyboard ownership after the tab click transferred it to the active console.
    click_text(&mut window, &ctx, area, "first.md");
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
    while fs::read_to_string(&marker).unwrap_or_default() != "outside-marker\n" {
        assert!(
            Instant::now() < deadline,
            "outside click did not route input to B's PTY"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(fs::read_to_string(&marker).unwrap(), "outside-marker\n");
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
    wait_for_text(&mut window, &ctx, area, "second.md");
    assert_eq!(window.explorer.target(Some(ids[2])), Some(ids[2]));
    assert_eq!(
        window.explorer.navigation[&ids[2]].root,
        Some(second.0.clone())
    );

    // Hide and reopen retain each pane's tree and pin state. The panel remains an overlay.
    click_text(&mut window, &ctx, area, "Pin this console");
    assert_eq!(window.explorer.pinned, Some(ids[2]));
    window
        .explorer
        .navigation
        .get_mut(&ids[0])
        .unwrap()
        .selected = Some(first.0.join("first.md"));
    window.explorer.navigation.get_mut(&ids[0]).unwrap().scroll = 120.0;
    click_text(&mut window, &ctx, area, "Hide");
    assert!(!window.explorer.visible);
    toggle(&mut window, &ctx);
    frame(&mut window, &ctx, area, vec![]);
    assert!(window.explorer.visible);
    assert_eq!(
        window.explorer.navigation[&ids[0]].selected,
        Some(first.0.join("first.md"))
    );
    assert_eq!(window.explorer.navigation[&ids[0]].scroll, 120.0);
    assert_eq!(window.explorer.pinned, Some(ids[2]));
    assert_eq!(
        window
            .all_panes()
            .map(|pane| (pane.name, pane.measured))
            .collect::<Vec<_>>(),
        before,
        "the overlay must not change console geometry"
    );
}
