use super::*;
use std::process::Command;

#[test]
fn each_platform_keeps_its_log_where_its_own_logs_live() {
    let home = std::env::temp_dir().join("ana");
    let home = Some(home.as_os_str());
    let under = |parts: &[&str]| {
        let mut path = std::env::temp_dir().join("ana");
        path.extend(parts);
        Some(path)
    };
    assert_eq!(
        directory(Os::Linux, None, home, None),
        under(&[".local", "state", "ivy"])
    );
    assert_eq!(
        directory(Os::Mac, None, home, None),
        under(&["Library", "Logs", "Ivy"])
    );
    assert_eq!(
        directory(Os::Windows, None, None, home),
        under(&["Ivy", "logs"])
    );
}

#[test]
fn an_absolute_state_directory_rules_on_every_platform_and_a_relative_one_is_no_place() {
    let isolated = std::env::temp_dir().join("isolated");
    let home = std::env::temp_dir().join("ana");
    let (state, home) = (Some(isolated.as_os_str()), Some(home.as_os_str()));
    for os in [Os::Linux, Os::Mac, Os::Windows] {
        assert_eq!(directory(os, state, home, home), Some(isolated.join("ivy")));
        assert_eq!(
            directory(os, Some(OsStr::new("state")), home, home),
            directory(os, None, home, home),
            "a relative state directory falls back to the platform's own"
        );
        for nowhere in [Some(OsStr::new("")), Some(OsStr::new("ana")), None] {
            assert_eq!(directory(os, None, nowhere, nowhere), None);
        }
    }
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

fn sink_in(name: &str) -> (PathBuf, Mutex<RotatingSink>) {
    let path = scratch(name).join(FILE);
    let sink = Mutex::new(RotatingSink::open(path.clone(), CAP).unwrap());
    (path, sink)
}

/// Three reads, so the pump sees three chunks.
fn three_chunks() -> impl Read {
    (&b"one\n"[..]).chain(&b"two\n"[..]).chain(&b"three\n"[..])
}

#[test]
fn the_pump_copies_every_byte_to_the_file_and_hands_it_on_until_the_pipe_ends() {
    let (path, sink) = sink_in("pump");
    let (to, from) = mpsc::sync_channel(ORIGINAL_BACKLOG);
    pump(three_chunks(), &sink, Some(to));
    assert_eq!(std::fs::read(&path).unwrap(), b"one\ntwo\nthree\n");
    assert_eq!(
        from.iter().collect::<Vec<_>>().concat(),
        b"one\ntwo\nthree\n"
    );
}

#[test]
fn an_original_that_stopped_reading_holds_up_neither_the_file_nor_the_pipe() {
    let (path, sink) = sink_in("stalled");
    // Room for one chunk and nobody taking it: a terminal paused with Ctrl+S.
    let (to, never_read) = mpsc::sync_channel(1);
    let (ended, pumped) = mpsc::channel();
    std::thread::spawn(move || {
        pump(three_chunks(), &sink, Some(to));
        let _ = ended.send(());
    });
    // A pump that waited for the terminal would never get here.
    assert!(
        pumped.recv_timeout(Duration::from_secs(5)).is_ok(),
        "the pump waited"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"one\ntwo\nthree\n");
    drop(never_read);
}

/// Four chunks; before the fourth is read, whatever the terminal has queued is taken.
struct Paused<'a> {
    chunks: Vec<&'static [u8]>,
    terminal: &'a mpsc::Receiver<Vec<u8>>,
    seen: Vec<Vec<u8>>,
}

impl Read for Paused<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.chunks.len() == 1 {
            self.seen.extend(self.terminal.try_iter());
        }
        let Some(chunk) = self.chunks.first().copied() else {
            return Ok(0);
        };
        self.chunks.remove(0);
        buffer[..chunk.len()].copy_from_slice(chunk);
        Ok(chunk.len())
    }
}

