//! Each native task owns the pipes of the client that accepted its prompt.
mod process;
mod wire;

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Instant;

pub(super) const MAX_TEXT: usize = 64 * 1024;
const MAX_DIALOGS: usize = 64;

#[derive(Clone, Debug)]
pub(super) struct Question {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) kind: Kind,
    pub(super) deadline: Option<Instant>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Kind {
    Select(Vec<String>),
    Input(String),
    Confirm(String),
    Editor(String),
}

#[derive(Debug)]
pub(super) enum Answer {
    Value(String),
    Confirm(bool),
    Cancel,
}

#[derive(Debug)]
pub(super) enum Event {
    Question(Question),
    Sent(String),
    Uncertain(String),
    Expired(String),
    Completed,
    Closed(String),
}

enum Message {
    Submit(String, Answer),
    Incoming(wire::Incoming),
    Written(String),
    TooLate(String),
    Failed(String),
}

type Wake = Arc<dyn Fn() + Send + Sync>;
type Inbox = mpsc::SyncSender<Message>;

struct Worker {
    stop: mpsc::Sender<()>,
    handle: thread::JoinHandle<()>,
}

static WORKERS: Mutex<Vec<Worker>> = Mutex::new(Vec::new());

/// Called after the event loop releases its windows: process cleanup can finish before main exits.
pub(crate) fn shutdown() {
    let workers = std::mem::take(&mut *WORKERS.lock().unwrap_or_else(|e| e.into_inner()));
    for worker in &workers {
        let _ = worker.stop.send(());
    }
    for worker in workers {
        let _ = worker.handle.join();
    }
}

pub(super) struct Session {
    inbox: Inbox,
    events: mpsc::Receiver<Event>,
    stop: mpsc::Sender<()>,
    available: BTreeMap<String, Question>,
}

impl Session {
    pub(super) fn start(
        program: PathBuf,
        cwd: PathBuf,
        task: String,
        wake: Wake,
    ) -> io::Result<Self> {
        if !program.is_absolute()
            || !cwd.is_absolute()
            || task.trim().is_empty()
            || task.len() > MAX_TEXT
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid Gentle client, project or task",
            ));
        }
        let (inbox, requests) = mpsc::sync_channel(16);
        let (send, events) = mpsc::sync_channel(16);
        let (stop, stopped) = mpsc::channel();
        let sender = inbox.clone();
        let handle = thread::Builder::new()
            .name("gentle-task".into())
            .spawn(move || {
                let emit = |event| {
                    let delivered = send.send(event).is_ok();
                    if delivered {
                        wake();
                    }
                    delivered
                };
                if let Err(reason) =
                    process::run(program, cwd, task, sender, requests, stopped, &emit)
                {
                    emit(Event::Closed(reason));
                }
            })?;
        let mut workers = WORKERS.lock().unwrap_or_else(|e| e.into_inner());
        workers.retain(|worker| !worker.handle.is_finished());
        workers.push(Worker {
            stop: stop.clone(),
            handle,
        });
        Ok(Self {
            inbox,
            events,
            stop,
            available: BTreeMap::new(),
        })
    }

    pub(super) fn try_event(&mut self) -> Option<Event> {
        let event = self.events.try_recv().ok()?;
        match &event {
            Event::Question(q) => {
                self.available.insert(q.id.clone(), q.clone());
            }
            Event::Expired(id) => {
                self.available.remove(id);
            }
            Event::Completed | Event::Closed(_) => self.available.clear(),
            _ => {}
        }
        Some(event)
    }

    pub(super) fn reply(&mut self, id: &str, answer: Answer) -> Result<(), &'static str> {
        let q = self
            .available
            .get(id)
            .ok_or("This Gentle question is no longer available.")?;
        if !q.accepts(&answer) {
            return Err("This answer does not match the current Gentle question.");
        }
        self.inbox
            .try_send(Message::Submit(id.into(), answer))
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => "Gentle is busy. Try again.",
                mpsc::TrySendError::Disconnected(_) => "The Gentle session has ended.",
            })?;
        self.available.remove(id);
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

impl Question {
    fn accepts(&self, answer: &Answer) -> bool {
        if self
            .deadline
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            return false;
        }
        match (&self.kind, answer) {
            (_, Answer::Cancel) | (Kind::Confirm(_), Answer::Confirm(_)) => true,
            (Kind::Select(options), Answer::Value(value)) => options.contains(value),
            (Kind::Input(_) | Kind::Editor(_), Answer::Value(value)) => value.len() <= MAX_TEXT,
            _ => false,
        }
    }
}

struct Dialogs {
    seen: BTreeSet<String>,
    pending: BTreeMap<String, Question>,
    writing: BTreeSet<String>,
}

impl Dialogs {
    fn new() -> Self {
        Self {
            seen: BTreeSet::new(),
            pending: BTreeMap::new(),
            writing: BTreeSet::new(),
        }
    }

    fn register(&mut self, q: Question) -> Result<bool, String> {
        if self.seen.contains(&q.id) {
            if self
                .pending
                .get(&q.id)
                .is_some_and(|original| original.title != q.title || original.kind != q.kind)
            {
                return Err(
                    "Gentle reused a pending question identifier for a different question.".into(),
                );
            }
            return Ok(false);
        }
        if self.seen.len() >= MAX_DIALOGS {
            return Err("This Gentle task exceeded the 64-question limit.".into());
        }
        self.seen.insert(q.id.clone());
        self.pending.insert(q.id.clone(), q);
        Ok(true)
    }

