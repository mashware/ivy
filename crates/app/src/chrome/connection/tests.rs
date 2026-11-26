#![cfg(test)]

use super::*;

#[cfg(windows)]
#[test]
fn fresh_launch_mouse_recipe_submits_literal_selection() {
    fn draw(
        ctx: &egui::Context,
        guide: &mut Guide,
        size: egui::Vec2,
        time: &mut f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        *time += 1.0 / 60.0;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                time: Some(*time),
                events,
                ..Default::default()
            },
            |ui| show(guide, ui.ctx(), &crate::theme::Theme::default()),
        );
        output.textures_delta.clear();
        output
    }
    fn settle(
        ctx: &egui::Context,
        guide: &mut Guide,
        size: egui::Vec2,
        time: &mut f64,
        frames: usize,
    ) -> egui::FullOutput {
        for _ in 1..frames {
            draw(ctx, guide, size, time, vec![]).drop_without_applying_deltas();
        }
        draw(ctx, guide, size, time, vec![])
    }
    fn visible_rect(output: &egui::FullOutput, label: &str) -> Option<Rect> {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let rect = Rect::from_min_size(text.pos, text.galley.size());
                shape
                    .clip_rect
                    .contains(rect.center())
                    .then(|| rect.intersect(shape.clip_rect))
            }
            _ => None,
        })
    }
    let recipe: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packaging/windows/installed-proof/launcher-mouse.json"
    )))
    .unwrap();
    let viewport = recipe["viewport"].as_array().unwrap();
    let size = egui::vec2(
        viewport[0].as_f64().unwrap() as f32,
        viewport[1].as_f64().unwrap() as f32,
    );
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let mut guide = Guide {
        open: true,
        ..Default::default()
    };
    let mut time = 0.0;
    let mut output = settle(&ctx, &mut guide, size, &mut time, 24);
    let executable = r"C:\Program Files\OpenCode installation with spaces\opencode.exe";
    let directory = r"C:\work\project with spaces\long directory to exercise wrapped previews\another directory with spaces\final project";
    for action in recipe["actions"].as_array().unwrap() {
        let target = action["target"].as_str().unwrap();
        let field = target.ends_with("executable") || target == "Working directory";
        match action["kind"].as_str().unwrap() {
            "scroll" => {
                let at = egui::pos2(
                    action["at"][0].as_f64().unwrap() as f32,
                    action["at"][1].as_f64().unwrap() as f32,
                );
                draw(
                    &ctx,
                    &mut guide,
                    size,
                    &mut time,
                    vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Line,
                            phase: egui::TouchPhase::Move,
                            delta: egui::vec2(0.0, action["lines"].as_f64().unwrap() as f32),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                )
                .drop_without_applying_deltas();
                output = settle(&ctx, &mut guide, size, &mut time, 24);
                assert!(
                    visible_rect(&output, target).is_some(),
                    "native launcher mouse recipe: scroll must reveal {target}"
                );
            }
            "click" => {
                let at = egui::pos2(
                    action["at"][0].as_f64().unwrap() as f32,
                    action["at"][1].as_f64().unwrap() as f32,
                );
                let label = visible_rect(&output, target);
                assert!(
                    label.is_some_and(|rect| field || rect.contains(at)),
                    "native launcher mouse recipe: click must hit visible {target} at {at:?}"
                );
                for pressed in [true, false] {
                    draw(
                        &ctx,
                        &mut guide,
                        size,
                        &mut time,
                        vec![
                            egui::Event::PointerMoved(at),
                            egui::Event::PointerButton {
                                pos: at,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    )
                    .drop_without_applying_deltas();
                }
                output = settle(&ctx, &mut guide, size, &mut time, 24);
                if field {
                    let focused = ctx
                        .memory(|memory| memory.focused())
                        .and_then(|id| ctx.read_response(id));
                    assert!(
                        focused.is_some_and(|response| response.rect.contains(at)),
                        "native launcher mouse recipe: click must focus {target}"
                    );
                }
            }
            "text" => {
                assert!(
                    visible_rect(&output, target).is_some(),
                    "native launcher mouse recipe: {target} must be visible before typing"
                );
                let value = match action["value"].as_str().unwrap() {
                    "executable" => executable,
                    "directory" => directory,
                    value => panic!("native launcher mouse recipe: unknown text value {value}"),
                };
                let event = |pressed| egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::CTRL,
                };
                draw(
                    &ctx,
                    &mut guide,
                    size,
                    &mut time,
                    vec![
                        egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                        event(true),
                        event(false),
                    ],
                )
                .drop_without_applying_deltas();
                draw(
                    &ctx,
                    &mut guide,
                    size,
                    &mut time,
                    vec![
                        egui::Event::ModifiersChanged(egui::Modifiers::NONE),
                        egui::Event::Text(value.into()),
                    ],
                )
                .drop_without_applying_deltas();
                output = settle(&ctx, &mut guide, size, &mut time, 18);
            }
            kind => panic!("native launcher mouse recipe: unknown action {kind}"),
        }
        assert!(
            guide.open,
            "native launcher mouse recipe: action closed the guide"
        );
    }
    output.drop_without_applying_deltas();
    let request = guide
        .fresh
        .request
        .take()
        .expect("native launcher mouse recipe: controls must submit the literal selection");
    assert_eq!(
        request.client,
        crate::launch::fresh::Client::OpenCode,
        "native launcher mouse recipe: client selection"
    );
    assert_eq!(
        request.images,
        [String::new(), String::new(), executable.into()],
        "native launcher mouse recipe: executable text"
    );
    assert_eq!(
        request.directory, directory,
        "native launcher mouse recipe: directory text"
    );
}

