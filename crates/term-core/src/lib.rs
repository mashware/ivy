#![deny(rustdoc::broken_intra_doc_links)]
// **A broken rustdoc link is invisible to the compiler, and half the repository is rustdoc.** When this
// was put in, the tree already had one pointing at a constant that an earlier rename had left
// behind: nobody noticed because `cargo doc` warns and does not fail. With the `deny` it fails, and the
// milestone that renames 46,458 lines will be grateful for it.
//
// It fires on `cargo doc`, **not** on `cargo test` — this repository has no continuous integration, so
// it still has to be invoked by hand. What changes is that now it breaks instead of warning.
//! A split: the PTY and the terminal grid.
//!
//! It depends on nothing visual: if the painting layer is rewritten, this crate still serves.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError, channel, sync_channel,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use portable_pty::{Child, CommandBuilder, MasterPty, native_pty_system};

mod screen;
mod view;

pub use screen::{Cell, Half, MouseReporting, SCROLLBACK, Screen, Scroll};
pub use view::{
    CellWidth, CombiningSpan, Cursor, CursorShape, TermColor, Underline, View, ViewCell,
};

/// Error of any operation on the pseudoterminal.
///
/// An opaque error is used instead of a bespoke enum because there are not yet variants that the
/// caller could handle differently. When there are, it is swapped in without breaking the signature.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub type Result<T> = std::result::Result<T, Error>;

/// Size of the pseudoterminal, in cells.
///
/// A bespoke type and not `portable-pty`'s: it keeps the dependency out of the public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

impl Default for Size {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

/// A program running inside a pseudoterminal.
///
/// The output arrives over a channel, fed by a dedicated reader thread: the consumer does not block
/// at the pace of the program. The bytes are delivered **raw**, neither interpreted nor validated as
/// UTF-8 —an escape sequence may arrive split across two messages—, because interpreting them is the
/// job of another layer.
pub struct Pty {
    child: Box<dyn Child + Send + Sync>,
    master: Box<dyn MasterPty + Send>,
    /// What is waiting to be written, on its way to the writer thread.
    ///
    /// **The write does not happen on the caller's thread, and that is the whole of it.**
    /// Writing to a pseudoterminal blocks when the program inside is not reading its input —which is
    /// exactly the brake this crate uses on the *output* side, on purpose— and the caller here is the
    /// thread that paints. Handing the bytes to a thread of its own is what keeps the window alive
    /// while the program catches up.
    to_write: Option<Sender<Vec<u8>>>,
    /// What has been handed over and not yet written, shared with the writer thread.
    input: Arc<Input>,
    /// **Kept although it is never joined**, and the difference matters: with the handle discarded,
    /// «we choose not to wait for it» is not a choice anybody can see in the code, and no test can
    /// reach the thread. See [`Pty::drop`] for why it is not joined.
    writer: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
    /// Goes quiet when the reader thread ends, which is how closing waits for it **with a deadline**.
    ///
    /// **Nothing is ever sent on it.** The thread owns the other half and never uses it; what says
    /// «the thread is gone» is that half being dropped, which turns a `recv` here into
    /// `Disconnected` at once. A channel is used and not the `JoinHandle` because `join` has no
    /// timed form, and waiting for that thread without a deadline is the whole of this defect.
    reader_ended: Receiver<()>,
    /// What pulls the reader thread out of a wait for room in the channel when it has to close.
    ///
    /// Without this, destroying the `Pty` while the channel is full **hangs the close**: the thread
    /// waits for a slot that would only appear if someone drained it, and the `join` below waits for
    /// the thread.
    stop: Arc<AtomicBool>,
}

/// What has been handed over for the program to read, and how much of it has not gone in yet.
///
/// **It exists because the fix for the blocking write traded a visible failure for an invisible
/// one.** With the write on a thread of its own the window no longer freezes — but a program that
/// stops reading for good leaves that thread stuck inside a single `write_all`, and everything typed
/// afterwards queues behind it, `Ctrl+C` included. `Pty::write` keeps answering «taken», the console
/// keeps repainting, and it simply stops listening.
///
/// **The old bug was at least legible.** This is what makes the new one legible too: whoever hosts
/// this crate can ask how much is owed and since when, and say so. Declaring it rather than deducing
/// it is what this project does everywhere else.
#[derive(Default)]
struct Input {
    /// Bytes handed over and not yet written. It only ever goes down when a write really lands.
    owed: AtomicUsize,
    /// When the oldest unwritten byte was handed over, in milliseconds since the epoch of
    /// [`Input::now`]. Zero means nothing is owed.
    waiting_since: AtomicU64,
    /// Whether the writer gave up, which means the program or the pseudoterminal is gone.
    gave_up: AtomicBool,
}

/// What the program has not taken yet, for whoever wants to say it out loud.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backlog {
    /// How many bytes were handed over and have not been written.
    pub bytes: usize,
    /// How long the oldest of them has been waiting.
    pub waiting: Duration,
    /// Whether the writer gave up: nothing else will ever go in through this pseudoterminal.
    pub gave_up: bool,
}

impl Input {
    /// The clock these two numbers share. Monotonic, so a change of the wall clock cannot make a
    /// backlog look like it has been waiting for a negative amount of time.
    fn now() -> u64 {
        std::time::Instant::now()
            .duration_since(*STARTED)
            .as_millis() as u64
    }

