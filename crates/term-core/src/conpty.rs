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
//! indefinitely; the reader therefore drains to end of file on Windows, and no close happens on
//! the interface pass.

use std::io::{self, Write};
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use portable_pty::{MasterPty, PtySize};
use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{INFINITE, TerminateProcess, WaitForSingleObject};

/// How long a closed console's program has, after its pseudoconsole is closed, before it is ended.
///
/// **The same 200 ms the Unix close gives after its hang-up**, for the same reason: a program that
/// catches the close to save its state is owed that time (D55). Closing the pseudoconsole is what
/// tells it — Windows sends the programs attached to it a close event.
const CLOSE_GRACE: Duration = Duration::from_millis(200);

/// Where a console's master lives on Windows: whoever takes it out and drops it closes the
/// pseudoconsole. Shared by the `Pty` (to resize, and to close on drop) and the exit watcher.
///
/// **The lock is never held across the drop**, and only this type takes it: the drop is
/// `ClosePseudoConsole`, which can wait for the host, and `resize` takes the same lock on the
/// interface thread.
#[derive(Clone)]
pub(crate) struct Slot(Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>);

impl Slot {
    pub(crate) fn new(master: Box<dyn MasterPty + Send>) -> Self {
        Self(Arc::new(Mutex::new(Some(master))))
    }

    fn lock(&self) -> MutexGuard<'_, Option<Box<dyn MasterPty + Send>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Resizes the pseudoconsole, or does nothing once it is closed: there is nothing left in it
    /// that could repaint.
    pub(crate) fn resize(&self, size: PtySize) -> crate::Result<()> {
        match self.lock().as_ref() {
            Some(master) => Ok(master.resize(size)?),
            None => Ok(()),
        }
    }

    /// Takes the master out; the caller owns it from here, and dropping it is the close.
    fn take(&self) -> Option<Box<dyn MasterPty + Send>> {
        self.lock().take()
    }
}

/// Watches the console's program and closes the pseudoconsole when it ends, so the reader gets its
/// end of file and the console reports an ordinary ending.
///
/// **Its wait has one way out, the program ending**, and every close provides it: closing the
/// pseudoconsole ends what is attached to it, and the close thread ends the program after
/// [`CLOSE_GRACE`]. A program that survives both keeps this thread for as long as it lives — the
/// trade D56 already makes for a program that cannot be ended.
///
/// When the watcher cannot be armed the console works as it did before it existed, without noticing
/// its program's exit — and standard error says so, once.
pub(crate) fn watch_exit(process: Option<BorrowedHandle<'_>>, slot: Slot) {
    let Some(process) = process else {
        unwatched("its program has no handle to wait on");
        return;
    };
    let owned = match process.try_clone_to_owned() {
        Ok(owned) => owned,
        Err(failure) => {
            unwatched(&failure.to_string());
            return;
        }
    };
    let armed = thread::Builder::new()
        .name("conpty-exit".to_owned())
        .stack_size(64 * 1024)
        .spawn(move || match wait(owned.as_handle(), INFINITE) {
            Some(true) => drop(slot.take()),
            // A wait that cannot be made never closes a console whose program may be running.
            _ => unwatched(&io::Error::last_os_error().to_string()),
        });
    if let Err(failure) = armed {
        unwatched(&failure.to_string());
    }
}

fn unwatched(why: &str) {
    let _ = writeln!(
        io::stderr().lock(),
        "a console will not notice its program ending: {why}"
    );
}

/// Closes a console: the pseudoconsole first, then, after [`CLOSE_GRACE`], the program if it is
/// still there — on a thread of its own, so the interface pass never waits for either.
///
/// **If that thread cannot be created, nothing is closed on this thread.** The program is ended at
/// once, and the master is leaked: closing here could wait for ever when the reader is not draining
/// (a reader that panicked does not), and a frozen window is the one outcome this must never have.
/// The reader and the writer then stay parked for the life of the process, which is said once.
pub(crate) fn close(slot: &Slot, process: Option<BorrowedHandle<'_>>, pid: Option<u32>) {
    let owned = process.and_then(|process| process.try_clone_to_owned().ok());
    let closing = slot.clone();
    let spawned = thread::Builder::new()
        .name("conpty-close".to_owned())
        .stack_size(64 * 1024)
        .spawn(move || {
            drop(closing.take());
            if let Some(process) = owned
                && wait(process.as_handle(), CLOSE_GRACE.as_millis() as u32) != Some(true)
                && let Err(refused) = terminate(process.as_handle())
            {
                refused_to_end(pid, &refused);
            }
        });
    if let Err(failure) = spawned {
        if let Some(process) = process
            && let Err(refused) = terminate(process)
        {
            refused_to_end(pid, &refused);
        }
        // The closure, and the clone of the slot in it, were dropped by `spawn`; the master is
        // still in the slot.
        std::mem::forget(slot.take());
        let _ = writeln!(
            io::stderr().lock(),
            "a console was closed without closing its pseudoconsole ({failure}): its reading and \
             writing threads stay for the life of the app"
        );
    }
}

/// D56's line, for Windows.
fn refused_to_end(pid: Option<u32>, refused: &io::Error) {
    let _ = writeln!(
        io::stderr().lock(),
        "a console was closed and Windows refused to end its program (pid {}): {refused}. \
         It may keep running",
        pid.map_or_else(|| "unknown".to_owned(), |pid| pid.to_string())
    );
}

