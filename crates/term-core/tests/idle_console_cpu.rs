//! **A console that is waiting costs no processor time — waiting for output, or waiting for room.**
//!
//! On Linux both threads of a console wait in a `poll`, because the master is non-blocking (D29,
//! amended): the reader for the program's output, the writer for room in the program's input. Either
//! one retrying «would block» without that wait would still deliver every byte — every other test
//! stays green over it — and would burn a whole core for as long as the console waited. That is only
//! visible as time spent, so this measures it.
//!
//! **A file of its own, and one test in it, and that is the point**: an integration test runs as its
//! own process, so the process's processor clock counts this test's consoles and nobody else's — and
//! with a single test, not even a sibling running in parallel. The two measurements go one after the
//! other for that reason.
#![cfg(target_os = "linux")]

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use term_core::{Pty, Size};

/// How long a console is left waiting while its cost is measured.
const IDLE: Duration = Duration::from_millis(500);

/// What that wait may cost at most. A spinning thread spends the whole of [`IDLE`], ten times this;
/// a waiting one spends a small fraction of it.
const AT_MOST: Duration = Duration::from_millis(50);

/// The processor time this whole process has used, every thread included.
fn processor_time() -> Duration {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: one valid, exclusively borrowed `timespec`.
    let answered = unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut now) };
    assert_eq!(answered, 0, "{}", std::io::Error::last_os_error());
    Duration::new(now.tv_sec as u64, now.tv_nsec as u32)
}

fn launch(script: &str, marker: &str) -> (Pty, Receiver<Vec<u8>>) {
    let (pty, output) = Pty::spawn("sh", &["-c", script], Size::default()).expect("launch sh");
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !String::from_utf8_lossy(&seen).contains(marker) {
        assert!(Instant::now() < deadline, "the marker never arrived");
        if let Ok(bytes) = output.recv_timeout(Duration::from_millis(100)) {
            seen.extend_from_slice(&bytes);
        }
    }
    (pty, output)
}

fn spent_over_idle() -> Duration {
    let before = processor_time();
    std::thread::sleep(IDLE);
    processor_time() - before
}

#[test]
fn a_waiting_console_costs_no_processor_time() {
    // The reader, waiting for a program that says nothing more.
    let (pty, output) = launch(
        "printf silent-from-now-on && exec sleep 60",
        "silent-from-now-on",
    );
    let spent = spent_over_idle();
    assert!(
        spent < AT_MOST,
        "an idle console cost {spent:?} of processor time in {IDLE:?}: its reader is spinning \
         instead of waiting for output"
    );
    drop(pty);
    drop(output);

    // The writer, waiting for room in a program that stopped reading. `&&` because with the echo
    // still on the line discipline would hand the payload straight back and nothing would wait.
    let (mut pty, output) = launch(
        "stty raw -echo && printf deaf-from-now-on && exec sleep 60",
        "deaf-from-now-on",
    );
    pty.write(&vec![b'z'; 256 * 1024]).expect("hand over");
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        pty.backlog().is_some_and(|backlog| backlog.bytes > 0),
        "the program took everything: the writer is not waiting for room, so its cost says nothing"
    );
    let spent = spent_over_idle();
    assert!(
        spent < AT_MOST,
        "a console whose program stopped reading cost {spent:?} of processor time in {IDLE:?}: its \
         writer is spinning instead of waiting for room"
    );
    drop(pty);
    drop(output);
}
