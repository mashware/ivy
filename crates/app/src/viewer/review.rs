//! Read-only Git review, with an explicit preview before sending comments.
mod message;
mod paint;
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
    /// Where Side by side divides its two columns, as a share of the visible width; the person
    /// drags it (asked for on 17 November 2026).
    split: f32,
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
            split: 0.5,
        }
    }
}

impl Review {
    pub(super) fn set_theme(&mut self, theme: crate::theme::Theme) {
        self.theme = theme;
    }
    pub(super) fn side_by_side(&self) -> bool {
        self.mode == Mode::SideBySide
    }
    /// The drawer's strip chooses the layout; the review only draws it.
    pub(super) fn set_side_by_side(&mut self, side_by_side: bool) {
        self.mode = if side_by_side {
            Mode::SideBySide
        } else {
            Mode::Unified
        };
    }
    pub(super) fn has_comments(&self) -> bool {
        !self.comments.is_empty() || self.draft.as_ref().is_some_and(|d| !d.text.is_empty())
    }
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        source_label: &str,
        repository: &Path,
        can_send: bool,
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
                        for index in 0..doc.files.len() {
                            ui.allocate_ui(egui::vec2(220.0, 26.0), |ui| {
                                file_row(self, ui, doc, index);
                            });
                        }
                    });
                });
            ui.separator();
            send = self.right_column(
                ui,
                doc,
                source_label,
                repository,
                (height - 60.0).max(80.0),
                can_send,
            )
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
                        send =
                            self.right_column(ui, doc, source_label, repository, height, can_send);
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
        can_send: bool,
    ) -> Option<String> {
        if height < 360.0 {
            egui::ScrollArea::vertical()
                .id_salt("review-short-column")
                .max_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    right_column_content(self, ui, doc, source_label, repository, height, can_send)
                })
                .inner
        } else {
            right_column_content(self, ui, doc, source_label, repository, height, can_send)
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
    can_send: bool,
) -> Option<String> {
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
    paint::comments(review, ui);
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
    let mut send = None;
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
                        .add_enabled(
                            can_send,
                            egui::Button::new(
                                RichText::new("Insert into console")
                                    .color(review.theme.background()),
                            )
                            .fill(review.theme.foreground()),
                        )
                        .clicked()
                    {
                        send = Some(text);
                    }
                    if !can_send {
                        ui.label("Origin console unavailable; comments cannot be inserted.");
                    }
                });
            });
        }
    }
    send
}

fn file_list(review: &mut Review, ui: &mut egui::Ui, doc: &Document, height: f32) {
    egui::ScrollArea::vertical()
        .id_salt("review-files")
        .max_height(height)
        .show(ui, |ui| {
            ui.add_space(4.0);
            for index in 0..doc.files.len() {
                file_row(review, ui, doc, index);
            }
        });
}

/// One changed file: its name first, then its folder dimmed, and its added and removed counts on
/// the same row, as `Git.dc.html` draws them.
fn file_row(review: &mut Review, ui: &mut egui::Ui, doc: &Document, index: usize) {
    let file = &doc.files[index];
    let count = |kind| {
        file.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| l.kind == kind)
            .count()
    };
    let (added, removed) = (count(Change::Added), count(Change::Removed));
    let path = Path::new(&file.display);
    let name = path.file_name().map_or_else(
        || file.display.clone(),
        |n| n.to_string_lossy().into_owned(),
    );
    let folder = path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| !p.is_empty());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
    let roles = crate::ui::style::roles(ui.ctx(), &review.theme);
    let (weak, strong) = (roles.tx3, roles.tx);
    if index == review.file {
        ui.painter().rect_filled(rect, 5, roles.sel);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 5, roles.hover);
    }
    let painter = ui.painter_at(rect);
    let font = egui::FontId::proportional(12.5);
    let small = egui::FontId::proportional(11.0);
    let green = review.theme.color(term_core::TermColor::Indexed(2));
    let red = review.theme.color(term_core::TermColor::Indexed(1));
    let mut right = rect.right() - 8.0;
    for (text, color) in [(format!("−{removed}"), red), (format!("+{added}"), green)] {
        let at = painter.text(
            egui::pos2(right, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            text,
            small.clone(),
            color,
        );
        right = at.left() - 6.0;
    }
    let mut job = egui::text::LayoutJob::default();
    job.append(&name, 0.0, egui::TextFormat::simple(font, strong));
    if let Some(folder) = &folder {
        job.append(folder, 8.0, egui::TextFormat::simple(small, weak));
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - rect.left() - 14.0).max(0.0));
    let galley = painter.layout_job(job);
    painter.galley(
        egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0),
        galley,
        strong,
    );
    if response.on_hover_text(&file.display).clicked() {
        review.file = index;
        review.hunk = 0;
        review.scroll_to = Some(0);
    }
}

