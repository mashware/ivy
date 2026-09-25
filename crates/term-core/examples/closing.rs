//! Reproduces what closing a console costs, and what makes it never finish.
//!
//! It is an example and not a test **for the same reason `hang.rs` is one**: a test that reproduces a
//! hang does not go red, it takes the suite with it. Run it by hand:
//!
//! ```text
//! cargo run --release --example closing -p term-core
//! ```
//!
//! What it does: launches a program in a pseudoterminal, waits until **the right party** says it is
//! up, and then **times `Drop for Pty`** — on a thread of its own, answering through a channel, so a
//! `Drop` that never returns is reported instead of hanging this example. That technique is this
//! crate's own precedent (`drain_until_closed`, and `hang.rs` beside it).
//!
//! **A marker somebody prints, never a fixed wait, and never «any output».** Chained with `;` the
//! shell carries on when a command fails and the row is measured against a program that never got
//! where it was supposed to be; three false greens were measured that way once. Accepting *any*
//! output is the same mistake wearing a better disguise: a `stty` that failed writes a diagnostic
//! into the pseudoterminal, which satisfies «something arrived» while the state the row needs was
//! never reached. Every row here waits for a **named marker**, and the rows with a survivor wait for
//! the marker **the survivor itself prints** — seeing it come out of the master is proof that the
//! survivor exists *and* holds the slave end, which is the whole premise. A backgrounded list exits
//! 0 whatever happens inside it, so nothing else would prove it.
//!
//! **Nothing here reads a cause off the clock.** Each row declares what it is measuring, because that
//! is known from the scenario; the elapsed time is then checked against the band that claim implies,
//! and a row outside its band is printed as such and makes the example exit non-zero. Inferring the
//! label from the duration was the first version's mistake: on a busy machine an ordinary close at
//! 130 ms would have been published as «somebody survived», and this crate's own deadline would have
//! been published as the program's own dying.
//!
//! The seven rows, in the order they run:
//!
//! | Row | Program | What it measures |
//! |---|---|---|
//! | `a live program` | a program alive and quiet | the ordinary close — the cost paid on every console |
//! | `already dead` | a program that has exited | the floor: what `Drop` costs with nothing to kill |
//! | `ignores SIGHUP` | `trap '' HUP` | what the grace after the hang-up costs |
//! | `grandchild, echo on` | a survivor, echo left on | the accidental rescue: it finishes anyway |
//! | `grandchild, -echo` | the same, `stty -echo` alone | **the hang**, with exactly one flag changed |
//! | `grandchild, raw -echo` | the same, `stty raw -echo` | the hang with the whole raw mode, as a TUI sets it |
//! | `stopped child` | a shell that SIGSTOPs itself | that the wait for the child is bounded too |
//!
//! The last three rows are the experiment and the fourth is their control: **same process structure,
//! and only the terminal's flags differ**.

use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::{Duration, Instant};
use term_core::{Pty, Size};

/// How long the programs stay alive.
///
/// Long enough that none of them ends on its own inside the deadline below — otherwise a row would
/// «finish» because the program died and not because `Drop` returned, which is the disguise that
/// would publish this bug as fixed. And short enough that what this example leaves behind is gone
/// within seconds: the survivors are deliberately out of reach of the close, so they outlive the run.
const SECONDS_ALIVE: u32 = 15;

/// How long a `Drop` is given before it is called a hang.
///
/// **It has to sit between the two facts it separates**: comfortably above the ~200 ms the grace
/// after the hang-up costs, and far below the seconds the programs live. At five seconds a hang is
/// reported as one after five seconds; with a deadline longer than `SECONDS_ALIVE` every row would
/// return and the example would print «nothing hangs» on a build that hangs.
const DEADLINE: Duration = Duration::from_secs(5);

/// What the program itself prints once it is up.
const READY: &str = "up";

/// What a **survivor** prints once it is up.
///
/// It is a marker of its own because it proves something the other one cannot: that the process the
/// row's whole point depends on exists, and that it can write through the slave end it inherited.
const HELD: &str = "held";

/// What a row is measuring, declared by the row and never deduced from the clock.
///
/// Each variant carries the sentence that explains the number and the band that sentence implies. The
/// band is a **check**, not a classifier: a row is only ever checked against its own.
#[derive(Clone, Copy)]
enum Expected {
    /// Nothing left to kill: the program had already gone before the close.
    Floor,
    /// The program dying after the hang-up, looked at every millisecond.
    Dying,
    /// That and this crate's abandon deadline: a holder of the slave end survived.
    DyingAndGiveUp,
    /// The whole grace after the hang-up, before resorting to SIGKILL.
    Grace,
}

