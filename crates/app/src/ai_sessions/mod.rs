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
use std::time::{Duration, Instant};
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
    let deadline = Instant::now() + timeout;
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
