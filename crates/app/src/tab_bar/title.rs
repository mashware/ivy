//! The tabs as the title bar paints them (D96): cards on the bar's bottom edge, the active one
//! opening onto the consoles below, each with its claim pill and its consoles' project dots.
//!
//! Painting and pointer routing read the same rectangles from here, so a card never paints one
//! target and answers another.

use super::{CLOSE_MIN_WIDTH, feedback};
use crate::Window;
use crate::theme::{Roles, metrics};
use crate::tree::Tab;
use crate::ui::claim_pill;
use eframe::egui::{self, Color32, CornerRadius, Rect, Stroke, pos2, vec2};
use std::time::SystemTime;

const ADD_WIDTH: f32 = 28.0;
/// The space between the last card and the add button.
const ADD_GAP: f32 = 4.0;
/// Below this the add button is neither painted nor answered.
const ADD_MIN_WIDTH: f32 = 20.0;
const MAX_WIDTH: f32 = 190.0;
const GAP: f32 = 2.0;
const PADDING: f32 = 10.0;
/// What a card too narrow for [`PADDING`] keeps on each side, and between its pill and label.
const TIGHT: f32 = 4.0;
/// The accent line along the open tab's top edge.
const ACCENT_LINE: f32 = 2.0;
const DOT: f32 = 6.0;
const DOT_GAP: f32 = 3.0;
/// The close target's width at the card's right edge.
const CLOSE_WIDTH: f32 = 26.0;
/// The narrowest card that keeps its pill whole and room for one character of its label.
const MIN_CARD: f32 = 2.0 * TIGHT + claim_pill::SIZE + TIGHT + 12.0;
/// The narrowest band that holds one such card, its gap and the add button; the title bar turns
/// compact rather than give the tabs less.
pub(crate) const MIN_BAND: f32 = MIN_CARD + GAP + ADD_GAP + ADD_WIDTH;

/// Where the tab `which` of `count` falls inside the tab band.
///
/// **All of the same width, and with no scrolling**, as the historical strip: past the point where
/// they stop fitting, the label is shortened inside its own card and navigation is by shortcut.
pub(crate) fn card(band: Rect, which: usize, count: usize) -> Rect {
    let width =
        ((band.width() - ADD_WIDTH - ADD_GAP).max(0.0) / count.max(1) as f32).min(MAX_WIDTH);
    let left = band.left() + width * which as f32;
    Rect::from_min_max(
        pos2(left.min(band.right()), band.top()),
        pos2(
            (left + (width - GAP).max(0.0)).min(band.right()),
            band.bottom(),
        ),
    )
}

/// The add button's slot, right after the last card.
pub(super) fn add(band: Rect, count: usize) -> Rect {
    let left = if count == 0 {
        band.left()
    } else {
        card(band, count - 1, count).right() + ADD_GAP
    }
    .min(band.right());
    Rect::from_min_max(
        pos2(left, band.top()),
        pos2((left + ADD_WIDTH).min(band.right()), band.bottom()),
    )
}

/// The add button where it is painted and answered: absent with no tabs or no room for it.
pub(crate) fn add_target(band: Rect, count: usize) -> Option<Rect> {
    Some(add(band, count)).filter(|add| count > 0 && add.width() >= ADD_MIN_WIDTH)
}

/// The part of the band the cards and the add button take; the title bar drags by the rest,
/// including an add slot too narrow to be a button.
pub(crate) fn occupied(band: Rect, count: usize) -> Option<Rect> {
    (count > 0).then(|| {
        let right = add_target(band, count)
            .map_or_else(|| card(band, count - 1, count).right(), |add| add.right());
        Rect::from_min_max(band.min, pos2(right, band.bottom()))
    })
}

/// What a card holds, left to right.
pub(super) struct Layout {
    pub body: Rect,
    pub close: Option<Rect>,
    pub pill: Rect,
    pub label: Rect,
    pub dots: Option<Rect>,
}

