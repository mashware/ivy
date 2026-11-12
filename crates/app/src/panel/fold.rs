//! Reader-owned block folds; the producer supplies only the initial choice.
use std::collections::HashMap;

use super::{Line, Panel, PanelState, Row, RowRole, Style, Theme, blocks, lay_out_line, sanitize};
use crate::painter::CellMetrics;
use eframe::egui::{Painter, Pos2, Rect};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum HeaderId {
    Key(String),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Header {
    id: HeaderId,
    folded: bool,
    summary: String,
}
pub(super) type Folds = HashMap<HeaderId, bool>;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PaintedHeader {
    area: Rect,
    id: HeaderId,
    folded: bool,
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
    state.folds.insert(header.id.clone(), !header.folded);
    state.layout = None;
    true
}

pub(super) fn is_gap(line: &Line) -> bool {
    line.style != Some(Style::Section)
        && line.text.is_empty()
        && line.reference.is_none()
        && line.mark.is_none()
}

pub(super) fn append_blocks(
    rows: &mut Vec<Row>,
    panel: &Panel,
    folds: &Folds,
    width: usize,
    theme: &Theme,
) {
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
                    sanitize(&line.text),
                    theme.panel_color(None, Some(Style::Section)),
                );
                row.role = RowRole::Fold(rows.len());
                row.header = Some(Header {
                    id,
                    folded: hidden,
                    summary: sanitize(line.summary.as_deref().unwrap_or("")),
                });
                rows.push(row);
            } else if !hidden || is_gap(line) {
                rows.extend(lay_out_line(line, column, width, theme));
            }
        }
    }
}

pub(super) fn paint_header(
    painter: &Painter,
    origin: Pos2,
    row: &Row,
    metrics: &CellMetrics,
    width: usize,
    discoverer: &mut crate::fonts::Discoverer,
    glyphs: &mut Vec<crate::painter::ColorGlyph>,
) -> Option<Rect> {
    let header = row.header.as_ref()?;
    let measure = |text: &str| {
        painter
            .layout_no_wrap(text.to_owned(), metrics.font(), row.color)
            .size()
            .x
    };
    let width = width as f32 * metrics.width();
    let prefix = if header.folded { "▸ " } else { "▾ " };
    let prefix = fit(prefix, width, &measure);
    let summary = fit(
        &header.summary,
        (width - measure(prefix)).max(0.0),
        &measure,
    );
    let left_width = (width - measure(summary)).max(0.0);
    let title_room = (left_width - measure(prefix) - measure(" ─ ")).max(0.0);
    let title = fit(&row.text, title_room, &measure);
    let mut head = format!("{prefix}{title}");
    let rule_width = (left_width - measure(&head) - measure("  ")).max(0.0);
    let rule = "─".repeat((rule_width / measure("─")) as usize);
    head.push_str(&format!(" {rule}"));
    let head = fit(&head, left_width, &measure).to_owned();
    let mut pieces = [
        Row::text(0, head, row.color),
        Row::text(0, summary.to_owned(), row.color),
    ];
    super::color::flag(&mut pieces);
    let mut inked = Rect::NOTHING;
    for (piece, x) in pieces
        .iter()
        .zip([origin.x, origin.x + width - measure(summary)])
    {
        discoverer.note_text(&piece.text);
        inked = inked.union(super::color::paint_text(
            painter,
            Pos2::new(x, origin.y),
            piece,
            metrics,
            discoverer,
            glyphs,
        ));
    }
    Some(inked)
}

fn fit<'a>(text: &'a str, width: f32, measure: &impl Fn(&str) -> f32) -> &'a str {
    let boundaries: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(at, _)| at)
        .chain(std::iter::once(text.len()))
        .collect();
    let mut low = 0;
    let mut high = boundaries.len();
    while low + 1 < high {
        let mid = (low + high) / 2;
        if measure(&text[..boundaries[mid]]) <= width {
            low = mid;
        } else {
            high = mid;
        }
    }
    &text[..boundaries[low]]
}

