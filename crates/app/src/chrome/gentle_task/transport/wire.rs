//! Gentle interactive RPC uses newline-delimited JSON on one process's pipes.
use super::helpers;
use super::{Answer, Inbox, Kind, MAX_TEXT, Message, Question};
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::time::{Duration, Instant};

mod stop;
pub(super) use stop::Stop;

const MAX_LINE: usize = 2 * 1024 * 1024;
pub(super) const PROMPT_ID: &str = "ivy-initial-task";

pub(super) enum Incoming {
    PromptResponse {
        id: String,
        handled: bool,
        success: bool,
    },
    Question(Question),
    Completed,
    Aborted,
    Rejected(String),
    StopControl(Value),
    HelperControl(Value),
    HelperReset,
    CommandError,
}

#[cfg(test)]
pub(super) fn receive(
    stdout: impl Read,
    target: &Inbox,
    updates: &impl Fn(Option<Value>),
) -> Result<(), String> {
    receive_projected(
        stdout,
        target,
        updates,
        &super::conversation::Display::new(std::sync::Arc::new(|| {})),
    )
}

pub(super) fn receive_projected(
    stdout: impl Read,
    target: &Inbox,
    updates: &impl Fn(Option<Value>),
    display: &super::conversation::Display,
) -> Result<(), String> {
    let mut stream = BufReader::new(stdout);
    let mut line = Vec::new();
    let mut assistant_failed = false;
    let mut channel = DesktopChannel::default();
    loop {
        line.clear();
        let len = stream
            .by_ref()
            .take((MAX_LINE + 1) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|_| "Could not read the Gentle connection.")?;
        if len == 0 {
            return Err("The Gentle connection closed.".into());
        }
        if len > MAX_LINE || line.last() != Some(&b'\n') {
            return Err("Gentle sent an incomplete or oversized RPC event.".into());
        }
        let projection: Value =
            serde_json::from_slice(&line).map_err(|_| "Gentle sent an invalid RPC event.")?;
        display.observe(&projection);
        let mut reset = false;
        let event = decode_record(
            &line,
            &mut assistant_failed,
            &mut channel,
            updates,
            &mut reset,
        )?;
        if reset
            && target
                .try_send(Message::Incoming(Incoming::HelperReset))
                .is_err()
        {
            return Err("Gentle reader control backlog exceeded its limit or closed.".into());
        }
        if let Some(event) = event
            && target.try_send(Message::Incoming(event)).is_err()
        {
            return Err("Gentle reader control backlog exceeded its limit or closed.".into());
        }
    }
}

#[cfg(test)]
fn decode(line: &[u8], assistant_failed: &mut bool) -> Result<Option<Incoming>, String> {
    decode_record(
        line,
        assistant_failed,
        &mut DesktopChannel::default(),
        &|_| {},
        &mut false,
    )
}

#[derive(Default)]
struct DesktopChannel {
    sequence: u64,
    switching: bool,
}

