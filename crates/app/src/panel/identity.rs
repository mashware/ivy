//! The panel's identity row: what this pane is working on, and who is driving it.
//!
//! **Two forms, chosen by the window's chrome.** The historical one — what the batches without the
//! integrated chrome still paint (D13) — writes the pane's identifier, the phase and the driver in
//! one line of text. The integrated one paints the phase as a chip and the driver beside it, and
//! never the identifier: a hexadecimal name says nothing to a person, who already finds the console
//! by its header (D96, D97).
//!
//! Both say only what the application knows of its own and what the producer published (D6): the
//! phase is the producer's free text, the driver is the connection or hook that holds the panel.

use super::{Row, color, fold, sanitize, wrap};
use crate::fonts::Discoverer;
use crate::painter::{CellMetrics, ColorGlyph};
use crate::theme::{Roles, Theme, metrics::RADIUS_CHIP};
use eframe::egui::{Color32, Painter, Pos2, Rect, vec2};

/// The room around the phase inside its chip, and between the chip and the driver.
const PAD: f32 = 6.0;

/// The integrated identity row's two parts and their inks: the phase, painted in the row's colour
/// on `fill`, and the driver in `rest`. Either may be empty, never both.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Chip {
    phase: String,
    driver: String,
    fill: Color32,
    rest: Color32,
}

/// Adds the identity row: the historical text when `kit` is nothing, the integrated chip when it
/// carries the integrated chrome's roles.
pub(super) fn lay_out(
    rows: &mut Vec<Row>,
    id: &str,
    phase: Option<&str>,
    held_by: Option<&str>,
    width: usize,
    theme: &Theme,
    kit: Option<&Roles>,
) {
    if let Some(roles) = kit {
        let phase = phase.map(sanitize).unwrap_or_default().trim().to_owned();
        let driver = held_by.map(sanitize).unwrap_or_default().trim().to_owned();
        if phase.is_empty() && driver.is_empty() {
            return;
        }
        // The text is what the font discovery and the overflow read; the chip is what is painted.
        let mut row = Row::text(0, format!("{phase} {driver}"), roles.tx2);
        row.chip = Some(Chip {
            phase,
            driver,
            fill: roles.hover,
            rest: roles.tx3,
        });
        rows.push(row);
        return;
    }
    // **Who is driving goes on the identity row, beside the pane's own name and its phase.** That
    // row answers «what is this pane», and with two transports the answer includes which one is in
    // charge — a person looking at a panel that will not change from the file has to be able to
    // see why without editing anything to find out.
    let identity = match (phase, held_by) {
        (Some(phase), Some(source)) => format!("{id} · {phase} · {source}"),
        (Some(phase), None) => format!("{id} · {phase}"),
        (None, Some(source)) => format!("{id} · {source}"),
        (None, None) => id.to_owned(),
    };
    wrap(
        rows,
        0,
        &sanitize(&identity),
        theme.panel_color(None, None),
        width,
    );
}

/// Paints the integrated identity row at `origin`, inside `width` cells, and returns what it inked
/// — or nothing when `row` is not that row.
///
/// **One row and cut, not wrapped**: the chip is a single shape. What is cut ends in «…», so a
/// shortened phase or driver never reads as a whole one, and a part that cannot show even that is
/// left out rather than drawn as an empty chip.
pub(super) fn paint(
    painter: &Painter,
    origin: Pos2,
    row: &Row,
    metrics: &CellMetrics,
    width: usize,
    discoverer: &mut Discoverer,
    glyphs: &mut Vec<ColorGlyph>,
) -> Option<Rect> {
    let chip = row.chip.as_ref()?;
    let right = origin.x + width as f32 * metrics.width();
    let measure = |text: &str| {
        painter
            .layout_no_wrap(text.to_owned(), metrics.font(), row.color)
            .size()
            .x
    };
    let mut x = origin.x;
    let mut inked = Rect::NOTHING;
    let mut piece = |text: &str, x: f32, ink: Color32| {
        let mut pieces = [Row::text(0, text.to_owned(), ink)];
        color::flag(&mut pieces);
        discoverer.note_text(text);
        color::paint_text(
            painter,
            Pos2::new(x, origin.y),
            &pieces[0],
            metrics,
            discoverer,
            glyphs,
        )
    };
    let phase = fold::fit_marked(&chip.phase, (right - x - 2.0 * PAD).max(0.0), &measure);
    if !phase.is_empty() {
        let face = Rect::from_min_size(
            Pos2::new(x, origin.y + 1.0),
            vec2(measure(&phase) + 2.0 * PAD, metrics.height() - 2.0),
        );
        painter.rect_filled(face, RADIUS_CHIP, chip.fill);
        inked = face.union(piece(&phase, x + PAD, row.color));
        x = face.right() + PAD;
    }
    let driver = fold::fit_marked(&chip.driver, (right - x).max(0.0), &measure);
    if !driver.is_empty() {
        inked = inked.union(piece(&driver, x, chip.rest));
    }
    Some(inked)
}