fn frame(ctx: &egui::Context, guide: &mut Guide, events: Vec<egui::Event>) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 1200.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            body(guide, ui);
        },
    );
    output.textures_delta.clear();
    output
}

fn center(output: &egui::FullOutput, text: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(label) if label.galley.text() == text => {
                Some(label.pos + label.galley.size() / 2.0)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{text} not painted"))
}

fn click(ctx: &egui::Context, guide: &mut Guide, at: egui::Pos2) -> egui::FullOutput {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(
        ctx,
        guide,
        vec![egui::Event::PointerMoved(at), button(true)],
    );
    frame(ctx, guide, vec![button(false)])
}

#[test]
fn an_opening_failure_is_visible_without_expanding_activity_controls() {
    let ctx = egui::Context::default();
    let mut guide = Guide {
        message: "The agent console could not be opened.".into(),
        ..Default::default()
    };
    let initial = frame(&ctx, &mut guide, vec![]);
    center(&initial, "The agent console could not be opened.");
    assert!(!painted(&initial).iter().any(|text| text == "Codex 0.167.0"));
}

#[test]
fn activity_controls_require_opening_and_explicit_save() {
    let ctx = egui::Context::default();
    let mut guide = Guide::default();
    let initial = frame(&ctx, &mut guide, vec![]);
    assert!(
        !initial.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Codex 0.167.0")
        )
    );
    let at = center(&initial, "Agent activity and input signals");
    click(&ctx, &mut guide, at);
    for _ in 0..30 {
        frame(&ctx, &mut guide, vec![]);
    }
    let opened = frame(&ctx, &mut guide, vec![]);
    let codex = center(&opened, "Codex 0.167.0");
    click(&ctx, &mut guide, codex);
    assert!(guide.desired.codex);
    assert!(!guide.save_preferences);
    let rendered = frame(&ctx, &mut guide, vec![]);
    let save = center(&rendered, "Save agent preferences");
    click(&ctx, &mut guide, save);
    assert!(guide.save_preferences);
}

#[test]
fn saved_preferences_show_completion_after_the_application_finishes() {
    let ctx = egui::Context::default();
    let mut guide = Guide {
        message: APPLYING_PREFERENCES.into(),
        ..Default::default()
    };
    let initial = frame(&ctx, &mut guide, vec![]);
    let at = center(&initial, "Agent activity and input signals");
    click(&ctx, &mut guide, at);
    for _ in 0..30 {
        frame(&ctx, &mut guide, vec![]);
    }
    guide.application_status(true);
    let pending = frame(&ctx, &mut guide, vec![]);
    center(&pending, APPLYING_PREFERENCES);
    guide.application_status(false);
    let applied = frame(&ctx, &mut guide, vec![]);
    center(&applied, "Saved and applied.");
    assert!(!applied.shapes.iter().any(
        |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == APPLYING_PREFERENCES)
    ));
    guide.message = "Save failed".into();
    guide.application_status(false);
    center(&frame(&ctx, &mut guide, vec![]), "Save failed");
}

