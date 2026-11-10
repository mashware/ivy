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
    window.board.enabled
        && (window.board.open
            || window.board.awaiting_release
            || window.board.blocked_frame == Some(ctx.cumulative_frame_nr()))
}

pub(crate) fn toggle(window: &mut Window, ctx: &egui::Context) {
    if !window.board.enabled || window.settings_editor.is_open() || window.pane_view.is_modal() {
        return;
    }
    let started = Instant::now();
    if window.board.open {
        close(window, ctx);
    } else {
        let selected = window.active().focus;
        window.board.open = true;
        window.board.original = Some(selected);
        window.board.selected = Some(selected);
        window.board.ordinal = 0;
        window.board.scroll_selected = true;
        window.board.pressed = None;
        window.board.activate = None;
        window.board.order.clear();
        window.board.viewport = None;
        cancel_gestures(window, ctx);
        window.board.blocked_frame = Some(ctx.cumulative_frame_nr());
    }
    window.board.timing.record_work(started.elapsed());
    window.timed_separately += window.board.timing.take_cost();
    ctx.request_repaint();
}

pub(super) fn close(window: &mut Window, ctx: &egui::Context) {
    if let Some(original) = window.board.original.take()
        && let Some(owner) = window
            .tabs
            .iter()
            .position(|tab| tab.panes().iter().any(|pane| pane.name == original))
    {
        window.active = owner;
        window.tabs[owner].focus = original;
    }
    window.board.open = false;
    window.board.pressed = None;
    window.board.activate = None;
    window.board.painted.clear();
    window.board.order.clear();
    window.board.blocked_frame = Some(ctx.cumulative_frame_nr());
    window.board.awaiting_release = ctx.input(|i| i.pointer.any_down());
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
    if !window.board.enabled || window.settings_editor.is_open() || window.pane_view.is_modal() {
        return false;
    }
    let frame = ui.ctx().cumulative_frame_nr();
    if window.board.input_frame == Some(frame) {
        return blocked(window, ui.ctx());
    }
    window.board.input_frame = Some(frame);
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
                }) == Some(Action::Board)
            }
            _ => false,
        })
    });
    window.board.timing.record_work(scan_started.elapsed());
    window.timed_separately += window.board.timing.take_cost();
    if !blocked(window, ui.ctx()) && !toggle_pressed {
        return false;
    }
    let was_open = window.board.open;
    if toggle_pressed {
        toggle(window, ui.ctx());
    }
    let started = Instant::now();
    let events = ui.input(|i| i.events.clone());
    window.board.blocked_frame = Some(ui.ctx().cumulative_frame_nr());
    window
        .header_input
        .block_for_settings(ui.ctx().cumulative_frame_nr());
    if window.board.open && was_open && !toggle_pressed {
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
                        | egui::Key::ArrowLeft
                        | egui::Key::ArrowRight
                        | egui::Key::Home
                        | egui::Key::End
                        | egui::Key::Enter
                ) =>
                {
                    navigate(&mut window.board, *key);
                }
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } => pointer(&mut window.board, *pos, *pressed),
                _ => {}
            }
            if !window.board.open {
                break;
            }
        }
    }
    if !window.board.open {
        window.board.awaiting_release = ui.input(|i| i.pointer.any_down());
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
    window.board.timing.record_work(started.elapsed());
    window.timed_separately += window.board.timing.take_cost();
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
        .position(|slot| Some(slot.pane) == state.selected)
        .unwrap_or(0);
    let last = state.order.len().saturating_sub(1);
    let next = match key {
        egui::Key::ArrowUp => super::layout::vertical(&state.order, index, false),
        egui::Key::ArrowDown => super::layout::vertical(&state.order, index, true),
        egui::Key::ArrowLeft => index.saturating_sub(1),
        egui::Key::ArrowRight => (index + 1).min(last),
        egui::Key::Home => 0,
        egui::Key::End => last,
        _ => return,
    };
    state.selected = state.order.get(next).map(|slot| slot.pane);
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
                .position(|slot| slot.pane == id)
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
    use crate::board::layout::Slot;
    /// Cards as a painting leaves them: `(pane, row, column)`.
    fn slots(cards: &[(usize, usize, usize)]) -> Vec<Slot> {
        cards
            .iter()
            .map(|&(pane, row, column)| Slot { pane, row, column })
            .collect()
    }
    #[test]
    fn release_keeps_the_pressed_identity_even_when_cards_reorder() {
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
            order: slots(&[(4, 0, 0), (8, 1, 0), (9, 2, 0)]),
            selected: Some(4),
            ..State::default()
        };
        navigate(&mut state, egui::Key::ArrowDown);
        navigate(&mut state, egui::Key::Enter);
        state.selected = Some(9);
        assert_eq!(state.activate, Some(8));
    }
    #[test]
    fn left_and_right_follow_reading_order_and_up_and_down_follow_the_grid() {
        let mut state = State {
            order: slots(&[(4, 0, 0), (8, 0, 1), (9, 0, 2), (2, 1, 0)]),
            selected: Some(4),
            ..State::default()
        };
        navigate(&mut state, egui::Key::ArrowDown);
        assert_eq!(
            state.selected,
            Some(2),
            "one grid row down, where reading order says 8"
        );
        navigate(&mut state, egui::Key::ArrowUp);
        assert_eq!(state.selected, Some(4), "same column, one row up");
        navigate(&mut state, egui::Key::ArrowLeft);
        assert_eq!(state.selected, Some(4), "nothing before the first card");
        state.selected = Some(9);
        navigate(&mut state, egui::Key::ArrowRight);
        assert_eq!(
            state.selected,
            Some(2),
            "reading order crosses into the next row"
        );
        state.selected = Some(9);
        navigate(&mut state, egui::Key::ArrowDown);
        assert_eq!(state.selected, Some(2), "clamped to the shorter row");
    }
}
