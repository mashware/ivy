//! Native foreground evidence, bounded to one console's ancestor chain.

use std::io;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(all(test, target_os = "linux"))]
mod tests;

/// Owns its descriptor while borrowing the console's revocable lifetime.
pub struct ForegroundScope {
    #[cfg(target_os = "linux")]
    native: linux::Scope,
}

/// Process identity includes kernel birth ticks so a recycled PID cannot inherit authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeProcess {
    pub pid: u32,
    pub birth: u64,
    pub parent: u32,
    pub group: u32,
    pub session: u32,
    pub executable: PathBuf,
    /// This process belongs to the pane's current foreground group, session and terminal.
    pub foreground: bool,
}

/// Processes ordered from the console's original child to the kernel-attested reporter.
pub struct ForegroundAncestry {
    pub foreground_group: u32,
    pub processes: Vec<NativeProcess>,
}

/// Only structural CLI tokens are read; an unexpected prefix stops before later payload arguments.
#[derive(Clone, Copy)]
pub enum Argument<'a> {
    Literal(&'a str),
    Uuid,
    /// Consumes one nonempty argument without keeping or interpreting its bytes.
    Opaque {
        max_bytes: usize,
    },
}

pub(crate) fn scope(pty: &crate::Pty) -> io::Result<ForegroundScope> {
    #[cfg(target_os = "linux")]
    return linux::scope(pty).map(|native| ForegroundScope { native });
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pty;
        Err(io::ErrorKind::Unsupported.into())
    }
}

impl ForegroundScope {
    pub fn is_open(&self) -> bool {
        #[cfg(target_os = "linux")]
        return self.native.is_open();
        #[cfg(not(target_os = "linux"))]
        false
    }

    /// The caller must obtain `pid` and `birth` from native socket credentials.
    pub fn inspect(&self, pid: u32, birth: u64) -> io::Result<ForegroundAncestry> {
        #[cfg(target_os = "linux")]
        return self.native.inspect(pid, birth);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (pid, birth);
            Err(io::ErrorKind::Unsupported.into())
        }
    }

    /// Attests a descendant chain; only processes marked foreground may grant client authority.
    /// Helpers can start a new session, but cannot supply or replace a foreground root.
    pub fn inspect_descendant(&self, pid: u32, birth: u64) -> io::Result<ForegroundAncestry> {
        #[cfg(target_os = "linux")]
        return self.native.inspect_descendant(pid, birth);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (pid, birth);
            Err(io::ErrorKind::Unsupported.into())
        }
    }
}

#[cfg(target_os = "linux")]
impl NativeProcess {
    /// Reads this exact process's directory, rather than a wrapper's foreground-group leader.
    pub fn working_directory(&self) -> io::Result<PathBuf> {
        linux::working_directory(self)
    }

    pub fn matches_arguments(&self, shapes: &[&[Argument<'_>]]) -> io::Result<bool> {
        linux::matches_arguments(self, shapes)
    }

    /// Call only after establishing a trusted interpreter executable; reads its script slot only.
    pub fn runs_script(&self, expected: &std::path::Path) -> io::Result<bool> {
        linux::runs_script(self, expected)
    }

    /// Pins this exact process for an event-driven exit notification.
    pub fn lifetime(&self) -> io::Result<std::os::fd::OwnedFd> {
        linux::lifetime(self.pid, self.birth)
    }
}
