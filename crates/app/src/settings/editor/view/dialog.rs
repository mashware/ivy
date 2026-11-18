//! The chrome's small dialogs, laid out like Settings (D98): a header with the title and a close
//! button, the body, and a footer whose primary action sits right-most on the accent.
use super::palette::{Palette, primary_style};
use eframe::egui;

/// The title and the close button over a rule. True when the close button was clicked.
pub(crate) fn header(ui: &mut egui::Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal_top(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            close = crate::ui::icons::close_button(ui).clicked();
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.heading(title);
            });
        });
    });
    ui.separator();
    close
}

/// The muted note ink and the error ink Settings paints with for `theme`.
#[cfg(test)]
pub(crate) fn inks(
    ctx: &egui::Context,
    theme: &crate::theme::Theme,
) -> (egui::Color32, egui::Color32) {
    let palette = Palette::of(ctx, theme);
    let mut inks = None;
    egui::Context::default()
        .run_ui(egui::RawInput::default(), |ui| {
            palette.apply(ui);
            inks = Some((ui.visuals().weak_text_color(), ui.visuals().error_fg_color));
        })
        .drop_without_applying_deltas();
    inks.unwrap()
}

/// What the person did with the rename dialog in this pass.
#[derive(Default)]
pub(crate) struct Rename {
    pub save: bool,
    pub cancel: bool,
    pub follow_title: bool,
}

/// A footer action, as tall as Settings' (footer.rs) and in its type size.
fn action(label: &str, framed: bool) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(label).size(14.0))
        .frame(framed)
        .min_size(egui::vec2(0.0, 40.0))
}

/// The rename dialog of a tab (`console == false`) or a console. `draft` is edited in place;
/// applying the result is the caller's.
pub(crate) fn rename(
    ctx: &egui::Context,
    theme: &crate::theme::Theme,
    title: &str,
    draft: &mut String,
    console: bool,
) -> Rename {
    let palette = Palette::of(ctx, theme);
    let width = 380.0_f32.min((ctx.viewport_rect().width() - 80.0).max(200.0));
    let mut out = Rename::default();
    egui::Window::new(title)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .constrain_to(ctx.viewport_rect())
        .movable(false)
        .frame(palette.frame().inner_margin(20.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            palette.apply(ui);
            // Auto-sized: the right-to-left header and footer would otherwise grow to fill the
            // viewport.
            ui.set_width(width);
            out.cancel = header(ui, title);
            ui.add_space(4.0);
            if console {
                ui.weak("A manual name overrides the terminal title. Leave empty to follow it.");
            }
            let field = ui.add(
                egui::TextEdit::singleline(draft)
                    // `min_size` does not reach the painted box; the margin around Settings' 15-point
                    // body is what makes the field as tall as the 36-point controls.
                    .margin(egui::Margin::symmetric(10, 10))
                    .min_size(egui::vec2(0.0, 36.0))
                    .desired_width(ui.available_width()),
            );
            field.request_focus();
            let valid = crate::session::valid_label(draft);
            if !valid {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    "Use at most 128 characters, without control characters.",
                );
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if console {
                    out.follow_title = ui.add(action("Use terminal title", false)).clicked();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.scope(|ui| {
                        primary_style(ui, &palette);
                        out.save = ui.add_enabled(valid, action("Save", true)).clicked();
                    });
                    out.cancel |= ui.add(action("Cancel", false)).clicked();
                });
            });
            out.save |= ui.input(|i| i.key_pressed(egui::Key::Enter));
            out.cancel |= ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    out
}
