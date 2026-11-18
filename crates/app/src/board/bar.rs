//! The Board's top bar on the kit: totals, the arrangement toggle, the narrowing and Close.
//!
//! Its controls are painted and pressed through the Board's own hit list, like a card's fold
//! (D93: the Board owns its painted child hit geometry), so one input pass decides what a press on
//! the Board reaches and the bar never competes with a card for it.
use super::{
    cards::Command,
    filter::{Arrangement, Filter},
};
use crate::theme::Roles;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Rect, Stroke, pos2, vec2};

pub(super) const HEIGHT: f32 = 48.0;
/// The Board's side gutter: the bar's controls and the arrangement below it share one edge.
pub(super) const GUTTER: i8 = 16;
const CONTROL: f32 = 28.0;
const GAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Arrange(Arrangement),
    ClaimingOnly(bool),
    EditQuery,
    ClearQuery,
    Close,
}

/// What the bar says about the whole Board, counted before any narrowing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Totals {
    pub(super) consoles: usize,
    pub(super) projects: usize,
    pub(super) claiming: usize,
}

/// Applies a bar press to the Board's own state; `true` asks the caller to close the Board.
pub(super) fn apply(arrangement: &mut Arrangement, filter: &mut Filter, action: Action) -> bool {
    match action {
        Action::Arrange(choice) => *arrangement = choice,
        Action::ClaimingOnly(only) => filter.claiming_only = only,
        Action::EditQuery => filter.editing = true,
        Action::ClearQuery => {
            filter.query.clear();
            filter.editing = false;
        }
        Action::Close => return true,
    }
    false
}

fn text_width(ui: &egui::Ui, text: &str, font: &FontId) -> f32 {
    ui.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE))
        .size()
        .x
}

/// One segmented control: the selected segment is filled with `hover` and inked `tx`, the others
/// read `tx2` over the container's `base`.
fn segmented(
    ui: &egui::Ui,
    left: f32,
    center_y: f32,
    segments: &[(String, bool, Action)],
    roles: &Roles,
    hits: &mut Vec<(Rect, Command)>,
) -> f32 {
    let font = FontId::proportional(12.5);
    let widths: Vec<f32> = segments
        .iter()
        .map(|(label, ..)| text_width(ui, label, &font) + 20.0)
        .collect();
    let outer = Rect::from_min_size(
        pos2(left, center_y - CONTROL / 2.0),
        vec2(widths.iter().sum::<f32>() + 4.0, CONTROL),
    );
    let painter = ui.painter();
    painter.rect(
        outer,
        CornerRadius::same(6),
        roles.base,
        Stroke::new(1.0, roles.line2),
        egui::StrokeKind::Inside,
    );
    let mut x = outer.left() + 2.0;
    for ((label, selected, action), width) in segments.iter().zip(widths) {
        let segment = Rect::from_min_size(pos2(x, outer.top() + 2.0), vec2(width, CONTROL - 4.0));
        if *selected {
            painter.rect_filled(segment, CornerRadius::same(4), roles.hover);
        }
        painter.text(
            segment.center(),
            Align2::CENTER_CENTER,
            label,
            font.clone(),
            if *selected { roles.tx } else { roles.tx2 },
        );
        hits.push((segment, Command::Bar(*action)));
        x += width;
    }
    outer.right()
}

/// One bar row; a narrow Board stacks rows instead of letting controls overlap or vanish.
const ROW: f32 = 40.0;
const FIELD_MIN: f32 = 140.0;
const FIELD_MAX: f32 = 240.0;

