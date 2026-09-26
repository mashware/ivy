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
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{
    Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError, channel, sync_channel,
};
use std::sync::{Arc, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use portable_pty::{Child, CommandBuilder, MasterPty, native_pty_system};

#[cfg(target_os = "linux")]
mod nonblocking;
mod screen;
mod view;

pub use screen::{Cell, Half, MouseReporting, SCROLLBACK, Screen, Scroll};
pub use view::{
    CellWidth, CombiningSpan, Cursor, CursorShape, Link, LinkSource, TermColor, Underline, View,
    ViewCell,
};

/// Error of any operation on the pseudoterminal.
///
/// An opaque error is used instead of a bespoke enum because there are not yet variants that the
/// caller could handle differently. When there are, it is swapped in without breaking the signature.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub type Result<T> = std::result::Result<T, Error>;

/// How a console's reading ended.
///
/// **It exists because every ending used to look the same from outside.** What reaches whoever hosts
/// this crate is the output channel closing, and a program that finished, a read that failed and a
/// reader that died all close it identically — so the host could tell that a console was over and
/// never why. This is that «why» for the endings a thread lives to report, and it is reported **by
/// the thread that ended**, which is what keeps it a fact rather than a guess: nothing outside is
/// asked to deduce a reason from a closed channel. **A reader that dies reports it too**, since the
/// unwinding is caught where the reading is recorded — it used to leave no reason at all, which made
/// the accessor's «nothing recorded» cover a console that was over.
///
/// **`EndedWell` is not «no error», it is «this is what an ending is supposed to look like».** A
/// program that exits and a console somebody closes both take that arm, and neither is worth saying
/// anything about. The name is kept over the shorter one-word alternative because this is the word
/// the decision
/// about what closing means is written in, and one vocabulary for both is worth more than one fewer
/// syllable at the point of use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// The program finished, or this console is being closed.
    ///
    /// **Both of them, deliberately.** No consumer acts on the difference, and the one place it
    /// could be told apart —whether the stop flag was up— is already what decides between this arm
    /// and [`Ending::NobodyListening`].
    EndedWell,
    /// The read failed.
    ///
    /// **`kind` alone is not enough, and that was measured rather than assumed.** The standard
    /// library flattens most of what a pseudoterminal can answer into one bucket whose whole printed
    /// form is the word «uncategorized», while the operating system's own number survives: errno 5,
    /// 70 and 75 all come out as that bucket and all keep their number. So both travel — the kind
    /// for anybody who wants to branch, the number for the sentence somebody writes.
    ///
    /// **And the obvious example is the wrong one, which a review had to point out.** `EIO` never
    /// reaches this arm: the pseudoterminal library maps it to end of file, on both platforms. What
    /// arrives here is whatever a device or a driver answers that `std` has no name for, and that is
    /// what the number is for.
    ///
    /// **Format it, do not branch on it.** The kind comes from a standard-library enum that gains
    /// variants between releases, so a `match` over it here is one that stops being exhaustive
    /// without this crate changing.
    Failed {
        kind: std::io::ErrorKind,
        os_error: Option<i32>,
    },
    /// The thread reading this console died on the way.
    ///
    /// **It says nothing about the program**, which may be perfectly alive and writing: what ended is
    /// the reading. And catching it is what makes this a reason at all — an uncaught panic reaches
    /// the host as a channel that closed with nothing recorded, which is indistinguishable from a
    /// console still working.
    ///
    /// **The runtime already says what and where.** Its own hook prints the panic with its file and
    /// line, so nothing here repeats that; this variant exists so the *window* stops reading a death
    /// as a clean end.
    Panicked,
    /// The reading stopped because nothing was left to deliver to, and nobody had asked for that.
    ///
    /// **This is not a console being closed, and telling the two apart is the whole reason this
    /// variant exists.** Closing raises the stop flag first, so it comes out as
    /// [`Ending::EndedWell`]. Reaching this arm means the other end of the output channel went away
    /// on its own — the program inside may well be alive and writing — which is exactly the silence
    /// this vocabulary was built to report, and calling it a clean end was the lie it exists against.
    NobodyListening,
}

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
    /// Always there until closing takes it: a child that refused the hang-up is handed to a thread
    /// of its own to be waited for (**D56**), so closing has to be able to give it away.
    child: Option<Box<dyn Child + Send + Sync>>,
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
    /// **Dropping this is what ends the writer's wait for room**: the thread waits on the other end
    /// of this pipe as well as on the master, and a pipe whose writing end is gone reads as ready.
    #[cfg(target_os = "linux")]
    knock: Option<std::io::PipeWriter>,
    reader: Option<JoinHandle<()>>,
    /// Goes quiet when the reader thread ends, which is how closing waits for it **with a deadline**.
    ///
    /// **Nothing is ever sent on it.** The thread owns the other half and never uses it; what says
    /// «the thread is gone» is that half being dropped, which turns a `recv` here into
    /// `Disconnected` at once. A channel is used and not the `JoinHandle` because `join` has no
    /// timed form, and waiting for that thread without a deadline is the whole of this defect.
    reader_ended: Receiver<()>,
    /// How the reading ended, written once by the reader thread and readable from here afterwards.
    ///
    /// **A cell and not a channel, because the answer has to survive being asked more than once.**
    /// Receiving consumes, so whoever asked second would find nothing — and this is meant to be
    /// asked by a painting loop, which asks about the same console again and again. It is the shape
    /// [`Pty::backlog`] already uses for the other direction: shared state a thread writes and the
    /// host reads on demand.
    ///
    /// **The cell keeps the first write, not the best one.** That is the right way round here —
    /// one writer, one call site — but it is worth knowing before adding a second: the more
    /// specific reason would be the one silently dropped.
    ending: Arc<OnceLock<Ending>>,
    /// What pulls the reader thread out of a wait for room in the channel when it has to close.
    ///
    /// Without this, destroying the `Pty` while the channel is full **hangs the close**: the thread
    /// waits for a slot that would only appear if someone drained it, and the `join` below waits for
    /// the thread.
    stop: Arc<AtomicBool>,
    /// How closing signals the child: `kill(2)`, except in the tests that forge a refusal.
    ///
    /// **A test cannot make a child it may not signal**: that takes a setuid program switching every
    /// user identity, and an authenticated one. So the refusal is what is forged, and everything
    /// after it — the wait, the thread, the line — runs for real against a real program.
    #[cfg(unix)]
    send_signal: fn(libc::pid_t, libc::c_int) -> std::io::Result<()>,
}

/// What has been handed over for the program to read, and how much of it has not gone in yet.
///
/// **It exists because the fix for the blocking write traded a visible failure for an invisible
/// one.** With the write on a thread of its own the window no longer freezes — but a program that
/// stops reading for good leaves that thread waiting for room to write a single chunk, and
/// everything typed afterwards queues behind it, `Ctrl+C` included. `Pty::write` keeps answering
/// «taken», the console keeps repainting, and it simply stops listening.
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
/// what the program's dying costs and not a millisecond more.
///
/// **It is spent per console, not per gesture**, and that is the number to quote: closing a tab drops
/// every pane it had, in series, inside the same interface pass. Four consoles each holding a
/// survivor is four times this, on top of four programs dying.
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

/// How long a program has, after the hang-up that closing its console sends, before it is killed.
///
/// **It is the grace `portable-pty`'s own `kill` gives, kept on purpose**: four looks 50 ms apart
/// after the first. A program that catches the hang-up to save its state is owed that time, and no
/// measurement here says how much of it any real program needs — so the number stays the library's
/// rather than one invented here. What this crate stopped taking from the library is the grid, not
/// the grace: see [`HANGUP_CHECK_EVERY`].
#[cfg(unix)]
const HANGUP_GRACE: Duration = Duration::from_millis(200);

/// How often closing looks whether the hung-up program has gone.
///
/// **The library looks every 50 ms, and that step was the whole cost of closing a console**: a shell
/// dies one or two milliseconds after the hang-up, misses the first look, and the window waited out
/// the step — once per console, inside the interface pass. At this pace the close costs about what
/// the dying costs, and the looks are `try_wait`, a non-blocking system call.
#[cfg(unix)]
const HANGUP_CHECK_EVERY: Duration = Duration::from_millis(1);

/// How many reads fit in the channel before the reader thread stops reading.
///
/// **This number is the terminal's brake, not a performance tuning knob.** While the channel has
/// room, the reader thread drains the pseudoterminal; when it fills up, it stops reading, the
/// kernel's buffer fills up in turn —very little fits: some 20 KiB measured on Linux— and the program
/// inside **blocks on write**, which is the only brake a terminal truly has.
///
/// Eight reads of [`READ_SIZE`] are 64 KiB: enough that a normal burst does not hit the ceiling, and
/// little enough that what is held back does not matter. The consumer can accumulate on its own all it
/// wants, but if it does so without limit it nullifies this brake — the ceiling only brakes if whoever
/// drains stops draining.
const READS_IN_FLIGHT: usize = 8;

