use super::*;
use crate::tests::window_of_tabs;
use eframe::egui::epaint::Shape;
use std::{fs, process::Command, time::Instant};

struct Directory(std::path::PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn frame(
    window: &mut crate::Window,
    ctx: &egui::Context,
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
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
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

fn click(window: &mut crate::Window, ctx: &egui::Context, label: &str) {
    let texts = frame(window, ctx, vec![]);
    let pos = texts
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("{label} not found: {texts:?}"))
        .1;
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        window,
        ctx,
        vec![egui::Event::PointerMoved(pos), button(true)],
    );
    frame(window, ctx, vec![button(false)]);
}

fn click_review(window: &mut crate::Window, ctx: &egui::Context, index: usize) {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    let metrics =
        crate::painter::CellMetrics::measure(ctx, egui::FontId::monospace(window.font_size));
    let content = crate::tab_bar::geometry(window, area, &metrics).consoles;
    let rect = crate::pane_view::view_tiling(window, content, &metrics).parts[index];
    let pane = window.active().panes()[index];
    let strip = pane
        .chrome_within(rect, &metrics, std::time::SystemTime::now(), window.headers)
        .header()
        .unwrap();
    let pos = (0..strip.width() as usize)
        .map(|x| egui::pos2(strip.left() + x as f32 + 0.5, strip.center().y))
        .find(|point| {
            crate::header::integrated::hit(strip, &metrics, true, *point)
                == Some(crate::header::integrated::Target::Review)
        })
        .unwrap();
    for pressed in [true, false] {
        frame(
            window,
            ctx,
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
}

fn choose_comparison(window: &mut crate::Window, ctx: &egui::Context, label: &str) {
    let selected = match window.viewer.kind {
        Kind::Staged => "Staged changes",
        Kind::Unstaged => "Unstaged changes",
        _ => "Local changes",
    };
    click(window, ctx, selected);
    click(window, ctx, label);
}

#[test]
fn git_viewer_opens_and_switches_modes_through_its_controls() {
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let path = std::env::temp_dir().join(format!(
        "viewer-ui-{}-{}",
        std::process::id(),
        u64::from_ne_bytes(nonce)
    ));
    fs::create_dir(&path).unwrap();
    let _directory = Directory(path.clone());
    let git = |args: &[&str]| {
        let result = Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    git(&["init", "-q"]);
    fs::write(path.join("example.txt"), "base\n").unwrap();
    git(&["add", "example.txt"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-qm",
        "base",
    ]);
    fs::write(path.join("example.txt"), "staged\n").unwrap();
    git(&["add", "example.txt"]);
    fs::write(path.join("example.txt"), "unstaged\n").unwrap();

    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    window.chrome.enable();
    frame(&mut window, &ctx, vec![]);
    frame(&mut window, &ctx, vec![]);
    let names: Vec<_> = window
        .active()
        .panes()
        .iter()
        .map(|pane| pane.name)
        .collect();
    let focused = window.active().focus;
    assert_eq!(focused, names[0]);
    for (name, cwd) in [(names[0], path.join("missing")), (names[1], path.clone())] {
        window
            .pane_called(name)
            .unwrap()
            .screen
            .advance(format!("\x1b]7;file://{}\x07", cwd.display()).as_bytes());
    }
    click_review(&mut window, &ctx, 1);
    assert_eq!(window.viewer.origin.as_ref().unwrap().pane, names[1]);
    assert_eq!(window.viewer.path, path.to_string_lossy());
    assert!(!window.viewer.editing_path);
    assert_eq!(window.active().focus, focused);
    assert!(
        !frame(&mut window, &ctx, vec![])
            .iter()
            .any(|(text, _)| text == "Repository directory")
    );
    let wait_for = |window: &mut crate::Window, expected: &str| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let painted = frame(window, &ctx, vec![]);
            if painted.iter().any(|(text, _)| text.contains(expected)) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "missing {expected}; message={:?}, requested={:?}, loading={}",
                window.viewer.message,
                window.viewer.requested,
                window.viewer.job.is_some()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    wait_for(&mut window, "unstaged");
    choose_comparison(&mut window, &ctx, "Staged changes");
    wait_for(&mut window, "staged");
    assert_eq!(window.viewer.kind, Kind::Staged);
    choose_comparison(&mut window, &ctx, "Unstaged changes");
    wait_for(&mut window, "unstaged");
}

#[test]
fn control_click_on_a_painted_markdown_path_opens_the_floating_viewer() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    frame(&mut window, &ctx, vec![]);
    frame(&mut window, &ctx, vec![]);
    let name = window.active().panes()[0].name;
    window
        .active_mut()
        .pane_called(name)
        .unwrap()
        .screen
        .advance(b"\x1b[2J\x1b[H/tmp/viewer-click.md");
    frame(&mut window, &ctx, vec![]);
    let metrics =
        crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let painted = window.active().panes()[0].painted_console.as_ref().unwrap();
    let pos = painted.area.min + egui::vec2(metrics.width() * 5.5, metrics.height() * 0.5);
    assert!(
        painted.local_at(pos, &metrics, painted.frame).is_some(),
        "painted target missing: {:?} row: {:?} size: {:?}",
        painted.view.local_path_at(0, 5),
        painted
            .view
            .row(0)
            .map(|row| row.iter().map(|c| c.c).collect::<String>()),
        painted.view.size()
    );
    let chord = egui::Modifiers {
        ctrl: true,
        ..Default::default()
    };
    frame(
        &mut window,
        &ctx,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::ModifiersChanged(chord),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: chord,
            },
        ],
    );
    assert!(window.viewer.open);
    assert_eq!(window.viewer.origin.as_ref().unwrap().pane, name);
    assert_eq!(window.viewer.path, "/tmp/viewer-click.md");
    frame(
        &mut window,
        &ctx,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: chord,
        }],
    );
    assert!(window.viewer.open);
}

#[test]
fn unavailable_console_directory_has_no_repository_picker_or_stale_content() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    window.chrome.enable();
    let ctx = egui::Context::default();
    frame(&mut window, &ctx, vec![]);
    frame(&mut window, &ctx, vec![]);
    let name = window.active().panes()[0].name;
    window
        .pane_called(name)
        .unwrap()
        .screen
        .advance(b"\x1b]7;file://remote.invalid/repository\x07");
    click_review(&mut window, &ctx, 0);
    let labels = frame(&mut window, &ctx, vec![]);
    assert!(window.viewer.open);
    assert!(window.viewer.requested.is_none());
    assert!(window.viewer.path.is_empty());
    assert!(window.viewer.message.contains("unavailable"));
    assert!(
        !labels
            .iter()
            .any(|(text, _)| text == "Repository directory" || text == "Open")
    );
}
