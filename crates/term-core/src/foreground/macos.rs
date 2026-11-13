use super::{ForegroundAncestry, NativeProcess};
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    path::{Path, PathBuf},
    sync::Weak,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

// proc_bsdinfo and proc_uniqidentifierinfo are Darwin proc_pidinfo ABIs. Keep them here so
// private/versioned details cannot leak into the cross-platform policy layer.
const PROC_PIDTBSDINFO: i32 = 3;
const PROC_PIDUNIQIDENTIFIERINFO: i32 = 17;
const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct BsdInfo {
    flags: u32,
    status: u32,
    xstatus: u32,
    pid: u32,
    ppid: u32,
    uid: u32,
    gid: u32,
    ruid: u32,
    rgid: u32,
    svuid: u32,
    svgid: u32,
    rfu_1: u32,
    comm: [u8; 16],
    name: [u8; 32],
    nfiles: u32,
    pgid: u32,
    pjobc: u32,
    e_tdev: u32,
    e_tpgid: u32,
    nice: i32,
    start_sec: u64,
    start_usec: u64,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct UniqueInfo {
    uuid: [u8; 16],
    unique: u64,
    parent_unique: u64,
    pidversion: i32,
    parent_pidversion: i32,
    reserve2: u64,
    reserve3: u64,
}
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidinfo(
        pid: i32,
        flavor: i32,
        arg: u64,
        buffer: *mut std::ffi::c_void,
        size: i32,
    ) -> i32;
    fn proc_pidpath(pid: i32, buffer: *mut std::ffi::c_void, size: u32) -> i32;
    fn getsid(pid: i32) -> i32;
}
pub(super) struct Scope {
    master: OwnedFd,
    child: Stat,
    child_life: OwnedFd,
    stop: Weak<AtomicBool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stat {
    pid: u32,
    parent: u32,
    group: u32,
    session: u32,
    birth: u64,
    tty: u32,
    parent_birth: u64,
}
pub(super) fn scope(pty: &crate::Pty) -> io::Result<Scope> {
    let pid = pty
        .child
        .as_ref()
        .and_then(|c| c.process_id())
        .ok_or_else(denied)? as u32;
    let child = stat(pid)?;
    let child_life = lifetime(pid, child.birth)?;
    let raw = unsafe {
        libc::fcntl(
            pty.master.as_raw_fd().ok_or_else(denied)?,
            libc::F_DUPFD_CLOEXEC,
            0,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let master = unsafe { OwnedFd::from_raw_fd(raw) };
    Ok(Scope {
        master,
        child,
        child_life,
        stop: Arc::downgrade(&pty.stop),
    })
}
impl Scope {
    pub(super) fn is_open(&self) -> bool {
        self.stop
            .upgrade()
            .is_some_and(|s| !s.load(Ordering::Acquire))
            && alive(&self.child_life)
    }
    pub(super) fn inspect(
        &self,
        pid: u32,
        birth: u64,
        detached: bool,
    ) -> io::Result<ForegroundAncestry> {
        if !self.is_open() || stat(self.child.pid)? != self.child {
            return Err(denied());
        }
        let fg = foreground(self.master.as_raw_fd())?;
        let mut cur = stat(pid)?;
        if cur.birth != birth || (!detached && cur.group != fg) {
            return Err(denied());
        }
        let mut rows = Vec::new();
        let mut snapshots = Vec::new();
        for _ in 0..64 {
            if !detached && (cur.session != self.child.session || cur.tty != self.child.tty) {
                return Err(denied());
            }
            let path = executable(cur.pid)?;
            rows.push(NativeProcess {
                pid: cur.pid,
                birth: cur.birth,
                parent: cur.parent,
                group: cur.group,
                session: cur.session,
                executable: path,
                foreground: cur.group == fg
                    && cur.session == self.child.session
                    && cur.tty == self.child.tty,
            });
            snapshots.push(cur);
            if cur.pid == self.child.pid {
                if cur.birth != self.child.birth || foreground(self.master.as_raw_fd())? != fg {
                    return Err(denied());
                };
                for s in snapshots {
                    if stat(s.pid)? != s {
                        return Err(denied());
                    }
                }
                if !self.is_open() {
                    return Err(denied());
                };
                rows.reverse();
                return Ok(ForegroundAncestry {
                    foreground_group: fg,
                    processes: rows,
                });
            }
            if cur.parent == 0 || cur.parent == cur.pid {
                return Err(denied());
            }
            let parent = stat(cur.parent)?;
            if parent.birth != cur.parent_birth {
                return Err(denied());
            }
            cur = parent;
        }
        Err(denied())
    }
}
fn stat(pid: u32) -> io::Result<Stat> {
    let mut b = BsdInfo::default();
    let n = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDTBSDINFO,
            0,
            (&mut b as *mut BsdInfo).cast(),
            std::mem::size_of::<BsdInfo>() as i32,
        )
    };
    if n != std::mem::size_of::<BsdInfo>() as i32 || b.pid != pid || b.status == 5 {
        return Err(denied());
    };
    let mut u = UniqueInfo::default();
    let n = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDUNIQIDENTIFIERINFO,
            0,
            (&mut u as *mut UniqueInfo).cast(),
            std::mem::size_of::<UniqueInfo>() as i32,
        )
    };
    if n != std::mem::size_of::<UniqueInfo>() as i32 || u.unique == 0 || u.pidversion < 0 {
        return Err(denied());
    };
    let sid = unsafe { getsid(pid as i32) };
    if sid <= 0 {
        return Err(denied());
    };
    Ok(Stat {
        pid,
        parent: b.ppid,
        group: b.pgid,
        session: sid as u32,
        birth: u.unique,
        tty: b.e_tdev,
        parent_birth: u.parent_unique,
    })
}
fn executable(pid: u32) -> io::Result<PathBuf> {
    let mut b = [0u8; PROC_PIDPATHINFO_MAXSIZE];
    let n = unsafe { proc_pidpath(pid as i32, b.as_mut_ptr().cast(), b.len() as u32) };
    if n <= 0 || n as usize >= b.len() {
        return Err(denied());
    };
    let s = std::ffi::CStr::from_bytes_until_nul(&b[..n as usize]).map_err(|_| denied())?;
    let p = PathBuf::from(std::ffi::OsStr::from_bytes(s.to_bytes()));
    if !p.is_absolute() {
        return Err(denied());
    }
    Ok(p)
}
use std::os::unix::ffi::OsStrExt;
fn foreground(fd: i32) -> io::Result<u32> {
    let mut p = 0i32;
    const TIOCGPGRP: u64 = 0x40047477;
    if unsafe { libc::ioctl(fd, TIOCGPGRP, &mut p) } < 0 || p <= 0 {
        return Err(denied());
    }
    Ok(p as u32)
}
pub(super) fn lifetime(pid: u32, birth: u64) -> io::Result<OwnedFd> {
    if stat(pid)?.birth != birth {
        return Err(denied());
    };
    let fd = unsafe { libc::kqueue() };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    };
    let q = unsafe { OwnedFd::from_raw_fd(fd) };
    let ev = libc::kevent {
        ident: pid as _,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR,
        fflags: libc::NOTE_EXIT | libc::NOTE_EXEC,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    if unsafe { libc::kevent(fd, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) } < 0
        || stat(pid)?.birth != birth
    {
        return Err(denied());
    };
    Ok(q)
}
fn alive(fd: &OwnedFd) -> bool {
    let mut p = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    unsafe { libc::poll(&mut p, 1, 0) == 0 && p.revents == 0 }
}
pub(super) fn working_directory(p: &NativeProcess) -> io::Result<PathBuf> {
    let before = stat(p.pid)?;
    if before.birth != p.birth || executable(p.pid)? != p.executable {
        return Err(denied());
    };
    let mut info = VnodePathInfo::default();
    let n = unsafe {
        proc_pidinfo(
            p.pid as i32,
            9,
            0,
            (&mut info as *mut VnodePathInfo).cast(),
            std::mem::size_of::<VnodePathInfo>() as i32,
        )
    };
    if n != std::mem::size_of::<VnodePathInfo>() as i32 {
        return Err(denied());
    };
    let path = c_path(&info.cdir.vip_path)?;
    if stat(p.pid)? != before || executable(p.pid)? != p.executable {
        return Err(denied());
    };
    Ok(path)
}
#[repr(C)]
struct VnodeInfoPath {
    vip_vi: [u64; 19],
    vip_path: [i8; 1024],
}
#[repr(C)]
struct VnodePathInfo {
    cdir: VnodeInfoPath,
    rdir: VnodeInfoPath,
}
impl Default for VnodeInfoPath {
    fn default() -> Self {
        Self {
            vip_vi: [0; 19],
            vip_path: [0; 1024],
        }
    }
}
impl Default for VnodePathInfo {
    fn default() -> Self {
        Self {
            cdir: VnodeInfoPath::default(),
            rdir: VnodeInfoPath::default(),
        }
    }
}
fn c_path(bytes: &[i8]) -> io::Result<PathBuf> {
    let bytes: Vec<u8> = bytes.iter().map(|b| *b as u8).collect();
    let c = std::ffi::CStr::from_bytes_until_nul(&bytes).map_err(|_| denied())?;
    let p = PathBuf::from(std::ffi::OsStr::from_bytes(c.to_bytes()));
    if !p.is_absolute() {
        return Err(denied());
    };
    Ok(p)
}
#[path = "arguments.rs"]
mod arguments;
pub(super) fn matches_arguments(
    p: &NativeProcess,
    shapes: &[&[super::Argument<'_>]],
) -> io::Result<bool> {
    let before = stat(p.pid)?;
    if before.birth != p.birth {
        return Err(denied());
    };
    let data = process_arguments(p.pid)?;
    let ok = arguments::matches(&mut data.as_slice(), shapes)?;
    Ok(ok && stat(p.pid)? == before && executable(p.pid)? == p.executable)
}
fn process_arguments(pid: u32) -> io::Result<Vec<u8>> {
    let mut mib = [1i32, 49, pid as i32];
    let mut size = 0usize;
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } < 0
        || size == 0
        || size > 1_048_576
    {
        return Err(denied());
    };
    let mut data = vec![0u8; size];
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            data.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } < 0
    {
        return Err(denied());
    };
    data.truncate(size);
    if data.len() < 5 {
        return Err(denied());
    }
    let argc = i32::from_ne_bytes(data[..4].try_into().map_err(|_| denied())?);
    if argc <= 0 || argc > 4096 {
        return Err(denied());
    }
    let mut pos = 4usize;
    while pos < data.len() && data[pos] == 0 {
        pos += 1
    }
    while pos < data.len() && data[pos] != 0 {
        pos += 1
    }
    while pos < data.len() && data[pos] == 0 {
        pos += 1
    }
    let start = pos;
    for _ in 0..argc {
        let Some(relative) = data[pos..].iter().position(|b| *b == 0) else {
            return Err(denied());
        };
        pos += relative + 1;
    }
    if pos <= start {
        return Err(denied());
    }
    Ok(data[start..pos].to_vec())
}
pub(super) fn runs_script(p: &NativeProcess, expected: &Path) -> io::Result<bool> {
    let before = stat(p.pid)?;
    if before.birth != p.birth || executable(p.pid)? != p.executable {
        return Err(denied());
    };
    let args = process_arguments(p.pid)?;
    let mut slots = args.split(|b| *b == 0);
    let _argv0 = slots.next();
    let Some(script) = slots.next() else {
        return Ok(false);
    };
    if !script.starts_with(b"/") {
        return Ok(false);
    }
    let candidate = PathBuf::from(std::ffi::OsStr::from_bytes(script));
    let matches = std::fs::canonicalize(candidate).is_ok_and(|path| path == expected);
    Ok(matches && stat(p.pid)? == before && executable(p.pid)? == p.executable)
}
fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}
