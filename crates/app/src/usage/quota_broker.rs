//! One private, revocable receiver per explicitly launched Claude process.

use panel_control::{
    quota::{self, Admission, Frame, MAX_FRAME, Outcome, Window},
    session::Peer,
};
use std::{
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, OwnedFd, RawFd},
        unix::net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use term_core::ForegroundScope;

const FRAME_DEADLINE: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum WindowState {
    Absent,
    Expired,
    Reported(Window),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Snapshot {
    Waiting,
    Observed {
        received_at: i64,
        five_hour: WindowState,
        seven_day: WindowState,
    },
    ProducerError,
    Disconnected,
}

/// Shared by all quota launches in one window. One pending repaint covers every visible change.
pub(super) struct Wake {
    state: Mutex<WakeState>,
    repaint: Box<dyn Fn() + Send + Sync>,
}

#[derive(Default)]
struct WakeState {
    visible: bool,
    pending: bool,
}

impl Wake {
    pub(super) fn new(repaint: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::default(),
            repaint: Box::new(repaint),
        })
    }

    pub(super) fn set_visible(&self, visible: bool) {
        let mut state = self.state.lock().unwrap();
        state.visible = visible;
        if !visible {
            state.pending = false;
        }
    }

    /// Clear before taking snapshots so a later worker update schedules its own repaint.
    pub(super) fn begin_frame(&self) {
        self.state.lock().unwrap().pending = false;
    }

    pub(super) fn changed(&self) {
        let mut state = self.state.lock().unwrap();
        if state.visible && !state.pending {
            state.pending = true;
            // Serialize the request with hiding the view. The callback queues a repaint.
            (self.repaint)();
        }
    }
}

struct Root {
    scope: ForegroundScope,
    lifetime: OwnedFd,
    executable: PathBuf,
}

impl Root {
    fn is_alive(&self) -> bool {
        let mut event = [poll_input(self.lifetime.as_raw_fd())];
        // SAFETY: the pidfd is pinned by Root for the duration of the nonblocking poll.
        let ready = unsafe { libc::poll(event.as_mut_ptr(), 1, 0) };
        ready == 0 && event[0].revents == 0
    }

    fn owns(&self, peer: &Peer) -> bool {
        peer.is_alive()
            && self
                .scope
                .inspect_descendant(peer.pid(), peer.birth())
                .is_ok_and(|chain| {
                    chain.processes.len() > 1 && chain.processes[0].executable == self.executable
                })
            && peer.is_alive()
    }
}

struct Model {
    root: Option<Arc<Root>>,
    snapshot: Snapshot,
    stopped: bool,
}

struct BrokerState {
    model: Mutex<Model>,
    wake: Arc<Wake>,
}

impl BrokerState {
    fn finish(&self, snapshot: Snapshot) {
        let mut model = self.model.lock().unwrap();
        if model.stopped
            && (model.snapshot == Snapshot::Disconnected || snapshot != Snapshot::Disconnected)
        {
            return;
        }
        model.stopped = true;
        model.snapshot = snapshot;
        model.root = None;
        drop(model);
        self.wake.changed();
    }

    fn publish(&self, root: &Arc<Root>, frame: Frame) {
        let mut model = self.model.lock().unwrap();
        if model.stopped
            || !model
                .root
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, root))
            || !root.scope.is_open()
            || !root.is_alive()
        {
            return;
        }
        let now = unix_seconds();
        model.snapshot = match frame.outcome {
            Outcome::Observed {
                five_hour,
                seven_day,
            } => Snapshot::Observed {
                received_at: now,
                five_hour: observed(five_hour, now),
                seven_day: observed(seven_day, now),
            },
            Outcome::ProducerError => Snapshot::ProducerError,
        };
        drop(model);
        self.wake.changed();
    }

    fn expire(&self) -> Option<Duration> {
        let mut model = self.model.lock().unwrap();
        let mut changed = false;
        let next = if let Snapshot::Observed {
            five_hour,
            seven_day,
            ..
        } = &mut model.snapshot
        {
            let now = chrono::Utc::now();
            changed = expire(five_hour, now.timestamp()) | expire(seven_day, now.timestamp());
            [deadline(five_hour), deadline(seven_day)]
                .into_iter()
                .flatten()
                .min()
                .map(|deadline| wait_time(deadline, now))
        } else {
            None
        };
        drop(model);
        if changed {
            self.wake.changed();
        }
        next
    }
}

