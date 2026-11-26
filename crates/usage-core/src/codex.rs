//! Streaming projection of local Codex usage rows into minimized accounting.

use crate::accounting::{Counts, Observation, Provider};
use crate::claude::Record;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::value::RawValue;
use sha2::Sha256;

const RECORD_LIMIT: usize = 1024 * 1024;
const LABEL_LIMIT: usize = 128;
const REASONING_INVALID: i64 = 1 << 5;

#[derive(Clone, Default)]
pub(crate) struct Parser {
    session: Option<[u8; 32]>,
    model: Option<[u8; 32]>,
    model_label: Option<&'static str>,
    previous: Option<Counters>,
    unproved_lineage: bool,
    baseline_lost: bool,
}

#[derive(Deserialize)]
struct Entry<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    #[serde(borrow)]
    payload: &'a RawValue,
    #[serde(default, borrow)]
    timestamp: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Session<'a> {
    id: &'a str,
    #[serde(borrow)]
    forked_from_id: Option<&'a RawValue>,
    #[serde(borrow)]
    source: Option<&'a RawValue>,
    thread_source: Option<&'a str>,
}

#[derive(Deserialize)]
struct Turn<'a> {
    #[serde(borrow)]
    model: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Event<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    #[serde(borrow)]
    info: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct Usage<'a> {
    #[serde(borrow)]
    last_token_usage: Option<&'a RawValue>,
    #[serde(borrow)]
    total_token_usage: Option<&'a RawValue>,
}

#[derive(Clone, Copy, Default)]
struct Counters {
    input: Option<i64>,
    cached: Option<i64>,
    output: Option<i64>,
    reasoning: Option<i64>,
    all: Option<i64>,
    flags: i64,
}

#[derive(Deserialize)]
struct RawCounters<'a> {
    #[serde(borrow)]
    input_tokens: Option<&'a RawValue>,
    #[serde(borrow)]
    cached_input_tokens: Option<&'a RawValue>,
    #[serde(borrow)]
    output_tokens: Option<&'a RawValue>,
    #[serde(borrow)]
    reasoning_output_tokens: Option<&'a RawValue>,
    #[serde(borrow)]
    total_tokens: Option<&'a RawValue>,
}

impl Counters {
    fn read(raw: &RawValue) -> Option<Self> {
        let counters: RawCounters<'_> = decode(raw)?;
        let mut flags = 0;
        let input = count(counters.input_tokens, Counts::INPUT_INVALID, &mut flags);
        let mut cached = count(
            counters.cached_input_tokens,
            Counts::READ_INVALID,
            &mut flags,
        );
        let output = count(counters.output_tokens, Counts::OUTPUT_INVALID, &mut flags);
        let mut reasoning = count(
            counters.reasoning_output_tokens,
            REASONING_INVALID,
            &mut flags,
        );
        let all = count(
            counters.total_tokens,
            Counts::INPUT_INVALID | Counts::OUTPUT_INVALID,
            &mut flags,
        );
        validate_subset(input, &mut cached, Counts::READ_INVALID, &mut flags);
        validate_subset(output, &mut reasoning, REASONING_INVALID, &mut flags);
        if let (Some(input), Some(output), Some(all)) = (input, output, all)
            && input.checked_add(output) != Some(all)
        {
            flags |= Counts::INPUT_INVALID | Counts::OUTPUT_INVALID;
        }
        Some(Self {
            input,
            cached,
            output,
            reasoning,
            all,
            flags,
        })
    }

    fn delta(self, previous: Self) -> Self {
        let mut flags = self.flags | previous.flags;
        let mut sub =
            |current: Option<i64>, previous: Option<i64>, bad: i64| match (current, previous) {
                (Some(current), Some(previous)) if current >= previous => Some(current - previous),
                (Some(_), Some(_)) => {
                    flags |= bad;
                    None
                }
                _ => None,
            };
        let input = sub(self.input, previous.input, Counts::INPUT_INVALID);
        let cached = sub(self.cached, previous.cached, Counts::READ_INVALID);
        let output = sub(self.output, previous.output, Counts::OUTPUT_INVALID);
        let reasoning = sub(self.reasoning, previous.reasoning, REASONING_INVALID);
        Self {
            input,
            cached,
            output,
            reasoning,
            all: None,
            flags,
        }
    }