impl Expected {
    /// What the number is made of. It comes from the scenario, so it is true before anything is timed.
    fn note(self) -> &'static str {
        match self {
            Expected::Floor => "nothing left to kill",
            Expected::Dying => "the program dying after SIGHUP",
            Expected::DyingAndGiveUp => "its dying plus this crate's abandon deadline",
            Expected::Grace => "the whole grace after SIGHUP, paid on the caller's thread",
        }
    }

    /// The milliseconds that claim implies, as `(at least, at most)`.
    ///
    /// The pieces, each measured: ~0 with nothing to kill; a millisecond or two for a program to die
    /// of the hang-up, looked at every millisecond; +100 ms of this crate's abandon deadline when a
    /// holder survives; and the 200 ms of grace when the program ignores SIGHUP. The ceiling of
    /// `Dying` is what catches a return of `portable-pty`'s own `kill`, whose 50 ms between looks
    /// was this row's whole cost — 50.72 ms median when this crate still called it. The margins are
    /// otherwise wide because this is a check for a number that does not match its own explanation,
    /// not a performance assertion.
    fn band(self) -> (f64, f64) {
        match self {
            Expected::Floor => (0.0, 20.0),
            Expected::Dying => (0.0, 20.0),
            Expected::DyingAndGiveUp => (100.0, 160.0),
            Expected::Grace => (200.0, 320.0),
        }
    }
}

/// A program with a survivor behind it, and both of them announcing themselves.
///
/// **The brace group is load-bearing and the first version of this got it wrong.** Written as
/// `stty … && printf … && setsid … & exec …`, the `&` backgrounds the WHOLE `&&` list, so `stty` runs
/// in a subshell while the main shell is already `exec`ing — the flags were never applied and the row
/// silently measured the row above it. With a brace group only the `setsid` goes to the background.
///
/// The survivor is a shell of its own so that it can print [`HELD`] before sleeping. Waiting for that
/// marker is what turns «a survivor should exist» into «a survivor exists»: `{ … & }` exits 0 even
/// when the command inside it is missing — measured — so on a host without `setsid(1)`, which is
/// every macOS, the row would otherwise reach `exec` with nobody holding the slave and quietly
/// measure the easy case.
fn with_survivor(flags: &str) -> String {
    format!(
        "{flags}printf {READY} && {{ setsid sh -c 'printf {HELD}; sleep {SECONDS_ALIVE}' & }} \
         && exec sleep {SECONDS_ALIVE}"
    )
}

fn main() {
    println!(
        "closing a console, measured. deadline {} s, programs live {} s\n",
        DEADLINE.as_secs(),
        SECONDS_ALIVE
    );
    println!("{:<24} {:>12}  what it means", "row", "Drop took");
    println!("{:-<78}", "");

    let mut off_band = 0;

    // A program alive and quiet: the ordinary case, paid on every console anybody closes. There is no
    // shell left after the `exec` — which is the point, since a shell of its own would be a second
    // process to kill.
    off_band += measure(
        "a live program",
        &format!("printf {READY} && exec sleep {SECONDS_ALIVE}"),
        READY,
        false,
        Expected::Dying,
    );

    // The floor. `exec true` leaves no program to kill, so what is left is the bookkeeping.
    off_band += measure(
        "already dead",
        &format!("printf {READY} && exec true"),
        READY,
        true,
        Expected::Floor,
    );

    // Ignoring the hang-up closing sends. The close then waits out the whole grace before resorting
    // to SIGKILL, and that wait is paid on the caller's thread.
    off_band += measure(
        "ignores SIGHUP",
        &format!("trap '' HUP && printf {READY} && exec sleep {SECONDS_ALIVE}"),
        READY,
        false,
        Expected::Grace,
    );

    // **The control the two rows below need, and it is a row and not a remark**: the same process
    // structure as they have, with the echo left on. So the only difference between this row and
    // those is the terminal's flags.
    //
    // It finishes, and by accident: on drop, `portable-pty`'s own master writer puts a `\n` and the
    // terminal's EOF character into the pseudoterminal (`UnixMasterWriter::drop`, `unix.rs:393-405`).
    // While the line discipline echoes, that comes straight back out of the master, the parked `read`
    // returns, and *then* the reader looks at the stop flag and leaves.
    off_band += measure(
        "grandchild, echo on",
        &with_survivor(""),
        HELD,
        false,
        Expected::Dying,
    );

    // **The hang, with exactly one flag changed from the row above.** With `-echo` there is nothing
    // to come back, so a reader parked in `read` with a survivor holding the slave has nothing left
    // to wake it — and this is the state a real console's program is in, because turning the echo off
    // is the first thing `vim`, `htop` or a TUI agent does.
    off_band += measure(
        "grandchild, -echo",
        &with_survivor("stty -echo && "),
        HELD,
        false,
        Expected::DyingAndGiveUp,
    );

    // **The same hang with the whole raw mode**, which is how a TUI actually sets it: `stty raw`
    // turns off canonical mode, `OPOST` and `ISIG` as well as the echo. It is here so that the row
    // above can be read as «the echo is the mechanism» — this one alone would prove the hang is
    // reachable without saying which flag removes the rescue.
    off_band += measure(
        "grandchild, raw -echo",
        &with_survivor("stty raw -echo && "),
        HELD,
        false,
        Expected::DyingAndGiveUp,
    );

    // **The sibling shape, and the reason it is a row and not a worry.** A *stopped* child never dies
    // of a signal it can hold, so `child.wait()` beside the reader's join has the same unbounded
    // shape and would block for ever — that is the adjacent risk. It does not happen: after its grace
    // the hang-up falls through to a real `SIGKILL`, which nobody holds, stopped processes included. So **the number below is that grace**, not the
    // cost of the wait: what the row proves is that the wait is reached and returns at all.
    off_band += measure(
        "stopped child",
        &format!("printf {READY} && kill -STOP $$ && exec sleep {SECONDS_ALIVE}"),
        READY,
        false,
        Expected::Grace,
    );

    if off_band > 0 {
        println!(
            "\n{off_band} row(s) did not cost what they say they cost. A number whose explanation \
             does not fit it is worse than no number: read them before quoting any of this."
        );
        std::process::exit(1);
    }
}

