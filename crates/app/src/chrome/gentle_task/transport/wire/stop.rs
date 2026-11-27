//! Stop receipts and ownership live on a single Gentle client's private pipes.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

pub(super) const SCHEMA: &str = "gentle-shell.desktop-rpc/v1";
const PREFIX: &str = "ivy-stop-";
const LIMIT: usize = 50;
const POLL: Duration = Duration::from_millis(250);
const TIMEOUT: Duration = Duration::from_secs(12);

pub(super) fn our_id(id: &Value) -> bool {
    id.as_str()
        .is_some_and(|id| id.starts_with(PREFIX) && id.len() <= 64)
}

#[derive(Clone, Copy, PartialEq)]
enum Group {
    Helpers,
    Jobs,
}
impl Group {
    fn methods(self) -> (&'static str, &'static str) {
        match self {
            Self::Helpers => ("list_subagents", "cancel_subagent"),
            Self::Jobs => ("list_jobs", "cancel_job"),
        }
    }
    fn fields(self) -> (&'static str, &'static str) {
        match self {
            Self::Helpers => ("agents", "agent"),
            Self::Jobs => ("jobs", "job"),
        }
    }
    fn ended(self, status: &str) -> Option<bool> {
        match (self, status) {
            (Self::Helpers, "running" | "waiting" | "queued") | (Self::Jobs, "running") => {
                Some(false)
            }
            (Self::Helpers, "completed" | "cancelled" | "timed_out" | "failed")
            | (Self::Jobs, "stopped" | "exited" | "failed") => Some(true),
            _ => None,
        }
    }
}

// Only new, explicitly listed IDs from this owner are eligible for a cancel request.
struct Work {
    group: Group,
    // An attempted ID stays here until the stop finishes, even when it falls out of a list.
    attempted: BTreeMap<String, bool>,
    prior: BTreeSet<String>,
    waiting: Vec<String>,
    ready: bool,
    done: bool,
    partial: bool,
    next_read: Instant,
}
impl Work {
    fn new(group: Group, now: Instant) -> Self {
        Self {
            group,
            attempted: BTreeMap::new(),
            prior: BTreeSet::new(),
            waiting: vec![],
            ready: false,
            done: false,
            partial: false,
            next_read: now,
        }
    }
    fn unavailable(&mut self) {
        self.partial = true;
        self.done = true;
    }
    fn list(&mut self, data: &Value, root_idle: bool, now: Instant) {
        let Some(rows) = data[self.group.fields().0]
            .as_array()
            .filter(|a| a.len() <= LIMIT)
        else {
            self.unavailable();
            return;
        };
        self.partial |= rows.len() == LIMIT;
        let mut seen = BTreeSet::new();
        for row in rows {
            let Some(id) = row["id"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 256)
            else {
                self.partial = true;
                continue;
            };
            let Some(ended) = row["status"].as_str().and_then(|s| self.group.ended(s)) else {
                self.partial = true;
                continue;
            };
            if !seen.insert(id) {
                self.partial = true;
                continue;
            }
            let previously_requested = self.prior.remove(id);
            if let Some(confirmed) = self.attempted.get_mut(id) {
                *confirmed = ended;
            } else if ended && previously_requested {
                self.attempted.insert(id.to_owned(), true);
            } else if !ended {
                if self.attempted.len() + self.prior.len() >= LIMIT {
                    self.partial = true;
                } else {
                    self.waiting.push(id.to_owned());
                    self.attempted.insert(id.to_owned(), false);
                }
            }
        }
        // Disappearance from a bounded owner view is never a terminal receipt.
        self.done = root_idle
            && self.prior.is_empty()
            && self.waiting.is_empty()
            && self.attempted.values().all(|ended| *ended);
        self.next_read = now + POLL;
    }
    fn cancel_reply(&mut self, id: &str, data: &Value) {
        let row = &data[self.group.fields().1];
        if row["id"].as_str() == Some(id)
            && let Some(ended) = row["status"].as_str().and_then(|s| self.group.ended(s))
            && let Some(previous) = self.attempted.get_mut(id)
        {
            *previous = ended;
            return;
        }
        self.partial = true;
    }
    fn summary(&self, noun: &str) -> String {
        let confirmed = self.attempted.values().filter(|value| **value).count();
        if self.partial || !self.done {
            format!("{noun}: {confirmed} confirmed ended (verification incomplete).")
        } else if self.attempted.is_empty() {
            format!("{noun}: none active in the final list.")
        } else {
            format!("{noun}: {confirmed} confirmed ended.")
        }
    }
}

