//! Read-only Git review, with an explicit preview before sending comments.
mod message;
use self::message::{preview_text, quote, safe};
use super::diff::{Change, DiffLine, Document};
use eframe::egui::{self, Color32, RichText};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Unified,
    SideBySide,
}
#[derive(Clone)]
struct Location {
    path: PathBuf,
    side: &'static str,
    line: u32,
}
struct Comment {
    location: Location,
    text: String,
}
struct Draft {
    location: Location,
    text: String,
}

pub(super) struct Review {
    file: usize,
    mode: Mode,
    draft: Option<Draft>,
    comments: Vec<Comment>,
    hunk: usize,
    scroll_to: Option<usize>,
    preview: bool,
    theme: crate::theme::Theme,
}

impl Default for Review {
    fn default() -> Self {
        Self {
            file: 0,
            mode: Mode::Unified,
            draft: None,
            comments: Vec::new(),
            hunk: 0,
            scroll_to: None,
            preview: false,
            theme: crate::theme::Theme::default(),
        }
    }
}

impl Review {
    pub(super) fn set_theme(&mut self, theme: crate::theme::Theme) {
        self.theme = theme;
    }
    pub(super) fn has_comments(&self) -> bool {
        !self.comments.is_empty() || self.draft.as_ref().is_some_and(|d| !d.text.is_empty())
    }
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui, doc: &Document) {
        if doc.files.is_empty() {
            return;
        }
        self.file = self.file.min(doc.files.len() - 1);
        let count = doc.files[self.file].hunks.len();
        if ui
            .add(egui::Button::new("Unified").selected(self.mode == Mode::Unified))
            .clicked()
        {
            self.mode = Mode::Unified;
        }
        if ui
            .add(egui::Button::new("Side by side").selected(self.mode == Mode::SideBySide))
            .clicked()
        {
            self.mode = Mode::SideBySide;
        }
        if ui
            .small_button("‹")
            .on_hover_text("Previous hunk")
            .clicked()
        {
            self.hunk = self.hunk.saturating_sub(1);
            self.scroll_to = Some(self.hunk);
        }
        if ui.small_button("›").on_hover_text("Next hunk").clicked() && count > 0 {
            self.hunk = (self.hunk + 1).min(count - 1);
            self.scroll_to = Some(self.hunk);
        }
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        source_label: &str,
        repository: &Path,
    ) -> Option<String> {
        if doc.files.is_empty() {
            ui.label("No changes.");
            return None;
        }
        self.file = self.file.min(doc.files.len() - 1);
        let height = ui.available_height().max(100.0);
        let mut send = None;
        if ui.available_width() < 560.0 {
            egui::ScrollArea::horizontal()
                .id_salt("review-files-narrow")
                .max_height(55.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (index, file) in doc.files.iter().enumerate() {
                            ui.vertical(|ui| {
                                if ui
                                    .selectable_label(index == self.file, &file.display)
                                    .clicked()
                                {
                                    self.file = index;
                                    self.hunk = 0;
                                    self.scroll_to = Some(0);
                                }
                                let added = file
                                    .hunks
                                    .iter()
                                    .flat_map(|h| &h.lines)
                                    .filter(|line| line.kind == Change::Added)
                                    .count();
                                let removed = file
                                    .hunks
                                    .iter()
                                    .flat_map(|h| &h.lines)
                                    .filter(|line| line.kind == Change::Removed)
                                    .count();
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        self.theme.color(term_core::TermColor::Indexed(2)),
                                        format!("+{added}"),
                                    );
                                    ui.colored_label(
                                        self.theme.color(term_core::TermColor::Indexed(1)),
                                        format!("−{removed}"),
                                    );
                                });
                            });
                        }
                    });
                });
            ui.separator();
            self.right_column(
                ui,
                doc,
                source_label,
                repository,
                (height - 60.0).max(80.0),
                &mut send,
            );
        } else {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(140.0, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_width(140.0);
                        ui.set_max_width(140.0);
                        file_list(self, ui, doc, height);
                    },
                );
                let edge = ui.cursor().left_top();
                ui.painter().line_segment(
                    [edge, edge + egui::vec2(0.0, height)],
                    egui::Stroke::new(1.0, self.theme.divider()),
                );
                ui.add_space(5.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width().max(20.0), height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.right_column(ui, doc, source_label, repository, height, &mut send);
                    },
                );
            });
        }
        send
    }

    fn right_column(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        source_label: &str,
        repository: &Path,
        height: f32,
        send: &mut Option<String>,
    ) {
        if height < 360.0 {
            egui::ScrollArea::vertical()
                .id_salt("review-short-column")
                .max_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    right_column_content(self, ui, doc, source_label, repository, height, send);
                });
        } else {
            right_column_content(self, ui, doc, source_label, repository, height, send);
        }
    }
}

