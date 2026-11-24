//! Board owns only its dock gestures and keyboard focus, never the console surface.
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
        && (window.board.awaiting_release
            || window.board.blocked_frame == Some(ctx.cumulative_frame_nr()))
}

pub(crate) fn toggle(window: &mut Window, ctx: &egui::Context) {
    if !window.board.enabled || window.settings_editor.is_open() || window.pane_view.is_modal() {
        return;
    }
    let started = Instant::now();
    if window.board.open {
        crate::sidebar::close(window, ctx);
    } else {
        crate::sidebar::select(window, crate::sidebar::Tool::Board);
    }
    window.board.timing.record_work(started.elapsed());
    window.timed_separately += window.board.timing.take_cost();
    ctx.request_repaint();
}

fn dismiss(window: &mut Window, ctx: &egui::Context) {
    if let Some(original) = window.board.original
        && let Some(owner) = window
            .tabs
            .iter()
            .position(|tab| tab.panes().iter().any(|pane| pane.name == original))
    {
        window.active = owner;
        window.tabs[owner].focus = original;
    }
    crate::sidebar::close(window, ctx);
    window.board.blocked_frame = Some(ctx.cumulative_frame_nr());
    window.board.awaiting_release = ctx.input(|i| i.pointer.any_down());
    cancel_gestures(window, ctx);
    ctx.request_repaint();
}

pub(crate) fn dismiss_for_chrome(window: &mut Window, ctx: &egui::Context) {
    dismiss(window, ctx);
}

pub(crate) fn reanchor(window: &mut Window) {
    if window.board.open {
        window.board.original = Some(window.active().focus);
    }
}

pub(crate) fn cancel_gestures(window: &mut Window, ctx: &egui::Context) {
    if window.board.pointer_owned {
        window.board.awaiting_release = true;
        window.board.pointer_owned = false;
    }
    window.board.keyboard_focus = false;
    window.board.filter.editing = false;
    window.board.surface.pressed = None;
    window.board.surface.child_pressed = None;
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
    ctx.stop_dragging();
    for tab in &mut window.tabs {
        for pane in tab.panes_mut() {
            pane.painted_console = None;
            pane.panel_state.nothing_was_drawn();
        }
    }
}