    fn pair(self) -> Option<(i64, i64)> {
        let input = self.input?;
        let output = self.output?;
        input.checked_add(output)?;
        (self.flags & (Counts::INPUT_INVALID | Counts::OUTPUT_INVALID) == 0)
            .then_some((input, output))
    }
}

impl Parser {
    pub(crate) fn invalidate_baseline(&mut self) {
        self.previous = None;
        self.baseline_lost = true;
    }

    pub(crate) fn parse(&mut self, line: &[u8], source_key: [u8; 32], salt: &[u8; 32]) -> Record {
        if line.len() > RECORD_LIMIT {
            self.invalidate_baseline();
            return Record::Unresolved;
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            return Record::Ignored;
        }
        let Ok(entry) = serde_json::from_slice::<Entry<'_>>(line) else {
            self.invalidate_baseline();
            return Record::Unresolved;
        };
        match entry.kind {
            "session_meta" => self.session(entry.payload, salt),
            "turn_context" => {
                let model = decode::<Turn<'_>>(entry.payload)
                    .and_then(|turn| string(turn.model?, LABEL_LIMIT))
                    .filter(|model| !model.is_empty());
                self.model_label = model.as_deref().and_then(|label| match label {
                    "gpt-5-codex" => Some("gpt-5-codex"),
                    "gpt-5.1-codex" => Some("gpt-5.1-codex"),
                    "gpt-5.1-codex-max" => Some("gpt-5.1-codex-max"),
                    "gpt-5.2-codex" => Some("gpt-5.2-codex"),
                    "gpt-5.3-codex" => Some("gpt-5.3-codex"),
                    "gpt-5.4" => Some("gpt-5.4"),
                    _ => None,
                });
                self.model =
                    model.map(|model| keyed(salt, b"codex usage model v1", &[model.as_bytes()]));
                Record::Ignored
            }
            "event_msg" => self.event(entry, source_key, salt),
            _ => Record::Ignored,
        }
    }

    fn session(&mut self, raw: &RawValue, salt: &[u8; 32]) -> Record {
        let Some(session) = decode::<Session<'_>>(raw) else {
            self.unproved_lineage = true;
            return Record::Unresolved;
        };
        let nested_source = session
            .source
            .is_some_and(|source| source.get().starts_with('{'));
        let invalid_id = session.id.is_empty()
            || session.id.len() > LABEL_LIMIT
            || !session
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'));
        if invalid_id
            || session.forked_from_id.is_some()
            || nested_source
            || !matches!(session.thread_source, None | Some("user"))
        {
            self.unproved_lineage = true;
            return Record::Unresolved;
        }
        let id = keyed(salt, b"codex usage session v1", &[session.id.as_bytes()]);
        if self.session.is_some_and(|existing| existing != id) {
            self.unproved_lineage = true;
            return Record::Unresolved;
        }
        self.session = Some(id);
        Record::Ignored
    }

    fn event(&mut self, entry: Entry<'_>, source_key: [u8; 32], salt: &[u8; 32]) -> Record {
        let Some(event) = decode::<Event<'_>>(entry.payload) else {
            self.invalidate_baseline();
            return Record::Unresolved;
        };
        if event.kind != "token_count" {
            return Record::Ignored;
        }
        // An event can carry rate limits only. There is no accounting payload.
        let Some(info) = event.info else {
            return Record::Ignored;
        };
        if self.unproved_lineage {
            return Record::Unresolved;
        }
        let Some(session) = self.session else {
            return Record::Unresolved;
        };
        let Some(usage) = decode::<Usage<'_>>(info) else {
            self.invalidate_baseline();
            return Record::Unresolved;
        };
        let Some(total) = usage.total_token_usage.and_then(Counters::read) else {
            self.invalidate_baseline();
            return Record::Unresolved;
        };
        let Some((input, output)) = total.pair() else {
            self.invalidate_baseline();
            return Record::Unresolved;
        };
        let last = usage.last_token_usage.and_then(Counters::read);
        let previous = self.previous;
        if let Some((old_input, old_output)) = previous.and_then(Counters::pair) {
            if (input, output) == (old_input, old_output) {
                return if previous.is_some_and(|previous| contradicts(previous, total)) {
                    Record::Unresolved
                } else {
                    Record::Ignored
                };
            }
            if input < old_input || output < old_output {
                // A regressive snapshot supplies neither a stable identity
                // nor a reason to forget the last accepted high-water mark.
                return Record::Unresolved;
            }
        }
        self.previous = Some(total);
        if previous.is_none() && self.baseline_lost {
            return Record::Unresolved;
        }
        if (input, output) == (0, 0) {
            return Record::Ignored;
        }
        let mut delta = if let Some(previous) = previous {
            let mut delta = total.delta(previous);
            if last.is_some_and(|last| contradicts(delta, last)) {
                return Record::Unresolved;
            }
            if let Some(last) = last.filter(|last| last.pair() == delta.pair()) {
                delta.cached = delta.cached.or(last.cached);
                delta.reasoning = delta.reasoning.or(last.reasoning);
                delta.flags |= last.flags;
            }
            delta
        } else {
            let Some(last) = last.filter(|last| last.pair() == Some((input, output))) else {
                // A verified total establishes a subsequent baseline without
                // assigning all earlier usage to the first observed instant.
                return Record::Unresolved;
            };
            if contradicts(total, last) {
                return Record::Unresolved;
            }
            last
        };
        validate_subset(
            delta.input,
            &mut delta.cached,
            Counts::READ_INVALID,
            &mut delta.flags,
        );
        validate_subset(
            delta.output,
            &mut delta.reasoning,
            REASONING_INVALID,
            &mut delta.flags,
        );
        let Some(at_ms) = entry
            .timestamp
            .and_then(|raw| string(raw, 48))
            .and_then(|raw| chrono::DateTime::parse_from_rfc3339(&raw).ok())
            .map(|date| date.timestamp_millis())
        else {
            return Record::Unresolved;
        };
        let counts = Counts {
            uncached_input: delta
                .input
                .zip(delta.cached)
                .map(|(input, cached)| input - cached),
            cache_read: delta.cached,
            cache_write: Some(0),
            cache_write_1h: Some(0),
            output: delta.output,
            reasoning_output: delta.reasoning,
            flags: delta.flags,
        };
        let event_key = keyed(
            salt,
            b"codex usage event v1",
            &[&session, &input.to_be_bytes(), &output.to_be_bytes()],
        );
        let values = [
            counts.uncached_input,
            counts.cache_read,
            counts.output,
            counts.reasoning_output,
        ];
        let bytes = values.map(|count| count.map(i64::to_be_bytes));
        let evidence_key = keyed(
            salt,
            b"codex usage evidence v1",
            &[
                &at_ms.to_be_bytes(),
                self.model.as_ref().map_or(&[], |key| key.as_slice()),
                bytes[0].as_ref().map_or(&[], |value| value.as_slice()),
                bytes[1].as_ref().map_or(&[], |value| value.as_slice()),
                bytes[2].as_ref().map_or(&[], |value| value.as_slice()),
                bytes[3].as_ref().map_or(&[], |value| value.as_slice()),
                &counts.flags.to_be_bytes(),
            ],
        );
        Record::Observation(Observation {
            provider: Provider::Codex,
            event_key,
            source_key,
            evidence_key,
            at_ms: Some(at_ms),
            model: self.model_label.map(str::to_owned),
            model_key: self.model,
            counts,
        })
    }
}

