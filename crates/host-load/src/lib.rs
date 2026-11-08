#![deny(rustdoc::broken_intra_doc_links)]
// **A broken rustdoc link is invisible to the compiler, and half the repository is rustdoc.** When
// this was put in, the tree already had one pointing to a constant that an earlier rename had left
// behind: nobody noticed because `cargo doc` warns and does not fail. With the `deny` it fails, and
// the milestone that renames 46,458 lines is going to be grateful for it.
//
// It fires on `cargo doc`, **not** on `cargo test` — this repository has no continuous integration,
// so it still has to be invoked. What changes is that now it breaks instead of warning.
//! Measures how long a terminal takes to **drain** a fixed load dumped at it through standard
//! output, and aggregates the result of several splits producing at once.
//!
//! # What it measures, and what it does not
//!
//! It measures what the terminal **pulls out of the pseudoterminal**, not what it **paints**. A
//! terminal that swallows fast and then drops frames looks good in this number and bad to the eye,
//! so this is a **floor**, not a ceiling.
//!
//! That makes it a good instrument to **compare other people's terminals** —if one already fails at
//! the drain, there is no need to look at the painting to rule it out— and a **bad criterion for
//! ours**: the mitigation the roadmap plans for F1.3 is to repaint only when the PTY produces
//! changes, that is, to coalesce frames, which would pass this test by construction.
//!
//! The painting is **unobservable from here**: no terminal protocol reports a presented frame, and
//! the round trip of `ESC[6n` only proves *consumed and parsed*. The instrument that would see it
//! is counters inside the app itself —bytes processed per frame, frames presented, lag of the last
//! one—, and that needs a paint loop that does not yet exist.
//!
//! # Protocol
//!
//! ```text
//! # 1. A single split: the reference
//! cargo run --release -p host-load -- run --tag wezterm-1
//!
//! # 2. N splits visible at once, one command per split, within the same 30 s window
//! cargo run --release -p host-load -- run --tag wezterm-4
//!
//! # 3. The aggregate of each run
//! cargo run --release -p host-load -- report --tag wezterm-4
//! ```
//!
//! What decides is **not the absolute MiB/s**, it is whether the aggregate **rises** with N or
//! falls. A terminal that genuinely spreads the work multiplies the aggregate; one that fights over
//! a shared thread keeps it the same or makes it worse.
//!
//! The splits have to be **visible at once**: a terminal that only mounts the active tab would
//! measure a single one no matter how open the others are.
//!
//! ## Keep the window width out of the experiment
//!
//! When splitting a window into N splits, each one narrows. The load's lines measure ~46 columns:
//! below that the terminal starts to **wrap**, which is extra work, and the aggregate would fall
//! with N for a reason that is not the one you want to measure.
//!
//! So **the honest comparison is at equal width**: either each split keeps ≥46 columns, or it is
//! measured with the same width per split in both phases. The tool cannot check it —from the
//! producing process the window's layout is not visible— so it is left in the hands of whoever
//! measures, and that is why it is written here.
//!
//! ## The two regimes read differently, and are not compared to each other
//!
//! Without `--rate` the load comes out as fast as the terminal allows, and the MiB/s answers "how
//! much it gives". With `--rate` it answers something else: **whether it held the requested
//! throughput**. Going at a pace, the total time includes the waits, so a terminal with room to
//! spare returns exactly the throughput it was asked for and no more — measured: `--rate 10` gives
//! 10.0 MiB/s dead on. What it reports is the drop: with a consumer that could not keep up, that
//! same command returned 0.3 MiB/s.
//!
//! That is, in throughput mode the measurement is **censored from above**: it serves to detect the
//! one that cannot hold up, not to rank the ones that can. Mixing the two regimes under one label
//! would give a meaningless aggregate, so [`aggregate`] **refuses to do it** instead of trusting it
//! to the discipline of whoever measures.
//!
//! # Reference numbers
//!
//! Measured on 8 August 2026, Wayland, hardware GPU, 22.7 MiB of load:
//!
//! ```text
//!             1 split               4 splits visible           drop    aggregate
//! WezTerm     0.80 s · 28.4 MiB/s   0.98 / 0.98 / 1.01 / 1.06   ×1.26   85.7 MiB/s
//! Orca        2.63 s ·  8.6 MiB/s   8.44 / 12.50 / 12.72 / 12.64  ×4.4    7.1 MiB/s
//! ```
//!
//! Orca moves **less** total work with four splits than with one: negative scaling. It is the
//! result that underpins decision D3 not to build on Electron and xterm.js.

