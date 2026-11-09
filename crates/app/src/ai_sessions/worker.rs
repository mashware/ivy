use super::{Handle, Shared, Status, error, programs, protocol, verify};
use panel_control::session::{Peer, Request};
use std::collections::HashMap;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};

type Ticket = (Arc<Shared>, Arc<AtomicBool>);
type Reply = mpsc::Sender<serde_json::Value>;

enum Command {
    Refresh(Weak<Shared>),
    Request(Arc<Shared>, u64, Peer, Request, Reply),
    Prepare(Vec<Arc<Shared>>, mpsc::Sender<Vec<Ticket>>),
}

struct Service {
    sender: SyncSender<Command>,
    knock: OwnedFd,
}
static SERVICE: OnceLock<Option<Service>> = OnceLock::new();

pub(super) struct Entry {
    pub(super) shared: Weak<Shared>,
    pub(super) epoch: u64,
    pub(super) programs: Vec<programs::Program>,
    pub(super) root: Option<verify::Root>,
    pub(super) generation: Option<String>,
    pub(super) sequence: u64,
    pub(super) challenge: Option<String>,
    pub(super) acknowledged: Arc<AtomicBool>,
    pub(super) frozen: bool,
}

impl Entry {
    fn new(shared: &Arc<Shared>) -> Self {
        let programs = shared
            .supplied
            .clone()
            .map(programs::supplied)
            .unwrap_or_else(programs::discover);
        if programs.is_empty() && shared.enabled.load(Ordering::Acquire) {
            shared.publish(None, None, Status::Unsupported);
        }
        Self {
            shared: Arc::downgrade(shared),
            epoch: shared.epoch.load(Ordering::Acquire),
            programs,
            root: None,
            generation: None,
            sequence: 0,
            challenge: None,
            acknowledged: Arc::clone(&shared.close_acknowledged),
            frozen: false,
        }
    }

    pub(super) fn revoke(&mut self, shared: &Shared) {
        self.root = None;
        self.generation = None;
        self.sequence = 0;
        self.challenge = None;
        self.acknowledged.store(true, Ordering::Release);
        if !self.frozen && !shared.frozen.load(Ordering::Acquire) {
            let previous = shared
                .published
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            shared.publish(None, previous.cwd, Status::Waiting);
        }
        shared.changed.notify_all();
    }
}

pub(super) fn request(shared: Arc<Shared>, peer: Peer, request: Request) -> serde_json::Value {
    let Some(service) = service() else {
        return error("unsupported");
    };
    let (reply, answer) = mpsc::channel();
    let epoch = shared.epoch.load(Ordering::Acquire);
    if service
        .send(Command::Request(shared, epoch, peer, request, reply))
        .is_err()
    {
        return error("busy");
    }
    answer
        .recv_timeout(Duration::from_millis(1800))
        .unwrap_or_else(|_| error("busy"))
}

pub(super) fn refresh(shared: &Arc<Shared>) {
    if !shared.enabled.load(Ordering::Acquire) {
        notify_existing();
        return;
    }
    if let Some(service) = service() {
        let _ = service.send(Command::Refresh(Arc::downgrade(shared)));
    }
}

pub(super) fn notify_existing() {
    if let Some(Some(service)) = SERVICE.get() {
        service.notify();
    }
}

pub(super) fn prepare_close(handles: &[&Handle], deadline: Instant) -> Vec<Ticket> {
    let Some(Some(service)) = SERVICE.get() else {
        return Vec::new();
    };
    let (reply, answer) = mpsc::channel();
    if service
        .send(Command::Prepare(
            handles
                .iter()
                .map(|handle| Arc::clone(&handle.shared))
                .collect(),
            reply,
        ))
        .is_err()
    {
        return Vec::new();
    }
    answer
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or_default()
}

fn service() -> Option<&'static Service> {
    SERVICE
        .get_or_init(|| {
            let (read, write) = io::pipe().ok()?;
            for fd in [read.as_raw_fd(), write.as_raw_fd()] {
                // SAFETY: flags are read and changed on live pipe descriptors owned above.
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                if flags < 0
                    || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
                {
                    return None;
                }
            }
            let (sender, receiver) = mpsc::sync_channel(64);
            std::thread::Builder::new()
                .name("ai-session-verifier".into())
                .spawn(move || run(read, receiver))
                .ok()?;
            Some(Service {
                sender,
                knock: write.into(),
            })
        })
        .as_ref()
}