fn decode_record(
    line: &[u8],
    assistant_failed: &mut bool,
    channel: &mut DesktopChannel,
    updates: &impl Fn(Option<Value>),
    reset: &mut bool,
) -> Result<Option<Incoming>, String> {
    let v: Value = serde_json::from_slice(line).map_err(|_| "Gentle sent an invalid RPC event.")?;
    match v["type"]
        .as_str()
        .ok_or("Gentle sent an untyped RPC event.")?
    {
        "extension_ui_request"
            if v["method"] == "setWidget" && v["widgetKey"] == "gentle-desktop-rpc" =>
        {
            let Some(lines) = v["widgetLines"].as_array() else {
                return Ok(None);
            };
            if let [line] = lines.as_slice()
                && let Some(line) = line.as_str()
                && let Ok(envelope) = serde_json::from_str::<Value>(line)
                && envelope["schema"] == stop::SCHEMA
            {
                if envelope["kind"] == "event" {
                    if let Some(seq) = envelope["seq"].as_u64().filter(|&seq| seq > 0) {
                        if seq < channel.sequence {
                            updates(None);
                            *reset = true;
                        }
                        channel.sequence = seq;
                    }
                    if envelope["event"] == "agents_update" {
                        updates(Some(envelope));
                    }
                } else if envelope["kind"] == "response" {
                    let changed = envelope["ok"] == true
                        && envelope["data"]["changed"] == true
                        && envelope["data"]["id"].is_string();
                    let switching =
                        envelope["ok"] == false && envelope["error"]["code"] == "session_switching";
                    if changed || (switching && !channel.switching) {
                        updates(None);
                        channel.sequence = 0;
                        *reset = true;
                    }
                    channel.switching = switching;
                    if helpers::our_id(&envelope["requestId"]) {
                        return Ok(Some(Incoming::HelperControl(envelope)));
                    }
                    if line.len() <= 128 * 1024 && stop::our_id(&envelope["requestId"]) {
                        return Ok(Some(Incoming::StopControl(envelope)));
                    }
                }
            }
            Ok(None)
        }
        "response" if helpers::our_id(&v["id"]) => Ok(Some(Incoming::HelperControl(v))),
        "response" if stop::our_id(&v["id"]) => Ok(Some(Incoming::StopControl(v))),
        "extension_ui_request" => dialog(&v).map(|q| q.map(Incoming::Question)),
        "extension_error" if v["event"] == "command" => Ok(Some(Incoming::CommandError)),
        "agent_settled" if v["aborted"] == true => {
            *assistant_failed = false;
            Ok(Some(Incoming::Aborted))
        }
        "agent_settled" if *assistant_failed => {
            *assistant_failed = false;
            Ok(Some(Incoming::Rejected(
                "The Gentle provider could not finish the task.".into(),
            )))
        }
        "agent_settled" => Ok(Some(Incoming::Completed)),
        "response"
            if v["command"] == "prompt"
                && v["id"]
                    .as_str()
                    .is_some_and(|id| id == PROMPT_ID || id.starts_with("ivy-follow-up-")) =>
        {
            Ok(Some(Incoming::PromptResponse {
                id: v["id"].as_str().expect("matched prompt id").into(),
                success: v["success"] == true,
                handled: v["data"]["disposition"] == "handled",
            }))
        }
        "message_end" if v["message"]["role"] == "assistant" => {
            *assistant_failed = matches!(
                v["message"]["stopReason"].as_str(),
                Some("error" | "aborted")
            );
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn dialog(v: &Value) -> Result<Option<Question>, String> {
    let method = v["method"]
        .as_str()
        .ok_or("Gentle sent a dialog without a method.")?;
    if matches!(
        method,
        "notify" | "setStatus" | "setWidget" | "setTitle" | "set_editor_text"
    ) {
        return Ok(None);
    }
    let id = v["id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .ok_or("Gentle sent an invalid dialog identifier.")?;
    let title = v["title"]
        .as_str()
        .ok_or("Gentle sent a dialog without a title.")?;
    let optional = |key: &str| match v.get(key) {
        None => Ok(""),
        Some(value) => value.as_str().ok_or("Gentle sent an invalid dialog field."),
    };
    let mut size = title.len();
    let kind = match method {
        "select" => {
            let options = v["options"]
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 128)
                .ok_or("Gentle sent an invalid choice list.")?;
            let mut choices = Vec::with_capacity(options.len());
            for option in options {
                let text = option.as_str().ok_or("Gentle sent a non-text choice.")?;
                size += text.len();
                choices.push(text.to_owned());
            }
            Kind::Select(choices)
        }
        "confirm" | "input" | "editor" => {
            let text = match method {
                "confirm" => optional("message")?,
                "input" => optional("placeholder")?,
                _ => optional("prefill")?,
            };
            size += text.len();
            match method {
                "confirm" => Kind::Confirm(text.into()),
                "input" => Kind::Input(text.into()),
                _ => Kind::Editor(text.into()),
            }
        }
        _ => return Err("This Gentle version requested an unsupported interaction.".into()),
    };
    if size > MAX_TEXT {
        return Err("Gentle sent an oversized question.".into());
    }
    let deadline = match v.get("timeout") {
        Some(value) => {
            let ms = value
                .as_u64()
                .filter(|ms| *ms <= i32::MAX as u64)
                .ok_or("Gentle sent an invalid dialog timeout.")?;
            (ms != 0).then(|| Instant::now() + Duration::from_millis(ms))
        }
        None => None,
    };
    Ok(Some(Question {
        id: id.into(),
        title: title.into(),
        kind,
        deadline,
    }))
}

pub(super) fn write(out: &mut impl Write, v: Value) -> io::Result<()> {
    let mut data = serde_json::to_vec(&v)?;
    data.push(b'\n');
    out.write_all(&data)?;
    out.flush()
}

pub(super) fn reply(id: &str, answer: Answer) -> Value {
    let mut result = json!({"type":"extension_ui_response","id":id});
    match answer {
        Answer::Cancel => result["cancelled"] = true.into(),
        Answer::Confirm(choice) => result["confirmed"] = choice.into(),
        Answer::Value(value) => result["value"] = value.into(),
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reply_shapes_preserve_literals_and_never_turn_answers_into_commands() {
        let literal = "quote\" and newline\n";
        let mut out = Vec::new();
        write(&mut out, reply("same-id", Answer::Value(literal.into()))).unwrap();
        assert_eq!(out.iter().filter(|b| **b == b'\n').count(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&out).unwrap(),
            json!({"type":"extension_ui_response","id":"same-id","value":literal})
        );
        assert_eq!(reply("confirm", Answer::Confirm(false))["confirmed"], false);
        assert_eq!(reply("cancel", Answer::Cancel)["cancelled"], true);
    }

    #[test]
    fn a_retry_can_recover_but_a_failed_final_response_is_not_success() {
        let failure =
            br#"{"type":"message_end","message":{"role":"assistant","stopReason":"error"}}"#;
        let recovered =
            br#"{"type":"message_end","message":{"role":"assistant","stopReason":"stop"}}"#;
        let settled = br#"{"type":"agent_settled","aborted":false}"#;
        let mut failed = false;
        assert!(decode(failure, &mut failed).unwrap().is_none());
        assert!(matches!(
            decode(settled, &mut failed),
            Ok(Some(Incoming::Rejected(_)))
        ));
        assert!(decode(recovered, &mut failed).unwrap().is_none());
        assert!(matches!(
            decode(settled, &mut failed),
            Ok(Some(Incoming::Completed))
        ));
    }

    #[test]
    fn only_a_correlated_desktop_widget_carries_control_data() {
        let record =
            json!({"schema":stop::SCHEMA,"kind":"response","requestId":"ivy-stop-17","ok":true});
        let mut envelope = json!({"type":"extension_ui_request","method":"setWidget","widgetKey":"gentle-desktop-rpc","widgetLines":[record.to_string()]});
        let mut failed = false;
        assert!(
            matches!(decode(&serde_json::to_vec(&envelope).unwrap(), &mut failed), Ok(Some(Incoming::StopControl(v))) if v == record)
        );
        envelope["widgetKey"] = "transcript".into();
        assert!(
            decode(&serde_json::to_vec(&envelope).unwrap(), &mut failed)
                .unwrap()
                .is_none()
        );
        envelope["widgetKey"] = "gentle-desktop-rpc".into();
        envelope["widgetLines"] = json!([
            json!({"schema":stop::SCHEMA,"requestId":"another-host","kind":"response"}).to_string()
        ]);
        assert!(
            decode(&serde_json::to_vec(&envelope).unwrap(), &mut failed)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn a_native_session_reset_separates_queued_pushes_before_rediscovery() {
        use std::cell::RefCell;
        let channel = &mut DesktopChannel::default();
        let callback = RefCell::new(Vec::new());
        let mut record = |inner: Value| {
            let outer = json!({"type":"extension_ui_request","method":"setWidget","widgetKey":"gentle-desktop-rpc","widgetLines":[inner.to_string()]});
            let mut reset = false;
            let _ = decode_record(
                &serde_json::to_vec(&outer).unwrap(),
                &mut false,
                channel,
                &|update| {
                    callback
                        .borrow_mut()
                        .push(update.map(|v| v["seq"].as_u64().unwrap()));
                },
                &mut reset,
            )
            .unwrap();
            reset
        };
        let event =
            |name, seq| json!({"schema":stop::SCHEMA,"kind":"event","event":name,"seq":seq});
        assert!(!record(event("agents_update", 4)));
        assert!(!record(event("session_snapshot", 9)));
        assert!(record(event("agents_update", 1)));
        assert_eq!(&*callback.borrow(), &[Some(4), None, Some(1)]);
        let switching = json!({"schema":stop::SCHEMA,"kind":"response","ok":false,"error":{"code":"session_switching"}});
        assert!(record(switching.clone()));
        assert!(!record(switching));
        assert!(record(
            json!({"schema":stop::SCHEMA,"kind":"response","ok":true,"data":{"changed":true,"id":"new-session"}})
        ));
    }

    #[test]
    fn oversized_or_partial_wire_records_fail_closed() {
        let (inbox, _rx) = std::sync::mpsc::sync_channel(1);
        let big = Cursor::new(vec![b'x'; MAX_LINE + 4]);
        assert!(
            receive(big, &inbox, &|_| {})
                .unwrap_err()
                .contains("oversized")
        );
        assert!(
            receive(Cursor::new(b"{}"), &inbox, &|_| {})
                .unwrap_err()
                .contains("incomplete")
        );
        assert!(
            dialog(
                &json!({"method":"select","title":"q","id":"1","options":["x".repeat(MAX_TEXT)]})
            )
            .is_err()
        );
    }
    #[test]
    fn text_flood_bypasses_control_capacity_and_question_still_arrives() {
        let display = super::super::conversation::Display::new(std::sync::Arc::new(|| {}));
        let mut bytes = Vec::new();
        for _ in 0..10000 {
            bytes.extend_from_slice(b"{\"type\":\"message_update\",\"assistantMessageEvent\":{\"type\":\"text_delta\",\"delta\":\"x\",\"contentIndex\":0}}\n");
        }
        bytes.extend_from_slice(b"{\"type\":\"extension_ui_request\",\"method\":\"confirm\",\"id\":\"q\",\"title\":\"Continue?\"}\n");
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let error = receive_projected(Cursor::new(bytes), &send, &|_| {}, &display).unwrap_err();
        assert!(error.contains("closed"));
        assert!(
            matches!(receive.try_recv(), Ok(Message::Incoming(Incoming::Question(q))) if q.id == "q")
        );
        let mut cache = super::super::conversation::Cache::default();
        while display.poll(&mut cache, &mut (64 * 1024)) {}
        assert_eq!(
            cache.rows.iter().map(|r| &*r.text).collect::<String>(),
            "x".repeat(10000)
        );
        assert_eq!(cache.rows.len(), 1);
    }
    #[test]
    fn saturated_reader_control_admission_fails_without_waiting_for_a_receiver() {
        let (send, _receive) = std::sync::mpsc::sync_channel(1);
        let record = b"{\"type\":\"agent_settled\",\"aborted\":false}\n";
        let mut bytes = record.to_vec();
        bytes.extend_from_slice(record);
        assert!(
            receive(Cursor::new(bytes), &send, &|_| {})
                .unwrap_err()
                .contains("backlog")
        );
    }
}