use std::time::Duration;

/// Size of the chunk the output is fed with.
///
/// It matches the PTY read buffer in `term-core`, but **what arrives on the other side is not a
/// block of this size**: Rust interposes a `LineWriter` on standard output, and measured with
/// `strace` each chunk comes out in two calls —a short one with the line tail that was left pending
/// and another of ~8 KiB up to the last break—. The bias is constant, so the comparison between
/// terminals survives; the absolute MiB/s carries that bias inside it.
pub const CHUNK: usize = 8 * 1024;

/// Every how many lines the load repaints the status bar.
const LINES_PER_REPAINT: usize = 50;

/// Builds the load.
///
/// **Deterministic on purpose**: the content of line `i` depends only on `i`, with no clock, no
/// randomness and no file path. Two different machines generate the same bytes, which is what makes
/// two measurements comparable.
///
/// The pattern carries color, bold, dim and a repaint with cursor movement **on purpose**: feeding
/// a terminal a single repeated character walks its parser's fast path and gives a number that lies
/// about the real case.
///
/// Lines stop being added upon reaching `target_bytes`, without cutting the last one: truncating
/// mid-way would leave a split escape sequence, and that would no longer be the same load.
pub fn build_load(target_bytes: usize) -> Vec<u8> {
    let mut load = Vec::with_capacity(target_bytes + 256);
    let mut i: usize = 1;
    while load.len() < target_bytes {
        let color = 31 + i % 7;
        let ms = 3 + i % 40;
        load.extend_from_slice(
            format!(
                "\x1b[{color}m[\x1b[1m{i:06}\x1b[0m\x1b[{color}m]\x1b[0m compiling module {i} … \x1b[32mdone\x1b[0m \x1b[2m({ms} ms)\x1b[0m\n"
            )
            .as_bytes(),
        );
        if i.is_multiple_of(LINES_PER_REPAINT) {
            // Save cursor, jump to the origin, paint in reverse video, come back: what a text
            // interface does and what gives the painter the most work.
            load.extend_from_slice(format!("\x1b7\x1b[H\x1b[7m {i} \x1b[0m\x1b8").as_bytes());
        }
        i += 1;
    }
    load
}

/// Dedicated ASCII programming scene; historical byte streams remain unchanged.
pub fn build_load_ligatures(target_bytes: usize) -> Vec<u8> {
    let mut load = Vec::with_capacity(target_bytes + 128);
    while load.len() < target_bytes {
        load.extend_from_slice(
            b"\x1b[32ma!=b => c==d === e !== f <= g >= h -> i :: j && k || l\x1b[0m\n",
        );
    }
    load
}

/// The characters the CJK load writes with, and why these ones.
///
/// **They cover the three blocks the fonts milestone pursues** —kanji, hiragana and hangul— plus a
/// Private Use Area icon, which is the other case. None of them are covered by egui's factory fonts,
/// so each one **genuinely triggers** the discovery path instead of grazing it.
const CJK_LETTERS: &[char] = &[
    '日', '本', '語', '漢', '字', '中', '文', 'あ', 'い', 'う', 'え', 'お', '한', '국', '어',
    '\u{f015}',
];

