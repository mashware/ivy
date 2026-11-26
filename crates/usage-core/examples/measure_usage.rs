//! Synthetic Claude history witness for the public usage worker.
//!
//! cargo run --release -p usage-core --example measure_usage -- many
//! cargo run --release -p usage-core --example measure_usage -- huge
//! cargo run --release -p usage-core --example measure_usage -- smoke 20
//!
//! The second argument is the maximum number of seconds per run. Each mode
//! measures a complete cold-store import, then stop during a separate import
//! of the same fixture. The second import may use the OS page cache.

use serde_json::{Value, json};
use std::env;
use std::fs::{self, DirBuilder, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use usage_core::{DateRange, Service, Snapshot, Status};

const GIB: u64 = 1024 * 1024 * 1024;
const EVENT_TOKENS: u128 = 160;
const FILLER_BYTES: usize = 4096;
const START: &str = "{\"type\":\"user\",\"message\":{\"content\":\"";
const END: &str = "\"}}\n";
const RECORD_DATE: &str = "2026-11-26T12:00:00Z";

#[derive(Clone, Copy)]
struct Fixture {
    mode: &'static str,
    sources: usize,
    bytes: u64,
}

impl Fixture {
    fn config(mode: &str) -> Option<Self> {
        let (mode, sources, bytes) = match mode {
            "many" => ("many", 10_000, GIB),
            "huge" => ("huge", 1, GIB),
            "smoke" => ("smoke", 160, 160 * 16 * 1024),
            _ => return None,
        };
        Some(Self {
            mode,
            sources,
            bytes,
        })
    }

    fn generate(self, path: &Path) -> io::Result<u128> {
        fs::create_dir(path)?;
        let filler = padding_record(FILLER_BYTES);
        let per_source_bytes = self.bytes / self.sources as u64;
        let larger_sources = self.bytes % self.sources as u64;

        for project_start in (0..self.sources).step_by(100) {
            let project = path.join(format!("project-{:03}", project_start / 100));
            fs::create_dir(&project)?;
            let end = self.sources.min(project_start + 100);
            for source in project_start..end {
                let mut file =
                    BufWriter::new(File::create(project.join(format!("{source}.jsonl")))?);
                let bytes = per_source_bytes + u64::from((source as u64) < larger_sources);
                let first = usage_record(source);
                let last = if self.mode == "huge" {
                    usage_record(source + 1)
                } else {
                    String::new()
                };
                let padding = bytes - (first.len() + last.len()) as u64;
                file.write_all(first.as_bytes())?;
                for _ in 0..padding / FILLER_BYTES as u64 {
                    file.write_all(&filler)?;
                }
                file.write_all(&padding_record((padding % FILLER_BYTES as u64) as usize))?;
                file.write_all(last.as_bytes())?;
                file.flush()?;
            }
        }
        let events = self.sources as u128 + u128::from(self.mode == "huge");
        Ok(events * EVENT_TOKENS)
    }
}

fn usage_record(id: usize) -> String {
    format!(
        concat!(
            "{{\"type\":\"assistant\",\"requestId\":\"req_synthetic_{id}\",",
            "\"timestamp\":\"{date}\",\"message\":{{\"id\":\"msg_synthetic_{id}\",",
            "\"model\":\"claude-sonnet-4-6\",\"usage\":{{\"input_tokens\":100,",
            "\"cache_read_input_tokens\":30,\"cache_creation_input_tokens\":10,",
            "\"cache_creation\":{{\"ephemeral_5m_input_tokens\":10,",
            "\"ephemeral_1h_input_tokens\":0}},\"output_tokens\":20}}}}}}\n"
        ),
        id = id,
        date = RECORD_DATE,
    )
}

fn padding_record(bytes: usize) -> Vec<u8> {
    if bytes == 0 {
        return Vec::new();
    }
    let mut record = vec![b' '; bytes];
    if bytes >= START.len() + END.len() {
        record[..START.len()].copy_from_slice(START.as_bytes());
        record[bytes - END.len()..].copy_from_slice(END.as_bytes());
    } else {
        record[bytes - 1] = b'\n';
    }
    record
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> io::Result<Self> {
        let parent = fs::canonicalize(env::temp_dir())?;
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce)
            .map_err(|_| io::Error::other("could not name scratch directory"))?;
        let path = parent.join(format!(
            "ivy-usage-measure-{:032x}",
            u128::from_ne_bytes(nonce)
        ));
        let builder = DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!(
                "Failed to remove generated fixtures under {}: {error}",
                self.0.display()
            );
        }
    }
}

#[derive(Default)]
struct Resources {
    cpu_us: Option<u64>,
    peak_rss: Option<u64>,
    rchar: Option<u64>,
    read_bytes: Option<u64>,
}

impl Resources {
    fn sample() -> Self {
        let (cpu_us, reported_peak) = cpu_and_peak_rss();
        let peak_rss = process_peak_rss(reported_peak);
        let (rchar, read_bytes) = process_reads();
        Self {
            cpu_us,
            peak_rss,
            rchar,
            read_bytes,
        }
    }