/// Launches the program, waits for `marker`, and times dropping the `Pty`.
///
/// Returns 1 if the row landed outside the band its [`Expected`] implies, so that the caller can make
/// the example exit non-zero. A row that could not be run at all counts the same way: a missing row
/// is not a passing row.
fn measure(
    row: &str,
    command: &str,
    marker: &str,
    wait_for_eof: bool,
    expected: Expected,
) -> usize {
    let size = Size { rows: 24, cols: 80 };
    let (pty, output) = match Pty::spawn("sh", &["-c", command], size) {
        Ok(pair) => pair,
        Err(problem) => {
            println!("{row:<24} {:>12}  {problem}", "not run");
            return 1;
        }
    };

    // Wait until the state the row needs is really there. Without this the `Drop` could be timed
    // against a program that had not started, which is the cheap case wearing the expensive one's
    // label — and where the marker is the survivor's, waiting for it is the only proof the survivor
    // exists at all.
    let give_up_at = Instant::now() + DEADLINE;
    let mut seen: Vec<u8> = Vec::new();
    while !seen.windows(marker.len()).any(|w| w == marker.as_bytes()) {
        match output.recv_timeout(give_up_at.saturating_duration_since(Instant::now())) {
            Ok(bytes) => seen.extend_from_slice(&bytes),
            Err(_) => {
                println!(
                    "{row:<24} {:>12}  nobody printed {marker:?} in {} s: {:?}",
                    "not run",
                    DEADLINE.as_secs(),
                    String::from_utf8_lossy(&seen)
                );
                return 1;
            }
        }
    }

    // For the row whose program exits, drain until the channel closes: that is end of file on the
    // master, which is proof the child released it rather than a guess that it had time to.
    if wait_for_eof {
        let gone = Instant::now() + DEADLINE;
        while output
            .recv_timeout(gone.saturating_duration_since(Instant::now()))
            .is_ok()
        {}
    }

    // **And then drained until the program goes quiet, which is the third thing this example got
    // wrong.** Keeping the receiver is not enough: the reader has escape routes that are not the
    // pseudoterminal at all. Stop draining and the bounded channel fills, so the thread is waiting
    // for room inside `deliver` — and `deliver` *does* consult the stop flag, so `Drop` pulls it out
    // and the `join` returns. Traced: in that shape the thread exits without ever issuing another
    // `read`. The hang needs the reader **parked inside `read`**, which is where a real console's
    // reader sits, and that means an empty channel.
    let quiet = Duration::from_millis(50);
    while output.recv_timeout(quiet).is_ok() {}

    // **The receiver is kept alive, and the first version of this example dropped it — which hid the
    // bug completely.** Dropping it makes the reader's next delivery fail, and *that* is a path on
    // which it does look at its stop flag and leave; so the `join` returned and every row came back
    // in 50 ms, including the one that is supposed to hang. The window does not drop its receiver
    // when it takes a console out of the tree: it stops draining it. That is what is reproduced here.
    let _keep_draining_stopped = output;

    // **`Drop` runs on a thread of its own so that a hang is a row and not the end of this example.**
    let (done, answer) = channel();
    let handle = std::thread::spawn(move || {
        let started = Instant::now();
        drop(pty);
        let _ = done.send(started.elapsed());
    });

    match answer.recv_timeout(DEADLINE) {
        Ok(took) => {
            let millis = took.as_secs_f64() * 1000.0;
            let (at_least, at_most) = expected.band();
            let within = (at_least..=at_most).contains(&millis);
            let note = if within {
                expected.note().to_string()
            } else {
                format!(
                    "OUT OF BAND — it says «{}», which would be {at_least:.0}-{at_most:.0} ms",
                    expected.note()
                )
            };
            println!("{row:<24} {millis:>10.2} ms  {note}");
            let _ = handle.join();
            usize::from(!within)
        }
        Err(RecvTimeoutError::Timeout) => {
            println!(
                "{row:<24} {:>12}  **HANGS** — still inside `Drop` after {} s",
                "no return",
                DEADLINE.as_secs()
            );
            // The thread is deliberately left behind: it is inside the very `join` this bug is
            // about, and waiting for it here would hang the example exactly as it hangs the window.
            1
        }
        Err(RecvTimeoutError::Disconnected) => {
            println!(
                "{row:<24} {:>12}  the thread died without answering: the close panicked",
                "no answer"
            );
            1
        }
    }
}
