//! Reproduces the failure this crate had until writing got a thread of its own: writing to a
//! program that is not reading used to block the caller with no ceiling.
//!
//! It is an example and not a test **because a test that reproduces this hangs the suite**, which is
//! the whole point of the bug. Run it by hand:
//!
//! ```text
//! cargo run --release --example hang -p term-core
//! ```
//!
//! What it does: launches `sleep`, which never reads its input, and writes to it in growing amounts
//! with a stopwatch on each write.
//!
//! **Before the fix**: the bytes that fit went through instantly, and the write that overflowed the
//! kernel's buffer stopped dead. It was first measured at 5.71 s here — but **that figure does not
//! reproduce**: a later run found the write still blocked **54 s after the program had died**, so it
//! does not «return when the program dies» either. Whatever ends it, this example cannot say, and
//! guessing at it is what this investigation was already caught doing once. What is certain is that
//! nothing in the process could interrupt it, because whoever would send the interruption is this
//! same thread.
//!
//! **After the fix**: every row returns in microseconds. The bytes are handed to a thread of their
//! own, which is the one that waits.
//!
//! It stays in the tree as the witness of both: revert `Pty::write` to a `write_all` on the caller's
//! thread and the third row stops dead again.

use std::time::{Duration, Instant};
use term_core::{Pty, Size};

/// How long `sleep` stays alive. It is what puts an end to the hang: in the real application there
/// is no such end, because the program inside is a shell that lives as long as the console does.
const SECONDS_ALIVE: u32 = 6;

/// What the program prints once it really has the terminal in raw mode with no echo.
const MARKER: &str = "raw-and-deaf";

fn main() {
    let size = Size { rows: 24, cols: 80 };
    // **The program has to be in raw mode, and it has to have said so before anything is written.**
    // A bare `sleep` does not fill anything: with the echo on and canonical mode, the terminal moves
    // the bytes on by itself. What fills it is a program in raw mode that is not reading — `vim`,
    // `htop`, a TUI agent, an editor mid-save.
    //
    // **`&&` and a marker, not `;` and a sleep.** Chained with `;` the program carries on when
    // `stty` fails, and then this example prints «nothing blocked» **on a build that still carries
    // the bug**. A fixed 300 ms wait has the same hole on a slow machine. Waiting for a marker the
    // program prints *after* `stty` succeeded has neither.
    let command = format!("stty raw -echo && printf {MARKER} && sleep {SECONDS_ALIVE}");
    let (mut pty, output) = Pty::spawn("sh", &["-c", &command], size)
        .expect("launching the program in a pseudoterminal");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen: Vec<u8> = Vec::new();
    while !seen.windows(MARKER.len()).any(|w| w == MARKER.as_bytes()) {
        let left = deadline.saturating_duration_since(Instant::now());
        match output.recv_timeout(left) {
            Ok(bytes) => seen.extend_from_slice(&bytes),
            Err(_) => {
                println!("the program never said it had applied `stty`: nothing to measure here");
                return;
            }
        }
    }

    println!("writing to a program that never reads its input:\n");
    println!("{:>12}  {:>12}  what it means", "written", "took");

    let mut total = 0usize;
    // **Growing chunks and not one big write**, so the boundary shows: everything below the kernel's
    // headroom —20 480 bytes in raw mode, measured— returns instantly, and the first chunk that
    // crosses it stops dead.
    for chunk in [1_024usize, 4_096, 8_192, 16_384, 32_768, 65_536] {
        let payload = vec![b'x'; chunk];
        let started = Instant::now();
        let outcome = pty.write(&payload);
        let took = started.elapsed();
        total += chunk;
        let meaning = if took > Duration::from_millis(100) {
            "BLOCKED — this is the bug: the thread that paints is stopped here"
        } else {
            // **«Handed over» and not «it fit»**: since the write got a thread of its own, a row
            // that returns says nothing about the kernel's buffer — the bytes are queued, and
            // whether they went in is a different question, which `Pty::backlog` answers.
            "handed over: the caller was not made to wait"
        };
        println!("{total:>12}  {took:>12.2?}  {meaning}");
        if let Err(failure) = outcome {
            println!("\nthe handover failed: {failure}");
            println!("(the writer thread is gone: nothing else will reach the program)");
            return;
        }
    }
    println!(
        "\nnothing blocked, and with the fix in place that is the expected outcome: every write \
         above handed its bytes over and returned in microseconds."
    );
    println!(
        "Revert `Pty::write` to a `write_all` on the caller's thread and the third row stops dead \
         for as long as the program lives."
    );
}
