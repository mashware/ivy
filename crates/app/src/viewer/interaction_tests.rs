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
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(area),
            events,
            ..Default::default()
        },
        |ui| {
            eframe::App::logic(window, ui.ctx(), &mut native);
            eframe::App::ui(window, ui, &mut native);
        },
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
            let offer =
                crate::header::integrated::Offer::of(rect, &metrics, window.headers, true, false);
            crate::header::integrated::hit(strip, offer, *point)
                == Some(crate::header::integrated::Target::Review)
        })
        .unwrap();
    // A header's controls are pressable once they were painted (D99): a pass over the header first.
    frame(window, ctx, vec![egui::Event::PointerMoved(pos)]);
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

    let script = format!("cd '{}' && sleep 30", path.display());
    let mut window = window_of_tabs([["sleep 30", script.as_str()]]);
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
    #[cfg(target_os = "linux")]
    {
        let deadline = Instant::now() + Duration::from_secs(5);
        while window
            .pane_called(names[1])
            .unwrap()
            .pty
            .working_directory()
            .as_ref()
            != Some(&path)
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
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
fn control_click_reveals_markdown_in_files_before_opening_its_viewer() {
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let path = std::env::temp_dir().join(format!("viewer-path-{:x}", u64::from_ne_bytes(nonce)));
    fs::create_dir(&path).unwrap();
    let _directory = Directory(path.clone());
    fs::write(
        path.join("viewer-click.md"),
        "CLICK_THROUGH_MARKDOWN_CONTENT",
    )
    .unwrap();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
    frame(&mut window, &ctx, vec![]);
    frame(&mut window, &ctx, vec![]);
    let name = window.active().panes()[0].name;
    window
        .active_mut()
        .pane_called(name)
        .unwrap()
        .screen
        .advance(
            format!(
                "\x1b]7;{}\x07\x1b[2J\x1b[H{}",
                url::Url::from_directory_path(&path).unwrap(),
                path.join("viewer-click.md").display()
            )
            .as_bytes(),
        );
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
    assert!(crate::sidebar::files_visible(&window));
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
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if frame(&mut window, &ctx, vec![])
            .iter()
            .any(|(text, _)| text == "viewer-click.md")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Markdown location was not revealed"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    click(&mut window, &ctx, "viewer-click.md");
    assert!(window.viewer.open);
    assert_eq!(window.viewer.origin.as_ref().unwrap().pane, name);
    assert_eq!(
        window.viewer.path,
        path.join("viewer-click.md").to_string_lossy()
    );
    loop {
        if frame(&mut window, &ctx, vec![])
            .iter()
            .any(|(text, _)| text.contains("CLICK_THROUGH_MARKDOWN_CONTENT"))
        {
            break;
        }
        assert!(Instant::now() < deadline, "Markdown was not rendered");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

crate::windows_unix_omission!(
    non_repository_console_paints_a_clear_message_after_review_click,
    "unix-only: verifies a real shell process and Linux repository state after a viewer interaction"
);

/// Ctrl+click on a screenshot path a console printed opens it in the drawer, pinned to that
/// console rather than to the focused one, without revealing it in Files.
#[test]
fn control_click_on_an_image_path_opens_it_pinned_to_its_console() {
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let path = std::env::temp_dir().join(format!("viewer-image-{:x}", u64::from_ne_bytes(nonce)));
    fs::create_dir(&path).unwrap();
    let _directory = Directory(path.clone());
    let shot = path.join("Screen Shot.PNG");
    ::image::RgbaImage::new(64, 32)
        .save_with_format(&shot, ::image::ImageFormat::Png)
        .unwrap();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    frame(&mut window, &ctx, vec![]);
    frame(&mut window, &ctx, vec![]);
    let names: Vec<_> = window.active().panes().iter().map(|p| p.name).collect();
    assert_eq!(window.active().focus, names[0]);
    window
        .active_mut()
        .pane_called(names[1])
        .unwrap()
        .screen
        .advance(format!("\x1b[2J\x1b[H{}", url::Url::from_file_path(&shot).unwrap()).as_bytes());
    frame(&mut window, &ctx, vec![]);
    let metrics =
        crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let painted = window.active().panes()[1].painted_console.as_ref().unwrap();
    let pos = painted.area.min + egui::vec2(metrics.width() * 5.5, metrics.height() * 0.5);
    assert!(painted.local_at(pos, &metrics, painted.frame).is_some());
    let chord = egui::Modifiers {
        ctrl: true,
        ..Default::default()
    };
    let control_click = |window: &mut crate::Window| {
        for pressed in [true, false] {
            frame(
                window,
                &ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::ModifiersChanged(chord),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: chord,
                    },
                ],
            );
        }
    };
    // A decode that has not finished refuses the click, and the drawer says which file it reads.
    let other = path.join("other.png");
    open_image_target(&mut window, names[0], other.clone()).unwrap();
    let stalled = window.image.stall();
    frame(&mut window, &ctx, vec![]);
    control_click(&mut window);
    assert_eq!(shown_image(&window), Some((names[1], shot.clone())));
    let still = format!("Still reading {}…", other.display());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !frame(&mut window, &ctx, vec![])
        .iter()
        .any(|(text, _)| *text == still)
    {
        assert!(Instant::now() < deadline, "{:?}", window.image.message());
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(stalled);
    control_click(&mut window);
    assert_eq!(
        crate::sidebar::active(&window),
        Some(crate::sidebar::Tool::Document)
    );
    assert_eq!(shown_image(&window), Some((names[1], shot)));
    assert!(!crate::sidebar::files_visible(&window));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !frame(&mut window, &ctx, vec![])
        .iter()
        .any(|(text, _)| text == "Screen Shot.PNG")
        || window.image.message().is_none() && !window.image.loaded()
    {
        assert!(Instant::now() < deadline, "{:?}", window.image.message());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(window.image.message(), None);
}

#[cfg(target_os = "linux")]
#[test]
fn non_repository_console_paints_a_clear_message_after_review_click() {
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let path = std::env::temp_dir().join(format!("viewer-nonrepo-{}", u64::from_ne_bytes(nonce)));
    fs::create_dir(&path).unwrap();
    let _directory = Directory(path.clone());
    let script = format!("cd '{}' && sleep 30", path.display());
    let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
    let name = window.active().panes()[0].name;
    let ctx = egui::Context::default();
    window.chrome.enable();
    let deadline = Instant::now() + Duration::from_secs(5);
    while window
        .pane_called(name)
        .unwrap()
        .pty
        .working_directory()
        .as_ref()
        != Some(&path)
    {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    frame(&mut window, &ctx, vec![]);
    frame(&mut window, &ctx, vec![]);
    click_review(&mut window, &ctx, 0);
    loop {
        let texts = frame(&mut window, &ctx, vec![]);
        assert!(
            !texts
                .iter()
                .any(|(text, _)| text.contains("Git failed:") || text.contains("usage: git"))
        );
        if texts
            .iter()
            .any(|(text, _)| text == "This folder is not a Git repository")
        {
            assert!(window.viewer.loaded.is_none());
            break;
        }
        assert!(Instant::now() < deadline, "{}", window.viewer.message);
        std::thread::sleep(Duration::from_millis(10));
    }
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
