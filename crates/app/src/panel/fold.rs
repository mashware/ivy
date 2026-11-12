//! Reader-owned block folds; the producer supplies only the initial choice.
use std::collections::HashMap;

use super::{Line, Panel, PanelState, Row, RowRole, Style, Theme, blocks, lay_out_line, sanitize};
use crate::painter::CellMetrics;
use eframe::egui::{Pos2, Rect};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum HeaderId {
    Key(String),
    Text(String),
}

type Header = (HeaderId, bool);
pub(super) type Folds = HashMap<HeaderId, bool>;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PaintedHeader {
    area: Rect,
    header: Header,
}

pub(crate) fn toggle_header_at(state: &mut PanelState, point: Pos2) -> bool {
    let Some(header) = state.drawn.as_ref().and_then(|drawn| {
        drawn
            .headers
            .iter()
            .find(|header| header.area.contains(point))
    }) else {
        return false;
    };
    // Both identity and value belong to the pixels, even if a new document already arrived.
    let (id, folded) = &header.header;
    state.folds.insert(id.clone(), !folded);
    state.layout = None;
    true
}

pub(super) fn is_gap(line: &Line) -> bool {
    line.style != Some(Style::Section)
        && line.text.is_empty()
        && line.reference.is_none()
        && line.mark.is_none()
}

pub(super) fn body(rows: &mut Vec<Row>, panel: &Panel, folds: &Folds, width: usize, theme: &Theme) {
    for block in blocks(&panel.lines) {
        let column = block
            .iter()
            .filter(|line| line.style != Some(Style::Section))
            .filter_map(|line| line.reference.as_deref())
            .map(|reference| sanitize(reference).chars().count())
            .max()
            .unwrap_or(0)
            .min(width.saturating_sub(super::MINIMUM_SLOT).saturating_sub(3));
        let mut hidden = false;
        for line in block {
            if line.style == Some(Style::Section) {
                let id = match &line.key {
                    Some(key) => HeaderId::Key(key.clone()),
                    None => HeaderId::Text(line.text.clone()),
                };
                hidden = folds.get(&id).copied().unwrap_or(line.collapsed);
                let mut row = Row::text(
                    0,
                    header_text(line, hidden, width),
                    theme.panel_color(None, Some(Style::Section)),
                );
                row.role = RowRole::Fold(rows.len());
                row.header = Some((id, hidden));
                rows.push(row);
            } else if !hidden || is_gap(line) {
                rows.extend(lay_out_line(line, column, width, theme));
            }
        }
    }
}

fn header_text(line: &Line, folded: bool, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let indicator = if folded { '▸' } else { '▾' };
    let summary = sanitize(line.summary.as_deref().unwrap_or(""));
    let summary: String = summary.chars().take(width.saturating_sub(2)).collect();
    let remaining = width.saturating_sub(2 + summary.chars().count());
    let title: String = sanitize(&line.text)
        .chars()
        .take(remaining.saturating_sub(3))
        .collect();
    let rule = "─".repeat(remaining.saturating_sub(title.chars().count() + 2));
    let text = if title.is_empty() {
        format!("{indicator} {}{summary}", "─".repeat(remaining))
    } else {
        format!("{indicator} {title} {rule} {summary}")
    };
    text.chars().take(width).collect()
}

pub(super) fn record_header(
    headers: &mut Vec<PaintedHeader>,
    row: &Row,
    index: usize,
    rect: Rect,
    metrics: &CellMetrics,
    overflowed: bool,
) {
    if !row.text.is_empty()
        && rect.width() > 0.0
        && !overflowed
        && let Some(header) = &row.header
    {
        let top = rect.top() + index as f32 * metrics.height();
        headers.push(PaintedHeader {
            area: Rect::from_min_max(
                Pos2::new(rect.left(), top),
                Pos2::new(rect.right(), top + metrics.height()),
            ),
            header: header.clone(),
        });
    }
}

