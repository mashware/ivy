//! The interface's colors as **roles** derived from whatever theme is in effect (D94).
//!
//! A role names what a color is for — the chrome behind the tabs, a raised menu, muted text — and
//! never a hex value, so a person who picks Nord or writes their own twenty-two colors gets an
//! interface built from that palette instead of one painted over it. The table is the design kit's;
//! each rule is written next to the role it produces.
//!
//! It is a pure function of a [`Theme`] and is called wherever a theme is painted — the live one, the
//! Settings draft, the Board's derived one — because a single cache could not serve all three.

use super::{Theme, mix, project_ink_contrast};
use eframe::egui::{Color32, epaint::Shadow};

/// The minimum contrast every text role keeps over the console background.
const TEXT_CONTRAST: f32 = 4.5;

/// The interface's colors, derived from a theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Roles {
    /// Whether the theme is a dark one, judged by which ink reads better on its background.
    pub dark: bool,
    /// The title bar and inactive tabs: one step behind the console.
    pub chrome: Color32,
    /// The console background, which is also the background of content (diffs, documents).
    pub base: Color32,
    /// The status panel and side lists.
    pub panel: Color32,
    /// Floating surfaces: menus, dropdowns, dialogs, secondary buttons.
    pub raised: Color32,
    /// Hover, and the fill of a selected segment.
    pub hover: Color32,
    /// A selected row.
    pub sel: Color32,
    /// Separators.
    pub line: Color32,
    /// Control borders.
    pub line2: Color32,
    pub tx: Color32,
    pub tx2: Color32,
    /// Muted text, never under 4.5:1 over `base`.
    pub tx3: Color32,
    /// Focus and the primary action.
    pub accent: Color32,
    /// Text on `accent`.
    pub on_accent: Color32,
    pub accent_soft: Color32,
    pub wait: Color32,
    pub wait_soft: Color32,
    pub block: Color32,
    pub block_soft: Color32,
    pub done: Color32,
    pub done_soft: Color32,
    /// The background of an added and a deleted diff line.
    pub add: Color32,
    pub del: Color32,
    /// What dims the window behind a modal dialog.
    pub scrim: Color32,
    /// The one shadow floating surfaces cast.
    pub shadow: Shadow,
}

/// The kit's measures, in points. Heights are what a control, a list row or a bar occupies.
pub(crate) mod metrics {
    pub const SPACING: [f32; 7] = [2.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0];
    pub const RADIUS_CHIP: u8 = 4;
    pub const RADIUS_CONTROL: u8 = 6;
    pub const RADIUS_CARD: u8 = 8;
    pub const RADIUS_FLOATING: u8 = 10;
    pub const RADIUS_DIALOG: u8 = 12;
    pub const CONTROL_HEIGHT: f32 = 28.0;
    pub const ROW_HEIGHT: f32 = 30.0;
    pub const TAB_HEIGHT: f32 = 34.0;
    pub const CONSOLE_HEADER_HEIGHT: f32 = 28.0;
    pub const TITLE_BAR_HEIGHT: f32 = 40.0;
}

impl Roles {
    pub(crate) fn of(theme: &Theme) -> Self {
        let bg = theme.background;
        let fg = theme.foreground;
        let dark =
            project_ink_contrast(Color32::WHITE, bg) >= project_ink_contrast(Color32::BLACK, bg);
        let accent = theme.focus;
        let tx = readable_or_extreme(fg, bg);
        let wait = theme.palette[3];
        let block = theme.palette[1];
        let done = theme.palette[4];
        Self {
            dark,
            chrome: if dark {
                mix(bg, Color32::BLACK, 0.22)
            } else {
                mix(bg, fg, 0.06)
            },
            base: bg,
            panel: theme.panel_background,
            raised: if dark {
                mix(bg, fg, 0.06)
            } else {
                Color32::WHITE
            },
            hover: mix(bg, fg, 0.09),
            sel: mix(bg, accent, 0.18),
            line: theme.divider,
            line2: mix(bg, fg, 0.17),
            tx,
            tx2: mix(tx, bg, 0.30),
            tx3: muted(tx, bg),
            accent,
            on_accent: extreme_for(accent),
            accent_soft: with_alpha(accent, 0.14),
            wait,
            wait_soft: with_alpha(wait, 0.15),
            block,
            block_soft: with_alpha(block, 0.15),
            done,
            done_soft: with_alpha(done, 0.15),
            add: with_alpha(accent, 0.13),
            del: with_alpha(block, 0.14),
            scrim: if dark {
                Color32::from_black_alpha(140)
            } else {
                with_alpha(mix(fg, Color32::BLACK, 0.5), 0.28)
            },
            shadow: Shadow {
                offset: [0, 16],
                blur: 40,
                spread: 0,
                color: if dark {
                    Color32::from_black_alpha(128)
                } else {
                    with_alpha(fg, 0.18)
                },
            },
        }
    }
}

/// The color with the given opacity, keeping its hue.
fn with_alpha(color: Color32, opacity: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (opacity * 255.0).round() as u8,
    )
}