fn right_column_content(
    review: &mut Review,
    ui: &mut egui::Ui,
    doc: &Document,
    source_label: &str,
    repository: &Path,
    height: f32,
    send: &mut Option<String>,
) {
    let footer = if review.preview {
        245.0
    } else if review.draft.is_some() {
        155.0
    } else if !review.comments.is_empty() {
        70.0
    } else {
        8.0
    };
    changes(review, ui, doc, (height - footer).max(60.0));
    comments(review, ui);
    if !review.comments.is_empty() {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("Preview comments").color(review.theme.background()),
                        )
                        .fill(review.theme.foreground()),
                    )
                    .clicked()
                {
                    review.preview = true;
                }
            });
        });
    }
    if review.preview {
        let text = preview_text(review, source_label, repository);
        ui.separator();
        ui.label("Preview for console");
        ui.label("The text is inserted without Enter. Submit it in the source console when ready.");
        egui::Frame::new()
            .fill(review.theme.background())
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .id_salt("review-preview")
                    .max_height(60.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&text).monospace().size(12.0))
                                .selectable(true),
                        );
                    });
            });
        if review.comments.iter().any(|c| !safe(&c.text)) {
            ui.colored_label(
                Color32::RED,
                "Comments cannot contain control characters other than tabs and newlines.",
            );
        } else if !review.comments.is_empty() {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Insert into console")
                                    .color(review.theme.background()),
                            )
                            .fill(review.theme.foreground()),
                        )
                        .clicked()
                    {
                        *send = Some(text);
                    }
                });
            });
        }
    }
}

fn file_list(review: &mut Review, ui: &mut egui::Ui, doc: &Document, height: f32) {
    egui::ScrollArea::vertical()
        .id_salt("review-files")
        .max_height(height)
        .show(ui, |ui| {
            ui.add_space(8.0);
            ui.indent("file-list-padding", |ui| {
                for (index, file) in doc.files.iter().enumerate() {
                    let added = file
                        .hunks
                        .iter()
                        .flat_map(|h| &h.lines)
                        .filter(|l| l.kind == Change::Added)
                        .count();
                    let removed = file
                        .hunks
                        .iter()
                        .flat_map(|h| &h.lines)
                        .filter(|l| l.kind == Change::Removed)
                        .count();
                    if ui
                        .selectable_label(index == review.file, &file.display)
                        .clicked()
                    {
                        review.file = index;
                        review.hunk = 0;
                        review.scroll_to = Some(0);
                    }
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            review.theme.color(term_core::TermColor::Indexed(2)),
                            format!("+{added}"),
                        );
                        ui.colored_label(
                            review.theme.color(term_core::TermColor::Indexed(1)),
                            format!("−{removed}"),
                        );
                    });
                    ui.add_space(5.0);
                }
            });
        });
}

