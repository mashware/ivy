use super::{Scope, ancestry, chain, inspection, process};
use std::io;
use std::os::windows::io::{AsHandle, BorrowedHandle, OwnedHandle};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

struct Root {
    handle: Arc<OwnedHandle>,
    identity: chain::Identity,
    stop: Weak<AtomicBool>,
    images: Vec<PathBuf>,
    inspection: inspection::Lane,
}

impl Root {
    fn current(&self) -> bool {
        self.stop
            .upgrade()
            .is_some_and(|s| !s.load(Ordering::Acquire))
            && process::is_alive(self.handle.as_handle())
    }
}

/// Evidence rooted in the actual child of an explicitly launched console.
#[derive(Clone)]
pub struct WindowsInvocation(Arc<Root>);

/// Admitted peer evidence, valid while its original console root remains live.
#[derive(Clone)]
pub struct WindowsInvocationWitness(Arc<Root>);

fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}

impl Scope {
    pub(in crate::foreground) fn fresh_invocation(
        &self,
        selected: &Path,
        recognized: &[PathBuf],
    ) -> io::Result<WindowsInvocation> {
        if !selected.is_absolute() || recognized.len() > 3 || !self.is_open() {
            return Err(denied());
        }
        let selected = std::fs::canonicalize(selected)?;
        let mut images = Vec::with_capacity(recognized.len() + 1);
        images.push(selected.clone());
        for image in recognized {
            if !image.is_absolute() {
                return Err(denied());
            }
            let image = std::fs::canonicalize(image)?;
            if !images.contains(&image) {
                images.push(image);
            }
        }
        if images.len() > 3 {
            return Err(denied());
        }
        let root_sequence = self.root_sequence.ok_or_else(denied)?;
        let actual = std::fs::canonicalize(process::executable(self.root.as_handle())?)?;
        if actual != selected
            || !self.is_open()
            || process::identity(self.root.as_handle())? != self.root_identity
            || ancestry::sequence(self.root.as_handle())? != root_sequence
        {
            return Err(denied());
        }
        Ok(WindowsInvocation(Arc::new(Root {
            handle: self.root.clone(),
            identity: chain::Identity {
                pid: self.root_identity.pid,
                birth: self.root_identity.birth,
                sequence: root_sequence,
            },
            stop: self.stop.clone(),
            images,
            inspection: inspection::Lane::default(),
        })))
    }
}

impl WindowsInvocation {
    /// Walk and revalidate a kernel-pinned peer chain away from the interface thread.
    pub fn attest(&self, peer: BorrowedHandle<'_>) -> io::Result<WindowsInvocationWitness> {
        self.attest_with_stop(peer, &AtomicBool::new(false))
    }

    /// D90 keeps filesystem inspection outside joined control workers.
    pub fn attest_with_stop(
        &self,
        peer: BorrowedHandle<'_>,
        stop: &AtomicBool,
    ) -> io::Result<WindowsInvocationWitness> {
        let peer = peer.try_clone_to_owned()?;
        let invocation = self.clone();
        inspection::run(
            &self.0.inspection,
            stop,
            || self.0.current(),
            inspection::DEADLINE,
            move || invocation.inspect(peer.as_handle()),
        )
    }

    fn inspect(&self, peer: BorrowedHandle<'_>) -> io::Result<WindowsInvocationWitness> {
        if !self.0.current() {
            return Err(denied());
        }
        let root = &self.0;
        let outcome = chain::walk(
            peer.try_clone_to_owned()?,
            root.identity,
            None,
            |handle| {
                let h = handle.as_handle();
                if !process::is_alive(h) {
                    return None;
                }
                let id = process::identity(h).ok()?;
                let sequence = ancestry::sequence(h).ok()?;
                let identity = chain::Identity {
                    pid: id.pid,
                    birth: id.birth,
                    sequence,
                };
                if identity != root.identity {
                    let image = std::fs::canonicalize(process::executable(h).ok()?).ok()?;
                    if root.images.contains(&image) {
                        return None;
                    }
                }
                Some(identity)
            },
            |handle| ancestry::parent(handle.as_handle()).ok(),
            |pid| process::open(pid).ok(),
        );
        if outcome != chain::Outcome::Inside || !root.current() {
            return Err(denied());
        }
        Ok(WindowsInvocationWitness(self.0.clone()))
    }
}

impl WindowsInvocationWitness {
    /// Bounded root lifetime and console revocation check; does not inspect the helper.
    pub fn current(&self) -> bool {
        self.0.current()
    }

    pub fn same_invocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
