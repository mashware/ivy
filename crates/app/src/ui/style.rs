//! egui's style, written from the theme's roles.
//!
//! **Both of egui's styles are written, and the preference pinned to the theme's own darkness.**
//! egui keeps a dark and a light style and picks one by following the desktop unless told otherwise,
//! so writing only «the» style would let a desktop switch to light swap egui's stock style in over
//! the theme the person chose.

use crate::theme::{Roles, Theme, metrics};
use eframe::egui::{self, CornerRadius, Stroke};

/// Installs the style for `theme`, once per change of theme.
///
/// The theme last installed is kept in egui's own memory rather than in the window, and the
/// comparison is the whole of the work on a frame where nothing changed.
pub(crate) fn install(ctx: &egui::Context, theme: &Theme) {
    let id = egui::Id::new("ivy-installed-theme");
    if ctx.data(|d| d.get_temp::<Theme>(id)) == Some(*theme) {
        return;
    }
    let roles = Roles::of(theme);
    ctx.all_styles_mut(|style| apply(style, &roles));
    ctx.set_theme(if roles.dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    ctx.data_mut(|d| d.insert_temp(id, *theme));
}

/// The roles of `theme`, memoized for the last theme asked about in egui's memory.
///
/// Hand-painted surfaces ask for them every frame, and deriving them walks the contrast search of
/// every text role; the comparison against the theme last asked about is the whole of the work on a
/// frame where nothing changed.
pub(crate) fn roles(ctx: &egui::Context, theme: &Theme) -> Roles {
    let id = egui::Id::new("ivy-roles");
    if let Some((cached, roles)) = ctx.data(|d| d.get_temp::<(Theme, Roles)>(id))
        && cached == *theme
    {
        return roles;
    }
    let roles = Roles::of(theme);
    ctx.data_mut(|d| d.insert_temp(id, (*theme, roles)));
    roles
}

/// Writes the roles into one style.
pub(crate) fn apply(style: &mut egui::Style, roles: &Roles) {
    let v = &mut style.visuals;
    *v = if roles.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.override_text_color = None;
    v.weak_text_color = Some(roles.tx3);
    v.hyperlink_color = roles.done;
    v.warn_fg_color = roles.wait;
    v.error_fg_color = roles.block;
    v.panel_fill = roles.base;
    v.window_fill = roles.raised;
    // Fields are sunk into the console's own background; buttons are raised above it.
    v.extreme_bg_color = roles.base;
    v.text_edit_bg_color = Some(roles.base);
    v.faint_bg_color = roles.panel;
    v.code_bg_color = roles.base;
    v.window_stroke = Stroke::new(1.0, roles.line2);
    v.window_corner_radius = CornerRadius::same(metrics::RADIUS_FLOATING);
    v.menu_corner_radius = CornerRadius::same(metrics::RADIUS_FLOATING);
    v.window_shadow = roles.shadow;
    v.popup_shadow = roles.shadow;
    v.selection.bg_fill = roles.sel;
    v.selection.stroke = Stroke::new(1.0, roles.tx);
    v.text_cursor.stroke = Stroke::new(2.0, roles.accent);
    let control = CornerRadius::same(metrics::RADIUS_CONTROL);
    v.widgets.noninteractive.bg_fill = roles.base;
    v.widgets.noninteractive.weak_bg_fill = roles.base;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, roles.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, roles.tx);
    v.widgets.noninteractive.corner_radius = control;
    for (widget, fill, border, ink) in [
        (&mut v.widgets.inactive, roles.raised, roles.line2, roles.tx),
        (&mut v.widgets.hovered, roles.hover, roles.line2, roles.tx),
        (&mut v.widgets.active, roles.hover, roles.accent, roles.tx),
        (&mut v.widgets.open, roles.hover, roles.line2, roles.tx),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, border);
        widget.fg_stroke = Stroke::new(1.0, ink);
        widget.corner_radius = control;
        widget.expansion = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::NamedTheme;
    use eframe::egui::Color32;

    fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    /// The style's fills are the theme's roles, written out, and both egui styles carry them — so a
    /// desktop that flips to light does not bring egui's stock light style back.
    #[test]
    fn both_styles_carry_the_themes_roles_whatever_the_desktop_prefers() {
        let ctx = egui::Context::default();
        install(&ctx, &Theme::default());
        for slot in [egui::Theme::Dark, egui::Theme::Light] {
            let style = ctx.style_of(slot);
            assert_eq!(style.visuals.window_fill, rgb(0x202427), "{slot:?}");
            assert_eq!(style.visuals.extreme_bg_color, rgb(0x13181b), "{slot:?}");
            assert_eq!(
                style.visuals.widgets.hovered.bg_fill,
                rgb(0x262b2d),
                "{slot:?}"
            );
            assert_eq!(
                style.visuals.widgets.active.bg_stroke.color,
                rgb(0x3cc384),
                "{slot:?}"
            );
            assert_eq!(style.visuals.hyperlink_color, rgb(0x72b7f2), "{slot:?}");
        }
        assert_eq!(
            ctx.options(|o| o.theme_preference),
            egui::ThemePreference::Dark
        );
    }

    /// A change of theme is installed on the next call; the same theme twice writes nothing new.
    #[test]
    fn a_new_theme_replaces_the_style_and_the_same_one_is_left_alone() {
        let ctx = egui::Context::default();
        install(&ctx, &Theme::default());
        install(&ctx, &NamedTheme::IvyDay.theme());
        assert_eq!(ctx.global_style().visuals.window_fill, Color32::WHITE);
        assert_eq!(ctx.global_style().visuals.extreme_bg_color, rgb(0xfafbf8));
        assert_eq!(
            ctx.options(|o| o.theme_preference),
            egui::ThemePreference::Light
        );
        // Somebody else writing the style is not undone by installing the same theme again.
        ctx.all_styles_mut(|s| s.visuals.window_fill = rgb(0x010203));
        install(&ctx, &NamedTheme::IvyDay.theme());
        assert_eq!(ctx.global_style().visuals.window_fill, rgb(0x010203));
    }
}
