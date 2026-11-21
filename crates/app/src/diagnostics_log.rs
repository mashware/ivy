//! The window's own standard error, kept in a file as well, and a panic recorded there (D113).

mod sink;
mod swap;

use sink::RotatingSink;
use std::{
    ffi::OsStr,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, PoisonError, TryLockError, mpsc},
    time::{Duration, Instant, SystemTime},
};

/// Past this many bytes the file becomes `ivy.log.1`, so the two never hold more than twice this.
const CAP: u64 = 4 * 1024 * 1024;
const FILE: &str = "ivy.log";
/// How long closing waits for the last lines to reach the file (D57: every wait is bounded).
const SHUTDOWN: Duration = Duration::from_millis(100);
/// How long a panic waits for the file's lock before writing through a handle of its own.
const RECORD_PATIENCE: Duration = Duration::from_millis(50);

static PATH: OnceLock<PathBuf> = OnceLock::new();

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
    swapped: Option<swap::Swapped>,
    done: mpsc::Receiver<()>,
    // Held for the life of the capture, outside the thread, so no panic can close the read end:
    // with it closed every `eprintln!` would fail, and `eprintln!` panics when it fails.
    _reader: std::io::PipeReader,
}

impl Drop for Capture {
    fn drop(&mut self) {
        let Some(live) = &mut self.live else { return };
        if let Some(swapped) = live.swapped.take() {
            // Restoring removes the pipe's last writer, which is what lets the thread read its end.
            swapped.restore();
        }
        // Abandoned past the deadline: a child that inherited standard error keeps the pipe open.
        let _ = live.done.recv_timeout(SHUTDOWN);
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
pub fn start_in(directory: Option<PathBuf>) -> Capture {
    match capture(directory) {
        Ok(live) => Capture { live: Some(live) },
        Err(why) => {
            let _ = writeln!(std::io::stderr().lock(), "ivy: no log file: {why}");
            Capture { live: None }
        }
    }
}

fn capture(directory: Option<PathBuf>) -> Result<Live, String> {
    let directory = directory.ok_or("no directory to keep it in")?;
    create_directory(&directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    let path = directory.join(FILE);
    let sink =
        RotatingSink::open(path.clone(), CAP).map_err(|e| format!("{}: {e}", path.display()))?;
    let (reader, swapped) = swap::swap().map_err(|e| format!("standard error: {e}"))?;
    let sink = Arc::new(Mutex::new(sink));
    let original = swapped.original();
    let (finished, done) = mpsc::channel();
    // Nobody reads the pipe until the thread runs, so any failure up to then puts standard error
    // back before saying so: said into the pipe, it would be the line that fills it.
    let spawned = reader.try_clone().and_then(|from| {
        let sink = Arc::clone(&sink);
        std::thread::Builder::new()
            .name("log file".into())
            .spawn(move || {
                pump(from, &sink, original);
                let _ = finished.send(());
            })
    });
    if let Err(why) = spawned {
        swapped.restore();
        return Err(format!("no thread to write it: {why}"));
    }
    install_hook(sink, path.clone(), swapped.original());
    let _ = PATH.set(path);
    Ok(Live {
        swapped: Some(swapped),
        done,
        _reader: reader,
    })
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

/// Copies every byte read into the file first and the original standard error second, until the
/// pipe has no writer left. The file goes first so an original that stops reading — a parent
/// that never drains its pipe — cannot starve it. This thread must never write to standard error
/// itself: it is the only reader of that pipe.
fn pump(mut from: impl Read, sink: &Mutex<RotatingSink>, mut original: Option<impl Write>) {
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
            if let Some(original) = &mut original {
                let _ = original.write_all(chunk);
            }
        }));
    }
}

fn install_hook(sink: Arc<Mutex<RotatingSink>>, path: PathBuf, original: Option<File>) {
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
        // Written here, not through the pipe: a panic on the main thread ends the process before
        // the thread could copy it.
        write_record(&sink, &path, text.as_bytes());
        if let Some(mut original) = original.as_ref() {
            let _ = original.write_all(text.as_bytes());
        }
    }));
}

/// The lock is the thread's too, and a panic inside that thread holds it: waiting for it there
/// would never end, so past a short patience the record goes through a handle of its own.
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
        Os::Linux => Some(absolute(home)?.join(".local/state/ivy")),
        Os::Mac => Some(absolute(home)?.join("Library/Logs/Ivy")),
        Os::Windows => Some(absolute(local)?.join("Ivy").join("logs")),
    }
}

fn absolute(value: Option<&OsStr>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests;
