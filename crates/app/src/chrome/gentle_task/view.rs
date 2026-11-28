//! The review stays mounted underneath the explicit task and reply surfaces.
use super::*;
use crate::ui::floating;

pub(super) enum Action {
    OpenConversation(u64),
    DiscardConversation(u64),
    Close,
    Launch,
    Reply(u64, String, Answer),
    Dismiss(u64),
    DiscardDraft(u64),
    Stop(u64),
    FollowUp(u64),
    CloseSession(u64),
}

pub(crate) fn before_keys(window: &mut Window, ui: &mut egui::Ui) -> bool {
    let conversation_owned = super::conversation_view::show(window, ui);
    if !interact(window, ui) && !conversation_owned {
        return false;
    }
    crate::sidebar::consume_input(window, ui.ctx());
    ui.disable();
    true
}

fn interact(window: &mut Window, ui: &mut egui::Ui) -> bool {
    let ctx = ui.ctx().clone();
    if !window.gentle_enabled {
        let state = &mut window.chrome.gentle_task;
        if !state.blocks(&ctx) {
            return false;
        }
        return claim(state, ui);
    }
    if window.chrome.gentle_task.surface.is_none()
        && (window.settings_editor.is_open()
            || window.chrome.menu
            || window.chrome.guide.blocked(&ctx)
            || window.chrome.updates.blocked(&ctx)
            || window.usage.blocks(&ctx)
            || window.pane_view.is_modal()
            || crate::board::blocked(window, &ctx))
    {
        window.chrome.gentle_task.pressed_badge = false;
        return false;
    }
    let state = &mut window.chrome.gentle_task;
    if state.surface.is_some() {
        let action = dialog(state, &window.theme, &ctx);
        match action {
            Some(Action::OpenConversation(id)) => {
                state.selected_conversation = Some(id);
                state.close(&ctx);
            }
            Some(Action::DiscardConversation(id)) => {
                if state.sessions.get(&id).is_some_and(|t| t.session.is_none()) {
                    state.sessions.remove(&id);
                    if state.selected_conversation == Some(id) {
                        state.selected_conversation = None;
                    }
                }
                state.prune(&ctx);
            }
            Some(Action::Close) => state.close(&ctx),
            Some(Action::Launch) => state.launch(&ctx),
            Some(Action::Reply(task, id, answer)) => state.respond(task, &id, answer, &ctx),
            Some(Action::Dismiss(id)) => {
                if let Some(task) = state.sessions.get_mut(&id) {
                    task.issue = None;
                    task.dialogs.retain(|d| d.status != Status::Uncertain);
                }
                state.prune(&ctx);
            }
            Some(Action::Stop(id)) => {
                if let Some(task) = state.sessions.get_mut(&id) {
                    task.request_stop();
                    ctx.request_repaint();
                }
            }
            Some(Action::FollowUp(id)) => {
                if state
                    .sessions
                    .get_mut(&id)
                    .is_some_and(Task::send_follow_up)
                {
                    state.close(&ctx);
                }
            }
            Some(Action::DiscardDraft(id)) => {
                if let Some(task) = state.sessions.get_mut(&id)
                    && task.session.is_none()
                {
                    task.follow_up.clear();
                }
                state.prune(&ctx);
            }
            Some(Action::CloseSession(id)) => {
                if let Some(task) = state.sessions.get_mut(&id) {
                    task.close_session();
                    if task.dialogs.is_empty() && task.follow_up.is_empty() {
                        task.issue = None;
                    }
                }
                state.prune(&ctx);
            }
            None => {}
        }
        return claim(state, ui);
    }
    if state.await_release || state.blocked_frame == Some(ctx.cumulative_frame_nr()) {
        return claim(state, ui);
    }
    let Some(label) = indicator(&state.sessions) else {
        if state.pressed_badge {
            state.pressed_badge = false;
            return claim(state, ui);
        }
        return false;
    };
    let focus_before = ctx.memory(|m| m.focused());
    let (anchor, offset, width) = badge_position(ctx.content_rect(), state.conversation_rect);
    let badge = egui::Area::new(egui::Id::new("gentle-question-indicator"))
        .order(egui::Order::Tooltip)
        .anchor(anchor, offset)
        .movable(false)
        .show(&ctx, |ui| {
            floating::style(ui, &window.theme);
            ui.set_max_width(width);
            ui.button(label)
        });
    let pressed = ctx.input(|i| {
        i.pointer.button_pressed(egui::PointerButton::Primary)
            && i.pointer
                .interact_pos()
                .is_some_and(|pos| badge.inner.rect.contains(pos))
    });
    if pressed {
        state.previous_focus = focus_before;
        state.pressed_badge = true;
    }
    if badge.inner.clicked() {
        if !state.pressed_badge {
            state.previous_focus = focus_before;
        }
        state.pressed_badge = false;
        if state.sessions.values().any(Task::visible_in_overview) {
            state.surface = Some(Surface::Overview);
        } else {
            state.selected_conversation = state.sessions.keys().next_back().copied();
        }
        ctx.request_repaint();
        return claim(state, ui);
    }
    if state.pressed_badge {
        if !ctx.input(|i| i.pointer.any_down()) {
            state.pressed_badge = false;
        }
        return claim(state, ui);
    }
    false
}

