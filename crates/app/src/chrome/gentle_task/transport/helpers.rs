//! Read the current owner's helper projection through Gentle Desktop RPC.
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub(super) const SCHEMA: &str = "gentle-shell.desktop-rpc/v1";
pub(in super::super) const LIMIT: usize = 50;
const PREFIX: &str = "ivy-helpers-";
const TIMEOUT: Duration = Duration::from_secs(12);
const REFRESH: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct Helper {
    pub(in super::super) id: String,
    pub(in super::super) name: String,
    pub(in super::super) state: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum Snapshot {
    Reset,
    Rows(Vec<Helper>),
    Unavailable,
}

#[derive(Copy, Clone, PartialEq)]
enum Read {
    Commands,
    Capabilities,
    List,
}

struct Pending {
    read: Read,
    id: String,
    deadline: Instant,
}

#[derive(Default)]
pub(super) struct Monitor {
    next_id: u64,
    supports_snapshot: Option<bool>,
    pending: Option<Pending>,
    refresh: Option<(Instant, Read)>,
    event_support: bool,
    sequence: u64,
}

type Update = (Option<Value>, Option<Snapshot>);

pub(super) fn our_id(v: &Value) -> bool {
    v.as_str()
        .and_then(|s| s.strip_prefix(PREFIX))
        .is_some_and(|digits| {
            !digits.is_empty() && digits.len() <= 20 && digits.bytes().all(|b| b.is_ascii_digit())
        })
}

impl Monitor {
    pub(super) fn supports_snapshot(&self) -> Option<bool> {
        self.supports_snapshot
    }

    pub(super) fn start(&mut self, now: Instant) -> Update {
        self.supports_snapshot = None;
        self.event_support = false;
        self.sequence = 0;
        self.refresh = None;
        (
            Some(self.request(Read::Commands, now)),
            Some(Snapshot::Reset),
        )
    }

    fn request(&mut self, read: Read, now: Instant) -> Value {
        self.next_id += 1;
        let id = format!("{PREFIX}{}", self.next_id);
        let value = match read {
            Read::Commands => json!({"type":"get_commands","id":id}),
            Read::Capabilities | Read::List => {
                let method = match read {
                    Read::Capabilities => "capabilities",
                    _ => "list_subagents",
                };
                let inner = json!({"schema":SCHEMA,"requestId":id,"method":method,"params":{}});
                json!({"id":id,"type":"prompt","message":format!("/gentle:desktop {inner}")})
            }
        };
        self.pending = Some(Pending {
            read,
            id,
            deadline: now + TIMEOUT,
        });
        value
    }

    pub(super) fn write_failed(&mut self, now: Instant) -> Snapshot {
        if let Some(request) = self.pending.take() {
            self.refresh = Some((now + REFRESH, request.read));
        }
        Snapshot::Unavailable
    }

    pub(super) fn tick(&mut self, now: Instant) -> Update {
        if self.pending.as_ref().is_some_and(|q| now >= q.deadline) {
            return (None, Some(self.write_failed(now)));
        }
        if self.pending.is_none()
            && let Some((when, read)) = self.refresh
            && now >= when
        {
            self.refresh = None;
            return (Some(self.request(read, now)), None);
        }
        (None, None)
    }

    pub(super) fn observe(&mut self, v: &Value, now: Instant) -> Update {
        if v["schema"] == SCHEMA && v["kind"] == "event" && v["event"] == "agents_update" {
            if !self.event_support {
                return (None, None);
            }
            let Some(seq) = v["seq"].as_u64().filter(|&seq| seq != 0) else {
                return self.unavailable_list(now);
            };
            if seq <= self.sequence {
                return (None, None);
            }
            self.sequence = seq;
            // This full push supersedes both the initial read and its timeout.
            if self.pending.as_ref().is_some_and(|r| r.read == Read::List) {
                self.pending = None;
            }
            return self.rows(&v["data"], now);
        }

        let Some(request) = self.pending.as_ref() else {
            return (None, None);
        };
        let native = v["type"] == "response" && v["id"] == request.id;
        let domain =
            v["schema"] == SCHEMA && v["kind"] == "response" && v["requestId"] == request.id;
        let success = match request.read {
            Read::Commands if native && v["command"] == "get_commands" => v["success"] == true,
            Read::Capabilities | Read::List if domain => v["ok"] == true,
            Read::Capabilities | Read::List
                if native && v["command"] == "prompt" && v["success"] == false =>
            {
                false
            }
            _ => return (None, None),
        };
        let request = self.pending.take().expect("matched helper request");
        if !success {
            if domain && v["error"]["code"] == "session_switching" {
                self.refresh = Some((now + REFRESH, Read::Commands));
                self.event_support = false;
                return (None, Some(Snapshot::Unavailable));
            }
            if request.read == Read::List {
                return self.unavailable_list(now);
            }
            return (None, Some(Snapshot::Unavailable));
        }
        let data = &v["data"];
        match request.read {
            Read::Commands => {
                if !data["commands"].as_array().is_some_and(|commands| {
                    commands
                        .iter()
                        .any(|c| c["name"] == "gentle:desktop" && c["source"] == "extension")
                }) {
                    return (None, Some(Snapshot::Unavailable));
                }
                (Some(self.request(Read::Capabilities, now)), None)
            }
            Read::Capabilities => {
                if let Some(methods) = data["methods"]
                    .as_array()
                    .filter(|m| m.iter().all(Value::is_string))
                {
                    self.supports_snapshot = Some(methods.iter().any(|m| m == "get_snapshot"));
                }
                if !has(&data["methods"], "list_subagents") {
                    return (None, Some(Snapshot::Unavailable));
                }
                self.event_support = has(&data["events"], "agents_update");
                (Some(self.request(Read::List, now)), None)
            }
            Read::List => self.rows(data, now),
        }
    }

    fn unavailable_list(&mut self, now: Instant) -> Update {
        self.refresh = Some((now + REFRESH, Read::List));
        (None, Some(Snapshot::Unavailable))
    }

    fn rows(&mut self, data: &Value, now: Instant) -> Update {
        let Some(helpers) = parse(data) else {
            return self.unavailable_list(now);
        };
        self.refresh = (!self.event_support).then_some((now + REFRESH, Read::List));
        (None, Some(Snapshot::Rows(helpers)))
    }
}

fn has(list: &Value, item: &str) -> bool {
    list.as_array()
        .is_some_and(|entries| entries.iter().any(|entry| entry == item))
}

fn label(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| {
        !s.trim().is_empty()
            && s.len() <= 256
            && !s.chars().any(|c| {
                c.is_control() || matches!(c, '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            })
    })
}

fn parse(data: &Value) -> Option<Vec<Helper>> {
    let rows = data["agents"]
        .as_array()
        .filter(|rows| rows.len() <= LIMIT)?;
    let mut ids = BTreeSet::new();
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let id = label(&row["id"])?;
        if !ids.insert(id) {
            return None;
        }
        let name = label(&row["agent"])?;
        let state = match row["status"].as_str()? {
            "queued" => "queued",
            "running" => "running",
            "waiting" => "waiting",
            "completed" => "completed",
            "failed" => "failed",
            "cancelled" => "cancelled",
            "timed_out" => "timed_out",
            _ => return None,
        };
        result.push(Helper {
            id: id.into(),
            name: name.into(),
            state,
        });
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(request: &Value, data: Value) -> Value {
        json!({"schema":SCHEMA,"kind":"response","ok":true,"requestId":request["id"],"data":data})
    }

    fn commands(request: &Value, source: &str) -> Value {
        json!({"type":"response","id":request["id"],"command":"get_commands","success":true,
               "data":{"commands":[{"name":"gentle:desktop","source":source}]}})
    }

    fn list_request(m: &mut Monitor, now: Instant, events: Value) -> Value {
        let start = m.start(now).0.unwrap();
        let capabilities = m.observe(&commands(&start, "extension"), now).0.unwrap();
        m.observe(
            &response(
                &capabilities,
                json!({"methods":["list_subagents"],"events":events}),
            ),
            now,
        )
        .0
        .unwrap()
    }

    fn push(seq: u64, state: &str) -> Value {
        json!({"schema":SCHEMA,"kind":"event","seq":seq,"event":"agents_update",
               "data":{"agents":[{"id":"shared","agent":"reader","status":state}]}})
    }

    fn state(update: Update) -> &'static str {
        let Some(Snapshot::Rows(rows)) = update.1 else {
            panic!("missing rows")
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "shared");
        assert_eq!(rows[0].name, "reader");
        rows[0].state
    }

    #[test]
    fn a_full_push_supersedes_the_pending_read_and_its_deadline() {
        let mut m = Monitor::default();
        let now = Instant::now();
        let request = list_request(&mut m, now, json!(["agents_update"]));
        assert_eq!(state(m.observe(&push(4, "running"), now)), "running");
        assert_eq!(
            m.observe(&response(&request, json!({"agents":[]})), now),
            (None, None)
        );
        assert_eq!(m.tick(now + TIMEOUT), (None, None));
        assert_eq!(m.observe(&push(3, "failed"), now), (None, None));
        assert_eq!(m.observe(&push(4, "cancelled"), now), (None, None));
        assert_eq!(state(m.observe(&push(12, "completed"), now)), "completed");
        let empty = json!({"schema":SCHEMA,"kind":"event","seq":15,"event":"agents_update","data":{"agents":[]}});
        assert_eq!(m.observe(&empty, now).1, Some(Snapshot::Rows(vec![])));
    }

    #[test]
    fn sessions_reusing_a_helper_id_keep_independent_snapshots() {
        let now = Instant::now();
        let mut a = Monitor::default();
        let mut b = Monitor::default();
        for host in [&mut a, &mut b] {
            list_request(host, now, json!(["agents_update"]));
        }
        assert_eq!(state(a.observe(&push(3, "failed"), now)), "failed");
        assert_eq!(state(b.observe(&push(1, "running"), now)), "running");
        assert_eq!(state(b.observe(&push(2, "waiting"), now)), "waiting");
        assert_eq!(a.observe(&push(2, "waiting"), now), (None, None));
    }

    #[test]
    fn owner_reset_invalidates_old_reads_and_restarts_sequence_numbers() {
        let mut m = Monitor::default();
        let now = Instant::now();
        let old = list_request(&mut m, now, json!(["agents_update"]));
        let (new, reset) = m.start(now);
        assert_eq!(reset, Some(Snapshot::Reset));
        assert_ne!(new.unwrap()["id"], old["id"]);
        assert_eq!(
            m.observe(&response(&old, json!({"agents":[]})), now),
            (None, None)
        );
        assert_eq!(m.observe(&push(999, "failed"), now), (None, None));
        list_request(&mut m, now, json!(["agents_update"]));
        assert_eq!(state(m.observe(&push(1, "running"), now)), "running");
    }

    #[test]
    fn missing_capabilities_and_prompt_templates_never_send_a_desktop_command() {
        let now = Instant::now();
        let mut m = Monitor::default();
        let start = m.start(now).0.unwrap();
        assert_eq!(
            m.observe(&commands(&start, "prompt"), now),
            (None, Some(Snapshot::Unavailable))
        );
        let start = m.start(now).0.unwrap();
        let caps = m.observe(&commands(&start, "extension"), now).0.unwrap();
        let unrelated =
            json!({"type":"response","command":"prompt","success":true,"id":caps["id"]});
        assert_eq!(m.observe(&unrelated, now), (None, None));
        assert_eq!(
            m.observe(&response(&caps, json!({"methods":[]})), now),
            (None, Some(Snapshot::Unavailable))
        );
        assert_eq!(m.tick(now + TIMEOUT), (None, None));
    }

    #[test]
    fn list_only_owners_refresh_and_invalid_rows_recover_via_a_correlated_read() {
        let now = Instant::now();
        let mut m = Monitor::default();
        let first = list_request(&mut m, now, json!([]));
        assert_eq!(
            m.observe(&response(&first, json!({"agents":[]})), now).1,
            Some(Snapshot::Rows(vec![]))
        );
        assert_eq!(m.observe(&push(7, "running"), now), (None, None));
        let second = m.tick(now + REFRESH).0.unwrap();
        assert_ne!(second["id"], first["id"]);
        assert_eq!(
            m.observe(&response(&second, json!({"agents":null})), now).1,
            Some(Snapshot::Unavailable)
        );
        assert!(m.tick(now + REFRESH).0.is_some());
        assert_eq!(
            m.tick(now + REFRESH + TIMEOUT).1,
            Some(Snapshot::Unavailable)
        );
    }

    #[test]
    fn producer_rows_keep_identifiers_unique_and_bounds_explicit() {
        let row = &push(1, "running")["data"]["agents"][0];
        assert!(parse(&json!({"agents":[row,row]})).is_none());
        assert!(parse(&json!({"agents":vec![row; LIMIT+1]})).is_none());
        for (field, replacement) in [("id", ""), ("agent", "two\nlines"), ("status", "invented")] {
            let mut bad = row.clone();
            bad[field] = replacement.into();
            assert!(parse(&json!({"agents":[bad]})).is_none());
        }
        let rows: Vec<_> = (0..LIMIT)
            .map(|i| json!({"id":i.to_string(),"agent":"worker","status":"queued"}))
            .collect();
        assert_eq!(parse(&json!({"agents":rows})).unwrap().len(), LIMIT);
    }
    #[test]
    fn optional_snapshot_support_requires_exact_capability_receipt_without_helpers() {
        let now = Instant::now();
        let mut m = Monitor::default();
        let start = m.start(now).0.unwrap();
        let capabilities = m.observe(&commands(&start, "extension"), now).0.unwrap();
        let mut foreign = response(&capabilities, json!({"methods":["get_snapshot"]}));
        foreign["requestId"] = json!("ivy-helpers-999");
        m.observe(&foreign, now);
        assert_eq!(m.supports_snapshot(), None);
        let update = m.observe(
            &response(&capabilities, json!({"methods":["get_snapshot"]})),
            now,
        );
        assert!(
            update.0.is_none(),
            "no list_subagents request when unavailable"
        );
        assert_eq!(m.supports_snapshot(), Some(true));
        m.start(now);
        assert_eq!(m.supports_snapshot(), None);
    }
}
