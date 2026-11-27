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
            Event::Completed => {
                self.session = None;
                self.dialogs.retain(|d| d.status == Status::Uncertain);
            }
            Event::Closed(reason) => self.disconnect(reason),
        }
    }

    fn set_status(&mut self, id: &str, status: Status) {
        if let Some(dialog) = self.dialogs.iter_mut().find(|d| d.question.id == id) {
            dialog.status = status;
        }
    }

    fn disconnect(&mut self, reason: String) {
        self.session = None;
        self.issue = Some(reason);
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

    fn prune(&mut self) {
        self.sessions.retain(|_, task| {
            task.session.is_some() || task.issue.is_some() || !task.dialogs.is_empty()
        });
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
            *error = "Finish a Gentle task or dismiss its status before starting another.".into();
            return;
        }
        if prompt.trim().is_empty() || prompt.len() > transport::MAX_TEXT {
            return;
        }
        let wake = ctx.clone();
        let result = super::gentle::resolve()
            .map_err(|err| err.to_string())
            .and_then(|program| {
                Session::start(
                    program,
                    directory.clone(),
                    prompt.clone(),
                    Arc::new(move || wake.request_repaint()),
                )
                .map_err(|err| err.to_string())
            });
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
    state.prune();
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
        let pos = point(&labels, "b-review");
        eprintln!(
            "click {pos:?}, layer {:?}, expected {:?}",
            ctx.layer_id_at(pos),
            egui::Id::new(("document-viewer", "git-viewer"))
        );
        click(&mut window, &ctx, pos);
        eprintln!(
            "clicked, layer {:?}, frame {}",
            ctx.layer_id_at(pos),
            ctx.cumulative_frame_nr()
        );
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
        fs::write(&exe,r#"#!/bin/sh
trap 'touch retired' EXIT
trap 'exit 0' TERM
IFS= read -r prompt || exit 1
printf '%s\n' '{"type":"extension_ui_request","id":"same-id","method":"select","title":"Continue?","options":["Proceed"]}'
IFS= read -r answer || exit 1
printf '%s\n' "$answer" > receipt
printf '%s\n' '{"type":"agent_settled"}'
"#).unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
        for (id, name) in [(1, "A"), (2, "B")] {
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
                },
            );
        }
        let labels = wait(&mut window, &ctx, "Gentle question");
        assert_eq!(point(&labels, "review anchor at second hunk"), anchor);
        click(&mut window, &ctx, point(&labels, "Gentle question"));
        let labels = wait(&mut window, &ctx, "Session #2");
        let choices: Vec<_> = labels.iter().filter(|(s, _)| s == "Proceed").collect();
        assert_eq!(choices.len(), 2, "{labels:?}");
        click(&mut window, &ctx, choices[1].1);
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
        crate::settings::interpret("gentle.enabled = off")
            .unwrap()
            .0
            .apply_to(&mut window);
        let labels = frame(&mut window, &ctx, vec![]);
        assert!(!labels.iter().any(|(t, _)| t.contains("Gentle question")));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !root.0.join("A/retired").exists() {
            assert!(Instant::now() < deadline, "disabling Gentle left A running");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!root.0.join("A/receipt").exists());
    }

    crate::windows_unix_omission!(
        reviewer_answers_the_background_session_at_the_same_visible_diff_line,
        "unix-only: uses a POSIX shell for private RPC pipes and a terminal directory witness"
    );
}
