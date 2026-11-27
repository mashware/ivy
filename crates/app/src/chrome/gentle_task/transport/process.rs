//! Pipe IO, launch and process retirement live off the GUI thread.
#[cfg(windows)]
mod windows;

use super::wire::{self, Incoming};
use super::{Answer, Dialogs, Event, HelperList, HelperMailbox, Inbox, Initial, Message, helpers};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

enum Outgoing {
    Command(serde_json::Value),
    Reply(String, Answer, Option<Instant>),
}

#[derive(Default)]
struct HelperFeed {
    epoch: u64,
    sequence: u64,
    latest: Option<Value>,
    invalid: Option<Value>,
}

impl HelperFeed {
    fn publish(&mut self, event: Option<Value>) {
        let Some(event) = event else {
            *self = Self {
                epoch: self.epoch + 1,
                ..Self::default()
            };
            return;
        };
        match event["seq"].as_u64() {
            Some(seq) if seq > self.sequence => {
                self.sequence = seq;
                self.latest = Some(event);
                self.invalid = None;
            }
            None | Some(0) => self.invalid = Some(event),
            _ => {}
        }
    }

    fn take(&mut self, epoch: u64) -> [Option<Value>; 2] {
        if self.epoch == epoch {
            [self.latest.take(), self.invalid.take()]
        } else {
            [None, None]
        }
    }
}

struct HelperWatch {
    monitor: helpers::Monitor,
    current: HelperList,
    display: HelperMailbox,
    feed: Arc<Mutex<HelperFeed>>,
    epoch: u64,
}

impl HelperWatch {
    fn apply(
        &mut self,
        (request, status): (Option<Value>, Option<helpers::Snapshot>),
        writer: &mpsc::SyncSender<Outgoing>,
    ) {
        if let Some(status) = status {
            self.publish(status);
        }
        if let Some(request) = request
            && writer.try_send(Outgoing::Command(request)).is_err()
        {
            let status = self.monitor.write_failed(Instant::now());
            self.publish(status);
        }
    }

    fn publish(&mut self, status: helpers::Snapshot) {
        let next = match status {
            helpers::Snapshot::Rows(rows) => HelperList {
                rows,
                unavailable: false,
            },
            helpers::Snapshot::Reset => HelperList::default(),
            helpers::Snapshot::Unavailable => {
                if self.current.unavailable {
                    return;
                }
                self.current.unavailable = true;
                self.display.publish(self.current.clone());
                return;
            }
        };
        if next != self.current {
            self.current = next;
            self.display.publish(self.current.clone());
        }
    }

    fn start(&mut self, writer: &mpsc::SyncSender<Outgoing>) {
        let update = self.monitor.start(Instant::now());
        self.apply(update, writer);
    }

    fn poll(&mut self, writer: &mpsc::SyncSender<Outgoing>) {
        let events = self
            .feed
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take(self.epoch);
        for event in events.into_iter().flatten() {
            let update = self.monitor.observe(&event, Instant::now());
            self.apply(update, writer);
        }
        let update = self.monitor.tick(Instant::now());
        self.apply(update, writer);
    }
}

fn prompt(id: &str, message: String) -> Outgoing {
    Outgoing::Command(json!({"type":"prompt", "id":id, "message":message}))
}

