//! Compact controls retain pane identity while the keyboard focus stays elsewhere.

use super::controls::Action;
use crate::{painter::CellMetrics, theme::Theme};
use eframe::egui;
use std::time::Instant;

pub(super) struct Menu {
    owner: usize,
    rect: egui::Rect,
    enabled: [bool; 3],
    folded: bool,
    selected: usize,
}

impl Menu {
    pub(super) fn new(
        owner: usize,
        at: egui::Pos2,
        viewport: egui::Rect,
        metrics: &CellMetrics,
        enabled: [bool; 3],
        folded: bool,
    ) -> Self {
        let size = egui::vec2(
            (32.0 * metrics.width()).min(viewport.width()),
            (3.0 * (metrics.height() + 10.0)).min(viewport.height()),
        );
        let at = egui::pos2(
            at.x.max(viewport.left()).min(viewport.right() - size.x),
            at.y.max(viewport.top()).min(viewport.bottom() - size.y),
        );
        Self {
            owner,
            rect: egui::Rect::from_min_size(at, size),
            enabled,
            folded,
            selected: 0,
        }
    }

    pub(super) fn owner(&self) -> usize {
        self.owner
    }

    fn row(&self, index: usize) -> egui::Rect {
        let height = self.rect.height() / 3.0;
        egui::Rect::from_min_size(
            self.rect.min + egui::vec2(0.0, index as f32 * height),
            egui::vec2(self.rect.width(), height),
        )
    }

    pub(super) fn action_at(&self, at: egui::Pos2) -> Option<(usize, Action)> {
        Action::ALL
            .into_iter()
            .enumerate()
            .find_map(|(index, action)| {
                let row = self.row(index);
                (self.enabled[index]
                    && row.contains(at)
                    && at.y < row.bottom()
                    && at.x < row.right())
                .then_some((self.owner, action))
            })
    }

    fn hover(&mut self, at: egui::Pos2) {
        if let Some(index) = (0..3).find(|&index| {
            let row = self.row(index);
            row.contains(at) && at.y < row.bottom() && at.x < row.right()
        }) {
            self.selected = index;
        }
    }

    pub(super) fn step(&mut self, delta: isize) {
        self.selected = (self.selected as isize + delta).rem_euclid(3) as usize;
    }

    pub(super) fn selected(&self) -> Option<(usize, Action)> {
        self.enabled[self.selected].then_some((self.owner, Action::ALL[self.selected]))
    }

    pub(super) fn paint(&self, ctx: &egui::Context, metrics: &CellMetrics, theme: &Theme) {
        let painter = ctx
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("pane-actions"),
            ))
            .with_clip_rect(ctx.viewport_rect());
        painter.rect_filled(self.rect, 2.0, theme.panel_background());
        painter.rect_stroke(
            self.rect,
            2.0,
            egui::Stroke::new(1.0, theme.divider()),
            egui::StrokeKind::Inside,
        );
        for (index, action) in Action::ALL.into_iter().enumerate() {
            let row = self.row(index);
            if self.enabled[index] && index == self.selected {
                painter.rect_filled(row.shrink(1.0), 0.0, theme.focus().gamma_multiply(0.25));
            }
            let text = if self.enabled[index] {
                action.label(self.folded).to_owned()
            } else if action == Action::TogglePanel {
                "No information panel to show".to_owned()
            } else {
                format!("{} (not enough room)", action.label(self.folded))
            };
            painter.text(
                row.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                text,
                egui::FontId::monospace(metrics.height() * 0.75),
                if self.enabled[index] {
                    theme.foreground()
                } else {
                    theme.divider()
                },
            );
        }
    }
}

