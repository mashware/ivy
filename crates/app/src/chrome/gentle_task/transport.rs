//! Each native task owns the pipes of the client that accepted its prompt.
pub(crate) mod conversation;
mod helpers;
pub(super) mod snapshot;
mod terminal;
use terminal::Terminal;
mod process;
pub(super) use helpers::{Helper, LIMIT as HELPER_LIMIT};
mod wire;

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
    Helpers(HelperList),
    Snapshot(snapshot::SnapshotUpdate),
    PromptAccepted,
    Notice(String),
    Handled,
    Question(Question),
    Sent(String),
    Uncertain(String),
    Expired(String),
    Completed,
    TurnFailed(String),
    Closed(String),
    StopOutcome { message: String, can_continue: bool },
}

enum Message {
    StopTask(Vec<String>),
    FollowUp(String),
    Submit(String, Answer),
    Incoming(wire::Incoming),
    Written(String),
    TooLate(String),
    Failed(String),
}

type Wake = Arc<dyn Fn() + Send + Sync>;
type Inbox = mpsc::SyncSender<Message>;

struct Worker {
    launch_claimed: Arc<AtomicBool>,
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
    // Abandon a slow pre-launch lookup after reserving its slot, so it cannot start a child.
    let active: Vec<_> = workers
        .into_iter()
        .filter(|w| w.launch_claimed.swap(true, Ordering::AcqRel))
        .collect();
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while active.iter().any(|w| !w.handle.is_finished()) {
        if Instant::now() >= deadline {
            crate::pty_output::say(
                "Gentle task cleanup exceeded its deadline. A worker may still be retiring its process.",
            );
            break;
        }
        thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Turn {
    Working,
    Stopping,
    BeforePrompt,
    Ready,
    Disconnected,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct HelperList {
    pub(super) rows: Vec<Helper>,
    pub(super) unavailable: bool,
}

#[derive(Clone)]
struct HelperMailbox {
    slot: Arc<Mutex<Option<HelperList>>>,
    wake: Wake,
}

impl HelperMailbox {
    fn new(wake: Wake) -> Self {
        Self {
            slot: Arc::default(),
            wake,
        }
    }

    fn publish(&self, snapshot: HelperList) {
        let old = self
            .slot
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .replace(snapshot);
        if old.is_none() {
            (self.wake)();
        }
    }

    fn take(&self) -> Option<HelperList> {
        match self.slot.try_lock() {
            Ok(mut slot) => slot.take(),
            Err(std::sync::TryLockError::WouldBlock) => {
                // An update already scheduled a frame; this frame can race its replacement.
                (self.wake)();
                None
            }
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner().take(),
        }
    }
}

struct Initial {
    display: conversation::Display,
    terminal: Terminal,
    helpers: HelperMailbox,
    task: String,
    claimed: Arc<AtomicBool>,
}

pub(super) struct Session {
    display: conversation::Display,
    terminal: Terminal,
    helpers: HelperMailbox,
    closing: Option<String>,
    initial_claimed: Arc<AtomicBool>,
    turn: Turn,
    inbox: Inbox,
    events: mpsc::Receiver<Event>,
    stop: mpsc::Sender<()>,
    available: BTreeMap<String, Question>,
}

impl Session {
    pub(super) fn start_installed(cwd: PathBuf, task: String, wake: Wake) -> io::Result<Self> {
        Self::start_with(cwd, task, wake, || {
            super::super::gentle::resolve().map_err(|e| e.to_string())
        })
    }

    #[cfg(all(test, unix))]
    pub(super) fn start(
        program: PathBuf,
        cwd: PathBuf,
        task: String,
        wake: Wake,
    ) -> io::Result<Self> {
        Self::start_with(cwd, task, wake, move || Ok(program))
    }

    fn start_with(
        cwd: PathBuf,
        task: String,
        wake: Wake,
        resolve: impl FnOnce() -> Result<PathBuf, String> + Send + 'static,
    ) -> io::Result<Self> {
        if !cwd.is_absolute() || task.trim().is_empty() || task.len() > MAX_TEXT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid Gentle client, project or task",
            ));
        }
        let (inbox, requests) = mpsc::sync_channel(16);
        let (send, events) = mpsc::sync_channel(16);
        let (stop, stopped) = mpsc::channel();
        let sender = inbox.clone();
        let mut workers = WORKERS.lock().unwrap_or_else(|e| e.into_inner());
        workers.retain(|worker| !worker.handle.is_finished());
        if workers.len() >= 16 {
            return Err(io::Error::other(
                "Gentle is still starting or retiring other tasks. Try again.",
            ));
        }
        let launch_claimed = Arc::new(AtomicBool::new(false));
        let worker_claim = launch_claimed.clone();
        let initial_claimed = Arc::new(AtomicBool::new(false));
        let helpers = HelperMailbox::new(wake.clone());
        let display = conversation::Display::new(wake.clone());
        display.user(&task);
        let terminal = Terminal::new(wake.clone());
        let worker_terminal = terminal.clone();
        let initial = Initial {
            display: display.clone(),
            terminal: terminal.clone(),
            helpers: helpers.clone(),
            task,
            claimed: initial_claimed.clone(),
        };
        let handle = thread::Builder::new()
            .name("gentle-task".into())
            .spawn(move || {
                let emit = |event| {
                    if let Event::Closed(reason) = event {
                        worker_terminal.fail(reason);
                        return false;
                    }
                    match send.try_send(event) {
                        Ok(()) => {
                            wake();
                            true
                        }
                        Err(_) => {
                            worker_terminal.fail(
                                "Gentle control backlog exceeded its limit or closed.".into(),
                            );
                            false
                        }
                    }
                };
                if stopped.try_recv().is_ok() {
                    return;
                }
                let result = resolve().and_then(|program| {
                    if worker_claim.swap(true, Ordering::AcqRel) {
                        return Ok(());
                    }
                    if !program.is_absolute() {
                        return Err("Gentle needs an absolute executable path.".into());
                    }
                    process::run(program, cwd, initial, sender, requests, stopped, &emit)
                });
                if let Err(reason) = result {
                    emit(Event::Closed(reason));
                }
            })?;
        workers.push(Worker {
            launch_claimed,
            stop: stop.clone(),
            handle,
        });
        Ok(Self {
            display,
            terminal,
            helpers,
            closing: None,
            initial_claimed,
            turn: Turn::Working,
            inbox,
            events,
            stop,
            available: BTreeMap::new(),
        })
    }

