//! Measures how much the grid absorbs in a massive dump.
//!
//! It is the risk the roadmap assigns to this milestone: a `cat` of a large file. The agreed
//! mitigation is **not** to put a cap inside the crate, but to know the number and to make the
//! shape of the API allow chunking — the cap will be set by whoever paints, in F1.3, once there is
//! a frame to measure it against.
//!
//! ```text
//! cargo run --release --example throughput -p term-core
//! ```
//!
//! In debug the number comes out several times worse; the one that counts is the `--release` one.

use std::time::Instant;

use term_core::{Screen, Size};

/// Size of the dump. Ten megs is a `cat` of a genuinely large file, not a toy.
const BYTES: usize = 10 * 1024 * 1024;

/// Chunk the grid is fed with, the size of the PTY read buffer.
const CHUNK: usize = 8192;

fn main() {
    // Text with line breaks and the odd color escape: feeding the parser a single repeated
    // character would measure the fast path and lie about the real case.
    let pattern = b"\x1b[32mlorem ipsum dolor sit amet\x1b[0m consectetur adipiscing elit\n";
    let mut dump = Vec::with_capacity(BYTES + pattern.len());
    while dump.len() < BYTES {
        dump.extend_from_slice(pattern);
    }

    let mut screen = Screen::new(Size::default());

    let start = Instant::now();
    for chunk in dump.chunks(CHUNK) {
        screen.advance(chunk);
    }
    let took = start.elapsed();

    let mib = dump.len() as f64 / (1024.0 * 1024.0);
    println!("dump:     {mib:.1} MiB in {CHUNK} B chunks");
    println!("time:     {took:.3?}");
    println!("absorbs:  {:.1} MiB/s", mib / took.as_secs_f64());
    println!(
        "per chunk: {:.1} µs on average",
        took.as_secs_f64() * 1e6 / dump.len().div_ceil(CHUNK) as f64
    );
}
