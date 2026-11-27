//! Pipe IO, launch and process retirement live off the GUI thread.
#[cfg(windows)]
mod windows;

use super::wire::{self, Incoming};
use super::{Answer, Dialogs, Event, Inbox, Message};
use serde_json::json;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

type Outgoing = (String, Answer, Option<Instant>);

pub(super) fn run(
    program: PathBuf,
    cwd: PathBuf,
    task: String,
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
    thread::Builder::new()
        .name("gentle-read".into())
        .spawn(move || {
            if let Err(reason) = wire::receive(stdout, &reader) {
                let _ = reader.send(Message::Failed(reason));
            }
        })
        .map_err(|e| e.to_string())?;
    let (writer, outgoing) = mpsc::sync_channel::<Outgoing>(8);
    thread::Builder::new()
        .name("gentle-write".into())
        .spawn(move || {
            let prompt = json!({"type":"prompt", "id":wire::PROMPT_ID,"message":task});
            if wire::write(&mut stdin, prompt).is_err() {
                let _ = inbox.send(Message::Failed("Could not send the task to Gentle.".into()));
                return;
            }
            for (id, answer, deadline) in outgoing {
                let result = if deadline.is_some_and(|t| Instant::now() >= t) {
                    Message::TooLate(id)
                } else if wire::write(&mut stdin, wire::reply(&id, answer)).is_ok() {
                    Message::Written(id)
                } else {
                    let _ = inbox.send(Message::Failed(
                        "The connection closed while writing a reply.".into(),
                    ));
                    return;
                };
                if inbox.send(result).is_err() {
                    break;
                }
            }
        })
        .map_err(|e| e.to_string())?;

    let mut dialogs = Dialogs::new();
    let result = loop {
        if stopped.try_recv().is_ok() {
            break Ok(());
        }
        dialogs.expire(emit);
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
            Message::Incoming(Incoming::Handled) => {
                emit(Event::Handled);
            }
            Message::Incoming(Incoming::Completed) => {
                emit(Event::Completed);
                break Ok(());
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
                if writer.try_send((id, answer, q.deadline)).is_err() {
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
    if result.is_err() {
        for id in dialogs.writing {
            emit(Event::Uncertain(id));
        }
    }
    drop(writer);
    result
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
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            // A launcher can exit before the client or its pipes. Keep its PID reserved
            // through the grace period, then reach any surviving group members.
            thread::sleep(Duration::from_millis(750));
            // SAFETY: this child has not been reaped, so its process group ID cannot be recycled.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
        #[cfg(windows)]
        windows::terminate(&self.job);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
