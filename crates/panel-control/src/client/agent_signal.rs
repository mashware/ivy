//! The metadata-only exchange has its own reply and one deadline (D80).

use super::Producer;
use crate::wire::AgentSnapshot;
#[cfg(unix)]
use interprocess::local_socket::traits::Stream as _;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

pub const MAX_FRAME: usize = 64 * 1024;
const EXCHANGE_BUDGET: Duration = Duration::from_secs(6);

/// A signal refusal retains the listener's explicit retryability, unlike legacy publications.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum SignalAnswer {
    Accepted {
        ok: bool,
    },
    Refused {
        ok: bool,
        error: String,
        retryable: bool,
    },
}

impl Producer {
    /// Sends only OpenCode metadata, with one six-second write/flush/reply budget.
    pub fn agent_signal(&mut self, snapshot: Option<&AgentSnapshot>) -> io::Result<SignalAnswer> {
        if snapshot.is_some_and(|s| {
            s.session.is_empty() || s.session.len() > 256 || s.session.chars().any(char::is_control)
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid agent session",
            ));
        }
        let mut line = serde_json::to_vec(&serde_json::json!({
            "tool":"agent_signal", "v":1, "client":"opencode", "snapshot":snapshot
        }))?;
        line.push(b'\n');
        exchange(&mut self.stream, &line, Instant::now() + EXCHANGE_BUDGET)
    }
}

trait TimedIo: Read + Write {
    fn receive_timeout(&self, remaining: Duration) -> io::Result<()>;
    fn send_timeout(&self, remaining: Duration) -> io::Result<()>;
}

impl TimedIo for crate::endpoint::Stream {
    fn receive_timeout(&self, remaining: Duration) -> io::Result<()> {
        self.set_recv_timeout(Some(remaining))
    }
    fn send_timeout(&self, remaining: Duration) -> io::Result<()> {
        self.set_send_timeout(Some(remaining))
    }
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "agent signal exchange timed out"))
}

fn exchange(
    stream: &mut impl TimedIo,
    mut line: &[u8],
    deadline: Instant,
) -> io::Result<SignalAnswer> {
    while !line.is_empty() {
        stream.send_timeout(remaining(deadline)?)?;
        let sent = match stream.write(line) {
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        remaining(deadline)?;
        if sent == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        line = &line[sent..];
    }
    loop {
        stream.send_timeout(remaining(deadline)?)?;
        match stream.flush() {
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        }
        remaining(deadline)?;
        break;
    }
    let mut reply = Vec::new();
    loop {
        stream.receive_timeout(remaining(deadline)?)?;
        let mut byte = [0];
        let count = match stream.read(&mut byte) {
            Err(failure) if failure.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        remaining(deadline)?;
        if count == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if byte[0] == b'\n' {
            break;
        }
        if reply.len() == MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "signal answer goes over 64 KiB",
            ));
        }
        reply.push(byte[0]);
    }
    decode(&reply)
}

fn decode(reply: &[u8]) -> io::Result<SignalAnswer> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Accepted {
        ok: bool,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Refused {
        ok: bool,
        error: String,
        #[serde(default)]
        retryable: bool,
    }
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Reply {
        Accepted(Accepted),
        Refused(Refused),
    }
    let reply: Reply = serde_json::from_slice(reply)
        .map_err(|failure| io::Error::new(io::ErrorKind::InvalidData, failure))?;
    match reply {
        Reply::Accepted(Accepted { ok: true }) => Ok(SignalAnswer::Accepted { ok: true }),
        Reply::Refused(Refused {
            ok: false,
            error,
            retryable,
        }) => Ok(SignalAnswer::Refused {
            ok: false,
            error,
            retryable,
        }),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid signal answer",
        )),
    }
}

#[cfg(test)]
mod tests;
