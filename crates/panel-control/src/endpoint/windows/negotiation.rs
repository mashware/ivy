//! Opt-in transport upgrade. Bootstrap is never authenticated by its numeric client PID.

use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Storage::FileSystem::*;

use super::rendezvous::Rendezvous;
use super::{Handle, PIPE_PATH_PREFIX, Stream, WindowsPeer, event, wide};

pub(super) const DEADLINE: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 256;
const HELLO: &[u8] = b"{\"tool\":\"peer_hello\",\"v\":1}\n";

/// Inspect only the first request; legacy first-frame bytes are replayed without reinterpretation.
pub(super) fn accept(bootstrap: Stream) -> io::Result<Stream> {
    accept_with_capture(bootstrap, WindowsPeer::candidate)
}

fn accept_with_capture(
    mut bootstrap: Stream,
    capture: impl FnOnce(&Stream) -> io::Result<std::sync::Arc<WindowsPeer>>,
) -> io::Result<Stream> {
    let first = first_request(&mut bootstrap)?;
    let line = first.split(|byte| *byte == b'\n').next().unwrap_or(&first);
    let hello = serde_json::from_slice::<Value>(line).ok();
    if hello
        .as_ref()
        .and_then(|value| value.get("tool"))
        .and_then(Value::as_str)
        != Some("peer_hello")
    {
        bootstrap.prefetched.lock().unwrap().extend(first);
        return Ok(bootstrap);
    }
    // A silent or partial legacy request does not acquire a new idle-expiry policy.
    let deadline = Instant::now() + DEADLINE;
    let result = if line.len() + 1 > MAX_FRAME || hello != Some(json!({"tool":"peer_hello","v":1}))
    {
        Err(invalid("invalid peer upgrade hello"))
    } else {
        upgrade_server(&mut bootstrap, deadline, capture)
    };
    match result {
        Ok(Some(verified)) => Ok(verified),
        Ok(None) => {
            reset_timeouts(&bootstrap)?;
            Ok(bootstrap)
        }
        Err(failure) => {
            // A refusal is bounded by the SAME deadline; it cannot extend shutdown or timeout.
            let _ = write_frame(
                &bootstrap,
                &json!({"ok":false,"error":"peer upgrade failed"}),
                deadline,
            );
            Err(failure)
        }
    }
}

fn first_request(stream: &mut Stream) -> io::Result<Vec<u8>> {
    stream.set_recv_timeout(Some(Duration::from_millis(100)))?;
    let mut received = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "bootstrap closed",
                ));
            }
            Ok(count) => {
                received.extend_from_slice(&chunk[..count]);
                if received.contains(&b'\n') || received.len() > crate::wire::MAX_REQUEST {
                    return Ok(received);
                }
            }
            Err(failure) if failure.kind() == io::ErrorKind::WouldBlock => continue,
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            Err(failure) => return Err(failure),
        }
    }
}

fn upgrade_server(
    bootstrap: &mut Stream,
    deadline: Instant,
    capture: impl FnOnce(&Stream) -> io::Result<std::sync::Arc<WindowsPeer>>,
) -> io::Result<Option<Stream>> {
    let candidate = match capture(bootstrap) {
        Ok(candidate) => candidate,
        Err(_) => {
            write_frame(bootstrap, &ready("unverified"), deadline)?;
            return Ok(None);
        }
    };
    // Identity proof ordering: retain H BEFORE creating this never-before-existing instance E.
    let name = fresh_name()?;
    let full_name = wide(std::ffi::OsStr::new(&format!("{PIPE_PATH_PREFIX}{name}")));
    let rendezvous = Rendezvous::create(&full_name, bootstrap.stop.clone())?;
    write_frame(
        bootstrap,
        &json!({"ok":true,"peer":{"v":1,"mode":"reconnect","name":name}}),
        deadline,
    )?;
    let mut stream = rendezvous.accept(deadline)?;
    if let Err(failure) = candidate.verify_fresh_connector(&stream) {
        let _ = write_frame(
            &stream,
            &json!({"ok":false,"error":"fresh peer verification failed"}),
            deadline,
        );
        return Err(failure);
    }
    stream.verified_peer = Some(candidate);
    write_frame(&stream, &ready("verified"), deadline)?;
    reset_timeouts(&stream)?;
    // Neither first-frame tails nor pending bootstrap bytes enter this authenticated stream.
    Ok(Some(stream))
}

