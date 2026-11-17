use crate::theme::{Roles, Theme, mix, project_ink_contrast};
use eframe::egui::{self, Color32};

/// Settings' colors: the roles of the theme the dialog previews (D96, superseding D67's neutral
/// palette), with the dialog's ink kept readable when a configuration collapsed the palette —
/// Settings is the one place that configuration can be undone from.
#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub surface: Color32,
    pub nav: Color32,
    pub ink: Color32,
    pub accent: Color32,
    pub selected_ink: Color32,
    pub hover: Color32,
    pub active: Color32,
    pub error: Color32,
    roles: Roles,
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

/// `role`, unless a collapsed palette made it the surface it must stand out from: then moved toward
/// the ink, so a hovered or pressed control still shows it.
fn distinct(role: Color32, from: Color32, ink: Color32, amount: f32) -> Color32 {
    if role == from { mix(from, ink, amount) } else { role }
}

impl Palette {
    pub fn new(theme: Theme) -> Self {
        let roles = Roles::of(&theme);
        let surface = roles.raised;
        Self {
            surface,
            nav: roles.panel,
            ink: readable(roles.tx, surface, 4.5),
            accent: roles.accent,
            selected_ink: roles.on_accent,
            hover: distinct(roles.hover, surface, roles.tx, 0.10),
            active: distinct(roles.sel, distinct(roles.hover, surface, roles.tx, 0.10), roles.tx, 0.20),
            error: readable(roles.block, surface, 4.5),
            roles,
        }
    }

    pub fn frame(self) -> egui::Frame {
        crate::ui::floating::dialog_frame(&self.roles)
    }

    pub fn apply(self, ui: &mut egui::Ui) {
        let style = ui.style_mut();
        crate::ui::style::apply(style, &self.roles);
        // The dialog keeps its own measures (D67's native review); only its colors are the kit's.
        super::settings_geometry(style);
        let v = &mut style.visuals;
        v.panel_fill = self.nav;
        v.weak_text_color = Some(readable(self.roles.tx3, self.surface, 4.5));
        v.error_fg_color = self.error;
        v.warn_fg_color = self.error;
        v.selection.stroke.color = readable(self.ink, self.active, 4.5);
        for (widget, fill) in [
            (&mut v.widgets.hovered, self.hover),
            (&mut v.widgets.active, self.hover),
            (&mut v.widgets.open, self.hover),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
        }
        v.selection.bg_fill = self.active;
        for widget in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            widget.fg_stroke.color = readable(self.ink, widget.bg_fill, 4.5);
        }
    }
}

pub(super) fn popup_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .corner_radius(crate::theme::metrics::RADIUS_FLOATING)
        .shadow(ui.visuals().popup_shadow)
        .inner_margin(10)
}

/// The primary action: filled with the accent, its label in the ink the accent carries.
pub(super) fn primary_style(ui: &mut egui::Ui, palette: &Palette) {
    let (accent, ink) = (palette.accent, palette.selected_ink);
    let v = &mut ui.style_mut().visuals;
    for (widget, amount) in [
        (&mut v.widgets.inactive, 0.0),
        (&mut v.widgets.hovered, 0.10),
        (&mut v.widgets.active, 0.20),
    ] {
        let fill = mix(accent, ink, amount);
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = egui::Stroke::new(1.0, fill);
        widget.fg_stroke.color = readable(ink, fill, 4.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::document::Document;
    use crate::theme::NamedTheme;

    fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    /// Settings shows the kit's roles of the theme it previews, written out, and not a palette of
    /// its own (D96).
    #[test]
    fn settings_is_painted_with_the_previewed_themes_roles() {
        for (id, raised, panel, line2, accent, on_accent) in [
            ("ivy-night", 0x202427, 0x171d21, 0x373b3d, 0x3cc384, 0x000000),
            ("ivy-day", 0xffffff, 0xf2f5f0, 0xd3d5d2, 0x0b7a4b, 0xffffff),
        ] {
            let theme = super::super::appearance::effective_theme(&Document::new(format!(
                "theme={id}\n"
            )));
            let p = Palette::new(theme);
            assert_eq!(
                [p.surface, p.nav, p.roles.line2, p.accent, p.selected_ink],
                [raised, panel, line2, accent, on_accent].map(rgb),
                "{id}"
            );
            assert_eq!(p.frame().fill, rgb(raised));
            assert_eq!(p.frame().stroke.color, rgb(line2));
        }
    }

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
                // The accent is the person's focus colour verbatim (D94); only ink is corrected.
                assert_eq!(p.accent, theme.focus(), "{name:?}");
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