pub(crate) fn before_keys(
    window: &mut Window,
    ui: &mut egui::Ui,
    metrics: &CellMetrics,
    chrome_owned: bool,
) -> bool {
    if !window.board.enabled || window.settings_editor.is_open() || window.pane_view.is_modal() {
        return false;
    }
    if chrome_owned {
        window.board.surface.pressed = None;
        window.board.surface.child_pressed = None;
        return blocked(window, ui.ctx());
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
    if toggle_pressed {
        toggle(window, ui.ctx());
        // The shortcut's key event must not reach the console on either transition.
        ui.input_mut(|i| {
            i.events
                .retain(|event| !matches!(event, egui::Event::Key { .. }))
        });
        return true;
    }
    if window.board.awaiting_release {
        let release = ui.input(|i| {
            i.events
                .iter()
                .any(|event| matches!(event, egui::Event::PointerButton { pressed: false, .. }))
        });
        if ui.input(|i| i.pointer.any_down()) {
            return true;
        }
        window.board.awaiting_release = false;
        if release {
            return true;
        }
    }
    if blocked(window, ui.ctx()) {
        return true;
    }
    if !window.board.open {
        return false;
    }

    let geometry = crate::tab_bar::geometry(window, ui.max_rect(), metrics);
    let Some(dock) = geometry.dock else {
        window.board.keyboard_focus = false;
        return false;
    };
    let started = Instant::now();
    let events = ui.input(|i| i.events.clone());
    let arrangement_before = window.board.arrangement;
    let mut owns_events = false;
    for event in &events {
        if window.board.keyboard_focus && super::filter::edit(&mut window.board.filter, event) {
            owns_events = true;
            continue;
        }
        match event {
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                repeat: false,
                ..
            } if window.board.keyboard_focus
                && window.board.filter.editing
                && window.board.arrangement == super::filter::Arrangement::ByState =>
            {
                window.board.filter.editing = false;
                owns_events = true;
            }
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                repeat: false,
                ..
            } if window.board.keyboard_focus => {
                dismiss(window, ui.ctx());
                owns_events = true;
            }
            egui::Event::Key {
                key,
                modifiers,
                pressed: true,
                ..
            } if window.board.keyboard_focus => {
                if window.board.arrangement == super::filter::Arrangement::ByMilestone {
                    let (handled, activate) = window
                        .board
                        .surface
                        .milestones
                        .keyboard(*key, modifiers.shift);
                    if let Some(pane) = activate {
                        window.board.activate = Some(pane);
                    }
                    if handled {
                        if *key == egui::Key::Tab {
                            window.board.filter.editing = false;
                        }
                        owns_events = true;
                        continue;
                    }
                }
                if matches!(
                    key,
                    egui::Key::ArrowUp
                        | egui::Key::ArrowDown
                        | egui::Key::ArrowLeft
                        | egui::Key::ArrowRight
                        | egui::Key::Home
                        | egui::Key::End
                        | egui::Key::Enter
                ) {
                    navigate(&mut window.board, *key);
                }
                owns_events = true;
            }
            egui::Event::Key { pressed: false, .. } if window.board.keyboard_focus => {
                owns_events = true
            }
            egui::Event::Text(_) | egui::Event::Paste(_) if window.board.keyboard_focus => {
                owns_events = true
            }
            egui::Event::MouseWheel { .. }
                if ui
                    .input(|i| i.pointer.hover_pos())
                    .is_some_and(|pos| dock.contains(pos)) =>
            {
                // Keep the event for Board's ScrollArea, while keeping console dispatch out.
                owns_events = true;
            }
            egui::Event::PointerMoved(_) if window.board.pointer_owned => {
                owns_events = true;
            }
            egui::Event::PointerMoved(pos) if dock.contains(*pos) => {
                owns_events = true;
            }
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                ..
            } => {
                if *pressed {
                    if dock.contains(*pos) {
                        // Own the whole gesture, including presses on empty dock space.
                        window.board.pointer_owned = true;
                        window.board.keyboard_focus = true;
                        pointer(&mut window.board, *pos, true);
                        owns_events = true;
                    } else {
                        // Clicking a console transfers keyboard ownership without hiding Board.
                        window.board.pointer_owned = false;
                        window.board.keyboard_focus = false;
                        window.board.filter.editing = false;
                        window.board.surface.pressed = None;
                        window.board.surface.child_pressed = None;
                    }
                } else if window.board.pointer_owned {
                    window.board.keyboard_focus = true;
                    pointer(&mut window.board, *pos, false);
                    window.board.pointer_owned = false;
                    owns_events = true;
                }
            }
            egui::Event::PointerButton { pos, .. } if dock.contains(*pos) => {
                owns_events = true;
            }
            _ => {}
        }
    }
    if window.board.arrangement != arrangement_before {
        // Remembered in the configuration file. A refusal (a symlinked or unreadable file, a
        // Settings save still applying) keeps the choice on screen for this session and is said at
        // the Board's foot, where somebody who launched Ivy from the desktop can see it.
        remember(window);
    }
    if std::mem::take(&mut window.board.close_requested) {
        dismiss(window, ui.ctx());
        owns_events = true;
    }
    if owns_events {
        window.board.blocked_frame = Some(frame);
        window.header_input.block_for_settings(frame);
    }
    window.board.timing.record_work(started.elapsed());
    window.timed_separately += window.board.timing.take_cost();
    owns_events
}

