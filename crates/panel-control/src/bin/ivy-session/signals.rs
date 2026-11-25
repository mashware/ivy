//! Fixed, bounded OpenCode stream; endpoint authentication remains Producer's job.

use panel_control::client::{Producer, SignalAnswer};
use panel_control::wire::AgentSnapshot;
use std::io::{self, BufRead, Write};

const MAX_FRAME: usize = 64 * 1024;

pub(super) fn run(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    #[cfg(windows)]
    let connection =
        panel_control::client::server_pid(std::env::var_os("PANEL_SERVER_PID").as_deref())
            .and_then(|_| Producer::of_this_pane());
    #[cfg(not(windows))]
    let connection = Producer::of_this_pane();
    let mut producer = match connection {
        Ok(producer) => producer,
        Err(failure) => {
            let retryable = matches!(
                failure.kind(),
                io::ErrorKind::NotConnected
                    | io::ErrorKind::ConnectionRefused
                    | io::ErrorKind::TimedOut
                    | io::ErrorKind::WouldBlock
                    | io::ErrorKind::Interrupted
            );
            emit(
                &mut output,
                &serde_json::json!({"ok":false,"ready":true,"v":1,
                "error":failure.to_string(),"retryable":retryable}),
            )?;
            return Err(failure);
        }
    };
    emit(
        &mut output,
        &serde_json::json!({"ok":true,"ready":true,"v":1}),
    )?;
    while let Some(line) = frame(&mut input)? {
        let snapshot = parse(&line)?;
        let answer: SignalAnswer = producer.agent_signal(snapshot.as_ref())?;
        emit(&mut output, &answer)?;
    }
    Ok(())
}

fn emit(output: &mut impl Write, value: &impl serde::Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signal output goes over 64 KiB",
        ));
    }
    output.write_all(&bytes)?;
    output.write_all(b"\n")?;
    output.flush()
}

fn frame(input: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "unterminated signal frame",
                ))
            };
        }
        let end = chunk.iter().position(|b| *b == b'\n');
        let take = end.unwrap_or(chunk.len());
        if take > MAX_FRAME - line.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "signal input goes over 64 KiB",
            ));
        }
        line.extend_from_slice(&chunk[..take]);
        input.consume(take + usize::from(end.is_some()));
        if end.is_some() {
            return Ok(Some(line));
        }
    }
}

fn parse(line: &[u8]) -> io::Result<Option<AgentSnapshot>> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Envelope {
        tool: String,
        v: u8,
        client: String,
        // RawValue preserves the distinction between a missing field and explicit null.
        snapshot: Box<serde_json::value::RawValue>,
    }
    let invalid = |detail| io::Error::new(io::ErrorKind::InvalidData, detail);
    let envelope: Envelope = serde_json::from_slice(line).map_err(|e| invalid(e.to_string()))?;
    if envelope.tool != "agent_signal" || envelope.v != 1 || envelope.client != "opencode" {
        return Err(invalid("unsupported agent signal".into()));
    }
    let snapshot: Option<AgentSnapshot> =
        serde_json::from_str(envelope.snapshot.get()).map_err(|e| invalid(e.to_string()))?;
    if snapshot.as_ref().is_some_and(|s| {
        s.session.is_empty() || s.session.len() > 256 || s.session.chars().any(char::is_control)
    }) {
        return Err(invalid("invalid agent session".into()));
    }
    Ok(snapshot)
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod tests;
