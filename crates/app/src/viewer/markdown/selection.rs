use super::{Block, Run};
use eframe::egui::{self, text::CCursor, text_selection::CCursorRange};
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
        Some(Position {
            block: fragment.block,
            column: fragment.column,
            character: fragment
                .galley
                .cursor_from_pos(pointer - fragment.origin)
                .index
                .0,
        })
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
        if self.response.dragged_by(egui::PointerButton::Primary)
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
                        anchor: self.position(origin)?,
                        end: self.position(endpoint)?,
                    })
                });
            } else if let (Some(range), Some(end)) = (selection.as_mut(), self.position(endpoint)) {
                range.end = end;
            }
            if selection.is_some() {
                let edge = self
                    .visible
                    .shrink2(egui::vec2(0.0, 8.0_f32.min(self.visible.height() / 2.0)));
                super::super::selection_scroll::while_dragging(ui, edge, viewport, height, pointer);
            }
        } else if self.response.clicked() || ui.input(|input| input.key_pressed(egui::Key::Escape))
        {
            *selection = None;
        }
        self.response
            .on_hover_and_drag_cursor(egui::CursorIcon::Text);
        if let Some(range) = *selection
            && range.anchor != range.end
            && ui.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy))
            })
        {
            ui.copy_text(copy(blocks, range));
        }
        for fragment in self.fragments {
            fragment.paint(ui.visuals(), *selection);
        }
    }
}

impl Fragment {
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
    let (from, to) = selection.ordered();
    let mut text = String::new();
    for (block, content) in blocks
        .iter()
        .enumerate()
        .take(to.block + 1)
        .skip(from.block)
    {
        if block != from.block {
            text.push('\n');
        }
        let cells = match content {
            Block::Line { runs, .. } => std::slice::from_ref(runs),
            Block::Table { cells, .. } => cells.as_slice(),
        };
        for (column, runs) in cells.iter().enumerate() {
            let Some((start, end)) = fragment_range(from, to, block, column) else {
                continue;
            };
            if column > 0 && (block != from.block || column > from.column) {
                text.push('\t');
            }
            text.extend(
                runs.iter()
                    .flat_map(|run: &Run| run.text.chars())
                    .skip(start)
                    .take(end - start),
            );
        }
    }
    text
}
