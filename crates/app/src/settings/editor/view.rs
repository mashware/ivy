//! Compact, staged configuration controls; only Save changes the running terminal.
use super::Editor;
use eframe::egui;
use std::time::Instant;
mod appearance;
mod colors;
mod footer;
pub(super) mod typography;

const SECTIONS: [&str; 4] = ["Appearance", "Cursor", "Shortcuts", "Startup"];
const ACCENT: egui::Color32 = egui::Color32::from_rgb(173, 195, 255);

pub(super) fn show(
    editor: &mut Editor,
    ctx: &egui::Context,
    discoverer: &mut crate::fonts::Discoverer,
) {
    let mut close = false;
    let popup_was_open = egui::Popup::is_any_open(ctx);
    egui::Modal::new(egui::Id::new("terminal-settings"))
        .frame(modal_frame())
        .show(ctx, |ui| {
            settings_style(ui);
            form(editor, ui, discoverer, &mut close);
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && !popup_was_open
        && !egui::Popup::is_any_open(ctx)
    {
        if editor.picker_open {
            editor.picker_open = false;
        } else {
            close = true;
        }
    }
    if close {
        discoverer.clear_preview(ctx);
        editor.picker_open = false;
        editor.picker_query.clear();
        editor.picker_filter = 0;
        editor.draft = None;
        editor.store = None;
        editor.fonts = None;
        ctx.memory_mut(|m| m.stop_text_input());
    }
}

pub(crate) fn style(ui: &mut egui::Ui, theme: crate::theme::Theme) {
    let style = ui.style_mut();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(theme.foreground());
    style.visuals.extreme_bg_color = theme.background();
    style.visuals.faint_bg_color = theme.panel_background();
    style.visuals.window_fill = theme.panel_background();
    style.visuals.panel_fill = theme.panel_background();
    style.visuals.selection.bg_fill = theme.focus();
    style.visuals.selection.stroke.color = theme.background();
    style.visuals.error_fg_color = theme.color(term_core::TermColor::Indexed(1));
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.bg_fill = theme.panel_background();
        widget.weak_bg_fill = theme.background();
        widget.bg_stroke.color = theme.divider();
        widget.fg_stroke.color = theme.foreground();
    }
    style.visuals.widgets.hovered.bg_fill = theme.divider();
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(10.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn shared_chrome_modal_controls_follow_the_effective_theme() {
        let ctx = egui::Context::default();
        let first = crate::theme::NamedTheme::TokyoNight.theme();
        let second = crate::theme::NamedTheme::GruvboxDark.theme();
        assert_ne!(first.focus(), second.focus());
        ctx.run_ui(Default::default(), |ui| {
            style(ui, first);
            assert_eq!(ui.visuals().selection.bg_fill, first.focus());
            assert_eq!(ui.visuals().override_text_color, Some(first.foreground()));
            style(ui, second);
            assert_eq!(ui.visuals().selection.bg_fill, second.focus());
            assert_eq!(ui.visuals().override_text_color, Some(second.foreground()));
        })
        .drop_without_applying_deltas();
    }
}

pub(crate) fn modal_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(25, 29, 38))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 55, 68)))
        .corner_radius(14)
        .inner_margin(0)
}

pub(crate) fn settings_style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(233, 237, 245));
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(21, 26, 34);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(43, 54, 80);
    style.visuals.selection.stroke.color = ACCENT;
    style.visuals.panel_fill = egui::Color32::from_rgb(25, 29, 38);
    style.visuals.window_fill = egui::Color32::from_rgb(35, 41, 54);
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(32, 37, 49);
    for (widgets, fill, stroke) in [
        (
            &mut style.visuals.widgets.inactive,
            [32, 37, 49],
            [54, 62, 80],
        ),
        (
            &mut style.visuals.widgets.hovered,
            [43, 50, 65],
            [90, 108, 144],
        ),
        (
            &mut style.visuals.widgets.active,
            [48, 57, 76],
            [169, 191, 255],
        ),
        (
            &mut style.visuals.widgets.open,
            [43, 50, 65],
            [90, 108, 144],
        ),
    ] {
        widgets.bg_fill = egui::Color32::from_rgb(fill[0], fill[1], fill[2]);
        widgets.weak_bg_fill = widgets.bg_fill;
        widgets.bg_stroke = egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(stroke[0], stroke[1], stroke[2]),
        );
        widgets.corner_radius = egui::CornerRadius::same(6);
    }
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.interact_size.y = 36.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(25.0));
}

