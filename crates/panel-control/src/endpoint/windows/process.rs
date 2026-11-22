//! A pinned process candidate becomes a peer only after a fresh pipe proves its connector.

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::Arc;

use windows_sys::Win32::Foundation::{FILETIME, WAIT_TIMEOUT};
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    WaitForSingleObject,
};

use super::Stream;

/// Process object retained continuously before its verified connection is created.
#[derive(Debug)]
pub(crate) struct WindowsPeer {
    pid: u32,
    birth: u64,
    lifetime: OwnedHandle,
}

impl WindowsPeer {
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn birth(&self) -> u64 {
        self.birth
    }

    pub(crate) fn is_alive(&self) -> bool {
        // SAFETY: the owned process handle remains live for this nonblocking wait.
        unsafe { WaitForSingleObject(self.lifetime.as_raw_handle(), 0) == WAIT_TIMEOUT }
    }

    /// This does NOT authenticate the bootstrap connector: its PID may already be recycled.
    pub(super) fn candidate(stream: &Stream) -> io::Result<Arc<Self>> {
        let pid = connector_pid(stream)?;
        // SAFETY: OpenProcess returns a new, non-inheritable process handle on success.
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: ownership of this successful OpenProcess handle is transferred exactly once.
        let lifetime = unsafe { OwnedHandle::from_raw_handle(handle) };
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        // SAFETY: all output pointers are writable and the handle owns a process object.
        if unsafe {
            GetProcessTimes(
                lifetime.as_raw_handle(),
                &mut creation,
                &mut exit,
                &mut kernel,
                &mut user,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let peer = Arc::new(Self {
            pid,
            birth: ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64,
            lifetime,
        });
        if peer.birth == 0 || !peer.is_alive() {
            return Err(io::Error::other("control peer candidate is not alive"));
        }
        Ok(peer)
    }

    /// Call only for a pipe instance CREATED AFTER this candidate handle was acquired.
    pub(super) fn verify_fresh_connector(&self, stream: &Stream) -> io::Result<()> {
        if connector_pid(stream)? != self.pid || !self.is_alive() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "fresh control connector does not match the pinned process",
            ));
        }
        Ok(())
    }
}

fn connector_pid(stream: &Stream) -> io::Result<u32> {
    let mut pid = 0;
    // SAFETY: stream owns the accepted server pipe handle and pid is writable.
    if unsafe { GetNamedPipeClientProcessId(stream.raw_handle(), &mut pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if pid == 0 {
        return Err(io::Error::other("control connector PID is missing"));
    }
    Ok(pid)
}
