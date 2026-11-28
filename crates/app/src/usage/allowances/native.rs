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

const NAME: &str = "ivy_allowances";
const MAX_LINE: usize = 256 * 1024;
const MAX_EXCHANGE: usize = 2 * 1024 * 1024;
/// JSON-RPC's codes for a method the server does not have and for parameters it rejects.
const NO_METHOD: i64 = -32601;
const BAD_PARAMS: i64 = -32602;

pub(super) fn read(cancelled: Arc<AtomicBool>) -> Result<Observation, Failure> {
    with_account(|current_account| read_inner(cancelled, current_account))
}

fn with_account(
    read: impl FnOnce(&mut Option<Account>) -> Result<Observation, Failure>,
) -> Result<Observation, Failure> {
    let mut current_account = None;
    read(&mut current_account).map_err(|failure| Failure::WithAccount {
        account: current_account,
        failure: Box::new(failure),
    })
}

fn read_inner(
    cancelled: Arc<AtomicBool>,
    current_account: &mut Option<Account>,
) -> Result<Observation, Failure> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let program = native_codex().map_err(|_| Failure::Unsupported)?;
    let args = ["--version"];
    let mut version = AllowanceProcess::start(&program, &args, false, deadline, cancelled.clone())?;
    let version_line = version_line(&mut version)?;
    version.finish()?;
    let version = client_version(&version_line)?;

    let args = ["app-server", "--listen", "stdio://", "--disable", "plugins"];
    let mut server = AllowanceProcess::start(&program, &args, true, deadline, cancelled)?;
    handshake(&mut server, &version)?;
    observe(&mut server, current_account)
}

fn version_line(process: &mut AllowanceProcess) -> Result<Vec<u8>, Failure> {
    process.line(256).map_err(|failure| match failure {
        Failure::Exited => Failure::Refused("Codex --version closed before reporting its version."),
        other => other,
    })
}

fn observe(
    server: &mut AllowanceProcess,
    current_account: &mut Option<Account>,
) -> Result<Observation, Failure> {
    let before = account(server, 2)?;
    *current_account = Some(before.clone());
    let quotas = rate_limits(server)?;
    let observed_at = chrono::Utc::now().timestamp();
    // A failed second account read must not reuse the identity from before the quota request.
    *current_account = None;
    let after = account(server, 4)?;
    *current_account = Some(after.clone());
    server.finish()?;
    if !before.matches(&after) {
        return Err(ACCOUNT_CHANGED);
    }
    bind_observation(before, after, quotas.map_err(quota_failure)?, observed_at)
}

