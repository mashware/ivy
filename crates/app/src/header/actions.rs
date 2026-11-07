//! Header gestures outlive their first frame: a toggle leaves the source widget alive.

use super::controls::{Action, Controls, Hit};
use super::menu::Menu;
use crate::{
    Window,
    painter::CellMetrics,
    pointer::MouseOutcome,
    tiling::{Orientation, can_split},
};
use eframe::egui;
use std::time::SystemTime;

#[derive(Default)]
pub(crate) struct Input {
    pub(super) menu: Option<Menu>,
    pub(super) held: Option<egui::PointerButton>,
    last_frame: Option<u64>,
    pub(super) blocked_frame: Option<u64>,
    pub(super) pending: Option<(usize, Action)>,
}

impl Input {
    pub(crate) fn block_for_settings(&mut self, frame: u64) {
        self.menu = None;
        self.pending = None;
        self.blocked_frame = Some(frame);
    }
}

pub(crate) fn enabled(
    window: &Window,
    name: usize,
    rect: egui::Rect,
    metrics: &CellMetrics,
) -> [bool; 3] {
    [
        can_split(rect, Orientation::Columns, metrics, window.headers),
        can_split(rect, Orientation::Rows, metrics, window.headers),
        window
            .active()
            .panes()
            .into_iter()
            .find(|pane| pane.name == name)
            .is_some_and(|pane| pane.panel_state.is_visible(SystemTime::now())),
    ]
}

/// Called inside the mouse stopwatch; splitting must not subtract its work a second time.
pub(crate) fn pointer(
    window: &mut Window,
    ui: &egui::Ui,
    rects: &[egui::Rect],
    metrics: &CellMetrics,
) -> Option<MouseOutcome> {
    if !window.headers.are_painted() {
        return None;
    }
    let frame = ui.ctx().cumulative_frame_nr();
    let quiet = || MouseOutcome {
        target: None,
        the_tree_changed: false,
    };
    if window.header_input.last_frame == Some(frame) {
        return (window.header_input.blocked_frame == Some(frame)).then(quiet);
    }
    window.header_input.last_frame = Some(frame);
    if let Some((name, action)) = window.header_input.pending.take() {
        window.header_input.blocked_frame = Some(frame);
        return Some(MouseOutcome {
            target: None,
            the_tree_changed: apply(window, name, action, ui.ctx(), metrics),
        });
    }
    let events = ui.ctx().input(|input| input.events.clone());
    if let Some(held) = window.header_input.held {
        if events.iter().any(|event| matches!(event, egui::Event::PointerButton {button, pressed:false,..} if *button == held))
                || !ui.ctx().input(|input| input.focused && input.pointer.button_down(held))
            {
                window.header_input.held = None;
            }
        window.header_input.blocked_frame = Some(frame);
        return Some(quiet());
    }
    if window.header_input.menu.is_some() {
        window.header_input.blocked_frame = Some(frame);
        let mut chosen = None;
        for event in &events {
            if let egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                ..
            } = event
            {
                window.header_input.held = ui
                    .ctx()
                    .input(|input| input.pointer.button_down(*button))
                    .then_some(*button);
                if *button == egui::PointerButton::Primary {
                    chosen = window
                        .header_input
                        .menu
                        .as_ref()
                        .and_then(|menu| menu.action_at(*pos));
                }
                window.header_input.menu = None;
                break;
            }
        }
        let changed =
            chosen.is_some_and(|(name, action)| apply(window, name, action, ui.ctx(), metrics));
        return Some(MouseOutcome {
            target: None,
            the_tree_changed: changed,
        });
    }
    for event in &events {
        let egui::Event::PointerButton {
            pos,
            button,
            pressed: true,
            modifiers,
        } = event
        else {
            continue;
        };
        if !modifiers.is_none() {
            continue;
        }
        let close = super::Button::in_a_tab_of(rects);
        let hit = window
            .active()
            .panes()
            .into_iter()
            .zip(rects)
            .find_map(|(pane, rect)| {
                let chrome = pane.chrome_within(*rect, metrics, SystemTime::now(), window.headers);
                if !chrome.strip_holds(*pos) {
                    return None;
                }
                let strip = chrome.header()?;
                let controls = Controls::within(
                    strip,
                    metrics,
                    super::button_within(strip, metrics, close).map(|button| button.rect()),
                );
                let hit = if *button == egui::PointerButton::Secondary {
                    Some(Hit::Menu)
                } else if *button == egui::PointerButton::Primary {
                    controls.hit(*pos)
                } else {
                    None
                };
                hit.map(|hit| (pane.name, *rect, strip, hit))
            });
        if let Some((name, rect, strip, hit)) = hit {
            window.header_input.held = ui
                .ctx()
                .input(|input| input.pointer.button_down(*button))
                .then_some(*button);
            // A quick click can contain both edges in one frame.
            if events.iter().any(|event| matches!(event, egui::Event::PointerButton {button:released, pressed:false,..} if released == button)) {
                    window.header_input.held = None;
                }
            window.header_input.blocked_frame = Some(frame);
            window.drag_in_progress = None;
            let changed = match hit {
                Hit::Action(action) if enabled(window, name, rect, metrics)[action.index()] => {
                    apply(window, name, action, ui.ctx(), metrics)
                }
                Hit::Action(_) => false,
                Hit::Menu => {
                    window.header_input.menu = Some(Menu::new(
                        name,
                        strip.left_bottom(),
                        ui.ctx().viewport_rect(),
                        metrics,
                        enabled(window, name, rect, metrics),
                        window.pane_called(name).is_some_and(|pane| pane.folded),
                    ));
                    false
                }
            };
            return Some(MouseOutcome {
                target: None,
                the_tree_changed: changed,
            });
        }
    }
    (window.header_input.blocked_frame == Some(frame)).then(quiet)
}

fn apply(
    window: &mut Window,
    name: usize,
    action: Action,
    ctx: &egui::Context,
    metrics: &CellMetrics,
) -> bool {
    let content = ctx.viewport_rect();
    let area = crate::tab_bar::geometry(window, content, metrics).consoles;
    let panes = window.active().panes();
    let Some(at) = panes.iter().position(|pane| pane.name == name) else {
        return false;
    };
    let rect = window.active().root.place(area, metrics.width()).parts[at];
    if !enabled(window, name, rect, metrics)[action.index()] {
        return false;
    }
    match action {
        Action::SplitRight | Action::SplitDown => {
            let before = window.active().panes().len();
            let direction = if action == Action::SplitRight {
                Orientation::Columns
            } else {
                Orientation::Rows
            };
            super::splitting::split(window, name, direction, ctx, content, metrics);
            window.active().panes().len() != before
        }
        Action::TogglePanel => {
            if let Some(pane) = window
                .active_mut()
                .panes_mut()
                .into_iter()
                .find(|pane| pane.name == name)
            {
                pane.folded = !pane.folded;
                true
            } else {
                false
            }
        }
    }
}