pub(super) struct Broker {
    state: Arc<BrokerState>,
    signal: UnixStream,
    endpoint: PathBuf,
}

impl Broker {
    pub(super) fn prepare(wake: Arc<Wake>) -> io::Result<Self> {
        let directory = panel_contract::PanelDir::create()?;
        panel_control::endpoint::private_to_this_user(directory.path())?;
        let endpoint = directory.path().join(panel_contract::CONTROL_SOCKET);
        let listener = UnixListener::bind(&endpoint)?;
        listener.set_nonblocking(true)?;
        let (signal, receive_signal) = UnixStream::pair()?;
        quota::capture_peer(&receive_signal).map_err(|error| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "Claude allowance reporting needs Linux socket peer verification ({error})."
                ),
            )
        })?;
        signal.set_nonblocking(true)?;
        receive_signal.set_nonblocking(true)?;
        let state = Arc::new(BrokerState {
            model: Mutex::new(Model {
                root: None,
                snapshot: Snapshot::Waiting,
                stopped: false,
            }),
            wake,
        });
        let worker_state = state.clone();
        std::thread::Builder::new()
            .name("ivy-claude-quota".into())
            .spawn(move || {
                // The listener and its pathname are retired together on every exit.
                let _directory = directory;
                if run(&worker_state, listener, receive_signal).is_err() {
                    worker_state.finish(Snapshot::ProducerError);
                }
            })?;
        Ok(Self {
            state,
            signal,
            endpoint,
        })
    }

    pub(super) fn endpoint(&self) -> &Path {
        &self.endpoint
    }

    /// The supplied scope must be for the dedicated launch, whose original child execs Claude.
    pub(super) fn attach(&self, scope: ForegroundScope, executable: PathBuf) -> io::Result<()> {
        let result = (|| {
            if !scope.is_open() {
                return Err(io::ErrorKind::NotConnected.into());
            }
            let lifetime = scope.root_lifetime()?;
            let mut model = self.state.model.lock().unwrap();
            if model.stopped || model.root.is_some() {
                return Err(io::Error::other("quota launch is already bound or stopped"));
            }
            model.root = Some(Arc::new(Root {
                scope,
                lifetime,
                executable,
            }));
            Ok(())
        })();
        if result.is_err() {
            self.startup_failed();
        } else {
            // Before attachment the listener stays unpolled. Connections already in its backlog
            // are considered only after the original process has been registered.
            let _ = (&self.signal).write_all(&[1]);
        }
        result
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        let mut snapshot = self.state.model.lock().unwrap().snapshot;
        if let Snapshot::Observed {
            five_hour,
            seven_day,
            ..
        } = &mut snapshot
        {
            // The first frame after opening is correct even if the worker's deadline just fired.
            let now = unix_seconds();
            expire(five_hour, now);
            expire(seven_day, now);
        }
        snapshot
    }

    pub(super) fn startup_failed(&self) {
        self.finish(Snapshot::ProducerError);
    }

    pub(super) fn stop(&self) {
        self.finish(Snapshot::Disconnected);
    }

    fn finish(&self, snapshot: Snapshot) {
        self.state.finish(snapshot);
        let _ = self.signal.shutdown(std::net::Shutdown::Both);
    }
}

impl Drop for Broker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(state: &BrokerState, listener: UnixListener, signal: UnixStream) -> io::Result<()> {
    let mut sequence = 0u64;
    loop {
        let root = {
            let model = state.model.lock().unwrap();
            if model.stopped {
                return Ok(());
            }
            model.root.clone()
        };
        let root_fd = root.as_ref().map_or(-1, |root| root.lifetime.as_raw_fd());
        let listener_fd = root.as_ref().map_or(-1, |_| listener.as_raw_fd());
        let mut events = [
            poll_input(signal.as_raw_fd()),
            poll_input(root_fd),
            poll_input(listener_fd),
        ];
        wait(&mut events, state.expire())?;
        if events[0].revents != 0 {
            if stopped(&signal) {
                state.finish(Snapshot::ProducerError);
                return Ok(());
            }
            continue;
        }
        if events[1].revents != 0 {
            state.finish(Snapshot::Disconnected);
            return Ok(());
        }
        if events[2].revents != 0
            && let Some(root) = root
        {
            let (stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                Err(error) => return Err(error),
            };
            if let Ok(frame) = receive(stream, state, &root, &signal, &mut sequence) {
                state.publish(&root, frame);
            }
        }
    }
}

