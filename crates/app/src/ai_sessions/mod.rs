//! Explicit client declarations bound to one native foreground invocation.

#[cfg(target_os = "linux")]
mod programs;
#[cfg(target_os = "linux")]
mod protocol;
#[cfg(all(test, target_os = "linux"))]
mod tests;
#[cfg(target_os = "linux")]
mod verify;
#[cfg(target_os = "linux")]
mod worker;

use panel_control::session::{Identity, SessionHandler};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::time::Instant;
use term_core::ForegroundScope;

#[cfg(target_os = "linux")]
pub(crate) use programs::{ResolvedLaunch, resolve_launch};

#[cfg(not(target_os = "linux"))]
pub(crate) struct ResolvedLaunch {
    pub(crate) program: PathBuf,
    pub(crate) prefix_args: Vec<String>,
    pub(crate) path: std::ffi::OsString,
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn resolve_launch(_: panel_control::session::Client) -> std::io::Result<ResolvedLaunch> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Disabled,
    Waiting,
    // Only the Linux session worker registers a client; elsewhere the state is matched, never set.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Registered,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub(crate) identity: Option<Identity>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) revision: u64,
    pub(crate) status: Status,
    pub(crate) sampled: bool,
}

pub(crate) struct Handle {
    shared: Arc<Shared>,
}

struct Shared {
    scope: ForegroundScope,
    enabled: AtomicBool,
    #[cfg(target_os = "linux")]
    frozen: AtomicBool,
    close_acknowledged: Arc<AtomicBool>,
    epoch: AtomicU64,
    published: Mutex<Snapshot>,
    changed: Condvar,
    wake: Arc<dyn Fn() + Send + Sync>,
    #[cfg(target_os = "linux")]
    supplied: Option<Vec<(panel_control::session::Client, PathBuf)>>,
}

impl Handle {
    pub(crate) fn new(scope: ForegroundScope, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self::build(scope, wake, None)
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn with_programs(
        scope: ForegroundScope,
        wake: Arc<dyn Fn() + Send + Sync>,
        programs: Vec<(panel_control::session::Client, PathBuf)>,
    ) -> Self {
        Self::build(scope, wake, Some(programs))
    }

    fn build(
        scope: ForegroundScope,
        wake: Arc<dyn Fn() + Send + Sync>,
        supplied: Option<Vec<(panel_control::session::Client, PathBuf)>>,
    ) -> Self {
        #[cfg(not(target_os = "linux"))]
        let _ = supplied;
        Self {
            shared: Arc::new(Shared {
                scope,
                enabled: AtomicBool::new(false),
                #[cfg(target_os = "linux")]
                frozen: AtomicBool::new(false),
                close_acknowledged: Arc::new(AtomicBool::new(false)),
                epoch: AtomicU64::new(0),
                published: Mutex::new(Snapshot {
                    identity: None,
                    cwd: None,
                    revision: 0,
                    status: Status::Disabled,
                    sampled: false,
                }),
                changed: Condvar::new(),
                wake,
                #[cfg(target_os = "linux")]
                supplied,
            }),
        }
    }

    pub(crate) fn callback(&self) -> SessionHandler {
        let weak = Arc::downgrade(&self.shared);
        Arc::new(move |peer, request| {
            let Some(shared) = weak.upgrade() else {
                return error("disabled");
            };
            if !shared.enabled.load(Ordering::Acquire) || !shared.scope.is_open() {
                return error("disabled");
            }
            #[cfg(target_os = "linux")]
            return worker::request(shared, peer, request);
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (peer, request);
                error("unsupported")
            }
        })
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        if self.shared.enabled.swap(enabled, Ordering::AcqRel) == enabled {
            return;
        }
        self.shared.epoch.fetch_add(1, Ordering::AcqRel);
        self.shared.publish(
            None,
            None,
            if enabled {
                Status::Waiting
            } else {
                Status::Disabled
            },
        );
        #[cfg(target_os = "linux")]
        worker::refresh(&self.shared);
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        self.shared
            .published
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(crate) fn snapshot_if_changed(&self, revision: u64) -> Option<Snapshot> {
        let snapshot = self
            .shared
            .published
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (snapshot.revision != revision).then(|| snapshot.clone())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.set_enabled(false);
        #[cfg(target_os = "linux")]
        worker::notify_existing();
    }
}

pub(crate) fn freeze_all(handles: &[&Handle], timeout: Duration) -> Vec<Snapshot> {
    #[cfg(target_os = "linux")]
    let deadline = Instant::now() + timeout;
    #[cfg(not(target_os = "linux"))]
    let _ = timeout;
    for handle in handles {
        handle
            .shared
            .close_acknowledged
            .store(false, Ordering::Release);
    }
    #[cfg(target_os = "linux")]
    let tickets = worker::prepare_close(handles, deadline);
    #[cfg(target_os = "linux")]
    for (shared, token) in &tickets {
        let mut snapshot = shared
            .published
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while !token.load(Ordering::Acquire) && Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            snapshot = shared
                .changed
                .wait_timeout(snapshot, remaining)
                .unwrap_or_else(|error| error.into_inner())
                .0;
        }
    }
    #[cfg(target_os = "linux")]
    for handle in handles {
        handle.shared.frozen.store(true, Ordering::Release);
        let mut snapshot = handle
            .shared
            .published
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !handle.shared.close_acknowledged.load(Ordering::Acquire)
            && (snapshot.identity.is_some() || snapshot.sampled)
        {
            snapshot.identity = None;
            snapshot.sampled = false;
            snapshot.revision = snapshot.revision.wrapping_add(1);
            snapshot.status = Status::Waiting;
        }
    }
    handles.iter().map(|handle| handle.snapshot()).collect()
}

