//! The thread that brings what the program writes, and the signal that it has died.
//!
//! It exists for a concrete reason: reading from the pseudoterminal **blocks** until there is
//! something to read, and a blocking wait inside the frame would freeze the window. The thread
//! waits on its own, leaves what it read in a queue and **wakes the window**. That waking is the
//! only thing that lets an agent write by itself and be seen to advance without touching the
//! keyboard.
//!
//! **And there is a signal that would break in silence if no one held it up.** The only way to
//! know that the program has died is for its output channel to close. By handing that channel to
//! another thread, whoever paints stops seeing it. Here it is relayed: the thread takes along a
//! sender that never sends anything and drops it on finishing, so the closure reaches the other
//! side **by construction**, not because someone remembers to raise a flag.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// How much is stored ahead before we stop picking up from the channel.
///
/// **It is the other half of the brake**, and without it the one below is worth nothing: the
/// `term-core` channel has a ceiling, but a thread that empties it the moment something arrives
/// makes that ceiling never be reached, and then nothing brakes the program. Measured: bounding
/// only the channel leaves memory consumption exactly the same as before.
///
/// One megabyte is eight frames of work ahead with the current budget: enough for the painting to
/// never end up waiting for bytes, and little enough that holding it makes no difference. What
/// does not fit is not lost: it stays in the program that wrote it, which waits.
const QUEUE_CEILING: usize = 1024 * 1024;

/// How often the thread wakes to check on its own whether there is room already.
///
/// The signal from whoever picks up is what normally wakes it; this deadline only exists so that a
/// lost signal costs a millisecond and not a hang.
const RECHECK: Duration = Duration::from_millis(1);

/// What is in the queue: the bytes and when each batch arrived.
#[derive(Default)]
struct Queue {
    bytes: VecDeque<u8>,
    /// When each batch arrived and up to which byte it reaches, counting from the beginning of
    /// everything that has passed through here. It exists to be able to answer **how long what is
    /// picked up has been waiting**: in a dump that wait is the stretch that dominates the visual
    /// delay —more than a second, against the microseconds of everything else— and without
    /// stamping it the number would say the application is instantaneous exactly when it is not.
    arrivals: VecDeque<(u64, Instant)>,
    /// How many bytes have been picked up in total. It is the origin against which arrivals are read.
    bytes_picked_up: u64,
}

/// The result of a pickup: whether a queue is left, and since when what was taken away was waiting.
pub struct Pickup {
    /// The budget cut off the pumping and there is a queue left to swallow.
    pub left_a_queue: bool,
    /// When the oldest of the picked-up bytes arrived, or nothing if none was picked up. It is the
    /// beginning of the stretch the user sees as delay.
    pub waiting_since: Option<Instant>,
}

/// What the program has written and has not been picked up yet, plus the signal of whether it is
/// still alive.
pub struct PtyOutput {
    /// Whether it has already been said that this console's program stopped taking what is typed.
    ///
    /// **It lives here and not beside the pseudoterminal** because this is the type the pump already
    /// asks on every frame, and the alternative was one more line inside the window's core — which
    /// **D28**'s ratchet does not admit. See [`DeafConsole`].
    deaf: std::cell::RefCell<DeafConsole>,
    /// Whether it has already been said that this console stopped receiving its program's output.
    ///
    /// It lives here for the same reason `deaf` does. Two cells rather than one holding both marks:
    /// each side is a decision of its own with its own reasons for staying quiet, and one cell would
    /// have made them one type. See [`QuietConsole`].
    quiet: std::cell::RefCell<QuietConsole>,
    /// A double-ended queue and not a vector: it is appended at the end and taken from the
    /// beginning, and taking from the beginning of a vector copies everything left behind it. With
    /// a dump in progress that would be megabytes copied on each frame, and that cost would end up
    /// inside the number this milestone exists to measure.
    queue: Arc<Mutex<Queue>>,
    /// Tells the thread that room has been made in the queue and it can pick up from the channel again.
    ///
    /// It goes apart from the lock and not inside `Queue` because whoever waits and whoever
    /// notifies are different threads and neither of the two owns the other.
    room: Arc<Condvar>,
    /// Takes the thread out of the wait for room when this object stops existing.
    ///
    /// Waiting for room is the only thing the thread does **without** looking at the channel, so
    /// there it does not find out that the program has died nor that no one is going to pick up any
    /// more. Without this flag, a destroyed bridge with the queue at the ceiling leaves its thread
    /// spinning forever.
    stop: Arc<AtomicBool>,
    /// It carries nothing: what matters is **when it closes**, which is when the thread finishes.
    /// The empty type leaves it written in the signature that no value is going to arrive through here.
    death: Receiver<std::convert::Infallible>,
}

