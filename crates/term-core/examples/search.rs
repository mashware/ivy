//! Measures **what one press of Enter costs when the search finds nothing**, which is the worst
//! case: the engine walks the whole history once before it can say so.
//!
//! ```text
//! cargo run --release --example search -p term-core
//! ```
//!
//! **It measures through `Screen::find`, the call the window makes**, not the engine below it: the
//! question is what the person's press costs, and the step around the engine's walk — reading the
//! selection and choosing where to start — is part of that press. A press that finds nothing does
//! not scroll.
//!
//! **It decides one thing**: whether a press has to be split across frames. Its cost lands on the
//! frame the press arrives in, so a walk longer than a few milliseconds is a visible stall. The
//! design set the line at **4 ms** for the slowest press (D61); under it, the walk stays whole.
//!
//! **In debug the numbers come out many times worse.** The one that counts is the `--release` one.

use std::time::{Duration, Instant};

use term_core::{SCROLLBACK, Screen, Search, Size, Toward};

/// A wide console, because the walk is paid per cell and a wide one has the most of them.
const COLS: u16 = 180;

/// The rows of the visible part, from the application's measurement runs.
const ROWS: u16 = 49;

/// How many presses are timed. The first one teaches the engine's lazy automata the grid, so it is
/// published apart from the rest rather than averaged into them.
const PRESSES: usize = 20;

fn main() {
    let mut screen = Screen::new(Size {
        rows: ROWS,
        cols: COLS,
    });
    // Lines as long as the console, so every cell of the history holds a character to walk: a
    // history of short lines would measure mostly blanks.
    for n in 0..SCROLLBACK + ROWS as usize {
        let line = format!("compiling module {n} of the workspace and its dependencies ");
        let padded: String = line.chars().cycle().take(COLS as usize).collect();
        screen.advance(padded.as_bytes());
        // A line of its own, so the history is lines and not one line wrapped across all of it.
        screen.advance(b"\r\n");
    }

    for (query, toward) in [("absent", Toward::Older), ("absent", Toward::Newer)] {
        let mut search = Search::literal(query).expect("an ordinary literal builds");
        let mut times = Vec::with_capacity(PRESSES);
        for _ in 0..PRESSES {
            let before = Instant::now();
            let found = screen.find(&mut search, toward);
            times.push(before.elapsed());
            assert!(!found, "the premise: «{query}» is not in the history");
        }

        let first = times[0];
        let mut rest = times[1..].to_vec();
        rest.sort();
        let median = rest[rest.len() / 2];
        let slowest = *rest.last().expect("more than one press");
        println!(
            "{toward:?}, nothing found, {} lines of {COLS} columns: first {}, then median {} and slowest {}",
            SCROLLBACK + ROWS as usize,
            millis(first),
            millis(median),
            millis(slowest),
        );
    }
}

fn millis(time: Duration) -> String {
    format!("{:.2} ms", time.as_secs_f64() * 1000.0)
}
