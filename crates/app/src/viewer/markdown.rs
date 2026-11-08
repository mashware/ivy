//! A deliberately inert, selectable Markdown rendering.
use eframe::egui::{self, FontId, TextFormat, text::LayoutJob};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::{cell::RefCell, sync::Arc};

// A single text layout must not monopolize the window thread.
const MAX_BLOCK_BYTES: usize = 16 * 1024;

#[derive(Clone, Default, Debug)]
struct Style {
    strong: bool,
    emphasis: bool,
    code: bool,
}

#[derive(Clone, Debug)]
struct Run {
    text: String,
    style: Style,
}

#[derive(Debug)]
enum Block {
    Line {
        runs: Vec<Run>,
        heading: Option<u8>,
        indent: usize,
        code: bool,
    },
    Table {
        cells: Vec<Vec<Run>>,
        widths: Arc<Vec<usize>>,
    },
}

#[derive(Debug, Default)]
pub(super) struct MarkdownDocument {
    blocks: Vec<Block>,
    heights: RefCell<(f32, Vec<f32>, Vec<f32>)>,
}

impl MarkdownDocument {
    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let mut doc = Self::default();
        let mut runs = Vec::new();
        let mut style = Style::default();
        let mut heading = None;
        let mut code_block = false;
        let mut lists: Vec<Option<u64>> = Vec::new();
        let mut table: Option<Vec<Vec<Vec<Run>>>> = None;
        let mut row: Vec<Vec<Run>> = Vec::new();
        let mut item_depth = 0usize;
        let mut indent = 0;
        let flush = |doc: &mut Self, runs: &mut Vec<Run>, heading, indent, code| {
            if !runs.is_empty() {
                doc.blocks.push(Block::Line {
                    runs: std::mem::take(runs),
                    heading,
                    indent,
                    code,
                });
            }
        };
        for event in Parser::new_ext(text, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH) {
            match event {
                Event::Start(tag) => match tag {
                    Tag::Heading { level, .. } => heading = Some(heading_number(level)),
                    Tag::CodeBlock(_) => code_block = true,
                    Tag::HtmlBlock => flush(&mut doc, &mut runs, heading, indent, false),
                    Tag::List(start) => {
                        flush(&mut doc, &mut runs, heading, indent, code_block);
                        lists.push(start);
                    }
                    Tag::Item => {
                        flush(&mut doc, &mut runs, heading, indent, code_block);
                        indent = lists.len().saturating_sub(1);
                        let prefix = match lists.last_mut() {
                            Some(Some(n)) => {
                                let s = format!("{n}. ");
                                *n += 1;
                                s
                            }
                            _ => "• ".to_owned(),
                        };
                        runs.push(Run {
                            text: prefix,
                            style: Style::default(),
                        });
                        item_depth += 1;
                    }
                    Tag::Emphasis => style.emphasis = true,
                    Tag::Strong => style.strong = true,
                    Tag::Table(_) => table = Some(Vec::new()),
                    Tag::TableRow | Tag::TableHead => row.clear(),
                    Tag::TableCell => {
                        runs.clear();
                    }
                    _ => {}
                },
                Event::End(tag) => match tag {
                    TagEnd::Heading(_) => {
                        flush(&mut doc, &mut runs, heading, indent, false);
                        heading = None;
                    }
                    TagEnd::Paragraph if item_depth == 0 => {
                        flush(&mut doc, &mut runs, heading, indent, false)
                    }
                    TagEnd::Paragraph => {}
                    TagEnd::HtmlBlock => flush(&mut doc, &mut runs, None, indent, false),
                    TagEnd::CodeBlock => {
                        flush(&mut doc, &mut runs, None, indent, true);
                        code_block = false;
                    }
                    TagEnd::Item => {
                        flush(&mut doc, &mut runs, heading, indent, false);
                        item_depth = item_depth.saturating_sub(1);
                    }
                    TagEnd::List(_) => {
                        lists.pop();
                        indent = lists.len().saturating_sub(1);
                    }
                    TagEnd::Emphasis => style.emphasis = false,
                    TagEnd::Strong => style.strong = false,
                    TagEnd::TableCell => {
                        row.push(std::mem::take(&mut runs));
                    }
                    TagEnd::TableRow | TagEnd::TableHead => {
                        if let Some(t) = table.as_mut() {
                            t.push(std::mem::take(&mut row));
                        }
                    }
                    TagEnd::Table => {
                        if let Some(t) = table.take() {
                            let mut widths = Vec::new();
                            for row in &t {
                                for (i, cell) in row.iter().enumerate() {
                                    if widths.len() <= i {
                                        widths.resize(i + 1, 0usize);
                                    }
                                    widths[i] = widths[i]
                                        .max(cell.iter().map(|run| run.text.chars().count()).sum());
                                }
                            }
                            let widths = Arc::new(widths);
                            for cells in t {
                                doc.blocks.push(Block::Table {
                                    cells,
                                    widths: Arc::clone(&widths),
                                });
                            }
                        }
                    }
                    _ => {}
                },
                Event::Text(text) if code_block => {
                    for line in text.split_inclusive('\n') {
                        runs.push(Run {
                            text: line.trim_end_matches('\n').to_owned(),
                            style: style.clone(),
                        });
                        flush(&mut doc, &mut runs, None, indent, true);
                    }
                }
                Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => runs.push(Run {
                    text: text.into_string(),
                    style: style.clone(),
                }),
                Event::Code(text) => {
                    let mut s = style.clone();
                    s.code = true;
                    runs.push(Run {
                        text: text.into_string(),
                        style: s,
                    });
                }
                Event::SoftBreak | Event::HardBreak => runs.push(Run {
                    text: "\n".into(),
                    style: style.clone(),
                }),
                Event::Rule => runs.push(Run {
                    text: "────────".into(),
                    style: style.clone(),
                }),
                _ => {}
            }
        }
        flush(&mut doc, &mut runs, heading, indent, code_block);
        for block in &doc.blocks {
            let bytes: usize = match block {
                Block::Line { runs, .. } => runs.iter().map(|run| run.text.len()).sum(),
                Block::Table { cells, .. } => {
                    cells.iter().flatten().map(|run| run.text.len()).sum()
                }
            };
            if bytes > MAX_BLOCK_BYTES {
                return Err("Markdown block exceeds the 16 KiB layout limit".into());
            }
        }
        Ok(doc)
    }

    pub(super) fn show(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .id_salt("markdown-content")
            .auto_shrink([false; 2])
            .show_viewport(ui, |ui, viewport| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let width = ui.available_width().max(1.0);
                let mut cache = self.heights.borrow_mut();
                if (cache.0 - width).abs() > 1.0 || cache.1.len() != self.blocks.len() {
                    cache.0 = width;
                    cache.2.clear();
                    cache.1 = self
                        .blocks
                        .iter()
                        .map(|block| estimate_height(block, width))
                        .collect();
                }
                if cache.2.len() != cache.1.len() + 1 {
                    rebuild_offsets(&mut cache);
                }
                let first = cache
                    .2
                    .partition_point(|y| *y < viewport.min.y)
                    .saturating_sub(1)
                    .min(self.blocks.len());
                let end =
                    (cache.2.partition_point(|y| *y <= viewport.max.y) + 1).min(self.blocks.len());
                ui.add_space(cache.2[first]);
                let mut changed = false;
                for index in first..end {
                    let is_code = |index: usize| {
                        matches!(self.blocks.get(index), Some(Block::Line { code: true, .. }))
                    };
                    let first_code = is_code(index) && (index == 0 || !is_code(index - 1));
                    let last_code = is_code(index) && !is_code(index + 1);
                    let response =
                        ui.scope(|ui| render_block(ui, &self.blocks[index], first_code, last_code));
                    let height =
                        response.response.rect.height().max(1.0) + ui.spacing().item_spacing.y;
                    if (cache.1[index] - height).abs() > 0.01 {
                        cache.1[index] = height;
                        changed = true;
                    }
                }
                ui.add_space((cache.2[self.blocks.len()] - cache.2[end]).max(0.0));
                if changed {
                    rebuild_offsets(&mut cache);
                }
            });
    }
}

