use super::Block;
use eframe::egui::{
    self,
    text::CCursor,
    text_selection::{CCursorRange, TextCursorState},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Position {
    block: usize,
    column: usize,
    character: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Range {
    anchor: Position,
    end: Position,
}

impl Range {
    fn ordered(self) -> (Position, Position) {
        (self.anchor.min(self.end), self.anchor.max(self.end))
    }
}

struct Fragment {
    id: egui::Id,
    block: usize,
    column: usize,
    origin: egui::Pos2,
    rect: egui::Rect,
    galley: Arc<egui::Galley>,
    painter: egui::Painter,
}

pub(super) struct Frame {
    visible: egui::Rect,
    response: egui::Response,
    fragments: Vec<Fragment>,
}

impl Frame {
    pub(super) fn new(ui: &egui::Ui, visible: egui::Rect) -> Self {
        let response = ui.interact(
            visible,
            ui.id().with("document-selection"),
            egui::Sense::click_and_drag() - egui::Sense::FOCUSABLE,
        );
        Self {
            visible,
            response,
            fragments: Vec::new(),
        }
    }

    pub(super) fn label(
        &mut self,
        ui: &mut egui::Ui,
        block: usize,
        column: usize,
        label: egui::Label,
    ) {
        let (origin, galley, response) = label.selectable(false).layout_in_ui(ui);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), galley.text())
        });
        if ui.is_rect_visible(response.rect) {
            self.fragments.push(Fragment {
                id: response.id,
                block,
                column,
                origin,
                rect: galley.rect.translate(origin.to_vec2()),
                galley,
                painter: ui.painter().clone(),
            });
        }
    }

    fn position(&self, pointer: egui::Pos2) -> Option<Position> {
        let distance =
            |range: egui::Rangef, point: f32| (point - point.clamp(range.min, range.max)).abs();
        let fragment = self.fragments.iter().min_by(|a, b| {
            distance(a.rect.y_range(), pointer.y)
                .total_cmp(&distance(b.rect.y_range(), pointer.y))
                .then_with(|| {
                    distance(a.rect.x_range(), pointer.x)
                        .total_cmp(&distance(b.rect.x_range(), pointer.x))
                })
        })?;
        Some(fragment.position(fragment.cursor_at(pointer)))
    }

    fn click(&self, ui: &egui::Ui, pointer: egui::Pos2, selection: &mut Option<Range>) {
        let Some(fragment) = self
            .fragments
            .iter()
            .find(|frag| frag.rect.contains(pointer))
        else {
            *selection = None;
            return;
        };
        let cursor = fragment.cursor_at(pointer);
        let end = fragment.position(cursor);
        let mut native = TextCursorState::default();
        native.pointer_interaction(ui, &self.response, cursor, &fragment.galley, false);
        if let Some(range) = native.char_range() {
            *selection = Some(Range {
                anchor: fragment.position(range.secondary),
                end: fragment.position(range.primary),
            });
        } else if ui.input(|input| input.modifiers.shift)
            && let Some(range) = selection.as_mut()
        {
            range.end = end;
        } else {
            *selection = Some(Range { anchor: end, end });
        }
    }

    pub(super) fn finish(
        self,
        ui: &egui::Ui,
        viewport: egui::Rect,
        height: f32,
        selection: &mut Option<Range>,
        blocks: &[Block],
    ) {
        let pointer = ui.input(|input| input.pointer.interact_pos());
        let dragging = self.response.dragged_by(egui::PointerButton::Primary);
        let released = self.response.drag_stopped_by(egui::PointerButton::Primary);
        if (dragging || released)
            && let Some(pointer) = pointer
        {
            let endpoint = egui::pos2(
                pointer.x,
                pointer.y.clamp(self.visible.top(), self.visible.bottom()),
            );
            if self.response.drag_started() {
                let origin = ui.input(|input| input.pointer.press_origin());
                *selection = origin.and_then(|origin| {
                    Some(Range {
                        anchor: if ui.input(|input| input.modifiers.shift) {
                            selection
                                .map(|range| range.anchor)
                                .or_else(|| self.position(origin))?
                        } else {
                            self.position(origin)?
                        },
                        end: self.position(endpoint)?,
                    })
                });
            } else if let (Some(range), Some(end)) = (selection.as_mut(), self.position(endpoint)) {
                range.end = end;
            }
            if dragging && selection.is_some() {
                let edge = self
                    .visible
                    .shrink2(egui::vec2(0.0, 8.0_f32.min(self.visible.height() / 2.0)));
                super::super::selection_scroll::while_dragging(ui, edge, viewport, height, pointer);
            }
        } else if self.response.clicked()
            && let Some(pointer) = pointer
        {
            self.click(ui, pointer, selection);
        } else if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            *selection = None;
        }
        self.response
            .on_hover_and_drag_cursor(egui::CursorIcon::Text);
        if let Some(range) = selection.as_mut() {
            if let Some(fragment) = self.fragments.iter().find(|frag| frag.contains(range.end)) {
                fragment.apply_keys(ui, range);
            }
            if ui.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy))
            }) {
                let text = copy(blocks, *range);
                if !text.is_empty() {
                    ui.copy_text(text);
                }
            }
        }
        for fragment in self.fragments {
            fragment.paint(ui.visuals(), *selection);
        }
    }
}