    pub(super) fn display(&self) -> conversation::Display {
        self.display.clone()
    }

    pub(super) fn request_stop(&mut self) -> Result<(), String> {
        if matches!(self.turn, Turn::Stopping | Turn::BeforePrompt) {
            return Ok(());
        }
        if self.turn == Turn::Disconnected {
            return Err("The Gentle session has ended.".into());
        }
        // The launcher can still start the empty conversation after a slow lookup.
        if !self.initial_claimed.swap(true, Ordering::AcqRel) {
            self.turn = Turn::BeforePrompt;
            return Ok(());
        }
        self.enqueue(Message::StopTask(self.available.keys().cloned().collect()))?;
        self.turn = Turn::Stopping;
        Ok(())
    }

    pub(super) fn follow_up(&mut self, message: &str) -> Result<(), String> {
        if self.turn != Turn::Ready {
            return Err("Gentle has not confirmed it can accept the next prompt.".into());
        }
        if message.trim().is_empty() || message.len() > MAX_TEXT {
            return Err("Enter a nonempty prompt within the size limit.".into());
        }
        self.enqueue(Message::FollowUp(message.to_owned()))?;
        self.turn = Turn::Working;
        Ok(())
    }

    fn enqueue(&self, message: Message) -> Result<(), String> {
        self.inbox.try_send(message).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => "Gentle is busy. Try again.".into(),
            mpsc::TrySendError::Disconnected(_) => "The Gentle session has ended.".into(),
        })
    }

    pub(super) fn try_event(&mut self) -> Option<Event> {
        if self.turn == Turn::Disconnected {
            return None;
        }
        let event = loop {
            if let Some(reason) = self.closing.take() {
                break Event::Closed(reason);
            }
            match self.events.try_recv() {
                // A queued natural completion can race the click. The supervisor sends at most
                // one terminal turn event before the next prompt is accepted.
                Ok(Event::Completed | Event::TurnFailed(_)) if self.turn == Turn::Stopping => continue,
                Ok(Event::Closed(reason)) => self.closing = Some(reason),
                Ok(event) => break event,
                Err(mpsc::TryRecvError::Disconnected) =>
                    self.closing = Some(self.terminal.take().unwrap_or_else(|| "The Gentle connection closed.".into())),
                Err(mpsc::TryRecvError::Empty) if self.terminal.failed() => {
                    self.closing = self.terminal.take();
                    continue;
                }
                Err(mpsc::TryRecvError::Empty) if self.turn == Turn::BeforePrompt =>
                    break Event::StopOutcome {
                        message: "The initial prompt was never sent. Follow-ups will use this session when Gentle starts.".into(),
                        can_continue: true,
                    },
                Err(mpsc::TryRecvError::Empty) => return self.helpers.take().map(Event::Helpers),
            }
            // The supervisor has published its final snapshot before sending the disconnect.
            if let Some(snapshot) = self.helpers.take() {
                return Some(Event::Helpers(snapshot));
            }
        };
        match &event {
            Event::Question(q) => {
                self.available.insert(q.id.clone(), q.clone());
            }
            Event::Expired(id) => {
                self.available.remove(id);
            }
            Event::Completed | Event::TurnFailed(_) => self.turn = Turn::Ready,
            Event::StopOutcome { can_continue, .. } => {
                self.turn = if *can_continue {
                    Turn::Ready
                } else {
                    Turn::Working
                };
            }
            Event::Closed(_) => {
                self.turn = Turn::Disconnected;
                self.available.clear();
            }
            _ => {}
        }
        Some(event)
    }

    pub(super) fn reply(&mut self, id: &str, answer: Answer) -> Result<(), &'static str> {
        if matches!(
            self.turn,
            Turn::BeforePrompt | Turn::Stopping | Turn::Disconnected
        ) {
            return Err("The Gentle question cannot be answered now.");
        }
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
    visible_at_stop: Vec<String>,
    seen: BTreeSet<String>,
    pending: BTreeMap<String, Question>,
    writing: BTreeSet<String>,
}