impl PtyOutput {
    /// Starts the thread that keeps the program's output.
    ///
    /// `wake` is what has to be done for the window to find out that there is something new; in the
    /// application it is asking it for a repaint. It is received as a function instead of receiving
    /// the whole window for two reasons: so this module does not depend on the interface framework
    /// —which is what is going to be rewritten— and so **the waking can be checked in a test**,
    /// which is the only way to prove the part of the milestone that says the window wakes on its own.
    ///
    /// Asking for the repaint too much costs nothing —they group themselves— and asking for it too
    /// little is a still window with work to show.
    ///
    /// **`wake` cannot panic.** If it did, the thread would die, a death that has not happened
    /// would be published, and along the way the output channel would be destroyed: the reader of
    /// the crate below would break on sending, would stop draining the pseudoterminal and the
    /// program would end up blocked on writing.
    ///
    /// **The thread finishes with whatever happens first:** the output channel closing —that is,
    /// the program dying— or this object being destroyed. The second is needed because waiting for
    /// room is the only thing the thread does without looking at the channel, and there it would
    /// not find out about either of the two things. Even so it is worth keeping it next to the
    /// `Pty`: separating them leaves a queue without an owner.
    pub fn new(output: Receiver<Vec<u8>>, wake: impl Fn() + Send + 'static) -> Self {
        let queue = Arc::new(Mutex::new(Queue::default()));
        let thread_queue = Arc::clone(&queue);
        let room = Arc::new(Condvar::new());
        let thread_room = Arc::clone(&room);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let (death_sender, death) = channel();

        std::thread::spawn(move || {
            // The sender travels to the thread without being used: dropping it **is** the signal
            // that the program has died. It is dropped by hand further below, and the order with
            // the final notification is not a detail: see the comment there.
            let death_sender = death_sender;
            // `for` over the channel finishes exactly when the other end closes, which is when the
            // program dies. The label is for the other exit, the one further below.
            'pickup: for chunk in output {
                // The stamp is taken **before** asking for the lock: if the frame held it, waiting
                // for it would be counted as arrival time and the wait that is published would come
                // out inflated by the contention, which is not what we want to measure.
                let arrival = Instant::now();
                // The lock is taken and released on each chunk: the frame also wants it, and
                // holding it while waiting for the next chunk would be blocking the painting
                // exactly during the wait this thread exists to avoid.
                // If the lock is poisoned, the data inside is still valid: what happened was a
                // panic somewhere else. Recovering it is the right thing; discarding it in silence
                // would leave the window frozen without anything explaining it.
                let mut queue = thread_queue.lock().unwrap_or_else(PoisonError::into_inner);
                // **This is where this thread stops picking up, and that is why the program
                // brakes.** While it waits, the channel of the crate below fills up, its reader
                // thread stops draining the pseudoterminal, the kernel buffer fills up and the
                // program blocks on writing. It waits with the chunk already in hand —received but
                // not enqueued—, which is what makes the channel notice the jam.
                while queue.bytes.len() >= QUEUE_CEILING {
                    // Whoever picks up is whoever makes room, so if this object no longer exists,
                    // the room is never going to arrive. It exits through the epilogue below and
                    // not with a `return` so as not to skip the order between publishing the death
                    // and notifying.
                    if thread_stop.load(Ordering::Relaxed) {
                        break 'pickup;
                    }
                    let (reacquired, _) = thread_room
                        .wait_timeout(queue, RECHECK)
                        .unwrap_or_else(PoisonError::into_inner);
                    queue = reacquired;
                }
                queue.bytes.extend(&chunk);
                let up_to = queue.bytes_picked_up + queue.bytes.len() as u64;
                queue.arrivals.push_back((up_to, arrival));
                drop(queue);
                wake();
            }
            // **First the death is published and then the notification is sent.** The other way
            // round there is a race with a silent ending: the frame that wakes on that last
            // notification may arrive before the signal is published, see the program as alive, and
            // never receive any more notifications — the window would stay showing as alive
            // something that already died, which is exactly what this module exists to prevent.
            drop(death_sender);
            wake();
        });

        Self {
            deaf: std::cell::RefCell::default(),
            quiet: std::cell::RefCell::default(),
            queue,
            room,
            stop,
            death,
        }
    }

    /// Takes away up to `max` bytes of what is accumulated. Returns whether anything was left unpicked.
    ///
    /// **The maximum is not an ornament and taking everything away was not an option.** A dump
    /// delivers two orders of magnitude faster than the grid interprets, so emptying the whole
    /// queue into the caller's buffer just moves the pile elsewhere and duplicates it while it
    /// lasts. With the cap, the caller's buffer never grows beyond what it is going to interpret.
    ///
    /// **The maximum is not the queue's ceiling**, and it is worth not confusing them: this one
    /// bounds the work of a frame, and the ceiling ([`QUEUE_CEILING`]) bounds the memory. Picking
    /// up is also what makes room, so ceasing to call this ends up braking the program inside —
    /// which is exactly what is wanted when the painting cannot keep up.
    ///
    /// It receives the destination instead of returning a new one so that the frame can always
    /// reuse the same one and not allocate memory once per frame.
    pub fn pick_up(&self, destination: &mut Vec<u8>, max: usize, silences: Silences) -> Pickup {
        // **What the program is NOT taking is looked at here**, where the frame already asks this
        // type what it has. It says it once and never fixes anything: there is nothing to fix from
        // here, the bytes are owed to a program that is not asking for them.
        //
        // **Neither line may take the window down, and `eprintln!` would.** It panics when the
        // write fails, so `agent-terminal | head` — a closed pipe on standard error — killed the
        // window at the exact moment a console had something to say. The failure predates this
        // delivery by one statement and is fixed in both, because leaving one of two adjacent lines
        // able to panic is worse than the shape it protects.
        if let Some(sentence) = self.deaf.borrow_mut().look(silences.deaf) {
            say(&sentence);
        }
        // **And how the reading ended is looked at in the same place, for the same reason**: the
        // alternative was the window's core, whose ratchet (**D28**) does not admit the lines.
        if let Some(sentence) = self.quiet.borrow_mut().look(silences.quiet) {
            say(&sentence);
        }
        let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);

        // The queue keeps its bytes in two stretches —it wraps around on reaching the end—, so we
        // have to copy from both. Copying from the slices and not from the iterator is what makes
        // this cost what it takes away and not what it leaves behind.
        let to_take = queue.bytes.len().min(max);
        let (first, second) = queue.bytes.as_slices();
        let from_first = first.len().min(to_take);
        destination.extend_from_slice(&first[..from_first]);
        destination.extend_from_slice(&second[..to_take - from_first]);
        queue.bytes.drain(..to_take);

        // When the first of the bytes just taken arrived: the batch whose last byte falls beyond
        // what has already been picked up. It is the oldest of those going in this delivery, so the
        // wait it measures is the worst of those resolved in this frame — the one that is seen.
        let waiting_since = queue
            .arrivals
            .iter()
            .find(|(up_to, _)| *up_to > queue.bytes_picked_up)
            .map(|(_, when)| *when);

        queue.bytes_picked_up += to_take as u64;
        // The batches already served whole stop mattering. Without this purge the list would grow
        // forever during a dump, which is exactly when it cannot afford it.
        let bytes_picked_up = queue.bytes_picked_up;
        queue.arrivals.retain(|(up_to, _)| *up_to > bytes_picked_up);

        let pickup = Pickup {
            left_a_queue: !queue.bytes.is_empty(),
            waiting_since,
        };
        // The lock is released **before** notifying: otherwise, the thread wakes and the first
        // thing it does is wait for us to release it.
        drop(queue);
        self.room.notify_one();
        pickup
    }

    /// Whether the program inside is still alive.
    ///
    /// It stops being so the moment the thread finishes, which is when its output channel closes.
    pub fn is_alive(&self) -> bool {
        !matches!(self.death.try_recv(), Err(TryRecvError::Disconnected))
    }
}