fn form(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    close: &mut bool,
) {
    let screen = ui.ctx().content_rect();
    let size = egui::vec2(
        (screen.width() - 32.0).clamp(1.0, 1000.0),
        (screen.height() - 32.0).clamp(1.0, 756.0),
    );
    let (outer, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let wide = size.x > 540.0;
    let sidebar = if size.x >= 780.0 { 184.0 } else { 150.0 };
    let mut main = outer;
    if wide {
        let side =
            egui::Rect::from_min_max(outer.min, egui::pos2(outer.min.x + sidebar, outer.max.y));
        ui.painter().rect_filled(
            side,
            egui::CornerRadius {
                nw: 14,
                ne: 0,
                sw: 14,
                se: 0,
            },
            egui::Color32::from_rgb(23, 27, 35),
        );
        ui.painter().vline(
            side.right(),
            side.y_range(),
            egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 54, 68)),
        );
        ui.painter().text(
            side.min + egui::vec2(28.0, 30.0),
            egui::Align2::LEFT_TOP,
            "Settings",
            egui::FontId::proportional(18.0),
            ui.visuals().text_color(),
        );
        ui.painter().text(
            side.left_bottom() + egui::vec2(28.0, -24.0),
            egui::Align2::LEFT_BOTTOM,
            "agent-terminal",
            egui::FontId::proportional(11.0),
            egui::Color32::from_rgb(105, 115, 134),
        );
        let nav = egui::Rect::from_min_max(
            side.min + egui::vec2(14.0, 82.0),
            side.max - egui::vec2(14.0, 20.0),
        );
        let mut nav_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("settings-navigation")
                .max_rect(nav),
        );
        nav_ui.set_clip_rect(nav.intersect(ui.clip_rect()));
        navigation(editor, &mut nav_ui, true);
        main.min.x = side.right();
    } else {
        let nav = egui::Rect::from_min_max(
            outer.min + egui::vec2(16.0, 12.0),
            egui::pos2(outer.right() - 16.0, outer.top() + 52.0),
        );
        let mut nav_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("settings-navigation")
                .max_rect(nav),
        );
        nav_ui.set_clip_rect(nav.intersect(ui.clip_rect()));
        navigation(editor, &mut nav_ui, false);
        main.min.y += 56.0;
    }
    let started = Instant::now();
    let errors = editor
        .draft
        .as_ref()
        .map(|d| d.errors())
        .unwrap_or_default();
    editor.timing.record_work(2, started.elapsed());
    let padding = if wide { 30.0 } else { 20.0 };
    let needed =
        98.0 + footer::height(editor, ui, (main.width() - 2.0 * padding).max(1.0), &errors) + 92.0;
    if main.height() < needed {
        let mut scroll_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("settings-short-window")
                .max_rect(main),
        );
        scroll_ui.set_clip_rect(main.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt("settings-main-fallback")
            .max_height(main.height())
            .auto_shrink([false, false])
            .show(&mut scroll_ui, |ui| {
                let width = ui.available_width();
                let height = 98.0
                    + footer::height(editor, ui, (width - 2.0 * padding).max(1.0), &errors)
                    + 92.0;
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
                main_column(editor, ui, discoverer, close, rect, padding, &errors);
            });
    } else {
        main_column(editor, ui, discoverer, close, main, padding, &errors);
    }
}

