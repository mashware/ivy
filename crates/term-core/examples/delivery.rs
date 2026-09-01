//! Measures how much the pseudoterminal delivers to a program that prints lines, with and without
//! the kernel's output processing.
//!
//! ```text
//! cargo run --release --example delivery -p term-core
//! ```
//!
//! The two numbers come from the **same** string and the same machine: the program inside turns the
//! switch on or off by itself with `stty`, so no API is needed to measure the difference, and what
//! is compared does not depend on two separate runs.
//!
//! **What costs is each line break, not each byte**, so the ratio depends on how many there are per
//! byte — that is why it is measured with two line lengths and not one. A single number looks
//! universal and is not.
//!
//! **This cost is paid on purpose.** We tried to remove it by turning off `OPOST` when opening the
//! pseudoterminal, and reverted it: putting the carriage return back from outside requires knowing
//! whether the program wants it, and that information disappears the moment you lie to it about the
//! state of its terminal. The full autopsy is in **D12** of `docs/02-decisiones.md`. The example
//! stays because the cost is still there and it is worth being able to look at it again.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use term_core::{Pty, Size};

/// Size of the file that is dumped. Twelve megs is a `cat` of a real log.
const BYTES: usize = 12 * 1024 * 1024;

/// Margin per read. It is not the total time: it is how long we wait for *something more* to arrive
/// before considering the delivery finished.
const DEADLINE: Duration = Duration::from_secs(60);

fn main() {
    // A file is dumped with `cat` and the lines are not generated with a shell loop. Measured: a
    // `while` of `sh` with `printf` delivers 12 MiB/s **with the switch set both ways**, because the
    // bottleneck is the loop itself. With that, the measurement gave "1×" and would have buried the
    // ticket's finding under a reassuring, false number.
    for (tag, line) in [
        (
            "log line   (70 B)",
            &b"lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod\n"[..],
        ),
        ("short line (2 B)", &b"y\n"[..]),
    ] {
        let file = seed(line);
        let without_lines = measure(&file, "stty -opost");
        let with_lines = measure(&file, "stty opost onlcr");
        let _ = std::fs::remove_file(&file);

        println!("{tag} — {} lines", BYTES / line.len());
        report("processing in the kernel", &with_lines);
        report("without kernel processing", &without_lines);
        println!(
            "  ratio: {:.0}×\n",
            with_lines.took.as_secs_f64() / without_lines.took.as_secs_f64()
        );
    }
}

/// What it cost to dump the file, and how much actually arrived on the other side.
struct Delivery {
    took: Duration,
    delivered: usize,
}

/// The throughput is computed over what the **program wrote**, not over what was delivered.
///
/// They are not the same and that is why it matters: with processing on, more bytes arrive than
/// were written —a carriage return per line break, and two if it is also translated here—, so
/// dividing by what was delivered would reward the slow configuration for padding the count.
fn report(tag: &str, delivery: &Delivery) {
    let written = BYTES as f64 / (1024.0 * 1024.0);
    let delivered = delivery.delivered as f64 / (1024.0 * 1024.0);
    println!(
        "  {tag:<26}: {:>9.3?}  →  {:>6.1} MiB/s   ({delivered:.1} MiB delivered)",
        delivery.took,
        written / delivery.took.as_secs_f64(),
    );
}

/// Writes the file that will be dumped.
fn seed(line: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join("term-core-delivery.txt");
    let mut file = std::fs::File::create(&path).expect("create the measurement file");
    let mut written = 0;
    while written < BYTES {
        file.write_all(line).expect("write the file");
        written += line.len();
    }
    file.flush().expect("close the file");
    path
}

fn measure(file: &Path, variant: &str) -> Delivery {
    let command = format!("{variant}; cat {}", file.display());
    let (_pty, channel) = Pty::spawn("sh", &["-c", &command], Size::default()).expect("spawn sh");

    // The clock starts with the first byte, not before: starting the shell and opening the file is
    // not what is being measured.
    let first = channel
        .recv_timeout(DEADLINE)
        .expect("the program did not write");
    let start = Instant::now();
    let mut delivered = first.len();
    while let Ok(chunk) = channel.recv_timeout(DEADLINE) {
        delivered += chunk.len();
    }

    Delivery {
        took: start.elapsed(),
        delivered,
    }
}
