//! Windows: who closes a console's pseudoconsole, and when (**D111**).
//!
//! **Closing the pseudoconsole is dropping the master, and nothing else closes it.** `portable-pty`
//! keeps the pseudoconsole in a value shared by the master and the slave and calls
//! `ClosePseudoConsole` when the last of them goes; the slave is dropped at spawn, and the output
//! pipe's write end was handed to the pseudoconsole's host and never kept by this process. So the
//! reader sees end of file only after the master is dropped — while a `Pty` holds it, a program
//! that exits leaves a console that never ends.
//!
//! **`ClosePseudoConsole` may wait until its last frame is read.** Before Windows 11 24H2 the call
//! is synchronous and Microsoft documents that an undrained output pipe can keep it waiting
//! indefinitely; the reader therefore drains to end of file on Windows (see `feed_and_record`), and
//! no close happens on the interface pass.

use std::io;
use std::os::windows::io::{AsRawHandle, BorrowedHandle, OwnedHandle, RawHandle};
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::MasterPty;

/// Where a console's master lives on Windows: whoever takes it out and drops it closes the
/// pseudoconsole. Shared by the `Pty` (to resize, and to close on drop) and the exit watcher.
pub(crate) type Slot = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;

const INFINITE: u32 = 0xFFFF_FFFF;
const WAIT_OBJECT_0: u32 = 0;

// Two functions of kernel32, declared here rather than pulled in through a bindings crate: they
// are the only ones this module needs, and their signatures have not changed since Windows XP.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn WaitForSingleObject(handle: RawHandle, milliseconds: u32) -> u32;
    fn TerminateProcess(process: RawHandle, exit_code: u32) -> i32;
}

/// Takes the master out of the slot and drops it **after the lock is released**, which is the
/// close. Answers whether this call was the one that closed it.
///
/// The lock is not held across the drop because the drop is `ClosePseudoConsole`, which can wait
/// for the host to finish, and `Pty::resize` takes the same lock on the interface thread.
pub(crate) fn close(slot: &Slot) -> bool {
    let master = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    let closed = master.is_some();
    drop(master);
    closed
}

/// Closes the pseudoconsole on a thread of its own, so the interface pass never waits for it.
///
/// If the thread cannot be created the close happens here: the reader drains to end of file on
/// Windows, so the call ends once the host has written its last frame, and leaving the
/// pseudoconsole open would leave the reader and the writer waiting for the life of the process.
pub(crate) fn close_elsewhere(slot: &Slot) {
    let master = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    let Some(master) = master else { return };
    // On failure the closure, and the master inside it, are dropped by `spawn` before it returns:
    // that is the close happening on this thread, as the paragraph above accepts.
    let _ = thread::Builder::new()
        .name("conpty-close".to_owned())
        .stack_size(64 * 1024)
        .spawn(move || drop(master));
}

/// Watches the console's program and closes the pseudoconsole when it ends, so the reader gets its
/// end of file and the console reports an ordinary ending.
///
/// **Its wait has one way out, the program ending**, and every close provides it: `Drop for Pty`
/// terminates the program, and closing the pseudoconsole ends what is still attached to it. A
/// program that survives both keeps this thread for as long as it lives — the trade D56 already
/// makes for a program that cannot be ended.
///
/// Not starting the watcher is not an error of the console: it is said once on standard error and
/// the console works as it did before this existed, without noticing its program's exit.
pub(crate) fn watch_exit(process: RawHandle, slot: Slot) {
    use std::io::Write;

    // SAFETY: the handle belongs to the child the caller just spawned and stays open for the length
    // of this call; it is duplicated here, so the watcher owns a handle of its own.
    let owned = match unsafe { BorrowedHandle::borrow_raw(process) }.try_clone_to_owned() {
        Ok(owned) => owned,
        Err(failure) => {
            let _ = writeln!(
                io::stderr().lock(),
                "a console will not notice its program ending: {failure}"
            );
            return;
        }
    };
    let watched = thread::Builder::new()
        .name("conpty-exit".to_owned())
        .stack_size(64 * 1024)
        .spawn(move || {
            if wait(&owned, INFINITE) {
                close(&slot);
            }
        });
    if let Err(failure) = watched {
        let _ = writeln!(
            io::stderr().lock(),
            "a console will not notice its program ending: {failure}"
        );
    }
}

/// Whether the process ended within `milliseconds`. A failed wait answers `false`: it can only fail
/// on a handle that is not a process, and closing a console whose program may be running is the one
/// thing an answer here must never cause.
fn wait(process: &OwnedHandle, milliseconds: u32) -> bool {
    // SAFETY: `process` is an open handle owned by the caller for the length of the call.
    unsafe { WaitForSingleObject(process.as_raw_handle(), milliseconds) == WAIT_OBJECT_0 }
}

/// Ends the program, and says so when Windows refuses.
///
/// **`portable-pty`'s own `kill` cannot be used for this**: it reads `TerminateProcess`'s result the
/// wrong way round and then discards it, so a refusal and a success look the same.
///
/// **Whether it already ended is asked of the handle, not of its exit code**: a program that exits
/// with 259 reports the same code Windows uses for «still running».
pub(crate) fn terminate(process: RawHandle) -> io::Result<()> {
    // SAFETY: the handle belongs to the console's child, which the caller owns for the whole call.
    let process = unsafe { BorrowedHandle::borrow_raw(process) };
    let ended = || unsafe { WaitForSingleObject(process.as_raw_handle(), 0) == WAIT_OBJECT_0 };
    if ended() {
        return Ok(());
    }
    // SAFETY: as above; `TerminateProcess` touches no memory of this process.
    if unsafe { TerminateProcess(process.as_raw_handle(), 1) } != 0 {
        return Ok(());
    }
    let refused = io::Error::last_os_error();
    // A program that ended between the look and the call answers «access denied»: that is not a
    // refusal, and the line written for one would be false.
    if ended() { Ok(()) } else { Err(refused) }
}

