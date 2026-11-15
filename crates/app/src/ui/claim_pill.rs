//! The claim a tab carries, as the design kit's 18-point pill.
//!
//! Wait and block are solid because they ask for action; done is soft because it does not urge; a
//! claim nobody is holding up is a dashed outline, so a stale claim never reads as a live one; no
//! claim at all is a small muted dot, which keeps the column occupied so eight tabs still scan as
//! one vertical line. The glyph is the application's own drawing, not a font character (D9).

use crate::panel::AttentionState;
use crate::theme::Roles;
use eframe::egui::{self, Color32, CornerRadius, Rect, Shape, Stroke};
use panel_contract::Attention;

/// The pill's side.
pub(crate) const SIZE: f32 = 18.0;
const RADIUS: u8 = 5;
/// The glyph's box inside the pill, the kit's 12-point mono character.
const GLYPH: f32 = 10.0;
const NONE_DOT: f32 = 6.0;

/// Paints the pill for `claim` centred in `slot`.
pub(crate) fn paint(
    painter: &egui::Painter,
    slot: Rect,
    claim: Option<AttentionState>,
    roles: &Roles,
) {
    let pill = Rect::from_center_size(slot.center(), egui::Vec2::splat(SIZE));
    let Some(claim) = claim else {
        painter.circle_filled(pill.center(), NONE_DOT / 2.0, roles.tx3);
        return;
    };
    let glyph = if !claim.backed {
        dashed_outline(painter, pill, roles.tx3);
        roles.tx3
    } else {
        let (fill, ink) = match claim.class {
            Attention::Wait => (roles.wait, roles.chrome),
            Attention::Block => (roles.block, roles.chrome),
            Attention::Done => (roles.done_soft, roles.done),
        };
        painter.rect_filled(pill, CornerRadius::same(RADIUS), fill);
        ink
    };
    let mark = Rect::from_center_size(pill.center(), egui::Vec2::splat(GLYPH));
    crate::tab_bar::indicator::paint(painter, mark, claim.class, glyph);
}

/// A rounded outline cannot be dashed by egui, so the dashes run along the four straight sides
/// and the corners stay open, which at this size reads as the kit's dashed border.
fn dashed_outline(painter: &egui::Painter, pill: Rect, color: Color32) {
    let r = pill.shrink(0.5);
    let inset = f32::from(RADIUS) - 1.0;
    let stroke = Stroke::new(1.0, color);
    for side in [
        [
            egui::pos2(r.left() + inset, r.top()),
            egui::pos2(r.right() - inset, r.top()),
        ],
        [
            egui::pos2(r.right(), r.top() + inset),
            egui::pos2(r.right(), r.bottom() - inset),
        ],
        [
            egui::pos2(r.right() - inset, r.bottom()),
            egui::pos2(r.left() + inset, r.bottom()),
        ],
        [
            egui::pos2(r.left(), r.bottom() - inset),
            egui::pos2(r.left(), r.top() + inset),
        ],
    ] {
        painter.extend(Shape::dashed_line(&side, stroke, 2.0, 2.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{NamedTheme, Theme};

    fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    fn painted(theme: &Theme, claim: Option<AttentionState>) -> Vec<Shape> {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(Default::default(), |ui| {
            let slot = Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(18.0, 34.0));
            paint(ui.painter(), slot, claim, &Roles::of(theme));
        });
        let shapes = output.shapes.iter().map(|s| s.shape.clone()).collect();
        output.drop_without_applying_deltas();
        shapes
    }

    fn claim(class: Attention, backed: bool) -> Option<AttentionState> {
        Some(AttentionState { class, backed })
    }

    /// The colour of every stroke and fill in the glyph: the one grouped shape the mark is.
    fn glyph_colors(shapes: &[Shape]) -> Vec<Color32> {
        let parts = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Vec(parts) => Some(parts.as_slice()),
                _ => None,
            })
            .unwrap_or_default();
        let mut colors = Vec::new();
        for shape in parts {
            match shape {
                Shape::Rect(r) => colors.push(r.fill),
                Shape::Path(p) => match p.stroke.color {
                    egui::epaint::ColorMode::Solid(c) => colors.push(c),
                    _ => colors.push(Color32::TRANSPARENT),
                },
                Shape::Circle(c) if c.fill != Color32::TRANSPARENT => colors.push(c.fill),
                Shape::Circle(c) => colors.push(c.stroke.color),
                Shape::LineSegment { stroke, .. } => colors.push(stroke.color),
                _ => {}
            }
        }
        colors
    }

    fn pill_fill(shapes: &[Shape]) -> Option<(Rect, Color32, CornerRadius)> {
        shapes.iter().find_map(|s| match s {
            Shape::Rect(r) => Some((r.rect, r.fill, r.corner_radius)),
            _ => None,
        })
    }

    /// **Literal colours, written out**: Ivy Night's wait, block and done slots, its chrome role
    /// for the glyph on a solid pill, and done's 15 % tint.
    #[test]
    fn solid_soft_dashed_and_dot_pills_paint_the_kits_colors() {
        let night = Theme::default();
        let pill = Rect::from_min_size(egui::pos2(10.0, 18.0), egui::vec2(18.0, 18.0));
        for (class, fill, ink) in [
            (Attention::Wait, rgb(0xf2b84b), rgb(0x0f1315)),
            (Attention::Block, rgb(0xf27a66), rgb(0x0f1315)),
            (
                Attention::Done,
                Color32::from_rgba_unmultiplied(0x72, 0xb7, 0xf2, 38),
                rgb(0x72b7f2),
            ),
        ] {
            let shapes = painted(&night, claim(class, true));
            assert_eq!(
                pill_fill(&shapes),
                Some((pill, fill, CornerRadius::same(5))),
                "{class:?}"
            );
            let glyph = glyph_colors(&shapes);
            assert!(!glyph.is_empty(), "{class:?}: no glyph");
            assert!(glyph.iter().all(|c| *c == ink), "{class:?}: {glyph:?}");
        }
        // Unbacked: no fill, four dashed sides and the glyph in muted text.
        let shapes = painted(&night, claim(Attention::Wait, false));
        assert_eq!(pill_fill(&shapes), None);
        let dashes = shapes
            .iter()
            .filter(
                |s| matches!(s, Shape::LineSegment { stroke, .. } if stroke.color == rgb(0x8e9390)),
            )
            .count();
        assert!(dashes >= 8, "the outline is dashed on four sides: {dashes}");
        assert!(glyph_colors(&shapes).iter().all(|c| *c == rgb(0x8e9390)));
        // No claim: one 6-point muted dot at the slot's centre and nothing else.
        let shapes = painted(&night, None);
        assert!(matches!(
            shapes.as_slice(),
            [Shape::Circle(c)] if c.radius == 3.0 && c.fill == rgb(0x8e9390) && c.center == egui::pos2(19.0, 27.0)
        ));
    }

    #[test]
    fn ivy_day_pills_take_the_day_palette() {
        let day = NamedTheme::IvyDay.theme();
        let shapes = painted(&day, claim(Attention::Block, true));
        assert_eq!(pill_fill(&shapes).map(|p| p.1), Some(rgb(0xb93a26)));
        assert!(glyph_colors(&shapes).iter().all(|c| *c == rgb(0xeceeeb)));
    }
}
