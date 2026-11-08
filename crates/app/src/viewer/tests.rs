use super::*;

struct Directory(std::path::PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn floating_viewer_stays_inside_the_available_area() {
    let area = egui::Rect::from_min_size(egui::pos2(13.0, 17.0), egui::vec2(1000.0, 600.0));
    let mut state = State::default();
    state.open_markdown();
    assert!(area.contains_rect(state.rect(area)));
    assert_eq!(state.rect(area).width(), 800.0);
    state.width = 10000.0;
    assert!(area.contains_rect(state.rect(area)));
    state.close();
    assert!(state.refresh_at.is_none());
}

#[test]
fn viewers_name_the_frame_work_they_remove() {
    let mut state = State::default();
    let cost = Duration::from_micros(37);
    state.record(cost);
    assert_eq!(state.take_cost(), cost);
    assert_eq!(state.take_cost(), Duration::ZERO);
    state.close_frame();
    state.close_frame();
    let mut report = Vec::new();
    state.publish(&mut report).unwrap();
    assert_eq!(
        String::from_utf8(report).unwrap(),
        "document viewers: frames=1, total=37µs\n"
    );
}

#[test]
fn background_refresh_does_not_move_or_hide_the_markdown() {
    fn positions(shape: &egui::Shape, result: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            egui::Shape::Text(text)
                if matches!(
                    text.galley.text(),
                    "Stable document" | "The preview stays still."
                ) =>
            {
                result.push((text.galley.text().to_owned(), text.pos));
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    positions(shape, result);
                }
            }
            _ => {}
        }
    }
    fn frame(state: &mut State, ctx: &egui::Context) -> Vec<(String, egui::Pos2)> {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                view::show(state, ui, rect, crate::theme::Theme::default());
            },
        );
        let mut found = Vec::new();
        for shape in &output.shapes {
            positions(&shape.shape, &mut found);
        }
        output.textures_delta.clear();
        found
    }
    let ctx = egui::Context::default();
    let mut state = State::default();
    state.open_markdown();
    let text = "# Stable document\n\nThe preview stays still.";
    state.loaded = Some(Loaded {
        text: text.into(),
        markdown: Some(markdown::MarkdownDocument::parse(text).unwrap()),
        diff: None,
    });
    frame(&mut state, &ctx);
    let before = frame(&mut state, &ctx);
    assert_eq!(before.len(), 2);
    let path = PathBuf::from("unused-refresh-result.md");
    state.job = Some((
        path.clone(),
        Kind::Markdown,
        Job::start(path, Kind::Markdown, ctx.clone()).unwrap(),
    ));
    let loading = frame(&mut state, &ctx);
    assert_eq!(
        loading, before,
        "background loading moved the existing document"
    );
    state.job = None;
    assert_eq!(frame(&mut state, &ctx), before);
}

#[cfg(unix)]
#[test]
fn viewer_typing_and_dismissal_never_reach_the_program() {
    use crate::tests::{one_frame_in, window_of_tabs};
    for markdown in [true, false] {
        let received = term_core::received::Received::new("viewer-input");
        let script = received.script("stty raw -echo");
        let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        received.wait_until_ready(Duration::from_secs(5));
        let names: Vec<_> = window.all_panes().map(|p| p.name).collect();
        if markdown {
            window.viewer.open_markdown();
        } else {
            window.viewer.open_diff();
        }
        one_frame_in(&mut window, &ctx, area, vec![]);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::Text("private path".into()),
                egui::Event::Paste("private paste".into()),
            ],
        );
        assert!(received.so_far().is_empty());
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Text("closing leak".into()),
            ],
        );
        assert!(!window.viewer.open);
        assert_eq!(
            window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
            names
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("SAFE".into())],
        );
        assert_eq!(received.wait_for(4, Duration::from_secs(5)), b"SAFE");
        window.viewer.close_frame();
        let mut report = Vec::new();
        window.viewer.publish(&mut report).unwrap();
        assert!(String::from_utf8(report).unwrap().contains("frames=1,"));
        assert!(window.timed_separately > Duration::ZERO);
    }
}

