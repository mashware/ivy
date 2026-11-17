//! The interface's colors as **roles** derived from whatever theme is in effect (D94).
//!
//! A role names what a color is for — the chrome behind the tabs, a raised menu, muted text — and
//! never a hex value, so a person who picks Nord or writes their own twenty-two colors gets an
//! interface built from that palette instead of one painted over it. The table is the design kit's;
//! each rule is written next to the role it produces.
//!
//! It is a pure function of a [`Theme`] rather than a cache refreshed when the theme changes,
//! because the themes it will be asked about differ: the live one, the Settings draft, the Board's
//! derived one.

use super::{Theme, mix, project_ink_contrast};
use eframe::egui::{Color32, epaint::Shadow};

/// The minimum contrast every text role keeps over every role surface text is set on.
const TEXT_CONTRAST: f32 = 4.5;

/// Below this contrast two colours read as one. The kit's own quietest step — Ivy Night's `hover`
/// over `raised`, about 1.09:1 — sits above it on purpose, so only a collapsed palette is moved.
pub(crate) const INVISIBLE: f32 = 1.03;

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
    /// Text: the theme's foreground, or black or white when a configuration made it unreadable.
    pub tx: Color32,
    /// Secondary text: 30 % toward the background, brought back until it reads.
    pub tx2: Color32,
    /// Muted text: 46 % toward the background, brought back until it reads.
    pub tx3: Color32,
    /// Focus and the primary action.
    pub accent: Color32,
    /// Text on `accent`.
    pub on_accent: Color32,
    /// The accent as a translucent tint, painted over a surface: an active background.
    pub accent_soft: Color32,
    pub wait: Color32,
    /// Translucent tint over a surface, like every `*_soft`.
    pub wait_soft: Color32,
    pub block: Color32,
    pub block_soft: Color32,
    pub done: Color32,
    pub done_soft: Color32,
    /// Translucent tints over the content background for an added and a deleted diff line.
    pub add: Color32,
    pub del: Color32,
    /// A translucent veil over the window behind a modal dialog.
    pub scrim: Color32,
    /// The one shadow floating surfaces cast.
    pub shadow: Shadow,
    /// Private, so a `Roles` exists only through [`Roles::of`] and its contrast rules.
    _derived: (),
}

/// The kit's measures, in points. The ones not listed here (control heights) arrive with the
/// surfaces that use them.
pub(crate) mod metrics {
    pub const RADIUS_CHIP: u8 = 4;
    pub const RADIUS_CONTROL: u8 = 6;
    pub const RADIUS_CARD: u8 = 8;
    pub const RADIUS_FLOATING: u8 = 10;
    /// A modal dialog (Settings), one step rounder than a menu.
    pub const RADIUS_DIALOG: u8 = 12;
    /// The title bar, which holds the tabs: the design's single 40-point strip.
    pub const TITLE_BAR_HEIGHT: f32 = 40.0;
    /// A tab inside the title bar, sitting on its bottom edge.
    pub const TAB_HEIGHT: f32 = 34.0;
    pub const CONSOLE_HEADER_HEIGHT: f32 = 28.0;
    /// A row of a floating menu or list.
    pub const MENU_ROW_HEIGHT: f32 = 30.0;
}

impl Roles {
    /// `panel` for a surface that sets `ink` on it, or `fallback` where the person's own `panel`
    /// colour would leave that ink unreadable — `panel` is configured apart from the text.
    pub(crate) fn panel_or(&self, ink: Color32, fallback: Color32) -> Color32 {
        if project_ink_contrast(ink, self.panel) >= TEXT_CONTRAST {
            self.panel
        } else {
            fallback
        }
    }

    /// `role`, unless it vanishes into one of the fills it is painted over: then `from` stepped
    /// toward the text until it stands apart from all of them.
    pub(crate) fn apart(
        &self,
        role: Color32,
        over: &[Color32],
        from: Color32,
        amount: f32,
    ) -> Color32 {
        let stands = |c: Color32| {
            over.iter()
                .all(|f| project_ink_contrast(c, *f) >= INVISIBLE)
        };
        if stands(role) {
            return role;
        }
        [amount, amount * 2.0, 0.6]
            .into_iter()
            .map(|step| mix(from, self.tx, step))
            .find(|c| stands(*c))
            .unwrap_or(self.tx)
    }

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
        let panel = theme.panel_background;
        // The surfaces step toward the readable text, not the raw foreground: for any theme whose
        // text reads they are the same colour, and a configuration that collapsed the foreground
        // onto the background still gets a hover, a raised menu and a border that show. A step
        // that would take a text surface under the text's own contrast goes the other way.
        let raised = if dark {
            text_surface(bg, tx, 0.06)
        } else {
            Color32::WHITE
        };
        let hover = text_surface(bg, tx, 0.09);
        // A focus colour set equal to the background would leave the selection on it.
        let selection_tint = if project_ink_contrast(accent, bg) < INVISIBLE {
            tx
        } else {
            accent
        };
        let chrome = if dark {
            mix(bg, Color32::BLACK, 0.22)
        } else {
            mix(bg, fg, 0.06)
        };
        // Secondary and muted text must read on the worst of the surfaces the roles themselves
        // name. A surface painted from them — a console header tinted with its project — brings
        // its text back on its own (D96).
        let surfaces = [bg, panel, raised, hover, chrome];
        Self {
            dark,
            chrome,
            base: bg,
            panel,
            raised,
            hover,
            sel: mix(bg, selection_tint, 0.18),
            line: theme.divider,
            line2: mix(bg, tx, 0.17),
            tx,
            tx2: faded(tx, bg, 30, &surfaces),
            tx3: faded(tx, bg, 46, &surfaces),
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
            _derived: (),
        }
    }
}

