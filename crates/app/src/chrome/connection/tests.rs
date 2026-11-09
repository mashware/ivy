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
