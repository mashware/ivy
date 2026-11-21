//! The window's own standard error, kept in a file as well, and a panic recorded there (D113).

mod sink;
mod swap;

use sink::RotatingSink;
use std::{
    ffi::OsStr,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, PoisonError, TryLockError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime},
};

/// Past this many bytes the file becomes `ivy.log.1`. The rotation happens between writes, so
/// each of the two can pass it by the write that reached it.
const CAP: u64 = 4 * 1024 * 1024;
const FILE: &str = "ivy.log";
/// How long closing waits for the last lines to reach the file (D57: every wait is bounded).
const SHUTDOWN: Duration = Duration::from_millis(100);
/// How long a panic waits for the file's lock before writing through a handle of its own.
const RECORD_PATIENCE: Duration = Duration::from_millis(50);
/// Chunks waiting for the original standard error. Past this they are dropped from it, never
/// from the file: a terminal that stopped reading must not stop the log or the window.
const ORIGINAL_BACKLOG: usize = 256;

static PATH: OnceLock<PathBuf> = OnceLock::new();
/// Standard error and the panic hook belong to the whole process: a second capture would save
/// the first one's pipe as «the original».
static TAKEN: AtomicBool = AtomicBool::new(false);

/// The file this window keeps, when it keeps one.
pub(crate) fn path() -> Option<&'static Path> {
    PATH.get().map(PathBuf::as_path)
}

/// While this lives, whatever the process writes to standard error also lands in the log.
/// Dropping it puts the original standard error back and waits, bounded, for the last lines.
#[must_use = "dropping the capture ends it"]
pub struct Capture {
    live: Option<Live>,
}

struct Live {
    swapped: swap::Swapped,
    done: mpsc::Receiver<()>,
    /// How many threads signal on `done` when they have copied everything.
    copiers: usize,
    // Held for the life of the capture, outside the thread, so that a copying thread that ends
    // early leaves writers blocked rather than failing — and `eprintln!` panics when it fails.
    _reader: std::io::PipeReader,
}

impl Capture {
    /// Ends the capture; true when every copying thread finished inside the deadline.
    fn close(&mut self) -> bool {
        let Some(live) = self.live.take() else {
            return true;
        };
        let Live {
            swapped,
            done,
            copiers,
            _reader,
        } = live;
        // Putting the original back removes the pipe's writer, which is what ends the copying.
        drop(swapped);
        let deadline = Instant::now() + SHUTDOWN;
        // Abandoned past the deadline: a program started with an inherited standard error keeps the
        // pipe open, and its threads have nothing left to say that the window is waiting for.
        (0..copiers).all(|_| {
            done.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .is_ok()
        })
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.close();
    }
}

/// What `main` ends with. A window that could not open is said here, while the capture still
/// stands: Rust's own report of an `Err` from `main` is printed after `main` returns, when the
/// capture is gone.
pub fn finish(outcome: eframe::Result, capture: Capture) -> std::process::ExitCode {
    let code = match outcome {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(std::io::stderr().lock(), "Error: {error:?}");
            std::process::ExitCode::FAILURE
        }
    };
    drop(capture);
    code
}

/// Starts keeping the log where this platform keeps logs, unless a measured run asked for the
/// window (D113: a measured run captures nothing).
pub fn start() -> Capture {
    if crate::any_measurement_variable(|which| std::env::var_os(which).is_some()) {
        return Capture { live: None };
    }
    start_in(directory_here())
}

/// Starts keeping the log in `directory`. Anything that fails leaves standard error as it was
/// and says why on it, once.
fn start_in(directory: Option<PathBuf>) -> Capture {
    match capture(directory) {
        Ok(live) => Capture { live: Some(live) },
        Err(why) => {
            let _ = writeln!(std::io::stderr().lock(), "ivy: no log file: {why}");
            Capture { live: None }
        }
    }
}

fn capture(directory: Option<PathBuf>) -> Result<Live, String> {
    if TAKEN.swap(true, Ordering::AcqRel) {
        return Err("this process already keeps one".into());
    }
    let directory = directory.ok_or("no directory to keep it in")?;
    create_directory(&directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    let path = directory.join(FILE);
    let sink =
        RotatingSink::open(path.clone(), CAP).map_err(|e| format!("{}: {e}", path.display()))?;
    // From here every early return drops `swapped`, which puts standard error back before the
    // line saying why is written.
    let (reader, swapped) = swap::swap().map_err(|e| format!("standard error: {e}"))?;
    let sink = Arc::new(Mutex::new(sink));
    let (finished, done) = mpsc::channel();
    let mut copiers = 1;
    let forward = match swapped.original() {
        Some(original) => {
            copiers += 1;
            Some(forwarder(original, finished.clone()).map_err(|e| format!("no thread: {e}"))?)
        }
        None => None,
    };
    let from = reader
        .try_clone()
        .map_err(|e| format!("standard error: {e}"))?;
    let copied = Arc::clone(&sink);
    std::thread::Builder::new()
        .name("log file".into())
        .spawn(move || {
            pump(from, &copied, forward);
            let _ = finished.send(());
        })
        .map_err(|e| format!("no thread: {e}"))?;
    install_hook(sink, path.clone(), swapped.original());
    let _ = PATH.set(path);
    Ok(Live {
        swapped,
        done,
        copiers,
        _reader: reader,
    })
}

/// The thread that writes to the original standard error, so that one which stops reading — a
/// terminal paused with Ctrl+S, a parent that never drains its pipe — holds up only itself.
fn forwarder(
    mut original: Box<dyn Write + Send>,
    finished: mpsc::Sender<()>,
) -> std::io::Result<mpsc::SyncSender<Vec<u8>>> {
    let (to, chunks) = mpsc::sync_channel::<Vec<u8>>(ORIGINAL_BACKLOG);
    std::thread::Builder::new()
        .name("log file: terminal".into())
        .spawn(move || {
            for chunk in chunks {
                let _ = original.write_all(&chunk);
            }
            let _ = finished.send(());
        })?;
    Ok(to)
}

#[cfg(unix)]
fn create_directory(directory: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)
}