/// `bg` stepped toward the text by `step`, or away from it where that would leave the text
/// unreadable on the result (a mid-grey background whose text was moved to black).
fn text_surface(bg: Color32, tx: Color32, step: f32) -> Color32 {
    let toward = mix(bg, tx, step);
    if project_ink_contrast(tx, toward) >= TEXT_CONTRAST {
        return toward;
    }
    let away =
        if project_ink_contrast(tx, Color32::WHITE) < project_ink_contrast(tx, Color32::BLACK) {
            Color32::BLACK
        } else {
            Color32::WHITE
        };
    mix(bg, away, step)
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

/// Text `percent` of the way toward the background, brought back toward the text one percent at a
/// time until it reads on every surface. At zero it is the text itself, which `readable_or_extreme`
/// already made readable on the background.
fn faded(tx: Color32, bg: Color32, percent: u8, surfaces: &[Color32]) -> Color32 {
    let reads = |ink: Color32| {
        surfaces
            .iter()
            .all(|surface| project_ink_contrast(ink, *surface) >= TEXT_CONTRAST)
    };
    (0..=percent)
        .rev()
        .map(|p| mix(tx, bg, f32::from(p) / 100.0))
        .find(|candidate| reads(*candidate))
        .unwrap_or(tx)
}

#[cfg(test)]
mod collapse_tests {
    use super::*;

    /// A foreground (and focus) collapsed onto the background still gives surfaces that show:
    /// hover, raised, the selection and the control border stand apart from the background and
    /// from each other.
    #[test]
    fn a_collapsed_foreground_still_leaves_surfaces_apart() {
        for hex in [0x777777u32, 0x000000, 0xffffff, 0x202020] {
            let c = Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
            let theme = Theme {
                background: c,
                foreground: c,
                focus: c,
                ..Theme::default()
            };
            let r = Roles::of(&theme);
            for (name, a, b) in [
                ("hover/base", r.hover, r.base),
                ("sel/base", r.sel, r.base),
                ("line2/base", r.line2, r.base),
                ("sel/hover", r.sel, r.hover),
            ] {
                assert!(project_ink_contrast(a, b) >= 1.03, "{hex:06x} {name}");
            }
            if r.dark {
                assert!(
                    project_ink_contrast(r.raised, r.base) >= 1.03,
                    "{hex:06x} raised"
                );
                assert!(
                    project_ink_contrast(r.hover, r.raised) >= 1.03,
                    "{hex:06x} hover/raised"
                );
            }
        }
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
        ] {
            assert_eq!(role, rgb(derived));
            assert!(distance(role, rgb(css)) <= 10, "{role:?} vs #{css:06x}");
        }
        // Muted text is brought back from 46 % to 41 % so it reads at 4.5:1 on hover, the lightest
        // surface it is set on; the design's #7E8A85 reads at about 4.0 there.
        assert_eq!(r.tx3, rgb(0x8e9390));
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
        // Forty-six percent toward this background does not reach 4.5:1 on hover, so muted text is
        // brought back to 34 %; secondary text reads at its 30 % as it is.
        assert_eq!(r.tx2, rgb(0x5a605c));
        assert_eq!(r.tx3, rgb(0x636865));
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
        // Secondary and muted text read on every surface text is set on, and never more than the
        // role above them: a secondary text fainter than muted text inverts the hierarchy.
        let text_reads = |r: &Roles, label: &str| {
            for surface in [r.base, r.panel, r.raised, r.hover, r.chrome] {
                for ink in [r.tx2, r.tx3] {
                    assert!(
                        project_ink_contrast(ink, surface) >= TEXT_CONTRAST,
                        "{label}: {ink:?} on {surface:?}"
                    );
                }
            }
            let over_base = |ink| project_ink_contrast(ink, r.base);
            assert!(over_base(r.tx) >= over_base(r.tx2), "{label}");
            assert!(over_base(r.tx2) >= over_base(r.tx3), "{label}");
        };
        for &named in NamedTheme::ALL {
            let theme = named.theme();
            let r = Roles::of(&theme);
            text_reads(&r, &format!("{named:?}"));
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
                text_reads(&r, &format!("{named:?} {hex:06x}"));
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
