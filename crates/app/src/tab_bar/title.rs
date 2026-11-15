//! The tabs as the title bar paints them (D96): cards on the bar's bottom edge, the active one
//! opening onto the consoles below, each with its claim pill and its consoles' project dots.
//!
//! Painting and pointer routing read the same rectangles from here, so a card never paints one
//! target and answers another.

use super::{CLOSE_MIN_WIDTH, feedback};
use crate::Window;
use crate::theme::{Roles, metrics};
use crate::ui::claim_pill;
use eframe::egui::{self, Color32, CornerRadius, Rect, Stroke, pos2, vec2};
use std::time::SystemTime;

const ADD_WIDTH: f32 = 28.0;
const MAX_WIDTH: f32 = 190.0;
const GAP: f32 = 2.0;
const PADDING: f32 = 10.0;
const DOT: f32 = 6.0;
const DOT_GAP: f32 = 3.0;
/// The close target's width at the card's right edge.
const CLOSE_WIDTH: f32 = 26.0;

/// Where the tab `which` of `count` falls inside the tab band.
///
/// **All of the same width, and with no scrolling**, as the historical strip: past the point where
/// they stop fitting, the label is cut off inside its own card and navigation is by shortcut.
pub(crate) fn card(band: Rect, which: usize, count: usize) -> Rect {
    let width = ((band.width() - ADD_WIDTH - 4.0).max(0.0) / count.max(1) as f32).min(MAX_WIDTH);
    let left = band.left() + width * which as f32;
    Rect::from_min_max(
        pos2(left.min(band.right()), band.top()),
        pos2(
            (left + (width - GAP).max(0.0)).min(band.right()),
            band.bottom(),
        ),
    )
}

/// The add button, right after the last card.
pub(super) fn add(band: Rect, count: usize) -> Rect {
    let left = if count == 0 {
        band.left()
    } else {
        card(band, count - 1, count).right() + 4.0
    }
    .min(band.right());
    Rect::from_min_max(
        pos2(left, band.top()),
        pos2((left + ADD_WIDTH).min(band.right()), band.bottom()),
    )
}

/// The part of the band the cards and the add button take; the title bar drags by the rest.
pub(crate) fn occupied(band: Rect, count: usize) -> Option<Rect> {
    (count > 0).then(|| Rect::from_min_max(band.min, pos2(add(band, count).right(), band.bottom())))
}

/// What a card holds, left to right.
pub(super) struct Layout {
    pub body: Rect,
    pub close: Option<Rect>,
    pub pill: Rect,
    pub label: Rect,
    pub dots: Option<Rect>,
}

/// The cross is painted on the active tab and on the one under the pointer only, but its target
/// exists on every card: egui resolves a press against the targets of the pass before, so a target
/// that appeared with the hover would lose the first press over it to the card's body.
pub(super) fn shows_close(ui: &egui::Ui, card: Rect, active: bool) -> bool {
    active
        || ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| card.contains(p))
}

impl Layout {
    pub fn new(card: Rect, dots: usize, close: bool) -> Self {
        // A card too narrow for the kit's air keeps the pill whole and gives the label what is left.
        let roomy = card.width() >= 2.0 * PADDING + claim_pill::SIZE + 20.0;
        let padding = if roomy { PADDING } else { 4.0 }.min(card.width() / 2.0);
        let gap = if roomy { 8.0 } else { 4.0 };
        let close = (close && card.width() >= CLOSE_MIN_WIDTH).then(|| {
            Rect::from_min_max(
                pos2(card.right() - CLOSE_WIDTH, card.top()),
                card.right_bottom(),
            )
        });
        let body = Rect::from_min_max(
            card.min,
            pos2(close.map_or(card.right(), |c| c.left()), card.bottom()),
        );
        let pill_width = claim_pill::SIZE.min((body.width() - 2.0 * padding).max(0.0));
        let pill = Rect::from_min_max(
            pos2(body.left() + padding, body.top()),
            pos2(body.left() + padding + pill_width, body.bottom()),
        );
        let label_left = (pill.right() + gap).min(body.right());
        let end = if close.is_some() {
            body.right()
        } else {
            (body.right() - padding).max(label_left)
        };
        let dots_width = dots as f32 * (DOT + DOT_GAP) - DOT_GAP;
        let dots = (dots > 0 && end - label_left - dots_width - 8.0 >= 24.0).then(|| {
            Rect::from_min_max(pos2(end - dots_width, body.top()), pos2(end, body.bottom()))
        });
        let label = Rect::from_min_max(
            pos2(label_left, body.top()),
            pos2(
                dots.map_or(end, |d| d.left() - 8.0).max(label_left),
                body.bottom(),
            ),
        );
        Self {
            body,
            close,
            pill,
            label,
            dots,
        }
    }
}

/// The fill a target takes under the pointer: the hover surface fading in, a step further when held.
fn feedback_fill(ui: &egui::Ui, id: egui::Id, roles: &Roles) -> Option<Color32> {
    let (hover, held) = feedback::hover_and_held(ui, id);
    if held {
        Some(roles.line2)
    } else {
        (hover > 0.0).then(|| roles.hover.gamma_multiply(hover))
    }
}

fn cross(painter: &egui::Painter, center: egui::Pos2, half: f32, color: Color32) {
    let stroke = Stroke::new(1.2, color);
    for (a, b) in [((-1.0, -1.0), (1.0, 1.0)), ((1.0, -1.0), (-1.0, 1.0))] {
        painter.line_segment(
            [
                center + vec2(a.0, a.1) * half,
                center + vec2(b.0, b.1) * half,
            ],
            stroke,
        );
    }
}