    /// Notes that these bytes were handed over.
    fn handed_over(&self, bytes: usize) {
        // **The stamp is only set by the first one waiting**, which is what makes `waiting` mean «how
        // long the oldest has been stuck» instead of «how long since the last keystroke».
        let _ = self.waiting_since.compare_exchange(
            0,
            Self::now().max(1),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        self.owed.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Notes that a write finished, well or badly.
    fn settle(&self, bytes: usize, failure: Option<&std::io::Error>) {
        if failure.is_some() {
            // **What is owed is NOT cleared here.** The bytes of this write and everything queued
            // behind it are never going in, and saying «nothing is owed» would be exactly the silence
            // this type exists against.
            self.gave_up.store(true, Ordering::Relaxed);
            return;
        }
        let left = self.owed.fetch_sub(bytes, Ordering::Relaxed) - bytes;
        if left == 0 {
            self.waiting_since.store(0, Ordering::Relaxed);
        } else {
            self.waiting_since
                .store(Self::now().max(1), Ordering::Relaxed);
        }
    }

    /// Whether the writer gave up: nothing else will ever go in through this pseudoterminal.
    fn gave_up(&self) -> bool {
        self.gave_up.load(Ordering::Relaxed)
    }

    /// What is owed right now, or nothing when the program is keeping up.
    fn backlog(&self) -> Option<Backlog> {
        let bytes = self.owed.load(Ordering::Relaxed);
        let gave_up = self.gave_up.load(Ordering::Relaxed);
        if bytes == 0 && !gave_up {
            return None;
        }
        let since = self.waiting_since.load(Ordering::Relaxed);
        let waiting = if since == 0 {
            Duration::ZERO
        } else {
            Duration::from_millis(Self::now().saturating_sub(since))
        };
        Some(Backlog {
            bytes,
            waiting,
            gave_up,
        })
    }
}

/// The origin of the monotonic clock the backlog is measured with.
static STARTED: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);

/// How long closing waits for the reader thread before abandoning it.
///
/// **It separates «microseconds» from «never», and it is only ever spent when somebody outlived the
/// program and kept the slave end open.** With nobody holding it, the child's death gives the reader
/// end of file and the thread is gone before this is consulted — measured: the ordinary close costs
/// what the library's poll costs and not a millisecond more.
///
/// **It is spent per console, not per gesture**, and that is the number to quote: closing a tab drops
/// every pane it had, in series, inside the same interface pass. Four consoles each holding a
/// survivor is four times this, on top of four of the library's polls.
///
/// **What survives, precisely**, because the obvious examples are not all right: a plain `sleep 60 &`
/// does inherit the slave end and does keep the read from ending — and it stays in the child's own
/// process group, so a signal to the group *would* reach it. What escapes a group signal is a process
/// that made a session of its own, `setsid` or a program that daemonises itself. `nohup` is **not** an
/// example of either: it replaces the terminal on all three descriptors with a file, so a `nohup`'d
/// survivor holds no slave end and cannot cause this at all.
///
/// A tenth of a second is three orders of magnitude above what the ordinary case needs, and it is
/// **not** a threshold anybody measured for how long a frozen window is tolerable: no number here
/// claims that. It is chosen to be plainly inside «a delay» and plainly outside «microseconds».
const READER_GIVE_UP: Duration = Duration::from_millis(100);

/// How many reads fit in the channel before the reader thread stops reading.
///
/// **This number is the terminal's brake, not a performance tuning knob.** While the channel has
/// room, the reader thread drains the pseudoterminal; when it fills up, it stops reading, the
/// kernel's buffer fills up in turn —very little fits: some 20 KiB measured on Linux— and the program
/// inside **blocks on write**, which is the only brake a terminal truly has.
///
/// Eight reads of 8 KiB are 64 KiB: enough that a normal burst does not hit the ceiling, and
/// little enough that what is held back does not matter. The consumer can accumulate on its own all it
/// wants, but if it does so without limit it nullifies this brake — the ceiling only brakes if whoever
/// drains stops draining.
const READS_IN_FLIGHT: usize = 8;

/// How often delivery is retried when the channel is full.
///
/// It is polled instead of waiting blocked because the standard library offers no send with a
/// stable deadline, and the wait has to be abandonable in order to close. With the channel full the
/// pace is set by whoever drains —on the order of a frame—, so this interval adds no perceptible
/// delay.
const RETRY: Duration = Duration::from_millis(1);

/// What this crate sets in the child's environment on its own, on top of what it is passed.
///
/// It is a value and not spread across calls so it can be **read at a glance and asserted on in a
/// test**: whoever injects environment into a user's program has to be able to show the complete list
/// of what it injects.
///
/// Without the terminal type and the truecolor hint, a full-screen program believes it is in a dumb
/// terminal and emits neither color nor the sequences the grid needs to interpret.
pub const FIXED_ENV: [(&str, &str); 2] = [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];

impl Pty {
    /// Opens a pseudoterminal, launches `program` inside and returns the channel of its output.
    ///
    /// The channel is handed back separately, not hung off the `Pty`, because whoever consumes it may
    /// want to do so from another thread — and a borrowed `Receiver` does not cross threads, though a
    /// moved one does. Handing it out through the constructor also avoids there being two ways to reach
    /// it.
    ///
    /// The child is set what [`FIXED_ENV`] says.
    ///
    /// **The channel has a ceiling, and that is what brakes the program inside.** See
    /// [`READS_IN_FLIGHT`]. Whoever drains the channel without ever stopping —a `for` over it, with no
    /// pace or limit of its own— leaves the ceiling without effect and hands the problem back to where
    /// it was.
    pub fn spawn(program: &str, args: &[&str], size: Size) -> Result<(Self, Receiver<Vec<u8>>)> {
        Self::spawn_with_env(program, args, size, &[])
    }

