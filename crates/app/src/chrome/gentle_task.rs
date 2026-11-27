//! Window-owned interactions for explicitly submitted Gentle tasks.
mod transport;
mod view;
pub(crate) use transport::shutdown;

use crate::Window;
use eframe::egui;
use std::{collections::BTreeMap, fmt, path::PathBuf, sync::Arc};
use transport::{Answer, Event, Kind, Question, Session};

#[derive(Default)]
pub(crate) struct GentleTasks {
    next: u64,
    sessions: BTreeMap<u64, Task>,
    surface: Option<Surface>,
    previous_focus: Option<egui::Id>,
    pressed_badge: bool,
    await_release: bool,
    blocked_frame: Option<u64>,
}

impl fmt::Debug for GentleTasks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gentle tasks")
            .field("count", &self.sessions.len())
            .finish_non_exhaustive()
    }
}

enum Surface {
    Compose {
        cwd: Option<PathBuf>,
        prompt: String,
        focus: bool,
        error: String,
    },
    Questions,
}

struct Task {
    directory: PathBuf,
    session: Option<Session>,
    dialogs: Vec<Dialog>,
    issue: Option<String>,
    initial_handled: bool,
    turn: Turn,
    follow_up: String,
    awaiting_prompt_receipt: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Turn {
    Working,
    Stopping,
    StopUnconfirmed,
    Ready,
}

struct Dialog {
    question: Question,
    draft: String,
    status: Status,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Status {
    Waiting,
    Submitting,
    Sent,
    Uncertain,
}

impl Task {
    fn needs_attention(&self) -> bool {
        self.turn == Turn::Stopping
            || (self.session.is_none() && !self.follow_up.is_empty())
            || self.issue.is_some()
            || self
                .dialogs
                .iter()
                .any(|d| matches!(d.status, Status::Waiting | Status::Uncertain))
    }

    fn receive(&mut self, event: Event) {
        match event {
            Event::Question(question) => {
                if self.dialogs.iter().any(|d| d.question.id == question.id) {
                    return;
                }
                let draft = match &question.kind {
                    Kind::Editor(prefill) => prefill.clone(),
                    _ => String::new(),
                };
                self.dialogs.push(Dialog {
                    question,
                    draft,
                    status: Status::Waiting,
                });
            }
            Event::Sent(id) => self.set_status(&id, Status::Sent),
            Event::Uncertain(id) => self.set_status(&id, Status::Uncertain),
            Event::Expired(id) => self.dialogs.retain(|d| d.question.id != id),
            Event::PromptAccepted => {
                if std::mem::take(&mut self.awaiting_prompt_receipt) {
                    self.follow_up.clear();
                }
            }
            Event::Handled => self.initial_handled = true,
            Event::Notice(reason) => self.issue = Some(reason),
            Event::TurnFailed(reason) => {
                self.awaiting_prompt_receipt = false;
                self.turn = Turn::Ready;
                self.issue = Some(reason);
            }
            Event::Completed => {
                self.turn = Turn::Ready;
                self.dialogs.retain(|d| d.status != Status::Sent);
            }
            Event::StopOutcome {
                message,
                can_continue,
            } => {
                self.turn = if can_continue {
                    Turn::Ready
                } else {
                    Turn::StopUnconfirmed
                };
                self.issue = Some(message);
            }
            Event::Closed(reason) => self.disconnect(reason),
        }
    }

    fn request_stop(&mut self) {
        if self.turn == Turn::Stopping {
            return;
        }
        let Some(session) = self.session.as_mut() else {
            return;
        };
        match session.request_stop() {
            Ok(()) => {
                self.turn = Turn::Stopping;
                self.issue = None;
            }
            Err(reason) => self.issue = Some(reason),
        }
    }