fn receive(
    stream: UnixStream,
    state: &BrokerState,
    root: &Root,
    signal: &UnixStream,
    sequence: &mut u64,
) -> io::Result<Frame> {
    let deadline = Instant::now() + FRAME_DEADLINE;
    let peer = quota::capture_peer(&stream)?;
    if !root.owns(&peer) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    stream.set_nonblocking(true)?;
    *sequence = sequence
        .checked_add(1)
        .ok_or_else(|| io::Error::other("quota sequence exhausted"))?;
    quota::write_line(
        &stream,
        &Admission {
            v: 1,
            sequence: *sequence,
        },
        MAX_FRAME,
    )?;
    let reader = FrameReader {
        stream,
        state,
        root,
        signal,
        deadline,
    };
    let frame: Frame = serde_json::from_slice(&quota::read_line(reader, MAX_FRAME)?)?;
    // A legitimate one-shot helper can exit as soon as it has sent this frame. The socket is
    // already bound to the admitted peer. The app checks the root and revocation again at apply.
    if Instant::now() >= deadline || frame.sequence != *sequence {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(frame)
}

struct FrameReader<'a> {
    stream: UnixStream,
    state: &'a BrokerState,
    root: &'a Root,
    signal: &'a UnixStream,
    deadline: Instant,
}

impl Read for FrameReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut events = [
            poll_input(self.signal.as_raw_fd()),
            poll_input(self.root.lifetime.as_raw_fd()),
            poll_input(self.stream.as_raw_fd()),
        ];
        loop {
            let now = Instant::now();
            if now >= self.deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let frame_wait = self.deadline - now;
            let wait_for = self
                .state
                .expire()
                .map_or(frame_wait, |expiry| expiry.min(frame_wait));
            wait(&mut events, Some(wait_for))?;
            if (events[0].revents != 0 && stopped(self.signal)) || events[1].revents != 0 {
                return Err(io::ErrorKind::NotConnected.into());
            }
            if events[2].revents != 0 {
                match self.stream.read(buffer) {
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                    result => return result,
                }
            }
        }
    }
}

fn stopped(mut signal: &UnixStream) -> bool {
    match signal.read(&mut [0; 4]) {
        Ok(size) => size == 0,
        Err(error) => !matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
        ),
    }
}

fn poll_input(fd: RawFd) -> libc::pollfd {
    libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    }
}

fn wait(events: &mut [libc::pollfd], timeout: Option<Duration>) -> io::Result<()> {
    let timeout = timeout.map_or(-1, |timeout| {
        i32::try_from(timeout.as_nanos().div_ceil(1_000_000)).unwrap_or(i32::MAX)
    });
    // SAFETY: the slice remains writable and each live descriptor is held by its owner.
    if unsafe { libc::poll(events.as_mut_ptr(), events.len() as _, timeout) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
        for event in events {
            event.revents = 0;
        }
    }
    Ok(())
}

fn unix_seconds() -> i64 {
    chrono::Utc::now().timestamp()
}

fn wait_time(deadline: i64, now: chrono::DateTime<chrono::Utc>) -> Duration {
    let remaining = deadline.saturating_sub(now.timestamp());
    Duration::from_secs(u64::try_from(remaining).unwrap_or_default()).saturating_sub(
        Duration::from_nanos(u64::from(now.timestamp_subsec_nanos())),
    )
}

fn observed(window: Option<Window>, now: i64) -> WindowState {
    let Some(window) = window else {
        return WindowState::Absent;
    };
    let mut state = WindowState::Reported(window);
    expire(&mut state, now);
    state
}

fn deadline(window: &WindowState) -> Option<i64> {
    match window {
        WindowState::Reported(window) => window.resets_at,
        WindowState::Expired | WindowState::Absent => None,
    }
}

fn expire(window: &mut WindowState, now: i64) -> bool {
    if deadline(window).is_some_and(|time| now >= time) {
        *window = WindowState::Expired;
        return true;
    }
    false
}

#[cfg(test)]
mod tests;
