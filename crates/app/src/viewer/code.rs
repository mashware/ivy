//! The Document tab's view of a file that is not Markdown: read-only, with line numbers and the
//! colours of [`super::highlight`].
use super::highlight::{self, Family, Line};
use super::palette::Palette;
use eframe::egui::{self, FontId, text::LayoutJob};
use std::path::Path;

const FONT_SIZE: f32 = 13.0;
/// Space between the line numbers and the text, and around the text.
const GUTTER_GAP: f32 = 14.0;
const PAD: f32 = 8.0;

/// A file laid out in lines and coloured, built on the loader thread.
pub(crate) struct CodeDocument {
    lines: Vec<Line>,
    /// The longest line in characters, which sets how far the view scrolls sideways.
    widest: usize,
}

impl CodeDocument {
    pub(super) fn parse(path: &Path, text: &str) -> Self {
        let first = text.lines().next().unwrap_or_default();
        let lines = highlight::scan(text, Family::of(path, first));
        let widest = lines
            .iter()
            .map(|line| line.text.chars().count())
            .max()
            .unwrap_or(0);
        Self { lines, widest }
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
                from.byte.min(text.len())
            } else {
                0
            };
            let end = if line == to.line {
                to.byte.min(text.len())
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

/// A place in the file: a line and a byte inside it, always on a character boundary.
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
}

impl CodeState {
    /// A new file starts with nothing marked.
    pub(super) fn reset(&mut self) {
        self.selection = None;
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
    let width = text_left + document.widest as f32 * glyph + 2.0 * PAD;
    let height = document.lines.len() as f32 * row + 2.0 * PAD;
    let select_all = ui.input(|i| {
        i.events.iter().any(|event| {
            matches!(event, egui::Event::Key { key: egui::Key::A, pressed: true, modifiers, .. }
                if modifiers.command)
        })
    });
    if select_all {
        state.selection = Some((Place::default(), document.end()));
    }
    egui::ScrollArea::both()
        .id_salt("code-content")
        .auto_shrink([false; 2])
        .show_viewport(ui, |ui, viewport| {
            let (content, response) = ui.allocate_exact_size(
                egui::vec2(width.max(ui.available_width()), height),
                egui::Sense::click_and_drag(),
            );
            let origin = content.min + egui::vec2(0.0, PAD);
            let first = ((viewport.min.y - PAD) / row).floor().max(0.0) as usize;
            let end = (((viewport.max.y - PAD) / row).ceil().max(0.0) as usize + 1)
                .min(document.lines.len());
            let painter = ui.painter_at(ui.clip_rect());
            let mut galleys = Vec::with_capacity(end.saturating_sub(first));
            for index in first..end {
                let top = origin.y + index as f32 * row;
                let line = &document.lines[index];
                painter.text(
                    egui::pos2(origin.x + gutter, top),
                    egui::Align2::RIGHT_TOP,
                    (index + 1).to_string(),
                    font.clone(),
                    palette.gutter,
                );
                let galley = painter.layout_job(job(line, &font, palette));
                let at = egui::pos2(origin.x + text_left, top);
                if let Some(range) = selected_in(state.selection, index, line.text.len()) {
                    let x =
                        |byte: usize| galley.pos_from_cursor(char_cursor(&line.text, byte)).min.x;
                    let (left, right) = (x(range.0), x(range.1).max(x(range.0) + glyph * 0.5));
                    let marked = egui::Rect::from_min_max(
                        egui::pos2(at.x + left, top),
                        egui::pos2(at.x + right, top + row),
                    );
                    painter.rect_filled(marked, 0.0, palette.selection);
                }
                if line.truncated {
                    painter.text(
                        at + egui::vec2(galley.size().x + glyph, 0.0),
                        egui::Align2::LEFT_TOP,
                        "… line truncated",
                        font.clone(),
                        palette.gutter,
                    );
                }
                galleys.push((index, at, galley.clone()));
                painter.galley(at, galley, palette.text);
            }
            let place_at = |pos: egui::Pos2| {
                let line = (((pos.y - origin.y) / row).floor().max(0.0) as usize)
                    .min(document.lines.len().saturating_sub(1));
                let text = document.lines.get(line).map_or("", |l| l.text.as_str());
                let byte = match galleys.iter().find(|(index, _, _)| *index == line) {
                    Some((_, at, galley)) => {
                        let chars = galley.cursor_from_pos(pos - *at).index.0;
                        text.char_indices()
                            .nth(chars)
                            .map_or(text.len(), |(b, _)| b)
                    }
                    None => {
                        let chars = ((pos.x - origin.x - text_left) / glyph).round().max(0.0);
                        text.char_indices()
                            .nth(chars as usize)
                            .map_or(text.len(), |(b, _)| b)
                    }
                };
                Place { line, byte }
            };
            let pointer = ui.input(|i| i.pointer.interact_pos());
            // A drag is only recognised once the pointer has travelled, so it is anchored where
            // the press happened, not where the pointer is by then.
            let pressed_at = ui.input(|i| i.pointer.press_origin());
            if response.drag_started()
                && let (Some(from), Some(pos)) = (pressed_at, pointer)
            {
                state.selection = Some((place_at(from), place_at(pos)));
            } else if response.dragged()
                && let (Some(pos), Some((anchor, _))) = (pointer, state.selection)
            {
                state.selection = Some((anchor, place_at(pos)));
            } else if response.clicked() {
                state.selection = None;
            }
        });
    let copy = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
    if copy && let Some((from, to)) = state.selection.filter(|(a, b)| a != b) {
        ui.ctx().copy_text(document.text_between(from, to));
    }
}

/// The coloured layout of one line.
fn job(line: &Line, font: &FontId, palette: &Palette) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (n, (start, token)) in line.runs.iter().enumerate() {
        let end = line
            .runs
            .get(n + 1)
            .map_or(line.text.len(), |(next, _)| *next);
        job.append(
            &line.text[*start..end],
            0.0,
            egui::TextFormat::simple(font.clone(), palette.of_token(*token)),
        );
    }
    job
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
    egui::text::CCursor::new(text[..byte.min(text.len())].chars().count())
}

#[cfg(test)]
mod tests;