fn changes(review: &mut Review, ui: &mut egui::Ui, doc: &Document, height: f32) {
    let file = &doc.files[review.file];
    ui.label(RichText::new(&file.display).color(ui.visuals().weak_text_color()));
    if review.mode == Mode::SideBySide {
        ui.horizontal(|ui| {
            let width = (ui.available_width() / 2.0).max(1.0);
            ui.allocate_ui(egui::vec2(width, 18.0), |ui| {
                ui.set_min_width(width);
                ui.weak("Before");
            });
            ui.weak("After");
        });
    }
    if let Some(notice) = &file.notice {
        ui.label(notice);
    }
    let mut rows = Vec::new();
    let mut starts = Vec::new();
    for (h, hunk) in file.hunks.iter().enumerate() {
        starts.push(rows.len());
        rows.push(Row::Header(h));
        match review.mode {
            Mode::Unified => rows.extend((0..hunk.lines.len()).map(|line| Row::Unified(h, line))),
            Mode::SideBySide => rows.extend((0..hunk.pairs.len()).map(|pair| Row::Pair(h, pair))),
        }
    }
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace).max(23.0);
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let glyph_width = ui
        .painter()
        .layout_no_wrap("M".into(), font, Color32::WHITE)
        .size()
        .x;
    let max_bytes = file
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .map(|line| line.text.len())
        .max()
        .unwrap_or(0);
    let columns = if review.mode == Mode::SideBySide {
        2.0
    } else {
        1.0
    };
    let content_width = ui
        .available_width()
        .max((max_bytes as f32 + 14.0) * glyph_width * columns);
    let mut area = egui::ScrollArea::both()
        .id_salt(("review-rows", review.file, review.mode == Mode::SideBySide))
        .max_height((height - 46.0).max(40.0))
        .auto_shrink([false, false]);
    if let Some(hunk) = review.scroll_to.take() {
        area =
            area.vertical_scroll_offset(starts.get(hunk).copied().unwrap_or(0) as f32 * row_height);
    }
    area.show_rows(ui, row_height, rows.len(), |ui, range| {
        ui.set_min_width(content_width);
        for row in &rows[range] {
            match *row {
                Row::Header(h) => {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(content_width, row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        &file.hunks[h].header,
                        egui::TextStyle::Monospace.resolve(ui.style()),
                        review.theme.foreground(),
                    );
                }
                Row::Unified(h, line) => paint_line(
                    review,
                    ui,
                    &file.path,
                    &file.hunks[h].lines[line],
                    None,
                    ui.available_width(),
                    row_height,
                ),
                Row::Pair(h, pair) => {
                    let hunk = &file.hunks[h];
                    let pair = &hunk.pairs[pair];
                    ui.horizontal(|ui| {
                        let width = (ui.available_width() / 2.0 - 2.0).max(1.0);
                        if let Some(i) = pair.old {
                            paint_line(
                                review,
                                ui,
                                &file.path,
                                &hunk.lines[i],
                                Some("old"),
                                width,
                                row_height,
                            );
                        } else {
                            ui.allocate_space(egui::vec2(width, row_height));
                        }
                        if let Some(i) = pair.new {
                            paint_line(
                                review,
                                ui,
                                &file.path,
                                &hunk.lines[i],
                                Some("new"),
                                width,
                                row_height,
                            );
                        } else {
                            ui.allocate_space(egui::vec2(width, row_height));
                        }
                    });
                }
            }
        }
    });
}

