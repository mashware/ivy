//! An independent native conversation leaves consoles and mounted reviews available.
use super::{view, *};
use transport::conversation::{Cache, Row};

#[derive(Default)]
pub(super) struct HistoryView {
    heights: BTreeMap<u64, (u32, f32)>,
}

pub(super) fn show(window: &mut Window, ui: &mut egui::Ui) -> bool {
    let ctx = ui.ctx().clone();
    let state = &mut window.chrome.gentle_task;
    state.conversation_rect = None;
    if !window.gentle_enabled
        || state.surface.is_some()
        || window.settings_editor.is_open()
        || window.chrome.menu
        || window.chrome.guide.blocked(&ctx)
        || window.chrome.updates.blocked(&ctx)
        || window.usage.blocks(&ctx)
        || window.pane_view.is_modal()
        || window.board.is_open()
    {
        return false;
    }
    let Some(id) = state.selected_conversation else {
        return false;
    };
    if !state.sessions.contains_key(&id) {
        state.selected_conversation = None;
        return false;
    }
    let mut open = true;
    let mut action = None;
    let mut selector_open = false;
    let bounds = ctx.content_rect();
    let available = egui::Rect::from_min_max(
        bounds.min + egui::vec2(8., 48.),
        bounds.max - egui::vec2(8., 8.),
    );
    let response = egui::Window::new("Gentle conversation")
        .id(egui::Id::new("gentle-native-conversation"))
        .open(&mut open)
        .default_pos(egui::pos2(400., 140.))
        .default_size(egui::vec2(620., 570.))
        .min_size(egui::vec2(300., 300.))
        .max_size(available.size().max(egui::vec2(300., 300.)))
        .constrain_to(available)
        .show(&ctx, |ui| {
            crate::ui::floating::style(ui, &window.theme);
            selector_open = egui::ComboBox::from_id_salt("native-session-selector")
                .selected_text(format!("Session #{id}"))
                .show_ui(ui, |ui| {
                    for (&other, task) in &state.sessions {
                        if ui
                            .selectable_label(
                                id == other,
                                format!("Session #{other} · {}", task.directory.display()),
                            )
                            .clicked()
                        {
                            state.selected_conversation = Some(other);
                        }
                    }
                })
                .inner
                .is_some();
            let task = state.sessions.get_mut(&id).expect("selected session");
            ui.label(task.directory.display().to_string());
            ui.strong(status(task));
            if let Some(issue) = &task.issue {
                ui.label(issue);
            }
            egui::ScrollArea::vertical()
                .id_salt(("native-history", id))
                .max_height((bounds.height() - 450.).clamp(80., 320.))
                .min_scrolled_height(80.)
                .stick_to_bottom(true)
                .show_viewport(ui, |ui, viewport| {
                    history(ui, &task.transcript, &mut task.history_view, viewport);
                });
            egui::ScrollArea::vertical()
                .id_salt(("native-state", id))
                .max_height(105.)
                .show(ui, |ui| {
                    producer(ui, &task.snapshot);
                    if let Some(list) = &task.helpers {
                        view::helper_rows(ui, list);
                    }
                });
            ui.separator();
            action = controls(ui, task, id);
        });
    let rect = response.as_ref().map(|r| r.response.rect);
    state.conversation_rect = rect;
    let over = selector_open
        || ctx.input(|i| {
            i.pointer
                .interact_pos()
                .is_some_and(|p| rect.is_some_and(|r| r.contains(p)))
        });
    let released_owned = state.conversation_pointer;
    if ctx.input(|i| i.pointer.any_pressed()) {
        state.conversation_pointer = over;
        state.conversation_focus = over;
        if !over {
            // A neighboring console must regain typing on the same click.
            ctx.memory_mut(|m| {
                if let Some(focus) = m.focused() {
                    m.surrender_focus(focus);
                }
            });
        }
    }
    let gesture = ctx.input(|i| {
        i.pointer.any_pressed()
            || i.pointer.any_released()
            || i.pointer.is_moving()
            || i.smooth_scroll_delta != egui::Vec2::ZERO
    });
    let pointer_owned = (over || released_owned) && gesture;
    if state.conversation_focus && !pointer_owned {
        // Text belongs to the native conversation while pointer input outside still works.
        ctx.input_mut(|i| {
            i.events.retain(|event| {
                matches!(
                    event,
                    egui::Event::PointerMoved(_)
                        | egui::Event::PointerGone
                        | egui::Event::PointerButton { .. }
                        | egui::Event::MouseWheel { .. }
                        | egui::Event::ModifiersChanged(_)
                        | egui::Event::WindowFocused(_)
                )
            })
        });
    }
    let owned = pointer_owned;
    if !ctx.input(|i| i.pointer.any_down()) {
        state.conversation_pointer = false;
    }
    dispatch(state, action, &ctx);
    if !open {
        state.selected_conversation = None;
        state.conversation_focus = false;
    }
    owned
}