fn handshake(server: &mut AllowanceProcess, version: &str) -> Result<(), Failure> {
    let hello = server
        .request(
            1,
            "initialize",
            json!({
                "clientInfo": {"name": NAME, "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"experimentalApi": true}
            }),
        )
        // Only a server that exits before its first answer points at this launch's flags; a
        // deadline, a malformed line or a wrong reply says nothing about them.
        .map_err(|failure| match failure {
            Failure::Exited => Failure::Refused(
                "Codex app-server exited before answering; it may refuse --disable plugins.",
            ),
            other => other,
        })?
        .map_err(|code| Failure::Rpc {
            method: "initialize",
            code,
        })?;
    require_agent(&hello, version)?;
    server.send(json!({"method": "initialized"}))
}

fn rate_limits(server: &mut AllowanceProcess) -> Result<Result<serde_json::Value, i64>, Failure> {
    server.request(
        3,
        "account/rateLimits/read",
        json!({"excludeResetCreditDetails": true}),
    )
}

/// What an error answer to the quota read means, by its JSON-RPC code.
fn quota_failure(code: i64) -> Failure {
    match code {
        NO_METHOD => Failure::Refused("Codex has no account/rateLimits/read method."),
        BAD_PARAMS => Failure::Refused("Codex rejected excludeResetCreditDetails."),
        code => Failure::Rpc {
            method: "account/rateLimits/read",
            code,
        },
    }
}

const ACCOUNT_CHANGED: Failure = Failure::Identity("Codex's account changed during the read.");

/// The release the client reports, any `codex-cli <major>.<minor>.<patch>[suffix]`.
fn client_version(line: &[u8]) -> Result<String, Failure> {
    let version = std::str::from_utf8(line)
        .ok()
        .and_then(|line| line.trim().strip_prefix("codex-cli "))
        .filter(|version| {
            let core = version.split(['-', '+']).next().unwrap_or_default();
            let parts: Vec<_> = core.split('.').collect();
            parts.len() == 3
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                && version.len() <= 64
                && !version.chars().any(char::is_whitespace)
        });
    version
        .map(str::to_owned)
        .ok_or(Failure::Refused("Codex reported no codex-cli version."))
}

/// The server that answers is the release that reported its version: its user agent names this
/// client and that release.
fn require_agent(hello: &serde_json::Value, version: &str) -> Result<(), Failure> {
    let user_agent = hello["userAgent"]
        .as_str()
        .and_then(|agent| agent.split_whitespace().next());
    if user_agent.and_then(|agent| agent.split_once('/')) == Some((NAME, version)) {
        Ok(())
    } else {
        Err(Failure::Refused(
            "Codex's app-server version differs from codex-cli.",
        ))
    }
}

fn account(server: &mut AllowanceProcess, id: u32) -> Result<Account, Failure> {
    let value = server
        .request(
            id,
            "accountSession/list",
            json!({"refreshWorkspaceMetadata": false}),
        )?
        .map_err(|code| match code {
            NO_METHOD => Failure::Refused("Codex has no accountSession/list method."),
            BAD_PARAMS => Failure::Refused("Codex rejected accountSession/list's parameters."),
            code => Failure::Rpc {
                method: "accountSession/list",
                code,
            },
        })?;
    Account::from_response(value)
        .map_err(|_| Failure::Refused("Codex gave no complete session, user and workspace."))
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
                if value["method"] == "account/updated" {
                    return Err(ACCOUNT_CHANGED);
                }
                if value.get("id").is_some() {
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
                Ok(0) => return Err(Failure::Exited),
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
                    Err(Failure::Refused("Codex exited unsuccessfully during the allowance read."))
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
    if cancelled.load(Ordering::Relaxed) {
        Err(Failure::Internal("Ivy cancelled the allowance read."))
    } else if Instant::now() >= deadline {
        Err(Failure::Internal("Ivy stopped the allowance read at its deadline."))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::failure_text;
    use super::*;

    fn refusal<T: std::fmt::Debug>(result: Result<T, Failure>) -> &'static str {
        match result {
            Err(Failure::Refused(reason)) => reason,
            other => panic!("a refusal was expected: {other:?}"),
        }
    }

    /// No release is pinned (D132): any `codex-cli <semver>` is read, and the evidence of each
    /// read decides. What does not name a release is refused by name.
    #[test]
    fn any_codex_cli_release_is_accepted_and_a_missing_one_is_named() {
        for (line, version) in [
            ("codex-cli 0.167.2", "0.167.2"),
            ("codex-cli 0.172.0\n", "0.172.0"),
            ("codex-cli 1.0.0-alpha.3", "1.0.0-alpha.3"),
        ] {
            assert_eq!(client_version(line.as_bytes()).unwrap(), version);
        }
        for line in [
            "codex 0.172.0",
            "codex-cli 0.172",
            "codex-cli",
            "codex-cli 0.x.1",
        ] {
            assert_eq!(
                refusal(client_version(line.as_bytes())),
                "Codex reported no codex-cli version."
            );
        }
        let hello = json!({"userAgent": "ivy_allowances/0.172.0 (Linux; x86_64)"});
        assert!(require_agent(&hello, "0.172.0").is_ok());
        assert_eq!(
            refusal(require_agent(&hello, "0.167.2")),
            "Codex's app-server version differs from codex-cli."
        );
    }

    fn scripted(script: &str) -> AllowanceProcess {
        AllowanceProcess::start(
            Path::new("/bin/sh"),
            &["-c", script],
            true,
            Instant::now() + Duration::from_secs(2),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    }

    fn account_reply(id: u32, user: Option<&str>) -> Value {
        let result = user.map_or_else(|| json!({"sessions": []}), |user| json!({
            "activeSessionId": user,
            "sessions": [{"sessionId": user, "isActive": true, "userId": user,
                "selectedWorkspaceAccountId": "shared"}]
        }));
        json!({"id": id, "result": result})
    }

    #[test]
    fn failed_reads_carry_the_account_actually_seen_before_the_failure() {
        use super::super::AllowanceView;
        let _serial = crate::client_attestation::CHILD_TEST_SERIAL.lock().unwrap();
        for (before, after, keep) in [
            (Some("a"), Some("a"), true),
            (Some("b"), Some("b"), false),
            (None, None, false),
            (Some("a"), None, false),
            (Some("a"), Some("b"), false),
        ] {
            let mut view = AllowanceView { enabled: true, ..Default::default() };
            let selected = Account::from_response(account_reply(2, Some("a"))["result"].take()).unwrap();
            view.install(bind_observation(selected.clone(), selected,
                json!({"accountId": "shared", "ordinaryUsageAllowed": false,
                    "rateLimits": {"limitId": "codex", "primary": {"usedPercent": 42,
                        "windowDurationMins": 300, "resetsAt": 5000}}}), 100));
            let script = [account_reply(2, before),
                json!({"id": 3, "error": {"code": -32603, "message": "private"}}),
                account_reply(4, after)]
                .into_iter().map(|reply| format!("read request\nprintf '%s\\n' '{reply}'\n"))
                .collect::<String>();
            let mut server = scripted(&script);
            view.install(with_account(|current| observe(&mut server, current)));
            let source = view.chip_source(200);
            assert_eq!(matches!(source, super::super::super::chip::Source::Read(_)), keep,
                "before={before:?} after={after:?}: {source:?}");
            assert!(!format!("{source:?}").contains("private"));
            if let super::super::super::chip::Source::Read(reading) = source {
                assert_eq!(reading.observed_at, 100);
                assert_eq!(reading.windows[0].window.used, Some(42.0));
                assert!(reading.last_failure.unwrap().contains("account/rateLimits/read"));
            }
        }
    }

    #[test]
    fn internal_failures_and_version_eof_name_the_operation_that_stopped() {
        let _serial = crate::client_attestation::CHILD_TEST_SERIAL.lock().unwrap();
        let mut closed = scripted("exit 0");
        assert_eq!(failure_text(&version_line(&mut closed).unwrap_err()),
            "Codex --version closed before reporting its version.");
        assert_eq!(failure_text(&check(Instant::now(), &AtomicBool::new(false)).unwrap_err()),
            "Ivy stopped the allowance read at its deadline.");
        assert_eq!(failure_text(&check(Instant::now() + Duration::from_secs(1), &AtomicBool::new(true)).unwrap_err()),
            "Ivy cancelled the allowance read.");
        let mut failed = scripted("exit 2");
        assert_eq!(failure_text(&failed.finish().unwrap_err()),
            "Codex exited unsuccessfully during the allowance read.");
    }

    fn answering(id: u32, code: i64) -> AllowanceProcess {
        scripted(&format!(
            "read request\nprintf '%s\\n' '{{\"id\":{id},\"error\":{{\"code\":{code},\"message\":\"m\"}}}}'"
        ))
    }

    /// Only a server that closes before its first answer is said to refuse the launch flags; any
    /// other handshake failure keeps its own cause, and an error answer carries its code.
    #[test]
    fn a_handshake_blames_the_flags_only_when_the_server_closed_first() {
        let _serial = crate::client_attestation::CHILD_TEST_SERIAL.lock().unwrap();
        let mut exits = scripted("read request\nexit 2");
        assert_eq!(
            refusal(handshake(&mut exits, "0.172.0")),
            "Codex app-server exited before answering; it may refuse --disable plugins."
        );
        let mut garbled = scripted("read request\nprintf 'not json\\n'\nread next");
        assert!(matches!(
            handshake(&mut garbled, "0.172.0"),
            Err(Failure::Unavailable)
        ));
        let mut refused = answering(1, -32600);
        let failure = handshake(&mut refused, "0.172.0").unwrap_err();
        assert_eq!(
            failure_text(&failure),
            "Codex answered initialize with error -32600."
        );
    }

    /// Error answers name what was refused: a missing method or rejected parameters by name, and
    /// any other JSON-RPC code by its number, so a failed read can be diagnosed.
    #[test]
    fn error_answers_are_named_by_method_and_code() {
        let _serial = crate::client_attestation::CHILD_TEST_SERIAL.lock().unwrap();
        for (code, said) in [
            (-32602, "Codex rejected excludeResetCreditDetails.".to_owned()),
            (-32601, "Codex has no account/rateLimits/read method.".to_owned()),
            (-32603, "Codex answered account/rateLimits/read with error -32603.".to_owned()),
            (-32600, "Codex answered account/rateLimits/read with error -32600.".to_owned()),
        ] {
            let mut server = answering(3, code);
            let answer = rate_limits(&mut server).unwrap();
            assert_eq!(failure_text(&quota_failure(answer.unwrap_err())), said);
        }
        for (code, said) in [
            (-32601, "Codex has no accountSession/list method.".to_owned()),
            (-32602, "Codex rejected accountSession/list's parameters.".to_owned()),
            (-32603, "Codex answered accountSession/list with error -32603.".to_owned()),
        ] {
            let mut server = answering(2, code);
            assert_eq!(failure_text(&account(&mut server, 2).unwrap_err()), said);
        }
        let mut incomplete = scripted(
            "read request\nprintf '%s\\n' '{\"id\":2,\"result\":{\"activeSessionId\":\"s\",\"sessions\":[]}}'",
        );
        assert_eq!(
            refusal(account(&mut incomplete, 2)),
            "Codex gave no complete session, user and workspace."
        );
        let mut switched = scripted(
            "read request\nprintf '%s\\n' '{\"method\":\"account/updated\",\"params\":{}}'\nread next",
        );
        assert!(matches!(
            account(&mut switched, 2),
            Err(Failure::Identity("Codex's account changed during the read."))
        ));
    }

    /// One read against the installed client and the person's own account, run by hand with
    /// `--ignored`: it prints which evidence the release returned, never an identifier.
    #[test]
    #[ignore = "reads the installed Codex client with the person's own account"]
    fn installed_codex_returns_the_identity_evidence() {
        let program = native_codex().expect("an installed native Codex");
        let deadline = Instant::now() + Duration::from_secs(15);
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut probe =
            AllowanceProcess::start(&program, &["--version"], false, deadline, cancelled.clone())
                .unwrap();
        let line = probe.line(256).unwrap();
        probe.finish().unwrap();
        let version = client_version(&line).unwrap();
        eprintln!("EVIDENCE version={version}");
        let args = ["app-server", "--listen", "stdio://", "--disable", "plugins"];
        let mut server =
            AllowanceProcess::start(&program, &args, true, deadline, cancelled).unwrap();
        let handshake = handshake(&mut server, &version);
        eprintln!("EVIDENCE handshake_and_user_agent={:?}", handshake.is_ok());
        handshake.unwrap();
        let before = account(&mut server, 2);
        eprintln!("EVIDENCE complete_account_before={}", before.is_ok());
        let quotas = rate_limits(&mut server).unwrap();
        match &quotas {
            Ok(value) => eprintln!(
                "EVIDENCE rate_limits=ok accountId_present={} ordinaryUsageAllowed_is_bool={} keys={:?}",
                value["accountId"].is_string(),
                value["ordinaryUsageAllowed"].is_boolean(),
                value
                    .as_object()
                    .map(|map| map.keys().cloned().collect::<Vec<_>>())
            ),
            Err(code) => eprintln!("EVIDENCE rate_limits_error_code={code}"),
        }
        let after = account(&mut server, 4);
        eprintln!("EVIDENCE complete_account_after={}", after.is_ok());
        server.finish().unwrap();
        let result = bind_observation(before.unwrap(), after.unwrap(), quotas.unwrap(), 0);
        match &result {
            Ok(observation) => eprintln!(
                "EVIDENCE bound=true buckets={} windows={}",
                observation.buckets.len(),
                observation
                    .buckets
                    .iter()
                    .map(|bucket| usize::from(bucket.primary.is_some())
                        + usize::from(bucket.secondary.is_some()))
                    .sum::<usize>()
            ),
            Err(failure) => eprintln!("EVIDENCE bound=false {failure:?}"),
        }
        result.unwrap();
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
        assert!(matches!(child.line(MAX_LINE), Err(Failure::Internal("Ivy cancelled the allowance read."))));
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
