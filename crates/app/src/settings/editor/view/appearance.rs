//! Theme samples and explicitly lazy font controls for the staged document.
use super::{ACCENT, Editor, colors, field_errors};
use crate::settings::document::Field;
use eframe::egui;
use std::time::Instant;

pub(super) fn show(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    errors: &[(String, String)],
) {
    ui.strong("Theme");
    let draft = editor.draft.as_mut().expect("open draft");
    let theme = draft.fields.iter_mut().find(|f| f.key == "theme").unwrap();
    let card_width = ((ui.available_width() - 12.0) / 2.0).clamp(130.0, 260.0);
    ui.horizontal_wrapped(|ui| {
        for (name, title) in [
            ("gruvbox-dark", "Gruvbox Dark"),
            ("tokyonight", "Tokyo Night"),
        ] {
            theme_card(ui, theme, name, title, card_width);
        }
    });
    ui.selectable_value(&mut theme.value, String::new(), "Built-in theme (inherit)");
    ui.add_space(10.0);
    fonts(editor, ui, discoverer, errors);
    ui.add_space(6.0);
    ui.weak(egui::RichText::new("Aa Bb Cc 0123456789 > ~/workspace").monospace());
    ui.separator();
    colors::show(editor.draft.as_mut().unwrap(), ui, errors);
}

fn theme_card(ui: &mut egui::Ui, field: &mut Field, name: &str, title: &str, width: f32) {
    let theme = crate::theme::NamedTheme::from_written(name)
        .unwrap()
        .theme();
    let selected = field.value == name;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 111.0), egui::Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, 7.0, theme.background());
    painter.rect_stroke(
        rect,
        7.0,
        egui::Stroke::new(
            if selected { 2.0 } else { 1.0 },
            if selected {
                ACCENT
            } else {
                egui::Color32::from_rgb(48, 55, 68)
            },
        ),
        egui::StrokeKind::Inside,
    );
    for (line, color, text) in [
        (0.0, theme.foreground(), "~/workspace"),
        (19.0, theme.foreground(), "$ cargo test"),
        (
            38.0,
            theme.color(term_core::TermColor::Indexed(2)),
            "All checks passed",
        ),
    ] {
        painter.text(
            rect.min + egui::vec2(12.0, 12.0 + line),
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::monospace(11.0),
            color,
        );
    }
    let bar = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 2.0, rect.max.y - 30.0),
        rect.max - egui::vec2(2.0, 2.0),
    );
    painter.rect_filled(bar, 4.0, egui::Color32::from_rgb(35, 41, 52));
    painter.text(
        bar.left_center() + egui::vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(233, 237, 245),
    );
    if selected {
        let at = bar.right_center() - egui::vec2(12.0, 0.0);
        painter.add(egui::Shape::line(
            vec![
                at + egui::vec2(-4.0, 0.0),
                at + egui::vec2(-1.0, 3.0),
                at + egui::vec2(5.0, -4.0),
            ],
            egui::Stroke::new(1.6, ACCENT),
        ));
    }
    if response
        .on_hover_text(format!(
            "Use {title}; explicit colour overrides are retained"
        ))
        .clicked()
    {
        field.value = name.into();
    }
}

fn fonts(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    errors: &[(String, String)],
) {
    let draft = editor.draft.as_mut().unwrap();
    let field = draft.fields.iter_mut().find(|f| f.key == "font").unwrap();
    ui.strong("Font family");
    ui.horizontal_wrapped(|ui| {
        let selected = if field.value.is_empty() {
            "Built-in default"
        } else {
            &field.value
        };
        let response = ui
            .button(format!("{selected} ..."))
            .on_hover_text("Browse installed font families");
        editor.font_rect = Some(response.rect);
        if response.clicked() {
            editor.font_requests += 1;
            if editor.fonts.is_none() {
                let started = Instant::now();
                editor.fonts = Some(discoverer.installed_families());
                editor.timing.record_work(3, started.elapsed());
            }
        }
        if !field.value.is_empty() && ui.small_button("Reset").clicked() {
            field.value.clear();
        }
    });
    if let Some(fonts) = &editor.fonts {
        egui::ComboBox::from_id_salt("settings-font")
            .selected_text("Choose installed family")
            .width(ui.available_width().min(280.0))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut field.value, String::new(), "Built-in default");
                for name in fonts {
                    ui.selectable_value(&mut field.value, name.clone(), name);
                }
            });
    }
    field_errors(ui, "font", errors);
    let size = draft.fields.iter_mut().find(|f| f.key == "size").unwrap();
    ui.horizontal_wrapped(|ui| {
        ui.label("Size · pt");
        ui.add(
            egui::TextEdit::singleline(&mut size.value)
                .hint_text("14 (inherit)")
                .desired_width(95.0),
        );
        if !size.value.is_empty() && ui.small_button("Reset").clicked() {
            size.value.clear();
        }
        ui.weak("4–96");
    });
    field_errors(ui, "size", errors);
}
