//! The claim as a band across the top of the panel (D97).
//!
//! **It replaces the claim's row of text only in the integrated chrome.** The historical panel —
//! what the batches without the integrated chrome paint — keeps that row; the integrated-chrome
//! batches paint this band, so their series are not comparable across D97 (D13).
//!
//! What it paints is the producer's declaration and nothing else (D18): the class picks the band,
//! the pill and the words, and whether somebody holds the claim decides between the live band and
//! the dimmed look. The age the design shows beside the words is not here: the panel holds no
//! moment at which it first observed a claim, and the last publication's date is not that moment.

use super::{AttentionState, attention_text, fold};
use crate::theme::{Roles, Theme};
use crate::ui::claim_pill;
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, vec2};
use panel_contract::Attention;

/// The band's height, the design kit's.
pub(super) const HEIGHT: f32 = 32.0;
/// What separates the pill from the panel's left edge and the words from its right one.
const PADDING: f32 = 12.0;
/// What separates the pill from the words.
const GAP: f32 = 8.0;
const TEXT_SIZE: f32 = 12.5;

/// Paints the strip at the top of `rect` and returns what is left below it for the rows.
///
/// **A band nobody holds gets no tint.** The tint is what reads as «this is live» from across the
/// window; a dimmed claim on a coloured band would say the opposite of its own words.
///
/// **The band gives way before the rows' last one does**: on a panel too short for the band and
/// one row of `cell`, it shrinks to leave that row — which is where «… N more» says what the panel
/// holds below — down to one row of its own, and only below that does the claim take the panel.
pub(super) fn paint(
    painter: &Painter,
    rect: Rect,
    cell: f32,
    claim: AttentionState,
    theme: &Theme,
    roles: &Roles,
) -> Rect {
    let height = HEIGHT
        .min(rect.height() - cell)
        .max(cell)
        .min(rect.height());
    let band = Rect::from_min_size(rect.min, vec2(rect.width(), height));
    if claim.backed {
        painter.rect_filled(band, 0.0, soft(claim.class, roles));
    }
    painter.line_segment(
        [
            Pos2::new(band.left(), band.bottom() - 0.5),
            Pos2::new(band.right(), band.bottom() - 0.5),
        ],
        Stroke::new(1.0, roles.line),
    );
    let slot = Rect::from_min_size(
        Pos2::new(band.left() + PADDING, band.top()),
        vec2(claim_pill::SIZE, band.height()),
    );
    claim_pill::paint(painter, slot, claim, roles);

    // **The words take `Theme::attention_color`**, the colour the historical row and the rest of the
    // application paint a claim with, so the two panels can never disagree on it.
    let ink = theme.attention_color(claim.class, claim.backed);
    let font = FontId::new(TEXT_SIZE, crate::fonts::ui_weight(painter.ctx(), 600));
    let left = slot.right() + GAP;
    let room = (band.right() - PADDING - left).max(0.0);
    let measure = |text: &str| {
        painter
            .layout_no_wrap(text.to_owned(), font.clone(), ink)
            .size()
            .x
    };
    // A band cannot wrap the way the old row did, so a narrow panel cuts the phrase rather than
    // letting it run under the console beside it — with «…», because «stuck on something» alone
    // would lose the «not you» that sets it apart; and with no words at all, only the pill, when not
    // even one letter fits beside the «…».
    let shown = fold::fit_marked(attention_text(claim.class), room, &measure);
    if !shown.is_empty() {
        painter.text(
            Pos2::new(left, band.center().y),
            Align2::LEFT_CENTER,
            shown,
            font,
            ink,
        );
    }
    Rect::from_min_max(Pos2::new(rect.left(), band.bottom()), rect.max)
}

