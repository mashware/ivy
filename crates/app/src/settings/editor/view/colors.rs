//! Colour controls preserve inheritance until a value is actually edited.
use super::field_errors;
use crate::{
    settings::document::{Document, Field},
    theme::Theme,
};
use eframe::egui;

const BASE: [(&str, &str); 6] = [
    ("background", "Background"),
    ("foreground", "Text"),
    ("cursor", "Cursor"),
    ("divider", "Divider"),
    ("focus", "Focus"),
    ("panel", "Panel"),
];

pub(super) fn show(draft: &mut Document, ui: &mut egui::Ui, errors: &[(String, String)]) {
    let theme_name = draft
        .fields
        .iter()
        .find(|f| f.key == "theme")
        .unwrap()
        .value
        .as_str();
    let inherited = crate::theme::NamedTheme::from_written(theme_name)
        .map(|t| t.theme())
        .unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        ui.strong("Terminal colours");
        if ui.small_button("Use theme colours").clicked() {
            for field in &mut draft.fields {
                if BASE.iter().any(|(key, _)| *key == field.key) || field.key.starts_with("color.")
                {
                    field.value.clear();
                }
            }
        }
    });
    ui.weak("Swatches inherit the theme. Edit a colour to override it.");
    let columns = if ui.available_width() >= 450.0 { 6 } else { 3 };
    egui::Grid::new("settings-base-colours")
        .num_columns(columns)
        .spacing(egui::vec2(12.0, 10.0))
        .show(ui, |ui| {
            for (index, (key, title)) in BASE.iter().enumerate() {
                let field = draft.fields.iter_mut().find(|f| f.key == *key).unwrap();
                swatch(ui, field, title, inherited_color(&inherited, key), errors);
                if (index + 1).is_multiple_of(columns) {
                    ui.end_row();
                }
            }
        });
    ui.add_space(8.0);
    egui::CollapsingHeader::new("ANSI palette: 16 colours")
        .id_salt("settings-ansi")
        .default_open(false)
        .show(ui, |ui| {
            let columns = if ui.available_width() >= 580.0 { 8 } else { 4 };
            egui::Grid::new("settings-ansi-colours")
                .num_columns(columns)
                .spacing(egui::vec2(12.0, 10.0))
                .show(ui, |ui| {
                    for index in 0..16 {
                        let key = format!("color.{index}");
                        let field = draft.fields.iter_mut().find(|f| f.key == key).unwrap();
                        swatch(
                            ui,
                            field,
                            &index.to_string(),
                            inherited.color(term_core::TermColor::Indexed(index)),
                            errors,
                        );
                        if (index as usize + 1).is_multiple_of(columns) {
                            ui.end_row();
                        }
                    }
                });
        });
}

fn swatch(
    ui: &mut egui::Ui,
    field: &mut Field,
    title: &str,
    inherited: egui::Color32,
    errors: &[(String, String)],
) {
    ui.push_id(&field.key, |ui| {
        ui.vertical(|ui| {
            ui.set_width(60.0);
            let color = parsed(&field.value).unwrap_or(inherited);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(56.0, 29.0), egui::Sense::click());
            ui.painter().rect_filled(rect, 5.0, color);
            ui.painter().rect_stroke(
                rect,
                5.0,
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                egui::StrokeKind::Inside,
            );
            let response = response.on_hover_text(format!(
                "{title}: {}: click to edit",
                if field.value.is_empty() {
                    "inherited"
                } else {
                    &field.value
                }
            ));
            egui::Popup::menu(&response).show(|ui| {
                ui.set_width(235.0);
                ui.strong(format!("{title} colour"));
                ui.weak(if field.value.is_empty() {
                    "Inherited from theme"
                } else {
                    "Explicit override"
                });
                let mut picked = color;
                if egui::color_picker::color_picker_color32(
                    ui,
                    &mut picked,
                    egui::color_picker::Alpha::Opaque,
                ) {
                    field.value = hex(picked);
                }
                ui.add(
                    egui::TextEdit::singleline(&mut field.value)
                        .hint_text(hex(inherited))
                        .desired_width(125.0),
                );
                ui.weak("Exact #rrggbb; blank inherits the theme.");
                if ui.button("Reset to theme").clicked() {
                    field.value.clear();
                }
                field_errors(ui, &field.key, errors);
            });
            ui.label(egui::RichText::new(title).size(11.0));
            ui.weak(
                egui::RichText::new(if field.value.is_empty() {
                    "Inherit"
                } else {
                    "Override"
                })
                .size(10.0),
            );
            if errors.iter().any(|(key, _)| key == &field.key) {
                ui.colored_label(ui.visuals().error_fg_color, "Invalid");
            }
        });
    });
}

