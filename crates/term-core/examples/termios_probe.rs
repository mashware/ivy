//! Can the terminal ask the kernel which mode the program inside is in?
//!
//! Spawns two children through `portable-pty` and reads the *slave's* line discipline off the
//! **master** file descriptor, which is the premise the diagnosis of #160 rests on. The first child
//! is left in the default line discipline; the second turns it off with `stty raw -echo`. If the
//! premise holds, the two runs print different flags.
//!
//! Run with:
//!   cargo run -p term-core --example termios_probe

// **Unix only, and the docblock above is why.** `MasterPty`'s termios accessors are all
// `#[cfg(unix)]` in `portable-pty`, so on Windows this file would not build — and F2.7 is Windows.
// An example that breaks the workspace build on the platform it explains is worse than no example.
#![cfg(unix)]
use std::io::Read;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

fn probe(label: &str, script: &str) {
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");

    let mut cmd = CommandBuilder::new("sh");
    cmd.arg("-c");
    cmd.arg(script);
    let mut child = pair.slave.spawn_command(cmd).expect("spawn");
    drop(pair.slave);

    // Synchronise on the child's own output: once "ready" has come back, the script has run past
    // its `stty` and is sitting in `cat`'s read.
    let mut reader = pair.master.try_clone_reader().expect("reader");
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    // The word «failed» is the second way out: a script whose `stty` failed prints it instead of
    // `ready`, and without it this loop would sit in a read that `cat` never answers.
    while !seen.ends_with(b"ready") && !seen.ends_with(b"failed") && Instant::now() < deadline {
        let mut buf = [0u8; 64];
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => seen.extend_from_slice(&buf[..n]),
            Err(e) => {
                eprintln!("{label}: read error {e}");
                break;
            }
        }
    }

    match pair.master.get_termios() {
        Some(t) => {
            // `local_flags` is a bitflags set: its Debug spells the flags by name, so this probe
            // needs no dependency of its own to say whether ICANON is on.
            println!("{label}: fd={:?}", pair.master.as_raw_fd());
            println!("{label}: local_flags = {:?}", t.local_flags);

            let start = Instant::now();
            let rounds = 10_000;
            for _ in 0..rounds {
                std::hint::black_box(pair.master.get_termios());
            }
            let per = start.elapsed() / rounds;
            println!("{label}: get_termios costs {per:?} per call");
        }
        None => println!("{label}: get_termios() returned None"),
    }

    let _ = child.kill();
    let _ = child.wait();
}

fn main() {
    probe("default   ", "printf ready; exec cat");
    probe(
        "stty raw  ",
        "stty raw -echo && printf ready || printf failed; exec cat",
    );
}
