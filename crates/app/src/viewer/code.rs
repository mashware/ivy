//! The Document tab's view of a file that is not Markdown: read-only, with line numbers and the
//! colours of [`super::highlight`].
use super::highlight::{self, Family, Line};
use super::palette::Palette;

mod rows;
use eframe::egui::{
    self, FontId,
    text::{CharIndex, LayoutJob},
};
use std::path::{Path, PathBuf};

const FONT_SIZE: f32 = 13.0;
/// Space between the line numbers and the text.
const GUTTER_GAP: f32 = 14.0;
/// Space around the text.
const PAD: f32 = 8.0;
const TRUNCATED: &str = "… line truncated";

/// A file laid out in lines and coloured, built on the loader thread.
pub(crate) struct CodeDocument {
    /// Which file this is, so another file's view does not inherit this one's scroll.
    path: PathBuf,
    lines: Vec<Line>,
    /// The longest painted line in characters: a first guess at how far the view scrolls sideways,
    /// widened by what painting measures (tabs and wide characters take more than one cell).
    widest: usize,
}

impl CodeDocument {
    pub(super) fn parse(path: &Path, text: &str) -> Self {
        let first = text.lines().next().unwrap_or_default();
        let lines = highlight::scan(text, Family::of(path, first));
        let widest = lines
            .iter()
            .map(|line| line.text[..line.shown].chars().count())
            .max()
            .unwrap_or(0);
        Self {
            path: path.to_owned(),
            lines,
            widest,
        }
    }

    /// The text between two places, as the file holds it: line numbers are never part of it.
    fn text_between(&self, from: Place, to: Place) -> String {
        let (from, to) = if (from.line, from.byte) <= (to.line, to.byte) {
            (from, to)
        } else {
            (to, from)
        };
        let mut out = String::new();
        for line in from.line..=to.line.min(self.lines.len().saturating_sub(1)) {
            let text = &self.lines[line].text;
            let start = if line == from.line {
                boundary(text, from.byte)
            } else {
                0
            };
            let end = if line == to.line {
                boundary(text, to.byte)
            } else {
                text.len()
            };
            out.push_str(&text[start..end.max(start)]);
            if line != to.line {
                out.push('\n');
            }
        }
        out
    }

    fn end(&self) -> Place {
        let line = self.lines.len().saturating_sub(1);
        Place {
            line,
            byte: self.lines.get(line).map_or(0, |l| l.text.len()),
        }
    }
}

/// A place in the file: a line and a byte inside it. It belongs to the document it was taken from,
/// so the selection is cleared whenever that document is replaced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Place {
    line: usize,
    byte: usize,
}

/// What the person has marked, kept in the file's terms so a selection longer than the screen
/// copies whole: only the rows on screen are laid out in a frame.
#[derive(Default)]
pub(crate) struct CodeState {
    selection: Option<(Place, Place)>,
    /// The widest line painted so far of the file it names, in points. It outlives a new version
    /// of that file, so an agent rewriting it does not pull the sideways scroll back under the
    /// reader.
    widest_seen: (PathBuf, f32),
    /// The row above the file: its path, Open and its menu.
    pub(super) header: super::code_header::Header,
    /// Long lines fold at the view's width instead of scrolling sideways. Off until asked for.
    pub(super) wrap: bool,
    heights: rows::Heights,
}

impl CodeState {
    /// A new document, or a new version of the open one, starts with nothing marked and its
    /// lines measured again.
    pub(super) fn reset(&mut self) {
        self.selection = None;
        self.heights = rows::Heights::default();
    }

    #[cfg(test)]
    pub(super) fn select(&mut self, from: (usize, usize), to: (usize, usize)) {
        let place = |(line, byte)| Place { line, byte };
        self.selection = Some((place(from), place(to)));
    }

    #[cfg(test)]
    pub(super) fn has_selection(&self) -> bool {
        self.selection.is_some()
    }
}

