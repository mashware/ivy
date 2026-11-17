//! What floating surfaces share: menus, dialogs and the docked viewers, built from the roles of the
//! theme they show.
//!
//! These surfaces used to start from egui's stock dark (or light) look and paint a palette of their
//! own over it — Settings kept a neutral one on purpose (D67). That stock start is what let the
//! desktop's light/dark switch reach them, and it is gone: every one now reads the roles (D98).

use crate::theme::{Roles, Theme, metrics};
use eframe::egui::{self, CornerRadius, FontId, Stroke, TextStyle};

/// Writes `theme`'s roles and the floating surfaces' spacing into `ui`'s own style.
///
/// The theme is passed rather than read from the installed style because the surfaces that call it
/// do not all show the live theme: Settings shows its draft, the Board its derived theme.
pub(crate) fn style(ui: &mut egui::Ui, theme: &Theme) {
    let roles = crate::ui::style::roles(ui.ctx(), theme);
    apply(ui.style_mut(), &roles);
}

pub(crate) fn apply(style: &mut egui::Style, roles: &Roles) {
    crate::ui::style::apply(style, roles);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    for (role, size) in [
        (TextStyle::Body, 13.0),
        (TextStyle::Button, 13.0),
        (TextStyle::Small, 11.0),
        (TextStyle::Heading, 20.0),
    ] {
        style.text_styles.insert(role, FontId::proportional(size));
    }
}

/// A menu, a dropdown or a popup list.
pub(crate) fn menu_frame(roles: &Roles) -> egui::Frame {
    egui::Frame::new()
        .fill(roles.raised)
        .stroke(Stroke::new(1.0, roles.line2))
        .corner_radius(CornerRadius::same(metrics::RADIUS_FLOATING))
        .shadow(roles.shadow)
        .inner_margin(4)
}

/// A modal dialog. Its content lays out its own margins: Settings' navigation runs to the edge.
pub(crate) fn dialog_frame(roles: &Roles) -> egui::Frame {
    egui::Frame::new()
        .fill(roles.raised)
        .stroke(Stroke::new(1.0, roles.line2))
        .corner_radius(CornerRadius::same(metrics::RADIUS_DIALOG))
        .shadow(roles.shadow)
        .inner_margin(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::NamedTheme;
    use eframe::egui::Color32;

    fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    /// A floating surface takes the theme it is handed, whatever egui was told about the desktop.
    #[test]
    fn a_floating_surface_reads_its_theme_and_not_the_desktop() {
        for preference in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
            let ctx = egui::Context::default();
            ctx.set_theme(preference);
            for (theme, raised, base, line2, ink) in [
                (Theme::default(), 0x202427, 0x13181b, 0x373b3d, 0xe4e8e2),
                (NamedTheme::IvyDay.theme(), 0xffffff, 0xfafbf8, 0xd3d5d2, 0x151d19),
            ] {
                ctx.run_ui(Default::default(), |ui| {
                    style(ui, &theme);
                    let v = ui.visuals();
                    assert_eq!(v.window_fill, rgb(raised), "{preference:?}");
                    assert_eq!(v.extreme_bg_color, rgb(base), "{preference:?}");
                    assert_eq!(v.widgets.inactive.bg_stroke.color, rgb(line2));
                    assert_eq!(v.widgets.inactive.fg_stroke.color, rgb(ink));
                    assert_eq!(ui.spacing().interact_size.y, 28.0);
                })
                .drop_without_applying_deltas();
            }
        }
    }

    #[test]
    fn menus_and_dialogs_float_on_raised_with_a_control_border() {
        let roles = Roles::of(&Theme::default());
        let menu = menu_frame(&roles);
        let dialog = dialog_frame(&roles);
        for frame in [menu, dialog] {
            assert_eq!(frame.fill, rgb(0x202427));
            assert_eq!(frame.stroke, Stroke::new(1.0, rgb(0x373b3d)));
            assert_eq!(frame.shadow, roles.shadow);
        }
        assert_eq!(menu.corner_radius, CornerRadius::same(10));
        assert_eq!(dialog.corner_radius, CornerRadius::same(12));
    }
}
