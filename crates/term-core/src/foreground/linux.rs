use super::{Ancestry, ForegroundAncestry, NativeProcess};
use std::io::{self, BufRead, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::sync::Weak;
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "arguments.rs"]
mod arguments;

pub(super) struct Scope {
    master: OwnedFd,
    child: Stat,
    child_lifetime: OwnedFd,
    stop: Weak<AtomicBool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Stat {
    pub pid: u32,
    pub parent: u32,
    pub group: u32,
    pub session: u32,
    pub birth: u64,
    tty: i64,
}

pub(super) fn scope(pty: &crate::Pty) -> io::Result<Scope> {
    let pid = pty
        .child
        .as_ref()
        .and_then(|child| child.process_id())
        .ok_or_else(denied)?;
    let child = stat(pid)?;
    let child_lifetime = lifetime(pid, child.birth)?;
    let fd = pty.master.as_raw_fd().ok_or_else(denied)?;
    Ok(Scope {
        master: crate::nonblocking::duplicate(fd)?,
        child,
        child_lifetime,
        stop: std::sync::Arc::downgrade(&pty.stop),
    })
}

impl Scope {
    pub(super) fn classify_ancestry(&self, pid: u32) -> Ancestry {
        self.diagnostic_ancestry(pid).unwrap_or(Ancestry::Unknown)
    }

    fn diagnostic_ancestry(&self, pid: u32) -> io::Result<Ancestry> {
        if pid == 0 || !self.is_open() {
            return Err(denied());
        }
        let peer = stat(pid)?;
        let peer_lifetime = lifetime(pid, peer.birth)?;
        if !self.is_open() || stat(self.child.pid)? != self.child {
            return Err(denied());
        }
        let verdict = super::classify_snapshots(
            pid,
            (self.child.pid, self.child.parent, self.child.birth),
            |process| stat(process).map(|s| (s.pid, s.parent, s.birth)),
        )?;
        if !self.is_open()
            || stat(self.child.pid)? != self.child
            || stat(pid)? != peer
            || !alive(&peer_lifetime)
        {
            return Err(denied());
        }
        Ok(verdict)
    }

    pub(super) fn is_open(&self) -> bool {
        self.stop
            .upgrade()
            .is_some_and(|stop| !stop.load(Ordering::Acquire))
            && alive(&self.child_lifetime)
    }

    pub(super) fn inspect(&self, pid: u32, birth: u64) -> io::Result<ForegroundAncestry> {
        self.ancestry(pid, birth, false)
    }

    pub(super) fn inspect_descendant(
        &self,
        pid: u32,
        birth: u64,
    ) -> io::Result<ForegroundAncestry> {
        self.ancestry(pid, birth, true)
    }

    fn ancestry(
        &self,
        pid: u32,
        birth: u64,
        detached_helper: bool,
    ) -> io::Result<ForegroundAncestry> {
        if !self.is_open() || stat(self.child.pid)?.birth != self.child.birth {
            return Err(denied());
        }
        let foreground = self.group()?;
        let mut current = stat(pid)?;
        if current.birth != birth || !detached_helper && current.group != foreground {
            return Err(denied());
        }
        let mut processes = Vec::new();
        let mut observed = Vec::new();
        // A hostile or unexpectedly deep topology must not monopolize the single verifier.
        for _ in 0..64 {
            if !detached_helper
                && (current.session != self.child.session || current.tty != self.child.tty)
            {
                return Err(denied());
            }
            let executable = std::fs::read_link(format!("/proc/{}/exe", current.pid))?;
            if stat(current.pid)? != current {
                return Err(denied());
            }
            processes.push(NativeProcess {
                pid: current.pid,
                birth: current.birth,
                parent: current.parent,
                group: current.group,
                session: current.session,
                executable,
                foreground: current.group == foreground
                    && current.session == self.child.session
                    && current.tty == self.child.tty,
            });
            observed.push(current);
            if current.pid == self.child.pid {
                if current.birth != self.child.birth
                    || self.group()? != foreground
                    || !self.is_open()
                {
                    return Err(denied());
                }
                for process in &observed {
                    if stat(process.pid)? != *process {
                        return Err(denied());
                    }
                }
                if self.group()? != foreground || !self.is_open() {
                    return Err(denied());
                }
                processes.reverse();
                return Ok(ForegroundAncestry {
                    foreground_group: foreground,
                    processes,
                });
            }
            if current.parent == 0 || current.parent == current.pid {
                return Err(denied());
            }
            current = stat(current.parent)?;
        }
        Err(denied())
    }

