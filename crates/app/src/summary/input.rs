//! Modal ownership begins before console dispatch and survives the closing frame.
use super::State;
use crate::{
    Window,
    keyboard::{Action, Chord},
    painter::CellMetrics,
};
use eframe::egui;
use std::time::Instant;

pub(crate) fn blocked(window: &Window, ctx: &egui::Context) -> bool {
    window.summary.enabled
        && (window.summary.open
            || window.summary.awaiting_release
            || window.summary.blocked_frame == Some(ctx.cumulative_frame_nr()))
}

pub(crate) fn toggle(window: &mut Window, ctx: &egui::Context) {
    if !window.summary.enabled || window.settings_editor.is_open() || window.pane_view.is_modal() {
        return;
    }
    let started = Instant::now();
    if window.summary.open {
        close(window, ctx);
    } else {
        let selected = window.active().focus;
        window.summary.open = true;
        window.summary.original = Some(selected);
        window.summary.selected = Some(selected);
        window.summary.ordinal = 0;
        window.summary.scroll_selected = true;
        window.summary.pressed = None;
        window.summary.activate = None;
        window.summary.order.clear();
        cancel_gestures(window, ctx);
        window.summary.blocked_frame = Some(ctx.cumulative_frame_nr());
    }
    window.summary.timing.record_work(started.elapsed());
    window.timed_separately += window.summary.timing.take_cost();
    ctx.request_repaint();
}

pub(super) fn close(window: &mut Window, ctx: &egui::Context) {
    if let Some(original) = window.summary.original.take()
        && let Some(owner) = window
            .tabs
            .iter()
            .position(|tab| tab.panes().iter().any(|pane| pane.name == original))
    {
        window.active = owner;
        window.tabs[owner].focus = original;
    }
    window.summary.open = false;
    window.summary.pressed = None;
    window.summary.activate = None;
    window.summary.painted.clear();
    window.summary.order.clear();
    window.summary.blocked_frame = Some(ctx.cumulative_frame_nr());
    window.summary.awaiting_release = ctx.input(|i| i.pointer.any_down());
    cancel_gestures(window, ctx);
    ctx.request_repaint();
}

pub(crate) fn cancel_gestures(window: &mut Window, ctx: &egui::Context) {
    window
        .header_input
        .block_for_settings(ctx.cumulative_frame_nr());
    window.drag_in_progress = None;
    window.link_press_in_progress = None;
    window.last_cell_reported = None;
    window.the_press_went_to_the_program = false;
    window.presses_for_the_program.clear();
    window.where_the_last_press_fell = None;
    egui::DragAndDrop::clear_payload(ctx);
    ctx.set_dragged_id(egui::Id::NULL);
    for tab in &mut window.tabs {
        for pane in tab.panes_mut() {
            pane.painted_console = None;
            pane.panel_state.nothing_was_drawn();
        }
    }
}

pub(crate) fn before_keys(window: &mut Window, ui: &mut egui::Ui, _metrics: &CellMetrics) -> bool {
    if !window.summary.enabled || window.settings_editor.is_open() || window.pane_view.is_modal() {
        return false;
    }
    let frame = ui.ctx().cumulative_frame_nr();
    if window.summary.input_frame == Some(frame) {
        return blocked(window, ui.ctx());
    }
    window.summary.input_frame = Some(frame);
    let scan_started = Instant::now();
    let toggle_pressed = ui.input(|i| {
        i.events.iter().any(|e| match e {
            egui::Event::Key {
                key,
                modifiers,
                pressed: true,
                repeat: false,
                ..
            } => {
                window.keymap.action_for(Chord {
                    ctrl: modifiers.ctrl,
                    shift: modifiers.shift,
                    alt: modifiers.alt,
                    key: *key,
                }) == Some(Action::Overview)
            }
            _ => false,
        })
    });
    window.summary.timing.record_work(scan_started.elapsed());
    window.timed_separately += window.summary.timing.take_cost();
    if !blocked(window, ui.ctx()) && !toggle_pressed {
        return false;
    }
    let was_open = window.summary.open;
    if toggle_pressed {
        toggle(window, ui.ctx());
    }
    let started = Instant::now();
    let events = ui.input(|i| i.events.clone());
    window.summary.blocked_frame = Some(ui.ctx().cumulative_frame_nr());
    window
        .header_input
        .block_for_settings(ui.ctx().cumulative_frame_nr());
    if window.summary.open && was_open && !toggle_pressed {
        for event in &events {
            match event {
                egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    repeat: false,
                    ..
                } => close(window, ui.ctx()),
                egui::Event::Key {
                    key, pressed: true, ..
                } if matches!(
                    key,
                    egui::Key::ArrowUp
                        | egui::Key::ArrowDown
                        | egui::Key::Home
                        | egui::Key::End
                        | egui::Key::Enter
                ) =>
                {
                    navigate(&mut window.summary, *key);
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } => pointer(&mut window.summary, *pos, *pressed),
                _ => {}
            }
            if !window.summary.open {
                break;
            }
        }
    }
    if !window.summary.open {
        window.summary.awaiting_release = ui.input(|i| i.pointer.any_down());
        // No closing gesture, clipboard event or dropped file reaches a console.
        ui.input_mut(|i| {
            i.events.retain(|e| {
                matches!(
                    e,
                    egui::Event::WindowFocused(_) | egui::Event::ModifiersChanged(_)
                )
            })
        });
    }
    window.summary.timing.record_work(started.elapsed());
    window.timed_separately += window.summary.timing.take_cost();
    true
}

fn navigate(state: &mut State, key: egui::Key) {
    if key == egui::Key::Enter {
        // Freeze what the key actually selected before asynchronous tree changes.
        state.activate = state.selected;
        return;
    }
    let index = state
        .order
        .iter()
        .position(|id| Some(*id) == state.selected)
        .unwrap_or(0);
    let next = match key {
        egui::Key::ArrowUp => index.saturating_sub(1),
        egui::Key::ArrowDown => (index + 1).min(state.order.len().saturating_sub(1)),
        egui::Key::Home => 0,
        egui::Key::End => state.order.len().saturating_sub(1),
        _ => return,
    };
    state.selected = state.order.get(next).copied();
    state.ordinal = next;
    state.scroll_selected = true;
}

fn pointer(state: &mut State, pos: egui::Pos2, pressed: bool) {
    if pressed {
        state.pressed = state.painted.iter().copied().find(|(_, r)| r.contains(pos));
        if let Some((id, _)) = state.pressed {
            state.selected = Some(id);
            state.ordinal = state
                .order
                .iter()
                .position(|name| *name == id)
                .unwrap_or(state.ordinal);
        }
    } else if let Some((id, rect)) = state.pressed.take()
        && rect.contains(pos)
    {
        state.activate = Some(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_keeps_the_pressed_identity_even_when_rows_reorder() {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        let mut state = State {
            painted: vec![(7, rect)],
            ..State::default()
        };
        pointer(&mut state, rect.center(), true);
        state.painted = vec![(8, rect)];
        pointer(&mut state, rect.center(), false);
        assert_eq!(state.activate, Some(7));
    }
    #[test]
    fn enter_freezes_the_selected_identity_before_removal_and_keys_are_ordered() {
        let mut state = State {
            order: vec![4, 8, 9],
            selected: Some(4),
            ..State::default()
        };
        navigate(&mut state, egui::Key::ArrowDown);
        navigate(&mut state, egui::Key::Enter);
        state.selected = Some(9);
        assert_eq!(state.activate, Some(8));
    }
}