#[cfg(not(unix))]
fn create_directory(directory: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)
}

/// Copies every byte read into the file, and hands it on to the original standard error without
/// waiting for it, until the pipe has no writer left. This thread must never write to standard
/// error itself: it is the only reader of that pipe.
fn pump(
    mut from: impl Read,
    sink: &Mutex<RotatingSink>,
    forward: Option<mpsc::SyncSender<Vec<u8>>>,
) {
    let mut buffer = vec![0; 8192];
    loop {
        let read = match from.read(&mut buffer) {
            Ok(0) => return,
            Ok(read) => read,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        let chunk = &buffer[..read];
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = sink
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .write(chunk);
        }));
        if let Some(forward) = &forward {
            let _ = forward.try_send(chunk.to_vec());
        }
    }
}

fn install_hook(
    sink: Arc<Mutex<RotatingSink>>,
    path: PathBuf,
    original: Option<Box<dyn Write + Send>>,
) {
    let original = Mutex::new(original);
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let text = record(
            &stamp(SystemTime::now()),
            thread.name().unwrap_or("<unnamed>"),
            info.payload_as_str().unwrap_or("Box<dyn Any>"),
            info.location()
                .map(|at| format!("{}:{}:{}", at.file(), at.line(), at.column())),
            &std::backtrace::Backtrace::force_capture().to_string(),
        );
        // Written here and not through the pipe: this replaces Rust's own report, and a panic in
        // the copying thread could not copy its own record.
        write_record(&sink, &path, text.as_bytes());
        if let Some(original) = original
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            let _ = original.write_all(text.as_bytes());
        }
    }));
}

/// The lock is the copying thread's too, and a panic inside that thread holds it: waiting for it
/// there would never end, so past a short patience the record goes through a handle of its own.
fn write_record(sink: &Mutex<RotatingSink>, path: &Path, text: &[u8]) {
    let started = Instant::now();
    loop {
        match sink.try_lock() {
            Ok(mut sink) => {
                let _ = sink.write(text);
                return;
            }
            Err(TryLockError::Poisoned(poisoned)) => {
                let _ = poisoned.into_inner().write(text);
                return;
            }
            Err(TryLockError::WouldBlock) if started.elapsed() < RECORD_PATIENCE => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(TryLockError::WouldBlock) => {
                if let Ok(mut file) = sink::append(path) {
                    let _ = file.write_all(text);
                }
                return;
            }
        }
    }
}

fn record(
    stamp: &str,
    thread: &str,
    message: &str,
    location: Option<String>,
    backtrace: &str,
) -> String {
    let at = location.unwrap_or_else(|| "an unknown location".into());
    format!("--- panic {stamp} thread '{thread}' ---\n{message}\nat {at}\n{backtrace}\n\n")
}

/// `2026-11-21T16:30:05Z`, in UTC: a log is read next to other machines' times.
fn stamp(now: SystemTime) -> String {
    let seconds = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = civil(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The calendar date `days` after 1970-01-01, by Howard Hinnant's days-to-civil algorithm.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn directory_here() -> Option<PathBuf> {
    let os = if cfg!(target_os = "macos") {
        Os::Mac
    } else if cfg!(windows) {
        Os::Windows
    } else {
        Os::Linux
    };
    directory(
        os,
        std::env::var_os("XDG_STATE_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("LOCALAPPDATA").as_deref(),
    )
}

#[derive(Clone, Copy)]
enum Os {
    Linux,
    Mac,
    Windows,
}

/// Where the log goes, from the values the environment holds. An absolute XDG_STATE_HOME rules
/// on every platform, as XDG_CONFIG_HOME does for the configuration, so an isolated run keeps
/// its own log. A value that is not an absolute path is no place: a relative one would put the
/// log inside whatever directory the window was started from.
fn directory(
    os: Os,
    state: Option<&OsStr>,
    home: Option<&OsStr>,
    local: Option<&OsStr>,
) -> Option<PathBuf> {
    if let Some(state) = absolute(state) {
        return Some(state.join("ivy"));
    }
    match os {
        Os::Linux => Some(absolute(home)?.join(".local").join("state").join("ivy")),
        Os::Mac => Some(absolute(home)?.join("Library").join("Logs").join("Ivy")),
        Os::Windows => Some(absolute(local)?.join("Ivy").join("logs")),
    }
}

fn absolute(value: Option<&OsStr>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests;
