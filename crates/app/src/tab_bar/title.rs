//! The tabs as the title bar paints them (D96): cards on the bar's bottom edge, the active one
//! opening onto the consoles below, each with its label and one project dot per console, ringed in
//! the colour of that console's claim — or, where the dots do not fit, one muted dot ringed with the
//! tab's loudest claim.
//!
//! Painting and pointer routing read the same rectangles from here, so a card never paints one
//! target and answers another.

use super::{CLOSE_MIN_WIDTH, feedback};
use crate::Window;
use crate::theme::{Roles, Theme, metrics};
use crate::tree::Tab;
use eframe::egui::{self, Color32, CornerRadius, Rect, Stroke, pos2};
use panel_contract::Attention;
use std::time::SystemTime;

const ADD_WIDTH: f32 = 28.0;
/// The space between the last card and the add button.
const ADD_GAP: f32 = 4.0;
/// Below this the add button is neither painted nor answered.
const ADD_MIN_WIDTH: f32 = 20.0;
const MAX_WIDTH: f32 = 190.0;
const GAP: f32 = 2.0;
const PADDING: f32 = 10.0;
/// What a card too narrow for [`PADDING`] keeps on each side.
const TIGHT: f32 = 4.0;
/// Below this width a card takes [`TIGHT`] instead of [`PADDING`]. The value is the one the claim
/// pill's slot set (D96); the pill left the card, the threshold did not, so cards keep their air at
/// the same widths.
const ROOMY: f32 = 58.0;
/// The accent line along the open tab's top edge.
const ACCENT_LINE: f32 = 2.0;
const DOT: f32 = 6.0;
/// A claiming console's ring, always in its class's full colour: 1.5 points read too heavy at the
/// desktop check (D96).
const RING_WIDTH: f32 = 1.0;
/// The band a ring is laid out in, wider than the stroke and centred on it, so the geometry does
/// not move with the stroke.
const RING_BAND: f32 = 1.5;
const _: () = assert!(
    RING_WIDTH <= RING_BAND,
    "the ring's stroke would leave its band"
);
/// From the dot's edge to the ring's band.
const RING_GAP: f32 = 1.5;
/// How far a ring's band reaches from its dot's centre.
const RING_REACH: f32 = DOT / 2.0 + RING_GAP + RING_BAND;
/// From one ring's band to the next; the painted strokes, thinner than their bands, end up half a
/// point further apart. At 1 point the owner read the rings as touching (D96).
const RING_SPACING: f32 = 4.0;
/// From one dot's centre to the next. Fixed whether or not anything claims, so a claim arriving
/// never moves the label.
const DOT_STEP: f32 = 2.0 * RING_REACH + RING_SPACING;
/// The room a label keeps beside every console's dot; below it the dots give way to one summary dot.
const LABEL_FLOOR: f32 = 24.0;
/// The room a label keeps beside the summary dot: the tab's number. The summary outranks the label,
/// so every card the bar keeps (down to [`MIN_CARD`]) can still say that something claims (D96).
const SUMMARY_LABEL_FLOOR: f32 = 8.0;
/// The air between the label and the dots.
const LABEL_AIR: f32 = 8.0;
/// The «new image» glyph, the size of the header's (D120).
const UNSEEN: f32 = 12.0;
/// The close target's width at the card's right edge.
const CLOSE_WIDTH: f32 = 26.0;
/// The narrowest card the title bar keeps before turning compact. The value is the pill era's
/// (a whole pill and one character of label); it was kept when the pill left so the compact
/// threshold D96 publishes did not move.
const MIN_CARD: f32 = 42.0;
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
    pub label: Rect,
    /// The dots and their rings' reach. Absent when the card cannot hold them all.
    pub dots: Option<Rect>,
    /// One dot's slot, only where the dots did not fit: the tab's loudest claim, ringed (D96,
    /// amended for #113). Reserved whether or not anything claims, so a claim never moves the label.
    pub summary: Option<Rect>,
    /// The «new image» glyph, at the label's right end, while one of the tab's consoles holds an
    /// image its agent opened that nobody has seen (D120). It gives way before the label's floor.
    pub unseen: Option<Rect>,
}

/// The air a card keeps on each side: the kit's on a roomy card, less on a narrow one.
fn padding(card: Rect) -> f32 {
    if card.width() >= ROOMY {
        PADDING
    } else {
        TIGHT
    }
    .min(card.width() / 2.0)
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

/// How many consoles a tab holds, and whether one holds an image nobody has seen, in one walk
/// that collects nothing.
fn pane_count(tab: &Tab) -> (usize, bool) {
    let mut count = 0;
    let mut unseen = false;
    tab.root.walk(&mut |pane| {
        count += 1;
        unseen |= pane.image.unseen();
    });
    (count, unseen)
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
        let (panes, unseen) = pane_count(tab);
        let mut layout = Layout::new(card, panes, count > 1, unseen);
        if !shows_close(ui, card, which == window.active) {
            layout.close = None;
            layout.body = card;
        }
        (which, card, layout)
    })
}