/// Paints every card and the add button; returns how many cards carried a claim.
pub(super) fn paint(window: &Window, ui: &egui::Ui, band: Rect, now: SystemTime) -> usize {
    let roles = crate::ui::style::roles(ui.ctx(), &window.theme);
    let count = window.tabs.len();
    let top = CornerRadius {
        nw: metrics::RADIUS_CARD,
        ne: metrics::RADIUS_CARD,
        sw: 0,
        se: 0,
    };
    let mut marked = 0;
    for (which, tab) in window.tabs.iter().enumerate() {
        let card = card(band, which, count);
        let active = which == window.active;
        let painter = ui.painter_at(card);
        let panes = tab.panes();
        let layout = Layout::new(card, panes.len(), count > 1);
        if active {
            painter.rect_filled(card, top, roles.base);
            painter.rect_stroke(
                card,
                top,
                Stroke::new(1.0, roles.line),
                egui::StrokeKind::Inside,
            );
            // Open at the bottom: the active tab is the top of the consoles under it.
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(card.left() + 1.0, card.bottom() - 1.0),
                    pos2(card.right() - 1.0, card.bottom()),
                ),
                0.0,
                roles.base,
            );
        }
        if let Some(fill) = feedback_fill(ui, feedback::id(ui, tab.name, "body"), &roles)
            && (!active || fill == roles.line2)
        {
            painter.rect_filled(layout.body, top, fill);
        }
        let attention = tab.attention(now);
        marked += usize::from(attention.is_some());
        claim_pill::paint(&painter, layout.pill, attention, &roles);
        let label = tab.label.clone().unwrap_or_else(|| {
            let number = tab.name + 1;
            if card.width() < 100.0 {
                number.to_string()
            } else {
                format!("Terminal {number}")
            }
        });
        crate::header::title_text::paint(
            &painter.with_clip_rect(layout.label.intersect(card)),
            layout.label.left_center(),
            egui::Align2::LEFT_CENTER,
            &label,
            egui::FontId::proportional(window.chrome.title_sizes.tab_size()),
            if active { roles.tx } else { roles.tx2 },
        );
        if let Some(dots) = layout.dots {
            for (n, pane) in panes.iter().enumerate() {
                let center = pos2(
                    dots.left() + DOT / 2.0 + n as f32 * (DOT + DOT_GAP),
                    dots.center().y,
                );
                match &pane.project {
                    Some(project) => {
                        painter.circle_filled(center, DOT / 2.0, project.ink);
                    }
                    None => {
                        painter.circle_stroke(center, DOT / 2.0 - 0.5, Stroke::new(1.0, roles.tx3));
                    }
                }
            }
        }
        if let Some(close) = layout.close.filter(|_| shows_close(ui, card, active)) {
            let id = feedback::id(ui, tab.name, "close");
            let hovered = feedback_fill(ui, id, &roles);
            if let Some(fill) = hovered {
                painter.rect_filled(
                    Rect::from_center_size(close.center(), egui::Vec2::splat(20.0)),
                    metrics::RADIUS_CHIP,
                    fill,
                );
            }
            let ink = if hovered.is_some() {
                roles.tx
            } else {
                roles.tx3
            };
            cross(&painter, close.center(), 3.5, ink);
        }
    }
    let add = add(band, count);
    if count > 0 && add.width() >= 20.0 {
        let painter = ui.painter_at(add);
        if let Some(fill) = feedback_fill(ui, ui.id().with("compact-tab-add"), &roles) {
            painter.rect_filled(
                Rect::from_center_size(add.center(), egui::Vec2::splat(ADD_WIDTH)),
                metrics::RADIUS_CONTROL,
                fill,
            );
        }
        let stroke = Stroke::new(1.3, roles.tx3);
        let c = add.center();
        painter.line_segment([c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)], stroke);
        painter.line_segment([c - vec2(0.0, 5.0), c + vec2(0.0, 5.0)], stroke);
    }
    marked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_the_add_button_and_their_contents_stay_disjoint_at_every_width() {
        for width in [0.0, 20.0, 64.0, 66.0, 120.0, 400.0, 1200.0] {
            let band = Rect::from_min_size(pos2(120.0, 6.0), vec2(width, 34.0));
            for count in [1, 2, 3, 12] {
                let add = add(band, count);
                for which in 0..count {
                    let card = card(band, which, count);
                    assert!(band.contains_rect(card), "{width} {count}");
                    assert!(card.width() <= MAX_WIDTH);
                    assert!(card.right() <= add.left() + f32::EPSILON || add.width() == 0.0);
                    if which + 1 < count {
                        assert!(card.right() <= super::card(band, which + 1, count).left());
                    }
                    for close in [false, true] {
                        for dots in [0, 1, 4] {
                            let layout = Layout::new(card, dots, close);
                            assert!(card.contains_rect(layout.body));
                            assert!(layout.label.left() >= layout.pill.right());
                            assert!(layout.label.right() <= layout.body.right());
                            if let Some(close) = layout.close {
                                assert_eq!(layout.body.right(), close.left());
                            }
                            if let Some(dots) = layout.dots {
                                assert!(dots.left() >= layout.label.right() + 8.0);
                                assert!(dots.right() <= layout.body.right());
                            }
                        }
                    }
                }
                assert_eq!(occupied(band, count).map(|r| r.right()), Some(add.right()));
            }
        }
        let wide = Rect::from_min_size(pos2(0.0, 6.0), vec2(1200.0, 34.0));
        let layout = Layout::new(card(wide, 0, 1), 0, false);
        assert_eq!(layout.pill.width(), claim_pill::SIZE);
        assert_eq!(layout.pill.left(), PADDING);
        assert_eq!(layout.label.left(), PADDING + claim_pill::SIZE + 8.0);
    }
}
