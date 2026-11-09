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

/// Opens only the endpoint inherited by this pane. No arbitrary endpoint or pane selector exists.
pub fn exchange(request: &Request) -> io::Result<serde_json::Value> {
    let outside = || {
        io::Error::new(
            io::ErrorKind::NotFound,
            "this helper is not running inside an Ivy pane",
        )
    };
    let dir = std::env::var_os(panel_contract::ENV_NAMES[1])
        .filter(|v| !v.is_empty())
        .ok_or_else(outside)?;
    let id = std::env::var_os(panel_contract::ENV_NAMES[0])
        .filter(|v| !v.is_empty())
        .ok_or_else(outside)?;
    let line = request.line().map_err(io::Error::other)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut stream = interprocess::local_socket::ConnectOptions::new()
        .name(crate::endpoint::name_of(
            std::path::Path::new(&dir),
            &id.to_string_lossy(),
        )?)
        .wait_mode(interprocess::ConnectWaitMode::Timeout(remaining(deadline)?))
        .connect_sync()?;
    let mut unwritten = line.as_slice();
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
