//! Identity captured from the accepted socket, before any request bytes are interpreted.

use std::io;
#[cfg(target_os = "linux")]
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use std::{
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd},
    sync::Arc,
};

use interprocess::local_socket::Stream;

/// An unforgeable connected-process witness. Payload fields can never construct one.
#[derive(Debug, Clone)]
pub struct Peer {
    pid: u32,
    birth: u64,
    #[cfg(target_os = "linux")]
    lifetime: Arc<OwnedFd>,
    #[cfg(target_os = "macos")]
    lifetime: Arc<OwnedFd>,
}

impl Peer {
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn birth(&self) -> u64 {
        self.birth
    }

    /// The exact kernel pidfd acquired with SO_PEERPIDFD, rather than reopened from a numeric PID.
    #[cfg(target_os = "linux")]
    pub fn lifetime(&self) -> Arc<OwnedFd> {
        Arc::clone(&self.lifetime)
    }

    pub fn is_alive(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            let mut descriptor = libc::pollfd {
                fd: self.lifetime.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: the owned descriptor and writable one-element array live through this call.
            let ready = unsafe { libc::poll(&mut descriptor, 1, 0) };
            ready == 0 && descriptor.revents == 0
        }
        #[cfg(not(target_os = "linux"))]
        {
            #[cfg(target_os = "macos")]
            {
                let mut p = libc::pollfd {
                    fd: self.lifetime.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                unsafe { libc::poll(&mut p, 1, 0) == 0 && p.revents == 0 }
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        }
    }

    pub(crate) fn capture(stream: &Stream) -> io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let Stream::UdSocket(stream) = stream;
            let socket = stream.as_fd().as_raw_fd();
            let credentials: libc::ucred = socket_option(socket, libc::SO_PEERCRED)?;
            if credentials.pid <= 0 {
                return Err(io::Error::other("invalid socket peer identity"));
            }
            let descriptor: libc::c_int = socket_option(socket, libc::SO_PEERPIDFD)?;
            if descriptor < 0 {
                return Err(io::Error::other("missing socket peer lifetime"));
            }
            // SAFETY: SO_PEERPIDFD returns a newly owned close-on-exec descriptor on success.
            let lifetime = Arc::new(unsafe { OwnedFd::from_raw_fd(descriptor) });
            let mut peer = Self {
                pid: credentials.pid as u32,
                birth: 0,
                lifetime,
            };
            if !peer.is_alive() {
                return Err(io::Error::other("socket peer has already exited"));
            }
            peer.birth = birth_of(peer.pid)?;
            // If the peer exited while /proc was read, its numeric PID cannot identify a successor.
            if !peer.is_alive() {
                return Err(io::Error::other("socket peer exited during verification"));
            }
            Ok(peer)
        }
        #[cfg(not(target_os = "linux"))]
        {
            #[cfg(target_os = "macos")]
            {
                return capture_macos(stream);
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = stream;
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "verified session peers require Linux",
                ))
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
struct AuditToken {
    val: [u32; 8],
}
#[cfg(target_os = "macos")]
fn capture_macos(stream: &Stream) -> io::Result<Peer> {
    let Stream::UdSocket(stream) = stream;
    let fd = stream.as_fd().as_raw_fd();
    let token: AuditToken = socket_option_macos(fd, 6 /* LOCAL_PEERTOKEN */)?;
    let pid = token.val[5];
    let version = token.val[7];
    if pid == 0 || version == 0 {
        return Err(io::Error::other("invalid local peer token"));
    }
    let birth = unique_identity(pid, version)?;
    let lifetime = Arc::new(macos_lifetime(pid, version, birth)?);
    let peer = Peer {
        pid,
        birth,
        lifetime,
    };
    if !peer.is_alive() || unique_identity(pid, version)? != birth || !peer.is_alive() {
        return Err(io::Error::other(
            "peer identity changed during verification",
        ));
    }
    Ok(peer)
}
#[cfg(target_os = "macos")]
fn socket_option_macos<T: Copy>(fd: libc::c_int, option: libc::c_int) -> io::Result<T> {
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut len = std::mem::size_of::<T>() as libc::socklen_t;
    if unsafe { libc::getsockopt(fd, 0, option, value.as_mut_ptr().cast(), &mut len) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if len as usize != std::mem::size_of::<T>() {
        return Err(io::Error::other("invalid local peer token size"));
    }
    Ok(unsafe { value.assume_init() })
}
#[cfg(target_os = "macos")]
#[repr(C)]
struct UniqueIdentity {
    uuid: [u8; 16],
    unique: u64,
    parent_unique: u64,
    pidversion: i32,
    parent_pidversion: i32,
    reserve2: u64,
    reserve3: u64,
}
#[cfg(target_os = "macos")]
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buf: *mut libc::c_void, size: i32) -> i32;
    fn kqueue() -> i32;
    fn kevent(
        kq: i32,
        changelist: *const libc::kevent,
        nchanges: i32,
        eventlist: *mut libc::kevent,
        nevents: i32,
        timeout: *const libc::timespec,
    ) -> i32;
}
#[cfg(target_os = "macos")]
fn unique_identity(pid: u32, version: u32) -> io::Result<u64> {
    let mut bsd = [0u8; 136];
    let n = unsafe { proc_pidinfo(pid as i32, 3, 0, bsd.as_mut_ptr().cast(), bsd.len() as i32) };
    if n != bsd.len() as i32 {
        return Err(io::Error::other("peer PID identity unavailable"));
    }
    let queried_pid = unsafe { std::ptr::read_unaligned(bsd.as_ptr().cast::<u32>().add(3)) };
    if queried_pid != pid {
        return Err(io::Error::other(
            "queried PID does not match local peer token",
        ));
    }
    let mut info = std::mem::MaybeUninit::<UniqueIdentity>::uninit();
    let size = std::mem::size_of::<UniqueIdentity>() as i32;
    let n = unsafe { proc_pidinfo(pid as i32, 17, 0, info.as_mut_ptr().cast(), size) };
    if n != size {
        return Err(io::Error::other("process unique identity unavailable"));
    };
    let i = unsafe { info.assume_init() };
    if i.pidversion < 0 || i.pidversion as u32 != version || i.unique == 0 {
        return Err(io::Error::other(
            "local peer token does not match process identity",
        ));
    };
    Ok(i.unique)
}
#[cfg(target_os = "macos")]
fn macos_lifetime(pid: u32, version: u32, birth: u64) -> io::Result<OwnedFd> {
    let fd = unsafe { kqueue() };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    };
    let q = unsafe { OwnedFd::from_raw_fd(fd) };
    let ev = libc::kevent {
        ident: pid as _,
        filter: -5,
        flags: 1 | 4,
        fflags: 0x80000000 | 0x20000000,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    if unsafe { kevent(fd, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) } < 0
        || unique_identity(pid, version)? != birth
    {
        return Err(io::Error::other("peer exited during lifetime registration"));
    };
    Ok(q)
}

#[cfg(target_os = "linux")]
fn socket_option<T: Copy>(socket: libc::c_int, option: libc::c_int) -> io::Result<T> {
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut length = std::mem::size_of::<T>() as libc::socklen_t;
    // SAFETY: the uninitialized output has precisely the capacity supplied to getsockopt.
    let result = unsafe {
        libc::getsockopt(
            socket,
            libc::SOL_SOCKET,
            option,
            value.as_mut_ptr().cast(),
            &mut length,
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    if length as usize != std::mem::size_of::<T>() {
        return Err(io::Error::other("invalid socket credential size"));
    }
    // SAFETY: getsockopt succeeded and confirmed that it initialized exactly T's byte width.
    Ok(unsafe { value.assume_init() })
}

#[cfg(target_os = "linux")]
fn birth_of(pid: u32) -> io::Result<u64> {
    use std::io::Read;
    let file = std::fs::File::open(format!("/proc/{pid}/stat"))?;
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(io::Error::other("process identity goes over the ceiling"));
    }
    let text =
        std::str::from_utf8(&bytes).map_err(|_| io::Error::other("invalid process identity"))?;
    text.rsplit_once(')')
        .and_then(|(_, fields)| fields.split_whitespace().nth(19))
        .and_then(|birth| birth.parse().ok())
        .ok_or_else(|| io::Error::other("missing process birth identity"))
}
