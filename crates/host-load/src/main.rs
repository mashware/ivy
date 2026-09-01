//! `host-load` commands. The measurement protocol, the reference numbers and —above all— the
//! instrument's limit are in the crate's documentation (`lib.rs`).
//!
//! ```text
//! host-load run    [--mib N] [--at S] [--rate M] [--tag T] [--results PATH] [--dump PATH]
//! host-load report [--tag T] [--results PATH]
//! ```

use std::fs::OpenOptions;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use host_load::{
    CHUNK, Excluded, Run, aggregate, build_load, build_load_cjk, build_load_color, pace_delay,
    start_instant,
};

type Error = Box<dyn std::error::Error + Send + Sync>;

const DEFAULT_MIB: usize = 22;
const DEFAULT_AT: u64 = 30;
const ONE_MIB: usize = 1_048_576;

const USAGE: &str = "\
host-load — measures how long a terminal takes to drain a fixed load

  host-load run    [--mib N] [--at S] [--rate M] [--tag T] [--results PATH] [--dump PATH]
  host-load report [--tag T] [--results PATH]

  --mib N       size of the load in MiB (default 22)
  --cjk         writes in Japanese, Chinese and Korean instead of ASCII
  --emoji       sprinkles the load with emojis, with and without a presentation selector
  --at S        start at the next multiple of S seconds (default 30; 0 = now)
  --rate M      target sustained throughput in MiB/s (default, as fast as it allows)
  --tag T       label of the batch; report groups by it
  --results R   results file (default, a fixed one in the temp directory)
  --dump R      writes the load to a file and exits, to compare bytes between machines
";

fn main() -> ExitCode {
    match dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("host-load: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("report") => report(&args[1..]),
        Some("--help" | "-h") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command «{other}»\n\n{USAGE}").into()),
    }
}

/// Command-line arguments.
///
/// They are parsed by hand and not with a third-party crate: it is six options and this crate has
/// no dependency, which makes it compilable anywhere measuring is needed.
struct Options {
    mib: usize,
    /// Whether the load is written in CJK. Only the font-discovery batch asks for it.
    cjk: bool,
    emoji: bool,
    at: u64,
    rate: Option<f64>,
    tag: String,
    results: std::path::PathBuf,
    dump: Option<std::path::PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Options, Error> {
    let mut o = Options {
        mib: DEFAULT_MIB,
        cjk: false,
        emoji: false,
        at: DEFAULT_AT,
        rate: None,
        tag: "no-tag".to_string(),
        // **Absolute** path on purpose: the N splits may be in different directories and a relative
        // path would make each one write to its own file, so the aggregate would come out of a
        // single measurement without anyone noticing.
        //
        // And with the user inside the name: the temp directory is shared, so two people measuring
        // at the same time on the same machine would mix their measurements under the same tag
        // without realizing it.
        results: std::env::temp_dir().join(format!(
            "host-load-{}.tsv",
            std::env::var("USER").unwrap_or_else(|_| "anon".to_string())
        )),
        dump: None,
    };

    let mut i = 0;
    while i < args.len() {
        let value = |i: usize| -> Result<&String, Error> {
            args.get(i + 1)
                .ok_or_else(|| format!("«{}» needs a value", args[i]).into())
        };
        match args[i].as_str() {
            "--mib" => o.mib = value(i)?.parse()?,
            "--cjk" => o.cjk = true,
            "--emoji" => o.emoji = true,
            "--at" => o.at = value(i)?.parse()?,
            "--rate" => o.rate = Some(value(i)?.parse()?),
            "--tag" => o.tag = value(i)?.clone(),
            "--results" => o.results = value(i)?.into(),
            "--dump" => o.dump = Some(value(i)?.into()),
            other => return Err(format!("unknown option «{other}»\n\n{USAGE}").into()),
        }
        i += 2;
    }
    Ok(o)
}

/// Now, since the epoch, **with a fraction of a second**.
///
/// The fraction is what makes N splits genuinely start at once; truncating it here was the bug that
/// let two measurements almost a second apart pass for concurrent.
fn now() -> Result<Duration, Error> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?)
}