fn status(task: &Task) -> &'static str {
    if task.session.is_none() {
        return if task.closed_by_user {
            "Session closed · retained conversation"
        } else {
            "Disconnected · retained conversation"
        };
    }
    if task.turn == Turn::Stopping {
        return "Stopping task…";
    }
    if task.turn == Turn::StopUnconfirmed {
        return "Stop unconfirmed";
    }
    if task.turn == Turn::Working && task.awaiting_prompt_receipt {
        return "Waiting for prompt acceptance";
    }
    if task.failed {
        return "Failed · ready for follow-up";
    }
    if task.initial_handled {
        return "Command handled · completion unverified";
    }
    match task.turn {
        Turn::Working => "Working",
        Turn::Stopping => "Stopping task…",
        Turn::StopUnconfirmed => "Stop unconfirmed",
        Turn::Ready if task.issue.is_some() => "Ready · see status",
        Turn::Ready => "Ready for follow-up",
    }
}

fn controls(ui: &mut egui::Ui, task: &mut Task, id: u64) -> Option<view::Action> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        if task.session.is_some() {
            if task.turn != Turn::Stopping && ui.button("Stop task").clicked() {
                action = Some(view::Action::Stop(id));
            }
            if ui.button("Close session").clicked() {
                action = Some(view::Action::CloseSession(id));
            }
        } else if ui.button("Discard conversation").clicked() {
            action = Some(view::Action::DiscardConversation(id));
        }
        if task
            .dialogs
            .iter()
            .any(|d| d.status == Status::Waiting || d.status == Status::Uncertain)
            && ui.button("Questions / status").clicked()
        {
            action = Some(view::Action::OpenConversation(id));
        }
    });
    if task.session.is_none() || task.turn == Turn::Ready {
        ui.add(
            egui::TextEdit::multiline(&mut task.follow_up)
                .id_salt(("native-composer", id))
                .hint_text("Continue this conversation…")
                .desired_width(f32::INFINITY)
                .desired_rows(2),
        );
        if task.follow_up.len() > transport::MAX_TEXT {
            ui.label("This message is too long to send.");
        }
        if ui
            .add_enabled(
                task.session.is_some()
                    && !task.follow_up.trim().is_empty()
                    && task.follow_up.len() <= transport::MAX_TEXT,
                egui::Button::new("Send follow-up"),
            )
            .clicked()
        {
            action = Some(view::Action::FollowUp(id));
        }
    }
    action
}

