//! Socket identity used only to label publications for diagnostics.
//! This witness is deliberately independent of session authorization.

use interprocess::local_socket::Stream;
use std::io;

#[cfg(unix)]
use std::os::fd::{AsFd, AsRawFd};

/// Kernel supplied process identity captured from an accepted local socket.
#[derive(Debug, Clone)]
pub struct DiagnosticPeer {
    pid: u32,
    #[cfg(target_os = "linux")]
    pidfd: Option<std::sync::Arc<std::os::fd::OwnedFd>>,
}

impl DiagnosticPeer {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// A momentary kernel liveness check. On Linux it uses the exact peer pidfd when available.
    pub fn is_alive(&self) -> bool {
        #[cfg(target_os = "linux")]
        if let Some(pidfd) = &self.pidfd {
            let mut p = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: p and the owned descriptor remain valid for this nonblocking poll.
            return unsafe { libc::poll(&mut p, 1, 0) == 0 && p.revents == 0 };
        }
        #[cfg(unix)]
        {
            // SAFETY: kill(pid, 0) performs no memory access and sends no signal.
            unsafe {
                libc::kill(self.pid as libc::pid_t, 0) == 0
                    || io::Error::last_os_error().kind() == io::ErrorKind::PermissionDenied
            }
        }
        #[cfg(not(unix))]
        {
            false
        }
    }

    pub(crate) fn capture(stream: &Stream) -> io::Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let Stream::UdSocket(stream) = stream;
            let fd = stream.as_fd().as_raw_fd();
            let credentials: libc::ucred = option(fd, libc::SOL_SOCKET, libc::SO_PEERCRED)?;
            if credentials.pid <= 0 {
                return Err(io::Error::other("invalid diagnostic peer pid"));
            }
            let descriptor: libc::c_int = option(fd, libc::SOL_SOCKET, libc::SO_PEERPIDFD)
                .ok()
                .unwrap_or(-1);
            let pidfd = if descriptor >= 0 {
                // SAFETY: successful SO_PEERPIDFD returns a fresh owned descriptor.
                Some(std::sync::Arc::new(unsafe {
                    std::os::fd::OwnedFd::from_raw_fd(descriptor)
                }))
            } else {
                None
            };
            let peer = Self {
                pid: credentials.pid as u32,
                pidfd,
            };
            if peer.is_alive() {
                Ok(peer)
            } else {
                Err(io::Error::other("diagnostic peer is not alive"))
            }
        }
        #[cfg(target_os = "macos")]
        {
            let Stream::UdSocket(stream) = stream;
            let pid: libc::pid_t =
                option(stream.as_fd().as_raw_fd(), 0, 2 /* LOCAL_PEERPID */)?;
            if pid <= 0 {
                return Err(io::Error::other("invalid diagnostic peer pid"));
            }
            let peer = Self { pid: pid as u32 };
            if peer.is_alive() {
                Ok(peer)
            } else {
                Err(io::Error::other("diagnostic peer is not alive"))
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = stream;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "diagnostic peers unavailable",
            ))
        }
    }
}

#[cfg(target_os = "linux")]
use std::os::fd::FromRawFd;

#[cfg(unix)]
fn option<T: Copy>(fd: libc::c_int, level: libc::c_int, name: libc::c_int) -> io::Result<T> {
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut length = std::mem::size_of::<T>() as libc::socklen_t;
    // SAFETY: getsockopt initializes value on success, with length checked below.
    if unsafe { libc::getsockopt(fd, level, name, value.as_mut_ptr().cast(), &mut length) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if length as usize != std::mem::size_of::<T>() {
        return Err(io::Error::other("unexpected peer credential size"));
    }
    // SAFETY: successful getsockopt initialized the exact-sized value.
    Ok(unsafe { value.assume_init() })
}