#[cfg(test)]
pub(super) fn lay_out(
    panel: Option<&Panel>,
    id: &str,
    width: usize,
    theme: &Theme,
    warning: Option<&str>,
    attention: Option<super::AttentionState>,
    held_by: Option<&str>,
) -> Vec<Row> {
    let mut rows = super::lay_out_head(panel, id, width, theme, warning, attention, held_by);
    if let Some(panel) = panel {
        body(&mut rows, panel, &Folds::default(), width, theme);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::tests::paint_the_state;
    use eframe::egui::{Shape, vec2};
    use std::time::SystemTime;

    fn state(lines: serde_json::Value) -> PanelState {
        let mut state = PanelState::default();
        publish(&mut state, lines);
        state
    }

    fn publish(state: &mut PanelState, lines: serde_json::Value) {
        state
            .deliver(
                1,
                "flow",
                panel_contract::parse_document(
                    &serde_json::json!({"header":false,"lines":lines}).to_string(),
                )
                .unwrap(),
                SystemTime::now(),
            )
            .unwrap();
    }

    fn paint(state: &mut PanelState, width: f32, height: f32) -> Vec<String> {
        fn collect(shape: Shape, texts: &mut Vec<String>) {
            match shape {
                Shape::Text(text) => texts.push(text.galley.job.text.clone()),
                Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, texts);
                    }
                }
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for shape in paint_the_state(
            state,
            Rect::from_min_size(Pos2::ZERO, vec2(width, height)),
            SystemTime::now(),
        ) {
            collect(shape, &mut texts);
        }
        texts
    }

    fn header() -> serde_json::Value {
        serde_json::json!({"text":"MRs","style":"section","summary":"2/4 merged","collapsed":true,"key":"mrs"})
    }

    fn click(state: &mut PanelState) {
        let area = state.drawn.as_ref().unwrap().headers[0].area;
        assert!(toggle_header_at(state, area.center()));
    }

    #[test]
    fn folded_body_is_absent_and_gaps_and_other_blocks_survive() {
        let mut state = state(serde_json::json!([
            "title",
            "",
            header(),
            "#1",
            "#2",
            "",
            "Now"
        ]));
        let text = paint(&mut state, 420.0, 600.0).join("\n");
        assert!(
            text.contains("title")
                && text.contains("MRs")
                && text.contains("2/4 merged")
                && text.contains("Now"),
            "{text}"
        );
        assert!(!text.contains("#1") && !text.contains("#2"), "{text}");
        let rows = &state.layout.as_ref().unwrap().rows;
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[1].text, "");
        assert_eq!(rows[3].text, "");
        click(&mut state);
        let text = paint(&mut state, 420.0, 600.0).join("\n");
        assert!(text.contains("#1") && text.contains("#2"));
        click(&mut state);
        assert!(!paint(&mut state, 420.0, 600.0).join("\n").contains("#1"));
    }

    #[test]
    fn choice_survives_republish_and_is_per_console() {
        let lines = serde_json::json!([header(), "#1"]);
        let mut first = state(lines.clone());
        let mut second = state(lines.clone());
        paint(&mut first, 420.0, 600.0);
        click(&mut first);
        publish(&mut first, lines);
        assert!(paint(&mut first, 420.0, 600.0).join("\n").contains("#1"));
        let published: serde_json::Value =
            serde_json::from_str(&first.document().unwrap()).unwrap();
        assert_eq!(published["lines"][0]["style"], "section");
        assert_eq!(published["lines"][0]["summary"], "2/4 merged");
        assert_eq!(published["lines"][0]["collapsed"], true);
        assert!(!paint(&mut second, 420.0, 600.0).join("\n").contains("#1"));
        click(&mut first);
        assert!(!paint(&mut first, 420.0, 600.0).join("\n").contains("#1"));
    }

    #[test]
    fn keyless_identity_uses_text_and_a_key_is_distinct_from_text() {
        let mut h = header();
        h.as_object_mut().unwrap().remove("key");
        let mut state = state(serde_json::json!([h, "body"]));
        paint(&mut state, 420.0, 600.0);
        click(&mut state);
        publish(&mut state, serde_json::json!([h, "body"]));
        assert!(paint(&mut state, 420.0, 600.0).join("\n").contains("body"));
        h["text"] = "renamed".into();
        publish(&mut state, serde_json::json!([h, "body"]));
        assert!(!paint(&mut state, 420.0, 600.0).join("\n").contains("body"));
        h["key"] = "MRs".into();
        publish(&mut state, serde_json::json!([h, "body"]));
        assert!(!paint(&mut state, 420.0, 600.0).join("\n").contains("body"));
    }

    #[test]
    fn click_uses_painted_identity_and_value_after_document_replacement() {
        let mut state = state(serde_json::json!([header(), "old body"]));
        paint(&mut state, 420.0, 600.0);
        let mut replacement = header();
        replacement["key"] = "other".into();
        replacement["collapsed"] = false.into();
        publish(&mut state, serde_json::json!([replacement, "new body"]));
        click(&mut state);
        assert!(
            paint(&mut state, 420.0, 600.0)
                .join("\n")
                .contains("new body")
        );
        assert_eq!(state.folds.get(&HeaderId::Key("mrs".into())), Some(&false));
        assert!(!state.folds.contains_key(&HeaderId::Key("other".into())));
        publish(&mut state, serde_json::json!([header(), "old body"]));
        assert!(
            paint(&mut state, 420.0, 600.0)
                .join("\n")
                .contains("old body")
        );
    }

    #[test]
    fn clipped_and_unpainted_headers_are_not_clickable() {
        let mut state = state(serde_json::json!(["title", header(), "body"]));
        paint(&mut state, 420.0, 600.0);
        let area = state.drawn.as_ref().unwrap().headers[0].area;
        paint(&mut state, 420.0, area.top() + area.height() * 0.5);
        assert!(state.drawn.as_ref().unwrap().headers.is_empty());
        assert!(!toggle_header_at(&mut state, area.center()));
        paint(&mut state, 420.0, 600.0);
        assert!(!toggle_header_at(&mut state, Pos2::new(10.0, 2.0)));
        state.nothing_was_drawn();
        assert!(!toggle_header_at(&mut state, area.center()));
        paint(&mut state, 0.0, 600.0);
        assert!(state.drawn.as_ref().unwrap().headers.is_empty());
    }

    #[test]
    fn header_ignores_line_adornments_and_nonheaders_ignore_fold_fields() {
        let mut h = header();
        h["collapsed"] = false.into();
        let mut plain = state(serde_json::json!([h,{"text":"body","ref":"#1"}]));
        let expected = paint(&mut plain, 420.0, 600.0);
        h["mark"] = "block".into();
        h["ref"] = "a very long reference".into();
        h["link"] = "https://example.test".into();
        h["indent"] = 60.into();
        let mut adorned = state(
            serde_json::json!([h,{"text":"body","ref":"#1","summary":"ignored","collapsed":true}]),
        );
        assert_eq!(paint(&mut adorned, 420.0, 600.0), expected);
        assert!(adorned.drawn.as_ref().unwrap().links.is_empty());
    }

    #[test]
    fn a_header_ends_the_previous_fold_even_without_a_gap() {
        let mut state = state(
            serde_json::json!(["before",header(),"hidden",{"text":"Next","style":"section"},"visible"]),
        );
        let text = paint(&mut state, 420.0, 600.0).join("\n");
        assert!(text.contains("before") && text.contains("visible"));
        assert!(!text.contains("hidden"));
        assert_eq!(state.drawn.as_ref().unwrap().headers.len(), 2);
    }

    #[test]
    fn summary_keeps_the_right_edge_and_outlasts_the_title() {
        for width in [420.0, 180.0, 135.0] {
            let mut state = state(serde_json::json!([header()]));
            let text = paint(&mut state, width, 600.0).join("\n");
            assert!(text.ends_with("2/4 merged"), "{width}: {text}");
            let layout = state.layout.as_ref().unwrap();
            assert_eq!(layout.rows[0].text.chars().count(), layout.width);
            assert_eq!(state.drawn.as_ref().unwrap().headers.len(), 1);
        }
    }

    #[test]
    fn fold_glyphs_fit_one_cell_in_the_builtin_font() {
        let ctx = eframe::egui::Context::default();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let font = eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE);
            ui.ctx().fonts_mut(|fonts| {
                let cell = fonts.glyph_width(&font, 'M');
                for glyph in ['▸', '▾', '─'] {
                    let width = fonts.glyph_width(&font, glyph);
                    assert!(width > 0.0 && width <= cell, "{glyph}: {width} > {cell}");
                }
            });
        });
        output.drop_without_applying_deltas();
    }
}