fn parsed(value: &str) -> Option<egui::Color32> {
    let digits = value
        .strip_prefix('#')
        .filter(|v| v.len() == 6 && v.is_ascii())?;
    let rgb = u32::from_str_radix(digits, 16).ok()?;
    Some(egui::Color32::from_rgb(
        (rgb >> 16) as u8,
        (rgb >> 8) as u8,
        rgb as u8,
    ))
}
fn hex(color: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}
fn inherited_color(theme: &Theme, key: &str) -> egui::Color32 {
    match key {
        "background" => theme.background(),
        "foreground" => theme.foreground(),
        "cursor" => theme.color(term_core::TermColor::Cursor),
        "divider" => theme.divider(),
        "focus" => theme.focus(),
        "panel" => theme.panel_background(),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_swatch_picker_does_not_create_an_override() {
        let mut draft = Document::new(String::new());
        let ctx = egui::Context::default();
        let mut target = egui::Pos2::ZERO;
        for step in 0..3 {
            let events = if step == 1 {
                vec![
                    egui::Event::PointerMoved(target),
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                vec![]
            };
            ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    target = ui.cursor().min + egui::vec2(20.0, 14.0);
                    let field = draft
                        .fields
                        .iter_mut()
                        .find(|f| f.key == "background")
                        .unwrap();
                    swatch(ui, field, "Background", Theme::default().background(), &[]);
                },
            )
            .drop_without_applying_deltas();
        }
        assert!(
            egui::Popup::is_any_open(&ctx),
            "the picker interaction must actually open a popup"
        );
        assert!(draft.fields.iter().all(|f| f.value.is_empty()));
        assert!(!draft.text().contains("background"));
    }

    #[test]
    fn ansi_palette_starts_collapsed_and_expands_to_all_sixteen_controls() {
        let ctx = egui::Context::default();
        let mut draft = Document::new(String::new());
        let mut target = egui::Pos2::ZERO;
        let mut labels = Vec::new();
        for step in 0..4 {
            let events = if step == 2 {
                vec![
                    egui::Event::PointerMoved(target),
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                vec![]
            };
            let output = ctx.run_ui(
                egui::RawInput {
                    time: Some(step as f64),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(900.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| show(&mut draft, ui, &[]),
            );
            labels.clear();
            for clipped in &output.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    let label = text.galley.text().to_owned();
                    if label == "ANSI palette: 16 colours" {
                        target = text.pos + text.galley.size() / 2.0;
                    }
                    labels.push(label);
                }
            }
            if step == 1 {
                assert!(!labels.iter().any(|s| s == "15"));
            }
            output.drop_without_applying_deltas();
        }
        for index in 0..16 {
            assert!(labels.contains(&index.to_string()), "missing ANSI {index}");
        }
        assert!(draft.fields.iter().all(|f| f.value.is_empty()));
    }

    #[test]
    fn exact_hex_and_invalid_values_are_distinct_from_inheritance() {
        assert_eq!(
            parsed("#aB01ff"),
            Some(egui::Color32::from_rgb(171, 1, 255))
        );
        assert_eq!(hex(parsed("#aB01ff").unwrap()), "#ab01ff");
        for invalid in ["", "#123", "red", "#12345z", "#123456ff"] {
            assert_eq!(parsed(invalid), None);
        }
        let mut draft = Document::new(String::new());
        let field = draft
            .fields
            .iter_mut()
            .find(|f| f.key == "background")
            .unwrap();
        field.value = "#12345z".into();
        assert!(draft.errors().iter().any(|(key, _)| key == "background"));
        draft
            .fields
            .iter_mut()
            .find(|f| f.key == "background")
            .unwrap()
            .value
            .clear();
        assert!(draft.errors().is_empty());
    }
}
