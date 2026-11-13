//! Which buttons a painting of the panel left reachable, and where.
//!
//! **It is the one piece of this module where being wrong costs more than a badly drawn panel**, so
//! it lives apart from the rows: a press reaches what this file says it reaches and nothing else.
//! Everything it refuses is refused by **absence** — a button that is not on the list is
//! unreachable, and no branch anywhere has to remember to say no.
//!
//! It came out of its parent under **D28**, which gives a module a ceiling of 600 production lines.
//! The delivery that wrote it is the one that filled that ceiling, and this is the piece that
//! delivery was already editing — which is the way through **D28** records.

use super::{PaintedButton, Row, RowRole};
use crate::painter::CellMetrics;
use eframe::egui::{Pos2, Rect};

/// Which buttons were drawn **whole**, and where.
///
/// **It walks every row and not only the visible ones, which is the fix for a defect the first
/// version had.** `clip` drops rows from the bottom, so a short panel leaves a label on screen with
/// its command scrolled away — and a run-detector over the visible rows alone cannot tell «the run
/// ended» from «the rows ended», so it published exactly that half-drawn button as pressable. A
/// press on it would have run bytes nobody could read, which is the one state the rule about seeing
/// a command before it runs forbids.
///
/// `whole` is how many rows survived the clip. A button whose last row is past it is absent from the
/// list, and absent from the list is absent from the frame's reach — there is no branch anywhere
/// that has to remember to refuse it.
pub(super) fn buttons_drawn(
    rows: &[Row],
    whole: usize,
    rect: Rect,
    metrics: &CellMetrics,
) -> Vec<PaintedButton> {
    let mut drawn: Vec<PaintedButton> = Vec::new();
    let mut open: Option<(usize, usize, usize)> = None;

    for (nth, row) in rows.iter().enumerate() {
        let belongs = match row.role {
            RowRole::Text | RowRole::Fold(_) => None,
            RowRole::ButtonLabel(action) | RowRole::ButtonBody(action) => Some(action),
        };
        let closes = match (open, belongs) {
            (Some((action, first, _)), Some(same)) if same == action => {
                open = Some((action, first, nth));
                None
            }
            (was, here) => {
                open = here.map(|action| (action, nth, nth));
                was
            }
        };
        if let Some((action, first, last)) = closes {
            keep_if_whole(&mut drawn, action, first, last, whole, rect, metrics);
        }
    }
    if let Some((action, first, last)) = open {
        keep_if_whole(&mut drawn, action, first, last, whole, rect, metrics);
    }
    drawn
}

/// Records that button **only if every one of its rows survived the clip**.
fn keep_if_whole(
    drawn: &mut Vec<PaintedButton>,
    action: usize,
    first: usize,
    last: usize,
    whole: usize,
    rect: Rect,
    metrics: &CellMetrics,
) {
    if last < whole {
        drawn.push(painted(action, first, last, rect, metrics));
    }
}

/// The rectangle a button's rows occupy, from the first to the last.
fn painted(
    action: usize,
    first: usize,
    last: usize,
    rect: Rect,
    metrics: &CellMetrics,
) -> PaintedButton {
    let top = rect.top() + first as f32 * metrics.height();
    let bottom = rect.top() + (last + 1) as f32 * metrics.height();
    PaintedButton {
        action,
        area: Rect::from_min_max(Pos2::new(rect.left(), top), Pos2::new(rect.right(), bottom)),
    }
}