impl Service {
    fn send(&self, command: Command) -> Result<(), ()> {
        self.sender.try_send(command).map_err(|_| ())?;
        self.notify();
        Ok(())
    }
    fn notify(&self) {
        // SAFETY: the live pipe writer receives one stack byte. A full pipe is already a wakeup.
        unsafe { libc::write(self.knock.as_raw_fd(), [1u8].as_ptr().cast(), 1) };
    }
}

fn run(mut pipe: io::PipeReader, receiver: Receiver<Command>) {
    let mut entries: HashMap<usize, Entry> = HashMap::new();
    loop {
        cleanup(&mut entries);
        let mut watched = vec![poll(pipe.as_raw_fd())];
        for entry in entries.values() {
            if let Some(root) = &entry.root {
                watched.push(poll(root.lifetime.as_raw_fd()));
            }
        }
        // SAFETY: initialized poll descriptors remain owned by the entries until this wait ends.
        let ready = unsafe { libc::poll(watched.as_mut_ptr(), watched.len() as libc::nfds_t, -1) };
        if ready < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
        let mut bytes = [0; 128];
        while pipe.read(&mut bytes).is_ok_and(|n| n > 0) {}
        cleanup(&mut entries);
        // Draining a fixed queue bounds work before liveness is reconsidered.
        for _ in 0..64 {
            let Ok(command) = receiver.try_recv() else {
                break;
            };
            handle(command, &mut entries);
        }
    }
}

fn handle(command: Command, entries: &mut HashMap<usize, Entry>) {
    match command {
        Command::Refresh(weak) => {
            if let Some(shared) = weak.upgrade() {
                entry(entries, &shared);
            }
        }
        Command::Request(shared, epoch, peer, request, reply) => {
            let result = if !shared.enabled.load(Ordering::Acquire)
                || shared.epoch.load(Ordering::Acquire) != epoch
            {
                error("disabled")
            } else {
                let entry = entry(entries, &shared);
                if entry.epoch != epoch {
                    error("disabled")
                } else {
                    protocol::apply(entry, &shared, peer, request)
                }
            };
            let _ = reply.send(result);
        }
        Command::Prepare(handles, reply) => {
            let tickets = handles
                .into_iter()
                .filter_map(|shared| {
                    if shared.frozen.load(Ordering::Acquire) {
                        return None;
                    }
                    let entry = entries.get_mut(&key(&shared))?;
                    let root = entry.root.as_ref()?;
                    if !shared.scope.is_open() || !root.current(&shared.scope) {
                        entry.revoke(&shared);
                        return None;
                    }
                    if root.client == panel_control::session::Client::Codex {
                        entry.frozen = true;
                        entry.acknowledged.store(true, Ordering::Release);
                        return None;
                    }
                    entry.challenge = Some(protocol::token().ok()?);
                    entry.acknowledged.store(false, Ordering::Release);
                    Some((shared, Arc::clone(&entry.acknowledged)))
                })
                .collect();
            let _ = reply.send(tickets);
        }
    }
}

fn entry<'a>(entries: &'a mut HashMap<usize, Entry>, shared: &Arc<Shared>) -> &'a mut Entry {
    let entry = entries
        .entry(key(shared))
        .or_insert_with(|| Entry::new(shared));
    if entry.epoch != shared.epoch.load(Ordering::Acquire) {
        *entry = Entry::new(shared);
    }
    entry
}

fn key(shared: &Arc<Shared>) -> usize {
    Arc::as_ptr(shared) as usize
}
fn poll(fd: i32) -> libc::pollfd {
    libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    }
}

fn cleanup(entries: &mut HashMap<usize, Entry>) {
    entries.retain(|_, entry| {
        let Some(shared) = entry.shared.upgrade() else {
            return false;
        };
        if !shared.enabled.load(Ordering::Acquire) {
            return false;
        }
        if !shared.scope.is_open() {
            entry.revoke(&shared);
            return false;
        }
        if entry.root.as_ref().is_some_and(|root| {
            let mut watched = poll(root.lifetime.as_raw_fd());
            // SAFETY: the root owns this pidfd; a zero timeout only observes its readiness.
            unsafe { libc::poll(&mut watched, 1, 0) != 0 }
        }) {
            entry.revoke(&shared);
        }
        true
    });
}
