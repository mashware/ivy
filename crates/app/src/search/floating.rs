//! The search's box: where it sits over its console, how it is painted, and what a press on it
//! reaches. **D61**.
//!
//! **Painted over the console and never cut out of it**, like the line it replaced: cutting room
//! for it would resize the program every time a search opens. **And painted, not built from egui's
//! text field**, because a text field takes every key it is given, and D61 hands every key that is
//! not the search's to the program — which closes the search. The query is edited by the keys
//! [`super::Steps`] already takes; the box adds what a mouse can do to it.

use eframe::egui::{self, Painter, Pos2, Rect, pos2, vec2};
use term_core::{Options, View};

use super::{Choice, Mode, Step};
use crate::painter::CellMetrics;
use crate::theme::Theme;
use crate::ui::icons::{Icon, paint as paint_icon};

/// Room between the box and the console's edges.
const MARGIN: f32 = 6.0;
/// Room between the box's edge and what is inside it.
const PAD: f32 = 4.0;
/// How many cells the query field asks for when the console has room for them.
const FIELD_CELLS: f32 = 26.0;
/// How many cells an option's row asks for, its label and its box included.
const LIST_CELLS: f32 = 18.0;

/// What a press on the box can reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// The query, or any part of the box that is not a button: it hands its console the keyboard.
    Field,
    Previous,
    Next,
    Options,
    Close,
    Choice(Choice),
}

impl Target {
    /// What the press asks of the search. **The same steps the keys ask for**: previous is
    /// `Shift+Enter`, next is `Enter`, close is `Escape`.
    pub(crate) fn step(self) -> Option<Step> {
        match self {
            Self::Field => None,
            Self::Previous => Some(Step::Go(term_core::Toward::Newer)),
            Self::Next => Some(Step::Go(term_core::Toward::Older)),
            Self::Options => Some(Step::List),
            Self::Close => Some(Step::Close),
            Self::Choice(choice) => Some(Step::Flip(choice)),
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Field => "",
            Self::Previous => "Previous match, newer (Shift+Enter)",
            Self::Next => "Next match, older (Enter)",
            Self::Options => "Search options",
            Self::Close => "Close search (Escape)",
            Self::Choice(choice) => choice.label(),
        }
    }
}

/// Where the box's parts are, worked out from the console, what is selected on screen and the
/// notices over it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Layout {
    /// The box, without its list.
    pub(crate) frame: Rect,
    pub(crate) list: Option<Rect>,
    pub(crate) targets: Vec<(Target, Rect)>,
    /// How many of the console's rows the box takes when it sits at the top: a step keeps its
    /// match below them wherever the view can scroll, so the box seldom has to move.
    pub(crate) clear_rows: usize,
}

/// What the painter recorded of a box it painted: the pass, and where each part was.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Painted {
    pub(crate) frame: u64,
    pub(crate) layout: Layout,
}

impl Painted {
    /// What a press at `at` reaches, if it fell on the box or its list.
    ///
    /// **Only a box painted in the pass before this one answers**, D99's rule for a header's
    /// controls: one recorded on a console that then stopped being painted — a background tab, a
    /// sibling of a maximized console — must not take a press nobody saw it under.
    pub(crate) fn hit(&self, now: u64, at: Pos2) -> Option<Target> {
        if self.frame + 1 != now {
            return None;
        }
        let inside = |rect: &Rect| rect.contains(at) && at.x < rect.right() && at.y < rect.bottom();
        if let Some((target, _)) = self.layout.targets.iter().find(|(_, rect)| inside(rect)) {
            return Some(*target);
        }
        let on_the_box =
            inside(&self.layout.frame) || self.layout.list.is_some_and(|list| inside(&list));
        on_the_box.then_some(Target::Field)
    }
}