/// Writes the arrangement on screen to the person's file. The store's own messages speak to the
/// Settings dialog, so the Board says what happened in its own words and leaves the detail on the
/// console that launched Ivy.
pub(super) fn remember(window: &mut Window) {
    let chosen = window.board.arrangement.into();
    match crate::settings::editor::runtime::save_board_arrangement(window, chosen) {
        Ok(()) => {
            window.board.remember_error = None;
            // What the file now holds, when there is a file and it took the line.
            if window.board.remember_in.is_some() {
                window.board.file_arrangement = Some(Some(chosen));
            }
        }
        Err(detail) => {
            eprintln!("the Board arrangement was not remembered: {detail}");
            window.board.remember_error = Some(
                "the configuration file could not be written (it is retried when the Board opens)"
                    .into(),
            );
        }
    }
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
        // Any press elsewhere leaves the filter field; a press on the field takes it back on release.
        state.filter.editing = false;
        state.surface.child_pressed = state
            .surface
            .child_hits
            .iter()
            .find(|(r, _)| r.contains(pos))
            .cloned();
        if state.surface.child_pressed.is_some() {
            state.surface.pressed = None;
            return;
        }
        state.surface.pressed = state
            .surface
            .painted
            .iter()
            .copied()
            .find(|(_, r)| r.contains(pos));
        if let Some((id, _)) = state.surface.pressed {
            state.selected = Some(id);
            state.ordinal = state
                .order
                .iter()
                .position(|slot| slot.pane == id)
                .unwrap_or(state.ordinal);
        }
    } else if let Some((rect, command)) = state.surface.child_pressed.take() {
        if let super::cards::Command::Bar(super::bar::Action::ShowAll(value)) = command {
            if rect.contains(pos) {
                state
                    .surface
                    .milestones
                    .apply(&super::milestones::state::Action::ShowAll(value));
            }
        } else if let super::cards::Command::Bar(action) = command {
            if rect.contains(pos) {
                state.close_requested |=
                    super::bar::apply(&mut state.arrangement, &mut state.filter, action);
            }
        } else if rect.contains(pos) {
            state
                .surface
                .apply_card_command(&command, &mut state.timing);
        }
    } else if let Some((id, rect)) = state.surface.pressed.take()
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
            surface: super::super::view::ViewState {
                painted: vec![(7, rect)],
                ..Default::default()
            },
            ..State::default()
        };
        pointer(&mut state, rect.center(), true);
        state.surface.painted = vec![(8, rect)];
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
    #[test]
    fn a_milestone_press_keeps_its_identity_and_opens_the_painted_column() {
        let plan = panel_contract::plan::parse_document_noting(include_str!(
            "../../tests/fixtures/board-milestones/plan.json"
        ))
        .unwrap()
        .0;
        let section = super::super::model::Section {
            key: None,
            kind: super::super::model::Kind::Repository {
                name: "TaxPlatform".into(),
            },
            entries: Vec::new(),
            roadmap: None,
            ink: None,
        };
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let roles = crate::theme::Roles::of(&crate::theme::Theme::default());
        let mut state = State {
            arrangement: super::super::filter::Arrangement::ByMilestone,
            ..Default::default()
        };
        let draw = |state: &mut State| {
            state.surface.milestones.hits.clear();
            state.surface.milestones.targets.clear();
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    super::super::milestones::paint_plan_with(
                        ui,
                        &section,
                        &plan,
                        &roles,
                        &mut state.surface.milestones,
                        &Default::default(),
                        &|_| super::super::observation::Column::Idle,
                        &mut |_, _, _, _, _| {},
                    );
                },
            );
            let texts = output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            output.drop_without_applying_deltas();
            state.surface.child_hits = state
                .surface
                .milestones
                .hits
                .iter()
                .map(|(r, a)| (*r, super::super::cards::Command::Milestone(a.clone())))
                .collect();
            texts
        };
        let before = draw(&mut state);
        assert!(!before.iter().any(|s| s.starts_with("H6 ·")));
        let at = state
            .surface
            .child_hits
            .iter()
            .find_map(|(r, a)| match a {
                super::super::cards::Command::Milestone(
                    super::super::milestones::state::Action::Strip(k),
                ) if k.milestone == "H6" => Some(r.center()),
                _ => None,
            })
            .unwrap();
        pointer(&mut state, at, true);
        state.surface.child_hits.clear();
        pointer(&mut state, at, false);
        let after = draw(&mut state);
        assert!(after.iter().any(|s| s == "H6 · Customers"));
        assert!(
            after
                .iter()
                .any(|s| s.contains("waiting on H2 (17/18), H5 (0/14)"))
        );
    }
}
