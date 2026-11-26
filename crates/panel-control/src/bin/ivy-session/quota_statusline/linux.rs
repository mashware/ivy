use panel_control::quota::{self, Admission, ENDPOINT_ENV, Frame, MAX_FRAME, Outcome, Window};
use serde::Deserialize;
use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

#[path = "cancellation.rs"]
mod cancellation;

const MAX_STATUSLINE_INPUT: usize = 64 * 1024;
const PUBLICATION_BUDGET: Duration = Duration::from_millis(40);

pub(super) fn run(command: Option<&OsStr>) -> i32 {
    let endpoint = std::env::var_os(ENDPOINT_ENV)
        .map(PathBuf::from)
        .filter(|path| private_socket(path));
    let Some(endpoint) = endpoint else {
        return command.map_or(0, exec_original);
    };
    let mut child = match command.map(spawn_original).transpose() {
        Ok(child) => child,
        Err(_) => return command.map_or(0, exec_original),
    };

    let mut forwarded = child.as_mut().and_then(|child| child.stdin.take());
    let copied = copy_for_statusline(std::io::stdin().lock(), &mut forwarded);
    drop(forwarded);
    let parsed = copied
        .map(|copied| {
            copied
                .filter(|bytes| !bytes.is_empty())
                .map(|bytes| observation(&bytes))
        })
        .unwrap_or(Some(Outcome::ProducerError));

    let publication = parsed.and_then(|outcome| try_publish(endpoint, outcome));
    let status = child.as_mut().map(cancellation::wait);
    if let Some(publication) = publication {
        // Output and child completion never wait on the broker. Briefly keep the helper
        // alive so the broker can verify the connecting process's lifetime.
        let _ = publication.recv_timeout(PUBLICATION_BUDGET);
    }
    match status {
        None => 0,
        Some(Ok(status)) => original_exit(status),
        Some(Err(_)) => 127,
    }
}

fn original(command: &OsStr) -> Command {
    let mut shell = Command::new("/bin/sh");
    shell.arg("-c").arg(command).env_remove(ENDPOINT_ENV);
    shell
}

fn exec_original(command: &OsStr) -> i32 {
    let _ = original(command).exec();
    127
}

fn spawn_original(command: &OsStr) -> io::Result<std::process::Child> {
    let mut child = original(command);
    child.stdin(Stdio::piped());
    cancellation::spawn(child)
}

fn original_exit(status: ExitStatus) -> i32 {
    if let Some(signal) = status.signal() {
        // Mirror the original command's termination when Claude checks the signal.
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
            libc::raise(signal);
        }
        return 128 + signal;
    }
    status.code().unwrap_or(127)
}

fn copy_for_statusline(
    mut input: impl Read,
    forwarded: &mut Option<impl Write>,
) -> io::Result<Option<Vec<u8>>> {
    let mut collected = Some(Vec::new());
    let mut chunk = [0; 8192];
    loop {
        let count = match input.read(&mut chunk) {
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(collected);
        }
        let bytes = &chunk[..count];
        if forwarded
            .as_mut()
            .is_some_and(|writer| writer.write_all(bytes).is_err())
        {
            // A script can exit without consuming all input. Keep its exit status.
            forwarded.take();
        }
        if let Some(sample) = &mut collected {
            if sample.len() + count <= MAX_STATUSLINE_INPUT {
                sample.extend_from_slice(bytes);
            } else {
                collected = None;
            }
        }
    }
}

#[derive(Deserialize)]
struct Statusline {
    rate_limits: Option<RateLimits>,
}

#[derive(Deserialize)]
struct RateLimits {
    five_hour: Option<ReportedWindow>,
    seven_day: Option<ReportedWindow>,
}

#[derive(Deserialize)]
struct ReportedWindow {
    used_percentage: Option<f64>,
    resets_at: Option<i64>,
}

fn observation(bytes: &[u8]) -> Outcome {
    let reported: Statusline = match serde_json::from_slice(bytes) {
        Ok(reported) => reported,
        Err(_) => return Outcome::ProducerError,
    };
    let window = |reported: ReportedWindow| Window {
        used_percentage: reported.used_percentage,
        resets_at: reported.resets_at,
    };
    let outcome = match reported.rate_limits {
        Some(limits) => Outcome::Observed {
            five_hour: limits.five_hour.map(window),
            seven_day: limits.seven_day.map(window),
        },
        None => Outcome::Observed {
            five_hour: None,
            seven_day: None,
        },
    };
    let sample = Frame {
        v: 1,
        sequence: 1,
        outcome,
    };
    if sample.validate() {
        outcome
    } else {
        Outcome::ProducerError
    }
}

fn private_socket(path: &Path) -> bool {
    path.is_absolute()
        && path
            .parent()
            .is_some_and(|path| panel_control::endpoint::private_to_this_user(path).is_ok())
        && path.symlink_metadata().is_ok_and(|meta| {
            meta.file_type().is_socket() && meta.uid() == unsafe { libc::geteuid() }
        })
}

fn try_publish(endpoint: PathBuf, outcome: Outcome) -> Option<Receiver<()>> {
    let (done, publication) = channel();
    std::thread::Builder::new()
        .name("quota-publication".into())
        .spawn(move || {
            let _ = publish(&endpoint, outcome);
            let _ = done.send(());
        })
        .ok()?;
    Some(publication)
}

fn publish(endpoint: &Path, outcome: Outcome) -> io::Result<()> {
    let stream = UnixStream::connect(endpoint)?;
    let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of_val(&credentials) as libc::socklen_t;
    // SAFETY: the stream stays open and getsockopt writes at most the structure size.
    let checked = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::addr_of_mut!(credentials).cast(),
            &mut length,
        )
    };
    if checked != 0
        || length as usize != std::mem::size_of_val(&credentials)
        || credentials.pid <= 0
        || credentials.uid != unsafe { libc::geteuid() }
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    stream.set_read_timeout(Some(PUBLICATION_BUDGET))?;
    stream.set_write_timeout(Some(PUBLICATION_BUDGET))?;
    let admission: Admission = serde_json::from_slice(&quota::read_line(&stream, MAX_FRAME)?)?;
    let frame = Frame {
        v: admission.v,
        sequence: admission.sequence,
        outcome,
    };
    if !frame.validate() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    quota::write_line(&stream, &frame, MAX_FRAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_capture_forwards_every_original_byte() {
        let input = vec![b'x'; MAX_STATUSLINE_INPUT + 1];
        let mut output = Some(Vec::new());
        assert!(
            copy_for_statusline(&input[..], &mut output)
                .unwrap()
                .is_none()
        );
        assert_eq!(output.unwrap(), input);
    }

    #[test]
    fn telemetry_selects_only_quota_fields_without_collapsing_zero() {
        let input = br#"{"transcript_path":"discarded","rate_limits":{"five_hour":{"used_percentage":0,"future_field":"discarded"},"spend_limit":{"anything":"discarded"}}}"#;
        assert_eq!(
            observation(input),
            Outcome::Observed {
                five_hour: Some(Window {
                    used_percentage: Some(0.0),
                    resets_at: None,
                }),
                seven_day: None,
            }
        );
        assert_eq!(observation(b"invalid"), Outcome::ProducerError);
    }
}
