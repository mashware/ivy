//! Metadata-only AI restoration requests. The endpoint names the pane; the kernel names the peer.

mod helper;
mod peer;

pub use helper::{exchange, hook_request, update_request};
pub use peer::Peer;

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Bounds the complete encoded request, including metadata escaping.
pub const MAX_REQUEST: usize = 16 * 1024;
/// Bounds official hook input before discarded fields can allocate without limit.
pub const MAX_HOOK_INPUT: usize = 64 * 1024;
/// Bounds replies both at the server and helper.
pub const MAX_REPLY: usize = 16 * 1024;

/// Called on a connection thread, never through the UI inbox. Implementations must bound their work.
pub type SessionHandler = Arc<dyn Fn(Peer, Request) -> serde_json::Value + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Client {
    Codex,
    Claude,
    Opencode,
}

impl Client {
    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "codex" => Ok(Self::Codex),
            "claude" => Ok(Self::Claude),
            "opencode" => Ok(Self::Opencode),
            _ => Err("unsupported session client".into()),
        }
    }
}

/// A fixed client and exact producer-declared conversation ID; never an executable or argv.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UncheckedIdentity", deny_unknown_fields)]
pub struct Identity {
    pub client: Client,
    pub session_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UncheckedIdentity {
    client: Client,
    session_id: String,
}

impl TryFrom<UncheckedIdentity> for Identity {
    type Error = String;

    fn try_from(raw: UncheckedIdentity) -> Result<Self, Self::Error> {
        Self::new(raw.client, raw.session_id)
    }
}

impl Identity {
    pub fn new(client: Client, session_id: impl Into<String>) -> Result<Self, String> {
        let session_id = session_id.into();
        validate_id(client, &session_id)?;
        Ok(Self { client, session_id })
    }
}

/// Only these four metadata operations exist. Unknown fields are rejected, including payload PIDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tool", deny_unknown_fields)]
pub enum Request {
    #[serde(rename = "session_observe")]
    Observe {
        client: Client,
        session_id: String,
        cwd: String,
        event: String,
        source: String,
    },
    #[serde(rename = "session_attach")]
    Attach { client: Client },
    #[serde(rename = "session_probe")]
    Probe { generation: String },
    #[serde(rename = "session_update")]
    Update {
        generation: String,
        sequence: u64,
        session_id: Option<String>,
        cwd: Option<String>,
        challenge: Option<String>,
    },
}

impl Request {
    /// Validates wire shape and every bounded field before invoking the application service.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_REQUEST {
            return Err("session request goes over the ceiling".into());
        }
        let request: Self = serde_json::from_slice(bytes)
            .map_err(|_| "invalid session request fields".to_owned())?;
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Observe {
                client,
                session_id,
                cwd,
                event,
                source,
            } => {
                if *client != Client::Codex {
                    return Err("only Codex uses session observations".into());
                }
                validate_id(*client, session_id)?;
                validate_cwd(cwd)?;
                if !matches!(event.as_str(), "SessionStart" | "SessionEnd")
                    || !matches!(
                        source.as_str(),
                        "startup" | "resume" | "clear" | "compact" | "fork" | "new" | "exit"
                    )
                {
                    return Err("unsupported Codex lifecycle metadata".into());
                }
            }
            Self::Attach { client } => {
                if *client == Client::Codex {
                    return Err("Codex uses session observations".into());
                }
            }
            Self::Probe { generation } => validate_token(generation)?,
            Self::Update {
                generation,
                sequence,
                session_id,
                cwd,
                challenge,
            } => {
                validate_token(generation)?;
                if *sequence == 0 {
                    return Err("session sequence must be positive".into());
                }
                if let Some(id) = session_id {
                    // The generation decides Claude/OpenCode. Accept their union here and the
                    // service validates against its attached client before committing.
                    if validate_id(Client::Claude, id).is_err() {
                        validate_id(Client::Opencode, id)?;
                    }
                }
                if let Some(cwd) = cwd {
                    validate_cwd(cwd)?;
                }
                if let Some(challenge) = challenge {
                    validate_token(challenge)?;
                }
            }
        }
        Ok(())
    }

    pub fn line(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut line = serde_json::to_vec(self).map_err(|_| "cannot encode session request")?;
        if line.len() > MAX_REQUEST {
            return Err("session request goes over the ceiling".into());
        }
        line.push(b'\n');
        Ok(line)
    }
}

pub fn validate_id(client: Client, id: &str) -> Result<(), String> {
    let valid = id.len() <= 128
        && match client {
            Client::Codex | Client::Claude => {
                id.len() == 36
                    && id.bytes().enumerate().all(|(i, c)| {
                        if matches!(i, 8 | 13 | 18 | 23) {
                            c == b'-'
                        } else {
                            c.is_ascii_hexdigit()
                        }
                    })
            }
            Client::Opencode => id.strip_prefix("ses_").is_some_and(|tail| {
                !tail.is_empty() && tail.bytes().all(|c| c.is_ascii_alphanumeric())
            }),
        };
    if valid {
        Ok(())
    } else {
        Err("invalid exact session ID".into())
    }
}

pub fn validate_cwd(cwd: &str) -> Result<(), String> {
    if cwd.len() <= 4096 && Path::new(cwd).is_absolute() && !cwd.chars().any(char::is_control) {
        Ok(())
    } else {
        Err("session directory must be a bounded absolute local path".into())
    }
}

fn validate_token(token: &str) -> Result<(), String> {
    if !token.is_empty()
        && token.len() <= 128
        && token
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        Ok(())
    } else {
        Err("invalid session generation or challenge".into())
    }
}

pub(crate) fn reply_line(reply: serde_json::Value) -> Vec<u8> {
    let mut line = serde_json::to_vec(&reply).unwrap_or_default();
    if !reply.is_object() || !reply["ok"].is_boolean() || line.len() > MAX_REPLY {
        line = br#"{"ok":false,"error":"invalid session service reply"}"#.to_vec();
    }
    line.push(b'\n');
    line
}

#[cfg(test)]
mod tests;