#[test]
fn what_the_terminal_missed_is_counted_to_it_when_it_takes_lines_again() {
    let (path, sink) = sink_in("missed");
    let (to, terminal) = mpsc::sync_channel(2);
    let mut paused = Paused {
        chunks: vec![b"one\n", b"two\n", b"three\n", b"four\n"],
        terminal: &terminal,
        seen: Vec::new(),
    };
    pump(&mut paused, &sink, Some(to));
    let mut seen = paused.seen;
    seen.extend(terminal.try_iter());
    let note = format!("[ivy: 6 bytes not shown here; see {}]\n", path.display());
    assert_eq!(
        seen,
        [&b"one\n"[..], b"two\n", note.as_bytes(), b"four\n"],
        "the terminal is told what it missed, before what follows"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"one\ntwo\nthree\nfour\n");
}

/// A terminal whose first write fails.
struct Flaky(usize, Vec<u8>);
impl Write for Flaky {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += 1;
        if self.0 == 1 {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        self.1.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_terminal_write_that_fails_does_not_stop_the_next_one() {
    let (to, chunks) = mpsc::sync_channel(ORIGINAL_BACKLOG);
    to.send(b"lost\n".to_vec()).unwrap();
    to.send(b"kept\n".to_vec()).unwrap();
    drop(to);
    let mut terminal = Flaky(0, Vec::new());
    copy_to_terminal(chunks, &mut terminal);
    assert_eq!(terminal.1, b"kept\n");
}

#[test]
fn a_lock_left_poisoned_by_a_panic_still_lets_the_file_be_written() {
    let (path, sink) = sink_in("poisoned");
    let _ = std::panic::catch_unwind(|| {
        let _held = sink.lock().unwrap();
        panic!("poison it");
    });
    assert!(sink.is_poisoned());
    pump(&b"line\n"[..], &sink, None);
    write_record(&sink, &path, b"record\n");
    assert_eq!(std::fs::read(&path).unwrap(), b"line\nrecord\n");
}

#[test]
fn a_record_written_while_the_lock_is_held_still_reaches_the_file() {
    let (path, sink) = sink_in("held");
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
// and its parent reads what reached the file, the child's real standard error, and what the child
// printed on its standard output.

const DIRECTORY: &str = "IVY_LOG_TEST_DIRECTORY";
const MARKER: &str = "a line written to standard error";

struct Child {
    file: String,
    original: String,
    said: String,
}

fn child(name: &str, directory: &Path, environment: &[(&str, &OsStr)]) -> Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            &format!("diagnostics_log::tests::{name}"),
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(DIRECTORY, directory)
        .env_remove("XDG_STATE_HOME");
    // A measuring session the developer left exported would veto the capture being tested.
    for measuring in crate::MEASUREMENT_VARIABLES {
        command.env_remove(measuring);
    }
    let output = command.envs(environment.iter().copied()).output().unwrap();
    assert!(output.status.success(), "{name} failed: {output:?}");
    Child {
        file: std::fs::read_to_string(directory.join(FILE)).unwrap_or_default(),
        original: String::from_utf8_lossy(&output.stderr).into_owned(),
        said: String::from_utf8_lossy(&output.stdout).into_owned(),
    }
}

fn directory_from_parent() -> Option<PathBuf> {
    std::env::var_os(DIRECTORY).map(PathBuf::from)
}

#[test]
#[ignore = "run by its parent in a process of its own"]
fn child_writes_one_line() {
    let mut capture = start_in(directory_from_parent());
    println!("log path: {:?}", path());
    eprintln!("{MARKER}");
    // Putting the original back is what ends the pipe: without it the copying threads would
    // never finish and closing would report the deadline.
    assert!(capture.close(), "the copying threads did not finish");
    if path().is_some() {
        assert!(
            start_in(directory_from_parent()).live.is_none(),
            "a second capture in one process is refused"
        );
    }
}

#[test]
fn a_line_on_standard_error_reaches_the_file_and_still_reaches_the_original() {
    let dir = scratch("line");
    let child = child("child_writes_one_line", &dir, &[]);
    assert_eq!(child.file, format!("{MARKER}\n"));
    assert!(child.original.contains(MARKER), "{}", child.original);
    assert!(
        child
            .original
            .contains("ivy: no log file: this process already keeps one"),
        "{}",
        child.original
    );
    let named = format!("log path: Some({:?})", dir.join(FILE));
    assert!(child.said.contains(&named), "{}", child.said);
}

#[test]
fn a_directory_that_cannot_be_made_leaves_standard_error_as_it_was_and_says_why() {
    let dir = scratch("refused");
    let blocker = dir.join("a file");
    std::fs::write(&blocker, b"").unwrap();
    let child = child("child_writes_one_line", &blocker.join("logs"), &[]);
    let because = format!("ivy: no log file: {}: ", blocker.join("logs").display());
    assert!(child.original.contains(&because), "{}", child.original);
    assert_eq!(
        child.original.matches("ivy: no log file: ").count(),
        1,
        "a start that failed leaves the next one free to try: {}",
        child.original
    );
    assert!(child.original.contains(MARKER), "{}", child.original);
    assert!(child.said.contains("log path: None"), "{}", child.said);
}

#[test]
#[ignore = "run by its parent in a process of its own"]
fn child_starts_from_its_environment() {
    let capture = start();
    eprintln!("{MARKER}");
    drop(capture);
}

#[test]
fn the_window_keeps_its_log_under_the_state_directory_and_a_measured_run_keeps_none() {
    let dir = scratch("environment");
    let state = [("XDG_STATE_HOME", dir.as_os_str())];
    let kept = child(
        "child_starts_from_its_environment",
        &dir.join("ivy"),
        &state,
    );
    assert_eq!(kept.file, format!("{MARKER}\n"));
    let measured_dir = scratch("measured");
    let measured = child(
        "child_starts_from_its_environment",
        &measured_dir.join("ivy"),
        &[
            ("XDG_STATE_HOME", measured_dir.as_os_str()),
            ("MEASURE_BOARD", OsStr::new("1")),
        ],
    );
    assert!(measured.original.contains(MARKER), "{}", measured.original);
    assert!(
        !measured_dir.join("ivy").exists(),
        "a measured run created the log"
    );
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
    // Read before the capture closes: the record must not depend on the copying thread.
    let written = std::fs::read_to_string(path().unwrap()).unwrap();
    println!("record before closing: {}", written.contains("--- panic "));
    drop(capture);
}

#[test]
fn a_panic_leaves_one_record_in_the_file_and_one_on_the_original() {
    let dir = scratch("panic");
    let child = child("child_panics_on_a_named_thread", &dir, &[]);
    assert!(
        child.said.contains("record before closing: true"),
        "{}",
        child.said
    );
    for text in [&child.file, &child.original] {
        assert_eq!(text.matches("--- panic ").count(), 1, "{text}");
        assert!(
            text.contains(" thread 'probe' ---\nthe probe gave up\nat "),
            "{text}"
        );
        let at = text.split("\nat ").nth(1).unwrap();
        let place = at.lines().next().unwrap();
        assert!(
            place.contains("diagnostics_log") && place.contains("tests.rs:"),
            "{place}"
        );
        let numbers: Vec<_> = place.rsplit(':').take(2).collect();
        assert!(
            numbers.iter().all(|n| n.parse::<u32>().is_ok()),
            "line and column: {place}"
        );
        assert!(at.contains("0: "), "a backtrace follows: {at}");
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
    let child = child("child_fails_to_open_the_window", &dir, &[]);
    for text in [&child.file, &child.original] {
        assert!(
            text.contains("Error: AppCreation(\"no display\")"),
            "{text}"
        );
    }
}
