//! Identity captured from the accepted socket, before any request bytes are interpreted.

use std::io;
#[cfg(target_os = "linux")]
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
#[cfg(target_os = "linux")]
use std::sync::Arc;

use interprocess::local_socket::Stream;

/// An unforgeable connected-process witness. Payload fields can never construct one.
#[derive(Debug, Clone)]
pub struct Peer {
    pid: u32,
    birth: u64,
    #[cfg(target_os = "linux")]
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
            false
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
            let _ = stream;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "verified session peers require Linux",
            ))
        }
    }
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
