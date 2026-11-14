//! Pure body rendering shared by independently interactive panel surfaces.
use super::{Row, color, fold, links};
use crate::{
    fonts::Discoverer,
    painter::{CellMetrics, ColorGlyph},
    theme::Theme,
};
use eframe::egui::{Painter, Pos2, Rect};
use panel_contract::{Panel, Style};

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
pub(crate) struct Body {
    rows: Vec<Row>,
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
        let mut rows = Vec::new();
        fold::append_blocks(&mut rows, &document, &reader.folds, width, theme);
        color::flag(&mut rows);
        Self { rows, hidden }
    }
    pub(crate) fn height(&self, metrics: &CellMetrics) -> f32 {
        self.rows.len() as f32 * metrics.height()
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
        let width = (rect.width() / metrics.width()) as usize;
        let mut hits = Vec::new();
        let mut glyphs = Vec::new();
        for (n, row) in self.rows.iter().enumerate() {
            let y = rect.top() + n as f32 * metrics.height();
            let row_rect = Rect::from_min_size(
                Pos2::new(rect.left(), y),
                eframe::egui::vec2(rect.width(), metrics.height()),
            );
            if !row_rect.intersects(painter.clip_rect()) {
                continue;
            }
            let at = Pos2::new(rect.left() + row.column as f32 * metrics.width(), y);
            discoverer.note_text(&row.text);
            let ink = paint_row(painter, at, row, metrics, width, discoverer, &mut glyphs);
            if let Some(header) = &row.header
                && painter.clip_rect().contains_rect(row_rect)
                && ink.right() <= rect.right()
            {
                hits.push((row_rect, Action::Fold(header.id.clone(), header.folded)));
            }
            if let Some(link) = &row.link {
                let painted = links::paint_label(painter, link, y, metrics, theme, discoverer);
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

pub(super) fn paint_row(
    painter: &Painter,
    at: Pos2,
    row: &Row,
    metrics: &CellMetrics,
    width: usize,
    discoverer: &mut Discoverer,
    glyphs: &mut Vec<ColorGlyph>,
) -> Rect {
    fold::paint_header(painter, at, row, metrics, width, discoverer, glyphs)
        .unwrap_or_else(|| color::paint_text(painter, at, row, metrics, discoverer, glyphs))
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
