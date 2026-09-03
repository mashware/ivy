//! Reproducer of the failure: output that is not drained has no ceiling.
//!
//! It walks the **real** chain —the pseudoterminal, the bridge thread and the collection with the
//! per-frame budget— without opening a window, and publishes how much resident memory the process
//! occupies meanwhile. With no UI in between, what is seen is only the storage; if it grows here, the
//! painting does not cause it.
//!
//! **It has a safety cap on purpose.** The case it reproduces exhausts the machine's memory if left to
//! it: the pseudoterminal delivers two orders of magnitude faster than it is collected. On reaching the
//! cap it stops and declares itself reproduced — letting it run to the end would add no information and
//! would add a system hang.
//!
//! Linux only: the measurement comes from `/proc/self/status`.
//!
//! ```text
//! cargo run --release --example leak -p app -- urandom     # raw bytes without end
//! cargo run --release --example leak -p app -- zeros       # the same, without the cost of generating them
//! cargo run --release --example leak -p app -- yes         # two-byte lines, without end
//! cargo run --release --example leak -p app -- bounded     # ~10 MiB: it terminates
//! cargo run --release --example leak -p app -- no-bridge   # urandom, consuming the channel bare
//! ```

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use app::pty_output::PtyOutput;
use term_core::{Pty, Size};

/// Copied from `app::PUMP_BUDGET`, which is private. If it changes there, here it stops measuring the
/// same thing.
const BUDGET: usize = 128 * 1024;

/// The gap between collections: what a frame lasts at 60 per second.
const FRAME: Duration = Duration::from_millis(16);

/// How much resident memory is tolerated before declaring the failure reproduced and stopping.
///
/// It is not a fine threshold: it is a firewall. With the chain healthy the occupancy stays at a few
/// megabytes, so overrunning this can only mean that nothing is holding back the producer.
const CEILING: u64 = 512 * 1024;

/// When to give up waiting for it to grow, in a scenario that should terminate.
const DEADLINE: Duration = Duration::from_secs(30);

/// How often a row of the table is noted.
const SAMPLING: Duration = Duration::from_millis(500);

fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_else(|| "urandom".into());

    let (program, args): (&str, &[&str]) = match scenario.as_str() {
        // Raw bytes and endless, in big blocks: the load that really delivers faster than it is
        // collected. It is the issue's case.
        "urandom" | "no-bridge" => ("cat", &["/dev/urandom"]),
        // The same without the cost of generating randomness, in case the machine had it expensive and
        // what was measuring the limit were the generator and not the chain.
        "zeros" => ("cat", &["/dev/zero"]),
        // `yes` writes without stopping, but in two-byte lines: each break is processed by the
        // pseudoterminal's line discipline, and that slows it far below a block dump. It is here because
        // the issue names it and because the result is not the one expected.
        "yes" => ("yes", &[]),
        // The same shape of load but finished: ~10 MiB and end of file. It is the case F1.3 measured
        // and accepted, and it is here so the comparison does not depend on remembering it.
        "bounded" => (
            "sh",
            &[
                "-c",
                "yes 0123456789abcdefghijklmnopqrstuvwxyz | head -n 280000",
            ],
        ),
        other => {
            eprintln!("unknown scenario: {other}");
            eprintln!("use: urandom | zeros | yes | bounded | no-bridge");
            std::process::exit(2);
        }
    };

    let resident_at_start = resident_kib().expect("/proc/self/status readable: this is Linux");
    println!("scenario: {scenario}");
    println!("resident at start: {resident_at_start} KiB");
    println!();
    println!("  t (s)   resident (KiB)   growth (KiB)   collected (MiB)");

    let (pty, output) = Pty::spawn(program, args, Size { rows: 24, cols: 80 })
        .expect("the pseudoterminal opens and the program launches");

    let verdict = match scenario.as_str() {
        "no-bridge" => pump_direct(output, resident_at_start),
        _ => pump_through_bridge(output, resident_at_start),
    };

    // Destroying the pseudoterminal kills the child and closes the channel. Explicit and before
    // printing the verdict: with `yes` inside, leaving it for the end of `main` would be to keep feeding
    // the queue while the conclusion is written.
    drop(pty);

    println!();
    match verdict {
        Verdict::Reproduced { seconds, growth } => {
            println!(
                "REPRODUCED: resident memory grew {growth} KiB in {seconds:.1} s and \
                 passed the {CEILING} KiB cap without stabilizing."
            );
            println!(
                "Nobody held back the program inside. Without the cap, this continues until the machine is exhausted."
            );
        }
        Verdict::Terminated {
            seconds,
            growth,
            collected,
        } => {
            println!(
                "TERMINATED: the program closed its output at {seconds:.1} s after delivering \
                 {:.1} MiB; the maximum growth was {growth} KiB.",
                collected as f64 / (1024.0 * 1024.0)
            );
            println!(
                "This case ends on its own: it is the one F1.3 measured and accepted, and it is not the issue's."
            );
        }
        Verdict::Deadline { growth } => {
            println!(
                "NO VERDICT: the {} s deadline ran out without closing or passing the cap \
                 (growth {growth} KiB).",
                DEADLINE.as_secs()
            );
        }
    }
}