#[test]
fn initial_help_explains_saved_setup_and_discloses_first_time_installation_only_on_request() {
    let ctx = egui::Context::default();
    let mut guide = Guide::default();
    let initial = frame(&ctx, &mut guide, vec![]);
    center(
        &initial,
        "Set up Ivy once per AI client. Ivy updates normally keep your saved MCP registration.",
    );
    center(
        &initial,
        "An empty panel only means no status has been published.",
    );
    center(
        &initial,
        "If Ivy is absent from this running session, restart Claude in this same Ivy console and resume your conversation. You do not need to close the terminal.",
    );
    assert!(!initial.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Copy first-time setup prompt")));
    for text in ["Recover AI sessions", "Copy recovery setup instructions"] {
        assert!(
            !initial
                .shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == text))
        );
    }
    let at = center(&initial, "Manual setup and diagnostics");
    click(&ctx, &mut guide, at);
    let manual = frame(&ctx, &mut guide, vec![]);
    center(&manual, "First-time setup");
    center(&manual, "Copy first-time setup prompt");
    for text in ["Recover AI sessions", "Copy recovery setup instructions"] {
        assert!(
            !manual
                .shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == text))
        );
    }
}

const TRANSLOCATED: &str =
    "/private/var/folders/rq/x/T/AppTranslocation/78459825-AB/d/Ivy.app/Contents/MacOS/ivy";

fn painted(output: &egui::FullOutput) -> Vec<String> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(label) => Some(label.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_translocated_mac_launch_offers_no_bridge_path() {
    let bridge = Bridge::beside(Some(TRANSLOCATED.into()), true);
    assert!(bridge.translocated);
    assert_eq!(bridge.path, None);
    // The same path read on another platform is not Gatekeeper's, and keeps the old behaviour.
    let elsewhere = Bridge::beside(Some(TRANSLOCATED.into()), false);
    assert!(!elsewhere.translocated);
    assert!(elsewhere.path.is_some());
    let installed = Bridge::beside(
        Some("/Applications/Ivy.app/Contents/MacOS/ivy".into()),
        true,
    );
    assert!(!installed.translocated);
    assert_eq!(
        installed.path.as_deref(),
        Some(
            std::path::Path::new("/Applications/Ivy.app/Contents/MacOS")
                .join(if cfg!(windows) {
                    "panel-mcp.exe"
                } else {
                    "panel-mcp"
                })
                .as_path()
        )
    );
}

#[test]
fn a_translocated_recovery_section_offers_no_path_either() {
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        recovery::show(recovery::Status::default(), true, &mut None, ui);
    });
    output.textures_delta.clear();
    let texts = painted(&output);
    assert!(
        texts
            .iter()
            .any(|text| text == translocation::NO_RECOVERY_PATHS)
    );
    for absent in ["Copy recovery helper path", "Copy recovery adapters path"] {
        assert!(!texts.iter().any(|text| text == absent), "{absent} offered");
    }
}

#[test]
fn a_translocated_guide_says_how_to_fix_it_and_shows_no_path_to_copy() {
    let ctx = egui::Context::default();
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(BRIDGE_KEY),
            Bridge::beside(Some(TRANSLOCATED.into()), true),
        )
    });
    let mut guide = Guide::default();
    let initial = frame(&ctx, &mut guide, vec![]);
    let at = center(&initial, "Manual setup and diagnostics");
    click(&ctx, &mut guide, at);
    let manual = frame(&ctx, &mut guide, vec![]);
    let texts = painted(&manual);
    assert_eq!(
        texts
            .iter()
            .filter(|text| *text == translocation::NOTICE)
            .count(),
        1,
        "the notice is painted once"
    );
    for absent in [
        "Copy first-time setup prompt",
        "Copy bridge path",
        "Check bridge again",
    ] {
        assert!(!texts.iter().any(|text| text == absent), "{absent} offered");
    }
    assert!(
        !texts.iter().any(|text| text.contains("AppTranslocation")),
        "a translocated path is painted"
    );
}

