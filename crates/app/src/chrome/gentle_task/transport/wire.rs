//! Gentle interactive RPC uses newline-delimited JSON on one process's pipes.
use super::{Answer, Inbox, Kind, MAX_TEXT, Message, Question};
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::time::{Duration, Instant};

const MAX_LINE: usize = 2 * 1024 * 1024;
pub(super) const PROMPT_ID: &str = "ivy-initial-task";

pub(super) enum Incoming {
    Handled,
    Question(Question),
    Completed,
}

pub(super) fn receive(stdout: impl Read, target: &Inbox) -> Result<(), String> {
    let mut stream = BufReader::new(stdout);
    let mut line = Vec::new();
    let mut assistant_failed = false;
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
        if let Some(event) = decode(&line, &mut assistant_failed)?
            && target.send(Message::Incoming(event)).is_err()
        {
            return Ok(());
        }
    }
}

fn decode(line: &[u8], assistant_failed: &mut bool) -> Result<Option<Incoming>, String> {
    let v: Value = serde_json::from_slice(line).map_err(|_| "Gentle sent an invalid RPC event.")?;
    match v["type"]
        .as_str()
        .ok_or("Gentle sent an untyped RPC event.")?
    {
        "extension_ui_request" => dialog(&v).map(|q| q.map(Incoming::Question)),
        "extension_error" if v["event"] == "command" => {
            Err("Gentle could not run the task command.".into())
        }
        "agent_settled" if v["aborted"] == true => Err("Gentle aborted the task.".into()),
        "agent_settled" if *assistant_failed => {
            Err("The Gentle provider could not finish the task.".into())
        }
        "agent_settled" => Ok(Some(Incoming::Completed)),
        "response" if v["id"] == PROMPT_ID && v["command"] == "prompt" => {
            if v["success"] != true {
                return Err("Gentle rejected the task.".into());
            }
            Ok((v["data"]["disposition"] == "handled").then_some(Incoming::Handled))
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
        assert!(decode(settled, &mut failed).is_err());
        assert!(decode(recovered, &mut failed).unwrap().is_none());
        assert!(matches!(
            decode(settled, &mut failed),
            Ok(Some(Incoming::Completed))
        ));
    }

    #[test]
    fn oversized_or_partial_wire_records_fail_closed() {
        let (inbox, _rx) = std::sync::mpsc::sync_channel(1);
        let big = Cursor::new(vec![b'x'; MAX_LINE + 4]);
        assert!(receive(big, &inbox).unwrap_err().contains("oversized"));
        assert!(
            receive(Cursor::new(b"{}"), &inbox)
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
}