fn rebuild_offsets(cache: &mut (f32, Vec<f32>, Vec<f32>)) {
    cache.2.clear();
    cache.2.reserve(cache.1.len() + 1);
    cache.2.push(0.0);
    for height in &cache.1 {
        cache
            .2
            .push(cache.2.last().copied().unwrap_or(0.0) + height);
    }
}

fn estimate_height(block: &Block, width: f32) -> f32 {
    match block {
        Block::Line {
            runs,
            heading,
            code,
            indent,
        } => {
            let size = text_size(*heading, *code);
            let available = (width - *indent as f32 * 18.0).max(1.0);
            let text: String = runs.iter().map(|r| r.text.as_str()).collect();
            let lines: usize = text
                .split('\n')
                .map(|line| {
                    if *code {
                        1
                    } else {
                        ((line.chars().count() as f32 * size * 0.6 / available).ceil() as usize)
                            .max(1)
                    }
                })
                .sum();
            lines as f32 * (size * 1.4 + 1.0) + block_spacing(*heading, *code, runs) + 4.0
        }
        Block::Table { .. } => 28.0,
    }
}

fn render_block(ui: &mut egui::Ui, block: &Block, first_code: bool, last_code: bool) {
    #[cfg(test)]
    tests::RENDERED_BLOCKS.with(|count| count.set(count.get() + 1));
    match block {
        Block::Line {
            runs,
            heading,
            indent,
            code,
        } => {
            let spacing = block_spacing(*heading, *code, runs);
            if heading.is_some_and(|level| level > 1) {
                ui.add_space(12.0);
            }
            if *code {
                egui::Frame::new()
                    .fill(ui.visuals().faint_bg_color)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: if first_code { 6 } else { 0 },
                        bottom: if last_code { 6 } else { 0 },
                    })
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.add(
                            egui::Label::new(layout(ui, runs, *heading, true))
                                .wrap_mode(egui::TextWrapMode::Extend)
                                .selectable(true),
                        );
                    });
            } else {
                ui.horizontal(|ui| {
                    ui.add_space(*indent as f32 * 20.0);
                    ui.add(egui::Label::new(layout(ui, runs, *heading, false)).selectable(true));
                });
            }
            ui.add_space(if *code && last_code {
                12.0
            } else if heading.is_some_and(|level| level > 1) {
                (spacing - 12.0).max(0.0)
            } else {
                spacing
            });
        }
        Block::Table { cells, widths } => {
            ui.horizontal(|ui| {
                for (index, cell) in cells.iter().enumerate() {
                    let width = widths.get(index).copied().unwrap_or(1) as f32 * 14.0 + 12.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, 20.0),
                        egui::Layout::left_to_right(egui::Align::Min),
                        |ui| {
                            ui.set_min_width(width);
                            ui.add(
                                egui::Label::new(layout(ui, cell, None, false))
                                    .wrap_mode(egui::TextWrapMode::Extend)
                                    .selectable(true),
                            );
                        },
                    );
                }
            });
        }
    }
}