/// Whether the cross shows: on the active tab, and on the one under the pointer.
///
/// Its target exists exactly where the cross shows, so a press on an inactive tab's right edge
/// selects it instead of closing it unseen. egui resolves a press against the targets of the pass
/// before, so the cross has to have been under the pointer for one pass before it can be pressed —
/// which is what a pointer moving onto it gives.
fn shows_close(ui: &egui::Ui, card: Rect, active: bool) -> bool {
    active
        || ui
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| card.contains(p))
}

/// How many consoles a tab holds, counted without collecting them.
fn pane_count(tab: &Tab) -> usize {
    let mut count = 0;
    tab.root.walk(&mut |_| count += 1);
    count
}

/// Every card in tab order with its rectangle and its layout: the one walk painting and pointer
/// routing both read, so a card cannot paint one target and answer another.
///
/// Where the cross is not shown its slot is the body's, while the label keeps the room it has with
/// the cross, so pointing at a tab does not reflow its label.
pub(super) fn cards<'a>(
    window: &'a Window,
    ui: &'a egui::Ui,
    band: Rect,
) -> impl Iterator<Item = (usize, Rect, Layout)> + 'a {
    let count = window.tabs.len();
    window.tabs.iter().enumerate().map(move |(which, tab)| {
        let card = card(band, which, count);
        let mut layout = Layout::new(card, pane_count(tab), count > 1);
        if !shows_close(ui, card, which == window.active) {
            layout.close = None;
            layout.body = card;
        }
        (which, card, layout)
    })
}