enum Verdict {
    /// Passed the memory cap without stabilizing: the failure is reproduced.
    Reproduced { seconds: f64, growth: u64 },
    /// The program closed its output on its own.
    Terminated {
        seconds: f64,
        growth: u64,
        collected: u64,
    },
    /// Neither one thing nor the other within the deadline.
    Deadline { growth: u64 },
}

/// The complete chain: bridge thread enqueuing, and one collection per frame with the budget.
///
/// It is what the application really does, with the painting removed. That the painting is not there is
/// precisely what makes the measurement conclusive: whatever grows here cannot have been caused by it.
fn pump_through_bridge(output: Receiver<Vec<u8>>, resident_at_start: u64) -> Verdict {
    let bridge = PtyOutput::new(output, || {});
    let mut received = Vec::with_capacity(BUDGET);
    let mut collected: u64 = 0;

    let started = Instant::now();
    let mut last_sample = Instant::now();
    let mut max_growth = 0;

    loop {
        received.clear();
        bridge.pick_up(&mut received, BUDGET);
        collected += received.len() as u64;

        let resident = resident_kib().unwrap_or(resident_at_start);
        let growth = resident.saturating_sub(resident_at_start);
        max_growth = max_growth.max(growth);

        if last_sample.elapsed() >= SAMPLING {
            row(started.elapsed(), resident, growth, collected);
            last_sample = Instant::now();
        }

        if growth > CEILING {
            return Verdict::Reproduced {
                seconds: started.elapsed().as_secs_f64(),
                growth,
            };
        }
        // The program has died and there is nothing left to collect: the only clean exit.
        if !bridge.is_alive() && received.is_empty() {
            return Verdict::Terminated {
                seconds: started.elapsed().as_secs_f64(),
                growth: max_growth,
                collected,
            };
        }
        if started.elapsed() > DEADLINE {
            return Verdict::Deadline { growth: max_growth };
        }

        std::thread::sleep(FRAME);
    }
}

/// No bridge: the pseudoterminal's channel is consumed directly, at the same rate.
///
/// It serves to separate the two queues on the path. Here the bridge's does not exist, so whatever
/// grows can only be the channel's that `Pty::spawn` returns.
fn pump_direct(output: Receiver<Vec<u8>>, resident_at_start: u64) -> Verdict {
    let mut collected: u64 = 0;
    let started = Instant::now();
    let mut last_sample = Instant::now();
    let mut max_growth = 0;

    loop {
        // The same budget the frame applies, counted in bytes and not in chunks.
        let mut of_this_frame = 0;
        while of_this_frame < BUDGET {
            match output.try_recv() {
                Ok(chunk) => {
                    of_this_frame += chunk.len();
                    collected += chunk.len() as u64;
                }
                Err(_) => break,
            }
        }

        let resident = resident_kib().unwrap_or(resident_at_start);
        let growth = resident.saturating_sub(resident_at_start);
        max_growth = max_growth.max(growth);

        if last_sample.elapsed() >= SAMPLING {
            row(started.elapsed(), resident, growth, collected);
            last_sample = Instant::now();
        }

        if growth > CEILING {
            return Verdict::Reproduced {
                seconds: started.elapsed().as_secs_f64(),
                growth,
            };
        }
        if started.elapsed() > DEADLINE {
            return Verdict::Deadline { growth: max_growth };
        }

        std::thread::sleep(FRAME);
    }
}

fn row(elapsed: Duration, resident: u64, growth: u64, collected: u64) {
    println!(
        "  {:5.1}   {resident:15}   {growth:17}   {:14.1}",
        elapsed.as_secs_f64(),
        collected as f64 / (1024.0 * 1024.0)
    );
}

/// Resident memory of the process, in KiB, read from `/proc/self/status`.
///
/// It is the only one of the memory measurements that answers the issue's question: not how much has
/// been reserved, but how much is really occupying the machine.
fn resident_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|number| number.parse().ok())
}