/// How much one read takes at most.
///
/// **It has a name because the brake's arithmetic is written in terms of it and lives elsewhere.**
/// [`READS_IN_FLIGHT`] reasons in KiB from the product of the two; when this was a bare literal
/// inside the reading loop, halving it would have quietly halved that brake.
///
/// **Two guards, because one of them only looks like a guard.** A validation round found the buffer
/// could be set to half this constant with the whole suite green, and tied a delivered chunk's
/// length to the name — which catches that. But **a review then found both sides of that test move
/// with the constant**: change this number and the test follows it, green, while the brake halves.
/// So the product is asserted against a **literal** as well, and that is the guard that cannot
/// follow a value. The channel's own bounded test reasons about the product in prose and asserts a
/// figure a thousandfold larger — it sees neither move.
const READ_SIZE: usize = 8192;

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

        // **Every descriptor the console needs is made before the program exists**, because each
        // of these can fail — a process out of descriptors is realistic with many consoles open —
        // and a failure after the spawn would drop a child nobody hangs up or waits for.
        let reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        #[cfg(target_os = "linux")]
        let (reader, room, knock) = {
            let master = pair
                .master
                .as_raw_fd()
                .ok_or("the pseudoterminal has no descriptor to wait on")?;
            nonblocking::set_nonblocking(master)?;
            let (knocked, knock) = std::io::pipe()?;
            let reader = nonblocking::Readable {
                reader,
                master: nonblocking::duplicate(master)?,
            };
            let room = nonblocking::Writable {
                master: nonblocking::duplicate(master)?,
                knocked,
            };
            (reader, room, knock)
        };

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

        // **A thread of its own for writing, and it is not symmetry for its own sake.** Writing to a
        // pseudoterminal whose program is not reading waits until somebody reads, and there are
        // cases measured in this repository where that is never. Here the waiting is done by a
        // thread that paints nothing and reads nothing, so the window keeps answering and the bytes
        // go in when the program comes back for them.
        let (to_write, waiting) = channel::<Vec<u8>>();
        let stop = Arc::new(AtomicBool::new(false));
        let input = Arc::new(Input::default());
        let input_for_thread = Arc::clone(&input);
        let stop_for_writer = Arc::clone(&stop);
        let writer = thread::spawn(move || {
            // Bounded: the only sender lives in `Pty`, and its destructor drops it right after
            // raising the stop flag. Off Linux what this thread may never leave is the
            // `write_all` in `write_chunk`, which is why it is not joined.
            while let Ok(bytes) = waiting.recv() {
                // **Abandoned on close, not drained into a pseudoterminal that is being killed.**
                // The sender is dropped first and the child killed right after, so without this the
                // last chunks would be written into a program that is already going away.
                if stop_for_writer.load(Ordering::Relaxed) {
                    break;
                }
                #[cfg(target_os = "linux")]
                let outcome = write_chunk(&mut writer, &bytes, &room);
                #[cfg(not(target_os = "linux"))]
                let outcome = write_chunk(&mut writer, &bytes);
                let failure = match outcome {
                    Ok(()) => None,
                    // What is owed stays owed: the console it was for is going away, and nobody is
                    // left to read the number.
                    Err(Unwritten::Closing) => break,
                    Err(Unwritten::Failed(failure)) => Some(failure),
                };
                input_for_thread.settle(bytes.len(), failure.as_ref());
                if failure.is_some() {
                    // **There is nobody left to write to** —the program died, the pseudoterminal is
                    // gone— and retrying would spin. What was queued behind this is **not** thrown
                    // away in silence: it stays counted as owed, and [`Pty::backlog`] is what
                    // says so out loud.
                    break;
                }
            }
            // `portable-pty`'s writer is dropped here, and its destructor writes a newline and the
            // end-of-file character into the master. On Linux, with the master non-blocking, a full
            // buffer makes that write fail at once instead of waiting for room for ever.
        });

        let (tx, output) = sync_channel(READS_IN_FLIGHT);
        let stop_for_thread = Arc::clone(&stop);
        let (ended, reader_ended) = channel::<()>();
        let ending = Arc::new(OnceLock::new());
        let ending_for_thread = Arc::clone(&ending);
        let reader = thread::spawn(move || {
            // Moved in and never used: what closing reads is this being dropped when the thread ends.
            let _ended = ended;
            feed_and_record(reader, &tx, &stop_for_thread, &ending_for_thread);
        });

        Ok((
            Self {
                child: Some(child),
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
                #[cfg(target_os = "linux")]
                knock: Some(knock),
                reader: Some(reader),
                reader_ended,
                ending,
                stop,
                #[cfg(unix)]
                send_signal,
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

    /// How the reading ended, or nothing while nothing has been recorded.
    ///
    /// **`None` is «nobody wrote a reason», and it now covers one case rather than two.** A reader
    /// that dies used to be the other one — the unwinding never reached the line that records the
    /// reason, so the channel said «over» and this said nothing — and that is caught where the
    /// reading is recorded, so such a thread reports [`Ending::Panicked`] like any other ending.
    ///
    /// What remains is **the reader abandoned at the closing deadline**: [`Pty::drop`] lets that
    /// thread go when a survivor holds the pseudoterminal open, and it may record a reason
    /// afterwards — into a cell nobody is left to read, because the console it belonged to is gone.
    ///
    /// So `Some` may be read as «this is what happened», and `None` must not be read as «it is
    /// fine»: what it means is that nothing was recorded, which for a console that is over now means
    /// one specific thing and not two.
    pub fn ending(&self) -> Option<Ending> {
        self.ending.get().copied()
    }
}

/// Reads until the reading ends and records **that** ending in the cell the host reads.
///
/// **The two halves are one function because nothing was binding them together.** A review found it
/// by mutation: with the recording hard-coded to one value, discarding everything the loop worked
/// out, all eighty tests stayed green — every failure was asserted on the loop directly and the only
/// tests that read the cell expected the value the mutation happened to write. This is the project's
/// own first test disguise, asserting where a value is recorded instead of the effect it has, and
/// the seam that closes it is a function a test can call with a source that fails.
///
/// **The recording goes before this returns, and the caller's channel is let go after it, on
/// purpose.** What tells the host a console is over is that channel being dropped, so recording
/// first is what makes the reason readable by anybody who noticed.
///
/// **What a test can hold of that ordering, measured rather than claimed**: putting the recording
/// after the channel is released leaves the suite green, because the gap is a few instructions and
/// the observer only looks once a blocking receive has already returned. The same disorder with a
/// wait in it turns that test red. So what is guarded is a gap somebody could open by putting work
/// between the two — the way this gets broken in practice — and not the instruction order itself.
fn feed_and_record(
    reader: impl Read,
    channel: &SyncSender<Vec<u8>>,
    stop: &AtomicBool,
    ending: &OnceLock<Ending>,
) {
    // **`AssertUnwindSafe` is required and is not a formality.** The reader is a
    // `Box<dyn Read + Send>`, and a boxed trait object carries no unwind-safety bound, so this does
    // not compile without the assertion — while the flag and the channel are unwind-safe on their
    // own. What is claimed is that a channel and a reader which a panic may have left half-used are
    // still sound to observe afterwards: the standard library's channels do not poison, and the
    // reader is dropped immediately below. Whoever widens this closure inherits that claim, which is
    // why it is written here and not at the call site.
    let how =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| feed(reader, channel, stop)))
            .unwrap_or(Ending::Panicked);
    let _ = ending.set(how);
}

/// Reads the pseudoterminal into the channel until there is nothing more to read, and answers how
/// that ended.
///
/// **Not called `pump`, and the collision is avoided on purpose.** In this workspace pumping means
/// *draining* that channel with a budget per frame —the pane's own method, and the number D13's
/// reports publish— and this is the end that fills it. One word for both directions is how a
/// vocabulary starts lying.
///
/// **It takes the source instead of owning one, and that is not tidiness: it is the only way this
/// loop can be tested.** What `Pty::spawn` hands it is a `Box<dyn Read + Send>` that
/// `portable-pty` builds over the master's descriptor, and nothing a test does to it makes it fail,
/// come back short, or ask to be retried — so for as long as the loop lived inside the thread,
/// every path through it except «bytes arrived» was unreachable from the suite. With the source as
/// a parameter a test hands it whatever it needs to prove.
fn feed(mut reader: impl Read, channel: &SyncSender<Vec<u8>>, stop: &AtomicBool) -> Ending {
    let mut buf = [0u8; READ_SIZE];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return Ending::EndedWell,
            Ok(n) => match deliver(channel, buf[..n].to_vec(), stop) {
                Ok(()) => {}
                // Somebody is closing this console: the flag went up before anything else happened,
                // which is the order `Drop for Pty` guarantees.
                Err(Undelivered::Closing) => return Ending::EndedWell,
                // Nobody asked, and there is nobody left to deliver to. **The distinction this arm
                // makes is the one a review had to point out**: the receiving end lives in another
                // thread, and that thread dying is not this console ending — the program inside can
                // be alive and writing into a pseudoterminal whose output now goes nowhere.
                Err(Undelivered::NobodyListening) => return Ending::NobodyListening,
            },
            // **An interrupted read is not the end of anybody's output**, and until now it ended the
            // console for good. `io::Read`'s own contract says a reader may answer this for no
            // reason and that the call should simply be made again; nothing below retries it — the
            // pseudoterminal library only absorbs the one errno that means end of file — so the
            // reader thread died and the program inside kept writing into a console nobody was
            // draining, until the kernel's buffer filled and it blocked.
            //
            // **The stop flag is consulted here and that is not decoration.** This is a wait, and
            // the rule this crate has already been bitten by twice is that a thread which waits is
            // not looking at what would wake it: without this, a source answering «interrupted»
            // back to back is a loop at full speed that closing cannot reach, and `Drop` would
            // abandon a **spinning** thread for the life of the process.
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                if stop.load(Ordering::Relaxed) {
                    return Ending::EndedWell;
                }
            }
            Err(e) => {
                return Ending::Failed {
                    kind: e.kind(),
                    os_error: e.raw_os_error(),
                };
            }
        }
    }
}

