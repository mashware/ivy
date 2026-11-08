//! Draft-only font controls and an isolated, cached specimen family.
use super::{Editor, appearance::selector, field_errors};
use eframe::egui;
use std::time::Instant;

#[derive(Default)]
pub(crate) struct PreviewState {
    requested: Option<String>,
    family: Option<egui::FontFamily>,
    queued_pass: Option<u64>,
    error: Option<String>,
    attempts: usize,
    loaded: bool,
}

impl PreviewState {
    fn update(
        &mut self,
        name: &str,
        explicit: bool,
        fonts: &mut crate::fonts::Discoverer,
        ctx: &egui::Context,
    ) {
        if self.requested.as_deref() == Some(name) && (!explicit || self.loaded) {
            return;
        }
        self.requested = Some(name.to_owned());
        self.attempts += 1;
        self.loaded = explicit;
        match fonts.preview_font((!name.is_empty()).then_some(name), ctx, explicit) {
            Ok(family) => {
                self.family = Some(family);
                self.queued_pass = Some(ctx.cumulative_pass_nr());
                self.error = None;
                ctx.request_repaint();
            }
            Err(_) => {
                self.error = Some("This font cannot be previewed. Choose another family.".into());
            }
        }
    }

    fn font(&self, ctx: &egui::Context, size: f32) -> egui::FontId {
        let family = self.family.as_ref().filter(|_| {
            self.queued_pass
                .is_some_and(|pass| pass < ctx.cumulative_pass_nr())
        });
        egui::FontId::new(size, family.cloned().unwrap_or(egui::FontFamily::Monospace))
    }
}