fn main_column(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    close: &mut bool,
    main: egui::Rect,
    padding: f32,
    errors: &[(String, String)],
) {
    let inner = egui::Rect::from_min_max(
        main.min + egui::vec2(padding, 0.0),
        main.max - egui::vec2(padding, 0.0),
    );
    let footer_height = footer::height(editor, ui, inner.width(), errors);
    let header_height = 98.0;
    // The fixed regions and body have independent rectangles: content cannot grow the modal.
    let footer_top = (main.bottom() - footer_height).max(main.top());
    let header_bottom = (main.top() + header_height).min(footer_top);
    let heading = egui::Rect::from_min_max(
        inner.min + egui::vec2(0.0, 26.0),
        egui::pos2(inner.right(), header_bottom),
    );
    let mut head_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("settings-heading")
            .max_rect(heading),
    );
    head_ui.set_clip_rect(heading.intersect(ui.clip_rect()));
    head_ui.heading(SECTIONS[editor.section.min(3)]);
    head_ui.label(
        egui::RichText::new(match editor.section {
            0 => "Your theme, colors and type.",
            1 => "Keep your place in the terminal.",
            2 => "Make familiar actions your own.",
            _ => "Choose what opens next time.",
        })
        .size(14.0)
        .color(egui::Color32::from_rgb(146, 155, 174)),
    );
    let body = egui::Rect::from_min_max(
        egui::pos2(inner.left(), header_bottom),
        egui::pos2(inner.right(), footer_top - 12.0).max(egui::pos2(inner.left(), header_bottom)),
    );
    let mut body_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("settings-body")
            .max_rect(body),
    );
    body_ui.set_clip_rect(body.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical()
        .id_salt("settings-content")
        .max_height(body.height())
        .auto_shrink([false, false])
        .show(&mut body_ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 6.0).max(0.0));
                    match editor.section {
                        0 => appearance::show(editor, ui, discoverer, errors),
                        1 => cursor(editor, ui),
                        2 => shortcuts(editor, ui, errors),
                        _ => startup(editor, ui),
                    }
                });
            });
        });
    let footer_rect = egui::Rect::from_min_max(egui::pos2(inner.left(), footer_top), inner.max);
    ui.painter().hline(
        main.x_range(),
        footer_top,
        egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 54, 68)),
    );
    let mut foot_ui = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("settings-footer")
            .max_rect(footer_rect),
    );
    foot_ui.set_clip_rect(footer_rect.intersect(ui.clip_rect()));
    footer::show(editor, &mut foot_ui, close, errors, discoverer);
}

fn navigation(editor: &mut Editor, ui: &mut egui::Ui, vertical: bool) {
    let controls = |ui: &mut egui::Ui| {
        for (index, name) in SECTIONS.iter().enumerate() {
            let selected = editor.section == index;
            let font = egui::FontId::proportional(14.0);
            let width = if vertical {
                ui.available_width()
            } else {
                ui.painter()
                    .layout_no_wrap((*name).to_owned(), font.clone(), ui.visuals().text_color())
                    .size()
                    .x
                    + 44.0
            };
            let ink = if selected {
                egui::Color32::from_rgb(196, 210, 255)
            } else {
                egui::Color32::from_rgb(146, 155, 174)
            };
            let response = ui.add_sized([width, 40.0], egui::Button::new("").frame(false));
            if selected || response.hovered() {
                ui.painter().rect_filled(
                    response.rect,
                    7.0,
                    if selected {
                        egui::Color32::from_rgb(41, 51, 75)
                    } else {
                        egui::Color32::from_rgb(32, 37, 49)
                    },
                );
            }
            let center = response.rect.left_center() + egui::vec2(17.0, 0.0);
            navigation_icon(ui.painter(), index, center, ink);
            ui.painter().text(
                response.rect.left_center() + egui::vec2(34.0, 0.0),
                egui::Align2::LEFT_CENTER,
                *name,
                font,
                ink,
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), *name)
            });
            if response.clicked() {
                editor.section = index;
            }
        }
    };
    if vertical {
        ui.vertical(controls);
    } else {
        egui::ScrollArea::horizontal()
            .id_salt("settings-navigation-scroll")
            .show(ui, |ui| {
                ui.horizontal(controls);
            });
    }
}

