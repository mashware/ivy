//! Versioned local messages for one Claude statusline invocation.

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

pub const MAX_FRAME: usize = 1024;
pub const ENDPOINT_ENV: &str = "IVY_CLAUDE_QUOTA_SOCKET";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_percentage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
}

impl Window {
    fn valid(&self) -> bool {
        self.used_percentage
            .is_none_or(|value| (0.0..=100.0).contains(&value))
            && self
                .resets_at
                .is_none_or(|value| (0..=253_402_300_799).contains(&value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub v: u8,
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "IncomingFrame")]
pub struct Frame {
    pub v: u8,
    pub sequence: u64,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Observed {
        five_hour: Option<Window>,
        seven_day: Option<Window>,
    },
    ProducerError,
}

#[derive(Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
enum IncomingFrame {
    Observed {
        v: u8,
        sequence: u64,
        five_hour: Option<Window>,
        seven_day: Option<Window>,
    },
    ProducerError {
        v: u8,
        sequence: u64,
    },
}

impl TryFrom<IncomingFrame> for Frame {
    type Error = &'static str;

    fn try_from(frame: IncomingFrame) -> Result<Self, Self::Error> {
        let frame = match frame {
            IncomingFrame::Observed {
                v,
                sequence,
                five_hour,
                seven_day,
            } => Self {
                v,
                sequence,
                outcome: Outcome::Observed {
                    five_hour,
                    seven_day,
                },
            },
            IncomingFrame::ProducerError { v, sequence } => Self {
                v,
                sequence,
                outcome: Outcome::ProducerError,
            },
        };
        frame
            .validate()
            .then_some(frame)
            .ok_or("invalid quota frame")
    }
}

impl Frame {
    pub fn validate(&self) -> bool {
        self.v == 1
            && self.sequence > 0
            && match self.outcome {
                Outcome::Observed {
                    five_hour,
                    seven_day,
                } => [five_hour, seven_day]
                    .into_iter()
                    .flatten()
                    .all(|w| w.valid()),
                Outcome::ProducerError => true,
            }
    }
}

/// Reads one complete newline-delimited frame. The size limit excludes the delimiter.
pub fn read_line(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    use std::io::{BufRead, BufReader};

    let mut bytes = Vec::new();
    BufReader::new(reader.take(limit as u64 + 1)).read_until(b'\n', &mut bytes)?;
    if bytes.pop() != Some(b'\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "quota message exceeds its ceiling or has no delimiter",
        ));
    }
    Ok(bytes)
}

pub fn write_line<T: Serialize>(mut writer: impl Write, value: &T, limit: usize) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "quota message exceeds its ceiling",
        ));
    }
    bytes.push(b'\n');
    writer.write_all(&bytes)
}

#[cfg(target_os = "linux")]
pub fn capture_peer(stream: &std::os::unix::net::UnixStream) -> io::Result<crate::session::Peer> {
    let stream = crate::endpoint::Stream::UdSocket(stream.try_clone()?.into());
    crate::session::Peer::capture(&stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_preserves_zero_and_independent_absence() {
        let raw = br#"{"v":1,"sequence":42,"outcome":"observed","five_hour":{"used_percentage":0.0},"seven_day":null}"#;
        let frame: Frame = serde_json::from_slice(raw).unwrap();
        assert_eq!(
            frame.outcome,
            Outcome::Observed {
                five_hour: Some(Window {
                    used_percentage: Some(0.0),
                    resets_at: None,
                }),
                seven_day: None,
            }
        );
        assert_eq!(serde_json::to_vec(&frame).unwrap(), raw);
    }

    #[test]
    fn wire_rejects_unknown_fields_wrong_version_and_invalid_ranges() {
        for input in [
            r#"{"v":1,"sequence":1,"outcome":"producer_error","source":"fake"}"#,
            r#"{"v":1,"sequence":1,"outcome":"observed","five_hour":{"x":0}}"#,
            r#"{"v":2,"sequence":1,"outcome":"producer_error"}"#,
            r#"{"v":1,"sequence":0,"outcome":"producer_error"}"#,
            r#"{"v":1,"sequence":1,"outcome":"observed","five_hour":{"used_percentage":101}}"#,
            r#"{"v":1,"sequence":1,"outcome":"observed","five_hour":{"resets_at":"0"}}"#,
        ] {
            assert!(serde_json::from_str::<Frame>(input).is_err(), "{input}");
        }
    }

    #[test]
    fn incomplete_and_oversized_messages_cannot_be_decoded() {
        assert_eq!(read_line(&b"abc\n"[..], 3).unwrap(), b"abc");
        assert!(read_line(&b"abc"[..], 3).is_err());
        assert!(read_line(&b"abcd\n"[..], 3).is_err());
    }
}
