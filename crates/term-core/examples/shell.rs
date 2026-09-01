//! Spawns a shell in a pseudoterminal, sends it `ls` and prints the raw output.
//!
//! It is the by-hand check of the milestone: `cargo run --example shell -p term-core`. The output
//! comes out exactly as the shell writes it, escape sequences included — interpreting them is the
//! job of another layer, and seeing them raw here is precisely the proof that they arrive whole.

use std::io::{self, Write};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use term_core::{Pty, Size};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut pty, pty_output) = Pty::spawn("zsh", &["-l"], Size::default())?;
    pty.write(b"ls\n")?;

    // The shell is still alive waiting for more commands, so the channel won't close on its own:
    // we read for a while and then exit.
    let until = Instant::now() + Duration::from_secs(2);
    let output = io::stdout();
    let mut output = output.lock();
    while Instant::now() < until {
        match pty_output.recv_timeout(Duration::from_millis(100)) {
            Ok(bytes) => {
                output.write_all(&bytes)?;
                output.flush()?;
            }
            // The shell died on its own (an `exit`, a failure to start): there is nothing more to
            // read and waiting out the deadline would only drag out the exit for no reason.
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => continue,
        }
    }

    // Dropping `pty` kills the shell and waits for the reader thread: no orphaned process is left.
    Ok(())
}