/// The band's tint for each class: the role's own soft tone.
fn soft(class: Attention, roles: &Roles) -> Color32 {
    match class {
        Attention::Wait => roles.wait_soft,
        Attention::Block => roles.block_soft,
        Attention::Done => roles.done_soft,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Context, Shape};

    fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    fn tint(hex: u32) -> Color32 {
        Color32::from_rgba_unmultiplied((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 38)
    }

    const PANEL: Rect = Rect {
        min: Pos2::new(100.0, 50.0),
        max: Pos2::new(400.0, 500.0),
    };

    /// Paints the strip alone and returns its shapes and what it left for the rows.
    fn strip(claim: AttentionState, place: Rect) -> (Vec<Shape>, Rect) {
        let ctx = Context::default();
        let theme = Theme::default();
        let mut left = Rect::NOTHING;
        let output = ctx.run_ui(Default::default(), |ui| {
            left = paint(
                &ui.painter_at(place),
                place,
                17.0,
                claim,
                &theme,
                &Roles::of(&theme),
            );
        });
        let shapes = output.shapes.iter().map(|s| s.shape.clone()).collect();
        output.drop_without_applying_deltas();
        (shapes, left)
    }

    fn band_fill(shapes: &[Shape]) -> Option<(Rect, Color32)> {
        shapes.iter().find_map(|s| match s {
            Shape::Rect(r) if r.rect.width() == PANEL.width() => Some((r.rect, r.fill)),
            _ => None,
        })
    }

    fn words(shapes: &[Shape]) -> Vec<(String, Color32, Pos2)> {
        shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text(t) => Some((t.galley.text().to_owned(), t.fallback_color, t.pos)),
                _ => None,
            })
            .collect()
    }

    fn pill(shapes: &[Shape]) -> Option<(Rect, Color32)> {
        shapes.iter().find_map(|s| match s {
            Shape::Rect(r) if r.rect.width() == claim_pill::SIZE => Some((r.rect, r.fill)),
            _ => None,
        })
    }

    /// **Literal colours, written out**: Ivy Night's wait, block and done, their 15 % tints for the
    /// band and the solid pill (soft for done), and the band 32 points tall across the whole panel.
    #[test]
    fn each_live_claim_paints_its_tinted_band_its_pill_and_its_words() {
        for (class, solid, phrase) in [
            (Attention::Wait, 0xf2b84b, "it is waiting for you"),
            (
                Attention::Block,
                0xf27a66,
                "stuck on something else, not you",
            ),
            (Attention::Done, 0x72b7f2, "it finished and nobody looked"),
        ] {
            let (shapes, left) = strip(
                AttentionState {
                    class,
                    backed: true,
                },
                PANEL,
            );
            let band = Rect::from_min_max(PANEL.min, Pos2::new(PANEL.right(), PANEL.top() + 32.0));
            assert_eq!(band_fill(&shapes), Some((band, tint(solid))), "{class:?}");
            let (pill_rect, pill_fill) = pill(&shapes).expect("the pill is painted");
            assert_eq!(pill_rect.center().y, band.center().y, "{class:?}");
            assert_eq!(pill_rect.left(), PANEL.left() + 12.0, "{class:?}");
            let expected_pill = if class == Attention::Done {
                tint(solid)
            } else {
                rgb(solid)
            };
            assert_eq!(pill_fill, expected_pill, "{class:?}");
            let painted = words(&shapes);
            assert_eq!(painted.len(), 1, "{class:?}: {painted:?}");
            assert_eq!(painted[0].0, phrase, "{class:?}");
            assert_eq!(painted[0].1, rgb(solid), "{class:?}");
            assert!(
                band.contains(painted[0].2) && painted[0].2.x > pill_rect.right(),
                "{class:?}: the words sit in the band, after the pill: {painted:?}"
            );
            assert_eq!(
                left,
                Rect::from_min_max(Pos2::new(PANEL.left(), band.bottom()), PANEL.max),
                "{class:?}: the rows start below the band"
            );
        }
    }

    /// **A claim nobody holds keeps the dimmed look**: no tint on the band, the dashed pill, and the
    /// words in the dimmed colour the application already answers with.
    #[test]
    fn an_unbacked_claim_has_no_tint_and_dimmed_words() {
        let theme = Theme::default();
        for class in Attention::ALL {
            let (shapes, _) = strip(
                AttentionState {
                    class,
                    backed: false,
                },
                PANEL,
            );
            assert_eq!(band_fill(&shapes), None, "{class:?}");
            assert_eq!(pill(&shapes), None, "{class:?}: the pill is an outline");
            let painted = words(&shapes);
            assert_eq!(
                painted[0].1,
                theme.attention_color(class, false),
                "{class:?}"
            );
            assert_ne!(
                painted[0].1,
                theme.attention_color(class, true),
                "{class:?}"
            );
        }
    }

    /// A narrow panel cuts the phrase inside the band instead of running it under the console, and
    /// marks the cut with «…».
    #[test]
    fn a_narrow_panel_cuts_the_words_inside_the_band() {
        let narrow = Rect::from_min_size(PANEL.min, vec2(120.0, 300.0));
        let (shapes, _) = strip(
            AttentionState {
                class: Attention::Block,
                backed: true,
            },
            narrow,
        );
        let painted = words(&shapes);
        assert_eq!(painted.len(), 1);
        // Cut, and said to be cut: a shortened phrase must not read as a whole one.
        let kept = painted[0]
            .0
            .strip_suffix('…')
            .expect("a cut phrase ends in «…»");
        assert!(
            !kept.is_empty()
                && "stuck on something else, not you".starts_with(kept)
                && kept.len() < "stuck on something else, not you".len(),
            "{painted:?}"
        );
        let ink = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Text(t) => Some(t.visual_bounding_rect()),
                _ => None,
            })
            .unwrap();
        assert!(ink.right() <= narrow.right() - 12.0 + 0.01, "{ink:?}");
    }
}