#[cfg(test)]
mod tests {
    use crate::panel::tests::paint_integrated;
    use crate::panel::{PanelState, Rect};
    use eframe::egui::{Color32, CornerRadius, Pos2, Shape, vec2};
    use std::time::SystemTime;

    fn rgb(hex: u32) -> Color32 {
        Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    fn painted(source: &str, document: &str) -> Vec<Shape> {
        let mut state = PanelState::default();
        state
            .deliver(
                7,
                source,
                panel_contract::parse_document(document).expect("a panel"),
                SystemTime::now(),
            )
            .expect("nobody holds it");
        paint_integrated(
            &mut state,
            Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0)),
            SystemTime::now(),
        )
    }

    fn texts(shapes: &[Shape]) -> Vec<(String, Color32, Rect)> {
        shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text(t) => Some((
                    t.galley.text().to_owned(),
                    t.fallback_color,
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                _ => None,
            })
            .collect()
    }

    /// **Literal colours, written out**: Ivy Night's hover fill under the phase, its secondary text
    /// for the phase and its muted text for the driver — and the pane's identifier nowhere.
    #[test]
    fn the_phase_is_a_chip_the_driver_sits_beside_it_and_the_id_is_not_painted() {
        let shapes = painted("claude", r#"{"phase":"plan review","lines":["the body"]}"#);
        let rows = texts(&shapes);
        assert!(
            rows.iter().all(|(text, _, _)| !text.contains("a1b2")),
            "the identifier is painted: {rows:?}"
        );
        let phase = rows.iter().find(|r| r.0 == "plan review").expect("phase");
        let driver = rows.iter().find(|r| r.0 == "claude").expect("driver");
        assert_eq!(phase.1, rgb(0xa5aaa6));
        assert_eq!(driver.1, rgb(0x8e9390));
        let face = shapes
            .iter()
            .find_map(|s| match s {
                Shape::Rect(r) if r.fill == rgb(0x262b2d) => Some((r.rect, r.corner_radius)),
                _ => None,
            })
            .expect("the chip's face is painted");
        assert_eq!(face.1, CornerRadius::same(4));
        assert!(
            face.0.left() < phase.2.left()
                && face.0.right() > phase.2.right()
                && face.0.contains(phase.2.center()),
            "the phase sits on its chip: {face:?} {phase:?}"
        );
        assert!(
            driver.2.left() >= face.0.right(),
            "the driver sits after the chip"
        );
    }

    /// With neither a phase nor a driver there is nothing to say, so there is no identity row and
    /// no chip.
    #[test]
    fn with_nothing_to_identify_there_is_no_identity_row() {
        let mut state = PanelState::default();
        let dir = panel_contract::PanelDir::create().expect("dir");
        crate::panel::tests::publish(&dir, r#"{"lines":["the body"]}"#);
        state.refresh(dir.path());
        let shapes = paint_integrated(
            &mut state,
            Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0)),
            SystemTime::now(),
        );
        let rows = texts(&shapes);
        assert_eq!(
            rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
            ["  the body"]
        );
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(s, Shape::Rect(r) if r.fill == rgb(0x262b2d)))
        );
    }
}
