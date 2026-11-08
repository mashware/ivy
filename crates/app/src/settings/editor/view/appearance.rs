//! Theme samples and explicitly lazy font controls for the staged document.
use super::{Editor, colors, field_errors};
use eframe::egui;
use std::time::Instant;

pub(super) fn show(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    errors: &[(String, String)],
) {
    theme_picker(editor, ui);
    ui.add_space(10.0);
    fonts(editor, ui, discoverer, errors);
    ui.add_space(6.0);
    ui.weak(egui::RichText::new("Aa Bb Cc 0123456789 > ~/workspace").monospace());
    ui.separator();
    colors::show(editor.draft.as_mut().unwrap(), ui, errors);
}

fn effective_theme(draft: &crate::settings::document::Document) -> crate::theme::Theme {
    let mut settings = crate::settings::Settings::default();
    for field in &draft.fields {
        if !field.value.is_empty()
            && (field.key == "theme"
                || field.key.starts_with("color.")
                || [
                    "background",
                    "foreground",
                    "cursor",
                    "divider",
                    "focus",
                    "panel",
                ]
                .contains(&field.key.as_str()))
        {
            let _ = crate::settings::apply(&mut settings, &field.key, &field.value);
        }
    }
    settings
        .theme
        .map_or_else(crate::theme::Theme::default, |t| t.theme())
        .with_colors(&settings.colors)
}

fn theme_picker(editor: &mut Editor, ui: &mut egui::Ui) {
    use crate::theme::NamedTheme;
    ui.strong("Theme");
    let selected = editor
        .draft
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .find(|f| f.key == "theme")
        .unwrap()
        .value
        .clone();
    let title =
        NamedTheme::from_written(&selected).map_or("Built-in theme (inherit)", |t| t.title());
    let mut opened = false;
    ui.horizontal(|ui| {
        ui.label(title);
        if ui
            .button(if editor.picker_open { "Done" } else { "Change" })
            .clicked()
        {
            editor.picker_open = !editor.picker_open;
            opened = editor.picker_open;
        }
    });
    let theme = effective_theme(editor.draft.as_ref().unwrap());
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 100.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, theme.background());
    let panel = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + rect.width() * 0.65, rect.min.y),
        rect.max,
    );
    painter.rect_filled(panel, 0.0, theme.panel_background());
    for (pos, text, color, clip) in [
        (
            rect.min + egui::vec2(10.0, 12.0),
            "$ cargo test",
            theme.foreground(),
            rect.intersect(egui::Rect::from_min_max(
                rect.min,
                egui::pos2(panel.min.x, rect.max.y),
            )),
        ),
        (
            rect.min + egui::vec2(10.0, 35.0),
            "All checks passed",
            theme.color(term_core::TermColor::Indexed(2)),
            rect.intersect(egui::Rect::from_min_max(
                rect.min,
                egui::pos2(panel.min.x, rect.max.y),
            )),
        ),
        (
            panel.min + egui::vec2(8.0, 12.0),
            "Status",
            theme.foreground(),
            panel,
        ),
        (
            panel.min + egui::vec2(8.0, 35.0),
            "Working",
            theme.color(term_core::TermColor::Indexed(4)),
            panel,
        ),
    ] {
        painter.with_clip_rect(clip).text(
            pos,
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::monospace(11.0),
            color,
        );
    }
    ui.weak("Explicit color overrides remain active in the preview.");
    if !editor.picker_open {
        return;
    }
    let search =
        ui.add(egui::TextEdit::singleline(&mut editor.picker_query).hint_text("Search themes"));
    if opened {
        search.request_focus();
    }
    ui.horizontal(|ui| {
        for (index, label) in ["All", "Dark", "Light"].iter().enumerate() {
            ui.selectable_value(&mut editor.picker_filter, index, *label);
        }
    });
    let query = editor.picker_query.to_lowercase();
    let mut choices: Vec<(&str, &str)> = Vec::new();
    if editor.picker_filter != 2
        && ("built-in theme (inherit)".contains(&query) || "inherit".contains(&query))
    {
        choices.push(("", "Built-in theme (inherit)"));
    }
    for &theme in NamedTheme::ALL {
        if (editor.picker_filter == 0 || (editor.picker_filter == 1) == theme.is_dark())
            && (theme.id().to_lowercase().contains(&query)
                || theme.title().to_lowercase().contains(&query))
        {
            choices.push((theme.id(), theme.title()));
        }
    }
    let direction = if search.has_focus() {
        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                1
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                -1
            } else {
                0
            }
        })
    } else {
        0
    };
    let mut current = selected.clone();
    if direction != 0 && !choices.is_empty() {
        let next = match choices.iter().position(|(id, _)| *id == selected) {
            Some(index) => {
                (index as isize + direction).clamp(0, choices.len() as isize - 1) as usize
            }
            None if direction > 0 => 0,
            None => choices.len() - 1,
        };
        current = choices[next].0.to_owned();
    }
    egui::ScrollArea::vertical()
        .id_salt("theme-picker")
        .max_height(180.0)
        .show(ui, |ui| {
            if choices.is_empty() {
                ui.weak("No matching themes.");
            }
            for (id, label) in choices {
                ui.horizontal(|ui| {
                    let response = ui.selectable_label(current == id, label);
                    if direction != 0 && current == id {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.clicked() {
                        current = id.to_owned();
                    }
                    let sample = NamedTheme::from_written(id)
                        .map_or_else(crate::theme::Theme::default, |t| t.theme());
                    for index in [1, 2, 4] {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            rect,
                            2.0,
                            sample.color(term_core::TermColor::Indexed(index)),
                        );
                    }
                });
            }
        });
    if current != selected {
        ui.ctx().request_repaint();
    }
    editor
        .draft
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.key == "theme")
        .unwrap()
        .value = current;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_keeps_valid_color_overrides_when_other_fields_are_invalid() {
        let mut draft = crate::settings::document::Document::new(
            "theme = tokyonight\nbackground = #123456\n".into(),
        );
        draft
            .fields
            .iter_mut()
            .find(|f| f.key == "size")
            .unwrap()
            .value = "invalid".into();
        assert_eq!(
            effective_theme(&draft).background(),
            egui::Color32::from_rgb(0x12, 0x34, 0x56)
        );
    }
}
