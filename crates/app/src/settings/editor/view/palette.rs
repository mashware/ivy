use crate::theme::{Roles, Theme, mix, project_ink_contrast};
use eframe::egui::{self, Color32};

/// Settings' colors: the roles of the theme the dialog previews (D98, superseding D67's neutral
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
    /// A control's border, and the caret and focus ring over a field: the kit's roles, moved only
    /// where a collapsed palette made them vanish.
    pub edge: Color32,
    pub caret: Color32,
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

/// Below this contrast two colours read as one. The kit's own quietest step — Ivy Night's `hover`
/// over `raised`, about 1.08:1 — sits above it on purpose, so only a collapsed palette is moved.
const INVISIBLE: f32 = 1.03;

/// `role`, unless a collapsed palette left it indistinguishable from the surface it must stand out
/// from: then moved toward the ink, so a hovered control, a field's edge or the caret still shows.
fn distinct(role: Color32, from: Color32, ink: Color32, amount: f32) -> Color32 {
    if project_ink_contrast(role, from) < INVISIBLE {
        mix(from, ink, amount)
    } else {
        role
    }
}

impl Palette {
    #[cfg(test)]
    pub fn new(theme: Theme) -> Self {
        Self::from_roles(Roles::of(&theme))
    }

    /// The palette of `theme`, its roles taken from the frame's memo.
    pub fn of(ctx: &egui::Context, theme: &Theme) -> Self {
        Self::from_roles(crate::ui::style::roles(ctx, theme))
    }

    fn from_roles(roles: Roles) -> Self {
        let surface = roles.raised;
        let ink = readable(roles.tx, surface, 4.5);
        let hover = distinct(roles.hover, surface, ink, 0.10);
        // The navigation sits on the person's own `panel` colour; where that leaves its text
        // unreadable it falls back to the dialog's surface rather than hide the way back.
        let nav = if project_ink_contrast(ink, roles.panel) >= 4.5 {
            roles.panel
        } else {
            surface
        };
        Self {
            surface,
            nav,
            ink,
            accent: roles.accent,
            selected_ink: roles.on_accent,
            hover,
            active: distinct(roles.sel, hover, ink, 0.20),
            edge: distinct(roles.line2, surface, ink, 0.40),
            caret: distinct(roles.accent, roles.base, ink, 1.0),
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
        for widget in [&mut v.widgets.inactive, &mut v.widgets.hovered] {
            widget.bg_stroke.color = self.edge;
        }
        v.widgets.active.bg_stroke.color = self.caret;
        v.widgets.open.bg_stroke.color = self.edge;
        v.text_cursor.stroke.color = self.caret;
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

/// A dropdown inside Settings: the kit's menu, drawn from the dialog's own (draft) style.
pub(super) fn popup_frame(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::menu(ui.style())
        .fill(ui.visuals().window_fill)
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.inactive.bg_stroke.color,
        ))
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
        // Its edge is the control border, so a collapsed accent still leaves the button findable.
        widget.bg_stroke =
            egui::Stroke::new(1.0, distinct(fill, palette.surface, palette.ink, 0.40));
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
    /// its own (D98).
    #[test]
    fn settings_is_painted_with_the_previewed_themes_roles() {
        for (id, raised, panel, line2, accent, on_accent) in [
            (
                "ivy-night",
                0x202427,
                0x171d21,
                0x373b3d,
                0x3cc384,
                0x000000,
            ),
            ("ivy-day", 0xffffff, 0xf2f5f0, 0xd3d5d2, 0x0b7a4b, 0xffffff),
        ] {
            let theme =
                super::super::appearance::effective_theme(&Document::new(format!("theme={id}\n")));
            let p = Palette::new(theme);
            assert_eq!(
                [p.surface, p.nav, p.roles.line2, p.accent, p.selected_ink],
                [raised, panel, line2, accent, on_accent].map(rgb),
                "{id}"
            );
            assert_eq!(p.frame().fill, rgb(raised));
            assert_eq!(p.frame().stroke.color, rgb(line2));
        }
        // A `panel` its text cannot be read on leaves the navigation on the dialog's surface.
        let unreadable = super::super::appearance::effective_theme(&Document::new(
            "theme=ivy-night\npanel=#e4e8e2\n".into(),
        ));
        let p = Palette::new(unreadable);
        assert_eq!(p.nav, p.surface);
        assert!(project_ink_contrast(p.ink, p.nav) >= 4.5);
    }

    #[test]
    fn every_catalog_theme_and_collapsed_overrides_keep_readable_states() {
        for name in NamedTheme::ALL {
            for overrides in [
                "",
                "background=#777777\npanel=#777777\nforeground=#777777\nfocus=#777777\ndivider=#777777\ncolor.red=#777777\n",
                "background=#ffffff\npanel=#ffffff\nforeground=#ffffff\nfocus=#ffffff\ndivider=#ffffff\n",
                "background=#000000\npanel=#000000\nforeground=#000000\nfocus=#000000\ndivider=#000000\n",
                // Nearly collapsed: one step apart is not apart for the eye.
                "background=#777777\npanel=#777777\nforeground=#787878\nfocus=#777777\ndivider=#777777\n",
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
                // Where a control is, and where the caret is, stays findable (D98).
                let base = Roles::of(&theme).base;
                assert!(
                    project_ink_contrast(p.hover, p.surface) >= INVISIBLE,
                    "{name:?}"
                );
                assert!(
                    project_ink_contrast(p.active, p.hover) >= INVISIBLE,
                    "{name:?}"
                );
                assert!(
                    project_ink_contrast(p.edge, p.surface) >= INVISIBLE,
                    "{name:?}"
                );
                assert!(project_ink_contrast(p.caret, base) >= INVISIBLE, "{name:?}");
                egui::Context::default()
                    .run_ui(Default::default(), |ui| {
                        p.apply(ui);
                        assert_eq!(ui.visuals().widgets.inactive.bg_stroke.color, p.edge);
                        assert_eq!(ui.visuals().text_cursor.stroke.color, p.caret);
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