fn changes(review: &mut Review, ui: &mut egui::Ui, doc: &Document, height: f32) {
    let file = &doc.files[review.file];
    ui.label(RichText::new(&file.display).color(ui.visuals().weak_text_color()));
    let before_width = if review.mode == Mode::SideBySide {
        Some(split_header(review, ui))
    } else {
        None
    };
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
    // Rows touch, as in `GitDiff.dc.html`: a run of added lines reads as one band.
    let row_height = (ui.text_style_height(&egui::TextStyle::Monospace) + 6.0).max(20.0);
    ui.spacing_mut().item_spacing.y = 0.0;
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
                Row::Unified(h, line) => paint::paint_line(
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
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let left = before_width.unwrap_or(ui.available_width() / 2.0).max(1.0);
                        if let Some(i) = pair.old {
                            paint::paint_line(
                                review,
                                ui,
                                &file.path,
                                &hunk.lines[i],
                                Some("old"),
                                left,
                                row_height,
                            );
                        } else {
                            ui.allocate_space(egui::vec2(left, row_height));
                        }
                        let right = ui.available_width().max(1.0);
                        let divider = ui.cursor().left();
                        if let Some(i) = pair.new {
                            paint::paint_line(
                                review,
                                ui,
                                &file.path,
                                &hunk.lines[i],
                                Some("new"),
                                right,
                                row_height,
                            );
                        } else {
                            ui.allocate_space(egui::vec2(right, row_height));
                        }
                        let roles = crate::ui::style::roles(ui.ctx(), &review.theme);
                        ui.painter().vline(
                            divider,
                            ui.min_rect().y_range(),
                            egui::Stroke::new(1.0, roles.line),
                        );
                    });
                }
            }
        }
    });
}