pub(crate) fn connect_verified(
    dir: &std::path::Path,
    id: &str,
    expected_pid: u32,
    deadline: Instant,
) -> io::Result<Stream> {
    remaining(deadline)?;
    let bootstrap = super::connect(dir, id)?;
    bootstrap.verify_server_pid(expected_pid)?;
    write_bytes(&bootstrap, HELLO, deadline)?;
    match read_frame(&bootstrap, deadline)? {
        PeerFrame {
            v: 1,
            mode,
            name: None,
        } if mode == "unverified" => {
            reset_timeouts(&bootstrap)?;
            Ok(bootstrap)
        }
        PeerFrame {
            v: 1,
            mode,
            name: Some(name),
        } if mode == "reconnect" && valid_name(&name) => {
            remaining(deadline)?;
            let stream = connect_fresh(&name)?;
            // Verify inherited identity BEFORE reading an acknowledgement or sending requests.
            stream.verify_server_pid(expected_pid)?;
            let reply = read_frame(&stream, deadline)?;
            if reply.v != 1 || reply.mode != "verified" || reply.name.is_some() {
                return Err(invalid("unexpected peer upgrade acknowledgement"));
            }
            reset_timeouts(&stream)?;
            Ok(stream)
        }
        _ => Err(invalid("invalid peer upgrade response")),
    }
}

fn connect_fresh(name: &str) -> io::Result<Stream> {
    // Callers validate a component, never a host, arbitrary path or full UNC name.
    let full = wide(std::ffi::OsStr::new(&format!("{PIPE_PATH_PREFIX}{name}")));
    // SAFETY: the terminated path remains live and the returned handle is owned exactly once.
    let raw = unsafe {
        CreateFileW(
            full.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            std::ptr::null_mut(),
        )
    };
    Ok(Stream::new(
        Handle::new(raw)?,
        std::sync::Arc::new(event()?),
    ))
}

fn valid_name(name: &str) -> bool {
    name.strip_prefix("ivy-peer-").is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn fresh_name() -> io::Result<String> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random)
        .map_err(|_| io::Error::other("peer pipe randomness unavailable"))?;
    Ok(format!(
        "ivy-peer-{}",
        random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    ok: bool,
    peer: PeerFrame,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PeerFrame {
    v: u32,
    mode: String,
    name: Option<String>,
}

fn read_frame(mut stream: &Stream, deadline: Instant) -> io::Result<PeerFrame> {
    let mut received = Vec::new();
    // A byte at a time is bounded to 256 bytes and never consumes the following protocol frame.
    while received.len() < MAX_FRAME {
        stream.set_recv_timeout(Some(remaining(deadline)?))?;
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "peer upgrade closed",
                ));
            }
            Ok(_) => {
                received.push(byte[0]);
                if byte[0] == b'\n' {
                    remaining(deadline)?;
                    let response: Envelope = serde_json::from_slice(&received)
                        .map_err(|_| invalid("invalid peer upgrade frame"))?;
                    if !response.ok {
                        return Err(invalid("peer upgrade refused"));
                    }
                    return Ok(response.peer);
                }
            }
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            Err(failure) => return Err(failure),
        }
    }
    Err(invalid("peer upgrade frame exceeds 256 bytes"))
}

fn ready(mode: &str) -> Value {
    json!({"ok":true,"peer":{"v":1,"mode":mode}})
}

fn write_frame(stream: &Stream, frame: &Value, deadline: Instant) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(frame).map_err(io::Error::other)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_FRAME {
        return Err(invalid("peer upgrade frame exceeds 256 bytes"));
    }
    write_bytes(stream, &bytes, deadline)
}

fn write_bytes(mut stream: &Stream, mut bytes: &[u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        stream.set_send_timeout(Some(remaining(deadline)?))?;
        match stream.write(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "peer upgrade closed",
                ));
            }
            Ok(count) => bytes = &bytes[count..],
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            Err(failure) => return Err(failure),
        }
    }
    remaining(deadline)?;
    Ok(())
}

pub(super) fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|time| !time.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "peer upgrade timed out"))
}

fn reset_timeouts(stream: &Stream) -> io::Result<()> {
    stream.set_recv_timeout(None)?;
    stream.set_send_timeout(None)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests;
