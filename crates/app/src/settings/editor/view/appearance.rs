//! Theme samples and explicitly lazy font controls for the staged document.
use super::{Editor, colors};
use eframe::egui;

pub(super) fn show(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    errors: &[(String, String)],
) {
    ui.strong("Theme & colors");
    let count = editor
        .draft
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .filter(|f| {
            !f.value.is_empty()
                && (f.key.starts_with("color.")
                    || [
                        "background",
                        "foreground",
                        "cursor",
                        "divider",
                        "focus",
                        "panel",
                    ]
                    .contains(&f.key.as_str()))
        })
        .count();
    ui.add_space(8.0);
    if ui.available_width() >= 500.0 {
        let available = ui.available_width();
        let gap = 24.0;
        let left = (available - gap) * 0.43;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(left);
                    theme_picker(editor, ui, count);
                },
            );
            ui.add_space(gap - ui.spacing().item_spacing.x);
            ui.allocate_ui_with_layout(
                egui::vec2((available - gap) - left, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width((available - gap) - left);
                    preview(editor, ui);
                },
            );
        });
    } else {
        theme_picker(editor, ui, count);
        ui.add_space(10.0);
        preview(editor, ui);
    }
    ui.add_space(10.0);
    let color_errors = errors
        .iter()
        .filter(|(key, _)| {
            key.starts_with("color.")
                || [
                    "background",
                    "foreground",
                    "cursor",
                    "divider",
                    "focus",
                    "panel",
                ]
                .contains(&key.as_str())
        })
        .count();
    ui.separator();
    let id = ui.make_persistent_id("settings-customize-colors");
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let width = ui.available_width();
    let theme = effective_theme(editor.draft.as_ref().unwrap());
    let mut label_clicked = false;
    let mut header = state.show_header(ui, |ui| {
        let label = ui.add(egui::Label::new("Customize colors").sense(egui::Sense::click()));
        let badge = format!("{count} override{}", if count == 1 { "" } else { "s" });
        let font = egui::FontId::proportional(10.0);
        let size = ui
            .painter()
            .layout_no_wrap(
                badge.clone(),
                font.clone(),
                egui::Color32::from_rgb(214, 190, 142),
            )
            .size();
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(size.x + 12.0, 20.0), egui::Sense::hover());
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(91, 81, 61)),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            badge,
            font,
            egui::Color32::from_rgb(214, 190, 142),
        );
        if color_errors > 0 {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!(
                    "{color_errors} error{}",
                    if color_errors == 1 { "" } else { "s" }
                ),
            );
        }
        if width >= 400.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for color in [
                    theme.color(term_core::TermColor::Indexed(2)),
                    theme.color(term_core::TermColor::Indexed(1)),
                    theme.color(term_core::TermColor::Indexed(4)),
                    theme.foreground(),
                    theme.background(),
                ] {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, color);
                }
            });
        }
        label_clicked = label.clicked();
    });
    if label_clicked {
        header.toggle();
    }
    header.body_unindented(|ui| {
        colors::show(editor.draft.as_mut().unwrap(), ui, errors);
    });
    ui.separator();
    ui.add_space(8.0);
    ui.strong("Typography");
    super::typography::show(editor, ui, discoverer, errors);
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

fn preview(editor: &Editor, ui: &mut egui::Ui) {
    let theme = effective_theme(editor.draft.as_ref().unwrap());
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 150.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 7.0, theme.background());
    let top = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + 27.0));
    painter.rect_filled(top, 5.0, egui::Color32::from_rgb(37, 41, 56));
    painter.text(
        top.min + egui::vec2(11.0, 7.0),
        egui::Align2::LEFT_TOP,
        ">_  Preview",
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(151, 160, 183),
    );
    let body = egui::Rect::from_min_max(egui::pos2(rect.min.x, top.max.y), rect.max);
    let panel = egui::Rect::from_min_max(
        egui::pos2(body.min.x + body.width() * 0.615, body.min.y),
        body.max,
    );
    let console = egui::Rect::from_min_max(body.min, egui::pos2(panel.min.x, body.max.y));
    painter.rect_filled(panel, 0.0, theme.panel_background());
    painter.with_clip_rect(panel).circle_filled(
        panel.min + egui::vec2(12.0, 60.0),
        2.5,
        theme.color(term_core::TermColor::Indexed(4)),
    );
    for (pos, text, color, clip) in [
        (
            console.min + egui::vec2(12.0, 11.0),
            "~/workspace",
            theme.color(term_core::TermColor::Indexed(4)),
            console,
        ),
        (
            console.min + egui::vec2(12.0, 32.0),
            "$ cargo test",
            theme.foreground(),
            console,
        ),
        (
            console.min + egui::vec2(12.0, 53.0),
            "All checks passed",
            theme.color(term_core::TermColor::Indexed(2)),
            console,
        ),
        (
            panel.min + egui::vec2(10.0, 11.0),
            "STATUS",
            theme.foreground(),
            panel,
        ),
        (
            panel.min + egui::vec2(10.0, 30.0),
            "Run checks",
            theme.foreground(),
            panel,
        ),
        (
            panel.min + egui::vec2(20.0, 53.0),
            "In progress",
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
}

pub(super) fn selector(ui: &mut egui::Ui, title: &str, width: f32) -> egui::Response {
    let response = ui.add_sized([width, 40.0], egui::Button::new(""));
    let text_clip =
        egui::Rect::from_min_max(response.rect.min, response.rect.max - egui::vec2(28.0, 0.0));
    ui.painter().with_clip_rect(text_clip).text(
        response.rect.left_center() + egui::vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(14.0),
        ui.visuals().text_color(),
    );
    let arrow = response.rect.right_center() - egui::vec2(15.0, 0.0);
    ui.painter().line_segment(
        [arrow + egui::vec2(-4.0, -2.0), arrow + egui::vec2(0.0, 2.0)],
        egui::Stroke::new(1.3, ui.visuals().weak_text_color()),
    );
    ui.painter().line_segment(
        [arrow + egui::vec2(0.0, 2.0), arrow + egui::vec2(4.0, -2.0)],
        egui::Stroke::new(1.3, ui.visuals().weak_text_color()),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), title)
    });
    response
}