impl Drop for PtyOutput {
    fn drop(&mut self) {
        // The thread is not waited on: it may be inside a read of the channel that takes as long as
        // the program takes to write, and blocking the window's closing for that would be swapping
        // one hang for another. It is enough for it to stop waiting for room, which is the only
        // thing it would not get out of on its own.
        self.stop.store(true, Ordering::Relaxed);
        self.room.notify_all();
    }
}

/// The two silences a console can fall into, as the frame finds them: what the program has not
/// taken, and how the reading ended.
///
/// **It is a value and not the pseudoterminal itself, because twenty of this module's tests move a
/// pickup without one.** Handing `&Pty` over would be simpler and impossible to cross halves of,
/// and it would demand a real pseudoterminal in every one of those tests — which is the same reason
/// the reading loop left its thread in the first place.
///
/// **And they travel together rather than as two arguments because of the ratchet.** Measured: two
/// arguments put `crates/app/src/lib.rs` three production lines over the number **D28**'s guard
/// records, because the formatter breaks a call whose arguments pass sixty characters into one line
/// per argument. The two accessors behind them stay apart on the pseudoterminal, which is **D38**'s
/// decision and not this type's business: one of them changes every frame and the other once per
/// console.
///
/// **The fields are private and `of` is the only way in from outside this module**, so no caller can
/// pair one console's backlog with another's ending. `Default` is «no silence at all», which is what
/// a test that is not about either of them wants.
#[derive(Default)]
pub struct Silences {
    /// What the program is not taking of what was typed into it. Read by [`DeafConsole`].
    deaf: Option<term_core::Backlog>,
    /// How the reading of its output ended. Read by [`QuietConsole`].
    quiet: Option<term_core::Ending>,
}

impl Silences {
    /// Reads both off one pseudoterminal, in one breath.
    pub fn of(pty: &term_core::Pty) -> Self {
        Self {
            deaf: pty.backlog(),
            quiet: pty.ending(),
        }
    }
}

/// Says one of this crate's lines on the developer channel, and **never takes the window down for
/// it**.
///
/// `eprintln!` panics when the write fails — a closed pipe on standard error, which is all
/// `agent-terminal | head` takes — and a window that dies because it had something to say is worse
/// than the silence the sentence was written against. What cannot be delivered is dropped: there is
/// no third channel to complain on.
fn say(sentence: &str) {
    use std::io::Write;

    let _ = writeln!(std::io::stderr().lock(), "{sentence}");
}

/// Decides whether a console that stopped receiving deserves a line on the developer channel, and
/// what that line says.
///
/// **It goes before this module's tests and not beside [`DeafConsole`]**, which is where the shape
/// it copies lives. **D28**'s counter stops at the first `mod tests` line, so a type declared after
/// it is not counted against the module's ceiling at all — and being counted is the point of a
/// ratchet. `DeafConsole` sitting on the other side of that line is a defect this type does not
/// inherit.
///
/// **It says what happened; it does not act on it.** Whether the window closes itself over a
/// console that ended badly, and what the person in front of the window is told, are two further
/// readers of the same value — neither of them here, and neither of them yet.
///
/// **Once, and only once per console** — the same rule [`DeafConsole`] follows, and for the same
/// reason: this runs on every frame, and an ending is a fact that does not change afterwards.
#[derive(Default)]
struct QuietConsole {
    said: bool,
}

