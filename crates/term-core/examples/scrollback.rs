//! Measures **what it costs to keep history**, which is what decides its cap.
//!
//! ```text
//! cargo run --release --example scrollback -p term-core
//! ```
//!
//! **It measures the engine below this crate, and that is on purpose.** `Screen` fixes history at
//! zero today, so measuring through its facade would require opening a hole in the production API
//! for an instrument. Here it is done the other way around: first the number, and the number
//! decides what API is needed.
//!
//! **In debug the three numbers come out several times worse.** The one that counts is the
//! `--release` one.
//!
//! The three questions it answers, and why each one:
//!
//! 1. **How much a line of history really takes up.** The cap is chosen in lines and paid in
//!    memory, so without this number the cap is a paper calculation — and there is no cap on
//!    splits, so it multiplies by however many consoles there are.
//! 2. **How much it costs to reflow with N stored lines.** Reflowing walks the **entire** history
//!    every time the number of columns changes, and that happens on **every frame** while a border
//!    is being dragged. Several points are measured: with just one, the extrapolation is an
//!    invented straight line.
//! 3. **Whether keeping history makes interpreting a dump more expensive.** It is the question that
//!    can reorder the milestone: if it rises, the tables of the D3 annex —with which this project
//!    judged its framework— have to be taken again.

use std::time::Instant;

use alacritty_terminal::Term;
use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Config;
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::vte::ansi::Processor;

/// The caps being compared. The first is today's —none— and the last, the one the engine ships by
/// default and that this measurement exists to avoid inheriting blindly.
///
/// **Without the 5,000 on purpose**: it distinguished nothing from the 3,500 above the noise, so it
/// was a row that reads like a data point and is not.
const CAPS: [usize; 4] = [0, 1_000, 3_500, 10_000];

/// How many times each point is repeated. **A single sample per point is not a measurement**: the
/// first version of this bench published three significant figures over a 20 % spread, and one step
/// of its table turned out to be noise. The **median** and the extremes are published.
const REPEATS: usize = 5;

/// How many columns it measures with. It is a realistic console width in a working window, and it
/// **matters**: what a line takes up and what it costs to reflow it grow with the columns.
const COLS: usize = 180;

/// How many rows the visible part has. It comes from the application's measurement runs.
const ROWS: usize = 49;

/// What is dumped to fill the history and to measure the interpreting.
const DUMP: usize = 8 * 1024 * 1024;

/// Chunk the grid is fed with, the size of the pseudoterminal read buffer.
const CHUNK: usize = 8192;

struct Dims {
    rows: usize,
    cols: usize,
}