fn navigation_icon(painter: &egui::Painter, index: usize, at: egui::Pos2, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.2, color);
    match index {
        0 => {
            painter.circle_stroke(at, 6.0, stroke);
            let points: Vec<_> = (0..=12)
                .map(|i| {
                    let angle =
                        std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 12.0;
                    at + egui::vec2(angle.cos(), angle.sin()) * 5.0
                })
                .collect();
            painter.add(egui::Shape::convex_polygon(
                points,
                color,
                egui::Stroke::NONE,
            ));
        }
        1 => {
            painter.add(egui::Shape::closed_line(
                vec![
                    at + egui::vec2(-4.0, -7.0),
                    at + egui::vec2(6.0, 1.0),
                    at + egui::vec2(0.0, 2.0),
                    at + egui::vec2(-2.0, 7.0),
                ],
                stroke,
            ));
        }
        _ => {
            painter.rect_stroke(
                egui::Rect::from_center_size(at, egui::vec2(14.0, 10.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            for x in [-4.0, 0.0, 4.0] {
                painter.circle_filled(at + egui::vec2(x, -1.5), 0.7, color);
            }
            painter.line_segment(
                [at + egui::vec2(-4.0, 2.0), at + egui::vec2(4.0, 2.0)],
                stroke,
            );
        }
    }
}

fn cursor(editor: &mut Editor, ui: &mut egui::Ui) {
    let field = editor
        .draft
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.key == "cursor.blink")
        .expect("cursor setting exists");
    for (value, label) in [
        ("", "Follow the program"),
        ("on", "Blink regardless of the program"),
        ("off", "Never blink"),
    ] {
        ui.radio_value(&mut field.value, value.into(), label);
    }
    ui.add_space(10.0);
    ui.weak("Blinking still stops after 15 seconds without typing and only runs in the focused console and window.");
}

fn startup(editor: &mut Editor, ui: &mut egui::Ui) {
    let field = editor
        .draft
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|field| field.key == "startup.restore")
        .expect("startup setting exists");
    let mut selection = if field.value.is_empty() {
        "none".to_owned()
    } else {
        field.value.clone()
    };
    for (value, label) in [
        ("none", "None: one fresh console"),
        ("layout", "Layout"),
        ("directories", "Layout and directories"),
    ] {
        if ui
            .radio_value(&mut selection, value.into(), label)
            .changed()
        {
            field.value = selection.clone();
        }
    }
    ui.add_space(10.0);
    ui.weak("Changes apply on the next launch. Turning restoration off keeps the previous saved layout. Additional windows are temporary and do not save their layout.");
}

fn shortcuts(editor: &mut Editor, ui: &mut egui::Ui, errors: &[(String, String)]) {
    ui.label("Blank uses the factory chord. 'none' hands it back to the program. Copy is controlled separately.");
    ui.add_space(12.0);
    let defaults = crate::keyboard::Keymap::default();
    let draft = editor.draft.as_mut().expect("an open editor has a draft");
    for field in draft
        .fields
        .iter_mut()
        .filter(|f| f.key.starts_with("key."))
    {
        if field.key == "key.copy" {
            let mut enabled = field.value.is_empty();
            if ui
                .checkbox(&mut enabled, "Enable copy (Ctrl+Shift+C)")
                .changed()
            {
                field.value = if enabled {
                    String::new()
                } else {
                    "none".into()
                };
            }
        } else {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let label_width = if ui.available_width() >= 500.0 {
                    165.0
                } else {
                    130.0_f32.min(ui.available_width())
                };
                ui.add_sized(
                    [label_width, 36.0],
                    egui::Label::new(field.key.trim_start_matches("key.").replace('-', " ")),
                );
                let action =
                    crate::keyboard::Action::from_written(field.key.trim_start_matches("key."));
                let hint = action
                    .and_then(|a| defaults.chord_for(a))
                    .map(written_chord)
                    .unwrap_or_default();
                ui.add_sized(
                    [150.0_f32.min(ui.available_width()), 36.0],
                    egui::TextEdit::singleline(&mut field.value)
                        .hint_text(hint)
                        .margin(egui::Margin::symmetric(12, 8))
                        .vertical_align(egui::Align::Center),
                );
                if ui
                    .button("None")
                    .on_hover_text("Hand this shortcut back to the program")
                    .clicked()
                {
                    field.value = "none".into();
                }
                if ui.button("Default").clicked() {
                    field.value.clear();
                }
            });
        }
        field_errors(ui, &field.key, errors);
    }
}

fn field_errors(ui: &mut egui::Ui, key: &str, errors: &[(String, String)]) {
    for (_, error) in errors.iter().filter(|(candidate, _)| candidate == key) {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
}

fn written_chord(chord: crate::keyboard::Chord) -> String {
    let mut parts = Vec::new();
    if chord.ctrl {
        parts.push("Ctrl");
    }
    if chord.shift {
        parts.push("Shift");
    }
    if chord.alt {
        parts.push("Alt");
    }
    parts.push(chord.key.name());
    parts.join("+")
}

#[cfg(test)]
mod tests;