/// Run only on Windows, by the prerelease workflow's Windows job (D108): the windows_exit prefix
/// is what that job filters and names.
#[cfg(test)]
mod tests {
    use crate::{Ending, Pty, READER_GIVE_UP, Size};
    use std::sync::mpsc::Receiver;
    use std::time::{Duration, Instant};

    /// Generous on purpose: a cold PowerShell on a CI runner takes seconds to start.
    const DEADLINE: Duration = Duration::from_secs(30);

    /// By absolute path, never a bare name: the launcher searches the current directory for one.
    fn powershell() -> String {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
        format!(r"{root}\System32\WindowsPowerShell\v1.0\powershell.exe")
    }

    fn spawn(command: &str) -> (Pty, Receiver<Vec<u8>>) {
        Pty::spawn(
            &powershell(),
            &[
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                command,
            ],
            Size::default(),
        )
        .expect("PowerShell starts in a console")
    }

    /// Waits for the program's own result, never for bytes: ConPTY writes before the program does,
    /// so the marker is built at run time and its echo cannot satisfy this.
    fn wait_for_marker(output: &Receiver<Vec<u8>>) {
        let deadline = Instant::now() + DEADLINE;
        let mut seen = Vec::new();
        while !String::from_utf8_lossy(&seen).contains("IVY_OK") {
            let left = deadline.saturating_duration_since(Instant::now());
            match output.recv_timeout(left) {
                Ok(bytes) => seen.extend_from_slice(&bytes),
                Err(_) => panic!(
                    "the program never wrote IVY_OK: {:?}",
                    String::from_utf8_lossy(&seen)
                ),
            }
        }
    }

    /// **The defect itself**: a program that ends on its own ends its console, with nobody closing
    /// it. Before the exit watcher this waited for ever, because the pseudoconsole stayed open.
    #[test]
    fn windows_exit_ordinary_end_is_reported() {
        let (pty, output) = spawn("Write-Output ('IVY' + '_OK')");
        wait_for_marker(&output);
        let deadline = Instant::now() + DEADLINE;
        while pty.ending().is_none() && Instant::now() < deadline {
            // Drained so the reader is never parked on a full channel; bounded by the deadline.
            let _ = output.recv_timeout(Duration::from_millis(10));
        }
        assert_eq!(
            pty.ending(),
            Some(Ending::EndedWell),
            "the program ended and its console never did"
        );
    }

    /// **Closing a live console does not abandon its reader.** The close used to drop the master
    /// last, after waiting the whole reader deadline for an end of file only that drop could give.
    /// The cost is the observable: an abandoned reader costs at least the deadline.
    #[test]
    fn windows_exit_close_does_not_abandon_the_reader() {
        let (pty, output) = spawn("Write-Output ('IVY' + '_OK'); Start-Sleep 30");
        wait_for_marker(&output);
        let started = Instant::now();
        drop(pty);
        let took = started.elapsed();
        assert!(
            took < READER_GIVE_UP,
            "closing took {took:?}: the reader was abandoned at the deadline instead of ending"
        );
    }

    /// **A writer parked on a program that reads nothing ends once its console is closed.** The
    /// premise is asserted first: if the host swallowed the whole write, the writer was never parked
    /// and the close would prove nothing.
    #[test]
    fn windows_exit_close_ends_a_parked_writer() {
        let (mut pty, output) = spawn("Write-Output ('IVY' + '_OK'); Start-Sleep 30");
        wait_for_marker(&output);
        pty.write(&vec![b'x'; 8 * 1024 * 1024])
            .expect("an empty queue takes a write whole");
        std::thread::sleep(Duration::from_secs(2));
        assert!(
            pty.backlog().is_some_and(|backlog| backlog.bytes > 0),
            "premise: the writer is parked on a program that reads nothing"
        );
        let writer = pty.writer.take().expect("the writer thread is there");
        drop(pty);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !writer.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            writer.is_finished(),
            "the writer is still parked after its console was closed"
        );
    }

    /// **An exit code of 259 is not «still running».** Windows uses that number for a live process,
    /// so a program that exits with it must be told apart by its handle — by the watcher, which
    /// would otherwise never end the console, and by closing, which would otherwise call it refused.
    #[test]
    fn windows_exit_an_exit_code_of_259_is_an_ended_program() {
        let (pty, output) = spawn("Write-Output ('IVY' + '_OK'); exit 259");
        wait_for_marker(&output);
        let deadline = Instant::now() + DEADLINE;
        while pty.ending().is_none() && Instant::now() < deadline {
            let _ = output.recv_timeout(Duration::from_millis(10));
        }
        assert_eq!(pty.ending(), Some(Ending::EndedWell));
        let process = pty
            .child
            .as_ref()
            .and_then(|child| child.as_raw_handle())
            .expect("the child has a process handle");
        assert!(
            super::terminate(process).is_ok(),
            "a program that already ended was reported as refusing to end"
        );
    }
}