/// Why a chunk did not get delivered.
///
/// **The two are not the same event, and answering one `Err` for both was a defect.** A review
/// found it: with a single failure the caller could only guess, and it guessed «somebody is
/// closing» — which turns a console whose reader is gone while its program still writes into «the
/// program finished», the exact silence this vocabulary exists to report.
enum Undelivered {
    /// The stop flag is up: whoever owns this console is closing it.
    Closing,
    /// The other end of the channel is gone and nobody raised the flag.
    NobodyListening,
}

/// Leaves a chunk in the channel, waiting for room, and gives up if it has to close.
///
/// The `Err` says **which** of the two reasons it was, because they mean opposite things to whoever
/// reports what happened to this console. See [`Undelivered`].
///
/// **The wait is the brake.** While it lasts, this thread does not read from the pseudoterminal, so
/// the kernel's buffer fills up and the program inside blocks on write. Nothing is discarded: the
/// bytes stay where they already were, in the program that wrote them.
fn deliver(
    channel: &SyncSender<Vec<u8>>,
    chunk: Vec<u8>,
    stop: &AtomicBool,
) -> std::result::Result<(), Undelivered> {
    let mut pending = chunk;
    loop {
        // It is checked before trying and not only after failing: if the close arrives right when
        // the channel has room, delivering one more chunk breaks nothing, but checking first makes
        // the exit from the loop not depend on the state of the channel.
        //
        // **And it is what tells a close apart from a reader that vanished**, which is why the order
        // is load-bearing rather than merely tidy: [`Pty::drop`] raises this flag before it drops
        // anything.
        //
        // **What this crate cannot enforce, said here rather than assumed**: the receiving end is
        // owned by whoever hosts this console, so «the flag goes up before that end disappears» also
        // depends on the host dropping the console before the thing holding the receiver. A host
        // that reversed that order would have every ordinary close recorded as nobody listening,
        // and nothing here could tell. Whoever first reads that distinction is where it has to be
        // pinned by a test.
        if stop.load(Ordering::Relaxed) {
            return Err(Undelivered::Closing);
        }
        match channel.try_send(pending) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(returned)) => {
                pending = returned;
                thread::sleep(RETRY);
            }
            Err(TrySendError::Disconnected(_)) => return Err(Undelivered::NobodyListening),
        }
    }
}

/// Why a chunk handed over for writing did not go in whole.
enum Unwritten {
    /// Whoever owns the console is closing it.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Closing,
    /// Nothing more can go in: the program's side is gone, or the write failed.
    Failed(std::io::Error),
}

/// Writes one chunk, waiting for room **where closing can reach the wait**.
///
/// The master is non-blocking (see the `nonblocking` module), so a full buffer answers «would block»
/// here instead of parking this thread inside the kernel, where — measured — the program dying does not
/// end the wait.
#[cfg(target_os = "linux")]
fn write_chunk(
    writer: &mut dyn Write,
    bytes: &[u8],
    room: &nonblocking::Writable,
) -> std::result::Result<(), Unwritten> {
    use std::io::ErrorKind;

    let mut rest = bytes;
    while !rest.is_empty() {
        match writer.write(rest) {
            Ok(0) => return Err(Unwritten::Failed(ErrorKind::WriteZero.into())),
            Ok(written) => rest = &rest[written..],
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            // Bounded: by room, by the console closing — `Drop for Pty` drops the knock — or by the
            // program's side of the pseudoterminal closing, which answers as `Room::Gone`.
            Err(e) if e.kind() == ErrorKind::WouldBlock => match room.wait() {
                Ok(nonblocking::Room::Ready) => {}
                Ok(nonblocking::Room::Closing) => return Err(Unwritten::Closing),
                Ok(nonblocking::Room::Gone) => {
                    return Err(Unwritten::Failed(std::io::Error::new(
                        ErrorKind::BrokenPipe,
                        "the program's side of the pseudoterminal is closed",
                    )));
                }
                Err(failure) => return Err(Unwritten::Failed(failure)),
            },
            Err(e) => return Err(Unwritten::Failed(e)),
        }
    }
    // **The `flush` travels with the write, error included**, as it did before the write was split:
    // a no-op for this writer, kept so this loop answers the same thing on every platform.
    writer.flush().map_err(Unwritten::Failed)
}

/// Writes one chunk, with the blocking write: a program that stops reading still parks this thread
/// inside it. **Off Linux on purpose, and not because the leak is absent there.** Windows's ConPTY is a
/// pipe this crate has not measured (F2.7), and macOS's `poll` is known not to wait on a
/// pseudoterminal at all — the filedescriptor crate beneath `portable-pty` switches to `select`
/// there for that reason — so the Linux shape would spin instead of waiting.
#[cfg(not(target_os = "linux"))]
fn write_chunk(writer: &mut dyn Write, bytes: &[u8]) -> std::result::Result<(), Unwritten> {
    writer
        .write_all(bytes)
        // **The `flush` travels with the write, error included.** It is a no-op on Windows's ConPTY
        // too, but discarding its `Result` would leave the loop happily writing into something that
        // is not delivering, on the one platform where it could ever say so.
        .and_then(|()| writer.flush())
        .map_err(Unwritten::Failed)
}

/// Hangs up the program and gives it [`HANGUP_GRACE`] to go before killing it.
///
/// **It is `portable-pty`'s own `kill` with a finer grid, and not a call to it**: that one sleeps
/// 50 ms between looks, and a program that dies a millisecond after the hang-up paid the whole step
/// on the interface pass. The signals are the same two, sent to the child's pid and never its group
/// — who else closing a console should reach is an open question this does not answer.
///
/// **Signalling by pid is safe only while the child is unreaped**, which is why every signal here
/// follows a `try_wait` that said so: a reaped pid can be handed to somebody else's process.
///
/// `send` is `kill(2)` everywhere but in the tests that forge a refusal.
#[cfg(unix)]
fn hang_up(
    child: &mut (dyn Child + Send + Sync),
    send: fn(libc::pid_t, libc::c_int) -> std::io::Result<()>,
) -> HangUp {
    // **Only a positive pid is signalled.** Zero would reach this application's own process group
    // and a negative one somebody's group. Without such a pid closing does what it did before this
    // crate signalled by itself — the library's `kill`, grid and all, and its refusal unseen: the
    // library does not tell a refused signal from any other failure. Reasoned unreachable here, where
    // a spawned child always has a pid and a pid always fits.
    let Some(pid) = child
        .process_id()
        .and_then(|pid| libc::pid_t::try_from(pid).ok())
        .filter(|pid| *pid > 0)
    else {
        let _ = child.kill();
        return HangUp::Waitable;
    };
    // **A refusal is any failure but «no such process»**: that one means the child died between the
    // look and the signal, and it is waitable at once. Anything else leaves a live program this
    // process cannot end — see D56.
    let signal = |number| match send(pid, number) {
        Err(refused) if refused.raw_os_error() != Some(libc::ESRCH) => Err(refused),
        _ => Ok(()),
    };
    // An error from `try_wait` counts as gone: `waitpid` failing means there is no child of ours to
    // wait for, so the `wait` that follows fails at once instead of blocking — and signalling a pid
    // that is not known to be ours is the one thing this must never do.
    if !matches!(child.try_wait(), Ok(None)) {
        return HangUp::Waitable;
    }
    if let Err(refused) = signal(libc::SIGHUP) {
        return HangUp::Refused(refused);
    }
    let deadline = Instant::now() + HANGUP_GRACE;
    while Instant::now() < deadline {
        thread::sleep(HANGUP_CHECK_EVERY);
        if !matches!(child.try_wait(), Ok(None)) {
            return HangUp::Waitable;
        }
    }
    match signal(libc::SIGKILL) {
        Ok(()) => HangUp::Waitable,
        Err(refused) => HangUp::Refused(refused),
    }
}

