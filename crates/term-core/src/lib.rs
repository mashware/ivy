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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
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
    writer: Box<dyn Write + Send>,
    reader: Option<JoinHandle<()>>,
    /// What pulls the reader thread out of a wait for room in the channel when it has to close.
    ///
    /// Without this, destroying the `Pty` while the channel is full **hangs the close**: the thread
    /// waits for a slot that would only appear if someone drained it, and the `join` below waits for
    /// the thread.
    stop: Arc<AtomicBool>,
}

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

        // The slave end is only needed by the child. Dropping it here is what makes the read of the
        // master reach end of file when the child dies; if it were kept open, the reader thread
        // would stay blocked forever.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;

        let (tx, output) = sync_channel(READS_IN_FLIGHT);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let reader = thread::spawn(move || {
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
                // The master is kept so it can be resized. Keeping it open does not prevent the
                // reader thread's end of file: that depends on nobody holding the slave end open,
                // and the only one who holds it is the child.
                master: pair.master,
                writer,
                reader: Some(reader),
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
    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
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
        // And then the usual order: killing the child is what closes its end of the
        // pseudoterminal and triggers the end of file in the reader thread. Waiting for the thread
        // before that would hang it, because it would still be blocked inside `read`.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;

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
}