/// The same load, but writing in CJK.
///
/// **It exists separately and `build_load` is not touched, and that is deliberate**: changing the
/// usual load would make the new runs incomparable with any of the published annexes. Only the run
/// that measures font discovery asks for this one.
///
/// **Deterministic just like the other**: the content of line `i` depends only on `i`.
pub fn build_load_cjk(target_bytes: usize) -> Vec<u8> {
    let mut load = Vec::with_capacity(target_bytes + 256);
    let mut i: usize = 1;
    while load.len() < target_bytes {
        let color = 31 + i % 7;
        // **A different word per line, rotating**, so that it is not always the same character:
        // the painter caches per character, so repeating a single one would walk the fast path and
        // measure the discovery of **one** font instead of that of a real screen.
        let word: String = (0..4)
            .map(|n| CJK_LETTERS[(i + n) % CJK_LETTERS.len()])
            .collect();
        load.extend_from_slice(
            format!(
                "\x1b[{color}m[\x1b[1m{i:06}\x1b[0m\x1b[{color}m]\x1b[0m {word} — compiling module {i} … \x1b[32m完了\x1b[0m\n"
            )
            .as_bytes(),
        );
        if i.is_multiple_of(LINES_PER_REPAINT) {
            load.extend_from_slice(format!("\x1b7\x1b[H\x1b[7m {i} \x1b[0m\x1b8").as_bytes());
        }
        i += 1;
    }
    load
}

/// The emojis the color load writes with, and why these ones.
///
/// **The three cases the presentation rule distinguishes, and not one more:** one that goes to
/// color by default, one text one **with** its selector —which has to come out in color— and one
/// text one **without** it —which does not—. Without all three, a run would paint everything by the
/// same path and measure half the work.
///
/// **And there are many different ones on purpose.** The painter remembers per character, so
/// repeating a single one would walk the fast path and publish the cost of rasterizing **one** glyph
/// as if it were that of a real screen.
const EMOJIS: &[&str] = &[
    "\u{1F600}",
    "\u{1F44D}",
    "\u{1F680}",
    "\u{2601}\u{FE0F}",
    "\u{1F389}",
    "\u{2764}\u{FE0F}",
    "\u{1F4E6}",
    "\u{2601}",
    "\u{1F525}",
    "\u{2B50}",
    "\u{1F41B}",
    "\u{2714}\u{FE0F}",
];

/// The same load, but sprinkled with emojis.
///
/// **It exists separately and `build_load` is not touched**, for the same reason as the CJK one:
/// changing the usual load would leave the new runs unable to be compared with any published annex.
/// Only the run that measures what it costs to rasterize a color glyph asks for this one.
///
/// **Deterministic just like the others**: the content of line `i` depends only on `i`.
pub fn build_load_color(target_bytes: usize) -> Vec<u8> {
    let mut load = Vec::with_capacity(target_bytes + 256);
    let mut i: usize = 1;
    while load.len() < target_bytes {
        let color = 31 + i % 7;
        let face: String = (0..3).map(|n| EMOJIS[(i + n) % EMOJIS.len()]).collect();
        let ms = 3 + i % 40;
        load.extend_from_slice(
            format!(
                "\x1b[{color}m[\x1b[1m{i:06}\x1b[0m\x1b[{color}m]\x1b[0m {face} compiling module {i} … \x1b[32mdone\x1b[0m \x1b[2m({ms} ms)\x1b[0m\n"
            )
            .as_bytes(),
        );
        if i.is_multiple_of(LINES_PER_REPAINT) {
            load.extend_from_slice(format!("\x1b7\x1b[H\x1b[7m {i} \x1b[0m\x1b8").as_bytes());
        }
        i += 1;
    }
    load
}