impl Shared {
    fn publish(&self, identity: Option<Identity>, cwd: Option<PathBuf>, status: Status) {
        let mut snapshot = self
            .published
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if snapshot.identity == identity
            && snapshot.cwd == cwd
            && snapshot.status == status
            && !snapshot.sampled
        {
            return;
        }
        snapshot.identity = identity;
        snapshot.cwd = cwd;
        snapshot.status = status;
        snapshot.sampled = false;
        snapshot.revision = snapshot.revision.wrapping_add(1);
        drop(snapshot);
        self.changed.notify_all();
        (self.wake)();
    }
}

fn error(kind: &str) -> serde_json::Value {
    serde_json::json!({"ok":false,"error":kind})
}

#[cfg(target_os = "linux")]
pub(crate) fn reap_later() {
    worker::reap_later();
}

#[cfg(all(test, windows))]
mod tests {
    macro_rules! windows_unix_omission {
        ($name:ident, $reason:literal) => {
            #[test]
            #[ignore = $reason]
            fn $name() {}
        };
    }

    windows_unix_omission!(
        claude_null_directory_uses_the_verified_client_not_its_shell_or_helper,
        "unix-only: requires Linux process ancestry and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        unavailable_claude_root_directory_revokes_the_verified_identity,
        "unix-only: requires Linux process ancestry and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        native_session_generations_sequences_helpers_and_idle_probes,
        "unix-only: requires Linux process ancestry and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        native_root_exit_invalidates_once_and_codex_keeps_last_accepted_report,
        "unix-only: requires Linux process exit observation and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        native_close_uses_one_deadline_and_strips_unacknowledged_live_clients,
        "unix-only: requires Linux process lifetime observation and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        native_close_accepts_only_a_post_probe_ack_and_retains_the_frozen_sample,
        "unix-only: requires Linux process lifetime observation and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        native_session_endpoint_rejects_a_reporter_from_another_real_pane,
        "unix-only: requires Linux foreground-process identity and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        detached_reporters_require_their_own_current_foreground_client_root,
        "unix-only: requires Linux process ancestry and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        native_close_rejects_an_old_challenge_even_when_the_sequence_increases,
        "unix-only: requires Linux process lifetime observation and Unix-domain client session sockets"
    );
    windows_unix_omission!(
        installed_session_client_probe,
        "unix-only: requires Linux process ancestry and Unix-domain client session sockets"
    );

    const CLAUDE_AUTHORIZED_MESSAGE: &str =
        "Responde solo OK, sin usar herramientas ni modificar archivos";