pub(super) fn run(
    program: PathBuf,
    cwd: PathBuf,
    initial: Initial,
    inbox: Inbox,
    requests: mpsc::Receiver<Message>,
    stopped: mpsc::Receiver<()>,
    emit: &impl Fn(Event) -> bool,
) -> Result<(), String> {
    if stopped.try_recv().is_ok() {
        return Ok(());
    }
    let mut command = Command::new(program);
    command
        .args(["--mode", "rpc"])
        .current_dir(cwd)
        .env("GENTLE_SHELL_INTERACTIVE_HOST", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for name in panel_contract::ENV_NAMES {
        command.env_remove(name);
    }
    command
        .env_remove("PANEL_SERVER_PID")
        .env_remove("IVY_SESSION_HELPER");
    #[cfg(target_os = "linux")]
    command.env_remove(panel_control::quota::ENDPOINT_ENV);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        let owner = std::process::id() as libc::pid_t;
        // SAFETY: only async-signal-safe syscalls and error construction follow fork.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != owner {
                    return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    let (child, job) =
        windows::spawn(&mut command).map_err(|e| format!("Could not start Gentle: {e}"))?;
    #[cfg(not(windows))]
    let child = command
        .spawn()
        .map_err(|e| format!("Could not start Gentle: {e}"))?;
    let mut owned = Owned {
        child,
        #[cfg(windows)]
        job,
    };
    let stdout = owned.child.stdout.take().expect("piped stdout");
    let mut stdin = owned.child.stdin.take().expect("piped stdin");
    let reader = inbox.clone();
    let feed = Arc::new(Mutex::new(HelperFeed::default()));
    let reader_feed = feed.clone();
    thread::Builder::new()
        .name("gentle-read".into())
        .spawn(move || {
            if let Err(reason) = wire::receive(stdout, &reader, &|event| {
                reader_feed
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .publish(event);
            }) {
                let _ = reader.send(Message::Failed(reason));
            }
        })
        .map_err(|e| e.to_string())?;
    let (writer, outgoing) = mpsc::sync_channel::<Outgoing>(16);
    thread::Builder::new()
        .name("gentle-write".into())
        .spawn(move || {
            for item in outgoing {
                let result = match item {
                    Outgoing::Command(value) => {
                        if wire::write(&mut stdin, value).is_ok() {
                            continue;
                        }
                        Message::Failed(
                            "The connection closed while sending a Gentle command.".into(),
                        )
                    }
                    Outgoing::Reply(id, answer, deadline) => {
                        if deadline.is_some_and(|t| Instant::now() >= t) {
                            Message::TooLate(id)
                        } else if wire::write(&mut stdin, wire::reply(&id, answer)).is_ok() {
                            Message::Written(id)
                        } else {
                            Message::Failed("The connection closed while writing a reply.".into())
                        }
                    }
                };
                let failed = matches!(result, Message::Failed(_));
                if inbox.send(result).is_err() || failed {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;

    let mut helpers = HelperWatch {
        monitor: helpers::Monitor::default(),
        current: HelperList::default(),
        display: initial.helpers,
        feed,
        epoch: 0,
    };
    let mut ready = initial.claimed.swap(true, Ordering::AcqRel);
    if !ready {
        writer
            .try_send(prompt(wire::PROMPT_ID, initial.task))
            .map_err(|_| "Could not send the initial prompt to Gentle.".to_string())?;
        helpers.start(&writer);
    }
    let mut expected_prompt = (!ready).then(|| wire::PROMPT_ID.to_owned());
    let mut has_assignment = !ready;
    let mut stop_after_ack = None;
    let mut follow_ups = 0u64;
    let mut stop = wire::Stop::default();
    let mut dialogs = Dialogs::new();
    let result = loop {
        if stopped.try_recv().is_ok() {
            break Ok(());
        }
        if stop_after_ack.is_some_and(|t| Instant::now() >= t) {
            stop_after_ack = None;
            emit(Event::StopOutcome {
                message: "Gentle has not acknowledged the prompt. The stop could not be confirmed."
                    .into(),
                can_continue: false,
            });
        }
        let tick = stop.tick(Instant::now());
        apply_stop(tick, &mut stop, &writer, &mut dialogs, &mut ready, emit)?;
        dialogs.expire(emit);
        helpers.poll(&writer);
        let msg = match requests.recv_timeout(Duration::from_millis(100)) {
            Ok(msg) => msg,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err("The Gentle connection closed.".into());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if owned.exited().map_err(|e| e.to_string())? {
                    break Err("Gentle ended before confirming the task.".into());
                }
                continue;
            }
        };
        match msg {
            Message::StopTask(visible) => {
                dialogs.visible_at_stop = visible;
                ready = false;
                if expected_prompt.is_some() {
                    // Pi can report idle while its asynchronous prompt preflight is still pending.
                    stop_after_ack = Some(Instant::now() + Duration::from_secs(12));
                    continue;
                }
                let update = stop.begin(Instant::now());
                apply_stop(update, &mut stop, &writer, &mut dialogs, &mut ready, emit)?;
            }
            Message::FollowUp(message) => {
                if !ready || stop_after_ack.is_some() || stop.active() {
                    emit(Event::StopOutcome {
                        message: "Gentle is still working on the previous request.".into(),
                        can_continue: false,
                    });
                    continue;
                }
                follow_ups += 1;
                let id = format!("ivy-follow-up-{follow_ups}");
                match writer.try_send(prompt(&id, message)) {
                    Ok(()) => {
                        ready = false;
                        expected_prompt = Some(id);
                        if has_assignment {
                            stop.follow_up();
                        } else {
                            helpers.start(&writer);
                        }
                        has_assignment = true;
                    }
                    Err(mpsc::TrySendError::Full(_)) => {
                        emit(Event::TurnFailed(
                            "Gentle is busy. The next prompt was not sent.".into(),
                        ));
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        break Err("The Gentle connection closed.".into());
                    }
                }
            }
            Message::Incoming(Incoming::HelperReset) => {
                helpers.epoch += 1;
                if has_assignment {
                    helpers.start(&writer);
                }
            }
            Message::Incoming(Incoming::HelperControl(value)) => {
                let update = helpers.monitor.observe(&value, Instant::now());
                helpers.apply(update, &writer);
            }
            Message::Incoming(Incoming::StopControl(value)) => {
                let update = stop.observe(&value, Instant::now());
                apply_stop(update, &mut stop, &writer, &mut dialogs, &mut ready, emit)?;
            }
            Message::Incoming(Incoming::PromptResponse {
                id,
                handled,
                success,
            }) => {
                if expected_prompt.as_deref() != Some(&id) {
                    continue;
                }
                expected_prompt = None;
                if success {
                    emit(Event::PromptAccepted);
                    if handled {
                        stop.handled();
                        emit(Event::Handled);
                    }
                } else {
                    ready = true;
                    emit(Event::TurnFailed(
                        "Gentle rejected the prompt. It was not accepted.".into(),
                    ));
                }
                if stop_after_ack.take().is_some() {
                    ready = false;
                    let update = stop.begin(Instant::now());
                    apply_stop(update, &mut stop, &writer, &mut dialogs, &mut ready, emit)?;
                }
            }
            Message::Incoming(event @ (Incoming::Completed | Incoming::Aborted)) => {
                let aborted = matches!(event, Incoming::Aborted);
                if stop.active() {
                    let settled = json!({"type":"agent_settled", "aborted":aborted});
                    let update = stop.observe(&settled, Instant::now());
                    apply_stop(update, &mut stop, &writer, &mut dialogs, &mut ready, emit)?;
                } else if !ready
                    && (expected_prompt.is_none() || follow_ups == 0)
                    && stop_after_ack.is_none()
                {
                    expected_prompt = None;
                    ready = true;
                    if aborted {
                        emit(Event::TurnFailed("Gentle reported that the current turn was aborted. Delegated work is unverified.".into()));
                    } else {
                        emit(Event::Completed);
                    }
                }
            }
            Message::Incoming(Incoming::CommandError) => {
                emit(Event::Notice(
                    "A Gentle extension command failed. Its work status is unverified.".into(),
                ));
            }
            Message::Incoming(Incoming::Rejected(reason)) => {
                if !stop.active()
                    && stop_after_ack.is_none()
                    && (expected_prompt.is_none() || follow_ups == 0)
                {
                    expected_prompt = None;
                    ready = true;
                    emit(Event::TurnFailed(reason));
                }
            }
            Message::Incoming(Incoming::Question(q)) => {
                match dialogs.register(q.clone()) {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(error) => break Err(error),
                }
                if !emit(Event::Question(q)) {
                    break Ok(());
                }
            }
            Message::Submit(id, answer) => {
                let Some(q) = dialogs.pending.remove(&id) else {
                    continue;
                };
                if !q.accepts(&answer) {
                    emit(Event::Expired(id));
                    continue;
                }
                dialogs.writing.insert(id.clone());
                if writer
                    .try_send(Outgoing::Reply(id, answer, q.deadline))
                    .is_err()
                {
                    break Err("The Gentle reply channel is unavailable.".into());
                }
            }
            Message::TooLate(id) => {
                dialogs.writing.remove(&id);
                emit(Event::Expired(id));
            }
            Message::Written(id) => {
                dialogs.writing.remove(&id);
                emit(Event::Sent(id));
            }
            Message::Failed(reason) => break Err(reason),
        }
    };
    helpers.poll(&writer);
    if result.is_err() {
        for id in dialogs.writing {
            emit(Event::Uncertain(id));
        }
    }
    drop(writer);
    result
}

fn apply_stop(
    (commands, mut outcome): (Vec<serde_json::Value>, Option<(String, bool)>),
    stop: &mut wire::Stop,
    writer: &mpsc::SyncSender<Outgoing>,
    dialogs: &mut Dialogs,
    ready: &mut bool,
    emit: &impl Fn(Event) -> bool,
) -> Result<(), String> {
    for command in commands {
        match writer.try_send(Outgoing::Command(command)) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(_)) => {
                outcome = stop.write_failed(Instant::now()).1;
                break;
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                return Err("The Gentle connection closed.".into());
            }
        }
    }
    if let Some((message, can_continue)) = outcome {
        *ready = can_continue;
        if stop.confirmed_all() {
            // Gentle supplies no helper identity for a dialog that arrived during reconciliation.
            for id in dialogs.visible_at_stop.drain(..) {
                if dialogs.pending.remove(&id).is_some() {
                    emit(Event::Expired(id));
                }
            }
        }
        for id in std::mem::take(&mut dialogs.writing) {
            emit(Event::Uncertain(id));
        }
        emit(Event::StopOutcome {
            message,
            can_continue,
        });
    }
    Ok(())
}

struct Owned {
    child: Child,
    #[cfg(windows)]
    job: std::os::windows::io::OwnedHandle,
}

impl Owned {
    /// The PID stays reserved until retirement can signal the client's whole process group.
    #[cfg(unix)]
    fn exited(&self) -> std::io::Result<bool> {
        // SAFETY: waitid writes a siginfo record and WNOWAIT preserves our child.
        unsafe {
            let mut info: libc::siginfo_t = std::mem::zeroed();
            let result = libc::waitid(
                libc::P_PID,
                self.child.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            );
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    return Ok(false);
                }
                return Err(error);
            }
            Ok(info.si_pid() != 0)
        }
    }

    #[cfg(windows)]
    fn exited(&mut self) -> std::io::Result<bool> {
        self.child.try_wait().map(|status| status.is_some())
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let pid = self.child.id() as i32;
            // The official launcher forwards termination to Pi. Sending it directly to both
            // would deliver two signals and force Pi to skip graceful shutdown.
            // SAFETY: the unreaped PID belongs to our launcher.
            let result = unsafe { libc::kill(pid, libc::SIGTERM) };
            report_signal_error(result);
            // A launcher can exit before the client or its pipes. Keep its PID reserved
            // through the grace period, then reach any surviving group members.
            thread::sleep(Duration::from_millis(750));
            // SAFETY: this child has not been reaped, so its process group ID cannot be recycled.
            let result = unsafe { libc::kill(-pid, libc::SIGKILL) };
            report_signal_error(result);
        }
        #[cfg(windows)]
        if let Err(error) = windows::terminate(&self.job) {
            crate::pty_output::say(&format!("Could not terminate the Gentle task job: {error}"));
        }
        if let Err(error) = stop_and_reap(&mut self.child) {
            crate::pty_output::say(&format!(
                "Gentle task cleanup could not confirm exit: {error}"
            ));
        }
    }
}

#[cfg(unix)]
fn report_signal_error(result: libc::c_int) {
    if result == -1 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            crate::pty_output::say(&format!(
                "Could not signal the Gentle task process: {error}"
            ));
        }
    }
}

fn stop_and_reap(child: &mut Child) -> std::io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    let kill = child.kill();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(kill.err().unwrap_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "the client did not exit before its retirement deadline",
                )
            }));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_bursts_retain_the_latest_projection_for_the_correct_owner() {
        let mut feed = HelperFeed::default();
        for seq in 1..=1024 {
            feed.publish(Some(json!({"seq":seq})));
        }
        feed.publish(Some(json!({"seq":8})));
        feed.publish(Some(json!({"seq":"invalid"})));
        assert_eq!(
            feed.take(0),
            [Some(json!({"seq":1024})), Some(json!({"seq":"invalid"}))]
        );
        assert_eq!(feed.take(0), [None, None]);
        feed.publish(Some(json!({"seq":1025})));
        feed.publish(None);
        feed.publish(Some(json!({"seq":1,"owner":"B"})));
        assert_eq!(feed.take(0), [None, None]);
        assert_eq!(feed.take(1), [Some(json!({"seq":1,"owner":"B"})), None]);
    }
}