fn contradicts(a: Counters, b: Counters) -> bool {
    [
        (a.input, b.input),
        (a.cached, b.cached),
        (a.output, b.output),
        (a.reasoning, b.reasoning),
        (a.all, b.all),
    ]
    .into_iter()
    .any(|(a, b)| a.zip(b).is_some_and(|(a, b)| a != b))
}

fn validate_subset(total: Option<i64>, subset: &mut Option<i64>, bit: i64, flags: &mut i64) {
    if total
        .zip(*subset)
        .is_some_and(|(total, subset)| subset > total)
    {
        *flags |= bit;
        *subset = None;
    }
}

fn count(raw: Option<&RawValue>, bit: i64, flags: &mut i64) -> Option<i64> {
    let raw = raw?;
    let value = decode::<i64>(raw).filter(|value| *value >= 0);
    if value.is_none() {
        *flags |= bit;
    }
    value
}

fn decode<'a, T: Deserialize<'a>>(raw: &'a RawValue) -> Option<T> {
    serde_json::from_str(raw.get()).ok()
}

fn string(raw: &RawValue, limit: usize) -> Option<String> {
    if raw.get().len() > 6 * limit + 2 {
        return None;
    }
    let value = decode::<String>(raw)?;
    (value.len() <= limit).then_some(value)
}