fn run(args: &[String]) -> Result<(), Error> {
    let o = parse_args(args)?;
    // Without `checked_mul`, a wild `--mib` would overflow silently when compiling in release and
    // would measure a size that is not the requested one.
    let bytes = o
        .mib
        .checked_mul(ONE_MIB)
        .ok_or_else(|| format!("--mib {} is too large", o.mib))?;
    // **The CJK and emoji loads are only asked for by the font batches.** The usual one is not
    // touched, so that the new batches remain comparable with the published annexes.
    //
    // **And the two flags together mean nothing**, so it is rejected instead of silently choosing
    // one: a batch launched with both would publish the cost of one language believing it measures
    // the other.
    let load = match (o.cjk, o.emoji) {
        (true, true) => {
            return Err(
                "--cjk and --emoji measure different things: you have to choose one".into(),
            );
        }
        (true, false) => build_load_cjk(bytes),
        (false, true) => build_load_color(bytes),
        (false, false) => build_load(bytes),
    };

    if let Some(path) = o.dump {
        std::fs::write(&path, &load)?;
        eprintln!("load of {} bytes written to {}", load.len(), path.display());
        return Ok(());
    }

    // If the output is not a terminal, this does not measure a terminal. It is not aborted
    // —redirecting is a legitimate use to check that the tool works— but it is marked in the
    // result, which is what keeps `report` from sneaking it into the aggregate.
    let tty = std::io::stdout().is_terminal();
    if !tty {
        eprintln!(
            "host-load: the output is not a terminal; this measurement does NOT measure a terminal"
        );
    }

    let target = start_instant(now()?, o.at);
    if let Some(wait) = target.checked_sub(now()?)
        && !wait.is_zero()
    {
        eprintln!(
            "host-load: starting in {:.1} s (launch the rest of the splits now)",
            wait.as_secs_f64()
        );
        thread::sleep(wait);
    }

    let rate_bytes = o.rate.map(|mib_s| mib_s * ONE_MIB as f64);
    let out = std::io::stdout();
    let mut out = out.lock();

    let real_start = now()?.as_secs_f64();
    let clock = Instant::now();
    let mut written: u64 = 0;
    for chunk in load.chunks(CHUNK) {
        out.write_all(chunk)?;
        written += chunk.len() as u64;
        if let Some(wait) = pace_delay(written, clock.elapsed(), rate_bytes) {
            out.flush()?;
            thread::sleep(wait);
        }
    }
    out.flush()?;
    let elapsed = clock.elapsed().as_secs_f64();

    let measurement = Run {
        tag: o.tag,
        bytes: load.len() as u64,
        seconds: elapsed,
        start: real_start,
        at: o.at,
        rate: o.rate,
        tty,
    };

    // The summary goes out through standard error: through standard output it would become part of
    // the load of the next one that redirects, and here there is nothing left to measure.
    eprintln!(
        "\n── {:.2} s · {:.1} MiB/s · {:.1} MiB · tag {} ──",
        measurement.seconds,
        measurement.mib_s(),
        measurement.bytes as f64 / ONE_MIB as f64,
        measurement.tag
    );

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&o.results)?;
    // A single write, line break included. `writeln!` emits two —the line and the `\n`— and
    // `O_APPEND` only makes each call atomic, not the pair: with N splits finishing at once, two
    // lines would interleave and both would be lost when reread.
    file.write_all(format!("{}\n", measurement.to_line()).as_bytes())?;
    eprintln!("recorded in {}", o.results.display());
    Ok(())
}

fn report(args: &[String]) -> Result<(), Error> {
    let o = parse_args(args)?;
    // That the file does not exist yet is normal: nothing has been measured yet. Any **other** read
    // failure does propagate — treating them all as "empty" would turn a permissions problem into a
    // "no results" that reads as if there really were no measurements.
    let contents = match std::fs::read_to_string(&o.results) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("could not read {}: {e}", o.results.display()).into()),
    };

    // Unreadable lines are counted and reported. Discarding them silently made "there were only 3
    // measurements" indistinguishable from "there were 4 and one got corrupted", which is exactly
    // what needs to be known.
    let mut unreadable = 0usize;
    let mut measurements = Vec::new();
    for line in contents.lines().filter(|l| !l.trim().is_empty()) {
        match Run::from_line(line) {
            Some(r) if r.tag == o.tag => measurements.push(r),
            Some(_) => {}
            None => unreadable += 1,
        }
    }

    if unreadable > 0 {
        println!(
            "warning: {unreadable} line(s) of the file could not be read (old format or corrupt write)"
        );
    }

    if measurements.is_empty() {
        println!("no results for the tag «{}»", o.tag);
        return Ok(());
    }

    let n = measurements.len();
    let aggregated = aggregate(measurements);

    println!("tag «{}» — {n} measurement(s)", o.tag);
    for r in &aggregated.included {
        println!("  {:>7.2} s · {:>7.1} MiB/s", r.seconds, r.mib_s());
    }
    for (r, why) in &aggregated.excluded {
        let reason = match why {
            Excluded::NoTty => "out: the output was not a terminal",
            Excluded::NoOverlap => "out: it did not coincide in time with the others",
            Excluded::MixedRegime => "out: the tag mixes burst with sustained throughput",
        };
        println!(
            "  {:>7.2} s · {:>7.1} MiB/s  ← {reason}",
            r.seconds,
            r.mib_s()
        );
    }

    match aggregated.mib_s {
        Some(aggregate) => println!(
            "\naggregate: {:.1} MiB/s over {} split(s)",
            aggregate,
            aggregated.included.len()
        ),
        None => println!(
            "\nno aggregate: there are no comparable measurements in this tag.\n\
             Repeat the batch launching all the splits within the same --at window,\n\
             with the same regime (all with --rate or none)."
        ),
    }
    Ok(())
}
