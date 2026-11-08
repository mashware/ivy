//! Fixed save actions and bounded notices share one measured row layout.
use super::{ACCENT, Editor};
use eframe::egui;

pub(super) const FOOTER_NOTE: &str = "Changes apply when saved. Your running programs stay open.";

fn action_width(ui: &egui::Ui, label: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(14.0),
            ui.visuals().text_color(),
        )
        .size()
        .x
        + 28.0
}

fn footer_rows(ui: &egui::Ui, width: f32) -> Vec<Vec<(&'static str, f32)>> {
    let actions: Vec<_> = [
        "Restore defaults",
        "Cancel",
        "Save changes",
        "Save and close",
    ]
    .into_iter()
    .map(|label| (label, action_width(ui, label)))
    .collect();
    let total = |items: &[(&str, f32)]| {
        items.iter().map(|(_, width)| width).sum::<f32>()
            + 8.0 * items.len().saturating_sub(1) as f32
    };
    if total(&actions) <= width {
        return vec![actions];
    }
    if total(&actions[1..]) <= width {
        return vec![vec![actions[0]], actions[1..].to_vec()];
    }
    let mut rows: Vec<Vec<(&str, f32)>> = vec![Vec::new()];
    for action in actions {
        let row = rows.last_mut().unwrap();
        if !row.is_empty() && total(row) + 8.0 + action.1 > width {
            rows.push(vec![action]);
        } else {
            row.push(action);
        }
    }
    rows
}

pub(super) fn height(
    editor: &Editor,
    ui: &egui::Ui,
    width: f32,
    errors: &[(String, String)],
) -> f32 {
    let rows = footer_rows(ui, width).len() as f32;
    let note = ui
        .painter()
        .layout(
            FOOTER_NOTE.to_owned(),
            egui::FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
            width,
        )
        .size()
        .y;
    16.0 + rows * 40.0
        + (rows - 1.0) * 8.0
        + 10.0
        + note
        + 16.0
        + if !errors.is_empty() || !editor.message.is_empty() {
            50.0
        } else {
            0.0
        }
}

pub(super) fn show(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    close: &mut bool,
    errors: &[(String, String)],
    discoverer: &mut crate::fonts::Discoverer,
) {
    let bounds = ui.max_rect();
    let mut top = bounds.top() + 16.0;
    if !errors.is_empty() || !editor.message.is_empty() {
        let rect = egui::Rect::from_min_size(
            egui::pos2(bounds.left(), top),
            egui::vec2(bounds.width(), 42.0),
        );
        let mut notice_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("settings-notices")
                .max_rect(rect),
        );
        notice_ui.set_clip_rect(rect.intersect(ui.clip_rect()));
        // A newly invalid field must not disappear behind a previous notice's scroll offset.
        if !errors.is_empty() {
            notice_ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("{} setting errors: fix them before saving.", errors.len()),
            );
        }
        if !editor.message.is_empty() {
            let available = notice_ui.available_height().max(0.0);
            egui::ScrollArea::vertical()
                .id_salt("notice-scroll")
                .min_scrolled_height(0.0)
                .max_height(available)
                .auto_shrink([false, false])
                .show(&mut notice_ui, |ui| {
                    ui.label(&editor.message);
                });
        }
        top += 50.0;
    }
    let can_save = errors.is_empty() && editor.store.is_some() && !editor.measuring;
    let rows = footer_rows(ui, bounds.width());
    for row in &rows {
        let total = row.iter().map(|(_, width)| width).sum::<f32>()
            + 8.0 * row.len().saturating_sub(1) as f32;
        let mut left = if row[0].0 == "Restore defaults" {
            bounds.left()
        } else {
            bounds.right() - total
        };
        for (index, &(label, width)) in row.iter().enumerate() {
            if index == 1 && row[0].0 == "Restore defaults" && rows.len() == 1 {
                left = bounds.right() - (total - row[0].1 - 8.0);
            }
            let primary = label == "Save and close";
            let quiet = matches!(label, "Restore defaults" | "Cancel");
            let mut button =
                egui::Button::new(egui::RichText::new(label).size(14.0).color(if primary {
                    egui::Color32::from_rgb(22, 38, 77)
                } else if quiet {
                    egui::Color32::from_rgb(146, 155, 174)
                } else {
                    ui.visuals().text_color()
                }))
                .frame(!quiet);
            if primary {
                button = button.fill(ACCENT);
            }
            let rect = egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(width, 40.0));
            let mut button_ui = ui.new_child(egui::UiBuilder::new().id_salt(label).max_rect(rect));
            button_ui.set_clip_rect(rect.intersect(ui.clip_rect()));
            if !(quiet || can_save) {
                button_ui.disable();
            }
            if button_ui.add_sized(rect.size(), button).clicked() {
                match label {
                    "Restore defaults" => {
                        editor.draft.as_mut().unwrap().defaults();
                        editor.message =
                            "Defaults staged. Save to apply, or Cancel to discard.".into();
                    }
                    "Cancel" => *close = true,
                    "Save changes" => {
                        editor.save_in_frame(discoverer, ui.ctx());
                    }
                    _ => {
                        if editor.save_in_frame(discoverer, ui.ctx()) {
                            *close = true;
                        }
                    }
                }
            }
            left += width + 8.0;
        }
        top += 48.0;
    }
    let note = ui.painter().layout(
        FOOTER_NOTE.to_owned(),
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(129, 140, 162),
        bounds.width(),
    );
    ui.painter().galley(
        egui::pos2(bounds.left(), top + 2.0),
        note,
        egui::Color32::from_rgb(129, 140, 162),
    );
}