    fn own_assistant_ok(bytes: &[u8]) -> bool {
        let mut plain = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index..].starts_with(b"\x1b[") {
                index += 2;
                while index < bytes.len() && !(0x40..=0x7e).contains(&bytes[index]) {
                    index += 1;
                }
                if index < bytes.len() && matches!(bytes[index], b'H' | b'f' | b'E' | b'F') {
                    plain.push(b'\n');
                }
            } else if bytes[index..].starts_with(b"\x1b]") {
                index += 2;
                while index < bytes.len()
                    && bytes[index] != 7
                    && !bytes[index..].starts_with(b"\x1b\\")
                {
                    index += 1;
                }
                if bytes.get(index) == Some(&27) {
                    index += 1;
                }
            } else {
                plain.push(if bytes[index] == b'\r' {
                    b'\n'
                } else {
                    bytes[index]
                });
            }
            index += 1;
        }
        let mut marker = false;
        for line in String::from_utf8_lossy(&plain).lines() {
            let line = line.trim();
            if let Some(body) = line.strip_prefix(['⏺', '●', '•']) {
                if body.trim() == "OK" {
                    return true;
                }
                marker = body.trim().is_empty();
            } else if marker && line == "OK" {
                return true;
            } else if !line.is_empty() {
                marker = false;
            }
        }
        false
    }

    #[test]
    fn single_claude_reply_classifier_requires_an_assistant_marker_not_the_prompt_echo() {
        assert!(!own_assistant_ok(
            format!("❯ {CLAUDE_AUTHORIZED_MESSAGE}\r\n").as_bytes()
        ));
        assert!(own_assistant_ok("\u{1b}[1m⏺\u{1b}[0m OK\r\n".as_bytes()));
        assert!(own_assistant_ok("●\u{1b}[3;1HOK\r\n".as_bytes()));
        assert!(!own_assistant_ok("❯ OK\r\n".as_bytes()));
    }

    #[cfg(windows)]
    mod codex_probe {
        macro_rules! windows_unix_omission {
            ($name:ident, $reason:literal) => {
                #[test]
                #[ignore = $reason]
                fn $name() {}
            };
        }

        windows_unix_omission!(
            installed_codex_hook_stdout_preflight,
            "unix-only: requires Linux Unix-domain client sockets and process identity"
        );
        windows_unix_omission!(
            installed_codex_hook_trust_preflight,
            "unix-only: requires Linux Unix-domain client sockets and process identity"
        );
        windows_unix_omission!(
            installed_codex_resume_only_status_diagnostic,
            "unix-only: requires Linux Unix-domain client sockets and process identity"
        );
        windows_unix_omission!(
            installed_codex_one_message_capture_and_exact_resume,
            "unix-only: requires Linux Unix-domain client sockets and process identity"
        );

        fn codex_accepts_hook_stdout(bytes: &[u8]) -> bool {
            if bytes.iter().all(u8::is_ascii_whitespace) {
                return true;
            }
            let Ok(serde_json::Value::Object(fields)) = serde_json::from_slice(bytes) else {
                return false;
            };
            fields.iter().all(|(key, value)| match key.as_str() {
                "continue" | "suppressOutput" => value.is_boolean(),
                "stopReason" | "systemMessage" => value.is_string() || value.is_null(),
                "hookSpecificOutput" => value.as_object().is_some_and(|output| {
                    output
                        .get("hookEventName")
                        .is_some_and(|event| event == "SessionStart")
                        && output.iter().all(|(key, value)| {
                            key == "hookEventName"
                                || key == "additionalContext"
                                    && (value.is_string() || value.is_null())
                        })
                }),
                _ => false,
            })
        }
        fn own_ok(plain: &str) -> bool {
            plain.lines().any(|line| {
                line.trim()
                    .strip_prefix('•')
                    .is_some_and(|body| body.trim() == "OK")
            })
        }
        #[test]
        fn codex_probe_filters_reply_schema_and_does_not_count_prompt_echo_as_ok() {
            assert!(!codex_accepts_hook_stdout(br#"{"ok":true}"#));
            assert!(codex_accepts_hook_stdout(b""));
            assert!(codex_accepts_hook_stdout(br#"{"continue":true}"#));
            assert!(!own_ok(
                "Responde solo OK, sin usar herramientas ni modificar archivos"
            ));
            assert!(!own_ok("› OK"));
            assert!(own_ok("• OK"));
        }
        // Mirrors the readiness check of the Linux probe's own `Screen` fixture.
        fn composer_ready(terminal: &term_core::Screen) -> bool {
            let view = terminal.view();
            let Some(cursor) = view.cursor() else {
                return false;
            };
            if cursor.shape == term_core::CursorShape::Hidden {
                return false;
            }
            let prefix: String = (0..cursor.col)
                .filter_map(|col| view.cell(cursor.row, col))
                .map(|cell| cell.c)
                .collect();
            prefix.trim_start().starts_with('›')
        }
        #[test]
        fn codex_probe_readiness_requires_the_live_composer_cursor_not_history() {
            let mut screen = term_core::Screen::new(term_core::Size {
                cols: 160,
                rows: 44,
            });
            screen.advance("\x1b[1;1H› old prompt\x1b[40;1H\x1b[?25h".as_bytes());
            assert!(!composer_ready(&screen));
            screen.advance("› ".as_bytes());
            assert!(composer_ready(&screen));
            screen.advance(b"\x1b[?25l");
            assert!(!composer_ready(&screen));
        }
    }
}
