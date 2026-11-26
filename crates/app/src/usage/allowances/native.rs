use super::{Account, Failure, Observation, bind_observation};
use crate::client_attestation::{Running, native_codex};
use serde_json::{Value, json};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const VERSION: &str = "0.167.2";
const NAME: &str = "ivy_allowances";
const MAX_LINE: usize = 256 * 1024;
const MAX_EXCHANGE: usize = 2 * 1024 * 1024;

pub(super) fn read(cancelled: Arc<AtomicBool>) -> Result<Observation, Failure> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let program = native_codex().map_err(|_| Failure::Unsupported)?;
    let args = ["--version"];
    let mut version = AllowanceProcess::start(&program, &args, false, deadline, cancelled.clone())?;
    let version_line = version.line(256)?;
    version.finish()?;
    require_version(&version_line)?;

    let args = ["app-server", "--listen", "stdio://", "--disable", "plugins"];
    let mut server = AllowanceProcess::start(&program, &args, true, deadline, cancelled)?;
    let hello = server
        .request(
            1,
            "initialize",
            json!({
                "clientInfo": {"name": NAME, "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"experimentalApi": true}
            }),
        )?
        .map_err(|_| Failure::Unsupported)?;
    let user_agent = hello["userAgent"]
        .as_str()
        .and_then(|agent| agent.split_whitespace().next());
    if user_agent.and_then(|agent| agent.split_once('/')) != Some((NAME, VERSION)) {
        return Err(Failure::Unsupported);
    }
    server.send(json!({"method": "initialized"}))?;
    let before = account(&mut server, 2)?;
    let quotas = server.request(
        3,
        "account/rateLimits/read",
        json!({"excludeResetCreditDetails": true}),
    )?;
    let observed_at = chrono::Utc::now().timestamp();
    let after = account(&mut server, 4)?;
    server.finish()?;
    bind_observation(before, after, quotas, observed_at)
}

fn require_version(line: &[u8]) -> Result<(), Failure> {
    let version = std::str::from_utf8(line)
        .ok()
        .and_then(|line| line.trim().strip_prefix("codex-cli "));
    if version == Some(VERSION) {
        Ok(())
    } else {
        Err(Failure::Unsupported)
    }
}

fn account(server: &mut AllowanceProcess, id: u32) -> Result<Account, Failure> {
    let value = server
        .request(
            id,
            "accountSession/list",
            json!({"refreshWorkspaceMetadata": false}),
        )?
        .map_err(|code| {
            if code == -32601 {
                Failure::Unsupported
            } else {
                Failure::Unavailable
            }
        })?;
    Account::from_response(value)
}

struct AllowanceProcess {
    running: Running,
    stdin: Option<ChildStdin>,
    buffer: Vec<u8>,
    received: usize,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    reaped: bool,
}