#[test]
fn the_last_step_asks_the_person_to_check_the_client_themselves() {
    let ctx = egui::Context::default();
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new(BRIDGE_KEY),
            Bridge::beside(
                Some("/Applications/Ivy.app/Contents/MacOS/ivy".into()),
                true,
            ),
        )
    });
    let mut guide = Guide::default();
    let initial = frame(&ctx, &mut guide, vec![]);
    let at = center(&initial, "Manual setup and diagnostics");
    click(&ctx, &mut guide, at);
    let manual = frame(&ctx, &mut guide, vec![]);
    assert!(
        center(&manual, SELF_CHECK).y > center(&manual, "3. Start the agent in an Ivy console").y,
        "the self-check is not part of the last step"
    );
    let prompt = setup_prompt(&Bridge::beside(
        Some("/Applications/Ivy.app/Contents/MacOS/ivy".into()),
        true,
    ));
    assert!(prompt.contains("ask me to open this client's MCP server list myself"));
}

#[cfg(windows)]
#[test]
fn fresh_launch_fields_and_button_are_reachable_in_a_short_modal() {
    fn draw(
        ctx: &egui::Context,
        guide: &mut Guide,
        size: egui::Vec2,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| show(guide, ui.ctx(), &crate::theme::Theme::default()),
        );
        output.textures_delta.clear();
        output
    }
    fn key(
        ctx: &egui::Context,
        guide: &mut Guide,
        size: egui::Vec2,
        key: egui::Key,
    ) -> egui::FullOutput {
        let event = |pressed| egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        draw(ctx, guide, size, vec![event(true)]).drop_without_applying_deltas();
        draw(ctx, guide, size, vec![event(false)])
    }
    fn visible(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let center = text.pos + text.galley.size() / 2.0;
                shape.clip_rect.contains(center).then_some(center)
            }
            _ => None,
        })
    }
    fn requested_paint(
        ctx: &egui::Context,
        guide: &mut Guide,
        size: egui::Vec2,
        focused: egui::FullOutput,
    ) -> egui::FullOutput {
        assert_eq!(
            focused
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .map(|viewport| viewport.repaint_delay),
            Some(std::time::Duration::ZERO),
            "native launcher keyboard workflow: focus must request its scroll repaint"
        );
        focused.drop_without_applying_deltas();
        draw(ctx, guide, size, vec![])
    }

    for (size, mouse) in [
        (egui::vec2(600.0, 400.0), true),
        (egui::vec2(960.0, 640.0), false),
    ] {
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        ctx.all_styles_mut(|style| {
            style.animation_time = 0.0;
            style.scroll_animation = egui::style::ScrollAnimation::none();
        });
        let mut guide = Guide {
            open: true,
            ..Default::default()
        };
        // A new egui Area measures itself invisibly before its first painted frame.
        draw(&ctx, &mut guide, size, vec![]).drop_without_applying_deltas();
        let initial = draw(&ctx, &mut guide, size, vec![]);
        assert!(
            !painted(&initial)
                .iter()
                .any(|text| text == "Claude executable" || text == "OpenCode executable"),
            "native launcher keyboard workflow: fresh launch fields must start collapsed"
        );
        assert!(visible(&initial, "No panels published yet").is_some());
        assert!(visible(&initial, "Reconnect an existing Claude session").is_some());
        initial.drop_without_applying_deltas();
        // Traverse the existing Close, reconnect-copy and manual controls before
        // entering the optional launcher, without changing their focus order.
        for _ in 0..3 {
            key(&ctx, &mut guide, size, egui::Key::Tab).drop_without_applying_deltas();
        }
        let focused = key(&ctx, &mut guide, size, egui::Key::Tab);
        let header = requested_paint(&ctx, &mut guide, size, focused);
        assert!(
            visible(&header, "Open a new agent console").is_some(),
            "native launcher keyboard workflow: focused header must be visible"
        );
        header.drop_without_applying_deltas();
        let expanded = key(&ctx, &mut guide, size, egui::Key::Space);
        assert!(
            guide.open,
            "native launcher keyboard workflow: expansion closed the dialog"
        );
        expanded.drop_without_applying_deltas();
        let focused = key(&ctx, &mut guide, size, egui::Key::Tab);
        let first = requested_paint(&ctx, &mut guide, size, focused);
        assert!(
            visible(&first, "Claude").is_some(),
            "native launcher keyboard workflow: first activation must reveal the Claude selector"
        );
        first.drop_without_applying_deltas();
        for _ in 0..2 {
            key(&ctx, &mut guide, size, egui::Key::Tab).drop_without_applying_deltas();
        }
        key(&ctx, &mut guide, size, egui::Key::Space).drop_without_applying_deltas();
        for _ in 0..2 {
            key(&ctx, &mut guide, size, egui::Key::Tab).drop_without_applying_deltas();
        }
        let focused = key(&ctx, &mut guide, size, egui::Key::Tab);
        let field = requested_paint(&ctx, &mut guide, size, focused);
        assert!(
            visible(&field, "OpenCode executable").is_some(),
            "native launcher keyboard workflow: focused executable field must be visible"
        );
        field.drop_without_applying_deltas();
        let executable = r"C:\Program Files\OpenCode\opencode.exe";
        draw(
            &ctx,
            &mut guide,
            size,
            vec![egui::Event::Text(executable.into())],
        )
        .drop_without_applying_deltas();
        let focused = key(&ctx, &mut guide, size, egui::Key::Tab);
        let field = requested_paint(&ctx, &mut guide, size, focused);
        assert!(
            visible(&field, "Working directory").is_some(),
            "native launcher keyboard workflow: focused directory field must be visible"
        );
        field.drop_without_applying_deltas();
        let directory = r"C:\work\project with spaces";
        draw(
            &ctx,
            &mut guide,
            size,
            vec![egui::Event::Text(directory.into())],
        )
        .drop_without_applying_deltas();
        let focused = key(&ctx, &mut guide, size, egui::Key::Tab);
        let bottom = requested_paint(&ctx, &mut guide, size, focused);
        let at = visible(&bottom, "Open agent console")
            .expect("native launcher keyboard workflow: focused launch button must be visible");
        bottom.drop_without_applying_deltas();
        assert!(guide.fresh.request.is_none());
        if mouse {
            for pressed in [true, false] {
                draw(
                    &ctx,
                    &mut guide,
                    size,
                    vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                )
                .drop_without_applying_deltas();
            }
        } else {
            key(&ctx, &mut guide, size, egui::Key::Enter).drop_without_applying_deltas();
        }
        let request = guide
            .fresh
            .request
            .take()
            .expect("fresh launch controls must submit the selected program");
        assert_eq!(request.client, crate::launch::fresh::Client::OpenCode);
        assert_eq!(
            request.images,
            [String::new(), String::new(), executable.into()]
        );
        assert_eq!(request.directory, directory);
    }
}