/// Black or white, whichever reads better on `surface`.
fn extreme_for(surface: Color32) -> Color32 {
    if project_ink_contrast(Color32::WHITE, surface)
        >= project_ink_contrast(Color32::BLACK, surface)
    {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

/// The theme's text, unless a collapsed configuration made it unreadable on its own background —
/// then black or white, so Settings, the only place to undo that configuration, stays legible.
fn readable_or_extreme(ink: Color32, surface: Color32) -> Color32 {
    if project_ink_contrast(ink, surface) >= TEXT_CONTRAST {
        ink
    } else {
        extreme_for(surface)
    }
}

/// Muted text: 46 % of the way toward the background, brought back toward the text until it reads.
fn muted(tx: Color32, bg: Color32) -> Color32 {
    let mut amount = 0.46;
    loop {
        let candidate = mix(tx, bg, amount);
        if project_ink_contrast(candidate, bg) >= TEXT_CONTRAST || amount <= 0.0 {
            return candidate;
        }
        amount -= 0.02;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::NamedTheme;

    fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    /// The largest per-channel distance between two colors.
    fn distance(a: Color32, b: Color32) -> u8 {
        [
            a.r().abs_diff(b.r()),
            a.g().abs_diff(b.g()),
            a.b().abs_diff(b.b()),
        ]
        .into_iter()
        .max()
        .unwrap()
    }

    /// **Literal values, written out**: an expectation computed with the derivation would measure the
    /// table against itself. The mixed roles are what the table gives over Ivy Night's slots, and the
    /// second check says how far each lands from the design kit's hand-picked CSS — the table blends in
    /// gamma space and the CSS was tuned by eye, so they meet within ten per channel, not exactly.
    #[test]
    fn ivy_night_roles_are_the_design_kits() {
        let r = Roles::of(&Theme::default());
        assert!(r.dark);
        for (role, slot_backed) in [
            (r.base, 0x13181b),
            (r.panel, 0x171d21),
            (r.line, 0x252e33),
            (r.tx, 0xe4e8e2),
            (r.accent, 0x3cc384),
            (r.wait, 0xf2b84b),
            (r.block, 0xf27a66),
            (r.done, 0x72b7f2),
        ] {
            assert_eq!(role, rgb(slot_backed));
        }
        for (role, derived, css) in [
            (r.chrome, 0x0f1315, 0x0e1215),
            (r.raised, 0x202427, 0x1c2327),
            (r.hover, 0x262b2d, 0x232c31),
            (r.sel, 0x1a372e, 0x1f3a2e),
            (r.line2, 0x373b3d, 0x344047),
            (r.tx2, 0xa5aaa6, 0xa4aea9),
            (r.tx3, 0x848886, 0x7e8a85),
        ] {
            assert_eq!(role, rgb(derived));
            assert!(distance(role, rgb(css)) <= 10, "{role:?} vs #{css:06x}");
        }
        assert_eq!(r.on_accent, Color32::BLACK);
    }

    #[test]
    fn ivy_day_roles_are_the_design_kits() {
        let r = Roles::of(&NamedTheme::IvyDay.theme());
        assert!(!r.dark);
        assert_eq!(r.base, rgb(0xfafbf8));
        assert_eq!(r.panel, rgb(0xf2f5f0));
        assert_eq!(r.raised, Color32::WHITE);
        assert_eq!(r.line, rgb(0xdde3dc));
        assert_eq!(r.accent, rgb(0x0b7a4b));
        assert_eq!(r.wait, rgb(0x9a5b00));
        assert_eq!(r.block, rgb(0xb93a26));
        assert_eq!(r.done, rgb(0x1d63b0));
        assert_eq!(r.chrome, rgb(0xeceeeb));
        assert!(distance(r.chrome, rgb(0xe9ede8)) <= 10);
        assert_eq!(r.on_accent, Color32::WHITE);
        // Forty-six percent toward this background does not reach 4.5:1, so muted text is brought back.
        assert_ne!(r.tx3, rgb(0x7e8380));
        assert!(project_ink_contrast(r.tx3, r.base) >= TEXT_CONTRAST);
    }

    #[test]
    fn every_theme_and_collapsed_configuration_keeps_its_text_readable() {
        let collapsed = |hex: u32| crate::settings::Colors {
            palette: [Some(rgb(hex)); 16],
            background: Some(rgb(hex)),
            foreground: Some(rgb(hex)),
            cursor: Some(rgb(hex)),
            divider: Some(rgb(hex)),
            focus: Some(rgb(hex)),
            panel: Some(rgb(hex)),
        };
        for &named in NamedTheme::ALL {
            let theme = named.theme();
            let r = Roles::of(&theme);
            assert!(
                project_ink_contrast(r.tx, r.base) >= TEXT_CONTRAST,
                "{named:?}"
            );
            assert!(
                project_ink_contrast(r.tx3, r.base) >= TEXT_CONTRAST,
                "{named:?}"
            );
            assert!(
                project_ink_contrast(r.on_accent, r.accent) >= TEXT_CONTRAST,
                "{named:?}"
            );
            // Without an explicit focus the accent is never the `done` claim's color.
            assert_ne!(r.accent, r.done, "{named:?}");
            assert_ne!(r.hover, r.base, "{named:?}");
            for hex in [0x777777, 0xffffff, 0x000000] {
                let r = Roles::of(&theme.with_colors(&collapsed(hex)));
                assert!(
                    project_ink_contrast(r.tx, r.base) >= TEXT_CONTRAST,
                    "{named:?} {hex:06x}"
                );
                assert!(
                    project_ink_contrast(r.tx3, r.base) >= TEXT_CONTRAST,
                    "{named:?} {hex:06x}"
                );
                assert!(
                    project_ink_contrast(r.on_accent, r.accent) >= TEXT_CONTRAST,
                    "{named:?} {hex:06x}"
                );
            }
        }
    }

    /// A person's own `focus` is the accent verbatim, even where it reads poorly: only text roles are
    /// corrected, never a color somebody chose (D59).
    #[test]
    fn an_explicit_focus_is_the_accent_as_written() {
        for &named in NamedTheme::ALL {
            let theme = named.theme().with_colors(&crate::settings::Colors {
                focus: Some(rgb(0xff00ff)),
                ..Default::default()
            });
            assert_eq!(Roles::of(&theme).accent, rgb(0xff00ff), "{named:?}");
        }
    }
}