fn segment_width(ui: &egui::Ui, segments: &[(String, bool, Action)]) -> f32 {
    let font = FontId::proportional(12.5);
    segments
        .iter()
        .map(|(label, ..)| text_width(ui, label, &font) + 20.0)
        .sum::<f32>()
        + 4.0
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    ui: &mut egui::Ui,
    width: f32,
    totals: Totals,
    arrangement: Arrangement,
    filter: &Filter,
    roles: &Roles,
    hits: &mut Vec<(Rect, Command)>,
) -> Rect {
    let title_font = FontId::new(15.0, crate::fonts::ui_weight(ui.ctx(), 600));
    let small = FontId::proportional(12.5);
    let summary = format!(
        "{} {} · {} {}",
        totals.consoles,
        if totals.consoles == 1 {
            "console"
        } else {
            "consoles"
        },
        totals.projects,
        if totals.projects == 1 {
            "project"
        } else {
            "projects"
        },
    );
    let arrange: [(String, bool, Action); 2] = [
        (
            "By state".into(),
            arrangement == Arrangement::ByState,
            Action::Arrange(Arrangement::ByState),
        ),
        (
            "By milestone".into(),
            arrangement == Arrangement::ByMilestone,
            Action::Arrange(Arrangement::ByMilestone),
        ),
    ];
    let narrowing: [(String, bool, Action); 2] = [
        (
            "All".into(),
            !filter.claiming_only,
            Action::ClaimingOnly(false),
        ),
        (
            format!("Claiming you  {}", totals.claiming),
            filter.claiming_only,
            Action::ClaimingOnly(true),
        ),
    ];
    let title_w = text_width(ui, "Board", &title_font);
    let summary_w = text_width(ui, &summary, &small);
    let close_w = text_width(ui, "Close", &small) + 20.0 + 34.0;
    let controls = [
        segment_width(ui, &arrange),
        segment_width(ui, &narrowing),
        FIELD_MIN,
    ];
    let lead = 16.0 + title_w + 10.0 + summary_w + 16.0;
    let one_row = lead + controls.iter().map(|w| w + GAP).sum::<f32>() + close_w + 16.0 <= width;
    // Where each control goes: (row, left). On one row they follow the summary; otherwise they flow
    // from the left edge onto rows under the title, wrapping before they would cross the edge.
    let mut placed = [(0usize, 0.0f32); 3];
    let mut rows = 1;
    if one_row {
        let mut x = lead;
        for (slot, w) in placed.iter_mut().zip(controls) {
            *slot = (0, x);
            x += w + GAP;
        }
    } else {
        let (mut row, mut x) = (1, 16.0);
        for (slot, w) in placed.iter_mut().zip(controls) {
            if x > 16.0 && x + w > width - 16.0 {
                row += 1;
                x = 16.0;
            }
            *slot = (row, x);
            x += w + GAP;
        }
        rows = row + 1;
    }
    let height = if one_row {
        HEIGHT
    } else {
        8.0 + ROW * rows as f32
    };
    let (bar, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(bar, 0.0, roles.chrome);
    painter.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, roles.line),
    );
    let center = |row: usize| {
        if one_row {
            bar.center().y
        } else {
            bar.top() + 4.0 + ROW * (row as f32 + 0.5)
        }
    };
    let y = center(0);
    painter.text(
        pos2(bar.left() + 16.0, y),
        Align2::LEFT_CENTER,
        "Board",
        title_font,
        roles.tx,
    );
    // The summary is the one thing the bar drops when even its own row is short.
    if one_row || 16.0 + title_w + 10.0 + summary_w + GAP + close_w + 16.0 <= width {
        painter.text(
            pos2(bar.left() + 16.0 + title_w + 10.0, y),
            Align2::LEFT_CENTER,
            summary,
            small.clone(),
            roles.tx3,
        );
    }
    let close = Rect::from_min_size(
        pos2(bar.right() - 16.0 - close_w, y - CONTROL / 2.0),
        vec2(close_w, CONTROL),
    );
    painter.rect(
        close,
        CornerRadius::same(6),
        roles.raised,
        Stroke::new(1.0, roles.line2),
        egui::StrokeKind::Inside,
    );
    painter.text(
        pos2(close.left() + 10.0, y),
        Align2::LEFT_CENTER,
        "Close",
        small,
        roles.tx,
    );
    painter.text(
        pos2(close.right() - 10.0, y),
        Align2::RIGHT_CENTER,
        "Esc",
        FontId::monospace(11.0),
        roles.tx3,
    );
    // Close goes first so nothing laid out after it can take its presses.
    hits.push((close, Command::Bar(Action::Close)));
    let (row, x) = placed[0];
    segmented(ui, bar.left() + x, center(row), &arrange, roles, hits);
    let (row, x) = placed[1];
    segmented(ui, bar.left() + x, center(row), &narrowing, roles, hits);
    let (row, x) = placed[2];
    let right = if row == 0 {
        close.left() - GAP
    } else {
        bar.right() - 16.0
    };
    let field_width = (right - (bar.left() + x)).clamp(1.0, FIELD_MAX);
    paint_field(
        &painter,
        Rect::from_min_size(
            pos2(bar.left() + x, center(row) - CONTROL / 2.0),
            vec2(field_width, CONTROL),
        ),
        filter,
        roles,
        hits,
    );
    bar
}