    fn group(&self) -> io::Result<u32> {
        // SAFETY: the owned descriptor is a live duplicate of this console's master.
        u32::try_from(unsafe { libc::tcgetpgrp(self.master.as_raw_fd()) })
            .ok()
            .filter(|pid| *pid > 0)
            .ok_or_else(denied)
    }
}

pub(super) fn stat(pid: u32) -> io::Result<Stat> {
    let mut text = String::new();
    std::fs::File::open(format!("/proc/{pid}/stat"))?
        .take(8193)
        .read_to_string(&mut text)?;
    if text.len() > 8192 {
        return Err(denied());
    }
    let (_, fields) = text.rsplit_once(") ").ok_or_else(denied)?;
    let fields: Vec<_> = fields.split_ascii_whitespace().collect();
    if fields.len() < 20 || matches!(fields[0], "Z" | "X" | "x") {
        return Err(denied());
    }
    let number = |index: usize| fields[index].parse::<u64>().map_err(|_| denied());
    Ok(Stat {
        pid,
        parent: u32::try_from(number(1)?).map_err(|_| denied())?,
        group: u32::try_from(number(2)?).map_err(|_| denied())?,
        session: u32::try_from(number(3)?).map_err(|_| denied())?,
        tty: fields[4].parse().map_err(|_| denied())?,
        birth: number(19)?,
    })
}

pub(super) fn lifetime(pid: u32, birth: u64) -> io::Result<OwnedFd> {
    if stat(pid)?.birth != birth {
        return Err(denied());
    }
    // SAFETY: pidfd_open takes integer arguments and returns a newly owned CLOEXEC descriptor.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: success above transfers this unique descriptor to the owner.
    let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    if stat(pid)?.birth != birth || !alive(&fd) {
        return Err(denied());
    }
    Ok(fd)
}

fn alive(fd: &OwnedFd) -> bool {
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll receives one initialized descriptor and never waits.
    unsafe { libc::poll(&mut poll, 1, 0) == 0 }
}

pub(super) fn working_directory(process: &NativeProcess) -> io::Result<std::path::PathBuf> {
    let before = stat(process.pid)?;
    let executable = format!("/proc/{}/exe", process.pid);
    if before.birth != process.birth || std::fs::read_link(&executable)? != process.executable {
        return Err(denied());
    }
    let lifetime = lifetime(process.pid, process.birth)?;
    let directory = std::fs::read_link(format!("/proc/{}/cwd", process.pid))?;
    // The kernel appends this suffix when the directory no longer has a usable pathname.
    if !directory.is_absolute()
        || directory
            .as_os_str()
            .as_encoded_bytes()
            .ends_with(b" (deleted)")
        || stat(process.pid)? != before
        || std::fs::read_link(&executable)? != process.executable
        || !alive(&lifetime)
    {
        return Err(denied());
    }
    Ok(directory)
}

pub(super) fn runs_script(process: &NativeProcess, expected: &Path) -> io::Result<bool> {
    use std::os::unix::ffi::OsStringExt;
    if stat(process.pid)?.birth != process.birth {
        return Err(denied());
    }
    let file = std::fs::File::open(format!("/proc/{}/cmdline", process.pid))?;
    let mut reader = io::BufReader::with_capacity(1, file.take(8194));
    let mut slot = Vec::new();
    for _ in 0..2 {
        slot.clear();
        reader.read_until(0, &mut slot)?;
        if slot.len() > 4097 || slot.last() != Some(&0) {
            return Ok(false);
        }
    }
    slot.pop();
    let candidate = std::path::PathBuf::from(std::ffi::OsString::from_vec(slot));
    // Relative script paths require the process cwd, which can change while its script stays live.
    if !candidate.is_absolute() {
        return Ok(false);
    }
    let matches = std::fs::canonicalize(candidate).is_ok_and(|path| path == expected);
    Ok(matches && stat(process.pid)?.birth == process.birth)
}

pub(super) fn matches_arguments(
    process: &NativeProcess,
    shapes: &[&[super::Argument<'_>]],
) -> io::Result<bool> {
    if stat(process.pid)?.birth != process.birth {
        return Err(denied());
    }
    let mut file = std::fs::File::open(format!("/proc/{}/cmdline", process.pid))?;
    let matches = arguments::matches(&mut file, shapes)?;
    Ok(matches && stat(process.pid)?.birth == process.birth)
}

fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}