#[cfg(windows)]
#[test]
fn fresh_console_opening_is_subtracted_once_from_the_frame() {
    use std::time::Duration;

    let ctx = egui::Context::default();
    let missing = std::env::temp_dir().join(format!(
        "ivy-accounting-no-shell-{}.exe",
        std::process::id()
    ));
    assert!(!missing.exists());
    crate::launch::TEST_SPAWN_PROGRAM
        .with(|program| *program.borrow_mut() = Some(missing.to_string_lossy().into_owned()));
    let opened = Window::opening(
        &ctx,
        crate::OpenWith::Factory {
            tabs: 1,
            layout: crate::OpeningLayout {
                columns: 1,
                rows: 1,
            },
        },
        "unused-factory-program",
        None,
        crate::session::Persistence::at(None),
    );
    crate::launch::TEST_SPAWN_PROGRAM.with(|program| program.borrow_mut().take());
    let mut window = opened.unwrap();
    window.chrome.enable();
    window.chrome.guide.open = true;
    window.chrome.guide.fresh.request = Some(Default::default());
    window.timed_separately = Duration::ZERO;
    window.mirror_this_frame = Duration::ZERO;
    window.chrome.frame = Duration::ZERO;
    let _ = window.chrome.take_cost();
    let _ = window.settings_editor.take_cost();
    let settings_before = window.settings_editor.recorded_frame_cost();
    let frame = eframe::Frame::_new_kittest();
    ctx.run_ui(Default::default(), |ui| {
        begin_pass(&mut window, ui, &frame);
    })
    .drop_without_applying_deltas();
    assert_eq!(
        window.tabs.len(),
        2,
        "the queued fresh console was not opened"
    );
    let chrome_cost = window.chrome.frame;
    assert!(!chrome_cost.is_zero());
    let settings_cost = window
        .settings_editor
        .recorded_frame_cost()
        .checked_sub(settings_before)
        .expect("named settings spans reset within the measured pass");
    window.active_mut().panes_mut()[0]
        .timings
        .record_round(crate::measure::Spans::of_a_round(
            Some(Instant::now()),
            Instant::now(),
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
        ));
    let mirror = window.mirror_this_frame;
    let framework = Duration::from_millis(7);
    // Deriving this total from timed_separately would incorporate the duplicate being tested.
    window.close_the_frame(chrome_cost + settings_cost + mirror + framework);
    assert_eq!(
        window.rest_of_frame.mean(),
        framework,
        "fresh console opening was subtracted twice from the frame"
    );
}

