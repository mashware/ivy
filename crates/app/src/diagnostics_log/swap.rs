//! Putting a pipe where the process's standard error is, and the original back.
//!
//! The pipe's own two ends, and the copy of the original kept for putting it back, are not handed
//! to the programs the window starts. Standard error itself is: a program started with an inherited
//! standard error writes into the pipe and holds it open, which is why closing the capture waits
//! for the copying thread only up to a deadline.

use std::io::{PipeReader, Write};

/// The pipe standing in for standard error. Putting the original back is its `Drop`, so every
/// early return of the capture puts it back without a line written for it. `None` only inside
/// that `Drop`.
pub(super) struct Swapped(Option<platform::State>);

/// Makes a fresh pipe the process's standard error and hands back its read end.
pub(super) fn swap() -> std::io::Result<(PipeReader, Swapped)> {
    let (reader, writer) = std::io::pipe()?;
    let state = platform::swap(writer)?;
    Ok((reader, Swapped(Some(state))))
}

impl Swapped {
    /// Something that writes to the original standard error directly; nothing when Windows gave
    /// the process no handle, or the copy cannot be made.
    pub(super) fn original(&self) -> Option<Box<dyn Write + Send>> {
        platform::original(self.0.as_ref()?)
    }
}

impl Drop for Swapped {
    /// The pipe loses its writer here, so its reader sees the end.
    fn drop(&mut self) {
        if let Some(state) = self.0.take() {
            platform::restore(state);
        }
    }
}

#[cfg(unix)]
mod platform {
    use std::{
        fs::File,
        io::{PipeWriter, Write},
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
    };

    const STDERR: libc::c_int = 2;

    /// A copy of what descriptor 2 was, kept to put it back.
    pub(super) struct State(OwnedFd);

    pub(super) fn swap(writer: PipeWriter) -> std::io::Result<State> {
        // SAFETY: `fcntl` only reads descriptor 2. Rust's runtime opens it on `/dev/null` before
        // `main` when the process was started without it, so a failure here is a real one
        // (no descriptors left) and the capture does not go ahead.
        let copy = unsafe { libc::fcntl(STDERR, libc::F_DUPFD_CLOEXEC, 3) };
        if copy < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: a non-negative result is a fresh descriptor nobody else owns.
        let saved = unsafe { OwnedFd::from_raw_fd(copy) };
        duplicate_onto_stderr(writer.as_raw_fd())?;
        Ok(State(saved))
    }

    pub(super) fn original(state: &State) -> Option<Box<dyn Write + Send>> {
        let copy = state.0.try_clone().ok()?;
        Some(Box::new(File::from(copy)))
    }

    pub(super) fn restore(state: State) {
        // Nothing to do with a failure: standard error is the one place it could be said.
        let _ = duplicate_onto_stderr(state.0.as_raw_fd());
    }

    fn duplicate_onto_stderr(fd: libc::c_int) -> std::io::Result<()> {
        loop {
            // SAFETY: `fd` is open for as long as the caller holds it; `dup2` replaces 2 in one
            // step and leaves it inheritable, as standard error is.
            if unsafe { libc::dup2(fd, STDERR) } >= 0 {
                return Ok(());
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::{
        fs::File,
        io::{PipeWriter, Write},
        os::windows::io::{AsRawHandle, BorrowedHandle, RawHandle},
    };
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::Console::{GetConsoleMode, GetStdHandle, STD_ERROR_HANDLE, SetStdHandle},
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

    /// A console is written as UTF-16 text, as Rust's own standard error does: the same bytes
    /// written raw would be read in the console's code page and garble anything outside ASCII.
    pub(super) fn original(state: &State) -> Option<Box<dyn Write + Send>> {
        if state.0.is_null() || state.0 == INVALID_HANDLE_VALUE {
            return None;
        }
        // SAFETY: the process's own standard error handle stays open for its whole life.
        let borrowed = unsafe { BorrowedHandle::borrow_raw(state.0) };
        let handle = borrowed.try_clone_to_owned().ok()?;
        let mut mode = 0;
        // SAFETY: `handle` is open; the call only fills `mode`.
        if unsafe { GetConsoleMode(handle.as_raw_handle(), &mut mode) } != 0 {
            Some(Box::new(super::Console {
                handle,
                pending: Vec::new(),
            }))
        } else {
            Some(Box::new(File::from(handle)))
        }
    }

    pub(super) fn restore(state: State) {
        // SAFETY: the original is what the process had; the writer is dropped after.
        unsafe { SetStdHandle(STD_ERROR_HANDLE, state.0) };
        drop(state.1);
    }
}

/// A console, written as UTF-16 text.
#[cfg(windows)]
struct Console {
    handle: std::os::windows::io::OwnedHandle,
    /// The start of a character whose last bytes are still in the pipe.
    pending: Vec<u8>,
}

#[cfg(windows)]
impl Write for Console {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        let (text, used) = whole_characters(&self.pending);
        let text: Vec<u16> = text.encode_utf16().collect();
        self.pending.drain(..used);
        let mut sent = 0;
        while sent < text.len() {
            let mut written = 0;
            let rest = &text[sent..];
            // SAFETY: the buffer and its length come from the same slice, and `written` is
            // filled by the call.
            let ok = unsafe {
                windows_sys::Win32::System::Console::WriteConsoleW(
                    std::os::windows::io::AsRawHandle::as_raw_handle(&self.handle),
                    rest.as_ptr().cast(),
                    rest.len().min(u32::MAX as usize) as u32,
                    &mut written,
                    std::ptr::null(),
                )
            };
            if ok == 0 || written == 0 {
                return Err(std::io::Error::last_os_error());
            }
            sent += written as usize;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The text that can be written now from `pending`, and how many of its bytes that used. Bytes
/// that are not UTF-8 become U+FFFD where they stand; only a character still arriving — the start
/// of one at the very end — is left for the next write.
#[cfg(any(windows, test))]
fn whole_characters(pending: &[u8]) -> (String, usize) {
    let mut text = String::new();
    let mut used = 0;
    loop {
        match std::str::from_utf8(&pending[used..]) {
            Ok(rest) => {
                text.push_str(rest);
                return (text, pending.len());
            }
            Err(error) => {
                let valid = used + error.valid_up_to();
                text.push_str(std::str::from_utf8(&pending[used..valid]).unwrap_or_default());
                match error.error_len() {
                    None => return (text, valid),
                    Some(invalid) => {
                        text.push(char::REPLACEMENT_CHARACTER);
                        used = valid + invalid;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::whole_characters;

    #[test]
    fn a_character_split_between_two_writes_waits_for_its_last_bytes() {
        let euro = "€".as_bytes();
        assert_eq!(whole_characters(&[b'a', euro[0], euro[1]]), ("a".into(), 1));
        assert_eq!(whole_characters(euro), ("€".into(), 3));
    }

    #[test]
    fn a_bad_byte_before_a_split_character_spoils_only_itself() {
        let euro = "€".as_bytes();
        let pending = [0xFF, b'a', euro[0], euro[1]];
        assert_eq!(whole_characters(&pending), ("\u{FFFD}a".into(), 2));
    }
}