/// Where the box goes over `console`.
///
/// **At the top-right, and at the bottom-right when the selection on screen or a notice lies under
/// it** — the held paste's question and the «stopped receiving» sentence keep the console's top
/// (D41), and the box must neither hide them nor take their presses. It is placed from the console,
/// the selection and those notices only — **never from whether the list is unfolded**, so
/// unfolding it cannot move the button a second press is aimed at. Everything is kept inside the
/// console: a part painted outside it would be clipped away and still take presses there.
pub(crate) fn layout(
    console: Rect,
    metrics: &CellMetrics,
    view: &View,
    list: bool,
    notices: &[Rect],
) -> Layout {
    let button = metrics.height() + 2.0 * PAD;
    let room = (console.width() - 2.0 * MARGIN).max(0.0);
    let width = (FIELD_CELLS * metrics.width() + 4.0 * button + 2.0 * PAD).min(room);
    let height = button + 2.0 * PAD;
    let at_top = Rect::from_min_size(
        pos2(console.right() - MARGIN - width, console.top() + MARGIN),
        vec2(width, height),
    );
    let covered = selection_rects(view, console, metrics)
        .chain(notices.iter().copied())
        .any(|under| under.intersects(at_top));
    let frame = if covered {
        at_top.translate(vec2(0.0, console.height() - height - 2.0 * MARGIN))
    } else {
        at_top
    };
    let clear_rows = ((at_top.bottom() - console.top()) / metrics.height()).ceil() as usize;

    let mut targets = Vec::with_capacity(8);
    let row = Rect::from_min_size(
        pos2(frame.left() + PAD, frame.top() + PAD),
        vec2(frame.width() - 2.0 * PAD, button),
    );
    let mut right = row.right();
    // Right to left: close, options, then the two steps — down for newer, and up for older, which
    // is where `Enter` walks.
    for target in [Target::Close, Target::Options, Target::Previous, Target::Next] {
        let left = right - button;
        if left < row.left() {
            break;
        }
        targets.push((
            target,
            Rect::from_min_max(pos2(left, row.top()), pos2(right, row.bottom())),
        ));
        right = left;
    }
    targets.push((
        Target::Field,
        Rect::from_min_max(row.min, pos2(right, row.bottom())),
    ));

    let list = list.then(|| {
        let width = (LIST_CELLS * metrics.width() + 2.0 * PAD).min(room);
        let height = Choice::ALL.len() as f32 * button + 2.0 * PAD;
        let wanted = if covered {
            frame.top() - height
        } else {
            frame.bottom()
        };
        // Moved back inside the console when it does not fit on its side of the box.
        let top = wanted.min(console.bottom() - height).max(console.top());
        let list = Rect::from_min_size(pos2(frame.right() - width, top), vec2(width, height));
        for (index, choice) in Choice::ALL.into_iter().enumerate() {
            targets.push((
                Target::Choice(choice),
                Rect::from_min_size(
                    pos2(list.left() + PAD, list.top() + PAD + index as f32 * button),
                    vec2(list.width() - 2.0 * PAD, button),
                ),
            ));
        }
        list.intersect(console)
    });
    let frame = frame.intersect(console);
    for (_, rect) in &mut targets {
        *rect = rect.intersect(console);
    }
    targets.retain(|(_, rect)| rect.is_positive());
    Layout {
        frame,
        list,
        targets,
        clear_rows,
    }
}

/// The selected cells on screen, one rectangle per row they sit on.
fn selection_rects<'a>(
    view: &'a View,
    console: Rect,
    metrics: &'a CellMetrics,
) -> impl Iterator<Item = Rect> + 'a {
    let cols = usize::from(view.size().cols).max(1);
    view.cells()
        .chunks_exact(cols)
        .enumerate()
        .filter_map(move |(row, cells)| {
            let first = cells.iter().position(|cell| cell.selected)?;
            let last = cells.iter().rposition(|cell| cell.selected)?;
            Some(Rect::from_min_max(
                pos2(
                    console.left() + first as f32 * metrics.width(),
                    console.top() + row as f32 * metrics.height(),
                ),
                pos2(
                    console.left() + (last + 1) as f32 * metrics.width(),
                    console.top() + (row + 1) as f32 * metrics.height(),
                ),
            ))
        })
}