impl Fragment {
    fn cursor_at(&self, pointer: egui::Pos2) -> CCursor {
        self.galley.cursor_from_pos(pointer - self.origin)
    }

    fn contains(&self, position: Position) -> bool {
        (self.block, self.column) == (position.block, position.column)
    }

    fn position(&self, cursor: CCursor) -> Position {
        Position {
            block: self.block,
            column: self.column,
            character: cursor.index.0,
        }
    }

    fn apply_keys(&self, ui: &egui::Ui, range: &mut Range) {
        let secondary = if self.contains(range.anchor) {
            CCursor::new(range.anchor.character)
        } else if range.anchor < range.end {
            self.galley.begin()
        } else {
            self.galley.end()
        };
        let mut native = CCursorRange {
            primary: CCursor::new(range.end.character),
            secondary,
            h_pos: None,
        };
        let os = ui.ctx().os();
        let changed = ui.input(|input| {
            let mut changed = false;
            for event in &input.events {
                changed |= native.on_event(os, event, &self.galley, self.id);
            }
            changed
        });
        if changed {
            if native.secondary != secondary {
                range.anchor = self.position(native.secondary);
            }
            range.end = self.position(native.primary);
        }
    }

    fn paint(mut self, visuals: &egui::Visuals, selection: Option<Range>) {
        if let Some(selection) = selection {
            let (from, to) = selection.ordered();
            if let Some((start, end)) = fragment_range(from, to, self.block, self.column) {
                let len = self.galley.end().index.0;
                let range =
                    CCursorRange::two(CCursor::new(start.min(len)), CCursor::new(end.min(len)));
                egui::text_selection::visuals::paint_text_selection(
                    &mut self.galley,
                    visuals,
                    &range,
                    None,
                );
            }
        }
        self.painter
            .galley(self.origin, self.galley, visuals.text_color());
    }
}

fn fragment_range(
    from: Position,
    to: Position,
    block: usize,
    column: usize,
) -> Option<(usize, usize)> {
    let place = (block, column);
    let (first, last) = ((from.block, from.column), (to.block, to.column));
    if place < first || place > last {
        return None;
    }
    let start = if place == first { from.character } else { 0 };
    let end = if place == last {
        to.character
    } else {
        usize::MAX
    };
    Some((start, end))
}

fn copy(blocks: &[Block], selection: Range) -> String {
    let (mut from, mut to) = selection.ordered();
    if from == to {
        from.character = 0;
        to.character = usize::MAX;
    }
    let mut text = String::new();
    let mut previous = None;
    for (block, content) in blocks
        .iter()
        .enumerate()
        .take(to.block + 1)
        .skip(from.block)
    {
        let cells = match content {
            Block::Line { runs, .. } => std::slice::from_ref(runs),
            Block::Table { cells, .. } => cells.as_slice(),
        };
        for (column, runs) in cells.iter().enumerate() {
            let Some((start, end)) = fragment_range(from, to, block, column) else {
                continue;
            };
            let mut characters = runs
                .iter()
                .flat_map(|run| run.text.chars())
                .skip(start)
                .take(end - start)
                .peekable();
            let Some(first) = characters.peek() else {
                continue;
            };
            if let Some(previous) = previous {
                if previous != block {
                    text.push('\n');
                    if blank_line_between(&blocks[previous], content) {
                        text.push('\n');
                    }
                } else if !text.ends_with(char::is_whitespace)
                    && !first.is_whitespace()
                    && !first.is_ascii_punctuation()
                {
                    text.push(' ');
                }
            }
            text.extend(characters);
            previous = Some(block);
        }
    }
    text
}

fn blank_line_between(previous: &Block, next: &Block) -> bool {
    // Reconstruct paragraph gaps without requiring the adjacent labels to be visible.
    let next_has_heading_gap =
        matches!(next, Block::Line { heading: Some(level), .. } if *level > 1);
    next_has_heading_gap
        || match previous {
            Block::Line { code: true, .. } => !matches!(next, Block::Line { code: true, .. }),
            Block::Line { runs, heading, .. } => super::block_spacing(*heading, false, runs) > 8.0,
            Block::Table { .. } => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_scroll_markdown_copy_keeps_paragraph_and_table_boundaries() {
        let document = super::super::MarkdownDocument::parse(
            "éclair **one**\n\nParagraph two\n\n| A | B |\n|---|---|\n| x | y |\n",
        )
        .unwrap();
        let from = Position {
            block: 0,
            column: 0,
            character: 1,
        };
        let to = Position {
            block: 3,
            column: 1,
            character: 1,
        };
        assert_eq!(
            copy(
                &document.blocks,
                Range {
                    anchor: to,
                    end: from
                }
            ),
            "clair one\n\nParagraph two\n\nA B\nx y",
        );
    }
}
