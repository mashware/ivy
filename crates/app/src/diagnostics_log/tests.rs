use super::*;
use std::{ffi::OsString, process::Command};

#[test]
fn each_platform_keeps_its_log_where_its_own_logs_live() {
    let home = Some(OsStr::new("/home/ana"));
    assert_eq!(
        directory(Os::Linux, None, home, None),
        Some(PathBuf::from("/home/ana/.local/state/ivy"))
    );
    assert_eq!(
        directory(Os::Mac, None, home, None),
        Some(PathBuf::from("/home/ana/Library/Logs/Ivy"))
    );
    #[cfg(windows)]
    assert_eq!(
        directory(
            Os::Windows,
            None,
            None,
            Some(OsStr::new(r"C:\Users\ana\AppData\Local"))
        ),
        Some(PathBuf::from(r"C:\Users\ana\AppData\Local\Ivy\logs"))
    );
}

#[test]
fn an_absolute_state_directory_rules_on_every_platform_and_a_relative_one_is_no_place() {
    let state = Some(OsStr::new("/tmp/isolated"));
    for os in [Os::Linux, Os::Mac] {
        assert_eq!(
            directory(os, state, Some(OsStr::new("/home/ana")), None),
            Some(PathBuf::from("/tmp/isolated/ivy"))
        );
        assert_eq!(
            directory(
                os,
                Some(OsStr::new("state")),
                Some(OsStr::new("/home/ana")),
                None
            ),
            directory(os, None, Some(OsStr::new("/home/ana")), None),
            "a relative state directory falls back to the platform's own"
        );
        assert_eq!(directory(os, None, Some(OsStr::new("")), None), None);
        assert_eq!(directory(os, None, Some(OsStr::new("ana")), None), None);
        assert_eq!(directory(os, None, None, None), None);
    }
    assert_eq!(
        directory(Os::Windows, None, None, Some(OsStr::new("Local"))),
        None
    );
}