/// Whether the process ended within `milliseconds`, or `None` when the wait itself failed — which
/// only a handle that is not a process can cause.
fn wait(process: BorrowedHandle<'_>, milliseconds: u32) -> Option<bool> {
    // SAFETY: the handle is open for the length of the call, as its borrow says.
    match unsafe { WaitForSingleObject(process.as_raw_handle(), milliseconds) } {
        WAIT_OBJECT_0 => Some(true),
        WAIT_TIMEOUT => Some(false),
        _ => None,
    }
}

/// Whether the process has ended, asked of its handle and never of its exit code: a program that
/// exits with 259 reports the same code Windows uses for «still running».
pub(crate) fn has_ended(process: BorrowedHandle<'_>) -> bool {
    wait(process, 0) == Some(true)
}

/// Ends the program, and says when Windows refuses.
///
/// **`portable-pty`'s own `kill` cannot be used for this**: it reads `TerminateProcess`'s result the
/// wrong way round and then discards it, so a refusal and a success look the same.
fn terminate(process: BorrowedHandle<'_>) -> io::Result<()> {
    if has_ended(process) {
        return Ok(());
    }
    // SAFETY: the handle is open for the length of the call; the call touches no memory of ours.
    if unsafe { TerminateProcess(process.as_raw_handle(), 1) } != 0 {
        return Ok(());
    }
    let refused = io::Error::last_os_error();
    // A program that ended between the look and the call answers «access denied»: that is not a
    // refusal, and the line written for one would be false.
    if has_ended(process) {
        Ok(())
    } else {
        Err(refused)
    }
}

/// Run only on Windows, by the prerelease workflow's Windows job (D108): the windows_exit prefix
/// is what that job filters and names.
#[cfg(test)]
mod tests {
    use crate::{Ending, Pty, Size, Termination};
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

    /// Drains the output until the console reports how it ended, or the deadline passes.
    fn wait_for_ending(pty: &Pty, output: &Receiver<Vec<u8>>) {
        let deadline = Instant::now() + DEADLINE;
        while pty.ending().is_none() && Instant::now() < deadline {
            // Bounded by the deadline; drained so the reader is never parked on a full channel.
            let _ = output.recv_timeout(Duration::from_millis(10));
        }
    }

    /// Whether `thread` finishes within five seconds.
    fn finishes<T>(thread: &std::thread::JoinHandle<T>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        thread.is_finished()
    }

    /// **The defect itself**: a program that ends on its own ends its console, with nobody closing
    /// it. Before the exit watcher this waited for ever, because the pseudoconsole stayed open.
    #[test]
    fn windows_exit_ordinary_end_is_reported() {
        let (pty, output) = spawn("Write-Output ('IVY' + '_OK')");
        wait_for_marker(&output);
        wait_for_ending(&pty, &output);
        assert_eq!(
            pty.ending(),
            Some(Ending::EndedWell),
            "the program ended and its console never did"
        );
    }

    /// **Closing a live console leaves no reading thread behind.** The close used to drop the
    /// master last, after giving up on a reader that only that drop could end; now the reader ends
    /// with the end of file the close produces. Its handle is taken so `Drop` does not wait for it:
    /// what is asserted is that it ends, whatever the close takes.
    #[test]
    fn windows_exit_close_ends_the_reader() {
        let (mut pty, output) = spawn("Write-Output ('IVY' + '_OK'); Start-Sleep 30");
        wait_for_marker(&output);
        let reader = pty.reader.take().expect("the reader thread is there");
        drop(pty);
        assert!(
            finishes(&reader),
            "the reader is still waiting after its console was closed"
        );
    }

    /// **A writer parked on a program that reads nothing ends once its console is closed.** The
    /// premise is asserted first: what is owed only goes down when a whole write lands, so a
    /// backlog two seconds later means the writer is still inside that write.
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
        assert!(
            finishes(&writer),
            "the writer is still parked after its console was closed"
        );
    }

    /// **An exit code of 259 is an ended program.** Windows uses that number for a live process,
    /// so a program that exits with it must be told apart by its handle — by the watcher, which
    /// would otherwise never end the console, by the status the window keeps a pane open on, and by
    /// closing, which would otherwise call it refused.
    #[test]
    fn windows_exit_an_exit_code_of_259_is_an_ended_program() {
        let (pty, output) = spawn("Write-Output ('IVY' + '_OK'); exit 259");
        wait_for_marker(&output);
        wait_for_ending(&pty, &output);
        assert_eq!(pty.ending(), Some(Ending::EndedWell));
        assert_eq!(
            pty.termination(),
            Termination::Ordinary,
            "the program ended and its status still says it is running"
        );
        let process = pty
            .child
            .as_ref()
            .and_then(|child| child.as_raw_handle())
            .expect("the child has a process handle");
        // SAFETY: the handle belongs to the child `pty` still owns.
        let process = unsafe { std::os::windows::io::BorrowedHandle::borrow_raw(process) };
        assert!(
            super::terminate(process).is_ok(),
            "a program that already ended was reported as refusing to end"
        );
    }
}