/// `kill(2)`, with its failure as the operating system's error.
#[cfg(unix)]
fn send_signal(pid: libc::pid_t, number: libc::c_int) -> std::io::Result<()> {
    // SAFETY: `kill` takes plain integers and touches no memory of this process.
    if unsafe { libc::kill(pid, number) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Elsewhere the library's `kill` has no grid to pay: on Windows it is `TerminateProcess`, at once.
/// Whether it can be refused there — an elevated program — and what that leaves is not known yet:
/// its failure is not looked at, so a refusal would be waited for as Unix's was before D56.
#[cfg(not(unix))]
fn hang_up(child: &mut (dyn Child + Send + Sync)) -> HangUp {
    let _ = child.kill();
    HangUp::Waitable
}

/// What hanging a program up left behind.
enum HangUp {
    /// Gone, sure to go at once, or a refusal this crate cannot see (the library's `kill`, and
    /// Windows): waiting for it is bounded as far as this knows.
    Waitable,
    /// The system refused the signal and the program is still running. Nothing this process can
    /// send will end it, so waiting for it is waiting for as long as it chooses to live.
    #[cfg_attr(not(unix), allow(dead_code))]
    Refused(std::io::Error),
}

impl Drop for Pty {
    fn drop(&mut self) {
        // First the flag: with the channel full, the thread is not inside `read` but waiting for
        // room, and neither killing the child nor closing the pseudoterminal pulls it out of there.
        // Without this, the `join` below waits forever and **closing the window hangs** — confirmed.
        self.stop.store(true, Ordering::Relaxed);
        // **The sender is dropped, the knock is dropped, and the writer thread is NEVER joined.**
        // Dropping the sender ends the loop between chunks; dropping the knock ends a wait for room
        // in the middle of one. The flag is what the thread reads to abandon what is still queued
        // instead of writing it into a pseudoterminal about to be killed.
        //
        // **Killing the child does NOT end a write waiting for room, which is measured and not
        // assumed**: on Linux the program's side closes, a read answers `EIO`, and a write parked in
        // the kernel keeps waiting — fifteen seconds and more after the drop, for the life of the
        // process. That is why, on Linux, the write waits outside the write call, on the knock as
        // well as on the master (D29, amended). **Not joined anyway**, because off Linux the write is
        // still the blocking one, and waiting for it would hang closing the console there.
        //
        // **What is queued at this point is abandoned, and that is an exception to what `write`
        // promises.** It is stated here rather than left to be discovered: closing a console throws
        // away what had been handed over and not yet written. There is nowhere for those bytes to go
        // — the program they were for is being killed on the next line.
        drop(self.to_write.take());
        #[cfg(target_os = "linux")]
        drop(self.knock.take());
        // Not joined, on purpose — see above. Taking it here says so where somebody looking for the
        // `join` will find it.
        let _ = self.writer.take();
        // And then the usual order: killing the child is what closes its end of the
        // pseudoterminal and triggers the end of file in the reader thread. Waiting for the thread
        // before that would hang it, because it would still be blocked inside `read`.
        if let Some(mut child) = self.child.take() {
            #[cfg(unix)]
            let left = hang_up(child.as_mut(), self.send_signal);
            #[cfg(not(unix))]
            let left = hang_up(child.as_mut());
            match left {
                HangUp::Waitable => {
                    // Bounded: on Unix, after its grace the hang-up falls through to a real
                    // `SIGKILL`, which nobody holds — not even a stopped process, the shape that
                    // would otherwise never be reaped. A program in uninterruptible sleep (a hung
                    // network mount) dies only when its system call returns, and until then this
                    // waits with it. When `try_wait` failed instead, there is no
                    // child of ours left to wait for and this fails at once. **Not on Windows**,
                    // nor on the Unix path with no pid, reasoned unreachable: there a refused kill
                    // is not seen, and this waits for as long as the program lives (#273).
                    let _ = child.wait();
                }
                // **Left running, and waited for somewhere that paints nothing** (D56). Measured:
                // waiting here froze the window for the program's whole life, and nothing this
                // process can send shortens it. Somebody still has to wait, or the program stays a
                // zombie once it ends; a thread parked in `wait` costs one thread per such console,
                // for as long as its program lives. If the thread cannot even be created the child
                // is dropped unwaited, and that zombie is the price of not freezing.
                HangUp::Refused(refused) => {
                    use std::io::Write;

                    let pid = child.process_id();
                    // Named so a leak audit can tell these apart; `wait` needs no stack to speak of.
                    let _ = thread::Builder::new()
                        .name("left-running".to_owned())
                        .stack_size(64 * 1024)
                        .spawn(move || {
                            // Abandoned on purpose: this thread is never joined, and it waits for
                            // as long as a program nothing here can end chooses to live (D56).
                            let _ = child.wait();
                        });
                    // **Not `eprintln!`**, which panics when standard error is a closed pipe
                    // (`agent-terminal | head`) — and a panic here is inside the interface pass,
                    // or an abort if this drop runs while unwinding.
                    //
                    // **"May", because this is written before the last copy of the master closes.**
                    // That close makes the kernel hang the terminal up and signal the program with
                    // an authority no refusal applies to, so most programs end a moment later. One
                    // that ignores or handles the hang-up does not, and nor does any while a copy of
                    // the master stays open — an abandoned reader holds one (D56). No condition is
                    // named in the line because this is not placed to know which of those holds.
                    let _ = writeln!(
                        std::io::stderr().lock(),
                        "a console was closed and the system refused the signal to end its program \
                         (pid {}): {refused}. It may keep running",
                        pid.map_or_else(|| "unknown".to_owned(), |pid| pid.to_string())
                    );
                }
            }
        }
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
                // gone well — but a reader that panicked no longer looks like this: the unwinding is
                // caught where the reading is recorded, so such a thread ends normally and its
                // reason is readable. What the `join` here still discards is a panic from anywhere
                // in that closure **outside** the reading itself, which is why it stays a `let _`
                // rather than becoming a claim.
                Err(RecvTimeoutError::Disconnected) => {
                    // Bounded: the channel only disconnects once the thread has ended.
                    let _ = reader.join();
                }
                // **Abandoned on purpose, and it costs a thread and a descriptor for as long as the
                // survivor stays silent.** Dropping the handle detaches it; the thread is still
                // waiting for input, but the stop flag above is already raised, so **the survivor's
                // next byte ends it** — the read returns, the delivery sees the flag and the thread
                // leaves. A survivor that never writes again holds them until the process ends. That
                // is the trade the writer thread still makes off Linux (D29), and a better one than
                // the window freezing for ever, which is what the alternative was.
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

    /// A source of bytes that answers exactly what a test needs, in the order it needs it.
    ///
    /// **This is what the extraction bought.** The reader `Pty::spawn` hands to [`feed`] is
    /// `portable-pty`'s, over a real descriptor: no test can make it fail, and every path through
    /// the loop except «bytes arrived» was out of reach of the suite. Running out of script is end
    /// of file, so a test that wants a failure has to put it there.
    ///
    /// **It answers a short read when the chunk does not fit**, and that is not defensive tidying: a
    /// real `Read` may hand back fewer bytes than it was offered, and clamping instead of panicking
    /// is what puts that shape within reach of a test at all. A review found this copying the whole
    /// chunk regardless of the buffer, which panics at 9 000 bytes against 8 192 — and a second
    /// review found the clamp **discarding** the tail, which is a fake that lies about the shape it
    /// claims to model. The tail is pushed back.
    struct Scripted(std::collections::VecDeque<std::io::Result<Vec<u8>>>);

    impl Scripted {
        fn of(turns: Vec<std::io::Result<Vec<u8>>>) -> Self {
            Self(turns.into())
        }
    }

    impl Read for Scripted {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.pop_front() {
                Some(Ok(bytes)) => {
                    let fits = bytes.len().min(buf.len());
                    buf[..fits].copy_from_slice(&bytes[..fits]);
                    // **The tail goes back, which is what makes this a short read and not a lossy
                    // one.** A review caught the first version discarding it: a real `Read` that
                    // answers fewer bytes than it was offered keeps the rest for the next call, so a
                    // test scripting more than a buffer's worth in one turn would have lost the
                    // excess and failed on content with no hint why.
                    if fits < bytes.len() {
                        self.0.push_front(Ok(bytes[fits..].to_vec()));
                    }
                    Ok(fits)
                }
                Some(Err(e)) => Err(e),
                None => Ok(0),
            }
        }
    }

    /// A turn no correct loop ever reaches, put after the one under test.
    ///
    /// **Without it, three of these tests asserted the value the defect produces.** A one-turn
    /// script that ends the loop and a one-turn script the loop *ignores* finish the same way: the
    /// next read runs out of script, answers end of file, and hands back the very
    /// `Ending::EndedWell` the assertion was looking for. Putting an unmistakable failure behind the
    /// turn under test is what turns «it returned this» into «it stopped there».
    fn never_reached() -> std::io::Result<Vec<u8>> {
        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    }

    /// **The brake's own number, against a literal, because every other guard follows the constant.**
    ///
    /// The test that ties a delivered chunk to `READ_SIZE` catches the buffer being halved and
    /// **not** the constant being halved: both of its sides move together, so it stays green while
    /// the ceiling this terminal rests on goes from 64 KiB to 32. **D8** chose that figure with
    /// measurements behind it and two docblocks reason in KiB from it, so it is the figure and not
    /// the expression that has to be pinned. This is the only assertion in the crate a rename or a
    /// new value cannot follow.
    #[test]
    fn the_brake_is_still_the_sixty_four_kibibytes_the_decision_chose() {
        assert_eq!(
            READ_SIZE * READS_IN_FLIGHT,
            64 * 1024,
            "the reader's ceiling is no longer 64 KiB: {} reads of {READ_SIZE} bytes. Whoever \
             changed it owes D8 a new measurement, because the brake is what stops a program \
             dumping without bound",
            READS_IN_FLIGHT
        );
    }

    #[test]
    fn a_reading_that_runs_out_of_source_ended_well_and_delivered_what_it_had() {
        let (tx, rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);

        let how = feed(Scripted::of(vec![Ok(b"hola".to_vec())]), &tx, &stop);

        assert_eq!(how, Ending::EndedWell);
        drop(tx);
        // **Exactly, not «contains».** A loop that ignored the read's length and delivered its whole
        // buffer would satisfy a substring check with the four letters followed by 8 188 zero bytes,
        // and the only test in this crate that catches that today goes through a real
        // pseudoterminal.
        assert_eq!(drain_until_closed(&rx), b"hola");
    }

    #[test]
    fn an_interrupted_read_is_retried_and_says_nothing() {
        let (tx, rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);
        let source = Scripted::of(vec![
            Ok(b"before".to_vec()),
            Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
            Ok(b"after".to_vec()),
        ]);

        let how = feed(source, &tx, &stop);

        // **Both halves, because either alone is satisfied by the defect.** That it ended well says
        // the interruption was not reported; that «after» arrived says the loop went back for it
        // instead of stopping. Before this change the same script ended as a failure and «after»
        // never came.
        assert_eq!(how, Ending::EndedWell);
        drop(tx);
        assert_eq!(drain_until_closed(&rx), b"beforeafter");
    }

    #[test]
    fn a_source_interrupted_without_pause_still_lets_closing_finish() {
        // A source that answers «interrupted» for ever, which is the shape that turns a retry into a
        // spin. The flag is already up, as `Drop` raises it before anything else.
        struct Always;
        impl Read for Always {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
            }
        }
        let (tx, _rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(true);

        // **The assertion is that this returns at all.** Without the flag inside the retry it never
        // does, and the test hangs the suite instead of failing it — which is why the call is made on
        // a thread of its own with a deadline, the same shape this crate uses for every test about a
        // wait that might not end.
        let (told, when_it_ends) = channel();
        thread::spawn(move || {
            let how = feed(Always, &tx, &stop);
            let _ = told.send(how);
        });

        match when_it_ends.recv_timeout(Duration::from_secs(3)) {
            Ok(how) => assert_eq!(how, Ending::EndedWell),
            Err(_) => panic!(
                "the reading never came back: a source that answers «interrupted» without pause is \
                 a loop at full speed that closing cannot reach, and this console's thread would be \
                 abandoned spinning for the life of the process"
            ),
        }
    }

    #[test]
    fn a_read_that_fails_is_stamped_with_the_kind_it_failed_with() {
        let (tx, rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);
        let source = Scripted::of(vec![
            Ok(b"before".to_vec()),
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe)),
        ]);

        let how = feed(source, &tx, &stop);

        // **The kind is asserted, not merely «it failed».** A sentence about this names the kind, so
        // a reading that loses it on the way is a reading that cannot be spoken about.
        assert_eq!(
            how,
            Ending::Failed {
                kind: std::io::ErrorKind::BrokenPipe,
                os_error: None
            }
        );
        // And what arrived before the failure is not thrown away with it.
        drop(tx);
        assert_eq!(drain_until_closed(&rx), b"before");
    }

    #[test]
    fn a_failure_reports_its_own_kind_and_not_a_fixed_one() {
        let (tx, _rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);
        let source = Scripted::of(vec![Err(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        ))]);

        // A second kind, because one kind alone is satisfied by a constant: with `BrokenPipe`
        // hard-coded the test above passes and this one does not.
        assert_eq!(
            feed(source, &tx, &stop),
            Ending::Failed {
                kind: std::io::ErrorKind::PermissionDenied,
                os_error: None
            }
        );
    }

    #[test]
    fn a_failure_carries_the_systems_own_number_where_the_kind_says_nothing() {
        let (tx, _rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);
        // Errno 5 is a number the standard library has no name for: its kind comes out of the bucket
        // whose entire printed form is the word «uncategorized», so a sentence built from the kind
        // alone would say nothing at all. The number is what survives, and that is the whole
        // argument for the second field.
        //
        // **It is a synthetic error on purpose, and it is NOT the real path** — a review had to
        // correct the first version of this comment, which claimed it was. That errno is what a
        // pseudoterminal answers when the other end is gone, and the library below maps exactly that
        // one to end of file, so it reaches the ordinary arm and never this one. What reaches this
        // one is whatever else a device answers with no name of its own; five is simply a number
        // that reliably has none, which is what this test needs.
        let source = Scripted::of(vec![Err(std::io::Error::from_raw_os_error(5))]);

        let Ending::Failed { kind, os_error } = feed(source, &tx, &stop) else {
            panic!("a read that failed did not come back as a failure");
        };

        assert_eq!(os_error, Some(5));
        // And the kind really is the useless one, which is why the number is carried at all: if this
        // ever stops being true, the argument for the second field has changed.
        assert_eq!(format!("{kind:?}"), "Uncategorized");
    }

    #[test]
    fn a_receiver_that_vanished_with_nobody_closing_is_not_a_clean_end() {
        let (tx, rx) = sync_channel(READS_IN_FLIGHT);
        // **Nobody raised the flag**, and that is the whole subject: the receiving end lives in
        // another thread, and that thread dying is not this console ending. The program inside can
        // still be alive and writing into an output that now goes nowhere — the silence this
        // vocabulary exists to report, and the first version of this test asserted it was a clean
        // end.
        let stop = AtomicBool::new(false);
        drop(rx);

        let how = feed(
            Scripted::of(vec![Ok(b"into the void".to_vec()), never_reached()]),
            &tx,
            &stop,
        );

        assert_eq!(how, Ending::NobodyListening);
    }

    #[test]
    fn a_console_being_closed_mid_delivery_ended_well() {
        let (tx, rx) = sync_channel(READS_IN_FLIGHT);
        // The flag `Drop` raises, with the receiver still there — the ordinary close, where the flag
        // goes up before anything is dropped. It must not be reported as a failure, and must not be
        // confused with the test above, which is the same delivery failing for the opposite reason.
        let stop = AtomicBool::new(true);

        let how = feed(
            Scripted::of(vec![Ok(b"one last chunk".to_vec()), never_reached()]),
            &tx,
            &stop,
        );

        drop(rx);
        assert_eq!(how, Ending::EndedWell);
    }

    #[test]
    fn the_channel_closes_when_the_program_finishes() {
        let (_pty, output) =
            Pty::spawn("sh", &["-c", "exit 0"], Size::default()).expect("launch sh");

        drain_until_closed(&output);
    }

    /// A source that dies rather than answering.
    ///
    /// **Until the unwinding was caught it took the recording with it**, so the host was left with a
    /// closed channel and no reason — which is the same answer a console that is still working
    /// gives.
    struct Dies;

    impl Read for Dies {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("the thread reading this console died");
        }
    }

    /// Runs a reading that dies, with the runtime's own panic hook silenced for the duration.
    ///
    /// **Silenced so a passing suite does not read like a failing one**, and put back straight
    /// after: the hook is process-wide, and leaving it off would hide the next real panic in this
    /// binary.
    fn what_a_dying_reader_records() -> Option<Ending> {
        let (tx, _rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);
        let cell = OnceLock::new();
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        feed_and_record(Dies, &tx, &stop, &cell);
        std::panic::set_hook(hook);
        cell.get().copied()
    }

    #[test]
    fn a_reader_that_dies_on_the_way_reports_it_instead_of_leaving_nothing() {
        // **Asserted on the cell and not on the call**, because the point is not that the panic was
        // swallowed: it is that what the host reads about this console says the reading died.
        assert_eq!(what_a_dying_reader_records(), Some(Ending::Panicked));
    }

    #[test]
    fn a_console_whose_reader_died_is_not_reported_as_having_ended_well() {
        // The half that matters to whoever paints, and it is a separate question: catching a panic
        // and recording it as an ordinary end would satisfy the test above's shape while
        // reintroducing the whole defect.
        assert_ne!(what_a_dying_reader_records(), Some(Ending::EndedWell));
    }

    #[test]
    fn what_gets_recorded_is_what_the_reading_answered() {
        let (tx, _rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);
        let cell = OnceLock::new();
        let source = Scripted::of(vec![Err(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        ))]);

        feed_and_record(source, &tx, &stop, &cell);

        // **A failure, on purpose, and that is the whole point of this test.** Every other assertion
        // about a failure in this module is made on the reading itself; nothing was checking that
        // the value the host ends up reading is the one the reading produced. A review proved it by
        // recording a fixed «ended well» and watching all eighty tests stay green.
        assert_eq!(
            cell.get(),
            Some(&Ending::Failed {
                kind: std::io::ErrorKind::PermissionDenied,
                os_error: None
            })
        );
    }

    #[test]
    fn a_source_with_nothing_at_all_to_give_ended_well() {
        let (tx, _rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);

        // **A program that writes nothing, without depending on a shell to write nothing.** The
        // real-pseudoterminal test below happens to cover this today because this host's `sh` emits
        // nothing at all on `exit 0` — but that is the host's behaviour, not this crate's, and the
        // next milestone takes this code to two platforms where nobody has checked. A validation
        // round found the binding was incidental; this makes it independent of what any shell does.
        assert_eq!(feed(Scripted::of(vec![]), &tx, &stop), Ending::EndedWell);
    }

    #[test]
    fn a_program_that_exited_is_readable_as_such_through_the_real_path() {
        let (pty, output) =
            Pty::spawn("sh", &["-c", "exit 0"], Size::default()).expect("launch sh");

        // **What this binds and what it does not, said here rather than left to the name.** It binds
        // that after the host has seen the channel close — the only thing that tells it a console is
        // over — the accessor answers, and answers the right thing, through a real pseudoterminal
        // rather than a scripted source. It does **not** bind the stamp's ordering against the
        // channel being let go: reordering those two lines leaves this green, because the gap is a
        // few instructions and this observer only looks after a blocking receive has returned. An
        // earlier name for this test claimed the ordering, which is why it was renamed.
        drain_until_closed(&output);

        assert_eq!(pty.ending(), Some(Ending::EndedWell));
    }

    #[test]
    fn a_source_that_hands_back_less_than_it_was_offered_is_read_to_the_end() {
        let (tx, rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);
        // A real `Read` may answer with fewer bytes than the buffer offers, and until this loop took
        // its source as a parameter nothing could hand it one that does. Two short turns in a row,
        // so «it stopped after the first» would be visible.
        let source = Scripted::of(vec![Ok(b"one".to_vec()), Ok(b"two".to_vec())]);

        let how = feed(source, &tx, &stop);

        assert_eq!(how, Ending::EndedWell);
        drop(tx);
        assert_eq!(drain_until_closed(&rx), b"onetwo");
    }

    #[test]
    fn one_read_hands_over_a_whole_buffer_and_the_next_one_carries_on() {
        let (tx, rx) = sync_channel(READS_IN_FLIGHT);
        let stop = AtomicBool::new(false);
        // A turn that fills the buffer and one behind it. The full one is the **only** length at
        // which `buf[..n]` and the whole buffer are the same slice, and it is the one length that
        // says how big that buffer is: every other test here hands over a handful of letters and
        // would read the same at any size. The size is not a tuning knob — the brake **D8** chose
        // is it times [`READS_IN_FLIGHT`] — and halving the buffer was measured to leave the whole
        // series green, which is why this length is scripted rather than a convenient short one.
        let source = Scripted::of(vec![Ok(vec![FILLER; READ_SIZE]), Ok(b"tail".to_vec())]);

        let how = feed(source, &tx, &stop);

        assert_eq!(how, Ending::EndedWell);
        drop(tx);
        let first = rx.recv_timeout(DEADLINE).expect("the first read");
        // The length and the content are asserted apart on purpose: comparing eight thousand
        // bytes against eight thousand bytes prints both of them when it fails, and a red test
        // nobody can read is most of the way back to no test.
        assert_eq!(
            first.len(),
            READ_SIZE,
            "a full read did not come back whole"
        );
        assert!(
            first.iter().all(|&byte| byte == FILLER),
            "what came back is not what was handed over"
        );
        // And a read that filled the buffer is not the end of anything: the turn behind it is
        // there so that stopping at a full read would be visible.
        assert_eq!(drain_until_closed(&rx), b"tail");
    }

    #[test]
    fn a_console_that_is_still_reading_has_no_ending_to_report() {
        let (pty, output) = Pty::spawn("sh", &["-c", "printf ready; sleep 30"], Size::default())
            .expect("launch sh");

        wait_for_marker(&output, "ready");

        // «Nothing to report» and «ended for no reason» are different answers, and this is the one
        // that keeps the absence from meaning anything.
        assert_eq!(pty.ending(), None);
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

    /// The hang this test was written against is demonstrated, not hypothetical: with the channel
    /// full, the reader thread is not inside `read` but waiting for room in [`deliver`], and
    /// `Drop for Pty` used to `join` it without a deadline — so with nothing pulling that thread
    /// out of its wait, neither the `kill` nor the end of file reaching it there, the `join` never
    /// returned and closing the window hung.
    ///
    /// **What it can still tell apart is narrower than that, and it is measured**: since closing
    /// abandons the reader after [`READER_GIVE_UP`] (**D32**), the close comes back whether or not
    /// the flag is ever consulted, and with the check deleted this test stays green. The flag's
    /// effect on this path is what it *costs*, and that is asserted by the test below.
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

    /// **The stop flag's only remaining witness on the delivery path.**
    ///
    /// `closing_an_ordinary_console_does_not_spend_the_abandon_deadline` measures a reader parked
    /// inside `read`, and its own comment says the drain there is what keeps it *off* the delivery
    /// path. This is that other path: with the channel full the reader waits for room in
    /// [`deliver`], where the flag is the only thing that can reach it — not the `kill`, not the
    /// end of file.
    ///
    /// **Measured, and it is the whole reason this exists**: with the flag never consulted, the
    /// test above — the one written against that very hang — stays green, because the reader is
    /// abandoned after [`READER_GIVE_UP`] and the close comes back either way. What the flag buys
    /// is that the thread *ends*, and a thread that ended is not waited for. So the cost is the
    /// only thing left that tells the two apart, and this asserts it.
    ///
    /// **The bound is [`READER_GIVE_UP`] and not a literal**, the same choice as the ordinary
    /// console's: it moves on its own if the constant ever does. Nothing here claims a number is
    /// fast — only that the reserve for a survivor is not spent when there is none.
    #[test]
    fn closing_with_the_channel_full_does_not_spend_the_abandon_deadline() {
        use std::sync::mpsc;

        let (pty, output) = Pty::spawn("cat", &["/dev/zero"], Size::default()).expect("launch cat");

        // Same wait and same reason as the test above: the channel has to be really full when the
        // close arrives, which is what parks the reader in the wait for room.
        thread::sleep(Duration::from_millis(200));

        let (done, answer) = mpsc::channel();
        let closing = thread::spawn(move || {
            let started = Instant::now();
            drop(pty);
            let _ = done.send(started.elapsed());
        });

        // Generous, because this is not the assertion: it only keeps the old hang from taking the
        // suite with it instead of turning it red.
        let took = answer
            .recv_timeout(Duration::from_secs(2))
            .expect("closing a console whose channel is full returned");

        assert!(
            took < READER_GIVE_UP,
            "closing with the channel full took {took:?}, the {READER_GIVE_UP:?} this crate \
             reserves for abandoning a reader: nothing pulled that thread out of its wait for room"
        );

        let _ = closing.join();
        // **Released only here, and that is load-bearing.** A receiver dropped any earlier makes
        // the delivery fail as disconnected, which ends the thread for a reason that is not the
        // flag — the close would come back fast and this test would be measuring the disguise.
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

    /// Waits up to `within` for a thread to end, and answers whether it did.
    ///
    /// **The thread ending is what these tests assert, never the drop returning**: `Drop for Pty`
    /// returns whether or not the writer ever ends, so «closing came back» would pass over the leak.
    #[cfg(target_os = "linux")]
    fn ends_within(thread: &JoinHandle<()>, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        while !thread.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        thread.is_finished()
    }

    /// Hands over `chunks` one-byte writes and answers how many of them the kernel took once the
    /// writer settles: what it is still owed stops moving when the buffer toward the program is full.
    ///
    /// **Settled means three equal readings in a row, 200 ms apart**, not two: a writer descheduled
    /// for one interval would otherwise read as settled with room still left.
    #[cfg(target_os = "linux")]
    fn one_byte_chunks_taken(pty: &mut Pty, chunks: usize) -> usize {
        for _ in 0..chunks {
            pty.write(&[FILLER]).expect("hand over");
        }
        let deadline = Instant::now() + DEADLINE;
        let mut readings = [usize::MAX - 1, usize::MAX];
        while Instant::now() < deadline {
            thread::sleep(Duration::from_millis(200));
            let now = pty.backlog().map_or(0, |backlog| backlog.bytes);
            if readings == [now, now] {
                return chunks - now;
            }
            readings = [readings[1], now];
        }
        panic!("what the writer owes was still moving after {DEADLINE:?}");
    }

    /// Whether the master has room for a write right now, asked without waiting.
    #[cfg(target_os = "linux")]
    fn has_room(pty: &Pty) -> bool {
        let fd = pty.master.as_raw_fd().expect("the master's descriptor");
        let mut watched = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };
        // SAFETY: one valid, exclusively borrowed `pollfd`, and a zero timeout.
        let ready = unsafe { libc::poll(&mut watched, 1, 0) };
        assert!(
            ready >= 0,
            "poll failed: {}",
            std::io::Error::last_os_error()
        );
        watched.revents & libc::POLLOUT != 0
    }

    /// **The regression test of the writer left behind by a close (#159's first half).** With the
    /// program alive and not reading, the writer waits for room in the pseudoterminal. Closing kills
    /// the program — and on Linux a write parked in the kernel does not end with it: measured, the
    /// slave gone (`POLLHUP`, a read answering `EIO`) and the write still waiting seconds later, for
    /// the life of the process. What closing must do is end that thread.
    ///
    /// **Linux only, like the fix**: elsewhere the write is still the blocking one (D29).
    #[cfg(target_os = "linux")]
    #[test]
    fn closing_a_console_whose_program_stopped_reading_ends_its_writer() {
        const MARKER: &str = "raw-and-deaf-for-good";
        // `&&` for the reason its siblings above give: with the echo still on, the line discipline
        // would hand the payload straight back and nothing would ever wait for room.
        let script = format!("stty raw -echo && printf {MARKER} && exec sleep 60");
        let (mut pty, output) =
            Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");
        wait_for_marker(&output, MARKER);

        pty.write(&vec![FILLER; 4 * PAYLOAD]).expect("hand over");
        thread::sleep(Duration::from_millis(300));
        // **The premise, asserted rather than assumed**: the writer has to be waiting for room when
        // the console closes, or this measures a writer that had already finished.
        assert!(
            pty.backlog().is_some_and(|backlog| backlog.bytes > 0),
            "the program took all {} bytes: the writer is not waiting for room, so closing would \
             prove nothing about a writer that is",
            4 * PAYLOAD
        );

        let writer = pty.writer.take().expect("the writer thread");
        drop(pty);
        assert!(
            ends_within(&writer, Duration::from_secs(2)),
            "the writer thread was still alive two seconds after its console closed: it is parked in \
             a write that nothing closing does can end"
        );
        drop(output);
    }

    /// **#159's second half: the writer that has nothing left to write, over a buffer that is
    /// full.** When the writer's loop ends, `portable-pty`'s own writer is dropped, and its destructor
    /// writes a newline and the terminal's end-of-file character into the pseudoterminal — `stty raw`
    /// leaves that character set. With the buffer toward the program exactly full, that write waits
    /// for room as well, so a fix that only reached the loop would still leave the thread behind.
    ///
    /// **Exactly full is calibrated, not guessed**: a first console measures how many one-byte writes
    /// the kernel takes before it stops, and the second is handed exactly that many — the same shape
    /// of writes, so the kernel's buffer ends up just as full and nothing is owed. And «full» is then
    /// asked of the master itself, because «nothing owed» alone would also hold with room left.
    #[cfg(target_os = "linux")]
    #[test]
    fn closing_a_console_whose_buffer_is_exactly_full_ends_its_writer() {
        const MARKER: &str = "raw-and-full";
        const MORE_THAN_FITS: usize = 4 * PAYLOAD;
        let script = format!("stty raw -echo && printf {MARKER} && exec sleep 60");

        let (mut probe, probe_output) =
            Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");
        wait_for_marker(&probe_output, MARKER);
        let fits = one_byte_chunks_taken(&mut probe, MORE_THAN_FITS);
        assert!(
            fits < MORE_THAN_FITS,
            "the kernel took all {MORE_THAN_FITS} bytes: nothing was ever full"
        );
        drop(probe);
        drop(probe_output);

        let (mut pty, output) =
            Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");
        wait_for_marker(&output, MARKER);
        let taken = one_byte_chunks_taken(&mut pty, fits);
        // The premise: every byte went in, so the loop has nothing to wait on and only the
        // destructor's write is left to find the buffer full.
        assert_eq!(
            taken, fits,
            "the second console took {taken} of the {fits} bytes the first one did: the buffer is not \
             in the state this test needs"
        );
        assert!(
            !has_room(&pty),
            "nothing is owed but the master still has room: the destructor's write would find room \
             and this would prove nothing about a full buffer"
        );

        let writer = pty.writer.take().expect("the writer thread");
        drop(pty);
        assert!(
            ends_within(&writer, Duration::from_secs(2)),
            "the writer thread was still alive two seconds after its console closed with nothing left \
             to write: the write its destructor makes waited for room"
        );
        drop(output);
    }

    /// **#159's third way out of the writer's wait: the program dies while the console stays
    /// open.** Nobody closes anything here, so no knock arrives; what ends the wait has to be the
    /// master answering a hang-up with no room. Before the fix the writer sat inside the kernel's
    /// write for the life of the process — measured, the program's death does not end it — so
    /// `Pty::write` kept answering «taken» into a console that would never take anything again.
    ///
    /// **The effect is asserted, not only the flag**: the writer thread ends and a later write is
    /// refused. `gave_up` is asserted beside them, not instead of them.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_program_that_dies_with_its_writer_waiting_for_room_makes_the_writer_give_up() {
        const MARKER: &str = "raw-and-deaf-until-it-dies";
        // `&&` for the reason its siblings above give: with the echo still on, the line discipline
        // would hand the payload straight back and nothing would ever wait for room. And the program
        // ends on its own, which is the whole case: the console is never closed. Four seconds leave
        // the premise below room on a loaded machine.
        let script = format!("stty raw -echo && printf {MARKER} && exec sleep 4");
        let (mut pty, output) =
            Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");
        wait_for_marker(&output, MARKER);

        pty.write(&vec![FILLER; 4 * PAYLOAD]).expect("hand over");
        thread::sleep(Duration::from_millis(300));
        // **The premise, asserted rather than assumed**: the program is still alive and the writer
        // is waiting for room, or what follows measures a writer that had already finished.
        assert!(
            pty.ending().is_none(),
            "the program ended before the writer was seen waiting: the premise cannot be checked"
        );
        assert!(
            pty.backlog()
                .is_some_and(|backlog| backlog.bytes > 0 && !backlog.gave_up),
            "the writer is not waiting for room with the program alive, so its death would prove \
             nothing about a writer that is"
        );

        // The program dies on its own; the reader records it.
        let deadline = Instant::now() + DEADLINE;
        while pty.ending().is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            pty.ending().is_some(),
            "the program did not end within {DEADLINE:?}: the case this test covers never happened"
        );

        let writer = pty.writer.take().expect("the writer thread");
        assert!(
            ends_within(&writer, Duration::from_secs(2)),
            "the writer thread was still alive two seconds after its program died, with the console \
             open: it is waiting for room that will never come"
        );
        assert!(
            pty.backlog().is_some_and(|backlog| backlog.gave_up),
            "the writer ended without saying it gave up"
        );
        assert!(
            pty.write(b"x").is_err(),
            "a write into a console whose program is gone was answered «taken»"
        );
        drop(pty);
        drop(output);
    }

    /// **The knock, on its own.** In the tests above the program dies, and a master whose program
    /// has gone answers a hang-up — which ends the writer's wait whether closing knocked or not, so
    /// none of them can see the knock. Here a survivor keeps the program's side of the pseudoterminal
    /// open, so there is no hang-up, and the only thing that can end the writer's wait is closing.
    ///
    /// **And the knock goes early, which is what the timing asserts.** With a survivor, closing
    /// spends the reader's whole abandon deadline, so the writer has had that long by the time the
    /// drop returns. A knock left for the end of the drop — the field falling with the struct — would
    /// still arrive, a few microseconds before this looks: the writer ending first is the order
    /// `Drop for Pty` chose, and this is where it is pinned — mutated: with the early drop removed,
    /// this goes red.
    #[cfg(target_os = "linux")]
    #[test]
    fn closing_a_console_whose_program_left_a_survivor_ends_its_writer_before_the_close_returns() {
        const MARKER: &str = "raw-deaf-and-held";
        // The brace group is load-bearing for the reason the survivor test further down gives: it
        // backgrounds only the survivor, and `setsid` makes it escape the hang-up closing sends.
        let script = format!(
            "{{ setsid sleep 10 & }} && stty raw -echo && printf {MARKER} && exec sleep 60"
        );
        let (mut pty, output) =
            Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");
        wait_for_marker(&output, MARKER);

        pty.write(&vec![FILLER; 4 * PAYLOAD]).expect("hand over");
        thread::sleep(Duration::from_millis(300));
        assert!(
            pty.backlog().is_some_and(|backlog| backlog.bytes > 0),
            "the program took all {} bytes: the writer is not waiting for room",
            4 * PAYLOAD
        );

        let writer = pty.writer.take().expect("the writer thread");
        let closing = Instant::now();
        drop(pty);
        let took = closing.elapsed();
        // The premise: the survivor really held the program's side, so the close waited out the
        // reader's deadline — otherwise a hang-up could have ended the writer instead of the knock.
        assert!(
            took >= READER_GIVE_UP,
            "closing took {took:?}, less than the {READER_GIVE_UP:?} a held pseudoterminal costs: \
             the survivor did not hold it, so this proves nothing about the knock"
        );
        assert!(
            writer.is_finished(),
            "the writer thread had not ended when a close that took {took:?} returned: nothing but \
             the knock can end its wait here, and it did not arrive in time"
        );
        drop(output);
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
    /// On the fixed tree it passes in about 0.15 s, which is the program's dying plus the 100 ms
    /// abandoned deadline plus the drain: the deadline really was spent.
    ///
    /// The drop itself runs on a thread of its own, answering through a channel, for the same reason
    /// `drain_until_closed` carries a deadline: on a build with the bug a direct `join` would never
    /// return and would take the suite down with it, publishing the hang as an eternal green.
    #[test]
    fn closing_does_not_hang_when_a_survivor_holds_the_pseudoterminal() {
        use std::sync::mpsc;

        /// **Both bounds carry weight, and the band between them is where this test binds.**
        /// Measured after the fix, this shape returns in some 100 ms — a millisecond or two for the
        /// program to die of the hang-up, plus the 100 ms this crate gives the reader before abandoning it —
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
        // measuring the easy case under this one's name. Measured when closing still paid the
        // library's 50 ms step: with the echo on it returned in 0.10 s, and with no survivor in
        // 0.05 s — and on the day of D55, with that step gone, in 1.20–1.37 ms both. The bound is the constant itself, so a future
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
    /// the wait returns at once: what is left is the program's dying, a millisecond or two. Asserting
    /// «under the deadline» therefore has ample headroom over what this costs and still fails the
    /// moment the deadline starts being paid on a healthy close — and it moves on its own if the
    /// constant ever changes, which a literal would not.
    ///
    /// It is **not** a performance assertion: nothing here claims the close is fast, only that the
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

    /// Waits until the console's program says `marker`, drains it to quiet, and times dropping it.
    ///
    /// The shape every closing test here needs, and the one that keeps a regression from taking the
    /// suite with it: the drop runs on a thread of its own and answers through a channel, so a close
    /// that never comes back is a red test after two seconds rather than a `cargo test` that never
    /// ends. The drain parks the reader inside `read`, for the reasons the tests above give.
    fn time_the_close(pty: Pty, output: Receiver<Vec<u8>>, marker: &str) -> Duration {
        use std::sync::mpsc;

        wait_for_marker(&output, marker);
        while output.recv_timeout(Duration::from_millis(50)).is_ok() {}

        let (done, answer) = mpsc::channel();
        let closing = thread::spawn(move || {
            let started = Instant::now();
            drop(pty);
            let _ = done.send(started.elapsed());
        });
        // Well under the life of every program these tests close — five seconds, or no end at all:
        // a close that came back only because the program ended on its own would otherwise pass for
        // a close that worked.
        let took = answer
            .recv_timeout(Duration::from_secs(2))
            .expect("closing the console returned");
        let _ = closing.join();
        // Released only after the close, as everywhere in this module.
        drop(output);
        took
    }

    /// **Closing a console whose program dies on hang-up costs what the dying costs, not a step of
    /// somebody's grid.** `portable-pty`'s own `kill` looks once right after its SIGHUP and then
    /// sleeps 50 ms between looks, so a program that needs any time at all to die —a shell takes one
    /// or two milliseconds— paid the whole step, once per console, inside the interface pass.
    ///
    /// **The bound is a literal on purpose, and it is the library's step, not a constant of this
    /// crate.** It states the defect: every close on the old path took *at least* that, because the
    /// sleep is a floor, so no machine however fast passes it by luck. And the fixed path measures a
    /// couple of milliseconds, so a loaded machine has some twenty-fold headroom before it could fail
    /// for a reason that is not the defect.
    ///
    /// `sleep` keeps the default hang-up disposition, which is what a shell or an agent does as far
    /// as this close is concerned: it dies of the signal.
    #[test]
    fn closing_a_console_does_not_wait_out_the_librarys_poll_step() {
        const LIBRARY_STEP: Duration = Duration::from_millis(50);
        const MARKER: &str = "up";

        let script = format!("printf {MARKER} && exec sleep 5");
        let (pty, output) = Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");
        let took = time_the_close(pty, output, MARKER);

        assert!(
            took < LIBRARY_STEP,
            "closing a console whose program dies on hang-up took {took:?}, at least the \
             {LIBRARY_STEP:?} between two looks of the library's poll: the close is waiting on a \
             grid, not on the program"
        );
    }

    /// **The other half: a program still gets its grace after the hang-up.** Killing outright on
    /// close would pass the test above too, and it would take away the time a program that catches
    /// the hang-up has to save its state. So a program that ignores the hang-up must be killed, and
    /// **not before [`HANGUP_GRACE`]**: the floor is what tells a grace apart from an immediate kill.
    ///
    /// `trap '' HUP` sets the disposition to ignored, and an ignored signal survives `exec`, so it is
    /// `sleep` itself that ignores it. The marker comes after the `trap` in the same `&&` chain, so it
    /// proves the trap was set — without it the test would time a program that died at once and fail
    /// on the floor for a reason that is not the grace. Unix only: elsewhere the close is the
    /// library's immediate `kill`, with no grace to owe.
    #[cfg(unix)]
    #[test]
    fn a_program_that_ignores_the_hang_up_is_killed_after_its_grace() {
        const MARKER: &str = "deaf";

        let script = format!("trap '' HUP && printf {MARKER} && exec sleep 5");
        let (pty, output) = Pty::spawn("sh", &["-c", &script], Size::default()).expect("launch sh");
        let took = time_the_close(pty, output, MARKER);

        assert!(
            took >= HANGUP_GRACE,
            "a program that ignores the hang-up was gone after {took:?}, before the \
             {HANGUP_GRACE:?} of grace it is owed: it was killed outright"
        );
    }

    /// **A program that catches the hang-up to save its state gets to finish saving, as long as the
    /// save fits in the library's grace.** The test above measures its floor against
    /// [`HANGUP_GRACE`] itself, so shortening the constant moves the floor with it and nothing goes
    /// red — measured: a 50 ms grace passed the whole suite. This one states the grace from the
    /// program's side instead, with a save that takes a literal 100 ms: comfortably inside the
    /// library's 200 ms, and past anything shorter than it by enough that a loaded machine is not
    /// what decides it.
    ///
    /// The program's trap runs only once its foreground `sleep` returns, which is why that sleep is
    /// ten milliseconds and not the whole wait. The save goes to a file, not to the console: the
    /// reader is being stopped while it happens, so bytes on the pseudoterminal prove nothing. The
    /// file's path reaches the script as `$0`, never spliced into it, so a temporary directory with a
    /// space in its name is still the file the assertion reads.
    #[cfg(unix)]
    #[test]
    fn a_program_that_catches_the_hang_up_finishes_saving_before_it_is_killed() {
        const MARKER: &str = "saving";

        let saved = std::env::temp_dir().join(format!("term-core-hang-up-{}", std::process::id()));
        let saved_path = saved.to_str().expect("a temporary path that is text");
        let _ = std::fs::remove_file(&saved);
        let script = format!(
            "trap 'sleep 0.1 && printf done > \"$0\" && exit 0' HUP && printf {MARKER} && \
             while :; do sleep 0.01; done"
        );
        let (pty, output) =
            Pty::spawn("sh", &["-c", &script, saved_path], Size::default()).expect("launch sh");
        // What the close cost is deliberately not asserted: the save alone takes some 110 ms of a
        // 200 ms grace, and a ceiling with less than two-fold headroom on a loaded machine would fail
        // for reasons that are not the grace.
        time_the_close(pty, output, MARKER);

        let finished = std::fs::read_to_string(&saved).is_ok_and(|body| body == "done");
        let _ = std::fs::remove_file(&saved);
        assert!(
            finished,
            "a program that needed 100 ms after the hang-up to save its state was killed before it \
             finished: the grace it is owed is shorter than the library's 200 ms"
        );
    }

    /// What closing a console whose program refuses some signal left: how long the close took,
    /// whether the program was still alive right after it, and whether it was reaped once the test
    /// ended it for real. `refusing` stands in for `kill(2)`.
    ///
    /// **Why each of the three can fail on its own defect.** The program lives thirty seconds, so a
    /// close that waited for it cannot come back under a second; a close that got there by some
    /// other way leaves nothing alive — though only a kill: the programs ignore SIGHUP, because the
    /// kernel's own hang-up is a signal no forgery reaches, so a close that sent one around the
    /// forgery would go unseen; and a child nobody waits for stays a zombie for the life
    /// of this process — `kill(pid, 0)` answers for a zombie exactly as for a live program.
    #[cfg(unix)]
    fn close_one_that_refuses(
        script: &str,
        marker: &str,
        refusing: fn(libc::pid_t, libc::c_int) -> std::io::Result<()>,
    ) -> (Duration, bool, bool) {
        let (mut pty, output) =
            Pty::spawn("sh", &["-c", script], Size::default()).expect("launch sh");
        pty.send_signal = refusing;
        let pid = pty
            .child
            .as_ref()
            .and_then(|child| child.process_id())
            .and_then(|pid| libc::pid_t::try_from(pid).ok())
            .expect("the child has a pid");
        // SAFETY: `kill` takes plain integers and touches no memory of this process.
        let exists = || unsafe { libc::kill(pid, 0) } == 0;

        let took = time_the_close(pty, output, marker);
        // **Looked at after a pause, not at once.** The kernel's hang-up lands when the last copy of
        // the master closes, and that can be the writer thread's, which the close never joins; a
        // program killed by it is gone a moment later. Looking at once let such a program pass for
        // alive on almost every run, so the test failed now and then instead of every time.
        thread::sleep(Duration::from_millis(200));
        let alive_after_closing = exists();
        // Ended by the test, the way it would end on its own: the refusal was forged, this signal is
        // real.
        // SAFETY: as above.
        unsafe { libc::kill(pid, libc::SIGKILL) };
        let deadline = Instant::now() + Duration::from_secs(2);
        while exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        (took, alive_after_closing, !exists())
    }

    #[cfg(unix)]
    fn refuse_every_signal(_: libc::pid_t, _: libc::c_int) -> std::io::Result<()> {
        Err(std::io::Error::from_raw_os_error(libc::EPERM))
    }

    #[cfg(unix)]
    fn refuse_only_the_kill(pid: libc::pid_t, number: libc::c_int) -> std::io::Result<()> {
        if number == libc::SIGKILL {
            refuse_every_signal(pid, number)
        } else {
            send_signal(pid, number)
        }
    }

    /// **A program the system will not let this process signal is left running, not waited for.**
    /// Reached with `exec pkexec <cmd>` in a shell: the console's own child becomes a program with
    /// every user identity root, the hang-up is refused, and closing used to wait for it to leave on
    /// its own — inside the interface pass, measured as a window the desktop offered to force-quit.
    /// A second is ten times the most this close can cost — the reader's deadline, which a program
    /// still holding the pseudoterminal may make it pay.
    ///
    /// **The program ignores the hang-up, and not as decoration.** The forgery covers the signals
    /// this crate sends and nothing else: once the last copy of the master closes, the kernel hangs
    /// the terminal up and signals the session leader itself, with no refusal to forge. A program
    /// that did not ignore it died a moment after the close, and this test passed only while its
    /// look came first. The marker comes after the `trap`, so the process holding the premise is the
    /// one that proves it.
    #[cfg(unix)]
    #[test]
    fn a_program_that_refuses_the_hang_up_is_left_running_and_reaped_when_it_ends() {
        const MARKER: &str = "root";

        let (took, alive, reaped) = close_one_that_refuses(
            &format!("trap '' HUP && printf {MARKER} && exec sleep 30"),
            MARKER,
            refuse_every_signal,
        );

        assert!(
            took < Duration::from_secs(1),
            "closing a console whose program refused the hang-up took {took:?}: it waited for the \
             program instead of leaving it"
        );
        assert!(
            alive,
            "the program was gone right after the close, so the close ended it some other way than \
             the refused signals — this test no longer exercises a refusal"
        );
        assert!(
            reaped,
            "the program left behind ended and stayed a zombie: nobody is waiting for it"
        );
    }

    /// **The same when only the kill at the end of the grace is refused.** The hang-up is accepted
    /// and ignored — `trap '' HUP`, which survives `exec` — so the close sits out its grace and then
    /// meets the refusal on the one signal left. A second is three times what that costs: the grace
    /// plus the reader's deadline.
    #[cfg(unix)]
    #[test]
    fn a_program_whose_kill_is_refused_after_its_grace_is_left_running_and_reaped() {
        const MARKER: &str = "deaf";

        let (took, alive, reaped) = close_one_that_refuses(
            &format!("trap '' HUP && printf {MARKER} && exec sleep 30"),
            MARKER,
            refuse_only_the_kill,
        );

        assert!(
            took < Duration::from_secs(1),
            "closing a console whose program's kill was refused took {took:?}: it waited for the \
             program instead of leaving it"
        );
        assert!(
            alive,
            "the program was gone right after the close although its hang-up is ignored and its kill \
             refused — this test no longer exercises a refusal"
        );
        assert!(
            reaped,
            "the program left behind ended and stayed a zombie: nobody is waiting for it"
        );
    }
}