fn theme_picker(editor: &mut Editor, ui: &mut egui::Ui, count: usize) {
    use crate::theme::NamedTheme;
    ui.weak("Color theme");
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
    let response = selector(ui, title, ui.available_width());
    let opened = response.clicked();
    egui::Popup::menu(&response)
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(35, 41, 54))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 81, 106)))
                .corner_radius(9)
                .inner_margin(10),
        )
        .width(
            300.0_f32
                .min(ui.ctx().content_rect().width() - 24.0)
                .max(80.0),
        )
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            super::settings_style(ui);
            ui.set_width(
                280.0_f32
                    .min(ui.ctx().content_rect().width() - 44.0)
                    .max(60.0),
            );
            let search = egui::Frame::new()
                .fill(egui::Color32::from_rgb(23, 28, 37))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 54, 68)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        let center = rect.center() - egui::vec2(1.0, 1.0);
                        let stroke = egui::Stroke::new(1.2, ui.visuals().weak_text_color());
                        ui.painter().circle_stroke(center, 4.0, stroke);
                        ui.painter().line_segment(
                            [center + egui::vec2(3.0, 3.0), center + egui::vec2(6.0, 6.0)],
                            stroke,
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut editor.picker_query)
                                .frame(egui::Frame::new())
                                .hint_text("Search themes...")
                                .desired_width(ui.available_width()),
                        )
                    })
                    .inner
                })
                .inner;
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
                .min_scrolled_height(
                    200.0_f32.min((ui.ctx().content_rect().height() - 180.0).max(64.0)),
                )
                .max_height(240.0)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    if choices.is_empty() {
                        ui.weak("No matching themes.");
                    }
                    for (id, label) in choices {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 32.0),
                            egui::Sense::click(),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                label,
                            )
                        });
                        if current == id || response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                5.0,
                                if current == id {
                                    egui::Color32::from_rgb(50, 57, 74)
                                } else {
                                    egui::Color32::from_rgb(42, 48, 62)
                                },
                            );
                        }
                        ui.painter().with_clip_rect(rect).text(
                            rect.left_center() + egui::vec2(8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::proportional(14.0),
                            ui.visuals().text_color(),
                        );
                        if direction != 0 && current == id {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if response.clicked() {
                            current = id.to_owned();
                        }
                        let sample = NamedTheme::from_written(id)
                            .map_or_else(crate::theme::Theme::default, |t| t.theme());
                        for (i, index) in [1, 2, 4].into_iter().enumerate() {
                            let dot = egui::Rect::from_center_size(
                                rect.right_center() - egui::vec2(30.0 - i as f32 * 10.0, 0.0),
                                egui::vec2(7.0, 7.0),
                            );
                            ui.painter().rect_filled(
                                dot,
                                2.0,
                                sample.color(term_core::TermColor::Indexed(index)),
                            );
                        }
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
        });
    if count == 0 {
        ui.weak("Colors inherit from this theme.");
    } else {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(egui::vec2(6.0, 6.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(dot.center(), 2.5, egui::Color32::from_rgb(196, 167, 110));
            ui.label(
                egui::RichText::new(format!(
                    "{count} color{} customized",
                    if count == 1 { "" } else { "s" }
                ))
                .size(14.0)
                .color(ui.visuals().weak_text_color()),
            );
        });
        ui.weak("Your color changes apply on top of this theme.");
    }
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
