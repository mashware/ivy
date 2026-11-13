//! Bounded CLI input translation and one request/reply exchange over the inherited endpoint.

use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

use interprocess::local_socket::prelude::*;
use serde::Deserialize;

use super::{Client, MAX_HOOK_INPUT, MAX_REPLY, Request};

#[derive(Deserialize)]
struct HookInput {
    session_id: String,
    cwd: String,
    #[serde(alias = "event")]
    hook_event_name: String,
    source: Option<String>,
}

/// Official hook input can contain transcript and prompt fields. Only these four keys survive.
pub fn hook_request(bytes: &[u8]) -> Result<Request, String> {
    if bytes.len() > MAX_HOOK_INPUT {
        return Err("hook input goes over the ceiling".into());
    }
    let input: HookInput =
        serde_json::from_slice(bytes).map_err(|_| "invalid Codex hook metadata")?;
    let source = match (input.source, input.hook_event_name.as_str()) {
        (Some(source), _) => source,
        (None, "SessionEnd") => "exit".to_owned(),
        _ => return Err("Codex hook source is missing".into()),
    };
    let request = Request::Observe {
        client: Client::Codex,
        session_id: input.session_id,
        cwd: input.cwd,
        event: input.hook_event_name,
        source,
    };
    request.validate()?;
    Ok(request)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    sequence: u64,
    session_id: Option<String>,
    cwd: Option<String>,
    challenge: Option<String>,
}

pub fn update_request(generation: String, bytes: &[u8]) -> Result<Request, String> {
    if bytes.len() > super::MAX_REQUEST {
        return Err("session input goes over the ceiling".into());
    }
    let input: UpdateInput =
        serde_json::from_slice(bytes).map_err(|_| "invalid session update metadata")?;
    let request = Request::Update {
        generation,
        sequence: input.sequence,
        session_id: input.session_id,
        cwd: input.cwd,
        challenge: input.challenge,
    };
    request.validate()?;
    Ok(request)
}

/// Validates and compacts a hook panel document before any endpoint connection is attempted.
pub fn publish_document(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() as u64 > panel_contract::MAX_BYTES {
        return Err(format!(
            "publication input goes over {} bytes",
            panel_contract::MAX_BYTES
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "publication input is not valid JSON".to_owned())?;
    if !value.is_object() {
        return Err("publication input must be a JSON object".to_owned());
    }
    let compact = serde_json::to_string(&value).map_err(|_| "cannot encode publication input")?;
    if compact.len() as u64 > panel_contract::MAX_BYTES {
        return Err(format!(
            "publication document goes over {} bytes",
            panel_contract::MAX_BYTES
        ));
    }
    Ok(compact)
}

/// Opens only the endpoint inherited by this pane. No arbitrary endpoint or pane selector exists.
pub fn exchange(request: &Request) -> io::Result<serde_json::Value> {
    exchange_line(&request.line().map_err(io::Error::other)?)
}

/// Exchanges a prebuilt, bounded newline JSON request over this pane's inherited endpoint.
pub fn exchange_line(line: &[u8]) -> io::Result<serde_json::Value> {
    let outside = || {
        io::Error::new(
            io::ErrorKind::NotFound,
            "this helper is not running inside an Ivy pane",
        )
    };
    let [id, dir] = panel_contract::ENV_NAMES.map(std::env::var_os);
    let (id, dir) =
        crate::client::whole_identity(id.as_deref(), dir.as_deref()).ok_or_else(outside)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut stream = interprocess::local_socket::ConnectOptions::new()
        .name(crate::endpoint::name_of(dir, id)?)
        .wait_mode(interprocess::ConnectWaitMode::Timeout(remaining(deadline)?))
        .connect_sync()?;
    let mut unwritten = line;
    while !unwritten.is_empty() {
        stream.set_send_timeout(Some(remaining(deadline)?))?;
        match stream.write(unwritten) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "session endpoint stopped reading",
                ));
            }
            Ok(written) => unwritten = &unwritten[written..],
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            Err(failure) => return Err(failure),
        }
    }
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        stream.set_recv_timeout(Some(remaining(deadline)?))?;
        let read = match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "session endpoint closed without a reply",
                ));
            }
            Ok(read) => read,
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            Err(failure) => return Err(failure),
        };
        let end = chunk[..read].iter().position(|byte| *byte == b'\n');
        let content = end.unwrap_or(read);
        if bytes.len() + content > MAX_REPLY {
            return Err(io::Error::other("session reply goes over the ceiling"));
        }
        bytes.extend_from_slice(&chunk[..content]);
        if end.is_some() {
            break;
        }
    }
    let reply: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| io::Error::other("invalid session reply"))?;
    if !reply.is_object() || !reply["ok"].is_boolean() {
        return Err(io::Error::other("invalid session reply"));
    }
    Ok(reply)
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|duration| !duration.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "session exchange timed out"))
}

#[cfg(test)]
mod publish_tests {
    use super::*;

    #[test]
    fn publish_input_requires_object_and_compacts_with_byte_ceilings() {
        assert_eq!(
            publish_document(b"{\n \"lines\": [\"a\"]\n}").unwrap(),
            r#"{"lines":["a"]}"#
        );
        assert_eq!(
            publish_document(b"[]").unwrap_err(),
            "publication input must be a JSON object"
        );
        assert_eq!(
            publish_document(b"{broken").unwrap_err(),
            "publication input is not valid JSON"
        );
        assert!(publish_document(&vec![b' '; panel_contract::MAX_BYTES as usize + 1]).is_err());
    }

    #[test]
    fn publish_silence_gate_requires_a_whole_identity_and_supported_endpoint() {
        use crate::client::whole_identity;
        use std::ffi::OsStr;
        assert!(whole_identity(None, None).is_none());
        assert!(whole_identity(Some(OsStr::new("pane")), None).is_none());
        assert!(whole_identity(Some(OsStr::new("pane")), Some(OsStr::new("relative"))).is_none());
        assert!(whole_identity(Some(OsStr::new("pane")), Some(OsStr::new("/tmp/panel"))).is_some());
    }
}
