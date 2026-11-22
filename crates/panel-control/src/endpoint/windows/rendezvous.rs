//! A fresh one-instance pipe, created only while its candidate process is already pinned.

use std::io;
use std::sync::Arc;
use std::time::Instant;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::IO::*;
use windows_sys::Win32::System::Pipes::*;
use windows_sys::Win32::System::Threading::*;

use super::{Handle, Security, Stream, event, new_pipe_with_instances, winerr};

pub(super) struct Rendezvous {
    pipe: Handle,
    stop: Arc<Handle>,
}

impl Rendezvous {
    pub(super) fn create(name: &[u16], stop: Arc<Handle>) -> io::Result<Self> {
        Ok(Self {
            pipe: new_pipe_with_instances(name, &Security::pipe()?, true, 1)?,
            stop,
        })
    }

    pub(super) fn accept(self, deadline: Instant) -> io::Result<Stream> {
        super::negotiation::remaining(deadline)?;
        if unsafe { WaitForSingleObject(self.stop.0, 0) } != WAIT_TIMEOUT {
            return Err(super::stopped());
        }
        let event = event()?;
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = event.0;
        // SAFETY: this stack frame retains the pipe, OVERLAPPED and event until completion drains.
        let connected = unsafe { ConnectNamedPipe(self.pipe.0, &mut overlapped) };
        if connected == 0 {
            let error = unsafe { GetLastError() };
            if error != ERROR_PIPE_CONNECTED {
                if error != ERROR_IO_PENDING {
                    return Err(io::Error::from_raw_os_error(error as i32));
                }
                self.wait_pending(&overlapped, deadline)?;
            }
        }
        // A simultaneous shutdown never promotes a new connection.
        if unsafe { WaitForSingleObject(self.stop.0, 0) } != WAIT_TIMEOUT {
            return Err(super::stopped());
        }
        super::negotiation::remaining(deadline)?;
        Ok(Stream::new(self.pipe, self.stop))
    }

    fn wait_pending(&self, overlapped: &OVERLAPPED, deadline: Instant) -> io::Result<()> {
        let handles = [self.stop.0, overlapped.hEvent];
        // Creation above checked the deadline, but it may expire before this pending wait.
        let millis = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min((u32::MAX - 1) as u128) as u32;
        // SAFETY: all handles and the pending operation are owned throughout this wait.
        let result = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, millis) };
        let failure = if result == WAIT_OBJECT_0 + 1 {
            None
        } else if result == WAIT_OBJECT_0 {
            Some(super::stopped())
        } else if result == WAIT_TIMEOUT {
            Some(io::Error::new(
                io::ErrorKind::TimedOut,
                "peer upgrade timed out",
            ))
        } else {
            Some(winerr())
        };
        let mut transferred = 0;
        if let Some(failure) = failure {
            // SAFETY: cancellation must finish before the OVERLAPPED/event/pipe are released.
            unsafe {
                CancelIoEx(self.pipe.0, overlapped);
                GetOverlappedResult(self.pipe.0, overlapped, &mut transferred, 1);
            }
            return Err(failure);
        }
        // SAFETY: the completion event was signalled; output remains writable.
        if unsafe { GetOverlappedResult(self.pipe.0, overlapped, &mut transferred, 0) } == 0 {
            return Err(winerr());
        }
        Ok(())
    }
}
