//! Headless interaction witnesses; desktop appearance remains a separate check.
use super::*;
use crate::settings::document::Document;

fn text_shapes(shape: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::epaint::Shape::Text(text) => out.push((
            text.galley.text().to_owned(),
            text.galley.rect.translate(text.pos.to_vec2()),
        )),
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                text_shapes(shape, out);
            }
        }
        _ => {}
    }
}
fn paint(
    editor: &mut Editor,
    ctx: &egui::Context,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    let mut fonts = crate::fonts::Discoverer::default();
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| show(editor, ui.ctx(), &mut fonts),
    );
    let mut text = Vec::new();
    for shape in &output.shapes {
        text_shapes(&shape.shape, &mut text);
    }
    output.drop_without_applying_deltas();
    text
}
fn click(pos: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
#[test]
fn opening_appearance_keeps_every_override_inherited_and_fonts_lazy() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    for label in ["Gruvbox Dark", "Tokyo Night", "Built-in theme (inherit)"] {
        assert!(text.iter().any(|(s, _)| s == label), "missing {label}");
    }
    assert!(
        !text.iter().any(|(s, _)| s == "15"),
        "ANSI palette must start collapsed"
    );
    assert!(
        editor
            .draft
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .all(|f| f.value.is_empty())
    );
    assert!(editor.fonts.is_none());
    assert_eq!(editor.font_requests, 0);
}
#[test]
fn narrow_short_dialog_keeps_footer_inside_the_window() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(460.0, 430.0);
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    for label in ["Restore defaults", "Cancel", "Save changes"] {
        let (_, rect) = text
            .iter()
            .find(|(s, _)| s == label)
            .unwrap_or_else(|| panic!("missing {label}: {text:?}"));
        assert!(
            rect.min.x >= 0.0 && rect.max.x <= size.x && rect.min.y >= 0.0 && rect.max.y <= size.y,
            "{label}: {rect:?}"
        );
    }
}
#[test]
fn clicking_theme_sample_changes_only_the_draft() {
    let mut editor = Editor {
        draft: Some(Document::new("# preserved\n".into())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    let center = text
        .iter()
        .find(|(s, _)| s == "Tokyo Night")
        .unwrap()
        .1
        .center();
    paint(&mut editor, &ctx, size, click(center));
    let text = editor.draft.as_ref().unwrap().text();
    assert!(text.contains("theme = tokyonight"));
    assert!(text.starts_with("# preserved\n"));
    assert!(editor.store.is_none());
    assert!(editor.message.is_empty());
}

#[test]
fn wide_dialog_keeps_heading_and_footer_inside_the_window_across_passes() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(800.0, 600.0);
    for pass in 0..6 {
        let text = paint(&mut editor, &ctx, size, vec![]);
        if pass == 0 {
            continue;
        }
        for label in [
            "Settings",
            "Appearance",
            "Save changes",
            "Cancel",
            "Restore defaults",
        ] {
            let matching: Vec<_> = text.iter().filter(|(s, _)| s == label).collect();
            assert!(!matching.is_empty(), "pass {pass}: missing {label}");
            for (_, rect) in matching {
                assert!(
                    rect.min.x >= 0.0
                        && rect.max.x <= size.x
                        && rect.min.y >= 0.0
                        && rect.max.y <= size.y,
                    "pass {pass}, {label}: {rect:?}"
                );
            }
        }
    }
}