pub(super) fn show(
    editor: &mut Editor,
    ui: &mut egui::Ui,
    fonts: &mut crate::fonts::Discoverer,
    errors: &[(String, String)],
) {
    ui.add_space(8.0);
    let mut explicit = false;
    let draft = editor.draft.as_mut().unwrap();
    ui.horizontal_wrapped(|ui| {
        ui.vertical(|ui| {
            let field = draft.fields.iter_mut().find(|f| f.key == "font").unwrap();
            ui.weak("Font family");
            let title = if field.value.is_empty() {
                "Built-in default"
            } else {
                &field.value
            };
            let width = (ui.available_width() - 190.0).max(120.0);
            let response = selector(ui, title, width);
            editor.font_rect = Some(response.rect);
            if response.clicked() {
                editor.font_requests += 1;
                response.request_focus();
                if editor.fonts.is_none() {
                    let started = Instant::now();
                    editor.fonts = Some(fonts.installed_families());
                    editor.timing.record_work(3, started.elapsed());
                }
            }
            egui::Popup::menu(&response)
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(35, 41, 54))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 81, 106)))
                        .corner_radius(9)
                        .inner_margin(10),
                )
                .width(width)
                .show(|ui| {
                    super::settings_style(ui);
                    let choices: Vec<&str> = std::iter::once("")
                        .chain(editor.fonts.iter().flatten().map(String::as_str))
                        .collect();
                    let direction = ui.input_mut(|i| {
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                            1
                        } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                            -1
                        } else {
                            0
                        }
                    });
                    if direction != 0 {
                        let current = choices
                            .iter()
                            .position(|name| name.eq_ignore_ascii_case(&field.value));
                        let index = current.map_or(
                            if direction > 0 { 0 } else { choices.len() - 1 },
                            |index| {
                                (index as isize + direction).clamp(0, choices.len() as isize - 1)
                                    as usize
                            },
                        );
                        field.value = choices[index].to_owned();
                        explicit = true;
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("settings-font-families")
                        .min_scrolled_height(
                            160.0_f32.min((ui.ctx().content_rect().height() - 180.0).max(64.0)),
                        )
                        .max_height(220.0)
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .show(ui, |ui| {
                            for name in choices {
                                let title = if name.is_empty() {
                                    "Built-in default"
                                } else {
                                    name
                                };
                                let selected = field.value.eq_ignore_ascii_case(name);
                                let row = ui.selectable_label(selected, title);
                                if direction != 0 && selected {
                                    row.scroll_to_me(Some(egui::Align::Center));
                                }
                                if row.clicked() {
                                    field.value = name.to_owned();
                                    explicit = true;
                                }
                            }
                        });
                    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                        ui.close();
                    }
                });
        });
        let size = draft.fields.iter_mut().find(|f| f.key == "size").unwrap();
        ui.vertical(|ui| {
            ui.weak("Size");
            let (rect, outer) =
                ui.allocate_exact_size(egui::vec2(113.0, 40.0), egui::Sense::click());
            ui.painter()
                .rect_filled(rect, 6.0, egui::Color32::from_rgb(32, 37, 49));
            ui.painter().rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(54, 62, 80)),
                egui::StrokeKind::Inside,
            );
            let font = egui::FontId::proportional(14.0);
            let line_height = ui.fonts_mut(|fonts| fonts.row_height(&font));
            let inner = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 10.0, rect.center().y - line_height / 2.0),
                egui::vec2(62.0, line_height),
            );
            let mut input = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("settings-font-size")
                    .max_rect(inner),
            );
            let response = input.add_sized(
                inner.size(),
                egui::TextEdit::singleline(&mut size.value)
                    .font(font)
                    .hint_text("14")
                    .frame(egui::Frame::new()),
            );
            if outer.clicked() {
                response.request_focus();
            }
            ui.painter().text(
                rect.right_center() - egui::vec2(10.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                "pt",
                egui::FontId::proportional(12.0),
                ui.visuals().weak_text_color(),
            );
        });
        if ui
            .vertical(|ui| {
                ui.add_space(
                    ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y,
                );
                ui.add_sized(
                    [52.0, 40.0],
                    egui::Button::new(
                        egui::RichText::new("Reset")
                            .size(14.0)
                            .color(ui.visuals().weak_text_color()),
                    )
                    .frame(false),
                )
                .clicked()
            })
            .inner
        {
            for field in &mut draft.fields {
                if matches!(field.key.as_str(), "font" | "size") {
                    field.value.clear();
                }
            }
            explicit = true;
        }
    });
    field_errors(ui, "font", errors);
    field_errors(ui, "size", errors);
    let name = draft
        .fields
        .iter()
        .find(|f| f.key == "font")
        .unwrap()
        .value
        .clone();
    let size = draft
        .fields
        .iter()
        .find(|f| f.key == "size")
        .unwrap()
        .value
        .parse::<f32>()
        .ok()
        .filter(|size| size.is_finite() && (4.0..=96.0).contains(size))
        .unwrap_or(14.0);
    let attempts = editor.preview.attempts;
    let started = Instant::now();
    editor.preview.update(&name, explicit, fonts, ui.ctx());
    if editor.preview.attempts != attempts {
        editor.timing.record_work(5, started.elapsed());
    }
    if let Some(error) = &editor.preview.error {
        ui.weak(error);
    }
    ui.add_space(10.0);
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(21, 25, 35))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 47, 63)))
        .corner_radius(7)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new("Aa Bb Cc 0123456789   > ~/workspace")
                    .font(editor.preview.font(ui.ctx(), size))
                    .color(ui.visuals().weak_text_color()),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pass(ctx: &egui::Context) {
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
    }
    fn metrics(ctx: &egui::Context, font: egui::FontId) -> egui::Vec2 {
        ctx.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap("MMMM".into(), font, egui::Color32::WHITE)
                .size()
        })
    }
    #[test]
    fn specimen_uses_real_selected_face_and_size_without_changing_terminal_or_reloading() {
        let ctx = egui::Context::default();
        pass(&ctx);
        let baseline = metrics(&ctx, egui::FontId::monospace(14.0));
        let mut fonts = crate::fonts::fixture_discoverer();
        let mut preview = PreviewState::default();
        preview.update("Ubuntu", true, &mut fonts, &ctx);
        // Named families are not used in the registration pass.
        assert_eq!(preview.font(&ctx, 14.0).family, egui::FontFamily::Monospace);
        pass(&ctx);
        let chosen = metrics(&ctx, preview.font(&ctx, 14.0));
        assert_ne!(chosen, baseline);
        assert_eq!(metrics(&ctx, egui::FontId::monospace(14.0)), baseline);
        assert!(metrics(&ctx, preview.font(&ctx, 49.0)).x > chosen.x);
        for _ in 0..3 {
            preview.update("Ubuntu", false, &mut fonts, &ctx);
            pass(&ctx);
        }
        assert_eq!(preview.attempts, 1);
        assert_eq!(metrics(&ctx, preview.font(&ctx, 14.0)), chosen);
    }
    #[test]
    fn merely_opening_and_reopening_font_preview_never_searches_the_machine() {
        let ctx = egui::Context::default();
        let mut fonts = crate::fonts::Discoverer::default();
        for name in ["", "A draft family not activated in the terminal"] {
            let mut preview = PreviewState::default();
            preview.update(name, false, &mut fonts, &ctx);
            pass(&ctx);
            assert!(!fonts.has_searched_the_machine());
            fonts.clear_preview(&ctx);
            pass(&ctx);
        }
    }
}