impl Layout {
    pub fn new(card: Rect, dots: usize, close: bool) -> Self {
        // A card too narrow for the kit's air keeps the pill whole and gives the label what is left.
        let roomy = card.width() >= 2.0 * PADDING + claim_pill::SIZE + 20.0;
        let padding = if roomy { PADDING } else { TIGHT }.min(card.width() / 2.0);
        let gap = if roomy { 8.0 } else { TIGHT };
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

/// The pill and the label of a card, for tests outside this module.
#[cfg(test)]
pub(crate) fn pill_and_label(card: Rect, dots: usize, close: bool) -> (Rect, Rect) {
    let layout = Layout::new(card, dots, close);
    (layout.pill, layout.label)
}

/// The fill a target takes under the pointer — the hover surface fading in, a step further when
/// held — and whether it is held.
fn feedback_fill(ui: &egui::Ui, id: egui::Id, roles: &Roles) -> (Option<Color32>, bool) {
    let (hover, held) = feedback::hover_and_held(ui, id);
    let fill = if held {
        Some(roles.line2)
    } else {
        (hover > 0.0).then(|| roles.hover.gamma_multiply(hover))
    };
    (fill, held)
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

/// A card's label as it fits in its room.
enum Fitted {
    /// Painted as it is, braille spinners included.
    Whole(String),
    /// Cut at the room with «…».
    Cut(std::sync::Arc<egui::Galley>),
}

/// The whole label when it fits; an unnamed tab's number when «Terminal N» does not; a named tab's
/// label cut with «…» while two of its characters still show, and its number below that.
fn fitted(
    painter: &egui::Painter,
    tab: &Tab,
    room: f32,
    font: &egui::FontId,
    color: Color32,
) -> Fitted {
    let number = (tab.name + 1).to_string();
    let width = |text: String| painter.layout_no_wrap(text, font.clone(), color).size().x;
    let Some(label) = &tab.label else {
        let whole = format!("Terminal {number}");
        return Fitted::Whole(if width(whole.clone()) <= room {
            whole
        } else {
            number
        });
    };
    if width(label.clone()) <= room {
        return Fitted::Whole(label.clone());
    }
    if width(label.chars().take(2).chain(['…']).collect()) > room {
        return Fitted::Whole(number);
    }
    let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), font.clone(), color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    Fitted::Cut(painter.layout_job(job))
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
    let font = egui::FontId::proportional(window.chrome.title_sizes.tab_size());
    let mut marked = 0;
    for (which, card, layout) in cards(window, ui, band) {
        let tab = &window.tabs[which];
        let active = which == window.active;
        let painter = ui.painter_at(card);
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
        } else {
            // At rest an inactive tab is a card of its own, so a row of them reads as tabs and
            // not as loose labels. The hover and held feedback below paints over it.
            painter.rect_filled(
                Rect::from_min_max(card.min, pos2(card.right(), card.bottom() - 1.0)),
                top,
                roles.tab,
            );
        }
        // The whole card's shape, short of the bar's bottom line, which stays drawn under it.
        let (fill, held) = feedback_fill(ui, feedback::id(ui, tab.name, "body"), &roles);
        if let Some(fill) = fill
            && (!active || held)
        {
            painter.rect_filled(
                Rect::from_min_max(card.min, pos2(card.right(), card.bottom() - 1.0)),
                top,
                fill,
            );
        }
        // Painted after the feedback, so a held press does not cover which tab is the open one.
        // The card's own shape clipped to its top band, not a thin rect with the card's radius:
        // epaint caps a radius at half the rect's height, so that rect kept square-ish corners
        // that poked out past the card's rounded ones.
        if active {
            painter
                .with_clip_rect(Rect::from_min_max(
                    card.min,
                    pos2(card.right(), card.top() + ACCENT_LINE),
                ))
                .rect_filled(card, top, roles.accent);
        }
        let attention = tab.attention(now);
        marked += usize::from(attention.is_some());
        match attention {
            Some(claim) => claim_pill::paint(&painter, layout.pill, claim, &roles),
            None => claim_pill::paint_unclaimed(&painter, layout.pill, &roles),
        }
        let ink = if active { roles.tx } else { roles.tx2 };
        let clip = painter.with_clip_rect(layout.label.intersect(card));
        match fitted(&painter, tab, layout.label.width(), &font, ink) {
            Fitted::Whole(text) => {
                crate::header::title_text::paint(
                    &clip,
                    layout.label.left_center(),
                    egui::Align2::LEFT_CENTER,
                    &text,
                    font.clone(),
                    ink,
                );
            }
            Fitted::Cut(galley) => {
                let at = pos2(
                    layout.label.left(),
                    layout.label.center().y - galley.size().y / 2.0,
                );
                clip.galley(at, galley, ink);
            }
        }
        if let Some(dots) = layout.dots {
            let mut n = 0;
            tab.root.walk(&mut |pane| {
                let center = pos2(
                    dots.left() + DOT / 2.0 + n as f32 * (DOT + DOT_GAP),
                    dots.center().y,
                );
                n += 1;
                match &pane.project {
                    Some(project) => {
                        painter.circle_filled(center, DOT / 2.0, project.ink);
                    }
                    None => {
                        painter.circle_stroke(center, DOT / 2.0 - 0.5, Stroke::new(1.0, roles.tx3));
                    }
                }
            });
        }
        // Advanced every pass, shown or not, so a pointer that leaves from the cross does not leave
        // its hover at full strength for the next time the cross appears.
        let (close_fill, _) = feedback_fill(ui, feedback::id(ui, tab.name, "close"), &roles);
        if let Some(close) = layout.close {
            if let Some(fill) = close_fill {
                painter.rect_filled(
                    Rect::from_center_size(close.center(), egui::Vec2::splat(20.0)),
                    metrics::RADIUS_CHIP,
                    fill,
                );
            }
            let ink = if close_fill.is_some() {
                roles.tx
            } else {
                roles.tx3
            };
            cross(&painter, close.center(), 3.5, ink);
        }
    }
    if let Some(add) = add_target(band, count) {
        let painter = ui.painter_at(add);
        if let (Some(fill), _) = feedback_fill(ui, ui.id().with("compact-tab-add"), &roles) {
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
                let end = add_target(band, count)
                    .map_or_else(|| card(band, count - 1, count).right(), |add| add.right());
                assert_eq!(occupied(band, count).map(|r| r.right()), Some(end));
            }
        }
        let wide = Rect::from_min_size(pos2(0.0, 6.0), vec2(1200.0, 34.0));
        let layout = Layout::new(card(wide, 0, 1), 0, false);
        assert_eq!(layout.pill.width(), claim_pill::SIZE);
        assert_eq!(layout.pill.left(), PADDING);
        assert_eq!(layout.label.left(), PADDING + claim_pill::SIZE + 8.0);
    }
}
