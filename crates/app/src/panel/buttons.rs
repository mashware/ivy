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

use super::{MARGIN, PaintedButton, Row, RowRole};
use crate::painter::CellMetrics;
use crate::theme::{Roles, metrics};
use eframe::egui::{CornerRadius, Pos2, Rect, Shape, Stroke, StrokeKind};

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
///
/// `framed` is the integrated chrome, whose buttons are painted inside a frame: there the reach is
/// that frame. The measured runs paint no frame and keep the rows' whole width (D97).
pub(super) fn buttons_drawn(
    rows: &[Row],
    whole: usize,
    rect: Rect,
    metrics: &CellMetrics,
    framed: bool,
) -> Vec<PaintedButton> {
    let mut drawn: Vec<PaintedButton> = Vec::new();
    let mut open: Option<Run> = None;

    for (nth, row) in rows.iter().enumerate() {
        let belongs = match row.role {
            RowRole::Text | RowRole::Fold(_) => None,
            RowRole::ButtonLabel(action) | RowRole::ButtonBody(action) => Some(action),
        };
        let body = matches!(row.role, RowRole::ButtonBody(_)).then_some(nth);
        let closes = match (open, belongs) {
            (Some(run), Some(same)) if same == run.action => {
                open = Some(Run {
                    last: nth,
                    body: run.body.or(body),
                    ..run
                });
                None
            }
            (was, here) => {
                open = here.map(|action| Run {
                    action,
                    first: nth,
                    last: nth,
                    body,
                });
                was
            }
        };
        if let Some(run) = closes {
            keep_if_whole(&mut drawn, run, whole, (rect, framed), metrics);
        }
    }
    if let Some(run) = open {
        keep_if_whole(&mut drawn, run, whole, (rect, framed), metrics);
    }
    drawn
}

/// One button's rows: the first, the last, and where its command starts.
#[derive(Clone, Copy)]
struct Run {
    action: usize,
    first: usize,
    last: usize,
    body: Option<usize>,
}

/// Records that button **only if every one of its rows survived the clip**.
fn keep_if_whole(
    drawn: &mut Vec<PaintedButton>,
    run: Run,
    whole: usize,
    (rect, framed): (Rect, bool),
    metrics: &CellMetrics,
) {
    if run.last < whole {
        drawn.push(painted(run, rect, metrics, framed));
    }
}

/// The rectangle a button occupies, which is **both what a press reaches and the frame that is
/// painted** — one function for the two, so the edge a person sees is the edge a click obeys.
///
/// Framed, it sits half a cell in from each side of the strip, where the text's margin leaves room,
/// and [`GAP`] in from its first and last row, so two buttons in a row read as two.
fn painted(run: Run, rect: Rect, metrics: &CellMetrics, framed: bool) -> PaintedButton {
    let row_top = |row: usize| rect.top() + row as f32 * metrics.height();
    let gap = if framed { GAP } else { 0.0 };
    PaintedButton {
        action: run.action,
        area: Rect::from_min_max(
            Pos2::new(
                rect.left() + inset(metrics, framed),
                row_top(run.first) + gap,
            ),
            Pos2::new(
                right_edge(rect, metrics, framed),
                row_top(run.last + 1) - gap,
            ),
        ),
        command: run.body.map(row_top),
    }
}

/// How far a button's frame stands in from each side of the strip: half a cell when framed.
fn inset(metrics: &CellMetrics, framed: bool) -> f32 {
    if framed {
        MARGIN * metrics.width() / 2.0
    } else {
        0.0
    }
}

/// A button's right edge: the frame's when framed, the strip's otherwise. Ink past it is outside the
/// button it belongs to, so the painting treats it as overflow and the button stays unreachable.
pub(super) fn right_edge(rect: Rect, metrics: &CellMetrics, framed: bool) -> f32 {
    rect.right() - inset(metrics, framed)
}

/// What separates a framed button from the rows above and below it: two buttons in a row stand
/// twice this apart, the design's six points.
const GAP: f32 = 3.0;

/// The frame's border. The kit's control border, `line2`, all but vanishes against the panel on a dark
/// theme, so the frame takes the muted text at half strength, which reads on both kinds of theme.
pub(super) fn border(roles: &Roles) -> eframe::egui::Color32 {
    roles.tx3.gamma_multiply(0.5)
}

/// The shape of a button's frame, queued under its text: the label on the raised surface, the
/// command on the console's own background below a rule, all inside a bordered rounded box.
///
/// **The command keeps the console's background on purpose**: it is the text that runs, and it is
/// set where text that runs is always set.
pub(super) fn frame(button: &PaintedButton, roles: &Roles) -> Shape {
    let radius = metrics::RADIUS_CONTROL;
    let area = button.area;
    let mut shapes = vec![Shape::rect_filled(area, radius, roles.raised)];
    if let Some(top) = button.command {
        let command = Rect::from_min_max(
            Pos2::new(area.left(), top.clamp(area.top(), area.bottom())),
            area.max,
        );
        shapes.push(Shape::rect_filled(
            command,
            CornerRadius {
                nw: 0,
                ne: 0,
                sw: radius,
                se: radius,
            },
            roles.base,
        ));
        shapes.push(Shape::line_segment(
            [command.left_top(), command.right_top()],
            Stroke::new(1.0, roles.line),
        ));
    }
    shapes.push(Shape::rect_stroke(
        area,
        radius,
        Stroke::new(1.0, border(roles)),
        StrokeKind::Inside,
    ));
    Shape::Vec(shapes)
}