enum Request {
    Abort,
    State,
    Commands,
    Capabilities,
    List(usize, bool),
    Cancel(usize, String),
}

impl Request {
    fn method(&self) -> &'static str {
        match self {
            Self::Abort => "abort",
            Self::State => "get_state",
            Self::Commands => "get_commands",
            Self::Capabilities => "capabilities",
            Self::List(0, _) => "list_subagents",
            Self::List(_, _) => "list_jobs",
            Self::Cancel(0, _) => "cancel_subagent",
            Self::Cancel(_, _) => "cancel_job",
        }
    }
    fn desktop(&self) -> bool {
        matches!(
            self,
            Self::Capabilities | Self::List(..) | Self::Cancel(_, _)
        )
    }
}

struct Attempt {
    deadline: Instant,
    root_idle: bool,
    root_settled: bool,
    final_idle: bool,
    state_checks_final: bool,
    abort_replied: bool,
    next_state: Instant,
    work: [Work; 2],
    pending: BTreeMap<String, Request>,
}

type Update = (Vec<Value>, Option<(String, bool)>);

#[derive(Default)]
pub(in super::super) struct Stop {
    sequence: u64,
    unconfirmed: [BTreeSet<String>; 2],
    attempt: Option<Attempt>,
    limited: bool,
    clean: bool,
    handled: bool,
}

