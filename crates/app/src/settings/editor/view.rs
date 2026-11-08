//! Compact, staged configuration controls; only Save changes the running terminal.
use super::Editor;
use eframe::egui;
use std::time::Instant;
mod appearance;
mod colors;

const SECTIONS: [&str; 3] = ["Appearance", "Cursor", "Shortcuts"];
const ACCENT: egui::Color32 = egui::Color32::from_rgb(173, 195, 255);

pub(super) fn show(
    editor: &mut Editor,
    ctx: &egui::Context,
    discoverer: &mut crate::fonts::Discoverer,
) {
    let mut close = false;
    let popup_was_open = egui::Popup::is_any_open(ctx);
    let frame = egui::Frame::new()
        .fill(egui::Color32::from_rgb(27, 31, 40))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 55, 68)))
        .corner_radius(10)
        .inner_margin(18);
    egui::Modal::new(egui::Id::new("terminal-settings"))
        .frame(frame)
        .show(ctx, |ui| {
            style(ui);
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
        editor.picker_open = false;
        editor.picker_query.clear();
        editor.picker_filter = 0;
        editor.draft = None;
        editor.store = None;
        editor.fonts = None;
        ctx.memory_mut(|m| m.stop_text_input());
    }
}

fn style(ui: &mut egui::Ui) {
    let style = ui.style_mut();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(233, 237, 245));
    style.visuals.extreme_bg_color = egui::Color32::from_rgb(21, 26, 34);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(43, 54, 80);
    style.visuals.selection.stroke.color = ACCENT;
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(35, 41, 52);
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

fn form(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    close: &mut bool,
) {
    let screen = ui.ctx().content_rect();
    let width = (screen.width() - 64.0).clamp(120.0, 780.0);
    ui.set_width(width);
    let wide = width >= 650.0;
    if !wide {
        ui.heading("Settings");
        navigation(editor, ui, false);
        ui.separator();
    }
    let started = Instant::now();
    let errors = editor
        .draft
        .as_ref()
        .map(|d| d.errors())
        .unwrap_or_default();
    editor.timing.record_work(2, started.elapsed());
    let height = (screen.height() - if wide { 265.0 } else { 335.0 }).clamp(32.0, 530.0);
    let mut divider_x = None;
    let row = ui.horizontal_top(|ui| {
        if wide {
            ui.vertical(|ui| {
                ui.set_width(130.0);
                ui.heading("Settings");
                ui.add_space(16.0);
                navigation(editor, ui, true);
            });
            // Reserve width only: a Separator in this unconstrained horizontal row
            // takes all available height and makes the centered modal grow each pass.
            let (gap, _) = ui.allocate_exact_size(egui::vec2(10.0, 0.0), egui::Sense::hover());
            divider_x = Some(gap.center().x);
        }
        ui.vertical(|ui| {
            ui.set_width((width - if wide { 160.0 } else { 0.0 }).max(100.0));
            ui.horizontal(|ui| {
                ui.heading(SECTIONS[editor.section.min(2)]);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("×")
                        .on_hover_text("Close Settings and discard unsaved edits")
                        .clicked()
                    {
                        *close = true;
                    }
                });
            });
            ui.weak(match editor.section {
                0 => "A comfortable place to work.",
                1 => "Keep your place in the terminal.",
                _ => "Make familiar actions your own.",
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .id_salt("settings-content")
                .max_height(height)
                .show(ui, |ui| match editor.section {
                    0 => appearance::show(editor, ui, discoverer, &errors),
                    1 => cursor(editor, ui),
                    _ => shortcuts(editor, ui, &errors),
                });
        });
    });
    if let Some(x) = divider_x {
        ui.painter().vline(
            x,
            row.response.rect.y_range(),
            ui.visuals().widgets.noninteractive.bg_stroke,
        );
    }
    ui.separator();
    footer(editor, ui, close, &errors, discoverer);
}

fn navigation(editor: &mut Editor, ui: &mut egui::Ui, vertical: bool) {
    let controls = |ui: &mut egui::Ui| {
        for (index, name) in SECTIONS.iter().enumerate() {
            ui.selectable_value(&mut editor.section, index, *name);
        }
    };
    if vertical {
        ui.vertical(controls);
    } else {
        ui.horizontal_wrapped(controls);
    }
}

fn footer(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    close: &mut bool,
    errors: &[(String, String)],
    discoverer: &mut crate::fonts::Discoverer,
) {
    if !errors.is_empty() {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            format!("{} setting errors: fix them before saving.", errors.len()),
        );
    }
    if !editor.message.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("settings-notices")
            .max_height(42.0)
            .show(ui, |ui| {
                ui.label(&editor.message);
            });
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Restore defaults").clicked() {
            editor.draft.as_mut().unwrap().defaults();
            editor.message = "Defaults staged. Save to apply, or Cancel to discard.".into();
        }
        if ui.button("Cancel").clicked() {
            *close = true;
        }
        let can_save = errors.is_empty() && editor.store.is_some() && !editor.measuring;
        if ui
            .add_enabled(
                can_save,
                egui::Button::new(
                    egui::RichText::new("Save changes").color(egui::Color32::from_rgb(20, 35, 71)),
                )
                .fill(ACCENT),
            )
            .clicked()
        {
            editor.save_in_frame(discoverer, ui.ctx());
        }
        if ui
            .add_enabled(can_save, egui::Button::new("Save and close"))
            .clicked()
            && editor.save_in_frame(discoverer, ui.ctx())
        {
            *close = true;
        }
    });
    ui.weak("Changes apply on Save. Running programs stay open.");
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
    ui.label("Blinking still stops after 15 seconds without typing and only runs in the focused console and window.");
}

fn shortcuts(editor: &mut Editor, ui: &mut egui::Ui, errors: &[(String, String)]) {
    ui.label("Blank uses the factory chord. 'none' hands it back to the program. Copy is controlled separately.");
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
            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(field.key.trim_start_matches("key.").replace('-', " "));
                let action =
                    crate::keyboard::Action::from_written(field.key.trim_start_matches("key."));
                let hint = action
                    .and_then(|a| defaults.chord_for(a))
                    .map(written_chord)
                    .unwrap_or_default();
                ui.add(
                    egui::TextEdit::singleline(&mut field.value)
                        .hint_text(hint)
                        .desired_width(150.0),
                );
                if ui
                    .small_button("None")
                    .on_hover_text("Hand this shortcut back to the program")
                    .clicked()
                {
                    field.value = "none".into();
                }
                if ui.small_button("Default").clicked() {
                    field.value.clear();
                }
            });
        }
        field_errors(ui, &field.key, errors);
    }
}

fn field_errors(ui: &mut egui::Ui, key: &str, errors: &[(String, String)]) {
    for (_, error) in errors.iter().filter(|(candidate, _)| candidate == key) {
        ui.colored_label(egui::Color32::LIGHT_RED, error);
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