fn paint_field(
    painter: &egui::Painter,
    field: Rect,
    filter: &Filter,
    roles: &Roles,
    hits: &mut Vec<(Rect, Command)>,
) {
    let (width, ink) = if filter.editing {
        (2.0, roles.accent)
    } else {
        (1.0, roles.line2)
    };
    painter.rect(
        field,
        CornerRadius::same(6),
        roles.base,
        Stroke::new(width, ink),
        egui::StrokeKind::Inside,
    );
    crate::ui::icons::paint(
        painter,
        pos2(field.left() + 14.0, field.center().y),
        crate::ui::icons::Icon::Search,
        13.0,
        roles.tx3,
    );
    let text_at = pos2(field.left() + 26.0, field.center().y);
    let clip = Rect::from_min_max(
        pos2(text_at.x, field.top()),
        pos2(field.right() - 24.0, field.bottom()),
    );
    let clipped = painter.with_clip_rect(clip.intersect(painter.clip_rect()));
    if filter.query.is_empty() {
        clipped.text(
            text_at,
            Align2::LEFT_CENTER,
            "Filter consoles",
            FontId::proportional(12.5),
            roles.tx3,
        );
    } else {
        let shown = clipped.text(
            text_at,
            Align2::LEFT_CENTER,
            &filter.query,
            FontId::proportional(12.5),
            roles.tx,
        );
        let clear = Rect::from_center_size(
            pos2(field.right() - 12.0, field.center().y),
            vec2(16.0, 16.0),
        );
        crate::ui::icons::paint(
            painter,
            clear.center(),
            crate::ui::icons::Icon::Close,
            // So it draws about as wide as the 13-point text cross it replaced, under the magnifier.
            10.0,
            roles.tx2,
        );
        hits.push((clear, Command::Bar(Action::ClearQuery)));
        if filter.editing {
            let caret = shown.right().min(clip.right()) + 1.0;
            painter.line_segment(
                [
                    pos2(caret, field.center().y - 7.0),
                    pos2(caret, field.center().y + 7.0),
                ],
                Stroke::new(1.0, roles.accent),
            );
        }
    }
    // The clear button was pushed first so a press on it is not taken by the field behind it.
    hits.push((field, Command::Bar(Action::EditQuery)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted(
        filter: &Filter,
        arrangement: Arrangement,
    ) -> (Vec<egui::epaint::ClippedShape>, Vec<(Rect, Command)>, Rect) {
        painted_at(1200.0, filter, arrangement)
    }
    fn painted_at(
        width: f32,
        filter: &Filter,
        arrangement: Arrangement,
    ) -> (Vec<egui::epaint::ClippedShape>, Vec<(Rect, Command)>, Rect) {
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let roles = Roles::of(&crate::theme::Theme::default());
        let mut hits = Vec::new();
        let mut bar = Rect::NOTHING;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            bar = paint(
                ui,
                width,
                Totals {
                    consoles: 7,
                    projects: 3,
                    claiming: 2,
                },
                arrangement,
                filter,
                &roles,
                &mut hits,
            );
        });
        let shapes = output.shapes.clone();
        output.drop_without_applying_deltas();
        (shapes, hits, bar)
    }
    fn text<'a>(
        shapes: &'a [egui::epaint::ClippedShape],
        needle: &str,
    ) -> Option<&'a egui::epaint::TextShape> {
        shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == needle => Some(t),
            _ => None,
        })
    }
    fn hit(hits: &[(Rect, Command)], action: Action) -> Rect {
        hits.iter()
            .find(|(_, c)| matches!(c, Command::Bar(a) if *a == action))
            .map(|(r, _)| *r)
            .expect("control painted")
    }

    #[test]
    fn the_bar_names_the_totals_and_every_control_sits_inside_it() {
        let (shapes, hits, bar) = painted(&Filter::default(), Arrangement::ByState);
        assert!(text(&shapes, "7 consoles · 3 projects").is_some_and(|t| bar.contains(t.pos)));
        assert!(text(&shapes, "Claiming you  2").is_some());
        for action in [
            Action::Arrange(Arrangement::ByState),
            Action::Arrange(Arrangement::ByMilestone),
            Action::ClaimingOnly(false),
            Action::ClaimingOnly(true),
            Action::EditQuery,
            Action::Close,
        ] {
            assert!(
                bar.contains_rect(hit(&hits, action)),
                "{action:?} outside the bar"
            );
        }
        let close = hit(&hits, Action::Close);
        assert!(
            close.right() > hit(&hits, Action::EditQuery).right(),
            "Close is the rightmost control"
        );
    }

    #[test]
    fn the_chosen_segment_is_inked_and_filled_and_the_other_is_not() {
        let roles = Roles::of(&crate::theme::Theme::default());
        let (shapes, hits, _) = painted(&Filter::default(), Arrangement::ByMilestone);
        assert_eq!(
            text(&shapes, "By milestone").unwrap().fallback_color,
            roles.tx
        );
        assert_eq!(text(&shapes, "By state").unwrap().fallback_color, roles.tx2);
        let chosen = hit(&hits, Action::Arrange(Arrangement::ByMilestone));
        let other = hit(&hits, Action::Arrange(Arrangement::ByState));
        let filled = |r: Rect| {
            shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(f) if f.rect == r && f.fill == roles.hover))
        };
        assert!(filled(chosen));
        assert!(!filled(other));
    }

    #[test]
    fn a_query_shows_a_clear_control_ahead_of_the_field_and_presses_change_state() {
        let filter = Filter {
            query: "api".into(),
            editing: true,
            ..Filter::default()
        };
        let (_, hits, _) = painted(&filter, Arrangement::ByState);
        let clear = hits
            .iter()
            .position(|(_, c)| matches!(c, Command::Bar(Action::ClearQuery)));
        let field = hits
            .iter()
            .position(|(_, c)| matches!(c, Command::Bar(Action::EditQuery)));
        assert!(
            clear.unwrap() < field.unwrap(),
            "the clear press must be found before the field's"
        );
        let mut arrangement = Arrangement::ByState;
        let mut filter = filter;
        assert!(!apply(
            &mut arrangement,
            &mut filter,
            Action::Arrange(Arrangement::ByMilestone)
        ));
        assert_eq!(arrangement, Arrangement::ByMilestone);
        assert!(!apply(
            &mut arrangement,
            &mut filter,
            Action::ClaimingOnly(true)
        ));
        assert!(filter.claiming_only);
        assert!(!apply(&mut arrangement, &mut filter, Action::ClearQuery));
        assert_eq!((filter.query.as_str(), filter.editing), ("", false));
        assert!(apply(&mut arrangement, &mut filter, Action::Close));
    }

    #[test]
    fn a_narrow_bar_stacks_its_controls_instead_of_hiding_or_overlapping_them() {
        let filter = Filter {
            query: "api".into(),
            ..Filter::default()
        };
        for width in [1200.0, 760.0, 520.0, 360.0] {
            let (_, hits, bar) = painted_at(width, &filter, Arrangement::ByState);
            let bars: Vec<Rect> = hits
                .iter()
                .filter(|(_, c)| !matches!(c, Command::Bar(Action::ClearQuery)))
                .map(|(r, _)| *r)
                .collect();
            assert!(
                bars.len() >= 6,
                "{width}: every control is painted, the field included"
            );
            for (i, a) in bars.iter().enumerate() {
                assert!(bar.contains_rect(*a), "{width}: {a:?} outside the bar");
                for b in &bars[i + 1..] {
                    if a.intersects(*b) {
                        let inner = a.intersect(*b);
                        // Segments of one control share an edge, never an area.
                        assert!(
                            inner.width() < 0.5 || inner.height() < 0.5,
                            "{width}: {a:?} overlaps {b:?}"
                        );
                    }
                }
            }
        }
        let (_, _, wide) = painted_at(1200.0, &filter, Arrangement::ByState);
        let (_, _, narrow) = painted_at(520.0, &filter, Arrangement::ByState);
        assert!(narrow.height() > wide.height(), "a narrow bar grows rows");
    }
}