fn keyed(salt: &[u8; 32], domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(salt).expect("fixed-size usage identity key");
    mac.update(domain);
    for field in fields {
        mac.update(&(field.len() as u64).to_be_bytes());
        mac.update(field);
    }
    mac.finalize().into_bytes().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: &[u8] = br#"{"type":"event_msg","timestamp":"2026-11-26T10:00:00Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"cached_input_tokens":30,"output_tokens":20,"reasoning_output_tokens":5,"total_tokens":120},"total_token_usage":{"input_tokens":100,"cached_input_tokens":30,"output_tokens":20,"reasoning_output_tokens":5,"total_tokens":120}}}}"#;
    const SECOND: &[u8] = br#"{"type":"event_msg","timestamp":"2026-11-26T10:00:00Z","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":100,"cached_input_tokens":30,"output_tokens":20,"reasoning_output_tokens":5},"total_token_usage":{"input_tokens":200,"cached_input_tokens":60,"output_tokens":40,"reasoning_output_tokens":10}}}}"#;
    const SESSION: &[u8] = br#"{"type":"session_meta","payload":{"id":"fixture-session"}}"#;

    fn row(parser: &mut Parser, line: &[u8]) -> Record {
        parser.parse(line, [1; 32], &[2; 32])
    }

    fn observation(parser: &mut Parser, line: &[u8]) -> Observation {
        let Record::Observation(observation) = row(parser, line) else {
            panic!("expected usage observation");
        };
        observation
    }

    #[test]
    fn duplicate_snapshots_do_not_count_equal_delta_twice() {
        let mut parser = Parser::default();
        assert!(matches!(row(&mut parser, SESSION), Record::Ignored));
        let first = observation(&mut parser, FIRST);
        assert_eq!(first.counts.uncached_input, Some(70));
        assert_eq!(first.counts.cache_read, Some(30));
        assert_eq!(first.counts.output, Some(20));
        assert_eq!(first.counts.reasoning_output, Some(5));
        assert!(matches!(row(&mut parser, FIRST), Record::Ignored));
        let second = observation(&mut parser, SECOND);
        assert_eq!(first.counts, second.counts);
        assert_eq!(first.at_ms, second.at_ms);
        assert_ne!(first.event_key, second.event_key);
        assert!(matches!(row(&mut parser, FIRST), Record::Unresolved));
        assert!(matches!(row(&mut parser, SECOND), Record::Ignored));
    }

    #[test]
    fn copied_sources_share_call_identity_and_model_names_remain_private() {
        let mut parser = Parser::default();
        row(&mut parser, SESSION);
        row(
            &mut parser,
            br#"{"type":"turn_context","payload":{"model":"private-canary"}}"#,
        );
        let mut copy = parser.clone();
        let first = observation(&mut parser, FIRST);
        let Record::Observation(second) = copy.parse(FIRST, [3; 32], &[2; 32]) else {
            panic!("expected copied usage observation");
        };
        assert_eq!(first.event_key, second.event_key);
        assert_eq!(first.evidence_key, second.evidence_key);
        assert_ne!(first.source_key, second.source_key);
        assert!(second.model.is_none());
        assert!(second.model_key.is_some());
    }

    #[test]
    fn inherited_or_unknown_baselines_do_not_become_usage() {
        let mut parser = Parser::default();
        assert!(matches!(row(&mut parser, FIRST), Record::Unresolved));
        row(&mut parser, SESSION);
        assert!(matches!(row(&mut parser, SECOND), Record::Unresolved));

        let mut fork = Parser::default();
        assert!(matches!(
            row(
                &mut fork,
                br#"{"type":"session_meta","payload":{"id":"child","forked_from_id":"parent"}}"#
            ),
            Record::Unresolved
        ));
        assert!(matches!(row(&mut fork, FIRST), Record::Unresolved));
    }
}