fn text_size(heading: Option<u8>, code: bool) -> f32 {
    if code {
        12.0
    } else {
        match heading {
            Some(1) => 25.0,
            Some(_) => 17.0,
            None => 14.0,
        }
    }
}

fn block_spacing(heading: Option<u8>, code: bool, runs: &[Run]) -> f32 {
    if code {
        0.0
    } else if heading == Some(1) {
        12.0
    } else if heading.is_some() {
        19.0
    } else if runs.first().is_some_and(|run| {
        run.text == "• "
            || run.text.ends_with(". ")
                && run.text.chars().next().is_some_and(|c| c.is_ascii_digit())
    }) {
        2.0
    } else {
        12.0
    }
}

fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn layout(ui: &egui::Ui, runs: &[Run], heading: Option<u8>, code_block: bool) -> LayoutJob {
    let mut job = LayoutJob::default();
    for run in runs {
        let size = text_size(heading, code_block || run.style.code);
        let font_id = if code_block || run.style.code {
            FontId::monospace(size)
        } else {
            FontId::proportional(size)
        };
        let mut format = TextFormat {
            font_id,
            color: ui.visuals().text_color(),
            italics: run.style.emphasis,
            ..Default::default()
        };
        if run.style.strong && heading.is_none() {
            format.font_id.size += 1.0;
        }
        if run.style.code && !code_block {
            format.background = ui.visuals().faint_bg_color;
        }
        job.append(&run.text, 0.0, format);
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::epaint::Shape;
    std::thread_local! {
        pub(super) static RENDERED_BLOCKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    fn collect<'a>(shape: &'a Shape, jobs: &mut Vec<&'a LayoutJob>) {
        match shape {
            Shape::Text(text) => jobs.push(&text.galley.job),
            Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, jobs);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn displays_formatted_markdown_without_opening_links() {
        let doc = MarkdownDocument::parse(
            "# Title\n\nplain *em* **bold** `inline`\n\n1. first\n   - nested\n2. second\n\n| A | B |\n|---|---|\n| x | y |\n\n```rs\nlet x = 1;\n```\n\n[link](https://example.com) <script>bad</script>",
        ).unwrap();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| doc.show(ui));
        let mut jobs = Vec::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut jobs);
        }
        let visible = jobs
            .iter()
            .map(|j| j.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "Title",
            "plain",
            "em",
            "bold",
            "inline",
            "1. first",
            "• nested",
            "2. second",
            "A",
            "B",
            "x",
            "y",
            "let x = 1;",
            "link",
            "<script>",
        ] {
            assert!(visible.contains(expected), "missing {expected}: {visible}");
        }
        let title = jobs.iter().find(|j| j.text == "Title").unwrap();
        let plain = jobs.iter().find(|j| j.text.contains("plain")).unwrap();
        assert!(title.sections[0].format.font_id.size > plain.sections[0].format.font_id.size);
        assert!(plain.sections.iter().any(|s| s.format.italics));
        assert!(
            plain
                .sections
                .iter()
                .any(|s| s.format.font_id.size > plain.sections[0].format.font_id.size)
        );
        assert!(
            plain
                .sections
                .iter()
                .any(|s| s.format.font_id.family == egui::FontFamily::Monospace)
        );
        let code = jobs.iter().find(|j| j.text.contains("let x = 1;")).unwrap();
        assert_eq!(
            code.sections[0].format.font_id.family,
            egui::FontFamily::Monospace
        );
        assert!(output.platform_output.commands.is_empty());
        output.textures_delta.clear();
    }
    #[test]
    fn virtualizes_large_document() {
        RENDERED_BLOCKS.with(|count| count.set(0));
        let doc = MarkdownDocument::parse(
            &(0..10_000)
                .map(|i| format!("Paragraph {i}\n\n"))
                .collect::<String>(),
        )
        .unwrap();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_max_height(200.0);
            doc.show(ui);
        });
        let mut jobs = Vec::new();
        for shape in &output.shapes {
            collect(&shape.shape, &mut jobs);
        }
        output.textures_delta.clear();
        RENDERED_BLOCKS.with(|count| assert!(count.get() < 100, "{} blocks laid out", count.get()));
        assert!(jobs.len() < 100, "{} text layouts", jobs.len());
        assert!(jobs.iter().any(|job| job.text.contains("Paragraph 0")));
        output.textures_delta.clear();
    }

    #[test]
    fn html_is_separate_from_heading_and_table() {
        let doc = MarkdownDocument::parse(
            "<div>raw</div>\n\n# Heading\n\n<section>other</section>\n\n| A | B |\n|---|---|\n| x | y |\n",
        ).unwrap();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| doc.show(ui));
        let mut jobs = Vec::new();
        for shape in &output.shapes {
            collect(&shape.shape, &mut jobs);
        }
        let html = jobs
            .iter()
            .find(|j| j.text.contains("<div>raw</div>"))
            .unwrap();
        let heading = jobs.iter().find(|j| j.text == "Heading").unwrap();
        assert!(html.sections[0].format.font_id.size < heading.sections[0].format.font_id.size);
        assert!(
            jobs.iter()
                .any(|j| j.text.contains("<section>other</section>"))
        );
        assert!(jobs.iter().any(|j| j.text == "A"));
        output.textures_delta.clear();
    }
    fn positions(shape: &Shape, result: &mut Vec<(String, f32)>) {
        match shape {
            Shape::Text(text) => result.push((text.galley.job.text.clone(), text.pos.x)),
            Shape::Vec(shapes) => {
                for shape in shapes {
                    positions(shape, result);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn table_columns_align_across_virtual_rows() {
        let doc = MarkdownDocument::parse("| very long heading | B |\n|---|---|\n| x | second |\n")
            .unwrap();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| doc.show(ui));
        let mut found = Vec::new();
        for shape in &output.shapes {
            positions(&shape.shape, &mut found);
        }
        output.textures_delta.clear();
        let x = |text: &str| found.iter().find(|(s, _)| s == text).unwrap().1;
        assert!((x("very long heading") - x("x")).abs() < 0.1);
        assert!((x("B") - x("second")).abs() < 0.1);
        output.textures_delta.clear();
    }

    #[test]
    fn repeated_viewport_does_not_rebuild_offsets_or_shape_all_blocks() {
        let doc = MarkdownDocument::parse(
            &(0..10_000)
                .map(|i| format!("Paragraph {i}\n\n"))
                .collect::<String>(),
        )
        .unwrap();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            RENDERED_BLOCKS.with(|count| count.set(0));
            let mut output = ctx.run_ui(Default::default(), |ui| {
                ui.set_max_height(200.0);
                doc.show(ui);
            });
            output.textures_delta.clear();
            RENDERED_BLOCKS
                .with(|count| assert!(count.get() < 100, "{} blocks laid out", count.get()));
            let mut jobs = Vec::new();
            for shape in &output.shapes {
                collect(&shape.shape, &mut jobs);
            }
            assert!(jobs.len() < 100);
            output.textures_delta.clear();
        }
        let cache = doc.heights.borrow();
        assert_eq!(cache.2.len(), 10_001);
    }
    #[test]
    fn rejects_oversized_single_layout_blocks() {
        let huge = "x".repeat(1024 * 1024);
        assert!(
            MarkdownDocument::parse(&huge)
                .unwrap_err()
                .contains("16 KiB")
        );
        assert!(
            MarkdownDocument::parse(&format!("```\n{huge}\n```"))
                .unwrap_err()
                .contains("16 KiB")
        );
        let ordinary = (0..100)
            .map(|i| format!("paragraph {i}\n\n"))
            .collect::<String>();
        assert!(MarkdownDocument::parse(&ordinary).is_ok());
    }
    #[test]
    fn fenced_code_background_is_contiguous() {
        let doc = MarkdownDocument::parse("```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n")
            .unwrap();
        let ctx = egui::Context::default();
        let mut fill = egui::Color32::TRANSPARENT;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            fill = ui.visuals().faint_bg_color;
            doc.show(ui);
        });
        fn rects(shape: &Shape, fill: egui::Color32, out: &mut Vec<egui::Rect>) {
            match shape {
                Shape::Rect(rect) if rect.fill == fill => out.push(rect.rect),
                Shape::Vec(shapes) => {
                    for shape in shapes {
                        rects(shape, fill, out);
                    }
                }
                _ => {}
            }
        }
        let mut backgrounds = Vec::new();
        for shape in &output.shapes {
            rects(&shape.shape, fill, &mut backgrounds);
        }
        output.textures_delta.clear();
        backgrounds.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
        assert_eq!(backgrounds.len(), 3, "{backgrounds:?}");
        for pair in backgrounds.windows(2) {
            assert!((pair[0].max.y - pair[1].min.y).abs() < 0.1, "{pair:?}");
        }
    }
}