#[test]
fn markdown_refresh_survives_atomic_save_and_obsolete_request() {
    use std::fs;
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "viewer-refresh-{}-{}",
        std::process::id(),
        u64::from_ne_bytes(nonce)
    ));
    fs::create_dir(&dir).unwrap();
    let _directory = Directory(dir.clone());
    let file = dir.join("plan.md");
    fs::write(&file, "# First").unwrap();
    let ctx = egui::Context::default();
    let mut state = State::default();
    state.open_markdown();
    state.path = file.to_string_lossy().into_owned();
    state.start(&ctx);
    let await_text = |state: &mut State, expected: &str| {
        let end = Instant::now() + Duration::from_secs(5);
        while state
            .loaded
            .as_ref()
            .is_none_or(|loaded| loaded.text != expected)
        {
            assert!(
                Instant::now() < end,
                "expected {expected}, error {}",
                state.message
            );
            state.poll(&ctx);
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    await_text(&mut state, "# First");
    assert!(state.refresh_at.is_some());
    let replacement = dir.join("replacement.md");
    fs::write(&replacement, "# Saved atomically").unwrap();
    fs::rename(&replacement, &file).unwrap();
    await_text(&mut state, "# Saved atomically");
    // Switching paths while a job exists must never publish that job into the new view.
    state.refresh_at = Some(Instant::now());
    state.poll(&ctx);
    let other = dir.join("other.md");
    fs::write(&other, "# Other file").unwrap();
    state.path = other.to_string_lossy().into_owned();
    state.start(&ctx);
    await_text(&mut state, "# Other file");
    state.close();
    assert!(state.refresh_at.is_none());
    fs::write(&other, "# Changed while closed").unwrap();
    state.open_markdown();
    await_text(&mut state, "# Changed while closed");
    fs::write(&other, "# Changed after reopen").unwrap();
    await_text(&mut state, "# Changed after reopen");
    state.close();
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn dragging_viewer_divider_keeps_the_gesture_out_of_the_consoles() {
    use crate::tests::{one_frame_in, window_of_tabs};

    let received = term_core::received::Received::new("viewer-divider");
    let script = received.script("stty raw -echo && printf '\\033[?1000h\\033[?1006h'");
    let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
    received.wait_until_ready(Duration::from_secs(5));
    let names: Vec<_> = window.all_panes().map(|p| p.name).collect();
    window.viewer.open_markdown();
    one_frame_in(&mut window, &ctx, area, vec![]);

    let metrics =
        crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let before = crate::tab_bar::geometry(&window, area, &metrics).consoles;
    let from = window.viewer.rect(before).left_center();
    let to = egui::pos2(from.x - 200.0, from.y);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    one_frame_in(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    one_frame_in(&mut window, &ctx, area, vec![egui::Event::PointerMoved(to)]);
    one_frame_in(&mut window, &ctx, area, vec![button(to, false)]);

    let after = crate::tab_bar::geometry(&window, area, &metrics).consoles;
    assert!(
        (window.viewer.rect(after).left() - to.x).abs() < 10.0,
        "divider did not follow the drag: {after:?}"
    );
    assert_eq!(after, before, "floating resize changed console geometry");
    assert_eq!(
        window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
        names
    );
    assert!(
        received.so_far().is_empty(),
        "divider gesture reached the program"
    );
}

#[test]
fn refreshing_local_changes_replaces_untracked_content() {
    use std::{fs, process::Command};
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "viewer-untracked-{}-{}",
        std::process::id(),
        u64::from_ne_bytes(nonce)
    ));
    fs::create_dir(&dir).unwrap();
    let _directory = Directory(dir.clone());
    let git = |args: &[&str]| {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        )
    };
    git(&["init", "-q"]);
    git(&["config", "user.name", "Test"]);
    git(&["config", "user.email", "test@example.invalid"]);
    fs::write(dir.join("tracked"), "base\n").unwrap();
    git(&["add", "tracked"]);
    git(&["commit", "-qm", "base"]);
    let file = dir.join("new.txt");
    fs::write(&file, "first\n").unwrap();
    let ctx = egui::Context::default();
    let mut state = State::default();
    state.open_diff();
    state.path = dir.to_string_lossy().into_owned();
    state.start(&ctx);
    let wait = |state: &mut State, expected: &str| {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            state.poll(&ctx);
            let matches = state
                .loaded
                .as_ref()
                .and_then(|l| l.diff.as_ref())
                .is_some_and(|d| {
                    d.files.iter().any(|f| {
                        f.path == std::path::Path::new("new.txt")
                            && f.hunks
                                .iter()
                                .any(|h| h.lines.iter().any(|l| l.text == expected))
                    })
                });
            if matches {
                break;
            }
            assert!(
                Instant::now() < end,
                "missing {expected}: {}",
                state.message
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    wait(&mut state, "first");
    fs::write(&file, "second\n").unwrap();
    state.refresh_at = Some(Instant::now());
    wait(&mut state, "second");
}

#[test]
fn repeated_request_invalidates_the_running_generation() {
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "viewer-generation-{}-{}",
        std::process::id(),
        u64::from_ne_bytes(nonce)
    ));
    std::fs::create_dir(&dir).unwrap();
    let _directory = Directory(dir.clone());
    let file = dir.join("test.md");
    std::fs::write(&file, "first").unwrap();
    let ctx = egui::Context::default();
    let mut state = State::default();
    state.open_markdown();
    state.path = file.to_string_lossy().into_owned();
    state.start(&ctx);
    assert!(state.job.is_some());
    let prior = state.job_generation;
    std::fs::write(&file, "second").unwrap();
    state.start(&ctx);
    assert_ne!(state.generation, prior);
    let end = Instant::now() + Duration::from_secs(5);
    while state
        .loaded
        .as_ref()
        .is_none_or(|loaded| loaded.text != "second")
    {
        state.poll(&ctx);
        assert!(
            Instant::now() < end,
            "new generation did not load: {}",
            state.message
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(state.job_generation, state.generation);
}