impl QuietConsole {
    /// Looks at how the reading ended, and returns the sentence to say the first time one is owed.
    ///
    /// **It returns the sentence instead of printing it**, like the rest of this crate's decisions:
    /// printing cannot be observed from inside the process, so what a test can hold on to is what
    /// was decided.
    ///
    /// **Three of the four endings are silent on this channel, each for its own reason.**
    ///
    /// - **An ordinary ending is not a silence.** A program that exits and a console somebody closes
    ///   both take that arm, and neither is worth a word.
    /// - **A panicked reader is already printed by the runtime's own hook**, with its file and its
    ///   line, so a sentence here would duplicate what the process has said. What that hook does
    ///   **not** say is which console it belonged to, and naming the reading thread is what would
    ///   fix that — in the crate that spawns it, not here.
    /// - **A vanished listener has no path to this line in this application**, and the reason is
    ///   worth writing because the crate below says out loud that it cannot enforce it: that arm is
    ///   taken only when the stop flag is down, and an ordinary close raises it first because a pane
    ///   declares its pseudoterminal **before** the bridge that holds the receiver. Reverse those
    ///   two fields and every ordinary close is recorded as nobody listening. **D38**, and the guard
    ///   is `the_pseudoterminal_is_declared_before_the_bridge_that_holds_its_receiver`.
    fn look(&mut self, ending: Option<term_core::Ending>) -> Option<String> {
        let term_core::Ending::Failed { kind, os_error } = ending? else {
            return None;
        };
        if self.said {
            return None;
        }
        self.said = true;
        // **The number travels with the kind because the kind alone can say nothing.** What a
        // driver answers that the standard library has no name for flattens into one bucket whose
        // whole printed form is a single word, while the operating system's number survives it.
        // Measured, and recorded in **D38**.
        Some(match os_error {
            Some(number) => format!(
                "agent-terminal: this console stopped receiving: read failed ({kind:?}, os error \
                 {number})"
            ),
            None => {
                format!("agent-terminal: this console stopped receiving: read failed ({kind:?})")
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};
    use term_core::{Pty, Size};

    /// Waits for something to hold, with a cap so a failure comes out as a failure and not as a
    /// test hung forever.
    fn wait_for(mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn what_the_program_writes_reaches_the_queue() {
        let (_pty, output) = Pty::spawn("sh", &["-c", "printf hola"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        let mut received = Vec::new();
        let arrived = wait_for(|| {
            pty_output.pick_up(&mut received, usize::MAX, Silences::default());
            received.windows(4).any(|v| v == b"hola")
        });

        assert!(
            arrived,
            "what the program wrote did not arrive: {received:?}"
        );
    }

    #[test]
    fn the_death_of_the_program_survives_having_handed_off_the_channel() {
        // This is the contract that breaks in silence months later: handing the channel to another
        // thread takes with it the only signal that the program has died. If this test goes red,
        // the window would stay open and alive in front of a program that no longer exists.
        let (_pty, output) = Pty::spawn("sh", &["-c", "exit 0"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        assert!(
            wait_for(|| !pty_output.is_alive()),
            "the program finished and the bridge still says it is alive"
        );
    }

    #[test]
    fn when_the_final_notification_arrives_the_death_is_already_published() {
        // The order between publishing the death and giving the final notification decides whether
        // the failure is visible or silent: if it notified first, the frame that notification wakes
        // could see the program as alive, and afterwards there would be no more notifications — the
        // window would stay showing as alive something already dead, forever.
        //
        // It is caught like this: the program writes nothing, so the only notification is the final
        // one; and the waker dawdles inside, so that the test can look at the state **while** the
        // thread is still inside that call. With the wrong order, here it would read "alive".
        let wakeups = Arc::new(AtomicUsize::new(0));
        let thread_wakeups = Arc::clone(&wakeups);
        let (_pty, output) = Pty::spawn("sh", &["-c", "exit 0"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, move || {
            thread_wakeups.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(300));
        });

        assert!(
            wait_for(|| wakeups.load(Ordering::SeqCst) > 0),
            "the notification that the program finished did not arrive"
        );
        assert!(
            !pty_output.is_alive(),
            "the final notification arrived before the death was published"
        );
    }

    #[test]
    fn while_the_program_lives_the_bridge_says_so() {
        let (_pty, output) = Pty::spawn("sh", &["-c", "sleep 30"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        // Without this check, a bridge that said "dead" from the first instant would pass the death
        // test without having relayed anything.
        std::thread::sleep(Duration::from_millis(100));
        assert!(pty_output.is_alive(), "the program is still running");
    }

    #[test]
    fn the_arrival_of_bytes_wakes_the_window() {
        // It is the piece that makes it possible to see an agent working without touching the
        // keyboard. Without this notification the bytes would reach the queue and **the window
        // would stay asleep**, showing something old until the user moved the mouse. That the queue
        // fills does not prove it: we have to check that the repaint was asked for.
        // The program writes and **stays alive** on purpose: if it finished, the notification the
        // thread sends on saying goodbye would cover the one tested here, and the test would pass
        // even if no one woke on the bytes arriving. Checked by mutating the code: without the
        // loop's notification, this test goes red.
        let wakeups = Arc::new(AtomicUsize::new(0));
        let thread_wakeups = Arc::clone(&wakeups);
        let (_pty, output) = Pty::spawn(
            "sh",
            &["-c", "printf algo; sleep 30"],
            Size { rows: 24, cols: 80 },
        )
        .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, move || {
            thread_wakeups.fetch_add(1, Ordering::SeqCst);
        });

        let mut received = Vec::new();
        assert!(wait_for(|| {
            pty_output.pick_up(&mut received, usize::MAX, Silences::default());
            !received.is_empty()
        }));
        assert!(
            wakeups.load(Ordering::SeqCst) > 0,
            "bytes arrived and no one woke the window"
        );
    }

    #[test]
    fn picking_up_leaves_the_queue_empty() {
        let (_pty, output) = Pty::spawn("sh", &["-c", "printf uno"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        // We wait for the program to finish before picking up. With a live program, the
        // pseudoterminal could split "uno" into two deliveries and the test would fail one day out
        // of many for a reason that has nothing to do with what it tests.
        assert!(wait_for(|| !pty_output.is_alive()));

        let mut first = Vec::new();
        pty_output.pick_up(&mut first, usize::MAX, Silences::default());
        assert!(!first.is_empty(), "nothing arrived to pick up");

        // What is already picked up is not delivered again: if it repeated, the screen would show
        // the same text twice without the program having written it twice.
        let mut second = Vec::new();
        pty_output.pick_up(&mut second, usize::MAX, Silences::default());
        assert!(second.is_empty(), "the queue returned the same thing twice");
    }

    #[test]
    fn the_maximum_is_respected_and_what_is_left_over_stays_for_next_time() {
        // Without the maximum, whoever picks up takes the whole queue: in a dump that is
        // duplicating megabytes in the buffer of whoever paints, and the per-frame budget bounds
        // nothing because the spending already happened before reaching it.
        let (_pty, output) = Pty::spawn(
            "sh",
            &["-c", "printf 0123456789"],
            Size { rows: 24, cols: 80 },
        )
        .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});
        assert!(wait_for(|| !pty_output.is_alive()));

        let mut first = Vec::new();
        let pickup = pty_output.pick_up(&mut first, 4, Silences::default());
        assert_eq!(first, b"0123", "the maximum was not respected");
        assert!(pickup.left_a_queue, "bytes were left and it said no");

        // And what was left over is still there, in its order, without having been lost or
        // repeated: it is what makes a dump be seen whole even if it takes several frames.
        let mut rest = Vec::new();
        let pickup = pty_output.pick_up(&mut rest, usize::MAX, Silences::default());
        assert_eq!(
            rest, b"456789",
            "what was left over did not come out whole nor in order"
        );
        assert!(
            !pickup.left_a_queue,
            "the queue was left empty and it said no"
        );
    }

    /// Leaves a queue whose content is **split into two stretches**: the ring wraps around and its
    /// two slices are the two halves of `content`, with `in_first` bytes in the front one.
    ///
    /// It is assembled by hand, and not by filling the bridge through its channel, because from
    /// outside there is no way to force it: the queue only wraps around if it has free space left
    /// behind the head, and the moment it has none it grows and relocates in one piece. A test that
    /// tried it from the channel would pass almost always **without ever having tested the case**,
    /// which is worse than not having it. That is why the two checks at the end: if one day the
    /// library changes policy and this stops wrapping around, the test fails here instead of
    /// turning green forever.
    fn queue_split_in_two(content: &[u8], in_first: usize) -> VecDeque<u8> {
        let mut queue: VecDeque<u8> = VecDeque::with_capacity(content.len() * 2);
        let capacity = queue.capacity();

        // It is filled to the brim and everything but the first stretch is emptied: the head ends
        // up stuck to the end of the ring, so the next thing that enters has to wrap around.
        let filler = capacity - in_first;
        queue.extend(std::iter::repeat_n(b'.', filler));
        queue.extend(&content[..in_first]);
        queue.drain(..filler);
        queue.extend(&content[in_first..]);

        assert_eq!(queue.capacity(), capacity, "the queue grew and relocated");
        assert!(
            !queue.as_slices().1.is_empty(),
            "the queue did not wrap around"
        );
        queue
    }

    /// A bridge with no program behind it, with the queue set by hand.
    ///
    /// The sender is returned and has to be kept alive: dropping it closes the channel, and the
    /// bridge reads that as the program having died.
    fn pty_output_with_queue(bytes: VecDeque<u8>) -> (std::sync::mpsc::Sender<Vec<u8>>, PtyOutput) {
        let (sender, output) = channel();
        let pty_output = PtyOutput::new(output, || {});
        let mut queue = pty_output
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // A single arrival that covers everything set by hand: without it, picking up would not
        // know since when it was waiting and would return that there is nothing to wait for, which
        // is different from zero.
        queue
            .arrivals
            .push_back((bytes.len() as u64, Instant::now()));
        queue.bytes = bytes;
        drop(queue);
        (sender, pty_output)
    }

    #[test]
    fn picking_up_joins_the_two_stretches_when_the_queue_has_wrapped_around() {
        // The queue keeps its bytes in two chunks the moment it wraps around. Whoever copies only
        // the first one leaves half the dump in silence, and whoever joins them the wrong way round
        // writes on the screen what the program sent later before what it sent first.
        let (_sender, pty_output) = pty_output_with_queue(queue_split_in_two(b"0123456789", 6));

        let mut destination = Vec::new();
        let pickup = pty_output.pick_up(&mut destination, usize::MAX, Silences::default());

        assert_eq!(
            destination, b"0123456789",
            "the two stretches did not come out in order"
        );
        assert!(
            !pickup.left_a_queue,
            "it took the whole queue and said something was left"
        );
    }

    #[test]
    fn the_maximum_that_falls_within_the_first_stretch_does_not_touch_the_second() {
        let (_sender, pty_output) = pty_output_with_queue(queue_split_in_two(b"0123456789", 6));

        let mut destination = Vec::new();
        assert!(
            pty_output
                .pick_up(&mut destination, 4, Silences::default())
                .left_a_queue,
            "six bytes were left"
        );
        assert_eq!(destination, b"0123", "the maximum was not respected");

        // And what was left is still astride the wrap, whole and in its order.
        let mut rest = Vec::new();
        assert!(
            !pty_output
                .pick_up(&mut rest, usize::MAX, Silences::default())
                .left_a_queue
        );
        assert_eq!(rest, b"456789");
    }

    #[test]
    fn the_maximum_that_falls_past_the_wrap_takes_from_both_stretches() {
        let (_sender, pty_output) = pty_output_with_queue(queue_split_in_two(b"0123456789", 6));

        let mut destination = Vec::new();
        assert!(
            pty_output
                .pick_up(&mut destination, 8, Silences::default())
                .left_a_queue,
            "two bytes were left"
        );

        // Six from the first stretch and two from the second: the cut falls inside the second,
        // which is where a badly done count goes out of range or leaves bytes behind.
        assert_eq!(destination, b"01234567");

        let mut rest = Vec::new();
        assert!(
            !pty_output
                .pick_up(&mut rest, usize::MAX, Silences::default())
                .left_a_queue
        );
        assert_eq!(rest, b"89");
    }

    #[test]
    fn what_is_picked_up_says_since_when_it_was_waiting() {
        // It is the stamp from which hangs the number this milestone publishes as visual delay.
        // Without it, the wait —which in a dump is more than a second— cannot be measured, and the
        // rest of the stretches are microseconds: the number would say the application goes
        // instantaneous exactly when it is seen lagging behind.
        let (_pty, output) = Pty::spawn("sh", &["-c", "printf hola"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});
        assert!(wait_for(|| !pty_output.is_alive()));

        std::thread::sleep(Duration::from_millis(50));
        let mut destination = Vec::new();
        let pickup = pty_output.pick_up(&mut destination, usize::MAX, Silences::default());

        let waiting_since = pickup
            .waiting_since
            .expect("bytes were picked up and it did not say since when they were waiting");
        assert!(
            waiting_since.elapsed() >= Duration::from_millis(50),
            "the wait came out shorter than the test slept"
        );
    }

    #[test]
    fn a_pickup_on_empty_does_not_invent_a_wait_of_zero() {
        // A frame with no output is not an instantaneous wait: it is that there was nothing to wait
        // for. Returning zero would put a zero in the series and would lower the published average
        // with frames in which nothing happened.
        let (_pty, output) = Pty::spawn("sh", &["-c", "sleep 30"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        let mut destination = Vec::new();
        let pickup = pty_output.pick_up(&mut destination, usize::MAX, Silences::default());

        assert!(destination.is_empty());
        assert!(pickup.waiting_since.is_none());
    }

    #[test]
    fn arrivals_already_served_do_not_accumulate() {
        // During a dump thousands of batches come in. If the marks were not purged on serving them,
        // the list would grow non-stop and searching it would get more expensive each frame —
        // inside the lock that the program's thread also needs.
        let (sender, pty_output) = pty_output_with_queue(VecDeque::new());
        for _ in 0..50 {
            sender
                .send(b"abcd".to_vec())
                .expect("the bridge is still alive");
        }

        let mut destination = Vec::new();
        assert!(wait_for(|| {
            pty_output.pick_up(&mut destination, usize::MAX, Silences::default());
            destination.len() == 200
        }));

        let queue = pty_output
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        assert!(
            queue.arrivals.is_empty(),
            "{} marks of already-served batches were left",
            queue.arrivals.len()
        );
    }

    #[test]
    fn the_destination_keeps_what_it_already_had() {
        // The frame reuses its buffer and may call with remnants of the previous pass still
        // unprocessed. If picking up overwrote them, bytes already read from the program would be lost.
        let (_pty, output) = Pty::spawn("sh", &["-c", "printf dos"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        let mut destination = b"antes".to_vec();
        assert!(wait_for(|| {
            pty_output.pick_up(&mut destination, usize::MAX, Silences::default());
            destination.len() > b"antes".len()
        }));
        assert!(
            destination.starts_with(b"antes"),
            "what was already there was lost"
        );
    }

    #[test]
    fn the_bridge_queue_does_not_grow_without_limit() {
        // Before the fix, a program that writes non-stop and that no one picks up made this queue
        // grow without a brake: with `cat /dev/zero` hundreds of MiB per second were measured in
        // the diagnosis. Here the program is left writing and **`pick_up` is not called at any
        // moment** during the wait: calling it even once would make room for the thread and it
        // would keep picking up, with which the drainage would be measured and not the ceiling.
        let (pty, output) = Pty::spawn("cat", &["/dev/zero"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});

        // At the hundreds of MiB/s measured, half a second of margin is more than enough for the
        // queue to reach the ceiling if the brake works, and for it to shoot far above the bound if
        // it does not.
        std::thread::sleep(Duration::from_millis(500));

        // The queue size is looked at with the lock, directly, and not with `pick_up`:
        // `pick_up(destination, usize::MAX)` would take the whole queue and make room for the
        // thread, so it would only serve to measure once — and this check needs to see the queue
        // while the thread is still blocked waiting, not after having emptied it.
        let accumulated = pty_output
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .bytes
            .len();

        // Upper bound: above the bridge's ceiling (1 MiB) there is margin that the brake here does
        // not control directly, and it has to be accounted for so as not to chase an exact number.
        // From nearest to farthest in the chain: the chunk that this very thread has "in hand"
        // while it waits, not enqueued yet (up to 8 KiB, the size of a `term-core` read); the
        // `sync_channel(8)` of the crate below, with up to eight 8 KiB reads in transit (64 KiB);
        // the chunk that the `term-core` reader thread in turn has in hand trying to deliver it
        // (another 8 KiB); and the pseudoterminal's kernel buffer, on the order of 20 KiB according
        // to what was measured in the investigation of this failure. Summed, something above
        // 1.1 MiB. It is left at up to 4 MiB so as not to depend on fine timing, and with that to
        // spare: at the rates measured in the diagnosis, without a brake the same 500 ms would have
        // accumulated several tens of MiB, not a few.
        const UPPER_BOUND: usize = 4 * 1024 * 1024;

        assert!(
            accumulated >= QUEUE_CEILING,
            "the queue did not even reach the ceiling ({QUEUE_CEILING} bytes) in 500 ms: only {accumulated} bytes; \
             the test is not subjecting the bridge to real pressure"
        );
        assert!(
            accumulated <= UPPER_BOUND,
            "the queue grew far above the ceiling and its margin: {accumulated} bytes \
             accumulated in 500 ms, bound {UPPER_BOUND} bytes — the brake that prevents this \
             queue from growing without limit is not acting"
        );

        // The pseudoterminal is closed so the program stops writing, and picking up continues until
        // the thread notices and finishes: this way the death check, which is what the rest of the
        // module lives off, is done on a bridge that has come out of the wait properly and not by
        // the emergency stop of the next test.
        drop(pty);
        let mut rest = Vec::new();
        assert!(wait_for(|| {
            pty_output.pick_up(&mut rest, usize::MAX, Silences::default());
            !pty_output.is_alive()
        }));
    }

    /// The thread of this module has **two** ways of going still, and only one is resolved by the
    /// channel: reading, it finds out the program has died because the channel closes; waiting for
    /// room, it does not look at the channel at all. As room is only made by whoever picks up, a
    /// destroyed bridge with the queue at the ceiling would leave its thread spinning forever — a
    /// leak for each pane that is closed at that instant, invisible today only because the process
    /// dies whole behind it.
    ///
    /// It is observed with a witness captured by the waker instead of by counting threads: the
    /// waker is destroyed **with** the thread, so its reference count dropping to one proves that
    /// the thread finished for real. Counting the process's threads would measure, by rebound,
    /// those of the rest of the suite.
    #[test]
    fn destroying_the_bridge_with_the_queue_full_ends_its_thread() {
        let witness = Arc::new(());
        let thread_witness = Arc::clone(&witness);

        // The `Pty` is kept alive on purpose throughout the test: if it closed, the thread would
        // exit through the usual path and this would pass even if the stop did not exist.
        let (_pty, output) = Pty::spawn("cat", &["/dev/zero"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, move || {
            let _ = &thread_witness;
        });

        // Without picking up even once: the queue reaches the ceiling and the thread stays waiting
        // for room.
        assert!(
            wait_for(|| {
                pty_output
                    .queue
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .bytes
                    .len()
                    >= QUEUE_CEILING
            }),
            "the queue did not reach the ceiling: the test is not leaving the thread where it has to be"
        );

        drop(pty_output);

        assert!(
            wait_for(|| Arc::strong_count(&witness) == 1),
            "the thread is still alive after destroying the bridge: it stayed waiting for a room \
             that no one is going to make for it any more"
        );
    }

    /// A pseudoterminal is not needed for any of what follows: the sentence is a decision over a
    /// value, and the value is the whole of what the reading thread reports.
    fn failed(errno: i32) -> Option<term_core::Ending> {
        let failure = std::io::Error::from_raw_os_error(errno);

        Some(term_core::Ending::Failed {
            kind: failure.kind(),
            os_error: failure.raw_os_error(),
        })
    }

    /// A failure with no number, which the value's shape allows and no known source produces.
    fn failed_namelessly() -> Option<term_core::Ending> {
        Some(term_core::Ending::Failed {
            kind: std::io::ErrorKind::InvalidData,
            os_error: None,
        })
    }

    #[test]
    fn a_failed_read_is_said_with_the_kind_and_the_number_the_system_gave() {
        let mut quiet = QuietConsole::default();

        // **Errno 70, and the number is the point.** It is what a device or a driver answers that
        // the standard library has no name for, so the kind flattens to a word user code cannot
        // even name and only the number says what happened. `EIO` would be the obvious choice and
        // is the wrong one: the pseudoterminal library maps it to end of file before this arm.
        assert_eq!(
            quiet.look(failed(70)).as_deref(),
            Some(
                "agent-terminal: this console stopped receiving: read failed (Uncategorized, os \
                 error 70)"
            )
        );
    }

    #[test]
    fn a_failure_the_system_gave_no_number_for_is_still_said() {
        // The number is an `Option` in the value this reads, so the sentence has to hold together
        // without one. Nothing under the pseudoterminal is known to produce this, which is why the
        // arm exists as a shape and not as a scenario.
        let mut quiet = QuietConsole::default();

        assert_eq!(
            quiet.look(failed_namelessly()).as_deref(),
            Some("agent-terminal: this console stopped receiving: read failed (InvalidData)")
        );
    }

    #[test]
    fn a_failed_read_is_said_once_and_the_frames_after_it_say_nothing() {
        let mut quiet = QuietConsole::default();
        let ending = failed(70);

        assert!(quiet.look(ending).is_some());

        assert_eq!(quiet.look(ending), None, "it was said twice");
    }

    #[test]
    fn the_three_endings_that_are_not_a_failure_say_nothing_on_this_channel() {
        // One assertion per reason, and each for its own: nothing recorded is a console still
        // reading; an ordinary ending is not a silence; the runtime's own hook has already printed
        // a panic with its file and its line; and a vanished listener is only reachable while the
        // pane is being destroyed, with no console left for a line to be about.
        for ending in [
            None,
            Some(term_core::Ending::EndedWell),
            Some(term_core::Ending::Panicked),
            Some(term_core::Ending::NobodyListening),
        ] {
            let mut quiet = QuietConsole::default();

            assert_eq!(quiet.look(ending), None, "{ending:?} was said");
        }
    }

    #[test]
    fn a_silent_ending_does_not_spend_the_sentence_a_later_failure_is_owed() {
        // The guard against the cheapest way of passing the test above: marking it said whatever
        // arrives. A console reads `EndedWell` on no frame before it fails —the value is written
        // once— but a reading that hands over any of the silent ones must leave the sentence unspent.
        let mut quiet = QuietConsole::default();

        assert_eq!(quiet.look(Some(term_core::Ending::EndedWell)), None);

        assert!(
            quiet.look(failed(70)).is_some(),
            "an ordinary ending spent the sentence the failure was owed"
        );
    }

    #[test]
    fn the_pickup_the_frame_already_makes_is_what_spends_it() {
        let (sender, output) = channel();
        drop(sender);
        let pty_output = PtyOutput::new(output, || {});
        let mut destination = Vec::new();
        let ending = failed(70);

        // Two frames, because that is what the promise is about: a console that failed is still
        // failed on the next pass, and the second pickup must not say it again.
        for _ in 0..2 {
            pty_output.pick_up(
                &mut destination,
                usize::MAX,
                Silences {
                    deaf: None,
                    quiet: ending,
                },
            );
        }

        // If the pump did not look, the sentence would still be owed. This is the whole binding
        // between the decision and the pass that runs once per frame per console.
        assert_eq!(
            pty_output.quiet.borrow_mut().look(ending),
            None,
            "the pickup did not look at how the reading ended"
        );
    }

    #[test]
    fn what_the_frame_hands_over_is_read_off_the_pseudoterminal_and_not_invented() {
        // The seam between the two halves. Without this, replacing either field of `Silences::of`
        // with `None` leaves every other test in this module green while no console ever gets a
        // line: the rest of them build the value by hand.
        let (pty, output) = Pty::spawn("sh", &["-c", "exit 0"], Size { rows: 24, cols: 80 })
            .expect("no pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});
        assert!(
            wait_for(|| !pty_output.is_alive()),
            "the console did not end"
        );

        let silences = Silences::of(&pty);

        assert_eq!(
            silences.quiet,
            pty.ending(),
            "the ending handed to the pump is not the one the pseudoterminal recorded"
        );
        assert_eq!(
            silences.quiet,
            Some(term_core::Ending::EndedWell),
            "a program that exited was not recorded as an ordinary ending"
        );
        // **The other half is deliberately not asserted here.** A console nobody typed into owes
        // nothing, so `backlog()` answers `None` and an assertion against it would be satisfied by
        // hardcoding `None` in the seam — measured, the mutation survives. Binding it needs a
        // program that stops reading and enough bytes owed to it, which is the input side's own
        // scenario and has no test in this crate at all. In the debt log.
    }

    #[test]
    fn a_bridge_whose_thread_lets_go_of_the_receiver_records_nobody_listening() {
        // **The order that must not happen, made to happen on purpose.** The crate below says,
        // where it matters, that it cannot enforce this: telling a close from a vanished listener
        // rests on the stop flag going up before the receiving end disappears, and the receiving end
        // belongs to whoever hosts the console. Here the pseudoterminal is kept alive and the
        // bridge's thread is made to let go of it, which is the reversal — and the reading reports
        // it as what it is instead of as a program that finished.
        //
        // **The queue has to be at its ceiling for the thread to exit at all**: dropping the bridge
        // raises a flag the thread only consults while waiting for room. That is the one exit that
        // takes the receiver with it, and it is why this arm exists at all — a review found the
        // exit after a first reading had ruled it out.
        let (pty, output) = Pty::spawn("cat", &["/dev/zero"], Size { rows: 24, cols: 80 })
            .expect("could not open the pseudoterminal");
        let pty_output = PtyOutput::new(output, || {});
        assert!(
            wait_for(|| {
                pty_output
                    .queue
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .bytes
                    .len()
                    >= QUEUE_CEILING
            }),
            "the queue did not reach the ceiling: the thread is not where the test needs it"
        );

        drop(pty_output);

        assert!(
            wait_for(|| pty.ending().is_some()),
            "the reading never ended after its listener went away"
        );
        assert_eq!(
            pty.ending(),
            Some(term_core::Ending::NobodyListening),
            "a listener that vanished on its own was recorded as something else"
        );
    }

    #[test]
    fn a_pickup_over_a_console_that_is_still_reading_spends_nothing() {
        let (sender, output) = channel();
        drop(sender);
        let pty_output = PtyOutput::new(output, || {});
        let mut destination = Vec::new();

        pty_output.pick_up(&mut destination, usize::MAX, Silences::default());

        // The other half of the binding: a pickup that marks it said whatever the ending is would
        // satisfy the test above and swallow the sentence of every console that fails afterwards.
        assert!(
            pty_output.quiet.borrow_mut().look(failed(70)).is_some(),
            "a pickup with nothing recorded spent the sentence a later failure is owed"
        );
    }
}

/// Says it, once per console, when the program has stopped taking what was typed into it.
///
/// **This exists because writing stopped blocking, and the failure it replaced was at least
/// visible.** Since the write is handed to a thread of its own, a program that stops reading for
/// good no longer freezes the window: it leaves a console that repaints and quietly ignores
/// everything typed, `Ctrl+C` included. Nothing on screen says so, and from the outside it reads as
/// the terminal being broken.
///
/// **It decides and it says it; it does not fix it.** There is nothing to fix from here — the bytes
/// are owed to a program that is not asking for them — so what is owed is what gets published.
///
/// **Once, and only once per console.** It runs on every frame, and a line per frame would bury the
/// error output of a program that has done nothing wrong. The same rule the family ceiling and the
/// clashing shortcut already follow.
#[derive(Default)]
pub struct DeafConsole {
    said: bool,
}

/// How long the oldest unwritten byte has to have been waiting before it is worth saying.
///
/// **A second, and it is chosen against the numbers this project already measured**, not for taste:
/// a program that is merely busy takes milliseconds to come back for its input —a full paste of
/// 5 MiB was measured going in in 24 ms once the reader drained— so a whole second of a byte sitting
/// unwritten is not a busy program, it is one that has stopped listening.
const WORTH_SAYING: Duration = Duration::from_secs(1);

impl DeafConsole {
    /// Looks at what the program has not taken, and returns the sentence to say the first time it
    /// deserves saying.
    ///
    /// **It returns the sentence instead of printing it**, like the rest of this crate's decisions:
    /// printing cannot be observed from inside the process, so what a test can hold on to is what
    /// was decided.
    pub fn look(&mut self, backlog: Option<term_core::Backlog>) -> Option<String> {
        let backlog = backlog?;
        if self.said || (backlog.waiting < WORTH_SAYING && !backlog.gave_up) {
            return None;
        }
        self.said = true;
        Some(if backlog.gave_up {
            format!(
                "agent-terminal: this console can no longer be typed into: the program stopped \
                 accepting input with {} byte(s) still owed to it",
                backlog.bytes
            )
        } else {
            format!(
                "agent-terminal: the program in this console is not taking what you type: {} \
                 byte(s) have been waiting {:?}. Everything typed since then, Ctrl+C included, is \
                 queued behind it",
                backlog.bytes, backlog.waiting
            )
        })
    }
}