impl Dimensions for Dims {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// Text with line breaks and the odd color escape: a single repeated character would measure the
/// parser's fast path and lie about the real case.
///
/// **And in two lengths, because the heaviest number depends on it.** A line shorter than the grid
/// **is not repositioned**: when narrowing, the engine pushes it whole and it leaves through the
/// cheap door, so a bench of short lines measures the reflow **that does not happen**. The first
/// version of this measured only the short one and published a number **four times** smaller than
/// the real one. Both regimes are real —a terminal session has both— so both are published and the
/// cap is chosen against the expensive one.
fn dump(wraps: bool) -> Vec<u8> {
    let short = b"\x1b[32mlorem ipsum dolor sit amet\x1b[0m consectetur adipiscing elit\n".to_vec();
    let pattern = if wraps {
        // A grid and a half: guarantees that each logical line takes up two physical ones and that
        // narrowing has to actually reposition it.
        let mut long = Vec::new();
        while long.len() < COLS * 3 / 2 {
            long.extend_from_slice(b"\x1b[32mlorem ipsum dolor\x1b[0m sit amet ");
        }
        long.push(b'\n');
        long
    } else {
        short
    };

    let mut bytes = Vec::with_capacity(DUMP + pattern.len());
    while bytes.len() < DUMP {
        bytes.extend_from_slice(&pattern);
    }
    bytes
}

/// The median and the extremes of a handful of samples, which is what is published for each point.
fn summary(
    mut samples: Vec<std::time::Duration>,
) -> (
    std::time::Duration,
    std::time::Duration,
    std::time::Duration,
) {
    samples.sort();
    (
        samples[samples.len() / 2],
        samples[0],
        samples[samples.len() - 1],
    )
}

fn with_cap(cap: usize) -> Term<VoidListener> {
    let config = Config {
        scrolling_history: cap,
        ..Config::default()
    };
    Term::new(
        config,
        &Dims {
            rows: ROWS,
            cols: COLS,
        },
        VoidListener,
    )
}

fn main() {
    let short = dump(false);
    let long = dump(true);
    let mib = short.len() as f64 / (1024.0 * 1024.0);

    println!("grid:     {ROWS}×{COLS} · dump of {mib:.1} MiB in {CHUNK} B chunks");
    println!(
        "samples:  {REPEATS} per point; the median is published and, in parentheses, the extremes\n"
    );

    // ── 1. What a cell takes up, and the cap that yields ─────────────────────────────────────────
    //
    // **The cell size is asked, not deduced from the layout**: summing the fields ignores the
    // padding the compiler puts between them, and the cap hangs off this number.
    //
    // **The rest of this section is a CALCULATION, not a measurement, and that is why it is said.**
    // The engine does not reserve the history in advance: its storage grows as it fills. So these
    // figures are **the cap** a console can come to occupy with the history full, not what it takes
    // up from the start.
    let cell = size_of::<Cell>();
    println!(
        "── what it takes up (measured: the cell; the rest is a calculation, and it is a CAP) ──"
    );
    println!("cell:     {cell} B  ← the only thing measured here");
    println!(
        "line:     {} B with {COLS} columns, not counting what the row reserves on top",
        cell * COLS
    );
    for cap in CAPS {
        let per_console = cell * COLS * cap;
        println!(
            "  cap {cap:>6}: up to {:>6.1} MiB per console · {:>6.1} MiB with four",
            per_console as f64 / (1024.0 * 1024.0),
            (per_console * 4) as f64 / (1024.0 * 1024.0)
        );
    }

    // ── 2. What it costs to reposition the text, in the two regimes ──────────────────────────────
    //
    // The history is genuinely filled before measuring, and the number of **columns** is changed,
    // which is what forces the repositioning. Round trip so as not to leave the grid growing to one
    // side and to dodge the engine's early exit when the size does not change.
    println!("\n── what it costs to reposition the text when the width changes ──");
    for (name, bytes) in [("short lines", &short), ("wrapping lines", &long)] {
        println!("  with {name}:");
        for cap in CAPS {
            let mut samples = Vec::with_capacity(REPEATS);
            for _ in 0..REPEATS {
                let mut term = with_cap(cap);
                let mut parser: Processor = Processor::new();
                for chunk in bytes.chunks(CHUNK) {
                    parser.advance(&mut term, chunk);
                }

                let rounds = 20;
                let start = Instant::now();
                for i in 0..rounds {
                    let cols = if i % 2 == 0 { COLS - 1 } else { COLS };
                    term.resize(Dims { rows: ROWS, cols });
                }
                samples.push(start.elapsed() / rounds);
            }
            let (mean, min, max) = summary(samples);
            println!(
                "    cap {cap:>6}: {mean:>10.3?} per width change  ({min:.3?} … {max:.3?}){}",
                if cap == 0 { "  ← today's" } else { "" }
            );
        }
    }

    // ── 3. Whether keeping history makes interpreting more expensive ─────────────────────────────
    //
    // **It is the question that can reorder the milestone.** If interpreting gets more expensive,
    // the tables of the published annexes were taken under another regime and have to be retaken.
    println!("\n── what it costs to interpret the dump ──");
    for cap in CAPS {
        let mut samples = Vec::with_capacity(REPEATS);
        for _ in 0..REPEATS {
            let mut term = with_cap(cap);
            let mut parser: Processor = Processor::new();
            let start = Instant::now();
            for chunk in short.chunks(CHUNK) {
                parser.advance(&mut term, chunk);
            }
            samples.push(start.elapsed());
        }
        let (mean, min, max) = summary(samples);
        println!(
            "  cap {cap:>6}: {mean:>10.3?} · {:>6.1} MiB/s  ({min:.3?} … {max:.3?}){}",
            mib / mean.as_secs_f64(),
            if cap == 0 { "  ← today's" } else { "" }
        );
    }
}