fn paint_line(
    review: &mut Review,
    ui: &mut egui::Ui,
    path: &Path,
    line: &DiffLine,
    side: Option<&'static str>,
    width: f32,
    height: f32,
) {
    let side = side.unwrap_or(if line.kind == Change::Removed {
        "old"
    } else {
        "new"
    });
    let number = if side == "old" { line.old } else { line.new };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let ink = match line.kind {
        Change::Added => Some(review.theme.color(term_core::TermColor::Indexed(2))),
        Change::Removed => Some(review.theme.color(term_core::TermColor::Indexed(1))),
        Change::Context => None,
    };
    if let Some(ink) = ink {
        let bg = review.theme.background();
        let mix = |a: u8, b: u8| ((a as u16 * 4 + b as u16) / 5) as u8;
        ui.painter().rect_filled(
            rect,
            0.0,
            Color32::from_rgb(
                mix(bg.r(), ink.r()),
                mix(bg.g(), ink.g()),
                mix(bg.b(), ink.b()),
            ),
        );
    }
    let number_text = |n: Option<u32>| n.map_or_else(String::new, |n| n.to_string());
    let gutter = if side == "old" || side == "new" {
        format!("{:>4}", number_text(number))
    } else {
        String::new()
    };
    let gutter = if review.mode == Mode::Unified {
        format!("{:>4} {:>4}", number_text(line.old), number_text(line.new))
    } else {
        gutter
    };
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    ui.painter().with_clip_rect(rect).text(
        rect.left_center() + egui::vec2(6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{gutter}  {}", line.text),
        font,
        review.theme.foreground(),
    );
    let plus = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 24.0, rect.top()),
        egui::vec2(24.0, height),
    );
    if number.is_some() && (response.hovered() || response.has_focus()) {
        ui.painter().text(
            plus.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            egui::FontId::proportional(16.0),
            review.theme.foreground(),
        );
    }
    if let Some(number) = number
        && response.clicked()
        && review.draft.is_none()
    {
        review.draft = Some(Draft {
            location: Location {
                path: path.to_path_buf(),
                side,
                line: number,
            },
            text: String::new(),
        });
    }
}

fn comments(review: &mut Review, ui: &mut egui::Ui) {
    if let Some(draft) = &mut review.draft {
        ui.separator();
        ui.label(format!(
            "Comment on {} {}:{}",
            quote(&draft.location.path.to_string_lossy()),
            draft.location.side,
            draft.location.line
        ));
        ui.add(egui::TextEdit::multiline(&mut draft.text).desired_width(f32::INFINITY));
        let mut add = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                cancel = ui.button("Cancel comment").clicked();
                add = ui
                    .add_enabled(
                        !draft.text.trim().is_empty() && safe(&draft.text),
                        egui::Button::new("Add comment"),
                    )
                    .clicked();
            });
        });
        if !safe(&draft.text) {
            ui.colored_label(
                Color32::RED,
                "Comment contains a forbidden control character.",
            );
        }
        if add {
            let draft = review.draft.take().unwrap();
            review.comments.push(Comment {
                location: draft.location,
                text: draft.text,
            });
            review.preview = false;
        } else if cancel {
            review.draft = None;
        }
    }
    let mut remove = None;
    for (index, comment) in review.comments.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!(
                "{} {}:{} - {}",
                quote(&comment.location.path.to_string_lossy()),
                comment.location.side,
                comment.location.line,
                comment.text
            ));
            if ui.small_button("Remove").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        review.comments.remove(index);
        review.preview = false;
    }
}