#[test]
fn the_stamp_is_a_utc_calendar_time() {
    let at = |seconds| stamp(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
    assert_eq!(at(0), "1970-01-01T00:00:00Z");
    assert_eq!(at(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(at(1_795_278_605), "2026-11-21T16:30:05Z");
    assert_eq!(at(4_107_542_399), "2100-02-28T23:59:59Z");
}

#[test]
fn a_record_names_its_thread_message_and_place_before_the_backtrace() {
    let text = record("T", "pump", "boom", Some("src/a.rs:1:2".into()), "0: frame");
    assert_eq!(
        text,
        "--- panic T thread 'pump' ---\nboom\nat src/a.rs:1:2\n0: frame\n\n"
    );
    assert!(record("T", "x", "y", None, "").contains("at an unknown location\n"));
}

/// A standard error that stopped existing: what the Dock leaves a Mac app.
struct Gone;
impl Write for Gone {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn the_pump_copies_every_byte_to_the_file_and_the_original_until_the_pipe_ends() {
    let dir = scratch("pump");
    let sink = Mutex::new(RotatingSink::open(dir.join(FILE), CAP).unwrap());
    let mut original = Vec::new();
    pump(&b"one\ntwo\n"[..], &sink, Some(&mut original));
    assert_eq!(std::fs::read(dir.join(FILE)).unwrap(), b"one\ntwo\n");
    assert_eq!(original, b"one\ntwo\n");
}

#[test]
fn an_original_that_fails_does_not_stop_the_file() {
    let dir = scratch("gone");
    let sink = Mutex::new(RotatingSink::open(dir.join(FILE), CAP).unwrap());
    pump(&b"still kept\n"[..], &sink, Some(Gone));
    assert_eq!(std::fs::read(dir.join(FILE)).unwrap(), b"still kept\n");
}

#[test]
fn a_record_written_while_the_lock_is_held_still_reaches_the_file() {
    let dir = scratch("held");
    let path = dir.join(FILE);
    let sink = Mutex::new(RotatingSink::open(path.clone(), CAP).unwrap());
    let held = sink.lock().unwrap();
    write_record(&sink, &path, b"record\n");
    drop(held);
    assert_eq!(std::fs::read(&path).unwrap(), b"record\n");
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ivy-log-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// The capture replaces the process's standard error, which the test runner shares between every
// test it runs in parallel. So each case below runs a single ignored test in a process of its own,
// and its parent reads what reached the file and what reached that process's real standard error.

const DIRECTORY: &str = "IVY_LOG_TEST_DIRECTORY";
const MARKER: &str = "a line written to standard error";

fn child(name: &str, directory: &Path) -> (String, String) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            &format!("diagnostics_log::tests::{name}"),
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(DIRECTORY, directory)
        .output()
        .unwrap();
    assert!(output.status.success(), "{name} failed: {output:?}");
    let file = std::fs::read_to_string(directory.join(FILE)).unwrap_or_default();
    (file, String::from_utf8_lossy(&output.stderr).into_owned())
}

fn directory_from_parent() -> Option<PathBuf> {
    std::env::var_os(DIRECTORY).map(|dir: OsString| PathBuf::from(dir))
}

#[test]
#[ignore = "run by its parent in a process of its own"]
fn child_writes_one_line() {
    let capture = start_in(directory_from_parent());
    eprintln!("{MARKER}");
    let closing = Instant::now();
    drop(capture);
    // Putting the original back is what ends the pipe; without it closing would sit out the
    // whole deadline every time.
    assert!(closing.elapsed() < SHUTDOWN, "{:?}", closing.elapsed());
}

#[test]
fn a_line_on_standard_error_reaches_the_file_and_still_reaches_the_original() {
    let dir = scratch("line");
    let (file, original) = child("child_writes_one_line", &dir);
    assert_eq!(file, format!("{MARKER}\n"));
    assert!(original.contains(MARKER), "{original}");
}

#[test]
fn a_directory_that_cannot_be_made_leaves_standard_error_as_it_was_and_says_why() {
    let dir = scratch("refused");
    let blocker = dir.join("a file");
    std::fs::write(&blocker, b"").unwrap();
    let (_, original) = child("child_writes_one_line", &blocker.join("logs"));
    assert!(original.contains("ivy: no log file: "), "{original}");
    assert!(original.contains(MARKER), "{original}");
    assert!(!blocker.join("logs").exists());
}

#[test]
#[ignore = "run by its parent in a process of its own"]
fn child_panics_on_a_named_thread() {
    let capture = start_in(directory_from_parent());
    let probe = std::thread::Builder::new()
        .name("probe".into())
        .spawn(|| panic!("the probe gave up"))
        .unwrap();
    // Bounded: the thread panics as soon as it starts.
    let _ = probe.join();
    drop(capture);
}

#[test]
fn a_panic_leaves_one_record_in_the_file_and_one_on_the_original() {
    let dir = scratch("panic");
    let (file, original) = child("child_panics_on_a_named_thread", &dir);
    for text in [&file, &original] {
        assert_eq!(text.matches("--- panic ").count(), 1, "{text}");
        assert!(
            text.contains(" thread 'probe' ---\nthe probe gave up\nat "),
            "{text}"
        );
        assert!(text.contains("diagnostics_log/tests.rs:"), "{text}");
    }
}

#[test]
#[ignore = "run by its parent in a process of its own"]
fn child_fails_to_open_the_window() {
    let capture = start_in(directory_from_parent());
    let failed: eframe::Result = Err(eframe::Error::AppCreation("no display".into()));
    assert_eq!(finish(failed, capture), std::process::ExitCode::FAILURE);
}

#[test]
fn a_window_that_cannot_open_says_so_in_the_file_and_on_the_original() {
    let dir = scratch("startup");
    let (file, original) = child("child_fails_to_open_the_window", &dir);
    assert!(
        file.contains("Error: AppCreation(\"no display\")"),
        "{file}"
    );
    assert!(
        original.contains("Error: AppCreation(\"no display\")"),
        "{original}"
    );
}
