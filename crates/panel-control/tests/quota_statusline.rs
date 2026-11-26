#![cfg(target_os = "linux")]

use panel_control::quota::{self, Admission, ENDPOINT_ENV, Frame, MAX_FRAME, Outcome, Window};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::UnixListener;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

fn helper(endpoint: &Path, original: &str) -> Command {
    let mut helper = Command::new(env!("CARGO_BIN_EXE_ivy-session"));
    helper
        .arg("quota-statusline")
        .arg(original)
        .env(ENDPOINT_ENV, endpoint)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    helper
}

fn readable(descriptor: &impl AsRawFd) -> io::Result<()> {
    let mut state = libc::pollfd {
        fd: descriptor.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: the descriptor remains open and poll updates exactly the single supplied entry.
    match unsafe { libc::poll(&mut state, 1, 2_000) } {
        1 => Ok(()),
        0 => Err(io::ErrorKind::TimedOut.into()),
        _ => Err(io::Error::last_os_error()),
    }
}

struct Endpoint {
    path: PathBuf,
    _dir: panel_contract::PanelDir,
}

impl Endpoint {
    fn bind() -> (Self, UnixListener) {
        let dir = panel_contract::PanelDir::create().unwrap();
        let path = dir.path().join("quota");
        let listener = UnixListener::bind(&path).unwrap();
        (Self { path, _dir: dir }, listener)
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn quota_statusline_preserves_original_while_publishing_only_the_projection() {
    let (endpoint, listener) = Endpoint::bind();
    let broker = std::thread::spawn(move || {
        readable(&listener).unwrap();
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        quota::write_line(&stream, &Admission { v: 1, sequence: 13 }, MAX_FRAME).unwrap();
        serde_json::from_slice::<Frame>(&quota::read_line(&stream, MAX_FRAME).unwrap()).unwrap()
    });
    let original =
        r#"test -z "$IVY_CLAUDE_QUOTA_SOCKET" || exit 99; cat; printf stderr >&2; exit 23"#;
    let input =
        br#"{"rate_limits":{"five_hour":{"used_percentage":0}},"transcript_path":"private"}"#;
    let mut child = helper(&endpoint.path, original).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(23));
    assert_eq!(output.stdout, input);
    assert_eq!(output.stderr, b"stderr");
    assert_eq!(
        broker.join().unwrap(),
        Frame {
            v: 1,
            sequence: 13,
            outcome: Outcome::Observed {
                five_hour: Some(Window {
                    used_percentage: Some(0.0),
                    resets_at: None,
                }),
                seven_day: None,
            },
        }
    );

    let mut child = helper(&endpoint.path, original).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(23));
    assert_eq!(output.stdout, input);
    assert_eq!(output.stderr, b"stderr");
}

struct DelegatedChild(OwnedFd);

impl Drop for DelegatedChild {
    fn drop(&mut self) {
        // SAFETY: the owned pidfd refers to the exact fixture child, including after exit.
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            );
        }
    }
}

#[test]
fn quota_statusline_cancellation_reaches_the_original_shell_child() {
    let (endpoint, _listener) = Endpoint::bind();
    let original = r#"sleep 30 & printf '%s\n' "$!"; wait"#;
    let mut child = helper(&endpoint.path, original).spawn().unwrap();
    drop(child.stdin.take());
    let mut stdout = child.stdout.take().unwrap();
    let mut line = Vec::new();
    let mut read_line = || {
        line.clear();
        loop {
            let mut byte = [0];
            readable(&stdout)?;
            stdout.read_exact(&mut byte)?;
            if byte[0] == b'\n' {
                return String::from_utf8(line.clone()).map_err(io::Error::other);
            }
            line.push(byte[0]);
        }
    };
    let pid: libc::pid_t = read_line().unwrap().parse().unwrap();
    // SAFETY: this reads the live fixture child's identity. The descriptor pins it until cleanup.
    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) as i32 };
    assert!(pidfd >= 0, "{}", io::Error::last_os_error());
    // SAFETY: a successful pidfd_open returns a fresh owned descriptor.
    let delegate = DelegatedChild(unsafe { OwnedFd::from_raw_fd(pidfd) });

    // SAFETY: the launched helper remains an owned child and is not reaped yet.
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    assert_eq!(child.wait().unwrap().signal(), Some(libc::SIGTERM));
    readable(&delegate.0).unwrap();
}
