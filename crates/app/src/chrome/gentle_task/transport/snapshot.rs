//! Correlated, optional producer task/activity reads; no state is inferred from text.
use super::helpers::SCHEMA;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

const PREFIX: &str = "ivy-view-";
const TIMEOUT: Duration = Duration::from_secs(12);
const REFRESH: Duration = Duration::from_secs(5);
const MAX_ITEMS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Todo {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) state: String,
    pub(crate) note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SnapshotState {
    pub(crate) items: Vec<Todo>,
    pub(crate) activity: Option<String>,
    pub(crate) truncated: bool,
    pub(crate) unavailable: bool,
}

impl Default for SnapshotState {
    fn default() -> Self {
        Self {
            items: vec![],
            activity: None,
            truncated: false,
            unavailable: true,
        }
    }
}

#[derive(Debug)]
pub(crate) enum SnapshotUpdate {
    Reset,
    Unavailable,
    Rows(SnapshotState),
}

impl SnapshotState {
    pub(crate) fn apply(&mut self, update: SnapshotUpdate) {
        match update {
            SnapshotUpdate::Rows(next) => *self = next,
            SnapshotUpdate::Unavailable => self.unavailable = true,
            SnapshotUpdate::Reset => *self = Self::default(),
        }
    }
}

#[derive(Default)]
pub(super) struct SnapshotMonitor {
    next: u64,
    support: Option<bool>,
    pending: Option<(String, Instant)>,
    refresh: Option<Instant>,
}

pub(super) fn our_id(v: &Value) -> bool {
    v.as_str()
        .and_then(|s| s.strip_prefix(PREFIX))
        .is_some_and(|digits| {
            !digits.is_empty() && digits.len() <= 20 && digits.bytes().all(|b| b.is_ascii_digit())
        })
}

impl SnapshotMonitor {
    pub(super) fn reset(&mut self) -> SnapshotUpdate {
        self.pending = None;
        self.support = None;
        self.refresh = None;
        SnapshotUpdate::Reset
    }

    pub(super) fn tick(
        &mut self,
        support: Option<bool>,
        now: Instant,
    ) -> (Option<Value>, Option<SnapshotUpdate>) {
        if self.support != support {
            let was_supported = self.support == Some(true);
            self.support = support;
            self.pending = None;
            self.refresh = Some(now);
            if support != Some(true) {
                return (None, was_supported.then_some(SnapshotUpdate::Unavailable));
            }
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, deadline)| now >= *deadline)
        {
            return (None, Some(self.failed(now)));
        }
        if support != Some(true)
            || self.pending.is_some()
            || self.refresh.is_some_and(|at| now < at)
        {
            return (None, None);
        }
        self.next += 1;
        let id = format!("{PREFIX}{}", self.next);
        self.pending = Some((id.clone(), now + TIMEOUT));
        self.refresh = None;
        let inner = json!({"schema":SCHEMA,"requestId":id,"method":"get_snapshot","params":{"sections":["todo","activity"]}});
        (
            Some(json!({"type":"prompt","id":id,"message":format!("/gentle:desktop {inner}")})),
            None,
        )
    }

    pub(super) fn failed(&mut self, now: Instant) -> SnapshotUpdate {
        self.pending = None;
        self.refresh = Some(now + REFRESH);
        SnapshotUpdate::Unavailable
    }

    pub(super) fn observe(&mut self, v: &Value, now: Instant) -> Option<SnapshotUpdate> {
        let (id, _) = self.pending.as_ref()?;
        if v["type"] == "response" && v["id"] == *id && v["command"] == "prompt" {
            return (v["success"] == false).then(|| self.failed(now));
        }
        if v["schema"] != SCHEMA || v["kind"] != "response" || v["requestId"] != *id {
            return None;
        }
        self.pending = None;
        self.refresh = Some(now + REFRESH);
        Some(if v["ok"] == true {
            parse(&v["data"]).map_or(SnapshotUpdate::Unavailable, SnapshotUpdate::Rows)
        } else {
            SnapshotUpdate::Unavailable
        })
    }
}

