//! The colours a project is told apart by (D96, amended by #141).
//!
//! Named colours rather than palette slots: once green (focus) and red, yellow and blue (the
//! wait, block and done claims) are kept out, a terminal palette has two hue families left, and the
//! owner reported four projects that read as two cyans and two pinks. Brown, maroon and white differ
//! from violet and pink by lightness, which no rule over hues alone would offer.

use super::{Theme, project_ink_contrast, project_inks_distinct};
use eframe::egui::Color32;

/// Every project ink reads at this ratio on the plain panel, as a non-text mark does.
const MARK_CONTRAST: f32 = 3.0;

/// How far (CIE76 ΔE) an ink keeps from the focus green and the three claim colours, so a project
/// never reads as a state. Measured on the twelve bundled themes: at 20 the pastel ones (Catppuccin
/// Mocha, Nord) still keep five or six inks.
const AWAY_FROM_MEANINGS: f32 = 20.0;

/// For a dark panel: violet, turquoise, pink, brown, white, maroon, sand, lavender-grey, then the
/// same names in a lighter second tone.
const ON_DARK: [u32; 15] = [
    0xb48cf0, 0x3fd0c0, 0xf08cc8, 0xc8946a, 0xe8e8e8, 0xd06a88, 0xd8c49a, 0x9c9cc8, //
    0xd8c4ff, 0xa4f2ea, 0xffc4e4, 0xe6b890, 0xf09ab4, 0xf2e4c4, 0xd0d0ee,
];

/// For a light panel: the same names, deeper, with a near-black slate for white; then a second
/// tone of each.
const ON_LIGHT: [u32; 16] = [
    0x7048c0, 0x0a7e74, 0xb8327e, 0x8a5a30, 0x3a4048, 0x8e2444, 0x7e6a3a, 0x5a5a8e, //
    0x9a70e0, 0x065a54, 0x8c1e5c, 0x6a4220, 0x60666e, 0x6a1430, 0x5e4e26, 0x40406e,
];

/// The inks projects take, in the order they are handed out. A colour that does not read at 3:1 on
/// the panel as written, that sits near what the theme's green or claims mean, or that is not
/// distinct from one already kept is dropped — never brought toward the text colour, which is what
/// washed the old inks out to near-white.
pub(crate) fn project_accents(theme: &Theme) -> Vec<Color32> {
    let panel = theme.panel_background();
    let dark =
        project_ink_contrast(Color32::WHITE, panel) >= project_ink_contrast(Color32::BLACK, panel);
    let meanings = [
        theme.palette[1],
        theme.palette[3],
        theme.palette[4],
        theme.focus(),
    ];
    let list: &[u32] = if dark { &ON_DARK } else { &ON_LIGHT };
    let mut kept: Vec<Color32> = Vec::new();
    for &value in list {
        let ink = Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8);
        if project_ink_contrast(ink, panel) >= MARK_CONTRAST
            && meanings
                .iter()
                .all(|meaning| delta_e(ink, *meaning) >= AWAY_FROM_MEANINGS)
            && kept.iter().all(|other| project_inks_distinct(ink, *other))
        {
            kept.push(ink);
        }
    }
    kept
}

/// CIE76 distance in Lab (D65): how different two colours look, which RGB distance is not.
pub(crate) fn delta_e(a: Color32, b: Color32) -> f32 {
    let (a, b) = (lab(a), lab(b));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn lab(c: Color32) -> [f32; 3] {
    let linear = |n: u8| {
        let x = f32::from(n) / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (linear(c.r()), linear(c.g()), linear(c.b()));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    [
        116.0 * f(y) - 16.0,
        500.0 * (f(x) - f(y)),
        200.0 * (f(y) - f(z)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::NamedTheme;

    fn rgb(value: u32) -> Color32 {
        Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
    }

    /// The owner's order on the factory theme, written out: violet, turquoise, pink, brown, white,
    /// maroon — and none of them near Ivy Night's red, yellow, blue or focus green.
    #[test]
    fn ivy_night_hands_out_the_named_colours_in_order() {
        let accents = project_accents(&NamedTheme::IvyNight.theme());
        assert_eq!(
            accents[..6],
            [
                rgb(0xb48cf0),
                rgb(0x3fd0c0),
                rgb(0xf08cc8),
                rgb(0xc8946a),
                rgb(0xe8e8e8),
                rgb(0xd06a88)
            ]
        );
        for ink in &accents {
            for meaning in [0xf27a66, 0xf2b84b, 0x72b7f2, 0x3cc384] {
                assert!(
                    delta_e(*ink, rgb(meaning)) >= 20.0,
                    "{ink:?} near {meaning:06x}"
                );
            }
        }
    }

    /// Every bundled theme: readable as written, away from its own meanings, pairwise distinct, and
    /// enough of them — eight on the two Ivy themes, five on the rest (Catppuccin Mocha's pastel
    /// claims sit where pink, maroon and sand would).
    #[test]
    fn every_theme_keeps_readable_distinct_inks_away_from_its_meanings() {
        for &named in NamedTheme::ALL {
            let theme = named.theme();
            let accents = project_accents(&theme);
            let floor = if matches!(named, NamedTheme::IvyNight | NamedTheme::IvyDay) {
                8
            } else {
                5
            };
            assert!(accents.len() >= floor, "{named:?}: only {}", accents.len());
            for (i, ink) in accents.iter().enumerate() {
                assert!(
                    project_ink_contrast(*ink, theme.panel_background()) >= 3.0,
                    "{named:?} {ink:?}"
                );
                for meaning in [
                    theme.palette[1],
                    theme.palette[3],
                    theme.palette[4],
                    theme.focus(),
                ] {
                    assert!(
                        delta_e(*ink, meaning) >= 20.0,
                        "{named:?} {ink:?} near {meaning:?}"
                    );
                }
                for other in &accents[..i] {
                    assert!(
                        project_inks_distinct(*ink, *other),
                        "{named:?} {ink:?} ~ {other:?}"
                    );
                }
            }
        }
    }

    /// The second tone comes after the first round, and an ink that cannot read is dropped rather
    /// than washed toward the text: on Ivy Night, turquoise comes back lighter after lavender.
    #[test]
    fn a_second_tone_follows_the_first_round_and_nothing_is_washed_out() {
        let theme = NamedTheme::IvyNight.theme();
        let accents = project_accents(&theme);
        assert_eq!(accents[7], rgb(0x9c9cc8));
        assert_eq!(accents[8], rgb(0xa4f2ea));
        let list: Vec<Color32> = ON_DARK.iter().map(|v| rgb(*v)).collect();
        assert!(
            accents.iter().all(|ink| list.contains(ink)),
            "an ink was altered: {accents:?}"
        );
    }

    /// A theme whose own claims sit on a list colour loses that colour: a red slot set to the
    /// maroon removes maroon, and nothing else moves into its place but the next name.
    #[test]
    fn a_claim_colour_on_a_list_colour_removes_it() {
        let mut palette = [None; 16];
        palette[1] = Some(rgb(0xd06a88));
        let theme = NamedTheme::IvyNight
            .theme()
            .with_colors(&crate::settings::Colors {
                palette,
                ..Default::default()
            });
        let accents = project_accents(&theme);
        assert!(!accents.contains(&rgb(0xd06a88)), "{accents:?}");
        assert_eq!(accents[5], rgb(0xd8c49a));
    }
}