enum Row {
    Header(usize),
    Unified(usize, usize),
    Pair(usize, usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::epaint::Shape;

    fn frame(
        review: &mut Review,
        ctx: &egui::Context,
        doc: &Document,
        events: Vec<egui::Event>,
    ) -> (Vec<(String, egui::Pos2)>, Option<String>) {
        fn collect(shape: &Shape, labels: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                Shape::Text(text) => labels.push((
                    text.galley.job.text.clone(),
                    text.pos + text.galley.rect.center().to_vec2(),
                )),
                Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, labels);
                    }
                }
                _ => {}
            }
        }
        let mut sent = None;
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                ui.horizontal_wrapped(|ui| review.toolbar(ui, doc));
                sent = review.show(ui, doc, "console", Path::new("/repo"));
            },
        );
        let mut labels = Vec::new();
        for shape in &output.shapes {
            collect(&shape.shape, &mut labels);
        }
        output.drop_without_applying_deltas();
        (labels, sent)
    }

    fn click(
        review: &mut Review,
        ctx: &egui::Context,
        doc: &Document,
        label: &str,
    ) -> Option<String> {
        let labels = frame(review, ctx, doc, vec![]).0;
        let pos = labels
            .iter()
            .find(|(s, _)| s == label || (label == "old" && s.ends_with("  old")))
            .unwrap_or_else(|| panic!("missing {label}: {labels:?}"))
            .1;
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            review,
            ctx,
            doc,
            vec![egui::Event::PointerMoved(pos), button(true)],
        );
        frame(review, ctx, doc, vec![button(false)]).1
    }

    #[test]
    fn preview_escapes_metadata_and_rejects_controls() {
        let mut review = Review::default();
        review.comments.push(Comment {
            location: Location {
                path: PathBuf::from("bad\nname"),
                side: "new",
                line: 4,
            },
            text: "Please fix\nthis".into(),
        });
        assert_eq!(
            preview_text(&review, "pane\u{1b}", Path::new("/repo")),
            "Git review for console \"pane\\u{1b}\"\nRepository: \"/repo\"\n\nFile: \"bad\\nname\"\nnew line 4\nPlease fix\nthis\n"
        );
        assert!(safe("a\nb\tc"));
        assert!(!safe("a\u{1b}[31m"));
    }

    #[test]
    fn user_can_comment_preview_and_send_a_line() {
        let doc = Document::parse(
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n",
        )
        .unwrap();
        let mut review = Review::default();
        let ctx = egui::Context::default();
        assert!(click(&mut review, &ctx, &doc, "old").is_none());
        assert!(review.draft.is_some());
        // TextEdit owns the typed text after its real click.
        review.draft.as_mut().unwrap().text = "Please revise".into();
        click(&mut review, &ctx, &doc, "Add comment");
        click(&mut review, &ctx, &doc, "Side by side");
        assert!(review.mode == Mode::SideBySide);
        click(&mut review, &ctx, &doc, "Preview comments");
        let sent = click(&mut review, &ctx, &doc, "Insert into console").unwrap();
        assert_eq!(
            sent,
            "Git review for console \"console\"\nRepository: \"/repo\"\n\nFile: \"a.txt\"\nold line 1\nPlease revise\n"
        );
    }
    #[test]
    fn empty_or_unsafe_review_cannot_be_sent() {
        let doc = Document::default();
        let ctx = egui::Context::default();
        let mut review = Review {
            preview: true,
            ..Review::default()
        };
        let (labels, sent) = frame(&mut review, &ctx, &doc, vec![]);
        assert!(sent.is_none());
        assert!(
            !labels
                .iter()
                .any(|(label, _)| label == "Insert into console")
        );
        review.comments.push(Comment {
            location: Location {
                path: "a".into(),
                side: "new",
                line: 1,
            },
            text: "bad\u{1b}".into(),
        });
        let (labels, sent) = frame(&mut review, &ctx, &doc, vec![]);
        assert!(sent.is_none());
        assert!(
            !labels
                .iter()
                .any(|(label, _)| label == "Insert into console")
        );
    }
    #[test]
    fn long_diff_line_scrolls_horizontally() {
        let long = format!("{} END", "x".repeat(300));
        let doc =
            Document::parse(&format!("diff --git a/a b/a\n@@ -0,0 +1 @@\n+{long}\n")).unwrap();
        let mut review = Review::default();
        let ctx = egui::Context::default();
        let labels = frame(&mut review, &ctx, &doc, vec![]).0;
        let before = labels
            .iter()
            .find(|(text, _)| text.ends_with(" END"))
            .unwrap()
            .1;
        frame(
            &mut review,
            &ctx,
            &doc,
            vec![
                egui::Event::PointerMoved(egui::pos2(500.0, before.y)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(-400.0, 0.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let labels = frame(&mut review, &ctx, &doc, vec![]).0;
        let after = labels
            .iter()
            .find(|(text, _)| text.ends_with(" END"))
            .unwrap()
            .1;
        assert!(
            after.x < before.x - 1.0,
            "line did not scroll: {before:?} -> {after:?}"
        );
    }
}