fn dispatch(state: &mut GentleTasks, action: Option<view::Action>, ctx: &egui::Context) {
    match action {
        Some(view::Action::Stop(id)) => {
            if let Some(t) = state.sessions.get_mut(&id) {
                t.request_stop();
            }
        }
        Some(view::Action::CloseSession(id)) => {
            if let Some(t) = state.sessions.get_mut(&id) {
                t.close_session();
            }
        }
        Some(view::Action::FollowUp(id)) => {
            if let Some(t) = state.sessions.get_mut(&id) {
                t.send_follow_up();
            }
        }
        Some(view::Action::DiscardConversation(id)) => {
            if state.sessions.get(&id).is_some_and(|t| t.session.is_none()) {
                state.sessions.remove(&id);
                state.selected_conversation = None;
            }
        }
        Some(view::Action::OpenConversation(_)) => {
            state.previous_focus = ctx.memory(|m| m.focused());
            state.surface = Some(Surface::Overview);
        }
        _ => {}
    }
}

fn producer(ui: &mut egui::Ui, snapshot: &transport::snapshot::SnapshotState) {
    if snapshot.unavailable {
        ui.label("Task/activity data is not currently available. Last known values are retained.");
        ui.label("Open New Gentle session for console-only interactions.");
    }
    if let Some(label) = &snapshot.activity {
        ui.label(format!("Activity · {label}"));
    }
    if snapshot.truncated {
        ui.label("Producer task list truncated.");
    }
    for todo in &snapshot.items {
        ui.label(format!("{} · {} · {}", todo.id, todo.title, todo.state));
        if let Some(note) = &todo.note {
            ui.label(note);
        }
    }
}

fn row_height(row: &Row, width: f32, heights: &BTreeMap<u64, (u32, f32)>) -> f32 {
    heights
        .get(&row.id)
        .filter(|(w, _)| *w == width.to_bits())
        .map_or_else(
            || {
                if row.role == "tool" {
                    44.
                } else {
                    ((row.text.len() as f32 * 7. / width.max(1.)).ceil().max(1.) * 18.) + 24.
                }
            },
            |(_, height)| *height,
        )
}

fn tool_title(row: &Row, continued: bool) -> String {
    if continued {
        return "Tool output continued".into();
    }
    let mut chars = row.text.lines().next().unwrap_or("Tool activity").chars();
    let mut title: String = chars.by_ref().take(120).collect();
    if chars.next().is_some() {
        title.push('…');
    }
    title
}

fn history(ui: &mut egui::Ui, cache: &Cache, state: &mut HistoryView, viewport: egui::Rect) {
    if cache.gap || cache.truncated {
        ui.label("Earlier conversation text was clipped to the retention limit.");
    }
    let retained: std::collections::BTreeSet<_> = cache.rows.iter().map(|r| r.id).collect();
    state.heights.retain(|id, _| retained.contains(id));
    let width = ui.available_width();
    let origin = ui.cursor().top();
    let mut y = 0.;
    let mut previous = None;
    for row in &cache.rows {
        let height = row_height(row, width, &state.heights);
        let visible = y + height >= viewport.top() - 100. && y <= viewport.bottom() + 100.;
        if visible {
            let top = ui.cursor().top();
            ui.push_id(row.id, |ui| {
                if previous != Some(row.message) {
                    ui.strong(row.role);
                }
                if row.role == "tool" {
                    egui::CollapsingHeader::new(tool_title(row, previous == Some(row.message)))
                        .id_salt(("tool-details", row.id))
                        .show(ui, |ui| {
                            ui.add(egui::Label::new(row.text.as_ref()).wrap().selectable(true));
                        });
                } else {
                    ui.add(egui::Label::new(row.text.as_ref()).wrap().selectable(true));
                }
            });
            state
                .heights
                .insert(row.id, (width.to_bits(), (ui.cursor().top() - top).max(1.)));
        } else {
            ui.add_space(height);
        }
        y = ui.cursor().top() - origin;
        previous = Some(row.message);
    }
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(all(test, windows))]
mod tests {
    crate::windows_unix_omission!(
        native_transcript_composer_and_neighbor_console_own_their_input_under_flood,
        "unix-only: drives executable Python shebang clients and a POSIX PTY receiver"
    );
}