    fn delta(&self, before: &Self) -> Value {
        json!({
            "process_cpu_us": difference(self.cpu_us, before.cpu_us),
            "incremental_process_peak_rss_bytes": difference(self.peak_rss, before.peak_rss),
            "process_peak_rss_before_bytes": before.peak_rss,
            "process_peak_rss_after_bytes": self.peak_rss,
            "process_read_transfer_bytes": difference(self.rchar, before.rchar),
            "process_storage_read_bytes": difference(self.read_bytes, before.read_bytes),
        })
    }
}

fn difference(after: Option<u64>, before: Option<u64>) -> Option<u64> {
    Some(after?.saturating_sub(before?))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn cpu_and_peak_rss() -> (Option<u64>, Option<u64>) {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return (None, None);
    }
    let usage = unsafe { usage.assume_init() };
    let timer_micros = |timer: libc::timeval| -> Option<u64> {
        let seconds = u64::try_from(timer.tv_sec).ok()?;
        let micros = u64::try_from(timer.tv_usec).ok()?;
        seconds.checked_mul(1_000_000)?.checked_add(micros)
    };
    let cpu = timer_micros(usage.ru_utime)
        .zip(timer_micros(usage.ru_stime))
        .and_then(|(user, system)| user.checked_add(system));
    let unit = if cfg!(target_os = "linux") { 1024 } else { 1 };
    let rss = u64::try_from(usage.ru_maxrss)
        .ok()
        .and_then(|rss| rss.checked_mul(unit));
    (cpu, rss)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn cpu_and_peak_rss() -> (Option<u64>, Option<u64>) {
    (None, None)
}

#[cfg(target_os = "linux")]
fn process_peak_rss(_: Option<u64>) -> Option<u64> {
    // getrusage can retain the launcher’s peak across exec. VmHWM belongs to
    // this address space, which starts fresh when the witness binary starts.
    let contents = fs::read_to_string("/proc/self/status").ok()?;
    let value = contents
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    value.checked_mul(1024)
}

#[cfg(not(target_os = "linux"))]
fn process_peak_rss(reported: Option<u64>) -> Option<u64> {
    reported
}

#[cfg(target_os = "linux")]
fn process_reads() -> (Option<u64>, Option<u64>) {
    let Ok(contents) = fs::read_to_string("/proc/self/io") else {
        return (None, None);
    };
    let metric = |prefix: &str| -> Option<u64> {
        contents
            .lines()
            .find_map(|line| line.strip_prefix(prefix))?
            .trim()
            .parse()
            .ok()
    };
    (metric("rchar:"), metric("read_bytes:"))
}

#[cfg(not(target_os = "linux"))]
fn process_reads() -> (Option<u64>, Option<u64>) {
    (None, None)
}

#[cfg(unix)]
fn device(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path).ok().map(|meta| meta.dev())
}

#[cfg(not(unix))]
fn device(_: &Path) -> Option<u64> {
    None
}

fn wall_ms(time: Duration) -> f64 {
    time.as_secs_f64() * 1000.0
}

fn start_worker(cache: PathBuf, source: &Path) -> io::Result<(Service, Receiver<()>)> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let service = Service::new(cache, move || {
        let _ = sender.try_send(());
    });
    let noon = chrono::DateTime::parse_from_rfc3339(RECORD_DATE)
        .expect("fixed fixture timestamp")
        .timestamp_millis();
    service.request(DateRange {
        start_ms: noon - 24 * 60 * 60 * 1000,
        end_ms: noon + 24 * 60 * 60 * 1000,
    });
    if !service.enable(vec![source.to_owned()]) {
        return Err(io::Error::other("usage worker rejected the enable command"));
    }
    Ok((service, receiver))
}

fn next_snapshot(service: &Service, wake: &Receiver<()>, deadline: Instant) -> Option<Snapshot> {
    loop {
        let snapshot = service.poll();
        if snapshot.is_some() {
            return snapshot;
        }
        wake.recv_timeout(deadline.checked_duration_since(Instant::now())?)
            .ok()?;
    }
}

struct Import {
    resources: Value,
    wall: Duration,
    first_importing: Option<Duration>,
    first_published: Option<Duration>,
    status: Option<Status>,
    tokens: Option<u128>,
    complete: bool,
}

fn import(cache: PathBuf, source: &Path, expected: u128, timeout: Duration) -> io::Result<Import> {
    let before = Resources::sample();
    let started = Instant::now();
    let deadline = started + timeout;
    let (service, wake) = start_worker(cache, source)?;
    let mut status = None;
    let mut tokens = None;
    let mut first_importing = None;
    let mut first_published = None;

    while let Some(snapshot) = next_snapshot(&service, &wake, deadline) {
        status = Some(snapshot.status);
        if snapshot.range.is_some() {
            tokens = Some(snapshot.totals.tokens);
        }
        if snapshot.status == Status::Importing {
            first_importing.get_or_insert_with(|| started.elapsed());
        }
        if snapshot.enabled && snapshot.last_success_at_ms.is_some() {
            first_published.get_or_insert_with(|| started.elapsed());
        }
        match snapshot.status {
            Status::Ready | Status::Empty | Status::Error | Status::Unsupported | Status::Limit => {
                break;
            }
            Status::Partial if tokens == Some(expected) => break,
            _ => {}
        }
    }
    let wall = started.elapsed();
    let resources = Resources::sample().delta(&before);
    let complete = status == Some(Status::Ready) && tokens == Some(expected);
    Ok(Import {
        resources,
        wall,
        first_importing,
        first_published,
        status,
        tokens,
        complete,
    })
}