    fn send_follow_up(&mut self) -> bool {
        if self.turn != Turn::Ready
            || self.follow_up.trim().is_empty()
            || self.follow_up.len() > transport::MAX_TEXT
        {
            return false;
        }
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        match session.follow_up(&self.follow_up) {
            Ok(()) => {
                self.awaiting_prompt_receipt = true;
                self.turn = Turn::Working;
                self.initial_handled = false;
                self.issue = None;
                true
            }
            Err(reason) => {
                self.issue = Some(reason);
                false
            }
        }
    }

    fn set_status(&mut self, id: &str, status: Status) {
        if let Some(dialog) = self
            .dialogs
            .iter_mut()
            .find(|d| d.question.id == id && d.status != Status::Uncertain)
        {
            dialog.status = status;
        }
    }

    fn disconnect(&mut self, reason: String) {
        self.session = None;
        self.turn = Turn::Ready;
        self.issue = Some(reason);
        self.retire_questions();
    }

    fn retire_questions(&mut self) {
        self.dialogs.retain(|d| d.status != Status::Waiting);
        for dialog in &mut self.dialogs {
            dialog.status = Status::Uncertain;
        }
    }
}

impl GentleTasks {
    pub(crate) fn blocks(&self, ctx: &egui::Context) -> bool {
        self.surface.is_some()
            || self.pressed_badge
            || self.await_release
            || self.blocked_frame == Some(ctx.cumulative_frame_nr())
    }

    fn close(&mut self, ctx: &egui::Context) {
        self.surface = None;
        self.pressed_badge = false;
        self.await_release = ctx.input(|i| i.pointer.any_down());
        self.blocked_frame = Some(ctx.cumulative_frame_nr());
        let previous = self.previous_focus.take();
        ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
            if let Some(id) = previous {
                memory.request_focus(id);
            }
        });
        ctx.request_repaint();
    }

    fn prune(&mut self, ctx: &egui::Context) {
        self.sessions
            .retain(|_, task| task.session.is_some() || task.needs_attention());
        if matches!(self.surface, Some(Surface::Questions))
            && !self.sessions.values().any(Task::needs_attention)
        {
            self.close(ctx);
        }
    }

    fn launch(&mut self, ctx: &egui::Context) {
        let Some(Surface::Compose {
            cwd, prompt, error, ..
        }) = &mut self.surface
        else {
            return;
        };
        let Some(directory) = cwd.clone() else { return };
        if self.sessions.len() >= 16 {
            *error = "Close a Gentle session or dismiss its status before starting another.".into();
            return;
        }
        if prompt.trim().is_empty() || prompt.len() > transport::MAX_TEXT {
            return;
        }
        let wake = ctx.clone();
        let result = Session::start_installed(
            directory.clone(),
            prompt.clone(),
            Arc::new(move || wake.request_repaint()),
        )
        .map_err(|err| err.to_string());
        match result {
            Ok(session) => {
                self.next += 1;
                self.sessions.insert(
                    self.next,
                    Task {
                        directory,
                        session: Some(session),
                        dialogs: Vec::new(),
                        issue: None,
                        initial_handled: false,
                        turn: Turn::Working,
                        follow_up: String::new(),
                        awaiting_prompt_receipt: false,
                    },
                );
                self.close(ctx);
            }
            Err(reason) => *error = reason,
        }
    }

    fn respond(&mut self, task_id: u64, id: &str, answer: Answer, ctx: &egui::Context) {
        let Some(task) = self.sessions.get_mut(&task_id) else {
            return;
        };
        if task.turn == Turn::Stopping {
            return;
        }
        let Some(dialog) = task
            .dialogs
            .iter_mut()
            .find(|d| d.question.id == id && d.status == Status::Waiting)
        else {
            return;
        };
        if dialog
            .question
            .deadline
            .is_some_and(|deadline| deadline <= std::time::Instant::now())
        {
            ctx.request_repaint();
            return;
        }
        let Some(session) = &mut task.session else {
            return;
        };
        match session.reply(id, answer) {
            Ok(()) => {
                dialog.status = Status::Submitting;
                self.close(ctx);
            }
            Err(message) => {
                task.issue = Some(message.into());
            }
        }
    }
}