impl Layout {
    pub fn new(card: Rect, dots: usize, close: bool, unseen: bool) -> Self {
        let padding = padding(card);
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
        let label_left = (body.left() + padding).min(body.right());
        let end = if close.is_some() {
            body.right()
        } else {
            (body.right() - padding).max(label_left)
        };
        // `n` dots, right-aligned, where the label keeps `floor` points and its air before them.
        let fit = |n: usize, floor: f32| {
            let width = n as f32 * DOT_STEP - RING_SPACING;
            (n > 0 && end - label_left - width - LABEL_AIR >= floor).then(|| {
                Rect::from_min_max(pos2(end - width, body.top()), pos2(end, body.bottom()))
            })
        };
        let all = fit(dots, LABEL_FLOOR);
        let summary = if all.is_none() {
            fit(dots.min(1), SUMMARY_LABEL_FLOOR)
        } else {
            None
        };
        let mut label = Rect::from_min_max(
            pos2(label_left, body.top()),
            pos2(
                all.or(summary)
                    .map_or(end, |d| d.left() - LABEL_AIR)
                    .max(label_left),
                body.bottom(),
            ),
        );
        let floor = if all.is_some() {
            LABEL_FLOOR
        } else {
            SUMMARY_LABEL_FLOOR
        };
        let unseen = (unseen && label.width() - UNSEEN - LABEL_AIR >= floor).then(|| {
            let glyph = Rect::from_min_max(pos2(label.right() - UNSEEN, body.top()), label.max);
            label.max.x = glyph.left() - LABEL_AIR;
            glyph
        });
        Self {
            body,
            close,
            label,
            dots: all,
            summary,
            unseen,
        }
    }
}

/// The label, the dots and the summary slot of a card, for tests outside this module.
#[cfg(test)]
pub(crate) fn label_dots_summary(
    card: Rect,
    dots: usize,
    close: bool,
) -> (Rect, Option<Rect>, Option<Rect>) {
    let layout = Layout::new(card, dots, close, false);
    (layout.label, layout.dots, layout.summary)
}

/// A dot's ring for a claim of `class`: one whole ring in its full colour, whether or not anybody
/// holds the claim up — the owner wanted every ring the same at the desktop check (D96).
fn ring(painter: &egui::Painter, center: egui::Pos2, class: Attention, theme: &Theme) {
    painter.circle_stroke(
        center,
        RING_REACH - RING_BAND / 2.0,
        Stroke::new(RING_WIDTH, theme.attention_color(class, true)),
    );
}