fn badge_position(
    bounds: egui::Rect,
    conversation: Option<egui::Rect>,
) -> (egui::Align2, [f32; 2], f32) {
    if let Some(rect) = conversation {
        let left = rect.left() - bounds.left() - 16.;
        let right = bounds.right() - rect.right() - 16.;
        if left >= right && left >= 80. {
            return (egui::Align2::LEFT_TOP, [8., 90.], left.min(330.));
        }
        if right >= 80. {
            return (egui::Align2::RIGHT_TOP, [-8., 90.], right.min(330.));
        }
    }
    (
        egui::Align2::CENTER_TOP,
        [0., 90.],
        (bounds.width() - 32.).clamp(80., 330.),
    )
}

fn indicator(sessions: &BTreeMap<u64, Task>) -> Option<String> {
    let mut waiting = sessions.iter().flat_map(|(&id, task)| {
        task.dialogs
            .iter()
            .filter(|d| task.turn != Turn::Stopping && d.status == Status::Waiting)
            .map(move |_| (id, task))
    });
    let (id, task, label) = if let Some((id, task)) = waiting.next() {
        let count = 1 + waiting.count();
        if count > 1 {
            return Some(format!("{count} Gentle questions"));
        }
        (id, task, "Gentle question")
    } else if let Some((&id, task)) = sessions.iter().find(|(_, task)| task.needs_attention()) {
        (id, task, "Gentle task status")
    } else {
        let mut count = 0;
        let mut bounded = false;
        let mut stale = false;
        for list in sessions.values().filter_map(|task| task.helpers.as_ref()) {
            count += list.rows.len();
            bounded |= list.rows.len() == transport::HELPER_LIMIT;
            stale |= list.unavailable && !list.rows.is_empty();
        }
        let plus = if bounded { "+" } else { "" };
        let qualifier = if stale { " · last known" } else { "" };
        return if count > 0 {
            Some(format!("Gentle helpers · {count}{plus}{qualifier}"))
        } else {
            (!sessions.is_empty()).then(|| format!("Gentle conversations · {}", sessions.len()))
        };
    };
    let project = task
        .directory
        .file_name()
        .unwrap_or(task.directory.as_os_str())
        .to_string_lossy();
    Some(format!("{label} · {project} · #{id}"))
}

fn claim(state: &mut GentleTasks, ui: &mut egui::Ui) -> bool {
    let ctx = ui.ctx();
    state.blocked_frame = Some(ctx.cumulative_frame_nr());
    if state.await_release {
        state.await_release = ctx.input(|i| i.pointer.any_down());
    }
    true
}

