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

/// The contrast text must keep over a surface to count as readable. A theme's own foreground is
/// kept as written even where one of its surfaces takes it under this (D94).
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

    /// Whether `colour` reads apart from every fill in `over`.
    pub(crate) fn stands_apart(colour: Color32, over: &[Color32]) -> bool {
        stands_apart(colour, over)
    }

    /// `role`, unless it vanishes into one of the fills it is painted over: then `from` stepped
    /// toward the text (see the free `apart`).
    pub(crate) fn apart(
        &self,
        role: Color32,
        over: &[Color32],
        from: Color32,
        amount: f32,
    ) -> Color32 {
        apart(role, over, from, self.tx, amount)
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
        // onto the background still gets a hover, a raised menu and a border that show (D98).
        // Only a foreground that had to be replaced changes anything here: for a theme whose
        // text reads, `tx` is the foreground and every surface is the design's mix — including
        // a narrow theme whose text sits under 4.5:1 on its own hover, as it did before.
        let corrected = tx != fg;
        let raised = if dark {
            text_surface(bg, tx, 0.06, corrected)
        } else {
            Color32::WHITE
        };
        let hover = text_surface(bg, tx, 0.09, corrected);
        let chrome = if dark {
            mix(bg, Color32::BLACK, 0.22)
        } else {
            text_surface(bg, tx, 0.06, corrected)
        };
        // The selection is a row text sits on, among the console, a menu and a hovered row; a
        // focus colour that lands it on any of them (the background itself, or a shade of the
        // menu) leaves it a step of the text instead — never the text itself.
        let sel = selection(
            mix(bg, accent, 0.18),
            &[bg, raised, hover],
            bg,
            tx,
            corrected,
        );
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
            sel,
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

/// `bg` stepped toward the text by `step`. Where the text had to be replaced (`may_turn`) and
/// that step would leave it unreadable on the result — a mid-grey background whose text became
/// black — the step goes the other way. A theme's own foreground is never second-guessed.
fn text_surface(bg: Color32, tx: Color32, step: f32, may_turn: bool) -> Color32 {
    let toward = mix(bg, tx, step);
    if !may_turn || project_ink_contrast(tx, toward) >= TEXT_CONTRAST {
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

/// Whether `colour` reads apart from every fill in `over` (at least [`INVISIBLE`]).
fn stands_apart(colour: Color32, over: &[Color32]) -> bool {
    over.iter()
        .all(|fill| project_ink_contrast(colour, *fill) >= INVISIBLE)
}

/// `role`, unless it vanishes into one of the fills it is painted over: then the nearest step of
/// `from` toward `tx` — from `amount` up, a point at a time — that stands apart from all of them
/// and from the text itself. Each step toward the text reads less well under it, so the nearest
/// is also the most readable. Where nothing between `from` and the text stands apart (the text
/// itself barely does), the first step.
fn apart(role: Color32, over: &[Color32], from: Color32, tx: Color32, amount: f32) -> Color32 {
    if stands_apart(role, over) && stands_apart(role, &[tx]) {
        return role;
    }
    (0..)
        .map(|n| amount + n as f32 / 100.0)
        .take_while(|step| *step < 0.95)
        .map(|step| mix(from, tx, step))
        .find(|c| stands_apart(*c, over) && stands_apart(*c, &[tx]))
        .unwrap_or_else(|| mix(from, tx, amount))
}

/// The selection: `tinted` when it stands apart from the fills it sits among, else an 18 % step of
/// the background toward the text (away from it only where the text was replaced and would not
/// read) — a surface text is drawn on, never the text colour itself.
fn selection(
    tinted: Color32,
    among: &[Color32],
    bg: Color32,
    tx: Color32,
    corrected: bool,
) -> Color32 {
    if stands_apart(tinted, among) {
        return tinted;
    }
    // The step, not a search: the selection is chosen to read as a step of the background, and
    // a further fallback toward the text is what once returned the text colour itself.
    text_surface(bg, tx, 0.18, corrected)
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
            } else {
                assert!(
                    project_ink_contrast(r.chrome, r.base) >= 1.03,
                    "{hex:06x} chrome"
                );
            }
            assert!(
                project_ink_contrast(r.tx, r.sel) >= 4.5,
                "{hex:06x} text on sel"
            );
        }
    }

    /// A theme whose own foreground reads, however narrowly, keeps the design's mixes: its menus
    /// stay a step lighter than the console, never darker.
    #[test]
    fn a_readable_foreground_keeps_the_design_mixes() {
        let rgb = |hex: u32| Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
        let theme = Theme {
            background: rgb(0x002b36),
            foreground: rgb(0x839496),
            ..Theme::default()
        };
        let r = Roles::of(&theme);
        assert_eq!(r.tx, rgb(0x839496));
        assert_eq!(r.raised, rgb(0x08313c));
        assert_eq!(r.hover, rgb(0x0c343f));
    }

    /// `apart` keeps a role that stands out; otherwise it takes its first step when that stands
    /// apart, a further step when a fill blocks the first, and never the text itself.
    #[test]
    fn apart_takes_the_nearest_visible_step_and_never_the_text() {
        let grey = |v: u8| Color32::from_rgb(v, v, v);
        let (bg, tx) = (grey(0x20), grey(0xe0));
        let keep = grey(0x60);
        assert_eq!(apart(keep, &[bg], bg, tx, 0.2), keep);
        // The first step stands apart from the background alone: it is the answer.
        assert_eq!(apart(bg, &[bg], bg, tx, 0.2), mix(bg, tx, 0.2));
        // A fill on the first step pushes it further, still a fill and short of the text.
        let first = mix(bg, tx, 0.2);
        let moved = apart(bg, &[bg, first], bg, tx, 0.2);
        assert!(stands_apart(moved, &[bg, first]));
        assert!(moved.r() > first.r() && moved.r() < tx.r(), "{moved:?}");
        // The nearest: a few levels past the blocked step, not far toward the text.
        assert!(
            moved.r() - first.r() <= 8,
            "{moved:?} is not the nearest step"
        );
        // A text one level off the fill leaves no colour between them that stands apart: the
        // first step, never the text or a shade of it.
        let near = grey(0x7a);
        assert_eq!(
            apart(grey(0x77), &[grey(0x77)], grey(0x77), near, 0.2),
            grey(0x78)
        );
        // A wider pair: the step stands apart from the fill and from the text both.
        let (fill, text) = (grey(0x60), grey(0x90));
        let step = apart(fill, &[fill], fill, text, 0.2);
        assert!(
            stands_apart(step, &[fill]) && stands_apart(step, &[text]),
            "{step:?}"
        );
        // Near black a step of the text is far apart: with the fills crowding the steps up to
        // 0x06, the only steps left that stand apart from them read as the text 0x10 itself, so
        // the answer is the first step, not a shade of the text.
        let (dark, dark_text) = (grey(0x00), grey(0x10));
        let crowd: Vec<Color32> = (0x00..=0x06).map(grey).collect();
        assert_eq!(
            apart(dark, &crowd, dark, dark_text, 0.2),
            mix(dark, dark_text, 0.2)
        );
        // Nothing stands apart (the fills cover every step): the first step, not the text.
        let all: Vec<Color32> = (0..=255).map(grey).collect();
        assert_eq!(apart(bg, &all, bg, tx, 0.2), first);
    }

    /// A narrow foreground with the focus on the background: the selection moves off the
    /// background but stays a surface, never the text colour drawn on it.
    #[test]
    fn a_moved_selection_is_never_the_text() {
        let rgb = |hex: u32| Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
        for (bg, fg) in [
            (0x000000, 0x777777),
            (0xfafafa, 0x707070),
            (0x050505, 0x7d7d7d),
        ] {
            let theme = Theme {
                background: rgb(bg),
                foreground: rgb(fg),
                focus: rgb(bg),
                ..Theme::default()
            };
            let r = Roles::of(&theme);
            assert!(
                project_ink_contrast(r.sel, r.base) >= 1.03,
                "{bg:06x} sel on base"
            );
            assert!(
                project_ink_contrast(r.tx, r.sel) >= 3.0,
                "{bg:06x} text on sel"
            );
        }
    }

    /// A theme's own narrow foreground is not second-guessed for the selection either: with the
    /// focus on the background, Solarized Dark's selection steps lighter like its menus, not into a
    /// hole darker than the console.
    #[test]
    fn a_moved_selection_steps_toward_a_readable_themes_text() {
        let rgb = |hex: u32| Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
        let theme = Theme {
            background: rgb(0x002b36),
            foreground: rgb(0x839496),
            focus: rgb(0x002b36),
            ..Theme::default()
        };
        let r = Roles::of(&theme);
        assert_eq!(r.sel, mix(rgb(0x002b36), rgb(0x839496), 0.18));
    }

    /// A focus colour that lands the selection on a menu's own surface moves the selection, so
    /// the chosen row still shows.
    #[test]
    fn a_selection_that_lands_on_the_menu_is_moved() {
        let ivy = Roles::of(&Theme::default());
        // The accent whose 18 % tint is `raised`: a third of the way from the background to the
        // text.
        let theme = Theme {
            focus: mix(ivy.base, ivy.tx, 0.06 / 0.18),
            ..Theme::default()
        };
        let r = Roles::of(&theme);
        for (name, fill) in [("base", r.base), ("raised", r.raised), ("hover", r.hover)] {
            assert!(project_ink_contrast(r.sel, fill) >= 1.03, "sel on {name}");
        }
        assert!(project_ink_contrast(r.tx, r.sel) >= 4.5, "text on sel");
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