pub(super) fn record_header(
    headers: &mut Vec<PaintedHeader>,
    row: &Row,
    index: usize,
    rect: Rect,
    metrics: &CellMetrics,
    overflowed: bool,
) {
    if (rect.width() / metrics.width()) as usize > 2 * super::MARGIN as usize
        && !overflowed
        && let Some(header) = &row.header
    {
        let top = rect.top() + index as f32 * metrics.height();
        headers.push(PaintedHeader {
            area: Rect::from_min_max(
                Pos2::new(rect.left(), top),
                Pos2::new(rect.right(), top + metrics.height()),
            ),
            id: header.id.clone(),
            folded: header.folded,
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
        append_blocks(&mut rows, panel, &Folds::default(), width, theme);
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
    fn header_beneath_the_more_marker_is_not_clickable() {
        let mut state = state(serde_json::json!(["title", header(), "body"]));
        paint(&mut state, 420.0, 600.0);
        let previous_area = state.drawn.as_ref().unwrap().headers[0].area;

        // One row fits: the title is visible and the header is counted under the overflow marker.
        let text = paint(&mut state, 420.0, previous_area.height() * 1.5).join("\n");
        assert!(text.contains("… 2 more"), "{text}");
        assert!(state.drawn.as_ref().unwrap().headers.is_empty());
        assert!(!toggle_header_at(&mut state, previous_area.center()));
    }

    #[test]
    fn blank_keyless_header_remembers_its_choice_and_nonheaders_stay_ordinary() {
        let blank = serde_json::json!({
            "text":"", "style":"section", "collapsed":true
        });
        let mut state = state(serde_json::json!([
            blank,
            {"text":"plain body", "summary":"ignored", "collapsed":true}
        ]));
        let first = paint(&mut state, 420.0, 600.0).join("\n");
        assert!(!first.contains("plain body"), "{first}");

        click(&mut state);
        let opened = paint(&mut state, 420.0, 600.0).join("\n");
        assert!(opened.contains("plain body"), "{opened}");
        assert_eq!(
            state.folds.get(&HeaderId::Text(String::new())),
            Some(&false)
        );

        publish(
            &mut state,
            serde_json::json!([
                blank,
                {"text":"plain body", "summary":"ignored", "collapsed":true}
            ]),
        );
        let republished = paint(&mut state, 420.0, 600.0).join("\n");
        assert!(republished.contains("plain body"), "{republished}");
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
            assert_eq!(layout.rows[0].text, "MRs");
            assert_eq!(state.drawn.as_ref().unwrap().headers.len(), 1);
        }
    }

    #[test]
    fn measured_summary_stays_whole_and_right_aligned_with_wide_text() {
        let mut h = header();
        h["text"] = "日本語の長い見出し".into();
        h["summary"] = "日本語".into();
        let mut state = state(serde_json::json!([h, "body"]));
        let place = Rect::from_min_size(Pos2::ZERO, vec2(180.0, 600.0));
        let shapes = paint_the_state(&mut state, place, SystemTime::now());
        fn text_shapes(shape: &Shape, found: &mut Vec<(String, Rect)>) {
            match shape {
                Shape::Text(text) => found.push((
                    text.galley.job.text.clone(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                )),
                Shape::Vec(shapes) => {
                    for shape in shapes {
                        text_shapes(shape, found);
                    }
                }
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for shape in &shapes {
            text_shapes(shape, &mut texts);
        }
        let summary = texts
            .iter()
            .find(|(text, _)| text == "日本語")
            .expect("whole summary painted");
        let title_and_rule = texts
            .iter()
            .find(|(text, _)| text.starts_with('▸') || text.starts_with('▾'))
            .expect("header title and rule painted");
        assert!(
            title_and_rule.1.right() <= summary.1.left() + 0.1,
            "header ink {:?} overlaps summary ink {:?}",
            title_and_rule.1,
            summary.1
        );
        let ctx = eframe::egui::Context::default();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrics = CellMetrics::measure(
                ui.ctx(),
                eframe::egui::FontId::monospace(crate::DEFAULT_FONT_SIZE),
            )
            .for_panel(ui.ctx());
            let width = ((place.width() / metrics.width()) as usize).saturating_sub(2);
            let right = metrics.width() * (1 + width) as f32;
            assert!(
                (summary.1.right() - right).abs() < 0.1,
                "{:?} vs {right}",
                summary.1
            );
        });
        output.drop_without_applying_deltas();
        assert_eq!(state.drawn.as_ref().unwrap().headers.len(), 1);
        click(&mut state);
        assert!(paint(&mut state, 180.0, 600.0).join("\n").contains("body"));
    }

    #[test]
    fn truncation_keeps_combining_sequences_together() {
        let text = "a\u{301}b";
        let measured = |text: &str| text.chars().count() as f32;
        assert_eq!(fit(text, 1.0, &measured), "");
        assert_eq!(fit(text, 2.0, &measured), "a\u{301}");
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