/// Paints the box over its console and says where it went, for the press of the next pass.
pub(crate) fn paint(
    ui: &egui::Ui,
    mode: &Mode,
    options: Options,
    view: &View,
    theme: &Theme,
    metrics: &CellMetrics,
    (console, notices): (Rect, &[Rect]),
) -> Painted {
    let layout = layout(console, metrics, view, mode.list(), notices);
    let painter = &ui.painter_at(console);
    let roles = crate::ui::style::roles(painter.ctx(), theme);
    let corner = 6.0;
    let stroke = egui::Stroke::new(1.0, roles.line2);
    for surface in std::iter::once(layout.frame).chain(layout.list) {
        painter.rect(
            surface,
            corner,
            roles.raised,
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    let pointer = ui.ctx().input(|input| input.pointer.hover_pos());
    let icon = metrics.height() * 0.85;
    for (index, (target, rect)) in layout.targets.iter().enumerate() {
        let hovered = pointer.is_some_and(|at| rect.contains(at));
        if *target != Target::Field {
            // Salted with the console's corner: every pane is painted with the same `ui`, and two
            // boxes sharing ids would merge their hover areas and tooltips.
            let id = ui.id().with((
                "search-box",
                console.min.x.to_bits(),
                console.min.y.to_bits(),
                index,
            ));
            ui.interact(*rect, id, egui::Sense::hover())
            .on_hover_text(target.hint());
        }
        match target {
            Target::Field => paint_the_field(painter, mode, *rect, &roles, metrics),
            Target::Choice(choice) => {
                if hovered {
                    painter.rect_filled(*rect, 4.0, roles.hover);
                }
                let tick = Rect::from_center_size(
                    pos2(rect.left() + rect.height() / 2.0, rect.center().y),
                    vec2(metrics.height() * 0.8, metrics.height() * 0.8),
                );
                painter.rect_stroke(tick, 3.0, stroke, egui::StrokeKind::Inside);
                if choice.is_on(options) {
                    paint_icon(painter, tick.center(), Icon::Check, icon * 0.8, roles.tx);
                }
                painter.text(
                    pos2(rect.left() + rect.height() + PAD, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    choice.label(),
                    metrics.font(),
                    roles.tx,
                );
            }
            button => {
                if hovered || (*button == Target::Options && mode.list()) {
                    painter.rect_filled(*rect, 4.0, roles.hover);
                }
                let glyph = match button {
                    // `Enter` walks toward older lines, which are up the screen.
                    Target::Next => Icon::CaretUp,
                    Target::Previous => Icon::Caret { open: true },
                    Target::Options => Icon::Gear,
                    _ => Icon::Close,
                };
                paint_icon(painter, rect.center(), glyph, icon, roles.tx);
            }
        }
    }
    Painted {
        frame: ui.ctx().cumulative_frame_nr(),
        layout,
    }
}

fn paint_the_field(
    painter: &Painter,
    mode: &Mode,
    rect: Rect,
    roles: &crate::theme::Roles,
    metrics: &CellMetrics,
) {
    painter.rect(
        rect,
        4.0,
        roles.base,
        egui::Stroke::new(1.0, roles.line2),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink2(vec2(PAD, 0.0));
    let clipped = painter.with_clip_rect(inner.intersect(painter.clip_rect()));
    // The status gives way to the query: it is left out where it would leave fewer than six
    // cells for what was typed.
    let wanted = mode.status();
    let fits = clipped
        .layout_no_wrap(wanted.to_owned(), metrics.font(), roles.tx2)
        .size()
        .x
        + 6.0 * metrics.width()
        <= inner.width();
    let status = if fits { wanted } else { "" };
    let said = clipped.layout_no_wrap(status.to_owned(), metrics.font(), roles.tx2);
    let status_left = inner.right() - said.size().x;
    clipped.galley(
        pos2(status_left, inner.center().y - said.size().y / 2.0),
        said,
        roles.tx2,
    );
    // The query keeps its end in view: what was typed last is what a person checks.
    let query = clipped.layout_no_wrap(format!("{}_", mode.query()), metrics.font(), roles.tx);
    let room = (status_left - inner.left() - PAD).max(0.0);
    let left = inner.left() - (query.size().x - room).max(0.0);
    let query_clip = Rect::from_min_max(inner.min, pos2(inner.left() + room, inner.bottom()));
    painter
        .with_clip_rect(query_clip.intersect(painter.clip_rect()))
        .galley(
            pos2(left, inner.center().y - query.size().y / 2.0),
            query,
            roles.tx,
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use term_core::{Screen, Search, Size, Toward};

    const ROWS: u16 = 10;

    fn metrics() -> CellMetrics {
        CellMetrics::of(10.0, 20.0)
    }

    fn console() -> Rect {
        Rect::from_min_size(pos2(100.0, 50.0), vec2(800.0, f32::from(ROWS) * 20.0))
    }

    /// A screen whose only «needle» sits on `row`, at the right end of the line so it lies under
    /// a box at the top-right, and selected.
    fn with_the_match_on(row: u16) -> Screen {
        let mut screen = Screen::new(Size {
            rows: ROWS,
            cols: 80,
        });
        for n in 0..ROWS {
            if n == row {
                screen.advance(format!("{:>79}", "needle").as_bytes());
            } else {
                screen.advance(format!("line {n}").as_bytes());
            }
            if n + 1 < ROWS {
                screen.advance(b"\r\n");
            }
        }
        let mut search = Search::literal("needle", Options::default()).unwrap();
        assert_eq!(
            screen.find(&mut search, Toward::Older, 0),
            term_core::Found::Match,
            "the premise: the needle is found"
        );
        screen
    }

    #[test]
    fn the_box_sits_top_right_inside_its_console() {
        let screen = Screen::new(Size {
            rows: ROWS,
            cols: 80,
        });
        let layout = layout(console(), &metrics(), &screen.view(), false, &[]);
        assert!(console().contains_rect(layout.frame), "{layout:?}");
        assert_eq!(layout.frame.right(), console().right() - MARGIN);
        assert_eq!(layout.frame.top(), console().top() + MARGIN);
        let kinds: Vec<Target> = layout.targets.iter().map(|(target, _)| *target).collect();
        assert_eq!(
            kinds,
            vec![
                Target::Close,
                Target::Options,
                Target::Previous,
                Target::Next,
                Target::Field
            ]
        );
    }

    #[test]
    fn the_box_moves_to_the_bottom_when_the_match_lies_under_it_and_only_then() {
        let under = layout(console(), &metrics(), &with_the_match_on(0).view(), false, &[]);
        assert_eq!(
            under.frame.bottom(),
            console().bottom() - MARGIN,
            "the box stayed over the match it found"
        );
        let clear = layout(console(), &metrics(), &with_the_match_on(5).view(), false, &[]);
        assert_eq!(clear.frame.top(), console().top() + MARGIN);
    }

    #[test]
    fn unfolding_the_list_never_moves_the_box() {
        for row in [0, 5] {
            let screen = with_the_match_on(row);
            let folded = layout(console(), &metrics(), &screen.view(), false, &[]);
            let unfolded = layout(console(), &metrics(), &screen.view(), true, &[]);
            assert_eq!(folded.frame, unfolded.frame);
            let list = unfolded.list.expect("the list is laid out");
            assert!(
                console().contains_rect(list),
                "the list left its console: {list:?}"
            );
            assert!(
                !list.intersects(folded.frame.shrink(1.0)),
                "the list covers the box"
            );
            assert_eq!(
                unfolded
                    .targets
                    .iter()
                    .filter(|(target, _)| matches!(target, Target::Choice(_)))
                    .count(),
                3
            );
        }
    }

    #[test]
    fn a_press_reaches_the_box_only_if_it_was_painted_in_the_previous_pass() {
        let screen = Screen::new(Size {
            rows: ROWS,
            cols: 80,
        });
        let painted = Painted {
            frame: 7,
            layout: layout(console(), &metrics(), &screen.view(), false, &[]),
        };
        let (_, close) = painted.layout.targets[0];
        assert_eq!(painted.hit(8, close.center()), Some(Target::Close));
        assert_eq!(
            painted.hit(9, close.center()),
            None,
            "a stale box took a press"
        );
        assert_eq!(painted.hit(7, close.center()), None);
        let gap = pos2(
            painted.layout.frame.left() + 1.0,
            painted.layout.frame.top() + 1.0,
        );
        assert_eq!(
            painted.hit(8, gap),
            Some(Target::Field),
            "a press on the box between its parts went through it"
        );
        assert_eq!(painted.hit(8, console().center() + vec2(0.0, 60.0)), None);
    }

    #[test]
    fn a_match_on_the_top_row_but_clear_of_the_box_leaves_it_at_the_top() {
        let mut screen = Screen::new(Size { rows: ROWS, cols: 80 });
        screen.advance(b"needle");
        let mut search = Search::literal("needle", Options::default()).unwrap();
        assert_eq!(
            screen.find(&mut search, Toward::Older, 0),
            term_core::Found::Match
        );
        let layout = layout(console(), &metrics(), &screen.view(), false, &[]);
        assert_eq!(layout.frame.top(), console().top() + MARGIN);
    }

    #[test]
    fn a_notice_at_the_top_sends_the_box_to_the_bottom() {
        let screen = Screen::new(Size { rows: ROWS, cols: 80 });
        let notice = Rect::from_min_size(console().min, vec2(console().width(), 60.0));
        let layout = layout(console(), &metrics(), &screen.view(), false, &[notice]);
        assert!(
            !layout.frame.intersects(notice),
            "the box covers the notice: {layout:?}"
        );
        let beside = Rect::from_min_size(console().min, vec2(100.0, 60.0));
        let layout = layout_with(&screen, &[beside]);
        assert_eq!(
            layout.frame.top(),
            console().top() + MARGIN,
            "a notice clear of the box moved it"
        );
    }

    fn layout_with(screen: &Screen, notices: &[Rect]) -> Layout {
        layout(console(), &metrics(), &screen.view(), false, notices)
    }

    #[test]
    fn on_a_short_console_every_part_stays_inside_it() {
        let short = Rect::from_min_size(pos2(100.0, 50.0), vec2(800.0, 60.0));
        let screen = Screen::new(Size { rows: 3, cols: 80 });
        for covered in [false, true] {
            let notices = if covered { vec![short] } else { Vec::new() };
            let layout = layout(short, &metrics(), &screen.view(), true, &notices);
            assert!(short.contains_rect(layout.frame), "{layout:?}");
            let list = layout.list.expect("the list is laid out");
            assert!(short.contains_rect(list), "the list left its console: {list:?}");
            for (target, rect) in &layout.targets {
                assert!(short.contains_rect(*rect), "{target:?} outside the console: {rect:?}");
            }
            assert!(
                layout
                    .targets
                    .iter()
                    .any(|(target, _)| matches!(target, Target::Choice(_))),
                "the options were dropped instead of moved inside"
            );
        }
    }

    #[test]
    fn each_option_reads_and_shows_its_own_choice() {
        let labels: Vec<&str> = Choice::ALL.iter().map(|choice| choice.label()).collect();
        assert_eq!(labels, vec!["Match case", "Whole word only", "Wrap around"]);
        for choice in Choice::ALL {
            let mut options = Options {
                match_case: false,
                whole_word: false,
                wrap: false,
            };
            choice.flip(&mut options);
            for other in Choice::ALL {
                assert_eq!(
                    other.is_on(options),
                    other == choice,
                    "flipping {choice:?} showed {other:?} as {}",
                    other.is_on(options)
                );
            }
        }
    }

    #[test]
    fn the_buttons_ask_what_their_keys_ask() {
        assert_eq!(Target::Next.step(), Some(Step::Go(Toward::Older)));
        assert_eq!(Target::Previous.step(), Some(Step::Go(Toward::Newer)));
        assert_eq!(Target::Close.step(), Some(Step::Close));
        assert_eq!(Target::Options.step(), Some(Step::List));
        assert_eq!(
            Target::Choice(Choice::Wrap).step(),
            Some(Step::Flip(Choice::Wrap))
        );
        assert_eq!(Target::Field.step(), None);
    }
}