/// First aligned instant at which to start, counted from the epoch.
///
/// **It works in fractions of a second, and that is the only thing that makes it useful.** The
/// first version subtracted two epochs already truncated to the second and slept that whole
/// difference, so the launch fraction was dragged along intact: two splits launched half a second
/// apart started half a second apart — measured, 470.314 against 470.807 when both should have gone
/// off at 470.000. Worse still, with `now` = 120.5 and `at` = 30 the truncated target fell **in the
/// past** and that split shot off while another launched 0.5 s later waited 29 s. Aligning badly is
/// worse than not aligning: the misalignment stays hidden.
///
/// With `at` at zero, right now. If `now` falls exactly on a multiple, it is `now`.
pub fn start_instant(now: Duration, at: u64) -> Duration {
    if at == 0 {
        return now;
    }
    let period = u128::from(at) * 1_000_000_000;
    match now.as_nanos() % period {
        0 => now,
        remainder => now + Duration::from_nanos((period - remainder) as u64),
    }
}

/// How long to wait before the next chunk so as not to exceed the target throughput.
///
/// `None` when there is no target throughput or when it is already running behind it: **it never
/// speeds up to catch up**. If the terminal does not give the requested throughput, the real
/// throughput comes out below it, and that *is* the result of the measurement.
pub fn pace_delay(
    bytes_written: u64,
    elapsed: Duration,
    rate_bytes: Option<f64>,
) -> Option<Duration> {
    let rate_bytes = rate_bytes?;
    if rate_bytes <= 0.0 {
        return None;
    }
    let should_have_taken = bytes_written as f64 / rate_bytes;
    let spare = should_have_taken - elapsed.as_secs_f64();
    // `Duration::from_secs_f64` panics with a non-finite or overflowed value, and an absurdly small
    // throughput (`--rate 1e-25`) produces one. Checking it here is what keeps the tool from
    // blowing up instead of saying it is not going to wait.
    (spare > 0.0 && spare.is_finite() && spare < Duration::MAX.as_secs_f64())
        .then(|| Duration::from_secs_f64(spare))
}

/// A measurement: what a split wrote and how long its terminal took to swallow it.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub tag: String,
    pub bytes: u64,
    pub seconds: f64,
    /// Real start instant in seconds since the epoch, **with fraction**.
    ///
    /// Storing it truncated to the second is what made the overlap check useless: two measurements
    /// 900 ms apart recorded the same second, passed for concurrent and their aggregate came out at
    /// double the real value.
    pub start: f64,
    /// The `--at` it was launched with. Informational.
    pub at: u64,
    /// Target throughput in MiB/s, if requested. Distinguishes the regime: [`aggregate`] does not
    /// mix a burst measurement with a throughput one, because they answer different questions.
    pub rate: Option<f64>,
    /// Whether the output was a real terminal. If it was not, the measurement does not measure a
    /// terminal.
    pub tty: bool,
}

/// Leaves the tag on a single line and without tabs.
///
/// It is not cosmetic: a tab splits the line into fields that are not the real ones and the
/// measurement is lost silently; a line break allows **sneaking in a made-up measurement** into the
/// results file.
fn tag_on_one_line(tag: &str) -> String {
    tag.replace(['\t', '\n', '\r'], " ")
}

impl Run {
    /// End of the execution window, in seconds since the epoch.
    fn end(&self) -> f64 {
        self.start + self.seconds
    }

    /// The two regimes are not compared to each other; this is what distinguishes them.
    fn regime(&self) -> Option<u64> {
        self.rate.map(f64::to_bits)
    }

    pub fn to_line(&self) -> String {
        format!(
            "{}\t{}\t{:.6}\t{:.6}\t{}\t{}\t{}",
            tag_on_one_line(&self.tag),
            self.bytes,
            self.seconds,
            self.start,
            self.at,
            match self.rate {
                Some(r) => format!("{r}"),
                None => "-".to_string(),
            },
            if self.tty { "yes" } else { "no" }
        )
    }

