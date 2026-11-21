//! Putting a pipe where the process's standard error is, and the original back.
//!
//! The pipe comes from `std::io::pipe`, whose two ends are not inherited by the programs the
//! window starts — git among them — so none of them can keep the pipe open past the capture.

use std::{fs::File, io::PipeReader};

/// The pipe standing in for standard error, and what to put back.
pub(super) struct Swapped(platform::State);

/// Makes a fresh pipe the process's standard error and hands back its read end.
pub(super) fn swap() -> std::io::Result<(PipeReader, Swapped)> {
    let (reader, writer) = std::io::pipe()?;
    let state = platform::swap(writer)?;
    Ok((reader, Swapped(state)))
}

impl Swapped {
    /// A handle on the original standard error, for writing to it directly; nothing when the
    /// process was born without one.
    pub(super) fn original(&self) -> Option<File> {
        platform::original(&self.0)
    }

    /// Puts the original back. The pipe loses its last writer, so its reader sees the end.
    pub(super) fn restore(self) {
        platform::restore(self.0);
    }
}

#[cfg(unix)]
mod platform {
    use super::File;
    use std::{
        io::PipeWriter,
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
    };

    const STDERR: libc::c_int = 2;

    /// A copy of what descriptor 2 was, or nothing when it was closed. Not inherited either.
    pub(super) struct State(Option<OwnedFd>);

    pub(super) fn swap(writer: PipeWriter) -> std::io::Result<State> {
        // SAFETY: `fcntl` only reads descriptor 2; a closed one returns -1, handled below.
        let copy = unsafe { libc::fcntl(STDERR, libc::F_DUPFD_CLOEXEC, 3) };
        // SAFETY: a non-negative result is a fresh descriptor nobody else owns.
        let saved = (copy >= 0).then(|| unsafe { OwnedFd::from_raw_fd(copy) });
        // SAFETY: both are open descriptors; `dup2` replaces 2 atomically and leaves it inheritable,
        // which is what standard error has to be for the programs that should share it.
        if unsafe { libc::dup2(writer.as_raw_fd(), STDERR) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(State(saved))
    }

    pub(super) fn original(state: &State) -> Option<File> {
        let copy = state.0.as_ref()?.try_clone().ok()?;
        Some(File::from(copy))
    }

    pub(super) fn restore(state: State) {
        match &state.0 {
            // SAFETY: both are open; this closes the pipe's writer at 2 in the same call.
            Some(saved) => unsafe { libc::dup2(saved.as_raw_fd(), STDERR) },
            // SAFETY: 2 holds the pipe's writer, which the capture put there.
            None => unsafe { libc::close(STDERR) },
        };
    }
}

#[cfg(windows)]
mod platform {
    use super::File;
    use std::{
        io::PipeWriter,
        os::windows::io::{AsRawHandle, BorrowedHandle, RawHandle},
    };
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::Console::{GetStdHandle, STD_ERROR_HANDLE, SetStdHandle},
    };

    /// What the process's standard error handle was — not owned: the process got it at birth —
    /// and the pipe's writer, kept open while it stands in for it.
    pub(super) struct State(RawHandle, PipeWriter);

    pub(super) fn swap(writer: PipeWriter) -> std::io::Result<State> {
        // SAFETY: reading the process's handle table entry has no preconditions.
        let original = unsafe { GetStdHandle(STD_ERROR_HANDLE) };
        // SAFETY: the writer stays open for as long as the state lives, and `restore` puts the
        // original back before it is closed.
        if unsafe { SetStdHandle(STD_ERROR_HANDLE, writer.as_raw_handle()) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(State(original, writer))
    }

    /// Nothing when the process was born without one: a windowed program started from the Start
    /// menu.
    pub(super) fn original(state: &State) -> Option<File> {
        if state.0.is_null() || state.0 == INVALID_HANDLE_VALUE {
            return None;
        }
        // SAFETY: the process's own standard error handle stays open for its whole life.
        let borrowed = unsafe { BorrowedHandle::borrow_raw(state.0) };
        borrowed.try_clone_to_owned().ok().map(File::from)
    }

    pub(super) fn restore(state: State) {
        // SAFETY: the original is what the process had; the writer is dropped after.
        unsafe { SetStdHandle(STD_ERROR_HANDLE, state.0) };
        drop(state.1);
    }
}
