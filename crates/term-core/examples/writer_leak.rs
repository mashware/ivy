//! Reproduction of #159: does closing a console whose program stopped reading leave its writer
//! thread behind?
//!
//! `cargo run --release --example writer_leak -p term-core`

use std::thread;
use std::time::{Duration, Instant};

use term_core::{Pty, Size};

fn threads() -> usize {
    std::fs::read_dir("/proc/self/task")
        .map(|d| d.count())
        .unwrap_or(0)
}

fn wait_for(output: &std::sync::mpsc::Receiver<Vec<u8>>, marker: &str) {
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Ok(bytes) = output.recv_timeout(Duration::from_millis(100)) {
            seen.extend_from_slice(&bytes);
            if String::from_utf8_lossy(&seen).contains(marker) {
                return;
            }
        }
    }
    panic!("marker {marker} never arrived");
}

fn case(name: &str, script: &str) {
    let before = threads();
    let (mut pty, output) = Pty::spawn("sh", &["-c", script], Size::default()).expect("spawn");
    wait_for(&output, "ready");
    // Far more than the kernel takes, so the writer is left waiting for room.
    pty.write(&vec![b'x'; 256 * 1024]).expect("hand over");
    thread::sleep(Duration::from_millis(300));
    let backlog = pty.backlog();
    let live = threads();
    let started = Instant::now();
    drop(pty);
    let took = started.elapsed();
    // Keep the output channel alive but drain it, so the reader is not what holds a thread.
    let drain = thread::spawn(move || while output.recv_timeout(Duration::from_secs(6)).is_ok() {});
    let mut samples = Vec::new();
    for ms in [10u64, 100, 500, 2000, 5000] {
        let target = started + Duration::from_millis(ms);
        if let Some(wait) = target.checked_duration_since(Instant::now()) {
            thread::sleep(wait);
        }
        samples.push(format!("{ms}ms={}", threads()));
    }
    let _ = drain.join();
    println!(
        "{name}: before={before} live={live} backlog={backlog:?} drop={took:?} after: {}",
        samples.join(" ")
    );
}

fn main() {
    case(
        "deaf, no survivor",
        "stty raw -echo && printf ready && exec sleep 3600",
    );
    case(
        "deaf, canonical (echo on)",
        "printf ready && exec sleep 3600",
    );
    case(
        "deaf, survivor holds the slave",
        "{ setsid sleep 30 & } ; stty raw -echo && printf ready && exec sleep 3600",
    );
}
