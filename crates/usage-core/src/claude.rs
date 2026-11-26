//! Bounded Claude Code transcript projection.
//!
//! Extracts and groups assistant usage with the format rules from
//! tokscale-core's src/sessions/claudecode.rs at d7a147512c3e404fa45f7e6783932fe42759e01c.
//! https://github.com/Nanako0129/tokscale-core
//!
//! MIT License
//!
//! Copyright (c) 2025 Junho Yeo
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use crate::accounting::{Counts, Observation};
use crate::pricing::Catalog;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::Sha256;

const MAX_RECORD_BYTES: usize = 1024 * 1024;
const MAX_ID_BYTES: usize = 128;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // Avoid a heap box for each retained observation.
pub(crate) enum Record {
    Ignored,
    Unresolved,
    Observation(Observation),
}

#[derive(Deserialize)]
struct Entry<'a> {
    #[serde(rename = "type", borrow)]
    kind: Option<&'a RawValue>,
    #[serde(borrow)]
    timestamp: Option<&'a RawValue>,
    #[serde(rename = "requestId", borrow)]
    request_id: Option<&'a RawValue>,
    #[serde(borrow)]
    message: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Message<'a> {
    #[serde(borrow)]
    id: Option<&'a RawValue>,
    #[serde(borrow)]
    model: Option<&'a RawValue>,
    #[serde(borrow)]
    usage: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Usage<'a> {
    #[serde(borrow)]
    input_tokens: Option<&'a RawValue>,
    #[serde(borrow)]
    cache_read_input_tokens: Option<&'a RawValue>,
    #[serde(borrow)]
    cache_creation_input_tokens: Option<&'a RawValue>,
    #[serde(borrow)]
    cache_creation: Option<&'a RawValue>,
    #[serde(borrow)]
    output_tokens: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct DurationSplit<'a> {
    #[serde(borrow)]
    ephemeral_5m_input_tokens: Option<&'a RawValue>,
    #[serde(borrow)]
    ephemeral_1h_input_tokens: Option<&'a RawValue>,
}

impl Usage<'_> {
    fn counts(self) -> Counts {
        let mut flags = 0;
        let input = counter(self.input_tokens, Counts::INPUT_INVALID, &mut flags);
        let cache_read = counter(
            self.cache_read_input_tokens,
            Counts::READ_INVALID,
            &mut flags,
        );
        let cache_write = counter(
            self.cache_creation_input_tokens,
            Counts::WRITE_INVALID,
            &mut flags,
        );
        // A bare whole-prompt count is not the uncached remainder.
        let uncached_input = if cache_read.is_some() || cache_write.is_some() {
            input
        } else {
            None
        };
        let cache_write_1h = hour_subset(self.cache_creation, cache_write, &mut flags);
        let output = counter(self.output_tokens, Counts::OUTPUT_INVALID, &mut flags);
        Counts {
            uncached_input,
            cache_read,
            cache_write,
            cache_write_1h,
            output,
            flags,
        }
    }
}

fn hour_subset(duration: Option<&RawValue>, total: Option<i64>, flags: &mut i64) -> Option<i64> {
    let Some(duration) = duration else {
        return (total == Some(0)).then_some(0);
    };
    let Some(split) = object::<DurationSplit<'_>>(duration) else {
        *flags |= Counts::DURATION_INVALID;
        return None;
    };
    let five = counter(
        split.ephemeral_5m_input_tokens,
        Counts::DURATION_INVALID,
        flags,
    );
    let hour = counter(
        split.ephemeral_1h_input_tokens,
        Counts::DURATION_INVALID,
        flags,
    );
    if *flags & Counts::DURATION_INVALID != 0 {
        return None;
    }
    let total = total?;
    match (five, hour) {
        (Some(five), Some(hour)) if five.checked_add(hour) == Some(total) => Some(hour),
        (None, Some(hour)) if hour <= total => Some(hour),
        (Some(five), None) if five <= total => Some(total - five),
        (None, None) => (total == 0).then_some(0),
        _ => {
            *flags |= Counts::DURATION_INVALID;
            None
        }
    }
}

fn object<'a, T: Deserialize<'a>>(raw: &'a RawValue) -> Option<T> {
    let json = raw.get();
    if !json.starts_with('{') {
        return None;
    }
    serde_json::from_str(json).ok()
}

fn counter(raw: Option<&RawValue>, invalid: i64, flags: &mut i64) -> Option<i64> {
    let raw = raw?;
    if let Ok(value) = serde_json::from_str::<i64>(raw.get())
        && value >= 0
    {
        return Some(value);
    }
    *flags |= invalid;
    None
}

