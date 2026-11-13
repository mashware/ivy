use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::{Child, ChildStdout, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// Failed discovery must not accumulate cleanup children while a slow or unkillable
// earlier resolver still owns OS resources; new discovery fails closed at this cap.
const MAX_CHILDREN: usize = 4;
static RESERVED: AtomicUsize = AtomicUsize::new(0);
static PENDING: Mutex<Vec<Pending>> = Mutex::new(Vec::new());

struct Slot;

impl Slot {
    fn reserve() -> io::Result<Self> {
        RESERVED
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_CHILDREN).then_some(count + 1)
            })
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "resolver cleanup capacity is full",
                )
            })?;
        Ok(Self)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        RESERVED.fetch_sub(1, Ordering::AcqRel);
    }
}

struct Pending {
    child: Child,
    lifetime: Option<Arc<OwnedFd>>,
    _slot: Slot,
}

pub(super) struct Running {
    pub(super) child: Option<Child>,
    pub(super) stdout: ChildStdout,
    pub(super) lifetime: Option<Arc<OwnedFd>>,
    slot: Option<Slot>,
}

impl Running {
    pub(super) fn start(command: &mut Command) -> io::Result<Self> {
        reap();
        let slot = Slot::reserve()?;
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().expect("resolver stdout is piped");
        let mut running = Self {
            child: Some(child),
            stdout,
            lifetime: None,
            slot: Some(slot),
        };
        // SAFETY: pidfd_open returns an owned descriptor for the unreaped child just spawned.
        let descriptor = unsafe {
            libc::syscall(
                libc::SYS_pidfd_open,
                running.child.as_ref().unwrap().id(),
                0,
            )
        };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: success above transfers this unique descriptor to the owner.
        running.lifetime = Some(Arc::new(unsafe { OwnedFd::from_raw_fd(descriptor as i32) }));
        let fd = running.stdout.as_raw_fd();
        // SAFETY: fcntl reads/changes flags on the owned stdout pipe descriptor.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(running)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if reaped(&mut child) {
            return;
        }
        let _ = child.kill();
        if reaped(&mut child) {
            return;
        }
        PENDING
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(Pending {
                child,
                lifetime: self.lifetime.take(),
                _slot: self.slot.take().unwrap(),
            });
        crate::ai_sessions::reap_later();
    }
}

fn reaped(child: &mut Child) -> bool {
    match child.try_wait() {
        Ok(Some(_)) => true,
        Err(error) if error.raw_os_error() == Some(libc::ECHILD) => true,
        _ => false,
    }
}

pub(super) fn reap() {
    PENDING
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .retain_mut(|pending| !reaped(&mut pending.child));
}

pub(super) fn watched() -> (Vec<Arc<OwnedFd>>, bool) {
    let pending = PENDING.lock().unwrap_or_else(|error| error.into_inner());
    (
        pending
            .iter()
            .filter_map(|pending| pending.lifetime.clone())
            .collect(),
        pending.iter().any(|pending| pending.lifetime.is_none()),
    )
}

#[cfg(test)]
mod tests;
