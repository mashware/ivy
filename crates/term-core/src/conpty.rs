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

// Two functions of `kernel32`, declared here rather than pulled in through a bindings crate: they
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
