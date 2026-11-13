//! Native foreground evidence, bounded to one console's ancestor chain.

use std::io;
use std::path::PathBuf;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(test, target_os = "linux"))]
mod tests;

#[cfg(test)]
mod ancestry_tests {
    use super::{Ancestry, classify_snapshots};
    use std::{cell::Cell, collections::HashMap, io};

    #[test]
    fn snapshot_walk_distinguishes_inside_and_outside_chains() {
        let inside = HashMap::from([(1, (1, 0, 10)), (2, (2, 1, 20)), (3, (3, 2, 30))]);
        assert_eq!(
            classify_snapshots(3, inside[&2], |pid| inside
                .get(&pid)
                .copied()
                .ok_or_else(|| io::ErrorKind::NotFound.into()))
            .unwrap(),
            Ancestry::Inside
        );
        let outside = HashMap::from([(1, (1, 0, 10)), (7, (7, 1, 70))]);
        assert_eq!(
            classify_snapshots(7, (2, 0, 20), |pid| {
                if pid == 2 {
                    Ok((2, 0, 20))
                } else {
                    outside
                        .get(&pid)
                        .copied()
                        .ok_or_else(|| io::ErrorKind::NotFound.into())
                }
            })
            .unwrap(),
            Ancestry::Outside
        );
    }

    #[test]
    fn snapshot_walk_rejects_missing_reparented_reused_exited_and_deep_chains() {
        let missing = HashMap::from([(2, (2, 1, 20)), (3, (3, 99, 30))]);
        assert!(
            classify_snapshots(3, (2, 1, 20), |pid| missing
                .get(&pid)
                .copied()
                .ok_or_else(|| io::ErrorKind::NotFound.into()))
            .is_err()
        );

        let reads = Cell::new(0);
        assert!(
            classify_snapshots(3, (2, 1, 20), |pid| {
                if pid == 3 {
                    let n = reads.get();
                    reads.set(n + 1);
                    Ok(if n == 0 { (3, 2, 30) } else { (3, 1, 30) })
                } else if pid == 2 {
                    Ok((2, 1, 20))
                } else {
                    Ok((1, 0, 10))
                }
            })
            .is_err(),
            "a changed parent link is uncertain"
        );

        let reused = HashMap::from([(1, (1, 0, 10)), (2, (2, 1, 21)), (3, (3, 2, 30))]);
        assert!(
            classify_snapshots(3, (2, 1, 20), |pid| reused
                .get(&pid)
                .copied()
                .ok_or_else(|| io::ErrorKind::NotFound.into()))
            .is_err(),
            "root birth mismatch is uncertain"
        );
        assert!(
            classify_snapshots(3, (2, 1, 20), |_| Err(io::ErrorKind::NotFound.into())).is_err(),
            "exit during inspection is uncertain"
        );

        let mut deep = HashMap::from([(1, (1, 0, 1))]);
        for pid in 2..=67 {
            deep.insert(pid, (pid, pid - 1, pid as i32));
        }
        assert!(
            classify_snapshots(67, (100, 0, 100), |pid| deep
                .get(&pid)
                .copied()
                .or_else(|| (pid == 100).then_some((100, 0, 100)))
                .ok_or_else(|| io::ErrorKind::NotFound.into()),)
            .is_err(),
            "more than 64 hops is uncertain"
        );
    }
}

/// Best-effort ancestry observation for diagnostics. Unknown never affects authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ancestry {
    Inside,
    Outside,
    Unknown,
}

fn classify_snapshots<B: Copy + Eq>(
    peer: u32,
    root: (u32, u32, B),
    mut read: impl FnMut(u32) -> io::Result<(u32, u32, B)>,
) -> io::Result<Ancestry> {
    if peer == 0 || root.0 == 0 || read(root.0)? != root {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let first = read(peer)?;
    let mut snapshots = Vec::new();
    let mut current = first;
    let verdict = loop {
        if current.0 == 0 || snapshots.len() == 64 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        snapshots.push(current);
        if current.0 == root.0 {
            if current != root {
                return Err(io::ErrorKind::InvalidData.into());
            }
            break Ancestry::Inside;
        }
        if current.0 == 1 {
            break Ancestry::Outside;
        }
        if current.1 == 0 || current.1 == current.0 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let parent = read(current.1)?;
        if parent.0 != current.1 || read(current.0)? != current {
            return Err(io::ErrorKind::InvalidData.into());
        }
        current = parent;
    };
    for snapshot in snapshots {
        if read(snapshot.0)? != snapshot {
            return Err(io::ErrorKind::InvalidData.into());
        }
    }
    if read(peer)? != first || read(root.0)? != root {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(verdict)
}

/// Owns its descriptor while borrowing the console's revocable lifetime.
pub struct ForegroundScope {
    #[cfg(target_os = "linux")]
    native: linux::Scope,
    #[cfg(target_os = "macos")]
    native: macos::Scope,
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
    #[cfg(target_os = "macos")]
    return macos::scope(pty).map(|native| ForegroundScope { native });
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pty;
        Err(io::ErrorKind::Unsupported.into())
    }
}

impl ForegroundScope {
    /// Classifies process ancestry without foreground-group or executable restrictions.
    /// Callers should supply a PID obtained from native socket credentials.
    pub fn classify_ancestry(&self, pid: u32) -> Ancestry {
        #[cfg(target_os = "linux")]
        return self.native.classify_ancestry(pid);
        #[cfg(target_os = "macos")]
        return self.native.classify_ancestry(pid);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = pid;
            Ancestry::Unknown
        }
    }

    pub fn is_open(&self) -> bool {
        #[cfg(target_os = "linux")]
        return self.native.is_open();
        #[cfg(target_os = "macos")]
        return self.native.is_open();
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        false
    }

    /// The caller must obtain `pid` and `birth` from native socket credentials.
    pub fn inspect(&self, pid: u32, birth: u64) -> io::Result<ForegroundAncestry> {
        #[cfg(target_os = "linux")]
        return self.native.inspect(pid, birth);
        #[cfg(target_os = "macos")]
        return self.native.inspect(pid, birth, false);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
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
        #[cfg(target_os = "macos")]
        return self.native.inspect(pid, birth, true);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (pid, birth);
            Err(io::ErrorKind::Unsupported.into())
        }
    }
}

#[cfg(target_os = "macos")]
impl NativeProcess {
    pub fn working_directory(&self) -> io::Result<PathBuf> {
        macos::working_directory(self)
    }
    pub fn matches_arguments(&self, shapes: &[&[Argument<'_>]]) -> io::Result<bool> {
        macos::matches_arguments(self, shapes)
    }
    pub fn runs_script(&self, expected: &std::path::Path) -> io::Result<bool> {
        macos::runs_script(self, expected)
    }
    pub fn lifetime(&self) -> io::Result<std::os::fd::OwnedFd> {
        macos::lifetime(self.pid, self.birth)
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