    /// Like [`Pty::spawn`], and additionally decides the environment variables in `env` for the child:
    /// a value sets them, `None` **removes them**.
    ///
    /// **Removing has to be possible, and it is not the same as not setting.** `portable-pty` starts
    /// by copying this process's environment, so what is not touched **is inherited**: a variable that
    /// the caller considers its own and this time has no value would still be there with the parent's
    /// value. With `None` it is deleted from the copy, and then the child does not see it come from
    /// wherever.
    ///
    /// It is applied **after** [`FIXED_ENV`], and on top of the inherited environment: setting replaces
    /// the copied entry, because `portable-pty`'s `CommandBuilder` stores them in a map keyed by name.
    ///
    /// The values are `OsStr` because a path is not guaranteed to be UTF-8 and this crate has no
    /// business being the one that discovers it.
    pub fn spawn_with_env(
        program: &str,
        args: &[&str],
        size: Size,
        env: &[(&str, Option<&OsStr>)],
    ) -> Result<(Self, Receiver<Vec<u8>>)> {
        let pair = native_pty_system().openpty(portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        for (name, value) in FIXED_ENV {
            cmd.env(name, value);
        }
        for (name, value) in env {
            match value {
                Some(value) => cmd.env(name, value),
                None => cmd.env_remove(name),
            }
        }
        let child = pair.slave.spawn_command(cmd)?;

        // The slave end is only needed by the child. Dropping it here is **necessary** for the read
        // of the master to reach end of file when the child dies — keeping it would block the reader
        // thread for ever — but it is not sufficient, and reading it as sufficient is the mistake
        // this crate paid for: anything the child launches inherits these same descriptors, so while
        // one of those lives there is no end of file either. See [`READER_GIVE_UP`].
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;

        // **A thread of its own for writing, and it is not symmetry for its own sake.** `write_all`
        // on a pseudoterminal whose program is not reading blocks until somebody reads, and there are
        // cases measured in this repository where that is never. Here it blocks a thread that paints
        // nothing and reads nothing, so the window keeps answering and the bytes go in when the
        // program comes back for them.
        let (to_write, waiting) = channel::<Vec<u8>>();
        let stop = Arc::new(AtomicBool::new(false));
        let input = Arc::new(Input::default());
        let input_for_thread = Arc::clone(&input);
        let stop_for_writer = Arc::clone(&stop);
        let writer = thread::spawn(move || {
            while let Ok(bytes) = waiting.recv() {
                // **Abandoned on close, not drained into a pseudoterminal that is being killed.**
                // The sender is dropped first and the child killed right after, so without this the
                // last chunks would be written into a program that is already going away.
                if stop_for_writer.load(Ordering::Relaxed) {
                    break;
                }
                let outcome = writer
                    .write_all(&bytes)
                    // **The `flush` travels with the write, error included.** It is a no-op on Unix
                    // for this writer —and on Windows's ConPTY too— but discarding its `Result`
                    // would leave the loop happily writing into something that is not delivering, on
                    // the one platform where it could ever say so.
                    .and_then(|()| writer.flush());
                input_for_thread.settle(bytes.len(), outcome.as_ref().err());
                if outcome.is_err() {
                    // **There is nobody left to write to** —the program died, the pseudoterminal is
                    // gone— and retrying would spin. What was queued behind this is **not** thrown
                    // away in silence: it stays counted as owed, and [`Pty::backlog`] is what
                    // says so out loud.
                    break;
                }
            }
        });

        let (tx, output) = sync_channel(READS_IN_FLIGHT);
        let stop_for_thread = Arc::clone(&stop);
        let (ended, reader_ended) = channel::<()>();
        let reader = thread::spawn(move || {
            // Moved in and never used: what closing reads is this being dropped when the thread ends.
            let _ended = ended;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if deliver(&tx, buf[..n].to_vec(), &stop_for_thread).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok((
            Self {
                child,
                // The master is kept so it can be resized.
                //
                // **And the sentence that used to be here was false, which is what let a hang live
                // for a month.** It said the reader's end of file «depends on nobody holding the
                // slave end open, and the only one who holds it is the child». The second half stops
                // being true the moment the program inside launches something that outlives it —
                // which is what a shell is for. That survivor inherits the slave's descriptors, so
                // there is no end of file while it lives, and see [`Pty::drop`] for what closing does
                // about it.
                master: pair.master,
                to_write: Some(to_write),
                input,
                writer: Some(writer),
                reader: Some(reader),
                reader_ended,
                stop,
            },
            output,
        ))
    }

    /// Tells the program that the terminal has changed size.
    ///
    /// It is the half that makes the program inside find out and repaint. The other half —reflowing
    /// the grid— is done by [`Screen::resize`], and **both are needed**: with only this one, the grid
    /// stays at the old size; with only the other, `vim` keeps painting as if nothing had happened.
    pub fn resize(&mut self, size: Size) -> Result<()> {
        self.master.resize(portable_pty::PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }

    /// Writes bytes to the program, as if they had been typed.
    ///
    /// **It returns without waiting for the program to accept them**, and that is the point: the
    /// caller is the thread that paints, and a pseudoterminal whose program is not reading its input
    /// blocks the writer for as long as it takes — measured in this repository as *never*, in more
    /// than one shape. What is handed over here is written **in order**, by a thread that blocks
    /// nothing anybody is looking at.
    ///
    /// **«Taken» is not «written», and the difference is visible**: what has been handed over and
    /// has not gone in is [`Pty::backlog`], and a program that stops reading for good leaves it
    /// growing. Whoever hosts this crate is expected to say so — otherwise the console simply stops
    /// listening without a word, which is a worse failure than the freeze this replaced.
    ///
    /// **The queue carries no ceiling.** The reason is volume, not principle: what arrives here is
    /// what somebody's hands produced —a keystroke, a paste the desktop already holds whole in
    /// memory— so there is no producer to outrun a consumer the way a program dumping hundreds of
    /// MiB/s does on the output side. That asymmetry is why **D8**'s ceiling is not copied here: D8
    /// chose ceilings *with backpressure* for a producer that had to be braked, and there is nothing
    /// to brake on this side. What a ceiling here would buy is bounded memory in one case —a program
    /// that never reads again— at the price of choosing what to throw away, and it would have to be
    /// backpressure rather than discarding. It is written up as a decision of its own.
    ///
    /// The error is returned when the writer thread is gone, which means the console is closing.
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        // **A writer that gave up is reported here and not left to be discovered.** Once it has
        // broken out of its loop nothing else will ever reach the program, and answering «taken» to
        // every write after that would be the silence this whole fix exists against. Measured before
        // this line existed: four writes after the program had died all returned `Ok`.
        if self.input.gave_up() {
            return Err("the program is no longer taking what is written to it".into());
        }
        // **`Option` only so that `Drop` can close the channel**; no caller can observe the `None`,
        // because a value being dropped no longer answers `&mut self`.
        let Some(to_write) = self.to_write.as_ref() else {
            return Err("the console is closing".into());
        };
        self.input.handed_over(bytes.len());
        to_write
            .send(bytes.to_vec())
            .map_err(|_| -> Error { "the console is closing".into() })?;
        Ok(())
    }

    /// What has been handed over to the program and it has not taken yet, or nothing when it is
    /// keeping up.
    ///
    /// **This is the half that keeps the fix honest.** Moving the write off the interface thread
    /// stopped the window freezing, and in exchange a program that stops reading for good leaves a
    /// console that repaints and no longer listens, with `Pty::write` answering «taken» every time.
    /// Nothing else in this crate can see that state; this is what lets whoever hosts it say so.
    pub fn backlog(&self) -> Option<Backlog> {
        self.input.backlog()
    }
}

/// Leaves a chunk in the channel, waiting for room, and gives up if it has to close.
///
/// Returning `Err` means "there is no one to deliver to": the channel closed or is closing. The
/// caller ends the read loop.
///
/// **The wait is the brake.** While it lasts, this thread does not read from the pseudoterminal, so
/// the kernel's buffer fills up and the program inside blocks on write. Nothing is discarded: the
/// bytes stay where they already were, in the program that wrote them.
fn deliver(
    channel: &SyncSender<Vec<u8>>,
    chunk: Vec<u8>,
    stop: &AtomicBool,
) -> std::result::Result<(), ()> {
    let mut pending = chunk;
    loop {
        // It is checked before trying and not only after failing: if the close arrives right when
        // the channel has room, delivering one more chunk breaks nothing, but checking first makes
        // the exit from the loop not depend on the state of the channel.
        if stop.load(Ordering::Relaxed) {
            return Err(());
        }
        match channel.try_send(pending) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(returned)) => {
                pending = returned;
                thread::sleep(RETRY);
            }
            Err(TrySendError::Disconnected(_)) => return Err(()),
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // First the flag: with the channel full, the thread is not inside `read` but waiting for
        // room, and neither killing the child nor closing the pseudoterminal pulls it out of there.
        // Without this, the `join` below waits forever and **closing the window hangs** — confirmed.
        self.stop.store(true, Ordering::Relaxed);
        // **The sender is dropped and the writer thread is NEVER joined**, and both halves matter.
        // Dropping it is what ends the loop; not joining is because that thread may be blocked
        // inside a `write_all` that will not return, so waiting for it would hang closing the
        // console — the very failure the flag above exists to prevent. The flag is what it reads to
        // abandon what is still queued instead of writing it into a pseudoterminal about to be
        // killed.
        //
        // **And killing the child does NOT unblock that write, which is measured and not assumed.**
        // Fifteen seconds after dropping a `Pty` whose writer was blocked, the thread was still
        // there. So a console closed while its program had stopped reading **leaks that thread and
        // what it was holding, for the life of the process** — once per such console. It is not
        // fixed here: undoing it needs the write to be interruptible, which is a different shape of
        // write, and it is ticketed with this measurement.
        //
        // **What is queued at this point is abandoned, and that is an exception to what `write`
        // promises.** It is stated here rather than left to be discovered: closing a console throws
        // away what had been handed over and not yet written. There is nowhere for those bytes to go
        // — the program they were for is being killed on the next line.
        drop(self.to_write.take());
        // Not joined, on purpose — see above. Taking it here says so where somebody looking for the
        // `join` will find it.
        let _ = self.writer.take();
        // And then the usual order: killing the child is what closes its end of the
        // pseudoterminal and triggers the end of file in the reader thread. Waiting for the thread
        // before that would hang it, because it would still be blocked inside `read`.
        let _ = self.child.kill();
        // Bounded, and the bound is the library's: after its grace period that `kill` falls through
        // to a real `SIGKILL`, which nobody holds — not even a stopped process, the shape that would
        // otherwise never be reaped. The one hole is that the library gives up early if the SIGHUP
        // itself could not be delivered, and this call discards that: with the child already gone
        // there is nothing to wait for anyway.
        let _ = self.child.wait();
        // **The reader is waited for with a deadline, and abandoned if it does not come.** Killing
        // the child closes *the child's* end of the pseudoterminal and nothing else: anything the
        // program launched that outlived it —a `sleep &`, a development server— inherited those same
        // descriptors, so while it lives there is no end of file, and this thread is parked inside
        // `read` where the stop flag above cannot reach it. It used to be joined without a deadline,
        // and this destructor runs **inside the interface pass**, so the whole window stayed frozen
        // with no repaint and no keyboard, for as long as that program lived.
        //
        // **Signalling the process group would not be enough**, which is worth writing down because
        // it is the obvious idea and because the weaker claim —that it would do nothing— is false. A
        // plain background job stays in the child's group and a group signal would reach it; a
        // survivor that made a session of its own is exactly what it does not reach. So a group
        // signal would close some of these and not others, and it would also kill the background
        // jobs of whoever closed the pane, which is a decision about what closing *means*. A deadline
        // bounds every case without taking that decision.
        if let Some(reader) = self.reader.take() {
            match self.reader_ended.recv_timeout(READER_GIVE_UP) {
                // The thread is gone and its half of the channel went with it. Gone, not necessarily
                // gone well: a reader that panicked looks exactly like this, and the `join` here
                // discards that too.
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = reader.join();
                }
                // **Abandoned on purpose, and it costs a thread and a descriptor for as long as the
                // survivor stays silent.** Dropping the handle detaches it; the thread is still
                // inside `read`, but the stop flag above is already raised, so **the survivor's next
                // byte ends it** — the read returns, the delivery sees the flag and the thread
                // leaves. A survivor that never writes again holds them until the process ends. That
                // is the same trade the writer thread already makes for the same reason, and a better
                // one than the window freezing for ever, which is what the alternative was.
                Err(RecvTimeoutError::Timeout) => drop(reader),
                // **Unreachable, and spelled out rather than folded into the arm above.** Nothing is
                // ever sent on that channel, so this cannot happen; if somebody ever gives it a
                // payload, abandoning a thread that just spoke would be a live thread thrown away in
                // silence, and this arm is where that has to be reconsidered.
                Ok(()) => drop(reader),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::{Duration, Instant};

    /// A generous margin: what is checked is that the channel ends up closing, not how fast it does
    /// so. Any bounded wait serves as long as it is not infinite.
    const DEADLINE: Duration = Duration::from_secs(10);

    /// Drains the channel until it closes and returns everything received.
    ///
    /// With a deadline on purpose: a `recv()` without a deadline would turn the failure these tests
    /// hunt —a reader thread that does not finish— into an eternal hang instead of a red test.
    fn drain_until_closed(pty_output: &Receiver<Vec<u8>>) -> Vec<u8> {
        let mut output = Vec::new();
        loop {
            match pty_output.recv_timeout(DEADLINE) {
                Ok(bytes) => output.extend_from_slice(&bytes),
                Err(RecvTimeoutError::Disconnected) => return output,
                Err(RecvTimeoutError::Timeout) => {
                    panic!(
                        "the channel was still open after {DEADLINE:?}: the reader thread did not finish"
                    )
                }
            }
        }
    }

    fn contains(output: &[u8], text: &str) -> bool {
        String::from_utf8_lossy(output).contains(text)
    }

    /// How much the tests that write hand over: more than the kernel's headroom, on purpose.
    ///
    /// The pseudoterminal takes some 20 KiB before a write blocks —measured on Linux— and the
    /// reproduction blocked at around 13 KiB cumulative. That difference is not explained,
    /// so instead of picking a number next to the boundary these tests write 64 KiB: past it by a
    /// margin wide enough that they do not depend on which side of it this machine falls.
    const PAYLOAD: usize = 64 * 1024;

    /// The byte the tests hand over, repeated.
    ///
    /// **A single repeated byte and a count, not a phrase.** What goes through a pseudoterminal
    /// crosses the line discipline, which translates and echoes; a payload whose journey can be
    /// verified *exactly* is worth more than one that reads nicely in a diff. The letter chosen is
    /// special to none of the modes involved.
    const FILLER: u8 = b'z';

    /// Waits until the program announces, through its own output, that it is already in the state
    /// the test needs.
    ///
    /// **It is a marker and not a `sleep` because the state is what makes the bug reproduce.** With
    /// the echo still on, the kernel echoes back what is written, our reader thread empties the
    /// output and that round trip drains the input buffer on its own: the diagnosis' first attempt
    /// put 128 000 bytes through without blocking. A test that wrote before `stty` had applied would
    /// go green on a build that still carries the bug.
    fn wait_for_marker(pty_output: &Receiver<Vec<u8>>, marker: &str) {
        let mut seen = Vec::new();
        while !contains(&seen, marker) {
            match pty_output.recv_timeout(DEADLINE) {
                Ok(bytes) => seen.extend_from_slice(&bytes),
                Err(_) => panic!(
                    "the program never announced {marker:?} in {DEADLINE:?}: {:?}",
                    String::from_utf8_lossy(&seen)
                ),
            }
        }
    }

    // Mind the `_pty` in the tests: it is not a leftover variable. As long as it lives, the child
    // lives; replacing it with `_` drops it on the spot, and `Drop` kills the program before it has
    // time to write. The test would not always fail — it would fail sometimes, which is worse.

    #[test]
    fn the_channel_closes_when_the_program_finishes() {
        let (_pty, output) =
            Pty::spawn("sh", &["-c", "exit 0"], Size::default()).expect("launch sh");

        drain_until_closed(&output);
    }

    #[test]
    fn a_nonexistent_program_gives_an_error_instead_of_a_panic() {
        let result = Pty::spawn("program-that-does-not-exist-anywhere", &[], Size::default());
        assert!(result.is_err());
    }

    #[test]
    fn what_the_program_writes_arrives_over_the_channel() {
        let (_pty, channel) =
            Pty::spawn("sh", &["-c", "printf hola"], Size::default()).expect("launch sh");

        let output = drain_until_closed(&channel);

        assert!(
            contains(&output, "hola"),
            "the program's output did not arrive: {output:?}"
        );
    }

    #[test]
    fn the_program_receives_the_terminal_type() {
        // Without this, a full-screen program believes it is in a dumb terminal and emits neither
        // color nor the sequences the grid needs to interpret.
        let (_pty, channel) =
            Pty::spawn("sh", &["-c", "printf %s \"$TERM\""], Size::default()).expect("launch sh");

        let output = drain_until_closed(&channel);

        assert!(
            contains(&output, "xterm-256color"),
            "the child did not see TERM: {:?}",
            String::from_utf8_lossy(&output)
        );
    }

    /// The half that makes a console's identity **actually reach** the program inside. Without this
    /// test, an implementation that stored the pairs and did not set them would leave the whole suite
    /// green.
    #[test]
    fn the_program_receives_the_variables_passed_to_it() {
        let (_pty, channel) = Pty::spawn_with_env(
            "sh",
            &["-c", "printf %s \"$TEST_VARIABLE\""],
            Size::default(),
            &[("TEST_VARIABLE", Some(OsStr::new("3f2a9c14")))],
        )
        .expect("launch sh");

        let output = drain_until_closed(&channel);

        assert!(
            contains(&output, "3f2a9c14"),
            "the child did not see the variable: {:?}",
            String::from_utf8_lossy(&output)
        );
    }

    /// The B side: what is not passed does not appear.
    ///
    /// The name is bespoke to this test on purpose. With one that someone might have defined —`PANEL_ID`,
    /// for example— the test **inherits** that value and goes red because of the environment of whoever
    /// runs it, not because of the code: confirmed, it happens when running the suite inside a console of
    /// this application.
    #[test]
    fn without_passing_anything_the_variable_is_not_defined() {
        let (_pty, channel) = Pty::spawn(
            "sh",
            &["-c", "printf %s \"${TEST_VARIABLE-unset}\""],
            Size::default(),
        )
        .expect("launch sh");

        let output = drain_until_closed(&channel);

        assert!(
            contains(&output, "unset"),
            "the variable was defined without anyone passing it: {:?}",
            String::from_utf8_lossy(&output)
        );
    }

    /// **Removing is not not-setting**, and this is the difference that upholds that a console without a
    /// directory does not keep the identity inherited from the application that launched it.
    ///
    /// It is tested against `TERM`, which this crate always sets: if `None` did not remove, the child
    /// would see it with the value of [`FIXED_ENV`]. This way the test does not need the environment of
    /// whoever runs it to have anything in particular.
    #[test]
    fn with_none_by_value_the_variable_is_removed_even_though_the_crate_sets_it() {
        let (_pty, channel) = Pty::spawn_with_env(
            "sh",
            &["-c", "printf %s \"${TERM-removed}\""],
            Size::default(),
            &[("TERM", None)],
        )
        .expect("launch sh");

        let output = drain_until_closed(&channel);

        assert!(
            contains(&output, "removed"),
            "TERM was still defined: {:?}",
            String::from_utf8_lossy(&output)
        );
    }

    /// Before the fix the channel was `mpsc::channel()`, unbounded: a program that writes without
    /// stopping and no one draining made what accumulated grow without a ceiling for as long as the
    /// wait lasted. This test reproduces exactly that —`cat /dev/zero` without consuming the channel— and
    /// checks that what ends up accumulating has a ceiling, not that the ceiling is the exact nominal
    /// one (that is already fixed by [`READS_IN_FLIGHT`]).
    ///
    /// The limit of the check is not arbitrary: eight reads of 8 KiB are 64 KiB of channel, plus
    /// whatever fits in the pseudoterminal's kernel buffer —~20 KiB measured on Linux—, plus at most
    /// one read already pulled from the master and still without room in the channel. That gives a real
    /// ceiling on the order of 90-100 KiB. The margin is left at 1 MiB —ten times that ceiling, so as
    /// not to depend on the exact slack of this machine's kernel buffer— and well below what an
    /// unbounded channel would accumulate in the same wait: `cat /dev/zero` delivers on the order of
    /// hundreds of MiB per second, so in 200 ms an unbounded channel would already have exceeded that
    /// margin several times over. It is exactly that distance the test has to catch.
    #[test]
    fn without_consuming_it_what_arrives_is_bounded() {
        let (_pty, output) =
            Pty::spawn("cat", &["/dev/zero"], Size::default()).expect("launch cat");

        // Without touching the channel: gives the reader thread time to fill it, get stuck waiting
        // for room and make the `cat` inside block on write. That is the state to be measured.
        thread::sleep(Duration::from_millis(200));

        let mut received = 0usize;
        while let Ok(chunk) = output.try_recv() {
            received += chunk.len();
        }

        const GENEROUS_MARGIN: usize = 1024 * 1024; // 1 MiB — see the function's comment.
        assert!(
            received > 0,
            "nothing arrived from \"cat /dev/zero\" in 200 ms: the channel is not being fed"
        );
        assert!(
            received <= GENEROUS_MARGIN,
            "{received} bytes accumulated without consuming the channel: the brake is not acting"
        );
    }

    /// The hang this test covers is demonstrated, not hypothetical: with the channel full, the
    /// reader thread is not inside `read` but waiting for room in [`deliver`], and `Drop for Pty`
    /// does `kill` → `wait` → `join`. Without the `stop` flag that the `Drop` raises before killing
    /// the child, nothing pulls that thread out of its wait —neither the `kill` nor the end of file
    /// reach it while it is blocked there— and the `join` never returns: closing the window hangs.
    ///
    /// The check runs on a separate thread and uses `recv_timeout` to wait for its finish notice,
    /// not a direct `join` of the test's thread: if the hang returns, this test has to fall red
    /// with a message, not stay waiting forever along with it.
    #[test]
    fn closing_with_the_channel_full_does_not_hang() {
        use std::sync::mpsc;

        let (pty, output) = Pty::spawn("cat", &["/dev/zero"], Size::default()).expect("launch cat");

        // Same reason as in the previous test: give the channel time to really fill up. `output`
        // is not touched from here on — the channel has to stay full while `pty` is destroyed,
        // which is exactly the condition of the hang.
        thread::sleep(Duration::from_millis(200));

        let (notice_tx, notice_rx) = mpsc::channel();
        let drop_thread = thread::spawn(move || {
            drop(pty);
            // If the drop hangs, this send never arrives; the `recv_timeout` below is what
            // turns that into a red test instead of a hang of the test itself.
            let _ = notice_tx.send(());
        });

        const CLOSE_DEADLINE: Duration = Duration::from_secs(5);
        if notice_rx.recv_timeout(CLOSE_DEADLINE).is_err() {
            panic!(
                "destroying the Pty with the channel full did not finish in {CLOSE_DEADLINE:?}: \
                 the close hangs"
            );
        }

        // The drop already finished, so this join is immediate; it is here for hygiene, not because
        // it is needed for the test to be correct.
        let _ = drop_thread.join();
        drop(output);
    }

    /// **The regression test of the blocking write.** `Pty::write` was a `write_all` + `flush` on a blocking
    /// descriptor, run on the caller's thread — which in this application is the thread that paints.
    /// When the program inside stops reading its input, the kernel's buffer fills and that write
    /// does not return: the window stops repainting and the `Ctrl+C` that travels the same thread
    /// cannot interrupt it. Measured in the diagnosis, the write that crossed 13 312 bytes took
    /// **5.71 s** — not a property of the bug, but what the program had left to live. With a shell
    /// inside a console, which lives as long as the console does, that is *never*.
    ///
    /// **The write is done on a thread of its own, and that is the whole shape of this test.** On a
    /// build with the bug, calling `Pty::write` from here would not return and would hang the suite
    /// along with it — the bug itself, published as an eternal green. The deadline plus the
    /// `recv_timeout` are what turn it into a red test, the same reason `drain_until_closed` carries
    /// one.
    #[test]
    fn the_write_returns_even_though_the_program_never_reads_its_input() {
        use std::sync::mpsc;

        /// Well below the six seconds the program has left to live, and that is the load-bearing
        /// half: on a build with the bug the write returns **when the program dies**, so a deadline
        /// past that would watch the write «return» and publish the bug as fixed. The other half is
        /// not tight either — three seconds are some eight hundred thousand times what the fixed
        /// write measures (3.62 µs).
        const HANDOVER_DEADLINE: Duration = Duration::from_secs(3);
        const MARKER: &str = "raw-and-deaf";

        // A program in raw mode with no echo that never reads its input: `vim`, `htop`, `less` or a
        // TUI agent, which is precisely what somebody pastes into. `sleep` and not something
        // endless on purpose — on a build with the bug the write thread this test leaves behind
        // stays blocked inside `write_all`, and a bounded program is what lets it end on its own
        // instead of leaving an orphan behind for as long as the machine is up.
        // **Chained with `&&` and not with `;`, and it is the difference between a guard and a
        // decoration.** With `;` the marker prints even when `stty` failed, so on a machine without
        // it —or on the platforms F2.7 is heading for— this test would find its marker, write into a
        // terminal that still echoes, and pass **in hundredths of a second over a live bug**.
        // Measured: bug restored plus `stty` missing gave a green in 0.02 s.
        let script = format!("stty raw -echo && printf {MARKER} && sleep 6");
        let (mut pty, output) =
            Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");

        wait_for_marker(&output, MARKER);

        let (handed_over, notice) = mpsc::channel();
        thread::spawn(move || {
            let payload = vec![FILLER; PAYLOAD];
            let _ = handed_over.send(pty.write(&payload));
        });

        match notice.recv_timeout(HANDOVER_DEADLINE) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => panic!("the write failed instead of being handed over: {error}"),
            Err(_) => panic!(
                "handing over {PAYLOAD} bytes did not return in {HANDOVER_DEADLINE:?}: the write is \
                 blocking on the caller's thread, which is the thread that paints"
            ),
        }
    }

    /// The other half of the fix: **the write stopped waiting, and it did not start losing.** A
    /// queue that returns straight away and drops what does not fit would pass the test above and
    /// silently swallow what somebody typed — the failure that takes longest to understand from the
    /// other side of the screen, and the reason **D8** refuses to discard on the output side too.
    ///
    /// `cat` copies its input back out, so what comes back through the channel of [`Pty::spawn`] is
    /// the payload itself and the check is a count. It starts reading **a second late** on purpose:
    /// the bytes have to be handed over while nobody is reading, which is the only state in which
    /// the queue holds anything at all.
    ///
    /// Raw mode is not decoration here either, and for a second reason: canonical mode **silently
    /// discards past 4 095 bytes with no newline** —a separate bug, found on the way and with an
    /// issue of its own— so a 64 KiB payload would come back short for a reason that has nothing to
    /// do with the one this test hunts.
    #[test]
    fn nothing_handed_over_is_lost_when_the_program_reads_late() {
        use std::sync::mpsc;

        const MARKER: &str = "raw-and-late";
        /// Printed when the program is about to start reading, a second before it does.
        const READING: &str = "now-reading";

        // `&&` for the same reason as its sibling above, and **a second marker for a second
        // reason**: with the echo still on, the terminal hands the payload back by itself and the
        // count reaches its total **without `cat` ever running**. Measured: a correct build with
        // `stty` missing passed in 0.10 s instead of the ~1.1 s the `sleep` imposes, and the
        // «nothing foreign arrived» assertion cannot see it — echoed bytes are the same filler.
        // The second marker is what says the reader really took over.
        let script =
            format!("stty raw -echo && printf {MARKER} && sleep 1 && printf {READING} && exec cat");
        let (mut pty, output) =
            Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");

        wait_for_marker(&output, MARKER);

        // The write goes on a thread of its own for the reason the previous test explains, plus one
        // of this test's own: on a build with the bug, writing from here would **deadlock** rather
        // than block for a while — the write waits for `cat` to read, `cat` waits for room in the
        // output channel, and the only one who could make room is this thread, stuck in the write.
        let (handed_over, notice) = mpsc::channel();
        let (counted, wait_for_the_count) = mpsc::channel::<()>();
        let writer = thread::spawn(move || {
            let payload = vec![FILLER; PAYLOAD];
            let _ = handed_over.send(pty.write(&payload));
            // The `Pty` is held until the count is done. Dropping it here kills `cat` mid-flight
            // and whatever was still queued would never be handed over: the test would go red for a
            // reason that is not the one it covers.
            let _ = wait_for_the_count.recv();
        });

        match notice.recv_timeout(DEADLINE) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => panic!("the write failed instead of being handed over: {error}"),
            Err(_) => panic!("handing over {PAYLOAD} bytes did not return in {DEADLINE:?}"),
        }

        // **The handover has to have happened while nobody was reading, or this test measures
        // nothing.** Its whole premise is that 64 KiB go in with the program not taking them; if the
        // machine were loaded enough for `cat` to be running already, what would be verified is that
        // a pseudoterminal delivers bytes, which nobody doubts.
        wait_for_marker(&output, READING);

        let mut arrived = 0usize;
        let mut foreign = 0usize;
        while arrived < PAYLOAD {
            match output.recv_timeout(DEADLINE) {
                Ok(bytes) => {
                    arrived += bytes.iter().filter(|byte| **byte == FILLER).count();
                    foreign += bytes.iter().filter(|byte| **byte != FILLER).count();
                }
                Err(_) => panic!(
                    "only {arrived} of the {PAYLOAD} bytes handed over came back in {DEADLINE:?}, \
                     plus {foreign} that were not the payload's: what was written is being lost"
                ),
            }
        }
        assert_eq!(
            foreign, 0,
            "{foreign} bytes nobody wrote came back: the journey is not the exact one this count is \
             measured against, so the count proves less than it claims"
        );

        let _ = counted.send(());
        let _ = writer.join();
    }

    /// **The regression test of closing a console whose program left a survivor behind.** `Drop for
    /// Pty` used to join the reader thread with no deadline at all. That thread sits blocked inside
    /// `read` on a descriptor cloned from the master, and the `stop` flag the destructor raises is
    /// only ever consulted **after** a `read` returns — so it cannot reach a thread parked inside
    /// one. Killing the child closes *the child's* end of the pseudoterminal and nothing else:
    /// anything the program launched that outlives it inherited those same descriptors, so while
    /// that survivor lives there is no end of file, the `read` never returns and neither did the
    /// `join`. Measured: a `Drop` that had still not returned after eight seconds. And this
    /// destructor runs inside the interface pass, so the whole window stayed frozen — no repaint, no
    /// keyboard — for as long as that program lived.
    ///
    /// **Two conditions at once are what make it reproduce, and either one missing hides it
    /// completely.**
    ///
    /// The first is the survivor, which is why the shell here launches one that **announces itself**
    /// — see [`HELD`]. It holds the slave end open, and by making a session of its own it is exactly
    /// what a signal to the child, or to the child's group, does not reach. **The brace group around
    /// it is load-bearing.** Written as `stty -echo && … & exec sleep 5`, the `&` backgrounds the
    /// *whole* `&&` list: `stty` would run in a subshell while the main shell was already `exec`ing,
    /// the echo would never go off, and this test would quietly exercise the easy case while claiming
    /// the hard one. That mistake was made and measured while the example beside this crate was
    /// being written.
    ///
    /// The second is the echo being off. On drop, `portable-pty`'s own master writer puts a `\n` and
    /// the terminal's EOF character into the pseudoterminal; while the line discipline echoes, that
    /// comes straight back out of the master, the parked `read` returns **by accident**, and only
    /// then does the reader look at its stop flag and leave. With `-echo` there is nothing to come
    /// back — which is the state a real console's program is in, because turning the echo off is the
    /// first thing `vim`, `htop` or a TUI agent does.
    ///
    /// **The `stty` sits before the survivor in the same `&&` chain, so the marker proves the mode
    /// changed too**: it cannot arrive unless `stty` exited 0. That is not decoration — a test over a
    /// terminal that never set the mode it needs is satisfied by the kernel instead of by its
    /// subject, and would pass in hundredths of a second over a live bug.
    ///
    /// **The channel is drained to empty and its receiver is kept alive.** Both are escape routes the
    /// reader has that are not the pseudoterminal: a delivery that fails and a wait for room both end
    /// up somewhere the stop flag *does* reach, and the thread then leaves without ever issuing
    /// another `read`. What the drain reproduces is what the window actually does when it takes a
    /// console out of the tree — it stops draining the channel, it does not drop it.
    ///
    /// **Which parts actually make this test bind was mutated rather than argued**, because a
    /// scenario that prevents the defect by itself is how this bug survived four attempts. Each row
    /// is one deliberate break, run against a tree with the deadline in `Drop` reverted to the
    /// unbounded `join`:
    ///
    /// | Mutation | Result on the broken tree |
    /// |---|---|
    /// | none — the test as written | **FAILED in 2.06 s** — the close never came back |
    /// | echo left on (`stty -echo` removed) | **FAILED in 0.10 s** — the close came back in 50.7 ms, under the floor |
    /// | the drain to quiet removed | **FAILED in 2.01 s** |
    /// | the receiver released before the close | **FAILED in 2.06 s** |
    /// | the survivor's command made nonexistent | **FAILED in 5.01 s** — nobody printed the marker |
    ///
    /// **Every one of them is red, and two of them are red because of the floor**, not because of the
    /// close: a mutation that removes the hang's premise now fails for saying so rather than passing
    /// quietly. An earlier version of this test had no floor and no survivor marker, and the first
    /// two rows above passed on it in hundredths of a second. What is left as insurance rather than
    /// as proof is the drain and the receiver's lifetime — with this marker the reader is parked
    /// inside `read` either way — and they stay because they are what the window does.
    ///
    /// On the fixed tree it passes in 0.20 s, which is the library's 50 ms poll plus the 100 ms
    /// abandoned deadline plus the drain: the deadline really was spent.
    ///
    /// The drop itself runs on a thread of its own, answering through a channel, for the same reason
    /// `drain_until_closed` carries a deadline: on a build with the bug a direct `join` would never
    /// return and would take the suite down with it, publishing the hang as an eternal green.
    #[test]
    fn closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal() {
        use std::sync::mpsc;

        /// **Both bounds carry weight, and the band between them is where this test binds.**
        /// Measured after the fix, this shape returns in some 150 ms — 50 ms of the library's fixed
        /// poll after its SIGHUP, plus the 100 ms this crate gives the reader before abandoning it —
        /// so a deadline anywhere near that would go red because the machine was busy. And the
        /// survivor only lives five seconds: a deadline past that would watch the close «return»
        /// because the program died on its own rather than because `Drop` came back, and publish the
        /// hang as fixed. Two seconds is more than ten times the cost and well under half the
        /// survivor's life. It is deliberately not a performance figure: what is asserted is
        /// «bounded, not never».
        const CLOSE_DEADLINE: Duration = Duration::from_secs(2);
        /// What the **survivor** prints once it is up.
        ///
        /// **It is the survivor's own and not the shell's, and that is the difference between
        /// assuming the premise and proving it.** `{ … & }` is a backgrounded list: it exits 0
        /// whatever happens inside it, so a missing `setsid` — every macOS, and F2.7 is next — would
        /// let the `&&` chain reach the `exec` with nobody holding the slave end, and this test would
        /// pass on a build that hangs, measuring the easy case. Seeing this marker come out of the
        /// master proves the survivor exists *and* that it can write through the descriptor it
        /// inherited, which is the whole premise. And it is printed last in the chain, so it also
        /// proves the `stty` before it exited 0.
        const HELD: &str = "held";
        /// How long the shell and its survivor live. Long enough that neither ends inside the
        /// deadline above — otherwise the close would finish for a reason that is not the fix.
        const SECONDS_ALIVE: u32 = 5;

        let script = format!(
            "stty -echo && {{ setsid sh -c 'printf {HELD}; sleep {SECONDS_ALIVE}' & }} && exec sleep {SECONDS_ALIVE}"
        );
        let (pty, output) = Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");

        // A marker somebody prints, never a `sleep`: what makes the bug reproduce is the state, and a
        // fixed wait would time the close against a shell that had not got there yet.
        wait_for_marker(&output, HELD);

        // And then drained until the program goes quiet, so the channel is **empty** when the close
        // happens. See the doc comment: a full channel parks the reader somewhere the stop flag
        // reaches it, and the hang needs it parked inside `read`.
        let quiet = Duration::from_millis(50);
        while output.recv_timeout(quiet).is_ok() {}

        let (done, answer) = mpsc::channel();
        let closing = thread::spawn(move || {
            let started = Instant::now();
            drop(pty);
            // If the close hangs this send never arrives, and the `recv_timeout` below is what turns
            // that into a red test instead of a hang of the suite.
            let _ = done.send(started.elapsed());
        });

        let took = match answer.recv_timeout(CLOSE_DEADLINE) {
            Ok(took) => took,
            Err(RecvTimeoutError::Timeout) => panic!(
                "closing the Pty did not return in {CLOSE_DEADLINE:?} with a survivor holding the \
                 slave end and the echo off: the destructor is waiting for a reader thread that \
                 will never come out of its `read`"
            ),
            // Told apart from the timeout on purpose: a `Drop` that panicked would otherwise be
            // reported as a hang, and somebody would go looking for the wrong defect.
            Err(RecvTimeoutError::Disconnected) => {
                panic!("the closing thread ended without answering: `Drop` panicked")
            }
        };

        // **And a floor, which is what makes this test about the deadline rather than about closing
        // in general.** Anything under it means the close came back *without* spending the abandon
        // deadline, and in this shape there is only one way that happens: the reader was woken by
        // something — the echo left on by accident, a survivor that was never there — so the row is
        // measuring the easy case under this one's name. Measured: with the echo on it returns in
        // 0.10 s, and with no survivor in 0.05 s. The bound is the constant itself, so a future
        // delivery that makes abandoning cheaper moves it instead of going red.
        assert!(
            took >= READER_GIVE_UP,
            "the close came back in {took:?}, under the {READER_GIVE_UP:?} it has to spend when a \
             holder survives: something woke the reader, so this is not the case this test is named \
             after"
        );

        // Immediate — the close already answered. On the failing path above the thread is left
        // behind on purpose: it is inside the very wait this test is about.
        let _ = closing.join();
        // The receiver is only released here, after the close. Dropping it any earlier is the
        // disguise the doc comment describes.
        drop(output);
    }

    /// **The other half of the claim, and without it the whole deadline can be deleted green.**
    ///
    /// The test above proves closing comes back when a survivor holds the slave end. It says nothing
    /// about what closing costs when nobody does — and that is the case every console goes through,
    /// so a delivery that made *every* close wait out the deadline would be a real regression with a
    /// green suite. Measured: swapping the timed wait for a plain sleep, or zeroing nothing at all
    /// and simply always abandoning, leaves the whole workspace passing while every closed console
    /// costs a tenth of a second more, inside the interface pass, multiplied by the panes of the tab.
    ///
    /// **The bound is [`READER_GIVE_UP`] itself, not a number typed here.** With nothing holding the
    /// slave end the child's death gives the reader end of file, so the thread has already gone and
    /// the wait returns at once: what is left is the library's own fixed poll, some 50 ms. Asserting
    /// «under the deadline» therefore has two-fold headroom over what this costs and still fails the
    /// moment the deadline starts being paid on a healthy close — and it moves on its own if the
    /// constant ever changes, which a literal would not.
    ///
    /// It is **not** a performance assertion: nothing here claims 50 ms is fast, only that the
    /// hundred milliseconds this crate reserves for a survivor are not spent when there is none.
    #[test]
    fn closing_an_ordinary_console_does_not_spend_the_abandon_deadline() {
        use std::sync::mpsc;

        const HOW_LONG_IT_LIVES: u32 = 5;
        const MARKER: &str = "up";

        let script = format!("printf {MARKER} && exec sleep {HOW_LONG_IT_LIVES}");
        let (pty, output) = Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");

        // The same discipline as the test above, and for the same reasons: the program says when it
        // is up, and the channel is drained to empty so the reader is parked inside `read` rather
        // than somewhere the stop flag reaches it. Without the drain this test would be measuring a
        // reader that left through the delivery path, which is not the path it is about.
        wait_for_marker(&output, MARKER);
        while output.recv_timeout(Duration::from_millis(50)).is_ok() {}

        let (done, answer) = mpsc::channel();
        let closing = thread::spawn(move || {
            let started = Instant::now();
            drop(pty);
            let _ = done.send(started.elapsed());
        });

        // Generous, because this is not the assertion — it only keeps a hang from taking the suite
        // with it, exactly as above.
        let took = answer
            .recv_timeout(Duration::from_secs(2))
            .expect("closing an ordinary console returned");
        let _ = closing.join();
        drop(output);

        assert!(
            took < READER_GIVE_UP,
            "closing a console nobody outlived took {took:?}, which is at least the \
             {READER_GIVE_UP:?} reserved for abandoning a reader: the deadline is being paid on \
             every close, and a tab pays it once per pane inside the interface pass"
        );
    }
}