    /// Re-reads a line from the results file.
    ///
    /// Returns `None` for anything that is not a valid measurement — including **non-finite**
    /// numbers. A `seconds` of `NaN` was not discarded on its own: `f64::max` and `f64::min` ignore
    /// `NaN`s, so the measurement contributed neither to the earliest end nor to the slowest one,
    /// but its bytes did enter the sum and **inflated the aggregate**.
    pub fn from_line(line: &str) -> Option<Self> {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 7 {
            return None;
        }
        let seconds: f64 = fields[2].parse().ok()?;
        let start: f64 = fields[3].parse().ok()?;
        if !seconds.is_finite() || seconds < 0.0 || !start.is_finite() || start < 0.0 {
            return None;
        }
        let rate = match fields[5] {
            "-" => None,
            other => {
                let r: f64 = other.parse().ok()?;
                r.is_finite().then_some(r)?;
                Some(r)
            }
        };
        Some(Self {
            tag: fields[0].to_string(),
            bytes: fields[1].parse().ok()?,
            seconds,
            start,
            at: fields[4].parse().ok()?,
            rate,
            tty: fields[6] == "yes",
        })
    }

    pub fn mib_s(&self) -> f64 {
        if self.seconds <= 0.0 {
            return 0.0;
        }
        self.bytes as f64 / self.seconds / 1_048_576.0
    }
}

/// Why a measurement is left out of the aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Excluded {
    /// The output was not a terminal: the number measures a pipe or a file, not a terminal.
    NoTty,
    /// The tag's measurements did not coincide in time, so they never competed with each other.
    NoOverlap,
    /// The tag mixes burst with sustained throughput, which answer different questions.
    MixedRegime,
}

/// The aggregate of a tag, and what has been left out when computing it.
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregate {
    pub included: Vec<Run>,
    pub excluded: Vec<(Run, Excluded)>,
    /// `None` when there is nothing aggregable. It is not an error: it is having no answer.
    pub mib_s: Option<f64>,
}

/// Aggregates the measurements of a tag.
///
/// The aggregate is `Σ bytes / max(seconds)`. It divides by the **maximum** and not by the mean
/// because the run occupies the terminal until the last split finishes: with the mean, four splits
/// of 8, 12, 12 and 12 s would look faster than they were.
///
/// **It refuses to aggregate what did not compete**, and the three checks come from real errors: a
/// measurement whose output was not a terminal (a pipe swallowed 22 MiB at 3,010 MiB/s),
/// measurements launched in different windows that never overlapped, and burst mixed with sustained
/// throughput under the same tag.
pub fn aggregate(runs: Vec<Run>) -> Aggregate {
    let mut excluded = Vec::new();
    let mut candidates = Vec::new();
    for run in runs {
        if run.tty {
            candidates.push(run);
        } else {
            excluded.push((run, Excluded::NoTty));
        }
    }

    if candidates.is_empty() {
        return without_aggregate(excluded);
    }

    // Regime: the majority rules, and whatever does not fit is declared out instead of
    // contaminating the sum.
    let regime = candidates[0].regime();
    if candidates.iter().any(|r| r.regime() != regime) {
        excluded.extend(candidates.into_iter().map(|r| (r, Excluded::MixedRegime)));
        return without_aggregate(excluded);
    }

    // They all overlap if the latest start falls before the earliest end. With a single
    // measurement there is nothing to overlap with, so the check does not apply — checking it
    // anyway discarded a legitimate zero-duration measurement.
    if candidates.len() > 1 {
        let latest_start = candidates.iter().map(|r| r.start).fold(f64::MIN, f64::max);
        let earliest_end = candidates.iter().map(Run::end).fold(f64::MAX, f64::min);
        if latest_start >= earliest_end {
            excluded.extend(candidates.into_iter().map(|r| (r, Excluded::NoOverlap)));
            return without_aggregate(excluded);
        }
    }

    let bytes: u64 = candidates.iter().map(|r| r.bytes).sum();
    let slowest = candidates.iter().map(|r| r.seconds).fold(0.0, f64::max);
    let mib_s = (slowest > 0.0).then(|| bytes as f64 / slowest / 1_048_576.0);

    Aggregate {
        included: candidates,
        excluded,
        mib_s,
    }
}