fn parse(data: &Value) -> Option<SnapshotState> {
    let session = &data["state"]["session"];
    let todo = &session["todo"];
    let rows = todo["items"]
        .as_array()
        .filter(|items| items.len() <= MAX_ITEMS)?;
    let mut ids = BTreeSet::new();
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let id = row["id"].as_u64()?;
        if !ids.insert(id) {
            return None;
        }
        let title = row["title"].as_str()?.to_owned();
        let state = row["state"].as_str()?.to_owned();
        let note = if row["note"].is_null() {
            None
        } else {
            Some(row["note"].as_str()?.to_owned())
        };
        items.push(Todo {
            id,
            title,
            state,
            note,
        });
    }
    Some(SnapshotState {
        items,
        activity: Some(session["activity"]["label"].as_str()?.to_owned()),
        truncated: todo["truncated"].as_bool()?,
        unavailable: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn data() -> Value {
        json!({"state":{"session":{"todo":{"items":[{"id":1,"title":"Build feed","state":"in_progress","note":null}],"truncated":true},"activity":{"label":"Working"}}}})
    }
    fn response(id: &Value, data: Value) -> Value {
        json!({"schema":SCHEMA,"kind":"response","requestId":id,"ok":true,"data":data})
    }
    #[test]
    fn reads_only_advertised_sections_and_keeps_numeric_identity() {
        let mut monitor = SnapshotMonitor::default();
        let now = Instant::now();
        assert!(monitor.tick(None, now).0.is_none());
        assert!(monitor.tick(Some(false), now).0.is_none());
        let request = monitor.tick(Some(true), now).0.unwrap();
        let inner: Value = serde_json::from_str(
            request["message"]
                .as_str()
                .unwrap()
                .strip_prefix("/gentle:desktop ")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(inner["params"], json!({"sections":["todo","activity"]}));
        let SnapshotUpdate::Rows(state) = monitor
            .observe(&response(&request["id"], data()), now)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(state.items[0].id, 1);
        assert!(state.truncated);
        assert_eq!(state.activity.as_deref(), Some("Working"));
    }
    #[test]
    fn receipts_foreign_late_and_reset_cannot_replace_rows() {
        let now = Instant::now();
        let mut monitor = SnapshotMonitor::default();
        let first = monitor.tick(Some(true), now).0.unwrap();
        assert!(
            monitor
                .observe(
                    &json!({"type":"response","id":first["id"],"command":"prompt","success":true}),
                    now
                )
                .is_none()
        );
        assert!(
            monitor
                .observe(&response(&json!("ivy-view-999"), data()), now)
                .is_none()
        );
        let mut wrong = response(&first["id"], data());
        wrong["schema"] = json!("foreign");
        assert!(monitor.observe(&wrong, now).is_none());
        assert!(matches!(
            monitor.tick(Some(true), now + TIMEOUT).1,
            Some(SnapshotUpdate::Unavailable)
        ));
        assert!(
            monitor
                .observe(&response(&first["id"], data()), now)
                .is_none()
        );
        monitor.reset();
        let second = monitor.tick(Some(true), now).0.unwrap();
        assert_ne!(first["id"], second["id"]);
        assert!(
            monitor
                .observe(&response(&first["id"], data()), now)
                .is_none()
        );
        let mut state = parse(&data()).unwrap();
        let mut invalid = data();
        invalid["state"]["session"]["todo"]["items"][0]["id"] = json!("1");
        state.apply(
            monitor
                .observe(&response(&second["id"], invalid), now)
                .unwrap(),
        );
        assert!(state.unavailable);
        assert_eq!(state.items[0].id, 1);
    }
    #[test]
    fn malformed_and_oversized_sections_are_not_empty() {
        assert!(parse(&json!({})).is_none());
        let mut d = data();
        d["state"]["session"]["todo"]["items"] = json!([]);
        assert!(parse(&d).unwrap().items.is_empty());
        d["state"]["session"]["todo"]["items"] = json!(vec![
            json!({"id":1,"title":"x","state":"pending","note":null});
            201
        ]);
        assert!(parse(&d).is_none());
        let mut d = data();
        d["state"]["session"]["todo"]["items"][0]["title"] = json!(42);
        assert!(parse(&d).is_none());
    }

    #[test]
    fn producer_unicode_and_multiline_text_survives_the_snapshot() {
        let mut monitor = SnapshotMonitor::default();
        let now = Instant::now();
        let request = monitor.tick(Some(true), now).0.unwrap();
        let title = "界".repeat(499) + "…";
        let note = "First line\nSecond line\twith a tab";
        let mut value = data();
        value["state"]["session"]["todo"]["items"][0]["title"] = json!(title);
        value["state"]["session"]["todo"]["items"][0]["note"] = json!(note);
        let update = monitor
            .observe(&response(&request["id"], value), now)
            .unwrap();
        let SnapshotUpdate::Rows(snapshot) = update else {
            panic!("valid producer text made the complete task list unavailable")
        };
        assert_eq!(snapshot.items[0].title, title);
        assert_eq!(snapshot.items[0].note.as_deref(), Some(note));
    }
}