impl Dialogs {
    fn new() -> Self {
        Self {
            visible_at_stop: Vec::new(),
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
            return Err("This Gentle session reached its 64-question limit. Start a new Gentle session to continue.".into());
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
    fn client_lookup_runs_outside_the_frame_and_delivers_its_failure() {
        let frame_thread = thread::current().id();
        let (woke, wake) = mpsc::channel();
        let mut session = Session::start_with(
            std::env::temp_dir(),
            "task".into(),
            Arc::new(move || {
                let _ = woke.send(());
            }),
            move || {
                assert_ne!(thread::current().id(), frame_thread);
                Err("No installed client".into())
            },
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(Event::Closed(reason)) = session.try_event() {
                assert_eq!(reason, "No installed client");
                break;
            }
            wake.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
        }
    }

    #[test]
    fn question_precedes_a_burst_and_disconnect_follows_its_last_snapshot() {
        use std::sync::atomic::AtomicUsize;
        let requests = Arc::new(AtomicUsize::new(0));
        let count = requests.clone();
        let helpers = HelperMailbox::new(Arc::new(move || {
            count.fetch_add(1, Ordering::Relaxed);
        }));
        let (inbox, _) = mpsc::sync_channel(2);
        let (tx, events) = mpsc::sync_channel(2);
        let (stop, _) = mpsc::channel();
        let mut session = Session {
            display: conversation::Display::new(Arc::new(|| {})),
            terminal: Terminal::new(Arc::new(|| {})),
            helpers: helpers.clone(),
            closing: None,
            initial_claimed: Arc::new(AtomicBool::new(true)),
            turn: Turn::Working,
            inbox,
            events,
            stop,
            available: Default::default(),
        };
        for n in 0..512 {
            helpers.publish(HelperList {
                rows: vec![Helper {
                    id: n.to_string(),
                    name: "Reviewer".into(),
                    state: "running",
                }],
                unavailable: n == 511,
            });
        }
        tx.send(Event::Question(Question {
            id: "pick".into(),
            title: "Pick".into(),
            kind: Kind::Select(vec!["One".into()]),
            deadline: None,
        }))
        .unwrap();
        tx.send(Event::Closed("Lost".into())).unwrap();
        assert_eq!(requests.load(Ordering::Relaxed), 1);
        assert!(matches!(session.try_event(), Some(Event::Question(q)) if q.id == "pick"));
        assert!(
            matches!(session.try_event(), Some(Event::Helpers(list)) if list.unavailable && list.rows[0].id == "511")
        );
        assert!(matches!(session.try_event(), Some(Event::Closed(reason)) if reason == "Lost"));
        assert!(session.try_event().is_none());
    }

    #[test]
    fn an_expired_or_foreign_choice_is_never_queued() {
        let (inbox, messages) = mpsc::sync_channel(2);
        let (tx, events) = mpsc::sync_channel(2);
        let (stop, _) = mpsc::channel();
        let mut session = Session {
            display: conversation::Display::new(Arc::new(|| {})),
            terminal: Terminal::new(Arc::new(|| {})),
            helpers: HelperMailbox::new(Arc::new(|| {})),
            closing: None,
            initial_claimed: Arc::new(AtomicBool::new(true)),
            turn: Turn::Working,
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
import json, os, pathlib, signal, sys
assert sys.argv[1:] == ["--mode", "rpc"]
assert os.environ.get("GENTLE_SHELL_INTERACTIVE_HOST") == "1"
prompt = json.loads(sys.stdin.readline())
assert prompt["type"] == "prompt"
if prompt["message"] == "persistent":
    def send(event):
        print(json.dumps(event), flush=True)
    def ack(command, data):
        send(dict(type="response", id=command["id"], command=command["type"], success=True, data=data))
    def record(name, message):
        with pathlib.Path(name).open("a") as f:
            f.write(str(message) + "\n")
    def ask(dialog):
        send(dict(type="extension_ui_request", id=dialog, method="select", title="Work?",options=["Yes"]))
    record("prompt-pids", os.getpid())
    ack(prompt, dict(disposition="started"))
    ask("initial-dialog")
    for line in sys.stdin:
        command = json.loads(line)
        kind = command["type"]
        if kind == "abort":
            record("aborts", command["id"])
            ask("late-" + command["id"])
            ack(command, {})
        elif kind == "extension_ui_response":
            assert command["id"].startswith("late-")
            record("late-answers", command["value"])
        elif kind == "get_state":
            ack(command, dict(isStreaming=False, isCompacting=False, pendingMessageCount=0))
        elif kind == "get_commands":
            ack(command, dict(commands=[dict(name="gentle:desktop", source="extension")]))
        elif kind == "prompt" and command["message"].startswith("/gentle:desktop "):
            inner = json.loads(command["message"].split(" ",1)[1])
            method = inner["method"]
            data = {"capabilities":dict(methods=["list_subagents","cancel_subagent","list_jobs","cancel_job"]),
                    "list_subagents":dict(agents=[]),"list_jobs":dict(jobs=[])}[method]
            if not inner["requestId"].startswith("ivy-helpers-"):
                record("desktop-methods", method)
            receipt = dict(schema=inner["schema"],kind="response",requestId=inner["requestId"],ok=True,data=data)
            send(dict(type="extension_ui_request", method="setWidget", widgetKey="gentle-desktop-rpc",widgetLines=[json.dumps(receipt)]))
        else:
            assert kind == "prompt" and command["message"] in ("next", "finish")
            record("prompt-pids", os.getpid())
            record("user-prompts", command["message"])
            send(dict(type="response", id="ivy-follow-up-99", command="prompt",success=False))
            send(dict(type="agent_settled",aborted=False))
            ack(command, dict(disposition="started"))
            if command["message"] == "finish":
                send(dict(type="agent_settled",aborted=False))
            else:
                ask("next-dialog")
    sys.exit(0)
print(json.dumps(dict(type="response", id=prompt["id"], command="prompt", success=True, data=dict(disposition="handled"))), flush=True)
def retired(*_):
    pathlib.Path("ended").write_text("closed without replying")
    sys.exit(0)
if prompt["message"] == "retire":
    signal.signal(signal.SIGTERM, retired)
print(json.dumps(dict(type="extension_ui_request",method="select",id="same-id",title="Pick",options=["Left","Right"])),flush=True)
line = sys.stdin.readline()
while line and json.loads(line)["type"] == "get_commands":
    command = json.loads(line)
    print(json.dumps(dict(type="response", command="get_commands", id=command["id"], success=True, data=dict(commands=[]))),flush=True)
    line = sys.stdin.readline()
if prompt["message"] == "retire" and not line:
    retired()
reply = json.loads(line)
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
        let (lookup_entered, entered) = mpsc::channel();
        let (allow_lookup, lookup) = mpsc::channel();
        let executable = program.clone();
        let mut deferred = Session::start_with(
            root.clone(),
            "must never run".into(),
            Arc::new(|| {}),
            move || {
                lookup_entered.send(()).unwrap();
                lookup.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(executable)
            },
        )
        .unwrap();
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        deferred.request_stop().unwrap();
        assert!(
            matches!(deferred.try_event(), Some(Event::StopOutcome {can_continue:true,message}) if message.contains("never sent"))
        );
        deferred.follow_up("first after lookup").unwrap();
        allow_lookup.send(()).unwrap();
        await_event(&mut deferred, false);
        deferred
            .reply("same-id", Answer::Value("Left".into()))
            .unwrap();
        await_event(&mut deferred, true);
        assert_eq!(
            fs::read_to_string(root.join("first after lookup")).unwrap(),
            "Left"
        );
        assert!(!root.join("must never run").exists());

        let mut kept = start("persistent");
        let mut neighbor = start("neighbor");
        await_event(&mut neighbor, false);
        let next = |session: &mut Session| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(event) = session.try_event() {
                    return event;
                }
                assert!(Instant::now() < deadline, "missing session event");
                thread::sleep(Duration::from_millis(5));
            }
        };
        for prompt in ["next", "finish"] {
            assert!(matches!(next(&mut kept), Event::PromptAccepted));
            let Event::Question(prior) = next(&mut kept) else {
                panic!("missing active question");
            };
            kept.request_stop().unwrap();
            kept.request_stop().unwrap();
            assert!(kept.follow_up("too soon").is_err());
            let Event::Question(late) = next(&mut kept) else {
                panic!("missing late question");
            };
            assert!(matches!(next(&mut kept), Event::Expired(id) if id == prior.id));
            assert!(
                matches!(next(&mut kept), Event::StopOutcome {can_continue:true, message} if message.contains("none active"))
            );
            assert!(kept.reply(&prior.id, Answer::Cancel).is_err());
            kept.reply(&late.id, Answer::Value("Yes".into())).unwrap();
            assert!(matches!(next(&mut kept), Event::Sent(id) if id == late.id));
            kept.follow_up(prompt).unwrap();
            assert!(kept.follow_up("duplicate").is_err());
        }
        assert!(matches!(next(&mut kept), Event::PromptAccepted));
        assert!(matches!(next(&mut kept), Event::Completed));
        neighbor
            .reply("same-id", Answer::Value("Left".into()))
            .unwrap();
        await_event(&mut neighbor, true);
        assert_eq!(fs::read_to_string(root.join("neighbor")).unwrap(), "Left");
        assert_eq!(
            fs::read_to_string(root.join("user-prompts")).unwrap(),
            "next\nfinish\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("aborts"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        assert_eq!(
            fs::read_to_string(root.join("late-answers")).unwrap(),
            "Yes\nYes\n"
        );
        let methods = fs::read_to_string(root.join("desktop-methods")).unwrap();
        assert_eq!(
            methods
                .lines()
                .filter(|method| *method == "capabilities")
                .count(),
            2,
            "both stops verified delegation"
        );
        let pids = fs::read_to_string(root.join("prompt-pids")).unwrap();
        let mut pids = pids.lines();
        let first = pids.next().unwrap();
        assert_eq!(pids.collect::<Vec<_>>(), [first, first]);
        drop(kept);
        let mut handled = start("retire");
        await_event(&mut handled, false);
        drop(handled);
        let deadline = Instant::now() + Duration::from_secs(5);
        while fs::read_to_string(root.join("ended")).unwrap_or_default()
            != "closed without replying"
        {
            assert!(
                Instant::now() < deadline,
                "explicit End did not retire the handled task"
            );
            thread::sleep(Duration::from_millis(10));
        }
        assert!(!root.join("retire").exists());
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, unix))]
mod saturation_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;
    #[test]
    fn stalled_gui_observes_independent_failure_and_worker_retires() {
        let root =
            std::env::temp_dir().join(format!("ivy-gentle-saturation-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let program = root.join("client");
        std::fs::write(&program, "#!/usr/bin/env python3\nimport json,time\nfor i in range(100):\n print(json.dumps({'type':'extension_ui_request','method':'confirm','id':str(i),'title':'Question'}),flush=True)\n time.sleep(0.02)\ntime.sleep(30)\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut session =
            Session::start(program, root.clone(), "task".into(), Arc::new(|| {})).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !session.terminal.failed() {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
        let mut closed = false;
        for _ in 0..18 {
            if matches!(session.try_event(), Some(Event::Closed(reason)) if reason == "Gentle control backlog exceeded its limit or closed.")
            {
                closed = true;
                break;
            }
        }
        assert!(closed);
        drop(session);
        loop {
            let workers = WORKERS.lock().unwrap();
            let running = workers.iter().any(|w| !w.handle.is_finished());
            drop(workers);
            if !running {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "worker blocked with GUI not draining"
            );
            thread::sleep(Duration::from_millis(10));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
