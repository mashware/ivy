//! What the program on the far side of the pseudoterminal **received**, read from a file it wrote.
//!
//! **Test support, and never the screen or the master.** Every test about input can check the bytes
//! handed to the pseudoterminal, and those are ours; what the program got is on the other side of the
//! line discipline, which translates, holds and truncates — and which **echoes**. With `ECHO` on, the
//! kernel writes back out of the master what was written in, rendering a control exactly as `cat -v`
//! would, so a test that reads the screen or counts bytes on the master can be satisfied by the
//! kernel instead of by the program. That happened twice on one day: a mouse test that stayed green
//! against an answer the kernel had written, and a probe of a long paste that reported nothing lost
//! because it was counting the echo. A file is written by the program alone.
//!
//! **No snapshot of the terminal's mode**, and that was weighed rather than forgotten. The prelude is
//! chained with `&&`, so one that fails never marks the program ready. One that exits zero without
//! setting its mode is left to the comparison: a mode that governs input — canonical lines, `ICRNL`,
//! signals — changes the bytes the program reads, so comparing the file byte for byte catches it. A
//! mode that governs only the echo does not, and does not need to: the echo never reaches the file.
//!
//! Compiled for this crate's tests and, behind the «testing» feature, for the tests of whoever hosts
//! it. The decision is that a host turns the feature on only from a development dependency.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A file a program writes what it reads into, in a directory of its own that goes when this does.
pub struct Received {
    dir: PathBuf,
}

impl Received {
    /// Makes a fresh directory for one program. `label` names it for whoever finds it left behind
    /// by a test that was killed.
    pub fn new(label: &str) -> Self {
        // Unique per instance and not only per process: tests run in parallel threads of one process,
        // and two with the same label would otherwise read each other's file.
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ivy-received-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // A killed run can leave this name behind for a later process that reuses the pid, and its
        // `ready` would mark a program ready that never ran its prelude. Created exclusively after
        // that, so a directory somebody else put there in between fails the test instead of being used.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).expect("create the directory the program writes into");
        Self { dir }
    }

    /// The `sh` script that runs `prelude`, marks the program ready only if it succeeded, and then
    /// becomes `cat` writing everything it reads into the file.
    ///
    /// An empty prelude leaves the terminal in the mode it starts in. Anything else is chained with
    /// `&&` and never `;`: a `;` marks ready after a `stty` that failed. That is why a prelude may not
    /// carry a `;` or a line break of its own — each would split it into commands whose failures the
    /// chain no longer sees. Several commands are joined with `&&` inside it.
    pub fn script(&self, prelude: &str) -> String {
        assert!(
            !prelude.contains([';', '\n']),
            "a prelude joins its commands with && so a failure stops it: {prelude:?}"
        );
        let ready = shell_word(&self.ready());
        let file = shell_word(&self.file());
        if prelude.is_empty() {
            format!("touch {ready} && exec cat > {file}")
        } else {
            format!("{prelude} && touch {ready} && exec cat > {file}")
        }
    }

    /// Whether the prelude succeeded and `cat` is about to read.
    fn is_ready(&self) -> bool {
        self.ready().exists()
    }

