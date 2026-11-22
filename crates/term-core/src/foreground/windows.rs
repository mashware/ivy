use super::{Ancestry, Argument};
use std::io;
use std::os::windows::io::{AsHandle, BorrowedHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};
mod ancestry;
mod arguments;
mod chain;
mod memory;
mod parameters;
mod process;
#[cfg(test)]
mod tests;

/// Captured native identity; birth is the process creation FILETIME.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub birth: u64,
}

pub(super) struct Scope {
    root: Arc<OwnedHandle>,
    root_identity: ProcessIdentity,
    anchor: Option<OwnedHandle>,
    anchor_identity: Option<ProcessIdentity>,
    stop: Weak<AtomicBool>,
}

pub(super) fn scope(pty: &crate::Pty) -> io::Result<Scope> {
    let raw = pty
        .child
        .as_ref()
        .and_then(|child| child.as_raw_handle())
        .ok_or(io::ErrorKind::PermissionDenied)?;
    // SAFETY: the child owns this handle for the duration of the borrow and duplication.
    let root = unsafe { BorrowedHandle::borrow_raw(raw) }.try_clone_to_owned()?;
    let root_identity = process::identity(root.as_handle())?;
    if !process::is_alive(root.as_handle()) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let parent_pid = ancestry::parent(root.as_handle()).unwrap_or(0);
    let (anchor, anchor_identity) = if parent_pid == std::process::id() {
        let handle = process::open(parent_pid)?;
        let identity = process::identity(handle.as_handle())?;
        if !process::is_alive(handle.as_handle())
            || ancestry::parent(root.as_handle())? != identity.pid
            || identity.birth >= root_identity.birth
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        (Some(handle), Some(identity))
    } else {
        (None, None)
    };
    Ok(Scope {
        root: Arc::new(root),
        root_identity,
        anchor,
        anchor_identity,
        stop: Arc::downgrade(&pty.stop),
    })
}
impl Scope {
    pub(super) fn is_open(&self) -> bool {
        self.stop
            .upgrade()
            .is_some_and(|s| !s.load(Ordering::Acquire))
            && process::is_alive(self.root.as_handle())
    }
    pub(super) fn classify_ancestry(&self, pid: u32) -> Ancestry {
        match process::open(pid) {
            Ok(handle) => ancestry::classify(self, handle),
            Err(_) => Ancestry::Unknown,
        }
    }
    pub(super) fn observe_process(&self, pid: u32) -> io::Result<ProcessObservation> {
        let handle = process::open(pid)?;
        let id = process::identity(handle.as_handle())?;
        if ancestry::classify(self, handle.as_handle().try_clone_to_owned()?) != Ancestry::Inside
            || !self.is_open()
            || !process::is_alive(handle.as_handle())
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(ProcessObservation {
            handle,
            id,
            root: self.root.clone(),
            stop: self.stop.clone(),
        })
    }
}
/// A pinned diagnostic observation, revoked with its console or root process.
/// It never proves that a program is in the foreground.
pub struct ProcessObservation {
    handle: OwnedHandle,
    id: ProcessIdentity,
    root: Arc<OwnedHandle>,
    stop: Weak<AtomicBool>,
}
impl ProcessObservation {
    pub fn identity(&self) -> ProcessIdentity {
        self.id
    }
    pub fn is_alive(&self) -> bool {
        self.stop
            .upgrade()
            .is_some_and(|s| !s.load(Ordering::Acquire))
            && process::is_alive(self.root.as_handle())
            && process::is_alive(self.handle.as_handle())
            && process::identity(self.handle.as_handle()).ok() == Some(self.id)
    }
    fn checked<T>(&self, f: impl FnOnce(BorrowedHandle<'_>) -> io::Result<T>) -> io::Result<T> {
        if !self.is_alive() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let result = f(self.handle.as_handle());
        if !self.is_alive() {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        result
    }
    pub fn executable(&self) -> io::Result<PathBuf> {
        self.checked(process::executable)
    }
    pub fn working_directory(&self) -> io::Result<PathBuf> {
        self.checked(parameters::working_directory)
    }
    pub fn matches_arguments(&self, shapes: &[&[Argument<'_>]]) -> io::Result<bool> {
        self.checked(|h| parameters::matches_arguments(h, shapes))
    }
    pub fn runs_script(&self, path: &Path) -> io::Result<bool> {
        self.checked(|h| parameters::runs_script(h, path))
    }
    /// The handle signals process exit only; it does not signal console revocation.
    pub fn lifetime(&self) -> io::Result<OwnedHandle> {
        self.checked(|h| h.try_clone_to_owned())
    }
}
