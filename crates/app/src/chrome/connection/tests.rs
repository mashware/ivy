use super::*;

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
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| show(guide, ui.ctx(), &crate::theme::Theme::default()),
        )
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

    for (size, mouse) in [
        (egui::vec2(600.0, 400.0), true),
        (egui::vec2(960.0, 640.0), false),
    ] {
        let ctx = egui::Context::default();
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
        let mut guide = Guide {
            open: true,
            ..Default::default()
        };
        let initial = draw(&ctx, &mut guide, size, vec![]);
        assert!(visible(&initial, "No panels published yet").is_some());
        assert!(visible(&initial, "Reconnect an existing Claude session").is_some());
        assert!(
            !painted(&initial)
                .iter()
                .any(|text| text == "Claude executable" || text == "OpenCode executable"),
            "fresh launch fields must start collapsed"
        );
        initial.drop_without_applying_deltas();
        // Real modal focus order: Close, section, three clients, three executable
        // fields, directory, launch. Focus must scroll controls into the clip.
        for _ in 0..2 {
            key(&ctx, &mut guide, size, egui::Key::Tab).drop_without_applying_deltas();
        }
        let expanded = key(&ctx, &mut guide, size, egui::Key::Space);
        assert!(guide.open, "keyboard expansion closed the dialog");
        assert!(visible(&expanded, "Claude executable").is_some());
        assert!(visible(&expanded, "Open agent console").is_none());
        expanded.drop_without_applying_deltas();
        for _ in 0..3 {
            key(&ctx, &mut guide, size, egui::Key::Tab).drop_without_applying_deltas();
        }
        key(&ctx, &mut guide, size, egui::Key::Space).drop_without_applying_deltas();
        for _ in 0..3 {
            key(&ctx, &mut guide, size, egui::Key::Tab).drop_without_applying_deltas();
        }
        let executable = r"C:\Program Files\OpenCode\opencode.exe";
        draw(
            &ctx,
            &mut guide,
            size,
            vec![egui::Event::Text(executable.into())],
        )
        .drop_without_applying_deltas();
        key(&ctx, &mut guide, size, egui::Key::Tab).drop_without_applying_deltas();
        let directory = r"C:\work\project with spaces";
        draw(
            &ctx,
            &mut guide,
            size,
            vec![egui::Event::Text(directory.into())],
        )
        .drop_without_applying_deltas();
        let bottom = key(&ctx, &mut guide, size, egui::Key::Tab);
        let at = visible(&bottom, "Open agent console")
            .expect("focus did not scroll the launch button into the short dialog");
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