/// The Before / After header with the handle that moves the column split; returns the Before
/// column's width.
fn split_header(review: &mut Review, ui: &mut egui::Ui) -> f32 {
    let width = ui.available_width().max(1.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
    let left =
        (width * review.split).clamp(80.0_f32.min(width / 2.0), (width - 80.0).max(width / 2.0));
    let handle = egui::Rect::from_center_size(
        egui::pos2(rect.left() + left, rect.center().y),
        egui::vec2(8.0, rect.height()),
    );
    let response = ui
        .interact(handle, ui.id().with("review-split"), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
        .on_hover_text("Drag to resize the two sides");
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        review.split = ((pointer.x - rect.left()) / width).clamp(0.15, 0.85);
    }
    let roles = crate::ui::style::roles(ui.ctx(), &review.theme);
    let weak = ui.visuals().weak_text_color();
    let font = egui::FontId::proportional(12.0);
    ui.painter().text(
        rect.left_center() + egui::vec2(6.0, 0.0),
        egui::Align2::LEFT_CENTER,
        "Before",
        font.clone(),
        weak,
    );
    ui.painter().text(
        egui::pos2(rect.left() + left + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "After",
        font,
        weak,
    );
    let grip = if response.hovered() || response.dragged() {
        roles.accent
    } else {
        roles.line2
    };
    ui.painter().vline(
        rect.left() + left,
        rect.y_range(),
        egui::Stroke::new(2.0, grip),
    );
    left
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

    fn window_frame(
        window: &mut crate::Window,
        ctx: &egui::Context,
        area: egui::Rect,
        events: Vec<egui::Event>,
    ) -> Vec<(String, egui::Pos2)> {
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
        let mut frame = eframe::Frame::_new_kittest();
        eframe::App::logic(window, ctx, &mut frame);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area),
                events,
                ..Default::default()
            },
            |ui| eframe::App::ui(window, ui, &mut frame),
        );
        let mut labels = Vec::new();
        for shape in &output.shapes {
            collect(&shape.shape, &mut labels);
        }
        output.drop_without_applying_deltas();
        labels
    }

    fn review_comment() -> Comment {
        Comment {
            location: Location {
                path: PathBuf::from("first.rs"),
                side: "new",
                line: 1,
            },
            text: "unsent review note".into(),
        }
    }

    fn loaded_diff() -> Document {
        Document::parse(
            "diff --git a/first.rs b/first.rs\n--- a/first.rs\n+++ b/first.rs\n@@ -1 +1 @@\n-old\n+new\n",
        )
        .unwrap()
    }

    #[test]
    fn docked_window_insert_is_live_only_and_other_git_requests_reveal_retained_review() {
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 1400.0));
        // Tall enough for the preview's Insert without scrolling the review's column.
        window.sidebar.set_height(900.0);
        crate::tests::one_frame_in(&mut window, &ctx, area, vec![]);
        crate::viewer::select_state(&mut window, false);
        let ids: Vec<_> = window.all_panes().map(|pane| pane.name).collect();
        window.viewer.kind = super::super::load::Kind::Local;
        window.viewer.path = "/repo".into();
        window.viewer.origin = Some(super::super::Source {
            pane: ids[0],
            label: "console A".into(),
            cwd: Some(Path::new("/repo").into()),
        });
        window.viewer.loaded = Some(super::super::load::Loaded {
            text: "diff".into(),
            markdown: None,
            diff: Some(loaded_diff()),
        });
        window.viewer.review.comments.push(review_comment());
        window.viewer.review.preview = true;
        window.viewer.open = true;
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Git);

        // An explicit request for another console while Files is selected must reveal the retained
        // source and the replacement warning instead of leaving Files selected or discarding it.
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
        crate::viewer::origin::open_diff_for_pane(&mut window, ids[1]);
        assert_eq!(
            crate::sidebar::active(&window),
            Some(crate::sidebar::Tool::Git)
        );
        assert_eq!(window.viewer.origin.as_ref().unwrap().pane, ids[0]);
        assert!(window.viewer.message.contains("Send or clear"));

        // The actual Window -> dock -> viewer -> Review path enables Insert for the live origin.
        window.viewer.message.clear();
        window_frame(&mut window, &ctx, area, vec![]);
        let painted = window_frame(&mut window, &ctx, area, vec![]);
        let insert = painted
            .iter()
            .find(|(text, _)| text == "Insert into console")
            .unwrap_or_else(|| panic!("live Git dock paints its Insert control: {painted:?}"))
            .1;
        let button = |pressed| egui::Event::PointerButton {
            pos: insert,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        window_frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(insert), button(true)],
        );
        window_frame(&mut window, &ctx, area, vec![button(false)]);
        assert!(
            window.viewer.pending_send.is_some(),
            "live Insert did not queue review text"
        );

        // Removing the origin identity makes the same dock render it as unavailable and blocks
        // delivery; no replacement pane can inherit the review.
        window.viewer.pending_send = None;
        window.viewer.origin.as_mut().unwrap().pane = usize::MAX;
        let labels = window_frame(&mut window, &ctx, area, vec![]);
        assert!(labels.iter().any(|(text, _)| text.contains("unavailable")));
        let insert = labels
            .iter()
            .find(|(text, _)| text == "Insert into console")
            .unwrap()
            .1;
        window_frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(insert),
                button(true),
                button(false),
            ],
        );
        assert!(window.viewer.pending_send.is_none());
    }

    #[test]
    fn git_review_context_survives_switches_through_every_sidebar_tool() {
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        crate::tests::one_frame_in(&mut window, &ctx, area, vec![]);
        let pane = window.active().panes()[0].name;
        crate::viewer::select_state(&mut window, false);
        window.viewer.kind = super::super::load::Kind::AgainstBase;
        window.viewer.path = "/repo".into();
        window.viewer.base = "release/73".into();
        window.viewer.origin = Some(super::super::Source {
            pane,
            label: "console A".into(),
            cwd: Some(Path::new("/repo").into()),
        });
        window.viewer.loaded = Some(super::super::load::Loaded {
            text: "diff".into(),
            markdown: None,
            diff: Some(Document::parse(
                "diff --git a/first.rs b/first.rs\n--- a/first.rs\n+++ b/first.rs\n@@ -1 +1 @@\n-old\n+new\n",
            ).unwrap()),
        });
        window.viewer.review.file = 0;
        window.viewer.review.hunk = 1;
        window.viewer.review.comments.push(Comment {
            location: Location {
                path: PathBuf::from("first.rs"),
                side: "new",
                line: 1,
            },
            text: "unsent review note".into(),
        });
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Git);

        for tool in [
            crate::sidebar::Tool::Markdown,
            crate::sidebar::Tool::Files,
            crate::sidebar::Tool::Board,
            crate::sidebar::Tool::Git,
        ] {
            crate::sidebar::select(&mut window, tool);
        }

        assert_eq!(
            crate::sidebar::active(&window),
            Some(crate::sidebar::Tool::Git)
        );
        assert_eq!(window.viewer.path, "/repo");
        assert_eq!(window.viewer.base, "release/73");
        assert_eq!(window.viewer.origin.as_ref().unwrap().pane, pane);
        assert_eq!(
            window
                .viewer
                .loaded
                .as_ref()
                .unwrap()
                .diff
                .as_ref()
                .unwrap()
                .files[0]
                .path,
            Path::new("first.rs")
        );
        assert_eq!(window.viewer.review.file, 0);
        assert_eq!(window.viewer.review.hunk, 1);
        assert_eq!(window.viewer.review.comments[0].text, "unsent review note");
    }

    #[test]
    fn explorer_transition_keeps_comments_and_rejects_a_different_document() {
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        crate::tests::one_frame_in(&mut window, &ctx, area, vec![]);
        window.viewer.path = "/tmp/original.md".into();
        window.viewer.review.draft = Some(Draft {
            location: Location {
                path: "original.rs".into(),
                side: "new",
                line: 1,
            },
            text: "Keep this comment".into(),
        });
        crate::viewer::hide(&mut window);
        let pane = window.active().panes()[0].name;
        assert!(!crate::viewer::open_markdown_target(
            &mut window,
            pane,
            "/tmp/other.md".into(),
            None
        ));
        assert_eq!(window.viewer.path, "/tmp/original.md");
        assert!(window.viewer.review.has_comments());
        assert!(window.viewer.message.contains("Send or clear"));
    }

    fn frame(
        review: &mut Review,
        ctx: &egui::Context,
        doc: &Document,
        events: Vec<egui::Event>,
    ) -> (Vec<(String, egui::Pos2)>, Option<String>) {
        frame_for_source(review, ctx, doc, events, "console", true)
    }

    fn frame_for_source(
        review: &mut Review,
        ctx: &egui::Context,
        doc: &Document,
        events: Vec<egui::Event>,
        source_label: &str,
        can_send: bool,
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
                sent = review.show(ui, doc, source_label, Path::new("/repo"), can_send);
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

    /// Every text the pass painted, with the rectangle it covers.
    fn painted(
        review: &mut Review,
        ctx: &egui::Context,
        doc: &Document,
        width: f32,
        events: Vec<egui::Event>,
    ) -> Vec<(String, egui::Rect)> {
        fn collect(shape: &Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                Shape::Text(text) => out.push((
                    text.galley.job.text.clone(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                )),
                Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, out)),
                _ => {}
            }
        }
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                review.show(ui, doc, "console", Path::new("/repo"), true);
            },
        );
        let mut out = Vec::new();
        for shape in &output.shapes {
            collect(&shape.shape, &mut out);
        }
        output.drop_without_applying_deltas();
        out
    }

    #[test]
    fn a_changed_file_is_one_row_name_first_and_a_click_on_it_shows_it() {
        let doc = Document::parse(concat!(
            "diff --git a/src/view/first.rs b/src/view/first.rs\n--- a/src/view/first.rs\n",
            "+++ b/src/view/first.rs\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/lib/second.rs b/lib/second.rs\n--- a/lib/second.rs\n",
            "+++ b/lib/second.rs\n@@ -1 +1,3 @@\n kept\n+one\n+two\n",
        ))
        .unwrap();
        // Wide, the list is a column beside the changes; narrow, a strip above them.
        for width in [1100.0, 500.0] {
            let mut review = Review::default();
            let ctx = egui::Context::default();
            let shapes = painted(&mut review, &ctx, &doc, width, vec![]);
            let row = shapes
                .iter()
                .find(|(text, _)| text == "second.rslib")
                .unwrap_or_else(|| panic!("{width}: no name-then-folder row in {shapes:?}"))
                .1;
            for count in ["+2", "−0"] {
                let at = shapes
                    .iter()
                    .filter(|(text, _)| text == count)
                    .map(|(_, rect)| *rect)
                    .find(|rect| rect.left() > row.left())
                    .unwrap_or_else(|| panic!("{width}: no {count} in {shapes:?}"));
                assert!(
                    (at.center().y - row.center().y).abs() < 2.0,
                    "{width}: {count} at {at:?}, row at {row:?}"
                );
            }
            assert_eq!(review.file, 0);
            let button = |pressed| egui::Event::PointerButton {
                pos: row.left_center() + egui::vec2(4.0, 0.0),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            let pos = row.left_center() + egui::vec2(4.0, 0.0);
            painted(
                &mut review,
                &ctx,
                &doc,
                width,
                vec![egui::Event::PointerMoved(pos), button(true)],
            );
            painted(&mut review, &ctx, &doc, width, vec![button(false)]);
            assert_eq!(review.file, 1, "{width}");
        }
    }

    #[test]
    fn side_by_side_columns_are_resized_by_dragging_their_divider() {
        let doc = Document::parse(
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n",
        )
        .unwrap();
        let mut review = Review::default();
        review.set_side_by_side(true);
        let ctx = egui::Context::default();
        let shapes = painted(&mut review, &ctx, &doc, 1100.0, vec![]);
        let after = |shapes: &[(String, egui::Rect)]| {
            shapes
                .iter()
                .find(|(text, _)| text == "After")
                .expect("the After column is labelled")
                .1
                .left()
        };
        let before = after(&shapes);
        let grip = egui::pos2(
            before - 6.0,
            shapes
                .iter()
                .find(|(t, _)| t == "After")
                .unwrap()
                .1
                .center()
                .y,
        );
        let button = |pressed| egui::Event::PointerButton {
            pos: grip,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        painted(
            &mut review,
            &ctx,
            &doc,
            1100.0,
            vec![egui::Event::PointerMoved(grip), button(true)],
        );
        for step in 1..=4 {
            let to = grip + egui::vec2(-40.0 * step as f32, 0.0);
            painted(
                &mut review,
                &ctx,
                &doc,
                1100.0,
                vec![egui::Event::PointerMoved(to)],
            );
        }
        painted(&mut review, &ctx, &doc, 1100.0, vec![button(false)]);
        let moved = after(&painted(&mut review, &ctx, &doc, 1100.0, vec![]));
        assert!(
            moved < before - 100.0,
            "the split did not follow the drag: {before} -> {moved}"
        );
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
        review.set_side_by_side(true);
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
    fn live_source_enables_insert_and_removed_source_disables_it() {
        let doc = Document::parse(
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n",
        )
        .unwrap();
        let comment = Comment {
            location: Location {
                path: PathBuf::from("a.txt"),
                side: "new",
                line: 1,
            },
            text: "Please revise".into(),
        };
        let ctx = egui::Context::default();
        let mut review = Review {
            preview: true,
            comments: vec![comment],
            ..Review::default()
        };
        let (labels, sent) = frame_for_source(&mut review, &ctx, &doc, vec![], "pane-1", true);
        assert!(sent.is_none());
        let insert = labels
            .iter()
            .find(|(label, _)| label == "Insert into console")
            .unwrap()
            .1;
        let button = |pressed| egui::Event::PointerButton {
            pos: insert,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame_for_source(
            &mut review,
            &ctx,
            &doc,
            vec![egui::Event::PointerMoved(insert), button(true)],
            "pane-1",
            true,
        );
        assert!(
            frame_for_source(&mut review, &ctx, &doc, vec![button(false)], "pane-1", true,)
                .1
                .is_some()
        );

        let (labels, sent) = frame_for_source(
            &mut review,
            &ctx,
            &doc,
            vec![],
            "pane-1 - unavailable",
            false,
        );
        assert!(sent.is_none());
        assert!(
            labels
                .iter()
                .any(|(label, _)| label.contains("unavailable"))
        );
        let insert = labels
            .iter()
            .find(|(label, _)| label == "Insert into console")
            .unwrap()
            .1;
        let unavailable_button = |pressed| egui::Event::PointerButton {
            pos: insert,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame_for_source(
            &mut review,
            &ctx,
            &doc,
            vec![
                egui::Event::PointerMoved(insert),
                unavailable_button(true),
                unavailable_button(false),
            ],
            "pane-1 - unavailable",
            false,
        );
        assert!(
            frame_for_source(
                &mut review,
                &ctx,
                &doc,
                vec![],
                "pane-1 - unavailable",
                false,
            )
            .1
            .is_none()
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