/// Where the `n`th dot of a card's dots sits.
fn dot_center(dots: Rect, n: usize) -> egui::Pos2 {
    pos2(
        dots.left() + RING_REACH + n as f32 * DOT_STEP,
        dots.center().y,
    )
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

/// Paints every card and the add button; returns how many cards painted at least one ring — what
/// was drawn, not what was claimed, so a claiming card too narrow even for one dot does not count.
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
        // News, not a claim: in the label's own ink, and not counted among the rings.
        if let Some(glyph) = layout.unseen {
            crate::ui::icons::paint(
                &painter,
                glyph.center(),
                crate::ui::icons::Icon::File(crate::ui::icons::FileKind::Image),
                UNSEEN,
                ink,
            );
        }
        if let Some(dots) = layout.dots {
            let mut n = 0;
            let mut ringed = false;
            tab.root.walk(&mut |pane| {
                let center = dot_center(dots, n);
                n += 1;
                match &pane.project {
                    Some(project) => {
                        painter.circle_filled(center, DOT / 2.0, project.ink);
                    }
                    None => {
                        painter.circle_stroke(center, DOT / 2.0 - 0.5, Stroke::new(1.0, roles.tx3));
                    }
                }
                // The claim is the console's own, as its producer declared it (D18), and it is
                // drawn around the dot, not on it.
                if let Some(claim) = pane.effective_attention(now) {
                    ring(&painter, center, claim.class, &window.theme);
                    ringed = true;
                }
            });
            marked += usize::from(ringed);
        } else if let Some(summary) = layout.summary {
            // No room for every console's dot: one muted dot carries the loudest claim of them all,
            // by the same order `Tab::attention` uses, walked without collecting the panes.
            let mut loudest = None;
            tab.root
                .walk(&mut |pane| loudest = loudest.max(pane.effective_attention(now)));
            if let Some(claim) = loudest {
                let center = dot_center(summary, 0);
                painter.circle_filled(center, DOT / 2.0, roles.tx3);
                ring(&painter, center, claim.class, &window.theme);
                marked += 1;
            }
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
            // The square that keeps the cross the seven points across the drawn one was.
            crate::ui::icons::paint(
                &painter,
                close.center(),
                crate::ui::icons::Icon::Close,
                11.0,
                ink,
            );
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
        // The square that keeps the plus the ten points across the drawn one was.
        crate::ui::icons::paint(
            &painter,
            add.center(),
            crate::ui::icons::Icon::Plus,
            13.0,
            roles.tx3,
        );
    }
    marked
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

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
                    for (close, unseen) in
                        [(false, false), (true, false), (false, true), (true, true)]
                    {
                        for n in [0, 1, 4] {
                            let layout = Layout::new(card, n, close, unseen);
                            assert!(card.contains_rect(layout.body));
                            // The label starts at the padding the card's width earns.
                            assert_eq!(
                                layout.label.left(),
                                (card.left() + padding(card)).min(layout.body.right()),
                                "{width} {count} {n}"
                            );
                            // Every card the bar keeps can say that something claims: all its
                            // dots, or the summary dot — at no width does it lose both.
                            if n > 0 && card.width() >= MIN_CARD {
                                assert!(
                                    layout.dots.is_some() || layout.summary.is_some(),
                                    "{width} {count} {n}: no room for any claim"
                                );
                            }
                            assert!(layout.label.right() <= layout.body.right());
                            if let Some(close) = layout.close {
                                assert_eq!(layout.body.right(), close.left());
                            }
                            assert!(layout.dots.is_none() || layout.summary.is_none());
                            if let Some(dots) = layout.dots {
                                assert!(dots.left() >= layout.label.right() + 8.0);
                                assert!(dots.right() <= layout.body.right());
                                // The last of the `n` painted rings, stroke included, ends inside
                                // the dots' rect.
                                let painted = RING_REACH - RING_BAND / 2.0 + RING_WIDTH / 2.0;
                                assert!(dot_center(dots, n - 1).x + painted <= dots.right() + 1e-3);
                            }
                            if let Some(summary) = layout.summary {
                                assert!(summary.left() >= layout.label.right() + 8.0);
                                assert_eq!(summary.right(), dots_end(&layout));
                                assert_eq!(summary.width(), 2.0 * RING_REACH);
                            }
                            // The glyph sits between the label and the dots, touching neither, and
                            // never takes the label under the floor its dots leave it.
                            assert!(unseen || layout.unseen.is_none());
                            if let Some(glyph) = layout.unseen {
                                assert!(glyph.left() >= layout.label.right() + LABEL_AIR);
                                let next = layout.dots.or(layout.summary);
                                assert!(next.is_none_or(|d| glyph.right() + LABEL_AIR <= d.left()));
                                assert!(glyph.right() <= layout.body.right());
                                assert!(layout.label.width() >= SUMMARY_LABEL_FLOOR);
                            }
                        }
                    }
                }
                let end = add_target(band, count)
                    .map_or_else(|| card(band, count - 1, count).right(), |add| add.right());
                assert_eq!(occupied(band, count).map(|r| r.right()), Some(end));
            }
        }
        // No slot is left of the label: it starts at the card's padding.
        let wide = Rect::from_min_size(pos2(0.0, 6.0), vec2(1200.0, 34.0));
        let layout = Layout::new(card(wide, 0, 1), 0, false, false);
        assert_eq!(layout.label.left(), PADDING);
        // A roomy card always has the glyph's room.
        assert!(
            Layout::new(card(wide, 0, 1), 4, true, true)
                .unseen
                .is_some()
        );
    }

    /// Where the dots end: the body's right edge with the cross, its padding short of it without
    /// (the body is the whole card then).
    fn dots_end(layout: &Layout) -> f32 {
        if layout.close.is_some() {
            layout.body.right()
        } else {
            layout.body.right() - padding(layout.body)
        }
    }

    /// Four consoles keep their dots, rings included, down to a 128-point card with the cross
    /// shown; the dots are 16 points apart and their rect is 60 wide. Below that the card keeps one
    /// 12-point summary slot at every width down to the narrowest card the bar keeps, through the
    /// two places the card's air and its cross change, so the claim never blinks while resizing.
    #[test]
    fn four_dots_fit_down_to_128_points_and_one_summary_slot_down_to_the_narrowest_card() {
        let at = |width: f32| Rect::from_min_size(pos2(0.0, 6.0), vec2(width, 34.0));
        let fits = Layout::new(at(128.0), 4, true, false);
        let dots = fits.dots.expect("128 points hold four dots");
        assert_eq!(fits.summary, None);
        assert_eq!(dots.width(), 60.0);
        assert_eq!(dot_center(dots, 1).x - dot_center(dots, 0).x, 16.0);
        let short = Layout::new(at(127.0), 4, true, false);
        assert_eq!(short.dots, None);
        let summary = short.summary.expect("127 points hold one summary slot");
        assert_eq!(summary.right(), 127.0 - CLOSE_WIDTH);
        assert_eq!(summary.width(), 12.0);
        for tenth in (MIN_CARD * 10.0) as u32..1270 {
            let layout = Layout::new(at(tenth as f32 / 10.0), 4, true, false);
            assert!(layout.dots.is_none() && layout.summary.is_some(), "{tenth}");
        }
    }
}