    fn expire(&mut self, emit: &impl Fn(Event) -> bool) {
        let now = Instant::now();
        self.pending.retain(|id, q| {
            let expired = q.deadline.is_some_and(|deadline| deadline <= now);
            if expired {
                emit(Event::Expired(id.clone()));
            }
            !expired
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn an_expired_or_foreign_choice_is_never_queued() {
        let (inbox, messages) = mpsc::sync_channel(2);
        let (tx, events) = mpsc::sync_channel(2);
        let (stop, _) = mpsc::channel();
        let mut session = Session {
            inbox,
            events,
            stop,
            available: Default::default(),
        };
        let q = Question {
            id: "current".into(),
            title: "Question".into(),
            kind: Kind::Select(vec!["one".into()]),
            deadline: None,
        };
        tx.send(Event::Question(q.clone())).unwrap();
        session.try_event().unwrap();
        assert!(session.reply("other", Answer::Cancel).is_err());
        assert!(
            session
                .reply("current", Answer::Value("two".into()))
                .is_err()
        );
        assert!(messages.try_recv().is_err());
        session
            .reply("current", Answer::Value("one".into()))
            .unwrap();
        assert!(session.reply("current", Answer::Cancel).is_err());
        assert!(
            matches!(messages.try_recv(),Ok(Message::Submit(id,Answer::Value(v))) if id=="current" && v=="one")
        );
        tx.send(Event::Question(Question {
            deadline: Some(Instant::now() - Duration::from_secs(1)),
            ..q
        }))
        .unwrap();
        session.try_event().unwrap();
        assert!(session.reply("current", Answer::Cancel).is_err());
        assert!(messages.try_recv().is_err());
    }

    #[test]
    fn repeats_never_open_a_second_prompt_or_prevent_an_independent_prompt() {
        let mut dialogs = Dialogs::new();
        let first = Question {
            id: "one".into(),
            title: "route".into(),
            kind: Kind::Select(vec!["Left".into()]),
            deadline: None,
        };
        assert!(dialogs.register(first.clone()).unwrap());
        assert!(!dialogs.register(first.clone()).unwrap());
        assert!(
            dialogs
                .register(Question {
                    id: "two".into(),
                    ..first.clone()
                })
                .unwrap()
        );
        assert_eq!(dialogs.pending.len(), 2);
        assert!(
            dialogs
                .register(Question {
                    title: "changed".into(),
                    ..first.clone()
                })
                .is_err()
        );
        dialogs.pending.remove("one");
        assert!(!dialogs.register(first).unwrap());
        assert!(dialogs.pending.contains_key("two"));
        assert!(!dialogs.pending.contains_key("one"));
    }

    crate::windows_unix_omission!(
        separate_rpc_children_receive_only_their_own_choice,
        "unix-only: directly launches an executable Python shebang fixture using private pipes"
    );
    #[cfg(unix)]
    #[test]
    fn separate_rpc_children_receive_only_their_own_choice() {
        use std::fs;
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut unique = [0; 8];
        getrandom::fill(&mut unique).unwrap();
        let root =
            std::env::temp_dir().join(format!("ivy-gentle-rpc-{:x}", u64::from_ne_bytes(unique)));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let program = root.join("fixture");
        fs::write(&program, r#"#!/usr/bin/env python3
import json, os, pathlib, sys
assert sys.argv[1:] == ["--mode", "rpc"]
assert os.environ.get("GENTLE_SHELL_INTERACTIVE_HOST") == "1"
prompt = json.loads(sys.stdin.readline())
assert prompt["type"] == "prompt"
print(json.dumps(dict(type="extension_ui_request",method="select",id="same-id",title="Pick",options=["Left","Right"])),flush=True)
reply = json.loads(sys.stdin.readline())
assert reply["id"] == "same-id" and reply["type"] == "extension_ui_response"
pathlib.Path(prompt["message"]).write_text("cancel" if reply.get("cancelled") else reply["value"])
print(json.dumps(dict(type="agent_settled",aborted=False)),flush=True)
"#).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let start = |name: &str| {
            Session::start(program.clone(), root.clone(), name.into(), Arc::new(|| {})).unwrap()
        };
        let mut left = start("left");
        let mut right = start("right");
        let await_event = |s: &mut Session, completed: bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                match s.try_event() {
                    Some(Event::Closed(why)) => panic!("{why}"),
                    Some(Event::Question(_)) if !completed => return,
                    Some(Event::Completed) if completed => return,
                    _ => thread::sleep(Duration::from_millis(5)),
                }
            }
            panic!("timed out waiting for the Gentle fixture");
        };
        await_event(&mut left, false);
        await_event(&mut right, false);
        right
            .reply("same-id", Answer::Value("Right".into()))
            .unwrap();
        await_event(&mut right, true);
        assert_eq!(fs::read_to_string(root.join("right")).unwrap(), "Right");
        assert!(!root.join("left").exists());
        left.reply("same-id", Answer::Cancel).unwrap();
        await_event(&mut left, true);
        assert_eq!(fs::read_to_string(root.join("left")).unwrap(), "cancel");
        fs::remove_dir_all(root).unwrap();
    }
}