/// The menu gets navigation before a console sees the same keys.
pub(crate) fn keys(window: &mut crate::Window, ctx: &egui::Context) -> bool {
    if window.header_input.menu.is_none() {
        return false;
    }
    let started = Instant::now();
    let mut menu = window
        .header_input
        .menu
        .take()
        .expect("the menu was just checked");
    let mut close = false;
    ctx.input_mut(|input| {
        input.events.retain(|event| match event {
            egui::Event::ModifiersChanged(modifiers) => {
                window.modifiers = *modifiers;
                true
            }
            egui::Event::WindowFocused(false) => {
                window.modifiers = egui::Modifiers::NONE;
                close = true;
                true
            }
            egui::Event::PointerMoved(at) => {
                menu.hover(*at);
                true
            }
            egui::Event::Key {
                key, pressed: true, ..
            } => {
                match key {
                    egui::Key::Escape => close = true,
                    egui::Key::ArrowDown => menu.step(1),
                    egui::Key::ArrowUp => menu.step(-1),
                    egui::Key::Enter => {
                        window.header_input.pending = menu.selected();
                        close = true;
                    }
                    _ => {}
                }
                false
            }
            egui::Event::Key { .. }
            | egui::Event::Text(_)
            | egui::Event::Paste(_)
            | egui::Event::Copy
            | egui::Event::Cut => false,
            _ => true,
        })
    });
    if !close {
        window.header_input.menu = Some(menu);
    }
    window.header_input.blocked_frame = Some(ctx.cumulative_frame_nr());
    window.timed_separately += started.elapsed();
    true
}

/// Owning a gesture must not dim the consoles; preserve any opacity already in effect.
pub(crate) fn block_pointer(ui: &mut egui::Ui, overlay_owned: bool) {
    let opacity = ui.opacity();
    ui.disable();
    if !overlay_owned {
        ui.set_opacity(opacity);
    }
}

pub(crate) fn pointer_blocked(window: &crate::Window, ctx: &egui::Context) -> bool {
    window.header_input.menu.is_some()
        || window.header_input.held.is_some()
        || window.header_input.blocked_frame == Some(ctx.cumulative_frame_nr())
}

pub(crate) fn paint(window: &mut crate::Window, ctx: &egui::Context, metrics: &CellMetrics) {
    if let Some(menu) = &window.header_input.menu {
        if !window
            .active()
            .panes()
            .iter()
            .any(|pane| pane.name == menu.owner())
        {
            window.header_input.menu = None;
            return;
        }
        let started = Instant::now();
        menu.paint(ctx, metrics, &window.theme);
        window
            .strips
            .a_strip_was_painted(&mut window.timed_separately, Some(started.elapsed()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_blocking_preserves_existing_transparency_and_disables_interaction() {
        fn rendered_colors(mode: u8) -> Vec<egui::Color32> {
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.set_opacity(0.7);
                if mode == 1 || mode == 2 {
                    block_pointer(ui, mode == 2);
                    assert!(
                        !ui.is_enabled(),
                        "pointer blocking left interaction enabled"
                    );
                }
                if mode == 3 {
                    ui.disable();
                }
                ui.painter().text(
                    egui::pos2(20.0, 20.0),
                    egui::Align2::LEFT_TOP,
                    "Translucent console",
                    egui::FontId::monospace(14.0),
                    egui::Color32::WHITE,
                );
            });
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(
                        text.galley
                            .rows
                            .iter()
                            .flat_map(|row| {
                                row.row
                                    .visuals
                                    .mesh
                                    .vertices
                                    .iter()
                                    .map(|vertex| vertex.color)
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .expect("console text is painted")
        }
        let baseline = rendered_colors(0);
        assert!(!baseline.is_empty());
        assert!(baseline.iter().all(|color| color.a() < 255));
        assert_eq!(rendered_colors(1), baseline);
        assert_eq!(rendered_colors(2), rendered_colors(3));
    }

    #[test]
    fn hovering_the_shared_row_edge_selects_the_same_action_as_clicking() {
        let mut menu = Menu::new(
            7,
            egui::pos2(100.0, 50.0),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 300.0)),
            &CellMetrics::of(8.0, 16.0),
            [true; 3],
            false,
        );
        let boundary = egui::pos2(120.0, 76.0);
        menu.hover(boundary);
        assert_eq!(menu.selected(), Some((7, Action::SplitDown)));
        assert_eq!(menu.action_at(boundary), Some((7, Action::SplitDown)));
    }
}