    /// Waits for the program to be ready, and fails the test if it is not within `within`.
    pub fn wait_until_ready(&self, within: Duration) {
        let deadline = Instant::now() + within;
        while !self.is_ready() {
            assert!(
                Instant::now() < deadline,
                "the program never got past its prelude in {within:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Everything the program has written so far; empty until it has started.
    pub fn so_far(&self) -> Vec<u8> {
        std::fs::read(self.file()).unwrap_or_default()
    }

    /// What the program has received once it holds at least `len` bytes, or whatever it holds when
    /// `within` runs out — so the caller compares, and its message says what did arrive.
    pub fn wait_for(&self, len: usize, within: Duration) -> Vec<u8> {
        let deadline = Instant::now() + within;
        loop {
            let got = self.so_far();
            if got.len() >= len || Instant::now() >= deadline {
                return got;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn ready(&self) -> PathBuf {
        self.dir.join("ready")
    }

    fn file(&self) -> PathBuf {
        self.dir.join("received")
    }
}

impl Drop for Received {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A path as one shell word. `$TMPDIR` is honoured by `temp_dir`, and nothing checks it is free of
/// spaces; a single quote in it is refused rather than escaped, since nobody has met one.
fn shell_word(path: &Path) -> String {
    let path = path.display().to_string();
    assert!(
        !path.contains('\''),
        "a temporary directory with a single quote in it: {path}"
    );
    format!("'{path}'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pty, Size};
    use std::sync::mpsc::Receiver;

    const DEADLINE: Duration = Duration::from_secs(10);

    fn console_running(script: &str) -> (Pty, Receiver<Vec<u8>>) {
        Pty::spawn("sh", &["-c", script], Size::default()).expect("launch the program")
    }

    fn wait_for_the_master_to_show(pty_output: &Receiver<Vec<u8>>, text: &str) {
        let mut seen = Vec::new();
        while !String::from_utf8_lossy(&seen).contains(text) {
            match pty_output.recv_timeout(DEADLINE) {
                Ok(bytes) => seen.extend_from_slice(&bytes),
                Err(_) => panic!(
                    "the master never showed {text:?} in {DEADLINE:?}: {:?}",
                    String::from_utf8_lossy(&seen)
                ),
            }
        }
    }

    /// **What the echo shows and the program did not get is not reported**, and what the program did
    /// get is — after the kernel's translation, not before it.
    ///
    /// A console starts canonical with echo on, so «abc» with no line end is echoed straight back out
    /// of the master while `cat` receives nothing: the line discipline is still holding the line. A
    /// helper that read the master would report «abc» here. The absence is clocked by an arrival and
    /// not by a timer: the line end is written next, and once the line has reached the file it must be
    /// exactly `abc\n` — `ICRNL` turned the `\r` the Enter key sends into the `\n` the program reads,
    /// where the master's echo carries `abc\r\n`.
    #[test]
    fn it_reports_what_the_program_got_and_not_what_the_echo_showed() {
        let received = Received::new("echo");
        let (mut pty, pty_output) = console_running(&received.script(""));
        received.wait_until_ready(DEADLINE);

        pty.write(b"abc").expect("write into the console");
        wait_for_the_master_to_show(&pty_output, "abc");
        assert!(
            received.so_far().is_empty(),
            "the program cannot have read a line that has not ended, and the helper says it holds \
             {:?}",
            String::from_utf8_lossy(&received.so_far())
        );

        pty.write(b"\r").expect("write into the console");
        let got = received.wait_for(4, DEADLINE);
        assert_eq!(
            got,
            b"abc\n",
            "the program should hold the line as the line discipline delivered it; it holds {:?}",
            String::from_utf8_lossy(&got)
        );
    }

    /// **A prelude cannot bring the `;` back from inside**: `stty raw; stty -echo` would chain the
    /// ready marker to the last command only, and a failed first one would go unseen.
    #[test]
    #[should_panic(expected = "a prelude joins its commands with &&")]
    fn a_prelude_with_a_semicolon_of_its_own_is_refused() {
        Received::new("semicolon").script("stty raw; stty -echo");
    }

    /// **A prelude that fails never marks the program ready.** Clocked by the program's own ending,
    /// which is an arrival: by the time the console has no program, whatever the script was going to
    /// do it has done. A ready marker showing up first ends the wait too, so a script that marks
    /// ready anyway fails on what it did rather than on a program that, now being `cat`, never ends.
    #[test]
    fn a_prelude_that_fails_never_marks_the_program_ready() {
        let received = Received::new("failed-prelude");
        let (pty, _pty_output) = console_running(&received.script("false"));

        let deadline = Instant::now() + DEADLINE;
        while pty.ending().is_none() && !received.is_ready() {
            assert!(
                Instant::now() < deadline,
                "the program neither ended nor got ready in {DEADLINE:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(
            !received.is_ready(),
            "a prelude that failed marked the program ready"
        );
    }
}