pub(super) fn show(
    document: &CodeDocument,
    state: &mut CodeState,
    ui: &mut egui::Ui,
    palette: &Palette,
) {
    let font = FontId::monospace(FONT_SIZE);
    let (row, glyph) =
        ui.fonts_mut(|fonts| (fonts.row_height(&font), fonts.glyph_width(&font, '0')));
    let digits = document.lines.len().max(1).to_string().len();
    let gutter = PAD + digits as f32 * glyph;
    let text_left = gutter + GUTTER_GAP;
    let mark = TRUNCATED.chars().count() as f32 * glyph + glyph;
    if state.widest_seen.0 != document.path {
        state.widest_seen = (document.path.clone(), 0.0);
    }
    let width =
        text_left + (document.widest as f32 * glyph).max(state.widest_seen.1) + mark + 2.0 * PAD;
    let previous_scroll = ui.spacing().scroll;
    // A position indicator must survive both idle time and the selection owning the pointer.
    ui.spacing_mut().scroll = egui::style::ScrollStyle {
        foreground_color: true,
        ..egui::style::ScrollStyle::solid()
    };
    // Wrapped, the text takes what the view leaves after the gutter, never less than a few
    // characters.
    let wrap = state.wrap.then(|| {
        (ui.available_width() - ui.spacing().scroll.allocated_width() - text_left - 2.0 * PAD)
            .max(glyph * 8.0)
    });
    state.heights.prepare(&document.lines, wrap, row, glyph);
    let height = state.heights.total() + 2.0 * PAD;
    let select_all = ui.input(|i| {
        i.events.iter().any(|event| {
            matches!(event, egui::Event::Key { key: egui::Key::A, pressed: true, modifiers, .. }
                if modifiers.command)
        })
    });
    if select_all {
        state.selection = Some((Place::default(), document.end()));
    }
    egui::ScrollArea::new([!state.wrap, true])
        .id_salt(("code-content", &document.path))
        .auto_shrink([false; 2])
        .show_viewport(ui, |ui, viewport| {
            let wide = if state.wrap { 0.0 } else { width };
            let (content, response) = ui.allocate_exact_size(
                egui::vec2(wide.max(ui.available_width()), height),
                egui::Sense::click_and_drag(),
            );
            let origin = content.min + egui::vec2(0.0, PAD);
            let heights = &mut state.heights;
            let first = heights.line_at(viewport.min.y - PAD);
            let end = (heights.line_at(viewport.max.y - PAD) + 1).min(document.lines.len());
            let painter = ui.painter_at(ui.clip_rect());
            let mut galleys = Vec::with_capacity(end.saturating_sub(first));
            for index in first..end {
                let top = origin.y + heights.top(index);
                let line = &document.lines[index];
                painter.text(
                    egui::pos2(origin.x + gutter, top),
                    egui::Align2::RIGHT_TOP,
                    (index + 1).to_string(),
                    font.clone(),
                    palette.gutter,
                );
                let mut laid = job(line, &font, palette);
                if let Some(width) = wrap {
                    laid.wrap.max_width = width;
                }
                let galley = painter.layout_job(laid);
                heights.measured(index, galley.size().y.max(row));
                let at = egui::pos2(origin.x + text_left, top);
                if !state.wrap {
                    state.widest_seen.1 = state.widest_seen.1.max(galley.size().x);
                }
                let shown = &line.text[..line.shown];
                if let Some(range) = selected_in(state.selection, index, line.shown) {
                    let (from, to) = (char_cursor(shown, range.0), char_cursor(shown, range.1));
                    for marked in marked_rows(&galley, from.index.0, to.index.0, glyph) {
                        painter.rect_filled(marked.translate(at.to_vec2()), 0.0, palette.selection);
                    }
                }
                if line.truncated() {
                    let last = galley.rows.last().map_or(egui::Rect::ZERO, |r| r.rect());
                    painter.text(
                        at + egui::vec2(last.right() + glyph, last.top()),
                        egui::Align2::LEFT_TOP,
                        TRUNCATED,
                        font.clone(),
                        palette.gutter,
                    );
                }
                galleys.push((index, at, galley.clone()));
                painter.galley(at, galley, palette.text);
            }
            if heights.settle() {
                ui.ctx().request_repaint();
            }
            let heights = &state.heights;
            let place_at = |pos: egui::Pos2| {
                let line = heights.line_at(pos.y - origin.y);
                let Some(whole) = document.lines.get(line) else {
                    return Place { line, byte: 0 };
                };
                let text = &whole.text[..whole.shown];
                let chars = match galleys.iter().find(|(index, _, _)| *index == line) {
                    Some((_, at, galley)) => galley.cursor_from_pos(pos - *at).index.0,
                    None => {
                        let chars = ((pos.x - origin.x - text_left) / glyph).round().max(0.0);
                        chars as usize
                    }
                };
                let past_end = chars >= text.chars().count();
                // What is not painted of a truncated line is still in the file: reaching past
                // the painted end takes the line whole, as copying it does.
                let byte = if whole.truncated() && past_end {
                    whole.text.len()
                } else {
                    text.char_indices()
                        .nth(chars)
                        .map_or(text.len(), |(b, _)| b)
                };
                Place { line, byte }
            };
            let pointer = ui.input(|i| i.pointer.interact_pos());
            let visible = viewport.translate(content.min.to_vec2());
            let endpoint = pointer.map(|pos| {
                egui::pos2(
                    pos.x,
                    pos.y
                        .clamp(visible.top(), (visible.bottom() - 0.5).max(visible.top())),
                )
            });
            // A drag is only recognised once the pointer has travelled, so it is anchored where
            // the press happened, not where the pointer is by then.
            let pressed_at = ui.input(|i| i.pointer.press_origin());
            if response.drag_started()
                && let (Some(from), Some(pos)) = (pressed_at, endpoint)
            {
                state.selection = Some((place_at(from), place_at(pos)));
            } else if response.dragged()
                && let (Some(pos), Some((anchor, _))) = (endpoint, state.selection)
            {
                state.selection = Some((anchor, place_at(pos)));
            } else if response.clicked() {
                state.selection = None;
            }
            if response.dragged_by(egui::PointerButton::Primary)
                && state.selection.is_some()
                && let Some(pos) = pointer
            {
                super::selection_scroll::while_dragging(ui, visible, viewport, height, pos);
            }
        });
    ui.spacing_mut().scroll = previous_scroll;
    let copy = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
    if copy && let Some((from, to)) = state.selection.filter(|(a, b)| a != b) {
        ui.ctx().copy_text(document.text_between(from, to));
    }
}