fn cancellation(cache: PathBuf, source: &Path, timeout: Duration) -> io::Result<Value> {
    let baseline = Resources::sample();
    let (service, wake) = start_worker(cache, source)?;
    let deadline = Instant::now() + timeout;
    let mut active = false;
    let mut latest = None;
    while let Some(snapshot) = next_snapshot(&service, &wake, deadline) {
        latest = Some(snapshot.status);
        match snapshot.status {
            Status::Importing if snapshot.enabled => {
                active = true;
                break;
            }
            Status::Ready | Status::Empty | Status::Error | Status::Unsupported | Status::Limit => {
                break;
            }
            _ => {}
        }
    }
    if !active {
        return Ok(json!({
            "observed_active_import": false,
            "last_status": latest.map(|status| format!("{status:?}")),
            "stop_to_disabled_ms": null,
        }));
    }

    // Give the worker time to enter a read or write slice after the first
    // status. A short fixture may already finish, which is checked below.
    thread::sleep(Duration::from_millis(20));
    if let Some(snapshot) = service.poll() {
        active = snapshot.status == Status::Importing;
    }
    let before_stop = Resources::sample();
    let stop = Instant::now();
    if !service.stop() {
        return Err(io::Error::other("usage worker rejected the stop command"));
    }

    let deadline = stop + timeout;
    let mut stopped = None;
    while let Some(snapshot) = next_snapshot(&service, &wake, deadline) {
        if !snapshot.enabled && snapshot.status == Status::Disabled {
            stopped = Some(stop.elapsed());
            break;
        }
    }
    Ok(json!({
        "observed_active_import": active,
        "process_read_transfer_bytes_before_stop": difference(before_stop.rchar, baseline.rchar),
        "stop_to_disabled_ms": stopped.filter(|_| active).map(wall_ms),
        "target_ms": 100,
        "target_met": stopped.filter(|_| active).map(|time| time <= Duration::from_millis(100)),
    }))
}

fn args() -> io::Result<(Fixture, Duration)> {
    let mut args = env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "smoke".into());
    let secs = args
        .next()
        .map_or(Some(240), |value| value.parse::<u64>().ok());
    match (Fixture::config(&mode), secs, args.next()) {
        (Some(fixture), Some(secs @ 1..=3600), None) => Ok((fixture, Duration::from_secs(secs))),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: measure_usage [smoke|many|huge] [timeout-seconds, 1–3600]",
        )),
    }
}

fn main() -> io::Result<()> {
    let (fixture, timeout) = args()?;
    let scratch = Scratch::new()?;
    let source = scratch.0.join("sources");
    let setup = Instant::now();
    let expected = fixture.generate(&source)?;
    let fixture_ms = wall_ms(setup.elapsed());
    let import = import(scratch.0.join("import-cache"), &source, expected, timeout)?;
    let stop = cancellation(scratch.0.join("stop-cache"), &source, timeout)?;
    let peak_target = import
        .complete
        .then(|| import.resources["incremental_process_peak_rss_bytes"].as_u64())
        .flatten()
        .map(|bytes| bytes <= 64 * 1024 * 1024);
    let report = json!({
        "scenario": fixture.mode,
        "platform": env::consts::OS,
        "architecture": env::consts::ARCH,
        "sqlite_version": rusqlite::version(),
        "debug_assertions": cfg!(debug_assertions),
        "device_id": device(&source),
        "fixture_source_count": fixture.sources,
        "fixture_logical_bytes": fixture.bytes,
        "fixture_generation_ms": fixture_ms,
        "import_wall_ms": wall_ms(import.wall),
        "first_importing_ms": import.first_importing.map(wall_ms),
        "first_publish_observed_ms": import.first_published.map(wall_ms),
        "import_status": import.status.map(|status| format!("{status:?}")),
        "expected_tokens": expected.to_string(),
        "observed_tokens": import.tokens.map(|tokens| tokens.to_string()),
        "complete": import.complete,
        "incremental_process_peak_target_bytes": 64 * 1024 * 1024,
        "incremental_process_peak_target_met": peak_target,
        "import_resources": import.resources,
        "cancellation": stop,
        "measurement_basis": "CPU, RSS and IO are process-wide, including the worker and witness. RSS is the high-water increase above pre-import process peak. Read counters include the ledger and control files. Null means unavailable or unobserved."
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("format resource evidence")
    );
    if import.complete {
        Ok(())
    } else {
        Err(io::Error::other(
            "fixture import did not reach verified totals",
        ))
    }
}