impl Stop {
    pub(in super::super) fn write_failed(&mut self, now: Instant) -> Update {
        if let Some(a) = &mut self.attempt {
            a.deadline = now;
        }
        self.tick(now)
    }
    pub(in super::super) fn active(&self) -> bool {
        self.attempt.is_some()
    }
    pub(in super::super) fn confirmed_all(&self) -> bool {
        self.clean
    }
    pub(in super::super) fn handled(&mut self) {
        self.handled = true;
    }
    pub(in super::super) fn follow_up(&mut self) {
        self.limited = !self.clean;
        self.unconfirmed = Default::default();
        self.clean = false;
        self.handled = false;
    }
    pub(in super::super) fn begin(&mut self, now: Instant) -> Update {
        if self.active() {
            return (vec![], None);
        }
        self.clean = false;
        let mut work = [Work::new(Group::Helpers, now), Work::new(Group::Jobs, now)];
        for (group, prior) in work.iter_mut().zip(std::mem::take(&mut self.unconfirmed)) {
            group.prior = prior;
        }
        if self.limited {
            for group in &mut work {
                group.unavailable();
            }
        }
        self.attempt = Some(Attempt {
            deadline: now + TIMEOUT,
            root_idle: false,
            root_settled: false,
            final_idle: false,
            state_checks_final: false,
            abort_replied: false,
            next_state: now,
            work,
            pending: BTreeMap::new(),
        });
        let mut commands = vec![self.request(Request::Abort)];
        if !self.limited {
            commands.push(self.request(Request::Commands));
        }
        (commands, None)
    }
    fn request(&mut self, req: Request) -> Value {
        self.sequence += 1;
        let id = format!("{PREFIX}{}", self.sequence);
        let command = if req.desktop() {
            let params = if let Request::Cancel(_, item) = &req {
                json!({"id":item})
            } else {
                json!({})
            };
            let inner =
                json!({"schema":SCHEMA,"requestId":id,"method":req.method(),"params":params});
            json!({"id":id,"type":"prompt","message":format!("/gentle:desktop {inner}")})
        } else {
            json!({"id":id,"type":req.method()})
        };
        self.attempt
            .as_mut()
            .expect("active stop")
            .pending
            .insert(id, req);
        command
    }
    pub(in super::super) fn observe(&mut self, event: &Value, now: Instant) -> Update {
        let Some(a) = &mut self.attempt else {
            return (vec![], None);
        };
        if now >= a.deadline {
            return self.tick(now);
        }
        if event["type"] == "agent_settled" {
            a.root_settled = true;
            return self.tick(now);
        }
        let desktop = event["schema"] == SCHEMA && event["kind"] == "response";
        let Some(id) = event[if desktop { "requestId" } else { "id" }].as_str() else {
            return (vec![], None);
        };
        let Some(req) = a.pending.get(id) else {
            return (vec![], None);
        };
        if !desktop {
            if event["type"] != "response" {
                return (vec![], None);
            }
            if req.desktop() {
                // The outer prompt receipt carries no domain-level result.
                if event["command"] != "prompt" || event["success"] != false {
                    return (vec![], None);
                }
            } else if event["command"] != req.method() {
                return (vec![], None);
            }
        } else if !req.desktop() {
            return (vec![], None);
        }
        let req = a.pending.remove(id).expect("correlated request");
        let ok = event[if desktop { "ok" } else { "success" }] == true;
        let data = &event["data"];
        let mut follow = None;
        match req {
            Request::Abort => {
                a.abort_replied = true;
            }
            Request::State => {
                a.root_idle = ok
                    && data["isStreaming"] == false
                    && data["isCompacting"] == false
                    && data["pendingMessageCount"].as_u64() == Some(0);
                a.final_idle = a.root_idle && a.state_checks_final;
                a.next_state = now + POLL;
            }
            Request::Commands => {
                if ok
                    && data["commands"].as_array().is_some_and(|commands| {
                        commands
                            .iter()
                            .any(|c| c["name"] == "gentle:desktop" && c["source"] == "extension")
                    })
                {
                    follow = Some(Request::Capabilities);
                } else {
                    for group in &mut a.work {
                        group.unavailable();
                    }
                }
            }
            Request::Capabilities => {
                for group in &mut a.work {
                    let (list, cancel) = group.group.methods();
                    if ok
                        && data["methods"].as_array().is_some_and(|methods| {
                            methods.iter().any(|m| m == list) && methods.iter().any(|m| m == cancel)
                        })
                    {
                        group.ready = true;
                    } else {
                        group.unavailable();
                    }
                }
            }
            Request::List(index, requested_while_idle) => {
                if ok {
                    a.work[index].list(data, a.root_idle && requested_while_idle, now);
                } else {
                    a.work[index].unavailable();
                }
            }
            Request::Cancel(index, item) => {
                a.work[index].cancel_reply(&item, data);
            }
        }
        let extra = follow.map(|req| self.request(req));
        let (mut next, outcome) = self.tick(now);
        next.extend(extra);
        (next, outcome)
    }
    pub(in super::super) fn tick(&mut self, now: Instant) -> Update {
        let Some(a) = &mut self.attempt else {
            return (vec![], None);
        };
        if now >= a.deadline || a.final_idle {
            let a = self.attempt.take().expect("active stop");
            let root = if a.final_idle && self.handled {
                "Gentle reports an idle conversation. Completion of the extension command is unverified."
            } else if a.final_idle && a.root_settled {
                "The current turn ended."
            } else if a.final_idle {
                "Gentle reports no active turn."
            } else {
                "Could not confirm that the turn ended."
            };
            self.clean =
                a.final_idle && !self.handled && a.work.iter().all(|w| w.done && !w.partial);
            let message = if self.limited {
                format!(
                    "{root} Earlier assignments share this conversation. Their helpers and jobs were left alone."
                )
            } else {
                format!(
                    "{root} {} {}",
                    a.work[0].summary("Helpers"),
                    a.work[1].summary("Jobs")
                )
            };
            // A retry can receive an empty recent list after an unresolved item vanished.
            // Carry those exact identities forward until the owner actually reports an ending.
            self.unconfirmed = a.work.map(|group| {
                let mut missing = group.prior;
                missing.extend(
                    group
                        .attempted
                        .into_iter()
                        .filter_map(|(id, ended)| (!ended).then_some(id)),
                );
                missing
            });
            return (vec![], Some((message, a.final_idle)));
        }
        // Reserve time for one independent root-state check when a desktop owner stays silent.
        if a.deadline.saturating_duration_since(now) < Duration::from_secs(2) {
            for work in &mut a.work {
                if !work.done {
                    work.unavailable();
                }
            }
        }
        let mut todo = vec![];
        let final_check = a.work.iter().all(|w| w.done);
        if a.abort_replied
            && (!a.root_idle || final_check)
            && now >= a.next_state
            && !a.pending.values().any(|p| matches!(p, Request::State))
        {
            a.state_checks_final = final_check;
            todo.push(Request::State);
        }
        for (index, group) in a.work.iter_mut().enumerate() {
            if !group.ready
                || group.done
                || a.pending
                    .values()
                    .any(|p| matches!(p,Request::List(i, _)|Request::Cancel(i,_) if *i==index))
            {
                continue;
            }
            if let Some(id) = group.waiting.pop() {
                todo.push(Request::Cancel(index, id));
            } else if now >= group.next_read {
                todo.push(Request::List(index, a.root_idle));
            }
        }
        (todo.into_iter().map(|r| self.request(r)).collect(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn physical(command: &Value, data: Value) -> Value {
        json!({"type":"response","id":command["id"],"command":command["type"],"success":true,"data":data})
    }

    fn inner(command: &Value) -> Value {
        serde_json::from_str(
            command["message"]
                .as_str()
                .unwrap()
                .strip_prefix("/gentle:desktop ")
                .unwrap(),
        )
        .unwrap()
    }

    fn response(request: &Value, data: Value) -> Value {
        json!({"schema":SCHEMA,"kind":"response","requestId":request["requestId"],"ok":true,"data":data})
    }

    #[test]
    fn a_running_cancel_receipt_and_a_foreign_terminal_receipt_cannot_finish_the_stop() {
        let mut stop = Stop::default();
        let mut now = Instant::now();
        let mut commands = VecDeque::from(stop.begin(now).0);
        let mut cancels = 0;
        let mut states = 0;
        let mut helper_lists = 0;
        let (message, resumable) = loop {
            if commands.is_empty() {
                now += POLL;
                let (more, result) = stop.tick(now);
                commands.extend(more);
                if let Some(result) = result {
                    break result;
                }
                continue;
            }
            let command = commands.pop_front().unwrap();
            let event = match command["type"].as_str().unwrap() {
                "abort" => physical(&command, Value::Null),
                "get_state" => {
                    states += 1;
                    physical(
                        &command,
                        json!({"isStreaming":false,"isCompacting":false,"pendingMessageCount":0}),
                    )
                }
                "get_commands" => physical(
                    &command,
                    json!({"commands":[{"source":"extension","name":"gentle:desktop"}]}),
                ),
                "prompt" => {
                    let request = inner(&command);
                    let data = match request["method"].as_str().unwrap() {
                        "capabilities" => {
                            json!({"methods":["list_subagents","cancel_subagent","list_jobs","cancel_job"]})
                        }
                        "list_jobs" => json!({"jobs":[]}),
                        "list_subagents" => {
                            helper_lists += 1;
                            let status = if helper_lists >= 3 {
                                "cancelled"
                            } else {
                                "running"
                            };
                            json!({"agents":[{"id":"exact","status":status}]})
                        }
                        "cancel_subagent" => {
                            cancels += 1;
                            assert_eq!(request["params"], json!({"id":"exact"}));
                            let mut wrong = response(
                                &request,
                                json!({"agent":{"id":"exact","status":"cancelled"}}),
                            );
                            wrong["requestId"] = "ivy-stop-stale".into();
                            assert!(stop.observe(&wrong, now).1.is_none());
                            assert!(
                                stop.observe(
                                    &physical(&command, json!({"disposition":"handled"})),
                                    now
                                )
                                .1
                                .is_none()
                            );
                            json!({"agent":{"id":"exact","status":"running"}})
                        }
                        other => panic!("unexpected desktop method {other}"),
                    };
                    response(&request, data)
                }
                other => panic!("unexpected Pi command {other}"),
            };
            let (more, result) = stop.observe(&event, now);
            commands.extend(more);
            if let Some(result) = result {
                break result;
            }
        };
        assert!(resumable && states >= 2 && helper_lists >= 3);
        assert!(message.contains("Helpers: 1 confirmed ended."));
        assert_eq!(cancels, 1);
        stop.follow_up();
        let first = stop.begin(now).0;
        assert!(first.iter().any(|c| c["type"] == "get_commands"));
    }

    #[test]
    fn absent_or_prompt_defined_desktop_command_never_injects_a_control_prompt() {
        let mut stop = Stop::default();
        let now = Instant::now();
        let commands = stop.begin(now).0;
        let discovery = commands
            .iter()
            .find(|r| r["type"] == "get_commands")
            .unwrap();
        let (next, _) = stop.observe(
            &physical(
                discovery,
                json!({"commands":[{"source":"prompt","name":"gentle:desktop"}]}),
            ),
            now,
        );
        assert!(next.is_empty());
        let ack = commands.iter().find(|r| r["type"] == "abort").unwrap();
        let state = stop
            .observe(&physical(ack, Value::Null), now)
            .0
            .pop()
            .unwrap();
        let (_, result) = stop.observe(
            &physical(
                &state,
                json!({"isStreaming":false,"isCompacting":false,"pendingMessageCount":0}),
            ),
            now,
        );
        let (message, ready) = result.unwrap();
        assert!(ready && message.contains("verification incomplete"));
        stop.follow_up();
        assert_eq!(stop.begin(now).0.len(), 1);
    }

    #[test]
    fn retry_keeps_a_missing_id_unverified_until_its_owner_reports_a_terminal_state() {
        let now = Instant::now();
        let mut stop = Stop::default();
        stop.begin(now);
        let initial = &mut stop.attempt.as_mut().unwrap().work[0];
        initial.list(
            &json!({"agents":[{"id":"original","status":"running"}]}),
            true,
            now,
        );
        assert!(!stop.write_failed(now).1.unwrap().1);
        stop.begin(now);
        let retry = &mut stop.attempt.as_mut().unwrap().work[0];
        retry.list(&json!({"agents":[]}), true, now);
        assert!(!retry.done && retry.waiting.is_empty());
        retry.list(
            &json!({"agents":[{"id":"original","status":"cancelled"}]}),
            true,
            now,
        );
        assert!(retry.done && retry.summary("Helpers").contains("1 confirmed ended"));
    }

    #[test]
    fn bounded_lists_and_missing_previously_seen_ids_never_imply_complete_coverage() {
        let now = Instant::now();
        let mut group = Work::new(Group::Jobs, now);
        let all: Vec<_> = (0..LIMIT)
            .map(|i| json!({"id":format!("id-{i}"),"status":"running"}))
            .collect();
        group.list(&json!({"jobs":all}), true, now);
        assert!(group.partial);
        group.waiting.clear();
        group.list(&json!({"jobs":[]}), true, now);
        assert!(!group.done);
        assert!(group.summary("Jobs").contains("verification incomplete"));
        group.cancel_reply("id-1", &json!({"job":{"id":"id-2","status":"stopped"}}));
        assert_eq!(group.attempted.get("id-1"), Some(&false));
    }
}