/// The coloured layout of one line.
fn job(line: &Line, font: &FontId, palette: &Palette) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (n, (start, token)) in line.runs.iter().enumerate() {
        let end = line.runs.get(n + 1).map_or(line.shown, |(next, _)| *next);
        job.append(
            &line.text[*start..end.min(line.shown)],
            0.0,
            egui::TextFormat::simple(font.clone(), palette.of_token(*token)),
        );
    }
    job
}

/// Where characters `from..to` of a laid-out line sit, one stretch per row they cross. A marked
/// line with nothing of it marked (an empty line inside a selection) still shows half a character.
fn marked_rows(galley: &egui::Galley, from: usize, to: usize, glyph: f32) -> Vec<egui::Rect> {
    let mut out = Vec::new();
    let mut first = 0;
    let mut sliver = None;
    for placed in &galley.rows {
        let count = placed.char_count_excluding_newline().0;
        let (a, b) = (from.max(first), to.min(first + count));
        let x = |column: usize| placed.pos.x + placed.x_offset(CharIndex(column - first));
        let rect = placed.rect();
        if a < b {
            out.push(egui::Rect::from_x_y_ranges(x(a)..=x(b), rect.y_range()));
        } else if sliver.is_none() && (first..=first + count).contains(&from) {
            sliver = Some(egui::Rect::from_x_y_ranges(
                x(from)..=x(from) + glyph * 0.5,
                rect.y_range(),
            ));
        }
        first += count;
    }
    if out.is_empty() {
        out.extend(sliver);
    }
    out
}

/// The bytes of line `index` the selection covers, if any.
fn selected_in(
    selection: Option<(Place, Place)>,
    index: usize,
    len: usize,
) -> Option<(usize, usize)> {
    let (a, b) = selection?;
    let (from, to) = if (a.line, a.byte) <= (b.line, b.byte) {
        (a, b)
    } else {
        (b, a)
    };
    if from == to || index < from.line || index > to.line {
        return None;
    }
    let start = if index == from.line { from.byte } else { 0 };
    let end = if index == to.line { to.byte } else { len };
    (start < end || index != to.line).then_some((start.min(len), end.min(len)))
}

fn char_cursor(text: &str, byte: usize) -> egui::text::CCursor {
    egui::text::CCursor::new(text[..boundary(text, byte)].chars().count())
}

/// `byte` brought back inside `text` and onto the start of a character.
fn boundary(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

#[cfg(test)]
mod tests;
