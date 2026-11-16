//! Pure body rendering shared by independently interactive panel surfaces.
use super::{Row, color, fold, links};
use crate::{
    fonts::Discoverer,
    painter::{CellMetrics, ColorGlyph},
    theme::Theme,
};
use eframe::egui::{Painter, Pos2, Rect};
use panel_contract::{Mark, Panel, Style};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(crate) enum Action {
    Fold(fold::HeaderId, bool),
    Link(String),
}
#[derive(Default)]
pub(crate) struct Reader {
    folds: fold::Folds,
    revision: u64,
}
impl Reader {
    pub(crate) fn apply(&mut self, action: &Action) {
        if let Action::Fold(key, closed) = action {
            self.folds.insert(key.clone(), !closed);
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
}
struct Prefix {
    mark: Option<(Mark, eframe::egui::Color32)>,
    reference: Option<Row>,
}
pub(crate) struct Body {
    rows: Vec<Row>,
    prefixes: HashMap<usize, Prefix>,
    pub(crate) hidden: usize,
}
impl Body {
    pub(crate) fn layout(
        panel: &Panel,
        reader: &Reader,
        width: usize,
        theme: &Theme,
        full: bool,
    ) -> Self {
        let mut document = panel.clone();
        let mut title = false;
        let mut closed = false;
        let mut shown = 0;
        let mut hidden = 0;
        document.lines.retain(|line| {
            if !title && line.style == Some(Style::Title) {
                title = true;
                return false;
            }
            if line.style == Some(Style::Section) || fold::is_gap(line) {
                closed = false;
            }
            if closed {
                return false;
            }
            if shown == 0 && fold::is_gap(line) {
                return false;
            }
            if !full && shown >= 18 {
                hidden += 1;
                return false;
            }
            shown += 1;
            if line.style == Some(Style::Section) {
                let key = match &line.key {
                    Some(key) => fold::HeaderId::Key(key.clone()),
                    None => fold::HeaderId::Text(line.text.clone()),
                };
                closed = reader.folds.get(&key).copied().unwrap_or(line.collapsed);
            }
            true
        });
        let column = panel
            .lines
            .iter()
            .filter(|l| l.style != Some(Style::Section))
            .filter_map(|l| l.reference.as_deref())
            .map(|s| super::sanitize(s).chars().count())
            .max()
            .unwrap_or(0)
            .min(12)
            .min(width.saturating_sub(super::MINIMUM_SLOT + 3));
        let mut rows = Vec::new();
        let mut prefixes = HashMap::new();
        for line in &document.lines {
            if line.style == Some(Style::Section) {
                let key = line.key.as_ref().map_or_else(
                    || fold::HeaderId::Text(line.text.clone()),
                    |key| fold::HeaderId::Key(key.clone()),
                );
                let mut row = Row::text(0, super::sanitize(&line.text), theme.foreground());
                row.header = Some(fold::Header {
                    id: key.clone(),
                    folded: reader.folds.get(&key).copied().unwrap_or(line.collapsed),
                    summary: super::sanitize(line.summary.as_deref().unwrap_or("")),
                });
                rows.push(row);
                continue;
            }
            let mut laid = super::lay_out_line(line, column, width, theme);
            let text_color = match line.style {
                Some(Style::Dim) => theme.foreground().gamma_multiply(0.5),
                Some(style) => theme.panel_color(None, Some(style)),
                None if line.mark == Some(Mark::Done) => theme.foreground().gamma_multiply(0.5),
                None => theme.foreground(),
            };
            for row in &mut laid {
                row.color = text_color;
            }
            if !fold::is_gap(line)
                && let Some(first) = laid.first_mut()
            {
                let count =
                    (2 + if column > 0 { column + 1 } else { 0 }).min(first.text.chars().count());
                first.text = format!(
                    "{}{}",
                    " ".repeat(count),
                    first.text.chars().skip(count).collect::<String>()
                );
                let mark = line.mark.map(|mark| {
                    let color = match mark {
                        Mark::Done | Mark::Pending => theme.foreground().gamma_multiply(0.5),
                        Mark::Current | Mark::Info => theme.focus(),
                        Mark::Wait | Mark::Block => theme.color(term_core::TermColor::Indexed(1)),
                    };
                    (mark, color)
                });
                let reference = line.reference.as_ref().map(|reference| {
                    Row::text(
                        2,
                        super::sanitize(reference).chars().take(column).collect(),
                        theme.foreground().gamma_multiply(0.5),
                    )
                });
                prefixes.insert(rows.len(), Prefix { mark, reference });
            }
            rows.extend(laid);
        }
        color::flag(&mut rows);
        for prefix in prefixes.values_mut() {
            if let Some(reference) = &mut prefix.reference {
                color::flag(std::slice::from_mut(reference));
            }
        }
        Self {
            rows,
            prefixes,
            hidden,
        }
    }
    pub(crate) fn height(&self, metrics: &CellMetrics) -> f32 {
        self.rows
            .iter()
            .enumerate()
            .map(|(n, row)| row_height(row, n, metrics))
            .sum()
    }
    pub(crate) fn rows(&self) -> usize {
        self.rows.len()
    }
    pub(crate) fn paint(
        &self,
        painter: &Painter,
        rect: Rect,
        metrics: &CellMetrics,
        theme: &Theme,
        discoverer: &mut Discoverer,
    ) -> Vec<(Rect, Action)> {
        let mut hits = Vec::new();
        let mut glyphs = Vec::new();
        let mut top = rect.top();
        for (n, row) in self.rows.iter().enumerate() {
            let step = row_height(row, n, metrics);
            let y = top
                + if row.header.is_some() && n > 0 {
                    4.0
                } else {
                    0.0
                };
            top += step;
            let row_rect = Rect::from_min_size(
                Pos2::new(rect.left(), y),
                eframe::egui::vec2(rect.width(), metrics.height()),
            );
            if !row_rect.intersects(painter.clip_rect()) {
                continue;
            }
            let at = Pos2::new(rect.left() + row.column as f32 * metrics.width(), y);
            discoverer.note_text(&row.text);
            let ink = if row.header.is_some() {
                paint_board_header(
                    painter,
                    Rect::from_min_size(at, eframe::egui::vec2(rect.width(), metrics.height())),
                    row,
                    metrics,
                    theme,
                    discoverer,
                    &mut glyphs,
                )
            } else {
                color::paint_text(painter, at, row, metrics, discoverer, &mut glyphs)
            };
            if let Some(prefix) = self.prefixes.get(&n) {
                if let Some((mark, ink)) = prefix.mark {
                    paint_mark(
                        painter,
                        Rect::from_min_size(
                            Pos2::new(rect.left(), y),
                            eframe::egui::vec2(12.0, metrics.height()),
                        ),
                        mark,
                        ink,
                        metrics.font(),
                    );
                }
                if let Some(reference) = &prefix.reference {
                    let origin =
                        Pos2::new(rect.left() + reference.column as f32 * metrics.width(), y);
                    discoverer.note_text(&reference.text);
                    color::paint_text(painter, origin, reference, metrics, discoverer, &mut glyphs);
                }
            }
            if let Some(header) = &row.header
                && painter.clip_rect().contains_rect(row_rect)
                && ink.right() <= rect.right()
            {
                hits.push((row_rect, Action::Fold(header.id.clone(), header.folded)));
            }
            if let Some(link) = &row.link {
                let painted =
                    links::paint_label_in(painter, rect, link, y, metrics, theme, discoverer);
                for link in links::reachable(vec![painted]) {
                    if painter.clip_rect().contains_rect(link.area) {
                        hits.push((link.area, Action::Link(link.target)));
                    }
                }
            }
        }
        for glyph in glyphs {
            glyph.paint(painter);
        }
        hits
    }
}

fn row_height(row: &Row, index: usize, metrics: &CellMetrics) -> f32 {
    if row.text.is_empty() && row.header.is_none() && row.link.is_none() {
        return 6.0;
    }
    metrics.height()
        + 2.0
        + if row.header.is_some() && index > 0 {
            4.0
        } else {
            0.0
        }
}

fn paint_board_header(
    painter: &Painter,
    rect: Rect,
    row: &Row,
    metrics: &CellMetrics,
    theme: &Theme,
    discoverer: &mut Discoverer,
    glyphs: &mut Vec<ColorGlyph>,
) -> Rect {
    let at = rect.min;
    let header = row.header.as_ref().unwrap();
    let dim = theme.foreground().gamma_multiply(0.5);
    let measure = |text: &str| {
        painter
            .layout_no_wrap(text.to_owned(), metrics.font(), theme.foreground())
            .size()
            .x
    };
    let summary: String = header.summary.chars().take(40).collect();
    let summary = fold::fit(&summary, (rect.width() - 24.0).max(0.0), &measure);
    let summary_left = rect.right() - measure(summary);
    let title = fold::fit(&row.text, (summary_left - at.x - 36.0).max(0.0), &measure);
    paint_chevron(
        painter,
        Rect::from_min_size(at, eframe::egui::vec2(8.0, metrics.height())),
        !header.folded,
        dim,
    );
    let mut pieces = [
        Row::text(0, title.into(), theme.foreground()),
        Row::text(0, summary.into(), dim),
    ];
    color::flag(&mut pieces);
    let mut title_ink = Rect::NOTHING;
    for (index, (piece, x)) in pieces.iter().zip([at.x + 14.0, summary_left]).enumerate() {
        discoverer.note_text(&piece.text);
        let ink = color::paint_text(
            painter,
            Pos2::new(x, at.y),
            piece,
            metrics,
            discoverer,
            glyphs,
        );
        if index == 0 {
            title_ink = ink;
        }
    }
    if title_ink.right() + 12.0 < summary_left {
        painter.line_segment(
            [
                Pos2::new(title_ink.right() + 6.0, rect.center().y),
                Pos2::new(summary_left - 6.0, rect.center().y),
            ],
            eframe::egui::Stroke::new(1.0, theme.divider()),
        );
    }
    rect
}

pub(crate) fn paint_chevron(painter: &Painter, rect: Rect, open: bool, ink: eframe::egui::Color32) {
    let c = rect.center();
    let points = if open {
        vec![
            c + eframe::egui::vec2(-3.0, -2.0),
            c + eframe::egui::vec2(3.0, -2.0),
            c + eframe::egui::vec2(0.0, 3.0),
        ]
    } else {
        vec![
            c + eframe::egui::vec2(-2.0, -3.0),
            c + eframe::egui::vec2(-2.0, 3.0),
            c + eframe::egui::vec2(3.0, 0.0),
        ]
    };
    painter.add(eframe::egui::Shape::convex_polygon(
        points,
        ink,
        eframe::egui::Stroke::NONE,
    ));
}

/// Contract marks are small vector symbols; their width never depends on a fallback font.
pub(crate) fn paint_mark(
    painter: &Painter,
    rect: Rect,
    mark: Mark,
    ink: eframe::egui::Color32,
    font: eframe::egui::FontId,
) {
    use eframe::egui::{Align2, Shape, Stroke};
    let c = rect.center();
    let stroke = Stroke::new(1.2, ink);
    match mark {
        Mark::Done => {
            painter.line_segment(
                [
                    c + eframe::egui::vec2(-4.0, 0.0),
                    c + eframe::egui::vec2(-1.0, 3.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + eframe::egui::vec2(-1.0, 3.0),
                    c + eframe::egui::vec2(4.0, -4.0),
                ],
                stroke,
            );
        }
        Mark::Current => {
            painter.add(Shape::convex_polygon(
                vec![
                    c + eframe::egui::vec2(-3.0, -4.0),
                    c + eframe::egui::vec2(4.0, 0.0),
                    c + eframe::egui::vec2(-3.0, 4.0),
                ],
                ink,
                Stroke::NONE,
            ));
        }
        Mark::Pending => {
            painter.circle_filled(c, 1.0, ink);
        }
        Mark::Block => {
            painter.circle_stroke(c, 4.0, stroke);
            painter.line_segment(
                [
                    c + eframe::egui::vec2(-3.0, 3.0),
                    c + eframe::egui::vec2(3.0, -3.0),
                ],
                stroke,
            );
        }
        Mark::Wait | Mark::Info => {
            painter.text(
                c,
                Align2::CENTER_CENTER,
                if mark == Mark::Wait { "?" } else { "i" },
                font,
                ink,
            );
        }
    }
}

pub(super) fn paint_row(
    painter: &Painter,
    at: Pos2,
    row: &Row,
    metrics: &CellMetrics,
    width: usize,
    discoverer: &mut Discoverer,
    glyphs: &mut Vec<ColorGlyph>,
) -> Rect {
    if let Some(inked) =
        super::identity::paint(painter, at, row, metrics, width, discoverer, glyphs)
    {
        return inked;
    }
    fold::paint_header(painter, at, row, metrics, width, discoverer, glyphs)
        .unwrap_or_else(|| color::paint_text(painter, at, row, metrics, discoverer, glyphs))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipped_board_links_keep_their_body_coordinates() {
        fn ink(text: &str, clip: Rect) -> (Pos2, usize) {
            let ctx = eframe::egui::Context::default();
            let mut hits = 0;
            let output=ctx.run_ui(eframe::egui::RawInput::default(),|ui| {
                let metrics=CellMetrics::measure(ui.ctx(),eframe::egui::FontId::monospace(14.0)).for_panel(ui.ctx());
                let rect=Rect::from_min_max(Pos2::new(100.0,20.0),Pos2::new(500.0,400.0));
                let panel=panel_contract::parse_document(&serde_json::json!({"lines":[{"text":text,"link":"https://example.test/pull/42"}]}).to_string()).unwrap();
                let body=Body::layout(&panel,&Reader::default(),(rect.width()/metrics.width())as usize,&Theme::default(),true);
                hits=body.paint(&ui.painter().with_clip_rect(clip),rect,&metrics,&Theme::default(),&mut Discoverer::default()).len();
            });
            let shapes = output.shapes.clone();
            output.drop_without_applying_deltas();
            let at = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    eframe::egui::Shape::Text(t) if t.galley.text() == "#42 ↗" => Some(t.pos),
                    _ => None,
                })
                .expect("link label painted");
            (at, hits)
        }
        let full = Rect::from_min_max(Pos2::new(100.0, 20.0), Pos2::new(500.0, 400.0));
        for text in [String::new(), "Review".to_owned(), "review ".repeat(25)] {
            let baseline = ink(&text, full).0;
            for clip in [
                Rect::from_min_max(Pos2::new(180.0, 20.0), full.max),
                Rect::from_min_max(full.min, Pos2::new(420.0, 400.0)),
            ] {
                let (at, hits) = ink(&text, clip);
                assert_eq!(at, baseline, "clipping relocated link label");
                if !clip.contains(baseline) {
                    assert_eq!(hits, 0, "clipped link became interactive");
                }
            }
        }
    }

    #[test]
    fn all_content_is_reachable_and_board_folds_are_independent() {
        let doc = panel_contract::parse_document(
            &serde_json::json!({"lines": (0..25).map(|n| format!("line-{n}")).collect::<Vec<_>>()})
                .to_string(),
        )
        .unwrap();
        let reader = Reader::default();
        let short = Body::layout(&doc, &reader, 80, &Theme::default(), false);
        assert_eq!(short.rows(), 18);
        assert_eq!(short.hidden, 7);
        let full = Body::layout(&doc, &reader, 80, &Theme::default(), true);
        assert!(full.rows.last().unwrap().text.contains("line-24"));
        let doc = panel_contract::parse_document(
            r#"{"lines":[{"text":"Block","style":"section","key":"k","collapsed":true},"secret"]}"#,
        )
        .unwrap();
        let mut board = Reader::default();
        board.apply(&Action::Fold(fold::HeaderId::Key("k".into()), true));
        assert!(
            Body::layout(&doc, &board, 80, &Theme::default(), true)
                .rows
                .iter()
                .any(|r| r.text.contains("secret"))
        );
        assert!(
            !Body::layout(&doc, &Reader::default(), 80, &Theme::default(), true)
                .rows
                .iter()
                .any(|r| r.text.contains("secret"))
        );
    }

    #[test]
    fn initial_cap_counts_source_lines_after_title_and_closed_content_not_wrapped_rows() {
        let mut lines = vec![
            serde_json::json!({"text":"Only heading", "style":"title"}),
            serde_json::json!({"text":"Closed section", "style":"section", "collapsed":true}),
            serde_json::json!("Hidden source line"),
            serde_json::json!({"text":"Open section", "style":"section"}),
        ];
        lines.extend(
            (0..18).map(|n| serde_json::json!(format!("visible-{n:02} {}", "wrapped ".repeat(6)))),
        );
        let doc = panel_contract::parse_document(&serde_json::json!({"lines":lines}).to_string())
            .unwrap();
        let body = Body::layout(&doc, &Reader::default(), 20, &Theme::default(), false);
        let text: String = body.rows.iter().map(|r| r.text.as_str()).collect();
        assert!(!text.contains("Only heading"));
        assert!(!text.contains("Hidden source line"));
        assert!(
            text.contains("visible-15"),
            "wrapping consumed the source-line allowance"
        );
        assert!(
            !text.contains("visible-16"),
            "more than eighteen displayed source lines escaped the cap"
        );
        assert_eq!(body.hidden, 2);
        assert!(
            body.rows() > 18,
            "fixture must wrap into more than eighteen screen rows"
        );
        let full = Body::layout(&doc, &Reader::default(), 20, &Theme::default(), true);
        assert!(full.rows.iter().any(|r| r.text.contains("visible-17")));
        assert_eq!(full.hidden, 0);
    }

    #[test]
    fn reference_body_keeps_separate_marks_refs_styles_indentation_and_summary_without_actions() {
        let panel = panel_contract::parse_document(include_str!(
            "../../tests/fixtures/board-reference/t2g.panel.json"
        ))
        .unwrap();
        let theme = Theme::default();
        let body = Body::layout(&panel, &Reader::default(), 100, &theme, true);
        assert_eq!(
            body.rows[0].text, "MRs",
            "first title and initial gap must be omitted"
        );
        assert_eq!(body.rows[0].header.as_ref().unwrap().summary, "2/4 merged");
        let profile = body
            .rows
            .iter()
            .position(|r| r.text.contains("profile selector"))
            .unwrap();
        let inbox = body
            .rows
            .iter()
            .position(|r| r.text.contains("inbox and search"))
            .unwrap();
        let pipeline = body
            .rows
            .iter()
            .find(|r| r.text.contains("pipeline"))
            .unwrap();
        assert_eq!(
            body.prefixes[&profile].reference.as_ref().unwrap().text,
            "#1"
        );
        assert_eq!(body.prefixes[&inbox].reference.as_ref().unwrap().text, "#3");
        assert_eq!(body.prefixes[&profile].mark.unwrap().0, Mark::Done);
        assert_eq!(body.prefixes[&inbox].mark.unwrap().0, Mark::Current);
        assert_eq!(
            body.rows[profile].color,
            theme.foreground().gamma_multiply(0.5)
        );
        assert_eq!(body.rows[inbox].color, theme.foreground());
        assert_eq!(pipeline.color, theme.foreground().gamma_multiply(0.5));
        let leading = |r: &Row| r.text.chars().take_while(|c| *c == ' ').count();
        assert!(
            leading(pipeline) >= leading(&body.rows[inbox]) + 5,
            "dim child indentation was flattened"
        );
        assert!(
            body.rows
                .iter()
                .all(|r| !r.text.contains("Aprobar") && !r.text.contains("sí, adelante")),
            "producer actions must not become reader controls"
        );
    }
}
