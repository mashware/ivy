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
            .layout_no_wrap(badge.clone(), font.clone(), ui.visuals().text_color())
            .size();
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(size.x + 12.0, 20.0), egui::Sense::hover());
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, ui.visuals().window_stroke.color),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            badge,
            font,
            ui.visuals().text_color(),
        );
        if color_errors > 0 {
            ui.colored_label(
                ui.visuals().error_fg_color,
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

pub(super) fn effective_theme(draft: &crate::settings::document::Document) -> crate::theme::Theme {
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
    settings.resolved_theme()
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
    painter.rect_filled(top, 5.0, ui.visuals().panel_fill);
    painter.text(
        top.min + egui::vec2(11.0, 7.0),
        egui::Align2::LEFT_TOP,
        ">_  Preview",
        egui::FontId::proportional(10.0),
        ui.visuals().text_color(),
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

/// A dropdown's button; `lead` is room left before the title for a sample painted over it.
pub(super) fn selector(ui: &mut egui::Ui, title: &str, width: f32, lead: f32) -> egui::Response {
    let response = ui.add_sized([width, 40.0], egui::Button::new(""));
    let text_clip =
        egui::Rect::from_min_max(response.rect.min, response.rect.max - egui::vec2(28.0, 0.0));
    ui.painter().with_clip_rect(text_clip).text(
        response.rect.left_center() + egui::vec2(12.0 + lead, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(14.0),
        ui.style().interact(&response).fg_stroke.color,
    );
    let arrow = response.rect.right_center() - egui::vec2(15.0, 0.0);
    ui.painter().line_segment(
        [arrow + egui::vec2(-4.0, -2.0), arrow + egui::vec2(0.0, 2.0)],
        egui::Stroke::new(1.3, ui.style().interact(&response).fg_stroke.color),
    );
    ui.painter().line_segment(
        [arrow + egui::vec2(0.0, 2.0), arrow + egui::vec2(4.0, -2.0)],
        egui::Stroke::new(1.3, ui.style().interact(&response).fg_stroke.color),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), title)
    });
    response
}

/// A theme's four-colour sample: its background, accent, and the waiting and finished claims —
/// the colours a person tells themes apart by. Returns where it ends.
pub(super) fn swatch(painter: &egui::Painter, left_center: egui::Pos2, theme: &crate::theme::Theme) -> f32 {
    let roles = crate::theme::Roles::of(theme);
    let colors = [roles.base, roles.accent, roles.wait, roles.done];
    let size = egui::vec2(8.0, 14.0);
    let top = left_center.y - size.y / 2.0;
    for (i, color) in colors.into_iter().enumerate() {
        let min = egui::pos2(left_center.x + i as f32 * size.x, top);
        painter.rect_filled(egui::Rect::from_min_size(min, size), 0.0, color);
    }
    let whole = egui::Rect::from_min_size(egui::pos2(left_center.x, top), egui::vec2(size.x * 4.0, size.y));
    painter.rect_stroke(
        whole,
        3.0,
        egui::Stroke::new(1.0, painter.ctx().global_style().visuals.widgets.inactive.bg_stroke.color),
        egui::StrokeKind::Outside,
    );
    whole.right()
}

fn check(painter: &egui::Painter, at: egui::Pos2, color: egui::Color32) {
    painter.add(egui::Shape::line(
        vec![at + egui::vec2(-5.0, 0.5), at + egui::vec2(-2.0, 3.5), at + egui::vec2(5.0, -3.5)],
        egui::Stroke::new(2.0, color),
    ));
}

/// The kit's segmented control: a sunk track holding one raised segment per choice.
fn segmented(ui: &mut egui::Ui, value: &mut usize, labels: &[&str]) {
    let (track, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::hover());
    let v = ui.visuals().clone();
    ui.painter().rect(
        track,
        crate::theme::metrics::RADIUS_CONTROL,
        v.extreme_bg_color,
        egui::Stroke::new(1.0, v.widgets.inactive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    let inner = track.shrink(3.0);
    let width = (inner.width() - 2.0 * (labels.len() as f32 - 1.0)) / labels.len() as f32;
    for (index, label) in labels.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            inner.min + egui::vec2(index as f32 * (width + 2.0), 0.0),
            egui::vec2(width, inner.height()),
        );
        let response = ui.interact(rect, ui.id().with(("segment", index)), egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, *value == index, *label)
        });
        if response.clicked() {
            *value = index;
        }
        let selected = *value == index;
        if selected || response.hovered() {
            ui.painter().rect_filled(rect, crate::theme::metrics::RADIUS_CHIP, v.widgets.hovered.bg_fill);
        }
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::new(12.0, crate::fonts::ui_weight(ui.ctx(), 500)),
            if selected { v.widgets.inactive.fg_stroke.color } else { v.weak_text_color() },
        );
    }
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
    let named = NamedTheme::from_written(&selected);
    let title = named.map_or("Ivy Night (default)", |t| t.title());
    // The sample is four 8-point strips and a 10-point gap before the title.
    let response = selector(ui, title, ui.available_width(), 42.0);
    let current = named.map_or_else(crate::theme::Theme::default, |t| t.theme());
    swatch(
        ui.painter(),
        response.rect.left_center() + egui::vec2(12.0, 0.0),
        &current,
    );
    let opened = response.clicked();
    let popup_style = ui.style().clone();
    egui::Popup::menu(&response)
        .frame(super::palette::popup_frame(ui))
        .width(
            300.0_f32
                .min(ui.ctx().content_rect().width() - 24.0)
                .max(80.0),
        )
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_style(popup_style);
            ui.set_width(
                280.0_f32
                    .min(ui.ctx().content_rect().width() - 44.0)
                    .max(60.0),
            );
            let search = egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .stroke(egui::Stroke::new(1.0, ui.visuals().window_stroke.color))
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
            segmented(ui, &mut editor.picker_filter, &["All", "Dark", "Light"]);
            let query = editor.picker_query.to_lowercase();
            let mut choices: Vec<(&str, &str)> = Vec::new();
            if editor.picker_filter != 2
                && ("ivy night (default)".contains(&query) || "inherit".contains(&query))
            {
                choices.push(("", "Ivy Night (default)"));
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
                            egui::vec2(ui.available_width(), crate::theme::metrics::MENU_ROW_HEIGHT),
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
                                crate::theme::metrics::RADIUS_CONTROL,
                                if current == id {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    ui.style().interact(&response).bg_fill
                                },
                            );
                        }
                        let sample = NamedTheme::from_written(id)
                            .map_or_else(crate::theme::Theme::default, |t| t.theme());
                        let swatch_end =
                            swatch(ui.painter(), rect.left_center() + egui::vec2(8.0, 0.0), &sample);
                        if current == id {
                            check(ui.painter(), rect.right_center() - egui::vec2(16.0, 0.0), ui.visuals().widgets.active.bg_stroke.color);
                        }
                        ui.painter().with_clip_rect(rect).text(
                            egui::pos2(swatch_end + 10.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            label,
                            egui::FontId::proportional(14.0),
                            if current == id {
                                ui.visuals().selection.stroke.color
                            } else {
                                ui.style().interact(&response).fg_stroke.color
                            },
                        );
                        if direction != 0 && current == id {
                            response.scroll_to_me(Some(egui::Align::Center));
                        }
                        if response.clicked() {
                            current = id.to_owned();
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
                .circle_filled(dot.center(), 2.5, ui.visuals().text_color());
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
