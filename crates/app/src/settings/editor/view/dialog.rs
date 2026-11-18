//! The chrome's small dialogs, laid out like Settings (D98): a header with the title and a close
//! button, the body, and a footer whose primary action sits right-most on the accent.
use super::palette::{Palette, primary_style};
use eframe::egui;

/// The title and the close button over a rule, centred on one line. True when the close button
/// was clicked. The title takes the kit's floating heading, not Settings' 25: these dialogs are a
/// single field or a guide, not a page of sections (#119). It is cut short rather than run under
/// the close button when the dialog is narrow.
pub(crate) fn header(ui: &mut egui::Ui, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close = crate::ui::icons::close_button(ui).clicked();
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(title).size(crate::ui::floating::HEADING))
                        .truncate(),
                );
            });
        });
    });
    ui.separator();
    close
}

/// What the person did with the rename dialog in this pass. `Save` is only ever a valid name, and
/// when one pass carries several (a click and a key), the terminal title wins, then saving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    FollowTitle,
    Save,
    Cancel,
}

/// A footer action's type size: Settings' (footer.rs).
const ACTION_TEXT: f32 = 14.0;

/// A footer action, as tall as Settings' (footer.rs) and in its type size.
fn action(label: &str, framed: bool) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(label).size(ACTION_TEXT))
        .frame(framed)
        .min_size(egui::vec2(0.0, 40.0))
}

/// The width a footer action takes: its label, plus the button's padding when it has a frame. Only
/// framed buttons are padded here because egui 0.36 lays a frameless one out with no margin; the
/// 320-point case of the narrow-window test (`tests/pane_rename.rs`) goes red if that changes.
fn action_width(ui: &egui::Ui, label: &str, framed: bool) -> f32 {
    let text = ui
        .painter()
        .layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(ACTION_TEXT),
            egui::Color32::PLACEHOLDER,
        )
        .size()
        .x;
    text + if framed {
        2.0 * ui.spacing().button_padding.x
    } else {
        0.0
    }
}

/// The rename dialog of a tab (`console == false`) or a console. `draft` is edited in place;
/// applying the outcome is the caller's.
pub(crate) fn rename(
    ctx: &egui::Context,
    theme: &crate::theme::Theme,
    title: &str,
    draft: &mut String,
    console: bool,
) -> Option<Outcome> {
    let palette = Palette::of(ctx, theme);
    let available = ctx.content_rect();
    // The chrome's other dialogs size against the content rectangle too (connection.rs).
    let width = (available.width() - 80.0).clamp(80.0, 380.0);
    let (mut follow_title, mut save, mut cancel) = (false, false, false);
    egui::Window::new(title)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .constrain_to(available)
        .movable(false)
        .frame(palette.frame().inner_margin(20.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            palette.apply(ui);
            // An auto-sized window settles on whatever its right-aligned rows leave it; the
            // dialog's width is chosen here instead.
            ui.set_width(width);
            cancel = header(ui, title);
            ui.add_space(4.0);
            if console {
                ui.weak("A manual name overrides the terminal title. Leave empty to follow it.");
            }
            let field = ui.add(
                egui::TextEdit::singleline(draft)
                    // Settings' 36-point controls (D98), reached through the margin around the
                    // 15-point body.
                    .margin(egui::Margin::symmetric(10, 10))
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
            // Two points of slack: egui rounds each widget's rect on its own, so a sum that lands
            // exactly on the width can still overflow by a fraction once laid out.
            let inline = console
                && action_width(ui, "Use terminal title", false)
                    + action_width(ui, "Cancel", false)
                    + action_width(ui, "Save", true)
                    + 2.0 * ui.spacing().item_spacing.x
                    + 2.0
                    <= ui.available_width();
            if console && !inline {
                // Too narrow for the three: the quiet one takes its own row, as Settings' footer
                // puts Restore defaults on its own (footer.rs).
                follow_title = ui.add(action("Use terminal title", false)).clicked();
            }
            ui.horizontal(|ui| {
                if inline {
                    follow_title = ui.add(action("Use terminal title", false)).clicked();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.scope(|ui| {
                        primary_style(ui, &palette);
                        save = ui.add_enabled(valid, action("Save", true)).clicked();
                    });
                    cancel |= ui.add(action("Cancel", false)).clicked();
                });
            });
            save = (save || ui.input(|i| i.key_pressed(egui::Key::Enter))) && valid;
            cancel |= ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    if follow_title {
        Some(Outcome::FollowTitle)
    } else if save {
        Some(Outcome::Save)
    } else if cancel {
        Some(Outcome::Cancel)
    } else {
        None
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Palette;
    use eframe::egui;

    /// The muted note ink and the error ink Settings paints with for `theme`.
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
}
