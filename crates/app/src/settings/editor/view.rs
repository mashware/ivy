//! Grouped controls edit a draft; only Save writes, and application waits for restart.
use super::Editor;
use eframe::egui;
use std::time::Instant;

pub(super) fn show(
    editor: &mut Editor,
    ctx: &egui::Context,
    discoverer: &mut crate::fonts::Discoverer,
) {
    let mut close = false;
    let popup_was_open = egui::Popup::is_any_open(ctx);
    egui::Modal::new(egui::Id::new("terminal-settings")).show(ctx, |ui| {
        form(editor, ui, discoverer, &mut close);
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && !popup_was_open
        && !egui::Popup::is_any_open(ctx)
    {
        close = true;
    }
    if close {
        editor.draft = None;
        editor.store = None;
        editor.fonts = None;
        ctx.memory_mut(|m| m.stop_text_input());
    }
}

fn form(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    close: &mut bool,
) {
    ui.set_width((ui.ctx().content_rect().width() - 48.0).clamp(100.0, 620.0));
    ui.heading("Settings");
    ui.label("Saved changes apply after restarting agent-terminal.");
    ui.horizontal_wrapped(|ui| {
        for (index, label) in ["Appearance", "Cursor", "Shortcuts"].iter().enumerate() {
            ui.selectable_value(&mut editor.section, index, *label);
        }
    });
    ui.separator();
    let started = Instant::now();
    let errors = editor
        .draft
        .as_ref()
        .map(|d| d.errors())
        .unwrap_or_default();
    editor.timing.record_work(2, started.elapsed());
    let height = (ui.ctx().content_rect().height() - 300.0).max(80.0);
    egui::ScrollArea::vertical()
        .max_height(height)
        .show(ui, |ui| match editor.section {
            0 => appearance(editor, ui, discoverer, &errors),
            1 => cursor(editor, ui),
            _ => shortcuts(editor, ui, &errors),
        });
    ui.separator();
    if !editor.message.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("settings-notices")
            .max_height(80.0)
            .show(ui, |ui| {
                ui.label(&editor.message);
            });
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Restore defaults").clicked() {
            editor.draft.as_mut().unwrap().defaults();
            editor.message = "Defaults staged. Save to keep them, or Cancel to discard.".into();
        }
        let valid = editor.draft.as_ref().is_some_and(|d| d.errors().is_empty());
        let can_save = valid && editor.store.is_some() && !editor.measuring;
        if ui
            .add_enabled(can_save, egui::Button::new("Save"))
            .clicked()
        {
            editor.save();
        }
        if ui.button("Cancel").clicked() {
            *close = true;
        }
    });
}

fn appearance(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    discoverer: &mut crate::fonts::Discoverer,
    errors: &[(String, String)],
) {
    ui.label(
        "Blank values use the built-in defaults. Explicit colours override the selected theme.",
    );
    let draft = editor.draft.as_mut().expect("an open editor has a draft");
    for field in &mut draft.fields {
        if field.key.starts_with("key.") || field.key == "cursor.blink" {
            continue;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(label(&field.key));
            match field.key.as_str() {
                "theme" => {
                    let selected = if field.value.is_empty() {
                        "Built-in default"
                    } else {
                        &field.value
                    };
                    egui::ComboBox::from_id_salt("settings-theme")
                        .selected_text(selected)
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                ("", "Built-in default"),
                                ("tokyonight", "Tokyo Night"),
                                ("gruvbox-dark", "Gruvbox Dark"),
                            ] {
                                ui.selectable_value(&mut field.value, value.into(), label);
                            }
                        });
                }
                "font" => {
                    ui.add(
                        egui::TextEdit::singleline(&mut field.value)
                            .hint_text("Built-in default")
                            .desired_width(150.0)
                            .interactive(false),
                    );
                    let response = ui.button("Installed fonts...");
                    editor.font_rect = Some(response.rect);
                    if response.clicked() {
                        editor.font_requests += 1;
                        if editor.fonts.is_none() {
                            let started = Instant::now();
                            editor.fonts = Some(discoverer.installed_families());
                            editor.timing.record_work(3, started.elapsed());
                        }
                    }
                    if let Some(fonts) = &editor.fonts {
                        egui::ComboBox::from_id_salt("settings-font")
                            .selected_text("Choose family")
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut field.value,
                                    String::new(),
                                    "Built-in default",
                                );
                                for name in fonts {
                                    ui.selectable_value(&mut field.value, name.clone(), name);
                                }
                            });
                    }
                }
                _ => {
                    ui.add(egui::TextEdit::singleline(&mut field.value).desired_width(120.0));
                }
            }
        });
        field_errors(ui, &field.key, errors);
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
    ui.label("Blinking still stops after 15 seconds without typing and only runs in the focused console and window.");
}

fn shortcuts(editor: &mut Editor, ui: &mut egui::Ui, errors: &[(String, String)]) {
    ui.label("Blank uses the factory chord. 'none' hands it back to the program. Copy and paste cannot be rebound.");
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
                        .desired_width(180.0),
                );
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

fn label(key: &str) -> String {
    if let Some(index) = key.strip_prefix("color.") {
        return format!("Palette {index}");
    }
    match key {
        "size" => "Font size (points)".into(),
        "font" => "Font family".into(),
        "theme" => "Theme".into(),
        "panel" => "Information panel background".into(),
        _ => {
            let mut text = key.to_owned();
            text[..1].make_ascii_uppercase();
            text
        }
    }
}
