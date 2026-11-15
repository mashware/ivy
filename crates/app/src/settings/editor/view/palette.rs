use crate::theme::{Theme, project_ink_contrast};
use eframe::egui::{self, Color32};

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub surface: Color32,
    pub ink: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub selected_ink: Color32,
    pub hover: Color32,
    pub active: Color32,
    pub error: Color32,
}

fn readable(candidate: Color32, surface: Color32, minimum: f32) -> Color32 {
    if project_ink_contrast(candidate, surface) >= minimum {
        candidate
    } else if project_ink_contrast(Color32::WHITE, surface)
        > project_ink_contrast(Color32::BLACK, surface)
    {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

fn blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

impl Palette {
    pub fn new(theme: Theme) -> Self {
        let surface = blend(theme.background(), theme.panel_background(), 0.5);
        // Custom overrides can collapse all theme roles to one color. Correct presentation
        // ink and boundaries without altering the user's terminal colors or draft.
        let ink = readable(theme.foreground(), surface, 4.5);
        let accent = readable(theme.focus(), surface, 3.0);
        Self {
            surface,
            ink,
            border: readable(theme.divider(), surface, 3.0),
            accent,
            selected_ink: readable(theme.foreground(), accent, 4.5),
            hover: blend(surface, ink, 0.10),
            active: blend(surface, ink, 0.20),
            error: readable(theme.color(term_core::TermColor::Indexed(1)), surface, 4.5),
        }
    }

    pub fn frame(self) -> egui::Frame {
        egui::Frame::new()
            .fill(self.surface)
            .stroke(egui::Stroke::new(1.0, self.border))
            .corner_radius(14)
            .inner_margin(0)
    }

    pub fn apply(self, ui: &mut egui::Ui) {
        // Retain the existing Preferences geometry and typography.
        super::settings_style(ui);
        let v = &mut ui.style_mut().visuals;
        *v = if project_ink_contrast(Color32::BLACK, self.surface)
            > project_ink_contrast(Color32::WHITE, self.surface)
        {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        v.override_text_color = None;
        v.weak_text_color = Some(self.ink);
        v.panel_fill = self.surface;
        v.window_fill = self.surface;
        // Inputs use the body surface so inherited text and error ink share its contrast.
        v.extreme_bg_color = self.surface;
        v.faint_bg_color = self.surface;
        v.window_stroke = egui::Stroke::new(1.0, self.border);
        v.hyperlink_color = readable(self.accent, self.surface, 4.5);
        v.warn_fg_color = self.error;
        v.error_fg_color = self.error;
        v.selection.bg_fill = self.accent;
        v.selection.stroke = egui::Stroke::new(1.0, self.selected_ink);
        for (widget, fill) in [
            (&mut v.widgets.noninteractive, self.surface),
            (&mut v.widgets.inactive, self.surface),
            (&mut v.widgets.hovered, self.hover),
            (&mut v.widgets.active, self.active),
            (&mut v.widgets.open, self.hover),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = egui::Stroke::new(1.0, self.border);
            widget.fg_stroke.color = readable(self.ink, fill, 4.5);
            widget.corner_radius = egui::CornerRadius::same(6);
        }
        v.widgets.active.bg_stroke = egui::Stroke::new(2.0, self.accent);
        v.widgets.open.bg_stroke = egui::Stroke::new(2.0, self.accent);
        v.text_cursor.stroke.color = self.ink;
    }
}

pub(super) fn popup_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .corner_radius(9)
        .inner_margin(10)
}

pub(super) fn primary_style(ui: &mut egui::Ui) {
    let accent = ui.visuals().selection.bg_fill;
    let ink = ui.visuals().selection.stroke.color;
    let v = &mut ui.style_mut().visuals;
    for (widget, amount) in [
        (&mut v.widgets.inactive, 0.0),
        (&mut v.widgets.hovered, 0.10),
        (&mut v.widgets.active, 0.20),
    ] {
        let fill = blend(accent, ink, amount);
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.fg_stroke.color = readable(ink, fill, 4.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::document::Document;
    use crate::theme::NamedTheme;

    #[test]
    fn every_catalog_theme_and_collapsed_overrides_keep_readable_states() {
        for name in NamedTheme::ALL {
            for overrides in [
                "",
                "background=#777777\npanel=#777777\nforeground=#777777\nfocus=#777777\ndivider=#777777\ncolor.red=#777777\n",
                "background=#ffffff\npanel=#ffffff\nforeground=#ffffff\nfocus=#ffffff\ndivider=#ffffff\n",
                "background=#000000\npanel=#000000\nforeground=#000000\nfocus=#000000\ndivider=#000000\n",
            ] {
                let draft = Document::new(format!("theme={}\n{overrides}", name.id()));
                let theme = super::super::appearance::effective_theme(&draft);
                let p = Palette::new(theme);
                assert!(project_ink_contrast(p.ink, p.surface) >= 4.5, "{name:?}");
                assert!(project_ink_contrast(p.error, p.surface) >= 4.5, "{name:?}");
                assert!(
                    project_ink_contrast(p.selected_ink, p.accent) >= 4.5,
                    "{name:?}"
                );
                assert!(project_ink_contrast(p.border, p.surface) >= 3.0, "{name:?}");
                assert!(project_ink_contrast(p.accent, p.surface) >= 3.0, "{name:?}");
                assert_ne!(p.surface, p.hover);
                assert_ne!(p.hover, p.active);
                egui::Context::default()
                    .run_ui(Default::default(), |ui| {
                        p.apply(ui);
                        for widget in [
                            ui.visuals().widgets.inactive,
                            ui.visuals().widgets.hovered,
                            ui.visuals().widgets.active,
                            ui.visuals().widgets.open,
                        ] {
                            assert!(
                                project_ink_contrast(widget.fg_stroke.color, widget.bg_fill) >= 4.5
                            );
                        }
                    })
                    .drop_without_applying_deltas();
            }
        }
    }
}