fn bounded_string(raw: Option<&RawValue>, max_bytes: usize) -> Option<String> {
    let raw = raw?.get();
    if raw.len() > max_bytes * 6 + 2 {
        return None;
    }
    let value: String = serde_json::from_str(raw).ok()?;
    (value.len() <= max_bytes).then_some(value)
}

fn identifier(raw: Option<&RawValue>) -> Option<String> {
    let value = bounded_string(raw, MAX_ID_BYTES)?;
    (!value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
    .then_some(value)
}

type UsageMac = Hmac<Sha256>;

fn domain_mac(salt: &[u8; 32], domain: &[u8]) -> UsageMac {
    let mut mac = UsageMac::new_from_slice(salt).expect("fixed-size usage identity key");
    mac.update(domain);
    mac
}

fn add_field(mac: &mut UsageMac, field: &[u8]) {
    mac.update(&(field.len() as u64).to_be_bytes());
    mac.update(field);
}

fn add_optional_counter(mac: &mut UsageMac, value: Option<i64>) {
    match value {
        Some(value) => add_field(mac, &value.to_be_bytes()),
        None => add_field(mac, &[]),
    }
}

fn event_key(salt: &[u8; 32], message: &str, request: &str) -> [u8; 32] {
    let mut mac = domain_mac(salt, b"claude usage event v1");
    add_field(&mut mac, message.as_bytes());
    add_field(&mut mac, request.as_bytes());
    mac.finalize().into_bytes().into()
}

fn model_key(salt: &[u8; 32], model: &[u8]) -> [u8; 32] {
    let mut mac = domain_mac(salt, b"claude usage model v1");
    add_field(&mut mac, model);
    mac.finalize().into_bytes().into()
}

fn evidence_key(
    salt: &[u8; 32],
    at_ms: Option<i64>,
    model_key: Option<[u8; 32]>,
    counts: Counts,
) -> [u8; 32] {
    let mut mac = domain_mac(salt, b"claude usage evidence v1");
    add_optional_counter(&mut mac, at_ms);
    add_field(
        &mut mac,
        model_key.as_ref().map_or(&[], |key| key.as_slice()),
    );
    for count in counts.ordered() {
        add_optional_counter(&mut mac, count);
    }
    add_field(&mut mac, &counts.flags.to_be_bytes());
    mac.finalize().into_bytes().into()
}

pub(crate) fn parse(line: &[u8], source_key: [u8; 32], salt: &[u8; 32]) -> Record {
    if line.len() > MAX_RECORD_BYTES {
        return Record::Unresolved;
    }
    match line.iter().find(|byte| !byte.is_ascii_whitespace()) {
        None => return Record::Ignored,
        Some(b'{') => {}
        _ => return Record::Unresolved,
    }
    let Ok(entry) = serde_json::from_slice::<Entry<'_>>(line) else {
        return Record::Unresolved;
    };
    if bounded_string(entry.kind, 32).as_deref() != Some("assistant") {
        return Record::Ignored;
    }
    let Some(message) = entry.message else {
        return Record::Unresolved;
    };
    let Some(message) = object::<Message<'_>>(message) else {
        return Record::Unresolved;
    };
    let model = bounded_string(message.model, MAX_ID_BYTES);
    if model.as_deref() == Some("<synthetic>") {
        return Record::Ignored;
    }
    let Some(usage) = message.usage else {
        return Record::Ignored;
    };
    let Some(usage) = object::<Usage<'_>>(usage) else {
        return Record::Unresolved;
    };
    let (Some(message_id), Some(request_id)) =
        (identifier(message.id), identifier(entry.request_id))
    else {
        return Record::Unresolved;
    };
    let event_key = event_key(salt, &message_id, &request_id);
    let model_key = if let Some(model) = &model {
        Some(model_key(salt, model.as_bytes()))
    } else {
        message
            .model
            .filter(|raw| raw.get().starts_with('"'))
            .map(|raw| model_key(salt, raw.get().as_bytes()))
    };
    let model = model.filter(|model| Catalog::bundled().known_model(model));
    let at_ms = bounded_string(entry.timestamp, 80)
        .and_then(|date| chrono::DateTime::parse_from_rfc3339(&date).ok())
        .map(|date| date.timestamp_millis());
    let counts = usage.counts();
    Record::Observation(Observation {
        event_key,
        source_key,
        evidence_key: evidence_key(salt, at_ms, model_key, counts),
        at_ms,
        model,
        model_key,
        counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounting::{Reducer, Resolution};

    fn line(model: &str, timestamp: &str, usage: &str) -> String {
        format!(
            r#"{{"type":"assistant","requestId":"req_fixture","timestamp":"{timestamp}","message":{{"id":"msg_fixture","model":"{model}","usage":{{{usage}}}}}}}"#
        )
    }

    fn observation(json: &str, source: [u8; 32]) -> Observation {
        match parse(json.as_bytes(), source, &[1; 32]) {
            Record::Observation(observation) => observation,
            record => panic!("Expected usage observation, got {record:?}"),
        }
    }

    #[test]
    fn whole_prompt_then_cache_split_reconciles_without_double_counting() {
        let make = |time, usage| line("claude-sonnet-4-6", time, usage);
        let bare = observation(
            &make(
                "2026-11-26T23:59:59Z",
                r#""input_tokens":42494,"output_tokens":0"#,
            ),
            [2; 32],
        );
        let split = observation(
            &make(
                "2026-11-27T00:00:00Z",
                r#""input_tokens":265,"cache_read_input_tokens":42000,"cache_creation_input_tokens":0,"output_tokens":120"#,
            ),
            [3; 32],
        );
        assert_eq!(bare.event_key, split.event_key);
        assert_ne!(bare.evidence_key, split.evidence_key);
        assert_eq!(bare.counts.uncached_input, None);
        assert_eq!(bare.counts.known_tokens(), Some(0));
        for mut incoming in [
            vec![bare.clone(), split.clone()],
            vec![split.clone(), bare.clone()],
        ] {
            incoming.sort_by_key(|row| row.counts.ordered());
            let mut reducer = Reducer::default();
            for observation in incoming {
                reducer.push_sorted(observation);
            }
            let result = reducer.finish().unwrap();
            assert_eq!(result.resolution, Resolution::Resolved);
            assert_eq!(result.counts.known_tokens(), Some(42_385));
            assert_eq!(result.at_ms, bare.at_ms);
        }
    }

    #[test]
    fn missing_negative_overflow_and_inconsistent_fields_stay_distinct() {
        let json = line(
            "claude-sonnet-4-6",
            "not-a-time",
            r#""input_tokens":null,"cache_read_input_tokens":-1,"cache_creation_input_tokens":5,"cache_creation":{"ephemeral_5m_input_tokens":4,"ephemeral_1h_input_tokens":2},"output_tokens":9223372036854775808"#,
        );
        let data = observation(&json, [2; 32]);
        assert_eq!(data.at_ms, None);
        assert_eq!(data.counts.uncached_input, None);
        assert_eq!(data.counts.cache_read, None);
        assert_eq!(data.counts.cache_write, Some(5));
        assert_eq!(data.counts.cache_write_1h, None);
        assert_eq!(data.counts.output, None);
        assert_eq!(
            data.counts.flags,
            Counts::READ_INVALID | Counts::DURATION_INVALID | Counts::OUTPUT_INVALID
        );
        assert!(!data.counts.is_complete());
        assert_eq!(data.counts.known_tokens(), Some(5));
    }

    #[test]
    fn model_secret_does_not_appear_in_observation() {
        let json = line(
            "sk-ant-private-canary",
            "2026-11-26T00:00:00Z",
            r#""output_tokens":1"#,
        );
        let data = observation(&json, [2; 32]);
        assert_eq!(data.model, None);
        assert!(data.model_key.is_some());
        assert!(!format!("{data:?}").contains("sk-ant-private-canary"));
    }

    #[test]
    fn rejects_oversized_records_and_positional_usage_arrays() {
        let fake = br#"{"type":"assistant","requestId":"req_fixture","message":{"id":"msg_fixture","usage":[100,30,10,null,20]}}"#;
        assert!(matches!(parse(fake, [2; 32], &[1; 32]), Record::Unresolved));
        let oversized = vec![b' '; MAX_RECORD_BYTES + 1];
        assert!(matches!(
            parse(&oversized, [2; 32], &[1; 32]),
            Record::Unresolved
        ));
    }

    #[test]
    fn non_usage_rows_and_rows_without_stable_identity_are_distinct() {
        let ignored = br#"{"type":"user","message":{"content":"canary"}}"#;
        let unresolved = br#"{"type":"assistant","message":{"usage":{"output_tokens":5}}}"#;
        assert!(matches!(parse(ignored, [2; 32], &[1; 32]), Record::Ignored));
        assert!(matches!(
            parse(unresolved, [2; 32], &[1; 32]),
            Record::Unresolved
        ));
    }
}