pub(crate) fn start(window: &mut Window, ctx: &egui::Context) {
    if !window.gentle_enabled || window.chrome.gentle_task.surface.is_some() {
        return;
    }
    let pane = window.active().focus;
    let cwd = crate::viewer::console_directory(window, pane);
    let state = &mut window.chrome.gentle_task;
    state.previous_focus = ctx.memory(|m| m.focused());
    state.surface = Some(Surface::Compose {
        cwd,
        prompt: String::new(),
        focus: true,
        error: String::new(),
    });
    state.blocked_frame = Some(ctx.cumulative_frame_nr());
    ctx.request_repaint();
}

pub(crate) fn poll(window: &mut Window, ctx: &egui::Context) {
    let state = &mut window.chrome.gentle_task;
    if !window.gentle_enabled {
        if state.surface.is_some() {
            state.close(ctx);
        }
        state.sessions.clear();
        return;
    }
    for task in state.sessions.values_mut() {
        // Leave frame time for review when several sessions publish together.
        for _ in 0..16 {
            let Some(event) = task.session.as_mut().and_then(|s| s.try_event()) else {
                break;
            };
            task.receive(event);
            ctx.request_repaint();
        }
    }
    state.prune(ctx);
}

pub(crate) use view::before_keys;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnect_keeps_unconfirmed_submission_and_clears_unanswered_routes() {
        let mut task = Task {
            directory: PathBuf::from("B"),
            session: None,
            dialogs: Vec::new(),
            issue: None,
            initial_handled: false,
            turn: Turn::Working,
            follow_up: String::new(),
            awaiting_prompt_receipt: false,
        };
        for id in ["sent", "waiting"] {
            task.receive(Event::Question(Question {
                id: id.into(),
                title: "Continue?".into(),
                kind: Kind::Confirm("Proceed".into()),
                deadline: None,
            }));
        }
        task.receive(Event::Sent("sent".into()));
        task.receive(Event::Closed("Lost contact.".into()));
        assert_eq!(task.dialogs.len(), 1);
        assert_eq!(task.dialogs[0].question.id, "sent");
        assert!(task.dialogs[0].status == Status::Uncertain);
        assert_eq!(task.issue.as_deref(), Some("Lost contact."));
    }

    #[test]
    fn stop_preserves_unconfirmed_work_and_drafts_until_the_client_reconciles_them() {
        let mut task = Task {
            directory: PathBuf::from("A"),
            session: None,
            dialogs: Vec::new(),
            issue: None,
            initial_handled: false,
            turn: Turn::Working,
            follow_up: String::new(),
            awaiting_prompt_receipt: false,
        };
        let question = |id: &str| {
            Event::Question(Question {
                id: id.into(),
                title: "Which route?".into(),
                kind: Kind::Input(String::new()),
                deadline: None,
            })
        };
        task.receive(question("submitted"));
        task.set_status("submitted", Status::Submitting);
        task.turn = Turn::Stopping;
        task.receive(Event::Uncertain("submitted".into()));
        task.receive(Event::Sent("submitted".into()));
        assert!(task.dialogs[0].status == Status::Uncertain);
        task.receive(question("late"));
        task.dialogs[1].draft = "keep this draft".into();
        task.receive(Event::StopOutcome {
            message: "Could not confirm the main turn stopped.".into(),
            can_continue: false,
        });
        assert!(task.turn == Turn::StopUnconfirmed);
        assert_eq!(task.dialogs[1].draft, "keep this draft");
        assert!(!task.send_follow_up());
        task.receive(Event::Expired("late".into()));
        task.receive(Event::StopOutcome {
            message: "Main turn stopped.".into(),
            can_continue: true,
        });
        assert_eq!(task.dialogs.len(), 1);
        assert!(task.turn == Turn::Ready);
        assert_eq!(task.issue.as_deref(), Some("Main turn stopped."));
        assert!(task.dialogs[0].status == Status::Uncertain);
    }

    #[test]
    fn acknowledgement_clears_only_a_submitted_follow_up_and_loss_preserves_its_draft() {
        let mut task = Task {
            directory: "A".into(),
            session: None,
            dialogs: vec![],
            issue: None,
            initial_handled: false,
            turn: Turn::Working,
            follow_up: "another prompt".into(),
            awaiting_prompt_receipt: false,
        };
        task.receive(Event::PromptAccepted);
        assert_eq!(task.follow_up, "another prompt");
        task.awaiting_prompt_receipt = true;
        task.receive(Event::TurnFailed("Rejected.".into()));
        assert_eq!(task.follow_up, "another prompt");
        task.awaiting_prompt_receipt = true;
        task.receive(Event::PromptAccepted);
        assert!(task.follow_up.is_empty());
        task.follow_up = "keep this".into();
        task.awaiting_prompt_receipt = true;
        task.receive(Event::Closed("Lost contact.".into()));
        task.issue = None;
        assert!(
            task.needs_attention(),
            "dismissing the status must keep the draft"
        );
        assert_eq!(task.follow_up, "keep this");
    }

    #[test]
    fn expiry_of_the_last_question_returns_keyboard_focus_without_closing_a_new_task() {
        let ctx = egui::Context::default();
        let editor = egui::Id::new("review-note");
        let mut state = GentleTasks {
            surface: Some(Surface::Questions),
            previous_focus: Some(editor),
            ..Default::default()
        };
        let mut task = Task {
            directory: "B".into(),
            session: None,
            dialogs: vec![],
            issue: None,
            initial_handled: false,
            turn: Turn::Working,
            follow_up: String::new(),
            awaiting_prompt_receipt: false,
        };
        for id in ["one", "two"] {
            task.receive(Event::Question(Question {
                id: id.into(),
                title: "Pick".into(),
                kind: Kind::Select(vec!["a".into()]),
                deadline: None,
            }));
        }
        state.sessions.insert(1, task);
        state
            .sessions
            .get_mut(&1)
            .unwrap()
            .receive(Event::Expired("one".into()));
        state.prune(&ctx);
        assert!(matches!(state.surface, Some(Surface::Questions)));
        state
            .sessions
            .get_mut(&1)
            .unwrap()
            .receive(Event::Expired("two".into()));
        state.prune(&ctx);
        assert!(state.surface.is_none());
        assert_eq!(ctx.memory(|m| m.focused()), Some(editor));
        assert!(state.blocks(&ctx), "dismissal frame must still claim input");
        state.surface = Some(Surface::Compose {
            cwd: None,
            prompt: "unfinished".into(),
            focus: false,
            error: String::new(),
        });
        state.prune(&ctx);
        assert!(
            matches!(state.surface,Some(Surface::Compose{prompt,..}) if prompt == "unfinished")
        );
    }

    #[cfg(unix)]
    #[test]
    fn reviewer_answers_the_background_session_at_the_same_visible_diff_line() {
        use egui::{Event as Input, Pos2};
        use std::{
            fs,
            os::unix::fs::PermissionsExt,
            process::Command,
            time::{Duration, Instant},
        };

        fn frame(
            window: &mut Window,
            ctx: &egui::Context,
            events: Vec<Input>,
        ) -> Vec<(String, Pos2)> {
            let mut host = eframe::Frame::_new_kittest();
            eframe::App::logic(window, ctx, &mut host);
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(1260., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| eframe::App::ui(window, ui, &mut host),
            );
            let labels = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    let egui::epaint::Shape::Text(text) = &shape.shape else {
                        return None;
                    };
                    let point = text.pos + text.galley.rect.center().to_vec2();
                    shape
                        .clip_rect
                        .contains(point)
                        .then(|| (text.galley.job.text.clone(), point))
                })
                .collect();
            output.drop_without_applying_deltas();
            labels
        }
        fn click(window: &mut Window, ctx: &egui::Context, pos: Pos2) {
            frame(window, ctx, vec![Input::PointerMoved(pos)]);
            for pressed in [true, false] {
                frame(
                    window,
                    ctx,
                    vec![Input::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    }],
                );
            }
        }
        fn wait(window: &mut Window, ctx: &egui::Context, label: &str) -> Vec<(String, Pos2)> {
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                let labels = frame(window, ctx, vec![]);
                if labels.iter().any(|(text, _)| text.contains(label)) {
                    return labels;
                }
                assert!(
                    Instant::now() < deadline,
                    "could not see {label}: {labels:?}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        fn point(labels: &[(String, Pos2)], s: &str) -> Pos2 {
            labels
                .iter()
                .find(|(t, _)| t.contains(s))
                .unwrap_or_else(|| panic!("missing {s}: {labels:?}"))
                .1
        }
        struct Dir(PathBuf);
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let root = Dir(std::env::temp_dir().join(format!(
            "ivy-gentle-review-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        )));
        fs::create_dir(&root.0).unwrap();
        let review = root.0.join("review");
        fs::create_dir(&review).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(&review)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}")
        };
        git(&["init", "--quiet"]);
        let original = (0..100).map(|i| format!("line {i}\n")).collect::<String>();
        fs::write(review.join("a-first"), "original\n").unwrap();
        fs::write(review.join("b-review"), &original).unwrap();
        git(&["add", "."]);
        git(&[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "prepare",
        ]);
        fs::write(review.join("a-first"), "modified\n").unwrap();
        let many = (0..50)
            .map(|i| format!("first change {i}\n"))
            .collect::<String>();
        fs::write(
            review.join("b-review"),
            original
                .replace("line 3\n", &many)
                .replace("line 92\n", "review anchor at second hunk\n"),
        )
        .unwrap();
        // The shell reports its fixture cwd for hosts without native PTY cwd discovery.
        let repo = review.to_str().unwrap().replace('\'', "'\"'\"'");
        let shell = format!(
            "cd '{repo}' && printf '\\033]7;file://localhost%s\\007' \"$PWD\" && exec sleep 120"
        );
        let mut window = crate::tests::window_of_tabs([
            [shell.as_str(), "sleep 120"],
            ["sleep 120", "sleep 120"],
        ]);
        let ctx = egui::Context::default();
        window.chrome.enable();
        window.gentle_enabled = true;
        window.sidebar.set_height(550.);
        let pane = window.active().focus;
        let deadline = Instant::now() + Duration::from_secs(5);
        while crate::viewer::console_directory(&mut window, pane).as_deref()
            != Some(review.as_path())
        {
            frame(&mut window, &ctx, vec![]);
            assert!(Instant::now() < deadline, "review project directory");
            std::thread::sleep(Duration::from_millis(5));
        }
        crate::viewer::open_diff_for_pane(&mut window, pane);
        wait(&mut window, &ctx, "b-review");
        for _ in 0..8 {
            frame(&mut window, &ctx, vec![]);
        }
        let labels = frame(&mut window, &ctx, vec![]);
        click(&mut window, &ctx, point(&labels, "b-review"));
        let labels = wait(&mut window, &ctx, "first change");
        let line = point(&labels, "first change");
        assert!(
            !labels
                .iter()
                .any(|(s, _)| s.contains("review anchor at second hunk"))
        );
        frame(
            &mut window,
            &ctx,
            vec![
                Input::PointerMoved(line),
                Input::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(0., -4000.),
                    modifiers: Default::default(),
                },
            ],
        );
        let labels = wait(&mut window, &ctx, "review anchor at second hunk");
        let mut anchor = point(&labels, "review anchor at second hunk");
        for _ in 0..30 {
            anchor = point(
                &frame(&mut window, &ctx, vec![]),
                "review anchor at second hunk",
            );
        }

        let exe = root.0.join("gentle-fixture");
        fs::write(&exe, r#"#!/usr/bin/env python3
import atexit, json, os, pathlib, signal, sys, time
file = pathlib.Path
atexit.register(lambda: file("retired").touch())
signal.signal(signal.SIGTERM, lambda *_: sys.exit())
schema = "gentle-shell.desktop-rpc/v1"
def emit(**value):
    print(json.dumps(value), flush=True)
def reply(message, data):
    emit(type="response", id=message["id"], command=message["type"], success=True, data=data)
def question(title, choice, dialog_id="same-id"):
    emit(type="extension_ui_request", id=dialog_id, method="select", title=title, options=[choice])
for line in sys.stdin:
    message = json.loads(line)
    kind = message["type"]
    if kind == "extension_ui_response":
        file("receipt").write_text(line)
        emit(type="agent_settled", aborted=False)
    elif kind == "get_commands":
        reply(message, {"commands":[{"name":"gentle:desktop","source":"extension"}]})
    elif kind == "abort":
        until = time.monotonic() + 8
        while not file("allow-stop").exists() and time.monotonic() < until:
            time.sleep(0.01)
        assert file("allow-stop").exists()
        file("aborted").write_text(str(os.getpid()))
        emit(type="agent_settled", aborted=True)
        reply(message, {})
    elif kind == "get_state":
        reply(message, {"isStreaming":False, "isCompacting":False, "pendingMessageCount":0})
    elif kind == "prompt":
        prompt = message["message"]
        if prompt.startswith("/gentle:desktop "):
            request = json.loads(prompt.split(" ",1)[1])
            values = {"capabilities":{"methods":["list_subagents","cancel_subagent","list_jobs","cancel_job"]},
                      "list_subagents":{"agents":[]}, "list_jobs":{"jobs":[]}}
            envelope = dict(schema=schema, kind="response", requestId=request["requestId"],
                            ok=True, data=values[request["method"]])
            emit(type="extension_ui_request", method="setWidget", widgetKey="gentle-desktop-rpc",
                 widgetLines=[json.dumps(envelope)])
            reply(message, {"disposition":"handled"})
        elif prompt == "ask":
            reply(message, {})
            file("initial").write_text(str(os.getpid()))
            question("Continue?", "Proceed")
            if file.cwd().name == "A":
                emit(type="agent_settled", aborted=False)
        else:
            reply(message, {})
            file("follow-up").write_text(str(os.getpid())+" "+prompt)
            question("Resume this assignment?", "Follow-up choice", "follow-up-id")
"#).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
        let add_task = |window: &mut Window, id: u64, name: &str| {
            let cwd = root.0.join(name);
            fs::create_dir(&cwd).unwrap();
            let session =
                Session::start(exe.clone(), cwd.clone(), "ask".into(), Arc::new(|| {})).unwrap();
            window.chrome.gentle_task.sessions.insert(
                id,
                Task {
                    directory: cwd,
                    session: Some(session),
                    dialogs: vec![],
                    issue: None,
                    initial_handled: false,
                    turn: Turn::Working,
                    follow_up: String::new(),
                    awaiting_prompt_receipt: false,
                },
            );
        };
        add_task(&mut window, 1, "A");
        add_task(&mut window, 2, "B");
        let labels = wait(&mut window, &ctx, "2 Gentle questions");
        assert_eq!(point(&labels, "review anchor at second hunk"), anchor);
        click(&mut window, &ctx, point(&labels, "Gentle question"));
        wait(&mut window, &ctx, "Stop task");
        for _ in 0..8 {
            frame(&mut window, &ctx, vec![]);
        }
        let labels = frame(&mut window, &ctx, vec![]);
        assert!(window.chrome.gentle_task.sessions[&1].turn == Turn::Ready);
        click(&mut window, &ctx, point(&labels, "Stop task"));
        let labels = wait(&mut window, &ctx, "Stopping task");
        let first_row = point(&labels, "Session #1").y;
        let second_row = labels
            .iter()
            .find(|(s, _)| s.contains("Session #2"))
            .map_or(f32::INFINITY, |(_, xy)| xy.y);
        assert!(
            !labels
                .iter()
                .any(|(s, xy)| (s == "Stop task" || s == "Proceed")
                    && (first_row..second_row).contains(&xy.y))
        );
        fs::write(root.0.join("A/allow-stop"), "").unwrap();
        wait(&mut window, &ctx, "Send follow-up");
        for _ in 0..8 {
            frame(&mut window, &ctx, vec![]);
        }
        let labels = frame(&mut window, &ctx, vec![]);
        assert!(!root.0.join("A/retired").exists());
        assert!(!root.0.join("B/aborted").exists());
        click(
            &mut window,
            &ctx,
            point(&labels, "Continue this conversation"),
        );
        let labels = frame(
            &mut window,
            &ctx,
            vec![Input::Text("second assignment".into())],
        );
        assert_eq!(
            window.chrome.gentle_task.sessions[&1].follow_up, "second assignment",
            "the selected session must own the typed draft"
        );
        click(&mut window, &ctx, point(&labels, "Send follow-up"));
        assert!(
            window.chrome.gentle_task.surface.is_none(),
            "the selected session must accept the follow-up click"
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !root.0.join("A/follow-up").exists() {
            frame(&mut window, &ctx, vec![]);
            assert!(Instant::now() < deadline, "A never received the follow-up");
            std::thread::sleep(Duration::from_millis(10));
        }
        let pid = fs::read_to_string(root.0.join("A/initial")).unwrap();
        assert_eq!(
            fs::read_to_string(root.0.join("A/follow-up")).unwrap(),
            format!("{pid} second assignment")
        );
        assert!(!root.0.join("B/follow-up").exists());
        let labels = wait(&mut window, &ctx, "2 Gentle questions");
        click(&mut window, &ctx, point(&labels, "Gentle questions"));
        wait(&mut window, &ctx, "Follow-up choice");
        for _ in 0..8 {
            frame(&mut window, &ctx, vec![]);
        }
        let labels = frame(&mut window, &ctx, vec![]);
        click(&mut window, &ctx, point(&labels, "Close session"));
        wait(&mut window, &ctx, "Proceed");
        for _ in 0..8 {
            frame(&mut window, &ctx, vec![]);
        }
        let labels = frame(&mut window, &ctx, vec![]);
        assert!(!labels.iter().any(|(s, _)| s.contains("Session #1")));
        click(&mut window, &ctx, point(&labels, "Proceed"));
        let receipt = root.0.join("B/receipt");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !receipt.is_file() || fs::read(&receipt).unwrap().is_empty() {
            frame(&mut window, &ctx, vec![]);
            assert!(Instant::now() < deadline, "B did not receive its choice");
            std::thread::sleep(Duration::from_millis(5));
        }
        let reply: serde_json::Value = serde_json::from_slice(&fs::read(receipt).unwrap()).unwrap();
        assert_eq!(reply["id"], "same-id");
        assert_eq!(reply["value"], "Proceed");
        assert!(!root.0.join("A/receipt").exists());
        let labels = frame(&mut window, &ctx, vec![]);
        assert_eq!(point(&labels, "review anchor at second hunk"), anchor);
        assert_eq!(window.active, 0);
        assert_eq!(window.active().focus, pane);
        add_task(&mut window, 3, "C");
        wait(&mut window, &ctx, "Gentle question");
        crate::settings::interpret("gentle.enabled = off")
            .unwrap()
            .0
            .apply_to(&mut window);
        let labels = frame(&mut window, &ctx, vec![]);
        assert!(!labels.iter().any(|(t, _)| t.contains("Gentle question")));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !root.0.join("C/retired").exists() {
            assert!(Instant::now() < deadline, "disabling Gentle left C running");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!root.0.join("C/receipt").exists());
    }

    crate::windows_unix_omission!(
        reviewer_answers_the_background_session_at_the_same_visible_diff_line,
        "unix-only: uses executable shebang fixtures and POSIX shell commands for a terminal directory witness"
    );
}
