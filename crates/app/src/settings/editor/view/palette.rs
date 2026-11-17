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
    pub error: Color32,
    /// The text cursor and a pressed control's border: the accent, or the ink where the accent
    /// would vanish. (A control's border and the hover fill are written into `roles` as `line2`
    /// and `hover`.)
    pub caret: Color32,
    /// The theme's roles with those corrections written back, so every field egui derives from
    /// them — the dialog's outline, a field's border, the selection — inherits the correction.
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
        let mut fixed = roles;
        fixed.tx = ink;
        fixed.tx3 = readable(roles.tx3, surface, 4.5);
        // The navigation sits on the person's own `panel` colour; where that leaves its text
        // unreadable it falls back to the dialog's surface rather than hide the way back.
        let nav = roles.panel_or(ink, surface);
        // Hover is painted on the dialog and, as the chosen segment, on a field's track (`base`).
        fixed.hover = fixed.apart(roles.hover, &[surface, roles.base], surface, 0.10);
        // The selection is painted over a field (`base`), the dialog, the navigation and a hovered
        // row alike, so it has to stand apart from every one of them.
        fixed.sel = fixed.apart(
            roles.sel,
            &[fixed.hover, roles.base, surface, nav],
            surface,
            0.20,
        );
        fixed.line2 = fixed.apart(roles.line2, &[surface], surface, 0.40);
        // The caret — the text cursor and a pressed control's border — is drawn over fields, the
        // dialog, the selection, a pressed control's hover fill and the navigation: the accent where
        // it stands apart from all of them, else the ink.
        let under = [roles.base, surface, fixed.sel, fixed.hover, nav];
        let caret = if Roles::stands_apart(roles.accent, &under) {
            roles.accent
        } else {
            ink
        };
        Self {
            surface,
            nav,
            ink,
            accent: roles.accent,
            selected_ink: roles.on_accent,
            caret,
            error: readable(roles.block, surface, 4.5),
            roles: fixed,
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
        v.error_fg_color = self.error;
        v.warn_fg_color = self.error;
        // Selected text, and egui's focus ring around a field, are drawn in this ink.
        v.selection.stroke.color = readable(self.ink, self.roles.sel, 4.5);
        v.widgets.active.bg_stroke.color = self.caret;
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
        // Its edge is its own fill, moved toward the ink only where it vanishes into the surface,
        // so a collapsed accent still leaves the button findable.
        widget.bg_stroke = egui::Stroke::new(
            1.0,
            palette
                .roles
                .apart(fill, &[palette.surface], palette.surface, 0.40),
        );
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
            // A normal theme's caret is its accent, not the ink.
            assert_eq!(p.caret, rgb(accent), "{id}");
            // A focus set to the hover shade or the navigation's colour would hide a pressed
            // control's border on that fill: the caret is then the ink.
            for (fill, name) in [(p.roles.hover, "hover"), (p.nav, "nav")] {
                let focus = format!("#{:02x}{:02x}{:02x}", fill.r(), fill.g(), fill.b());
                let theme = super::super::appearance::effective_theme(&Document::new(format!(
                    "theme={id}\nfocus={focus}\n"
                )));
                let q = Palette::new(theme);
                assert_eq!(q.caret, q.ink, "{id}: caret over {name}");
                for under in [q.roles.sel, q.roles.hover, q.nav] {
                    assert!(
                        project_ink_contrast(q.caret, under) >= 1.03,
                        "{id}: the ink caret over {name} vanishes"
                    );
                }
            }
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
                // Where a control is, the selection and the caret stay findable over every fill
                // they are painted on (D98). The floor is written out, not read from the code.
                let base = Roles::of(&theme).base;
                let apart = |a: Color32, over: &[Color32]| {
                    over.iter().all(|b| project_ink_contrast(a, *b) >= 1.03)
                };
                assert!(apart(p.roles.hover, &[p.surface, base]), "{name:?} hover");
                assert!(apart(p.roles.line2, &[p.surface]), "{name:?} edge");
                assert!(
                    apart(p.roles.sel, &[p.roles.hover, base, p.surface, p.nav]),
                    "{name:?} sel"
                );
                assert!(
                    apart(
                        p.caret,
                        &[base, p.surface, p.roles.sel, p.roles.hover, p.nav]
                    ),
                    "{name:?} caret"
                );
                assert_eq!(p.frame().stroke.color, p.roles.line2, "{name:?} outline");
                egui::Context::default()
                    .run_ui(Default::default(), |ui| {
                        p.apply(ui);
                        assert_eq!(ui.visuals().widgets.inactive.bg_stroke.color, p.roles.line2);
                        assert_eq!(ui.visuals().window_stroke.color, p.roles.line2);
                        assert_eq!(ui.visuals().selection.bg_fill, p.roles.sel);
                        assert_eq!(ui.visuals().text_cursor.stroke.color, p.caret);
                        primary_style(ui, &p);
                        let primary = ui.visuals().widgets.inactive.bg_stroke.color;
                        assert!(project_ink_contrast(primary, p.surface) >= 1.03, "{name:?}");
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