fn dialog(
    state: &mut GentleTasks,
    theme: &crate::theme::Theme,
    ctx: &egui::Context,
) -> Option<Action> {
    let surface = state.surface.as_mut()?;
    let compose = matches!(surface, Surface::Compose { .. });
    let mut action = None;
    let bounds = ctx.content_rect().size();
    let body_height = (bounds.y - 145.0).max(40.);
    let layer = egui::Id::new(("gentle-task-dialog", compose));
    egui::Modal::new(layer)
        .area(egui::Modal::default_area(layer).order(egui::Order::Tooltip))
        .frame(floating::dialog_frame(&crate::ui::style::roles(ctx, theme)).inner_margin(16))
        .show(ctx, |ui| {
            floating::style(ui, theme);
            crate::settings::editor::view::settings_geometry(ui.style_mut());
            ui.set_width((bounds.x - 64.0).clamp(64.0, 520.0));
            let title = if compose {
                "New Gentle task"
            } else {
                "Gentle tasks"
            };
            if crate::settings::editor::view::dialog::header(ui, title) {
                action = Some(Action::Close);
            }
            egui::ScrollArea::vertical()
                .id_salt("gentle-task-body")
                .max_height(body_height)
                .min_scrolled_height(if state.sessions.is_empty() {
                    40.
                } else {
                    body_height.min(560.)
                })
                .show(ui, |ui| {
                    if let Surface::Compose {
                        cwd,
                        prompt,
                        focus,
                        error,
                    } = surface
                    {
                        if let Some(path) = cwd {
                            ui.label(path.display().to_string());
                            let edit = ui.add(
                                egui::TextEdit::multiline(prompt)
                                    .hint_text("What should Gentle do?")
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(3),
                            );
                            if std::mem::take(focus) {
                                edit.request_focus();
                            }
                            let oversized = prompt.len() > transport::MAX_TEXT;
                            if oversized {
                                ui.label("This task is too long to send.");
                            }
                            if ui
                                .add_enabled(
                                    !oversized && !prompt.trim().is_empty(),
                                    egui::Button::new("Start task"),
                                )
                                .clicked()
                            {
                                action = Some(Action::Launch);
                            }
                        } else {
                            ui.label("The active console's working directory is unavailable.");
                        }
                        if !error.is_empty() {
                            ui.label(error.as_str());
                        }
                    }
                    for (&task_id, task) in &mut state.sessions {
                        if !compose && !task.visible_in_overview() {
                            continue;
                        }
                        if let Some(entry) = ui
                            .push_id(task_id, |ui| {
                                let action = entry(ui, task, task_id, compose);
                                if let Some(list) = &task.helpers {
                                    helper_rows(ui, list);
                                }
                                action
                            })
                            .inner
                        {
                            action = Some(entry);
                        }
                    }
                });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        action = Some(Action::Close);
    }
    action
}

pub(super) fn helper_rows(ui: &mut egui::Ui, list: &HelperList) {
    if !list.rows.is_empty() {
        ui.strong("Helpers");
    }
    if list.unavailable {
        ui.label(if list.rows.is_empty() {
            "Helper list unavailable."
        } else {
            "Helper list unavailable. Showing last known states."
        });
    }
    if list.rows.len() == transport::HELPER_LIMIT {
        ui.label(format!(
            "Showing {} helpers. More may exist.",
            transport::HELPER_LIMIT
        ));
    }
    for helper in &list.rows {
        ui.label(format!(
            "{} · {} · {}",
            helper.name, helper.state, helper.id
        ));
    }
}

pub(super) fn entry(
    ui: &mut egui::Ui,
    task: &mut Task,
    task_id: u64,
    compose: bool,
) -> Option<Action> {
    let mut action = None;
    let project = task.directory.display();
    ui.separator();
    ui.strong(format!("Session #{task_id} · {project}"));
    if ui.button("Open conversation").clicked() {
        action = Some(Action::OpenConversation(task_id));
    }
    if task.session.is_none() && ui.button("Discard conversation").clicked() {
        action = Some(Action::DiscardConversation(task_id));
    }
    if compose && task.initial_handled {
        ui.label("Command handled");
    }
    if let Some(issue) = &task.issue {
        ui.label(issue);
    }
    for dialog in &mut task.dialogs {
        if dialog.status == Status::Uncertain {
            ui.label(format!("Delivery uncertain · {}", dialog.question.title));
        } else if !compose && task.turn != Turn::Stopping && dialog.status == Status::Waiting {
            let response = ui
                .push_id(egui::Id::new((task_id, &dialog.question.id)), |ui| {
                    question(ui, dialog)
                })
                .inner;
            if let Some(answer) = response {
                action = Some(Action::Reply(task_id, dialog.question.id.clone(), answer));
            }
        }
    }
    if task.turn != Turn::Stopping
        && (task.issue.is_some() || task.dialogs.iter().any(|d| d.status == Status::Uncertain))
        && ui.button("Dismiss status").clicked()
    {
        action = Some(Action::Dismiss(task_id));
    }
    if task.session.is_none() {
        if !task.follow_up.is_empty() {
            ui.label("Follow-up draft");
            ui.add(egui::Label::new(&task.follow_up).selectable(true));
            if ui.button("Discard draft").clicked() {
                action = Some(Action::DiscardDraft(task_id));
            }
        }
        return action;
    }
    ui.horizontal_wrapped(|ui| {
        if task.turn == Turn::Stopping {
            ui.label("Stopping task…");
        } else {
            let label = if task.turn == Turn::StopUnconfirmed {
                "Retry stop"
            } else {
                "Stop task"
            };
            if ui.button(label).clicked() {
                action = Some(Action::Stop(task_id));
            }
        }
        if ui.button("Close session").clicked() {
            action = Some(Action::CloseSession(task_id));
        }
    });
    if task.turn == Turn::Ready {
        if ui
            .add(
                egui::TextEdit::multiline(&mut task.follow_up)
                    .hint_text("Continue this conversation…")
                    .desired_width(f32::INFINITY)
                    .desired_rows(2),
            )
            .changed()
        {
            task.awaiting_prompt_receipt = false;
        }
        let fits = task.follow_up.len() <= transport::MAX_TEXT;
        if !fits {
            ui.label("This message is too long to send.");
        }
        if ui
            .add_enabled(
                fits && !task.follow_up.trim().is_empty(),
                egui::Button::new("Send follow-up"),
            )
            .clicked()
        {
            action = Some(Action::FollowUp(task_id));
        }
    }
    action
}

fn question(ui: &mut egui::Ui, dialog: &mut Dialog) -> Option<Answer> {
    ui.label(&dialog.question.title);
    let mut answer = None;
    match &dialog.question.kind {
        Kind::Select(options) => {
            for option in options {
                if ui.button(option).clicked() {
                    answer = Some(Answer::Value(option.clone()));
                }
            }
        }
        Kind::Confirm(message) => {
            ui.label(message);
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    answer = Some(Answer::Confirm(true));
                }
                if ui.button("No").clicked() {
                    answer = Some(Answer::Confirm(false));
                }
            });
        }
        Kind::Input(_) | Kind::Editor(_) => {
            let edit = match &dialog.question.kind {
                Kind::Input(placeholder) => {
                    egui::TextEdit::singleline(&mut dialog.draft).hint_text(placeholder)
                }
                _ => egui::TextEdit::multiline(&mut dialog.draft).desired_rows(4),
            };
            ui.add(edit.desired_width(f32::INFINITY));
            let fits = dialog.draft.len() <= transport::MAX_TEXT;
            if !fits {
                ui.label("This reply is too long to send.");
            }
            if ui
                .add_enabled(fits, egui::Button::new("Send reply"))
                .clicked()
            {
                answer = Some(Answer::Value(dialog.draft.clone()));
            }
        }
    }
    if ui.button("Cancel question").clicked() {
        return Some(Answer::Cancel);
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_inventory_paints_availability_and_bounds_without_empty_controls() {
        fn labels(list: &HelperList) -> Vec<String> {
            let ctx = egui::Context::default();
            let output = ctx.run_ui(Default::default(), |ui| helper_rows(ui, list));
            let labels = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) => Some(text.galley.job.text.clone()),
                    _ => None,
                })
                .collect();
            output.drop_without_applying_deltas();
            labels
        }
        let mut list = HelperList::default();
        assert!(labels(&list).is_empty());
        list.unavailable = true;
        assert_eq!(labels(&list), ["Helper list unavailable."]);
        list.rows = (1..=49)
            .map(|n| transport::Helper {
                id: format!("helper-{n}"),
                name: "Inspector".into(),
                state: "queued",
            })
            .collect();
        let stale = labels(&list);
        assert!(stale.contains(&"Helper list unavailable. Showing last known states.".into()));
        assert!(stale.contains(&"Inspector · queued · helper-49".into()));
        assert!(!stale.iter().any(|s| s.contains("More may exist")));
        list.unavailable = false;
        list.rows.push(transport::Helper {
            id: "helper-50".into(),
            name: "Inspector".into(),
            state: "failed",
        });
        let bounded = labels(&list);
        assert!(bounded.contains(&"Showing 50 helpers. More may exist.".into()));
        assert!(bounded.contains(&"Inspector · failed · helper-50".into()));
        assert!(!bounded.iter().any(|s| s.contains("unavailable")));
    }
}
