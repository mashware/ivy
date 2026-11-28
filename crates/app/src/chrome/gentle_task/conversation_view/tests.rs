use super::*;
use egui::{Event as Input, Pos2};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};

fn frame(window: &mut Window, ctx: &egui::Context, events: Vec<Input>) -> Vec<(String, Pos2)> {
    let mut host = eframe::Frame::_new_kittest();
    eframe::App::logic(window, ctx, &mut host);
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1400., 950.),
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
            let pos = text.pos + text.galley.rect.center().to_vec2();
            shape
                .clip_rect
                .contains(pos)
                .then(|| (text.galley.job.text.clone(), pos))
        })
        .collect();
    output.drop_without_applying_deltas();
    labels
}

fn wait(window: &mut Window, ctx: &egui::Context, label: &str) -> Vec<(String, Pos2)> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let labels = frame(window, ctx, vec![]);
        if labels.iter().any(|(s, _)| s.contains(label)) {
            return labels;
        }
        assert!(Instant::now() < deadline, "missing {label}: {labels:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn point(labels: &[(String, Pos2)], label: &str) -> Pos2 {
    labels
        .iter()
        .find(|(s, _)| s.contains(label))
        .unwrap_or_else(|| panic!("missing {label}: {labels:?}"))
        .1
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

fn task(session: Session, directory: PathBuf) -> Task {
    Task {
        display: session.display(),
        transcript: Default::default(),
        snapshot: Default::default(),
        history_view: Default::default(),
        directory,
        session: Some(session),
        dialogs: vec![],
        issue: None,
        helpers: None,
        initial_handled: false,
        failed: false,
        closed_by_user: false,
        turn: Turn::Working,
        follow_up: String::new(),
        awaiting_prompt_receipt: false,
    }
}

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_transcript_composer_and_neighbor_console_own_their_input_under_flood() {
    let root = Fixture(std::env::temp_dir().join(format!(
        "ivy-native-frame-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    )));
    fs::create_dir_all(&root.0).unwrap();
    let exe = root.0.join("client");
    fs::write(&exe, r#"#!/usr/bin/env python3
import json, pathlib, sys
send=lambda v: print(json.dumps(v),flush=True)
prompt=json.loads(sys.stdin.readline())
send(dict(type="response",id=prompt["id"],command="prompt",success=True,data={}))
if prompt["message"]=="noisy":
 for i in range(640):
  send(dict(type="message_update",assistantMessageEvent=dict(type="text_delta",contentIndex=0,delta="x"*4096)))
 send(dict(type="extension_ui_request",id="same",method="select",title="Noisy question",options=["Answer noisy"]))
else:
 send(dict(type="message_end",message=dict(role="assistant",content=[dict(type="text",text="Visible assistant reply")],stopReason="stop")))
 send(dict(type="agent_settled",aborted=False))
for line in sys.stdin:
 v=json.loads(line)
 if v["type"]=="prompt":
  pathlib.Path("follow-up").write_text(v["message"])
  send(dict(type="response",id=v["id"],command="prompt",success=True,data={}))
  send(dict(type="agent_settled",aborted=False))
 elif v["type"]=="extension_ui_response":
  pathlib.Path("answer").write_text(v["value"])
  send(dict(type="agent_settled",aborted=False))
 elif v["type"]=="get_commands":
  send(dict(type="response",id=v["id"],command="get_commands",success=True,data=dict(commands=[])))
"#).unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    for name in ["quiet", "noisy"] {
        fs::create_dir(root.0.join(name)).unwrap();
    }
    let receiver = root.0.join("console-bytes");
    let shell = format!("stty raw -echo; cat > '{}'", receiver.display());
    let mut window = crate::tests::window_of_tabs([[shell.as_str(), "sleep 120"]]);
    let ctx = egui::Context::default();
    window.chrome.enable();
    window.gentle_enabled = true;
    let start = |name: &str| {
        Session::start(exe.clone(), root.0.join(name), name.into(), Arc::new(|| {})).unwrap()
    };
    window
        .chrome
        .gentle_task
        .sessions
        .insert(1, task(start("quiet"), root.0.join("quiet")));
    window
        .chrome
        .gentle_task
        .sessions
        .insert(2, task(start("noisy"), root.0.join("noisy")));
    window.chrome.gentle_task.selected_conversation = Some(1);
    let labels = wait(&mut window, &ctx, "Visible assistant reply");
    assert!(
        labels.iter().any(|(s, _)| s == "quiet"),
        "initial submitted user text must be visible"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !window.chrome.gentle_task.sessions[&2].transcript.gap
        || window.chrome.gentle_task.sessions[&2].dialogs.is_empty()
    {
        frame(&mut window, &ctx, vec![]);
        assert!(
            Instant::now() < deadline,
            "flood starved question or never evicted"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // A transcript selection owns the press, movement and release outside its window.
    let labels = frame(&mut window, &ctx, vec![]);
    let text = point(&labels, "Visible assistant reply");
    frame(
        &mut window,
        &ctx,
        vec![
            Input::PointerMoved(text),
            Input::PointerButton {
                pos: text,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    let outside = egui::pos2(150., 650.);
    frame(&mut window, &ctx, vec![Input::PointerMoved(outside)]);
    frame(
        &mut window,
        &ctx,
        vec![Input::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    frame(
        &mut window,
        &ctx,
        vec![Input::Text("MUST_NOT_REACH_SHELL".into())],
    );
    assert_eq!(fs::read(&receiver).unwrap_or_default(), b"");
    let labels = wait(&mut window, &ctx, "Continue this conversation");
    click(
        &mut window,
        &ctx,
        point(&labels, "Continue this conversation"),
    );
    let labels = frame(
        &mut window,
        &ctx,
        vec![Input::Text("follow only quiet".into())],
    );
    assert_eq!(
        window.chrome.gentle_task.sessions[&1].follow_up,
        "follow only quiet"
    );
    assert!(window.chrome.gentle_task.sessions[&2].follow_up.is_empty());
    assert_eq!(fs::read(&receiver).unwrap_or_default(), b"");
    click(&mut window, &ctx, point(&labels, "Send follow-up"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !root.0.join("quiet/follow-up").exists() {
        frame(&mut window, &ctx, vec![]);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        fs::read_to_string(root.0.join("quiet/follow-up")).unwrap(),
        "follow only quiet"
    );
    assert!(!root.0.join("noisy/follow-up").exists());
    // An uncovered console takes keyboard input while the conversation stays open.
    click(&mut window, &ctx, egui::pos2(150., 650.));
    frame(&mut window, &ctx, vec![Input::Text("N".into())]);
    let deadline = Instant::now() + Duration::from_secs(5);
    while fs::read(&receiver).unwrap_or_default() != b"N" {
        frame(&mut window, &ctx, vec![]);
        assert!(
            Instant::now() < deadline,
            "neighbor never received its byte"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(window.chrome.gentle_task.selected_conversation, Some(1));
    let labels = frame(&mut window, &ctx, vec![]);
    let title = point(&labels, "Gentle conversation");
    frame(&mut window, &ctx, vec![Input::PointerMoved(title)]);
    frame(
        &mut window,
        &ctx,
        vec![Input::PointerButton {
            pos: title,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    let moved = title - egui::vec2(0., 120.);
    frame(&mut window, &ctx, vec![Input::PointerMoved(moved)]);
    frame(
        &mut window,
        &ctx,
        vec![Input::PointerButton {
            pos: moved,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    let labels = frame(&mut window, &ctx, vec![]);
    let badge = point(&labels, "Gentle question");
    let native = window.chrome.gentle_task.conversation_rect.unwrap();
    assert!(
        native.top() <= 90.,
        "the window must cross the old badge position"
    );
    assert!(
        !native.contains(badge),
        "the question badge obscures the native conversation"
    );
    // The background question remains actionable through the existing modal route.
    let labels = frame(&mut window, &ctx, vec![]);
    click(&mut window, &ctx, point(&labels, "Gentle question"));
    let labels = wait(&mut window, &ctx, "Answer noisy");
    click(&mut window, &ctx, point(&labels, "Answer noisy"));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !root.0.join("noisy/answer").exists() {
        frame(&mut window, &ctx, vec![]);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!root.0.join("quiet/answer").exists());
    assert_eq!(
        fs::read_to_string(root.0.join("noisy/answer")).unwrap(),
        "Answer noisy"
    );
    println!(
        "native workload: 640 text deltas; 2621440 text bytes; cache gap=true; question reached and answered; neighbor receiver=N; follow-up isolated"
    );
    // Closing retains history and draft; explicit discard is the only removal.
    window
        .chrome
        .gentle_task
        .sessions
        .get_mut(&1)
        .unwrap()
        .follow_up = "keep draft".into();
    dispatch(
        &mut window.chrome.gentle_task,
        Some(view::Action::CloseSession(1)),
        &ctx,
    );
    let labels = wait(&mut window, &ctx, "Session closed · retained conversation");
    assert!(
        labels
            .iter()
            .any(|(s, _)| s.contains("Visible assistant reply"))
    );
    assert_eq!(
        window.chrome.gentle_task.sessions[&1].follow_up,
        "keep draft"
    );
    click(&mut window, &ctx, point(&labels, "Discard conversation"));
    assert!(!window.chrome.gentle_task.sessions.contains_key(&1));
    assert!(window.chrome.gentle_task.sessions.contains_key(&2));
    window
        .chrome
        .gentle_task
        .sessions
        .get_mut(&2)
        .unwrap()
        .receive(Event::Closed("Lost connection.".into()));
    window.chrome.gentle_task.selected_conversation = Some(2);
    let labels = wait(&mut window, &ctx, "Disconnected · retained conversation");
    assert!(
        !labels
            .iter()
            .any(|(s, _)| s.contains("Session closed · retained conversation"))
    );
}
