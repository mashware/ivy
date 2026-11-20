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
    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
        recovery::show(recovery::Status::default(), true, &mut None, ui);
    });
    let texts = painted(&output);
    assert!(texts.iter().any(|text| text == translocation::NOTICE));
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