fn without_aggregate(excluded: Vec<(Run, Excluded)>) -> Aggregate {
    Aggregate {
        included: Vec::new(),
        excluded,
        mib_s: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(bytes: u64, seconds: f64, start: f64, tty: bool) -> Run {
        Run {
            tag: "t".into(),
            bytes,
            seconds,
            start,
            at: 30,
            rate: None,
            tty,
        }
    }

    #[test]
    fn the_load_is_identical_across_two_passes() {
        assert_eq!(build_load(200_000), build_load(200_000));
    }

    #[test]
    fn the_load_carries_the_pattern_that_does_not_take_the_parsers_fast_path() {
        let load = build_load(200_000);
        let text = String::from_utf8_lossy(&load);

        assert!(text.contains("\x1b[1m"), "the bold is missing");
        assert!(text.contains("\x1b[2m"), "the dim is missing");
        assert!(text.contains("\x1b[32m"), "the color is missing");
        assert!(
            text.contains("\x1b7") && text.contains("\x1b8"),
            "the repaint that saves and restores the cursor is missing"
        );
        assert!(
            text.contains("\x1b[7m"),
            "the bar's reverse video is missing"
        );
    }

    #[test]
    fn the_load_reaches_the_requested_size_without_cutting_midway() {
        for target in [1_000, 100_000, 1_000_000] {
            let load = build_load(target);
            assert!(load.len() >= target, "it came up short with {target}");
            // Every unit that is added ends in a line break or in the `\x1b8` that restores the
            // cursor: if it ended in something else, an escape sequence would have been cut midway.
            let last = *load.last().expect("empty load");
            assert!(
                last == b'\n' || last == b'8',
                "the load of {target} ended midway, at byte {last}"
            );
        }
    }

    #[test]
    fn the_aligned_instant_is_the_next_multiple() {
        let aligned = start_instant(Duration::from_secs(1_000_001), 30);
        assert_eq!(aligned, Duration::from_secs(1_000_020));
    }

    #[test]
    fn if_already_aligned_it_does_not_wait_an_extra_cycle() {
        let aligned = start_instant(Duration::from_secs(1_000_020), 30);
        assert_eq!(aligned, Duration::from_secs(1_000_020));
    }

    #[test]
    fn with_at_zero_it_starts_right_away() {
        let now = Duration::from_millis(1_000_001_500);
        assert_eq!(start_instant(now, 0), now);
    }

    #[test]
    fn two_splits_launched_out_of_sync_start_at_the_same_instant() {
        // The bug this calculation exists to avoid: with whole-second arithmetic, one launched at
        // 1,000,000.05 and another at 1,000,000.95 started 900 ms apart.
        let a = start_instant(Duration::from_micros(1_000_000_050_000), 30);
        let b = start_instant(Duration::from_micros(1_000_000_950_000), 30);
        assert_eq!(a, b, "the two have to go off at the same instant");
        assert_eq!(a, Duration::from_secs(1_000_020));
    }

    #[test]
    fn a_fractional_instant_already_past_the_multiple_waits_for_the_next_one() {
        // With truncation to the second, 1,000,020.5 gave target 1,000,020 —in the past— and that
        // split shot off while another launched half a second later waited 29 s.
        let aligned = start_instant(Duration::from_micros(1_000_020_500_000), 30);
        assert_eq!(aligned, Duration::from_secs(1_000_050));
    }

    #[test]
    fn the_throughput_waits_for_the_spare_it_has() {
        // 1 MiB written with a target of 1 MiB/s should take 1 s; it takes 0.25 → it waits 0.75.
        let wait = pace_delay(1_048_576, Duration::from_millis(250), Some(1_048_576.0))
            .expect("it should wait");
        assert!(
            (wait.as_secs_f64() - 0.75).abs() < 1e-6,
            "it waited {wait:?}"
        );
    }

    #[test]
    fn the_throughput_never_speeds_up_to_catch_up() {
        // It is running behind the target: there is nothing to wait for, and it is not compensated
        // either.
        assert_eq!(
            pace_delay(1_048_576, Duration::from_secs(3), Some(1_048_576.0)),
            None
        );
    }

    #[test]
    fn without_target_throughput_there_is_no_wait() {
        assert_eq!(pace_delay(1_048_576, Duration::from_millis(1), None), None);
    }

    #[test]
    fn an_absurd_throughput_does_not_blow_up_the_tool() {
        // Before the finiteness check, this panicked inside `Duration::from_secs_f64`: the
        // calculated wait ran off to infinity.
        for absurd in [
            f64::MIN_POSITIVE,
            1e-320,
            1e-25,
            -1.0,
            0.0,
            f64::NAN,
            f64::INFINITY,
        ] {
            let _ = pace_delay(1_048_576, Duration::from_millis(1), Some(absurd));
        }
    }

    #[test]
    fn the_results_line_round_trips_unchanged() {
        let mut original = run(22_700_000, 1.234_567, 1_800_000_000.5, true);
        original.rate = Some(10.0);
        let round_trip = Run::from_line(&original.to_line()).expect("could not re-read the line");
        assert_eq!(original.tag, round_trip.tag);
        assert_eq!(original.bytes, round_trip.bytes);
        assert_eq!(original.rate, round_trip.rate);
        assert_eq!(original.tty, round_trip.tty);
        assert!((original.seconds - round_trip.seconds).abs() < 1e-6);
        assert!(
            (original.start - round_trip.start).abs() < 1e-6,
            "the fraction of the start instant was lost"
        );
    }

    #[test]
    fn a_tag_with_a_tab_or_break_does_not_split_the_line() {
        let mut r = run(1, 1.0, 0.0, true);
        r.tag = "with\ttab\nand break".into();
        let line = r.to_line();
        assert_eq!(line.split('\t').count(), 7);
        assert!(
            !line.contains('\n'),
            "a break would allow sneaking in a made-up measurement"
        );
    }

    #[test]
    fn a_measurement_with_non_finite_seconds_is_rejected_when_reread() {
        // Ignoring it in the maxima and minima is not enough: its bytes would enter the sum and
        // inflate the aggregate.
        for poison in ["NaN", "inf", "-1"] {
            let line = format!("t\t1048576\t{poison}\t100.0\t30\t-\tyes");
            assert!(
                Run::from_line(&line).is_none(),
                "it should be rejected: {line}"
            );
        }
    }

    #[test]
    fn the_aggregate_divides_by_the_slowest_split() {
        // Four overlapping splits of 1 MiB: the run lasts as long as the slowest one, 2 s.
        let mib = 1_048_576;
        let runs = vec![
            run(mib, 1.0, 100.0, true),
            run(mib, 2.0, 100.0, true),
            run(mib, 1.5, 100.0, true),
            run(mib, 1.2, 100.0, true),
        ];
        let a = aggregate(runs);
        assert_eq!(a.included.len(), 4);
        assert!(a.excluded.is_empty());
        assert!((a.mib_s.expect("no aggregate") - 2.0).abs() < 1e-9);
    }

    #[test]
    fn it_does_not_aggregate_measurements_that_never_coincided_in_time() {
        // The second starts when the first has already finished: they never competed.
        let runs = vec![
            run(1_048_576, 1.0, 100.0, true),
            run(1_048_576, 1.0, 500.0, true),
        ];
        let a = aggregate(runs);
        assert!(a.mib_s.is_none(), "there should be no aggregate");
        assert!(a.included.is_empty());
        assert_eq!(a.excluded.len(), 2);
        assert!(
            a.excluded
                .iter()
                .all(|(_, why)| *why == Excluded::NoOverlap)
        );
    }

    #[test]
    fn it_detects_the_sub_second_offset() {
        // The case the truncated instant hid: 900 ms of offset with measurements of 0.8 s.
        let runs = vec![
            run(1_048_576, 0.8, 100.05, true),
            run(1_048_576, 0.8, 100.95, true),
        ];
        let a = aggregate(runs);
        assert!(
            a.mib_s.is_none(),
            "they did not overlap: the aggregate would have come out at double"
        );
    }

    #[test]
    fn it_excludes_from_the_aggregate_what_did_not_come_out_through_a_terminal() {
        let runs = vec![
            run(1_048_576, 1.0, 100.0, true),
            run(1_048_576, 0.001, 100.0, false),
        ];
        let a = aggregate(runs);
        assert_eq!(a.included.len(), 1);
        assert_eq!(
            a.excluded,
            vec![(run(1_048_576, 0.001, 100.0, false), Excluded::NoTty)]
        );
        assert!((a.mib_s.expect("no aggregate") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn it_does_not_mix_burst_with_sustained_throughput() {
        let burst = run(1_048_576, 1.0, 100.0, true);
        let mut sustained = run(1_048_576, 1.0, 100.0, true);
        sustained.rate = Some(10.0);
        let a = aggregate(vec![burst, sustained]);
        assert!(a.mib_s.is_none(), "they answer different questions");
        assert!(
            a.excluded
                .iter()
                .all(|(_, why)| *why == Excluded::MixedRegime)
        );
    }

    #[test]
    fn a_tag_with_no_results_is_not_an_error() {
        let a = aggregate(Vec::new());
        assert!(a.mib_s.is_none());
        assert!(a.included.is_empty());
        assert!(a.excluded.is_empty());
    }

    #[test]
    fn the_emoji_load_brings_the_three_presentation_cases() {
        // **Without all three, the run measures half the work and nobody notices.** The painter
        // decides by Unicode's presentation rule: an emoji that goes to color **by default**, a text
        // one **with** the selector —which has to come out in color— and a text one **without** it
        // —which does not—. A load that only carried the first type would walk one path and publish
        // its cost as if it were all three.
        let load = String::from_utf8(build_load_color(64 * 1024)).expect("the load is not text");
        assert!(
            load.contains('\u{1F600}'),
            "there is no emoji of those that go to color by default"
        );
        assert!(
            load.contains("\u{2601}\u{FE0F}"),
            "there is no text symbol WITH an emoji selector: the case the rule converts is missing"
        );
        assert!(
            load.contains('\u{2601}')
                && load.matches('\u{2601}').count() > load.matches("\u{2601}\u{FE0F}").count(),
            "there is no text symbol WITHOUT a selector: the case that has to stay gray is missing"
        );
    }

    #[test]
    fn the_emoji_load_is_the_same_across_two_runs() {
        // **It is what makes two runs of the same batch comparable**, which is what the whole
        // measurement bench lives on: if the content changed between runs, averaging the two would
        // be averaging two different loads. All three load functions promise it; only the new one
        // checks it.
        assert_eq!(
            build_load_color(32 * 1024),
            build_load_color(32 * 1024),
            "two runs of the same load gave different bytes: the batches stop being comparable"
        );
    }

    #[test]
    fn a_single_split_has_nobody_to_overlap_with() {
        // Including the zero-duration one, which the overlap check discarded for comparing a
        // measurement with itself.
        for seconds in [1.0, 0.0] {
            let a = aggregate(vec![run(1_048_576, seconds, 100.0, true)]);
            assert_eq!(a.included.len(), 1, "with seconds = {seconds}");
        }
    }
}

#[cfg(test)]
mod ligature_tests {
    #[test]
    fn programming_scene_is_repeatable_ascii_with_supported_sequences() {
        let load = super::build_load_ligatures(8192);
        assert!(load.len() >= 8192);
        assert!(load.is_ascii());
        assert_eq!(load, super::build_load_ligatures(8192));
        let text = String::from_utf8(load).unwrap();
        for sequence in ["!=", "=>", "==", "==="] {
            assert!(text.contains(sequence));
        }
        assert!(text.ends_with('\n'));
    }
}