impl AllowanceProcess {
    fn start(
        program: &Path,
        args: &[&str],
        piped_input: bool,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, Failure> {
        check(deadline, &cancelled)?;
        let mut command = Command::new(program);
        command
            .args(args)
            .process_group(0)
            .current_dir("/")
            .env_clear()
            .envs(std::env::vars_os().filter(|(key, _)| {
                key.to_str().is_some_and(|key| {
                    matches!(
                        key,
                        "HOME"
                            | "PATH"
                            | "LANG"
                            | "LC_ALL"
                            | "CODEX_HOME"
                            | "XDG_CONFIG_HOME"
                            | "XDG_CACHE_HOME"
                            | "XDG_DATA_HOME"
                            | "XDG_STATE_HOME"
                            | "XDG_RUNTIME_DIR"
                            | "DBUS_SESSION_BUS_ADDRESS"
                    )
                })
            }))
            .stdin(if piped_input {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut running = Running::start(&mut command).map_err(|_| Failure::Unavailable)?;
        let stdin = running.child.as_mut().and_then(|child| child.stdin.take());
        Ok(Self {
            running,
            stdin,
            buffer: Vec::new(),
            received: 0,
            deadline,
            cancelled,
            reaped: false,
        })
    }

    fn send(&mut self, value: Value) -> Result<(), Failure> {
        check(self.deadline, &self.cancelled)?;
        let mut bytes = serde_json::to_vec(&value).map_err(|_| Failure::Unavailable)?;
        bytes.push(b'\n');
        // Requests are fixed and sequential. This cap also bounds the initialized notification
        // plus its following read to less than the Linux minimum pipe capacity.
        if bytes.len() > 512 {
            return Err(Failure::Unavailable);
        }
        self.stdin
            .as_mut()
            .ok_or(Failure::Unavailable)?
            .write_all(&bytes)
            .map_err(|_| Failure::Unavailable)
    }

    fn request(
        &mut self,
        id: u32,
        method: &str,
        params: Value,
    ) -> Result<Result<Value, i64>, Failure> {
        self.send(json!({"id":id,"method":method,"params":params}))?;
        loop {
            let line = self.line(MAX_LINE)?;
            let mut value: Value =
                serde_json::from_slice(&line).map_err(|_| Failure::Unavailable)?;
            if value.get("method").is_some() {
                if value.get("id").is_some() || value["method"] == "account/updated" {
                    return Err(Failure::Unavailable);
                }
                continue;
            }
            if value["id"].as_u64() != Some(u64::from(id)) {
                return Err(Failure::Unavailable);
            }
            return match (
                value.get("result").is_some(),
                value["error"]["code"].as_i64(),
            ) {
                (true, None) if value.get("error").is_none() => Ok(Ok(value["result"].take())),
                (false, Some(code)) => Ok(Err(code)),
                _ => Err(Failure::Unavailable),
            };
        }
    }

    fn line(&mut self, max: usize) -> Result<Vec<u8>, Failure> {
        loop {
            check(self.deadline, &self.cancelled)?;
            if let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
                if end > max {
                    return Err(Failure::Unavailable);
                }
                let mut line: Vec<_> = self.buffer.drain(..=end).collect();
                line.pop();
                return Ok(line);
            }
            if self.buffer.len() > max {
                return Err(Failure::Unavailable);
            }
            let mut chunk = [0; 4096];
            match self.running.stdout.read(&mut chunk) {
                Ok(0) => return Err(Failure::Unavailable),
                Ok(count) => {
                    self.received += count;
                    if self.received > MAX_EXCHANGE {
                        return Err(Failure::Unavailable);
                    }
                    self.buffer.extend_from_slice(&chunk[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    let mut fd = libc::pollfd {
                        fd: self.running.stdout.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // SAFETY: fd borrows the still-owned stdout pipe and is valid for this call.
                    unsafe { libc::poll(&mut fd, 1, 20) };
                }
                Err(_) => return Err(Failure::Unavailable),
            }
        }
    }

    fn finish(&mut self) -> Result<(), Failure> {
        self.stdin.take();
        let child = self.running.child.as_mut().ok_or(Failure::Unavailable)?;
        loop {
            check(self.deadline, &self.cancelled)?;
            if exited(child.id()).map_err(|_| Failure::Unavailable)? {
                signal_group(child.id());
                let status = child
                    .try_wait()
                    .map_err(|_| Failure::Unavailable)?
                    .ok_or(Failure::Unavailable)?;
                self.reaped = true;
                return if status.success() {
                    Ok(())
                } else {
                    Err(Failure::Unavailable)
                };
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for AllowanceProcess {
    fn drop(&mut self) {
        if !self.reaped
            && let Some(child) = &self.running.child
        {
            signal_group(child.id());
        }
    }
}

fn exited(pid: u32) -> io::Result<bool> {
    // SAFETY: siginfo_t is plain storage initialized to zero until waitid writes it.
    let mut status: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // WNOWAIT keeps the leader's pid reserved until we can terminate any remaining group members.
    // SAFETY: waitid writes to status and checks only the specified owned child.
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid,
            &mut status,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result == 0 {
        // SAFETY: waitid returned status for this child or left its pid zero.
        Ok(unsafe { status.si_pid() } != 0)
    } else {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            Ok(false)
        } else {
            Err(error)
        }
    }
}

fn signal_group(pid: u32) {
    // SAFETY: process_group(0) made this unreaped child's pid an owned group id.
    unsafe { libc::kill(-(pid as libc::pid_t), libc::SIGKILL) };
}

fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), Failure> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(Failure::Unavailable)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_admitted_protocol_version() {
        assert!(require_version(b"codex-cli 0.167.2").is_ok());
        assert!(matches!(
            require_version(b"codex-cli 0.167.20"),
            Err(Failure::Unsupported)
        ));
    }

    #[test]
    fn private_transport_rejects_wrong_replies_and_server_requests() {
        let _serial = crate::client_attestation::CHILD_TEST_SERIAL.lock().unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        for line in [
            r#"{"id":2,"result":{}}"#,
            r#"{"id":1,"method":"account/chatgptAuthTokens/refresh"}"#,
        ] {
            let script = format!("read request\nprintf '%s\\n' '{line}'");
            let mut child = AllowanceProcess::start(
                Path::new("/bin/sh"),
                &["-c", &script],
                true,
                Instant::now() + Duration::from_secs(2),
                cancelled.clone(),
            )
            .unwrap();
            assert!(matches!(
                child.request(1, "initialize", json!({})),
                Err(Failure::Unavailable)
            ));
        }
    }

    #[test]
    fn line_budget_and_cancellation_bound_a_live_pipe() {
        let _serial = crate::client_attestation::CHILD_TEST_SERIAL.lock().unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut child = AllowanceProcess::start(
            Path::new("/bin/sh"),
            &["-c", "read request\nprintf 'oversized\\n'\nread next"],
            true,
            Instant::now() + Duration::from_secs(2),
            cancelled.clone(),
        )
        .unwrap();
        child.send(json!({"method": "initialized"})).unwrap();
        assert!(matches!(child.line(4), Err(Failure::Unavailable)));
        cancelled.store(true, Ordering::Relaxed);
        assert!(matches!(child.line(MAX_LINE), Err(Failure::Unavailable)));
    }

    #[test]
    fn successful_leader_exit_terminates_process_group_members() {
        let _serial = crate::client_attestation::CHILD_TEST_SERIAL.lock().unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut child = AllowanceProcess::start(
            Path::new("/bin/sh"),
            &["-c", "/bin/sleep 30 &\nprintf '%s\\n' \"$!\""],
            false,
            Instant::now() + Duration::from_secs(2),
            cancelled.clone(),
        )
        .unwrap();
        let pid = String::from_utf8(child.line(32).unwrap()).unwrap();
        child.finish().unwrap();
        let status = format!("/proc/{pid}/stat");
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match std::fs::read_to_string(&status) {
                Ok(stat)
                    if stat
                        .rsplit_once(") ")
                        .is_some_and(|(_, tail)| tail.starts_with("Z ")) =>
                {
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                _ => assert!(
                    Instant::now() < deadline,
                    "allowance descendant still running"
                ),
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