#[test]
fn connection_scroll_paints_its_target_in_the_wheel_frame() {
    fn draw(
        ctx: &egui::Context,
        window: &mut Window,
        time: &mut f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        *time += 1.0 / 60.0;
        let mut frame = eframe::Frame::_new_kittest();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 640.0),
                )),
                time: Some(*time),
                events,
                ..Default::default()
            },
            |ui| eframe::App::ui(window, ui, &mut frame),
        );
        output.textures_delta.clear();
        output
    }
    fn visible(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let center = text.pos + text.galley.size() / 2.0;
                shape.clip_rect.contains(center).then_some(center)
            }
            _ => None,
        })
    }
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    crate::chrome::prepare(&ctx);
    let mut window = Window::with_tabs(&ctx, 1, crate::OpeningLayout::DEFAULT, None).unwrap();
    window.chrome.enable();
    window.headers = window.headers.integrated(true);
    open(&mut window, &ctx, false);
    let mut time = 0.0;
    for _ in 0..24 {
        draw(&ctx, &mut window, &mut time, vec![]).drop_without_applying_deltas();
    }
    let target = "Manual setup and diagnostics";
    let initial = draw(&ctx, &mut window, &mut time, vec![]);
    assert!(
        visible(&initial, target).is_none(),
        "Connect scroll fixture must initially clip the target"
    );
    initial.drop_without_applying_deltas();
    let scrolled = draw(
        &ctx,
        &mut window,
        &mut time,
        vec![
            egui::Event::PointerMoved(egui::pos2(480.0, 400.0)),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                phase: egui::TouchPhase::Move,
                delta: egui::vec2(0.0, -100.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(
        visible(&scrolled, target).is_some(),
        "Connect scrolling must paint the visible target in the wheel frame"
    );
    let at = visible(&scrolled, target).unwrap();
    scrolled.drop_without_applying_deltas();
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let clicked = draw(
        &ctx,
        &mut window,
        &mut time,
        vec![egui::Event::PointerMoved(at), button(true), button(false)],
    );
    assert!(
        window.chrome.guide.manual_open,
        "the next pointer click must open the visible Manual setup control"
    );
    clicked.drop_without_applying_deltas();
    draw(&ctx, &mut window, &mut time, vec![]).drop_without_applying_deltas();
    let passes = ctx.cumulative_pass_nr();
    draw(&ctx, &mut window, &mut time, vec![]).drop_without_applying_deltas();
    assert_eq!(
        ctx.cumulative_pass_nr(),
        passes + 1,
        "an unchanged Connect scroll offset must keep an idle frame to one pass"
    );
}
