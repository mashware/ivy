//! The bridge an agent launches: MCP on its standard input and output, this pane's channel on the
//! other side.
//!
//! **It is a binary of the channel's crate and not a crate of its own**, and that was decided
//! against a real alternative: the day the same channel is spoken over something else — the roadmap
//! already names an escape sequence as a candidate after F2.7 — the MCP vocabulary is the piece that
//! goes and the channel is the piece that stays, which is an argument for two crates. It is not this
//! ticket's day, and a second workspace member costs entries in six guard lists this delivery
//! already has to pay.
//!
//! **It exposes `panel_set` and nothing else.** The three `split_*` verbs are part of the same
//! frozen wire format and arrive with the scoping that makes them safe to answer; a tool advertised
//! here before then would be a promise taken back later.
//!
//! **Which pane it drives is not a parameter and cannot be one.** The two variables the program
//! inherited name its own pane's directory, the endpoint inside it is what the connection opens, and
//! no message carries an identifier for anybody to trust.

use std::io::{BufRead, Read, Write};

use panel_control::{Answered, MAX_REQUEST, Producer};

/// The version answered when the client does not ask for one.
///
/// **What a client asks for is echoed back when it asks**, which is unusual enough to say why: this
/// server has no behaviour that varies with the version — it advertises one tool and hands a JSON
/// document straight through — so refusing a client over a date would break the connection for a
/// difference that does not exist anywhere in this file.
const PROTOCOL_VERSION: &str = "2025-06-18";

fn main() {
    // **Opened lazily, on the first call that needs it.** A client that only lists the tools must
    // not fail because nothing is listening yet, and an agent's editor lists tools long before it
    // has anything to publish.
    let mut pane: Option<Producer> = None;

    bridge(std::io::stdin().lock(), std::io::stdout(), &mut pane);
}

/// Answers messages until the input ends.
///
/// **It takes its two ends rather than reaching for the process's**, so what it does with a message
/// it cannot decode can be asserted. Without that the only way to find out was to run the binary,
/// which is exactly why the decoding bug below went in unnoticed.
fn bridge(mut input: impl BufRead, mut output: impl Write, pane: &mut Option<Producer>) {
    // **The input is taken as bytes and decoded per line, rather than by a decoding iterator.**
    // Reading it as text makes one malformed byte end the whole session: the iterator yields an
    // error the loop cannot answer, so a client that garbled a single message would be left waiting
    // for ever on every message after it too. As bytes, the bad line is the only casualty and it
    // leaves with the parse error the protocol has for exactly this.
    let mut line = Vec::new();
    loop {
        line.clear();
        // **Bounded, because the input is another process's bytes.** `read_until` on its own grew
        // the buffer without limit for a client that sent no newline; here at most one request line
        // and a byte are held. A read that failed is named, kind and errno (D38), rather than read
        // as end of input — the agent's session would end with no diagnosis.
        let read = input
            .by_ref()
            .take(MAX_REQUEST as u64 + 1)
            .read_until(b'\n', &mut line);
        match read {
            Ok(0) => break,
            Ok(_) => {}
            Err(failure) => {
                eprintln!(
                    "panel-mcp: reading a message failed ({}{})",
                    failure.kind(),
                    failure
                        .raw_os_error()
                        .map_or(String::new(), |number| format!(", errno {number}"))
                );
                break;
            }
        }
        // **The delimiter is not part of the message.** Counting it flagged a message of exactly
        // `MAX_REQUEST` bytes as over, and then the swallow ate the *next* message, which the client
        // was left waiting on. The tail is only swallowed when the newline was not already read.
        let ended = line.last() == Some(&b'\n');
        let content = line.len() - usize::from(ended);
        if content > MAX_REQUEST {
            // **Answered, and the rest of the line swallowed in bounded chunks**, so what follows is
            // a fresh message and not its tail.
            let answered = failed(
                &serde_json::Value::Null,
                -32600,
                "the message goes over the ceiling",
            );
            if let Err(failure) = writeln!(output, "{answered}").and_then(|()| output.flush()) {
                eprintln!(
                    "panel-mcp: answering a message failed ({}{})",
                    failure.kind(),
                    failure
                        .raw_os_error()
                        .map_or(String::new(), |number| format!(", errno {number}"))
                );
                break;
            }
            if !ended && let Err(failure) = swallow_to_newline(&mut input) {
                eprintln!(
                    "panel-mcp: swallowing an over-long line failed ({}{})",
                    failure.kind(),
                    failure
                        .raw_os_error()
                        .map_or(String::new(), |number| format!(", errno {number}"))
                );
                break;
            }
            continue;
        }
        let answered = match std::str::from_utf8(&line) {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => answer(line, pane),
            Err(failure) => Some(failed(
                &serde_json::Value::Null,
                -32700,
                &format!("the message is not UTF-8 ({failure})"),
            )),
        };
        let Some(answered) = answered else {
            continue;
        };
        if writeln!(output, "{answered}").is_err() || output.flush().is_err() {
            break;
        }
    }
}

/// Reads and throws away the rest of an over-long line, **in bounded chunks and stopping exactly at
/// the newline**.
///
/// Without it the tail of the refused line would be parsed as the next message, which is
/// resynchronising onto a boundary that was never seen. A plain block read cannot be used: it takes
/// the bytes that follow the newline with it and eats the next message. `read_until` stops on the
/// delimiter, and the `take` bounds each round, so a tail with no newline cannot grow without limit.
fn swallow_to_newline(input: &mut impl BufRead) -> std::io::Result<()> {
    let mut scratch = Vec::new();
    loop {
        scratch.clear();
        let read = input
            .by_ref()
            .take(MAX_REQUEST as u64 + 1)
            .read_until(b'\n', &mut scratch)?;
        if read == 0 || scratch.last() == Some(&b'\n') {
            return Ok(());
        }
    }
}

/// Answers one message, or nothing when the message is a notification.
fn answer(line: &str, pane: &mut Option<Producer>) -> Option<String> {
    let message: serde_json::Value = match serde_json::from_str(line) {
        Ok(message) => message,
        // **Parse errors are answered without an id**, which is what the protocol asks for and what
        // keeps a client from waiting for ever on a message it garbled.
        Err(failure) => {
            return Some(failed(
                &serde_json::Value::Null,
                -32700,
                &failure.to_string(),
            ));
        }
    };
    // **A message with no id is a notification: it is ignored and never answered.** This server has
    // no notification it acts on, and answering one would be a message with no request behind it,
    // which a strict client closes the connection over.
    let id = message.get("id")?.clone();

    // **An id that arrived commits this server to answering it**, and the two `?` above and below
    // used to be the same gesture for two different things: a message with an id and no readable
    // method left through the notification door, so the client that sent it waited on that id for
    // ever. Silence is the one answer the protocol has no recovery from.
    let Some(method) = message.get("method").and_then(serde_json::Value::as_str) else {
        return Some(failed(&id, -32600, "the message carries no method"));
    };

    Some(match method {
        "initialize" => succeeded(&id, initialized(&message)),
        "tools/list" => succeeded(&id, tools()),
        "tools/call" => called(&id, &message, pane),
        "ping" => succeeded(&id, serde_json::json!({})),
        other => failed(&id, -32601, &format!("no method \"{other}\"")),
    })
}

/// What this server says it is.
fn initialized(message: &serde_json::Value) -> serde_json::Value {
    let asked_for = message
        .get("params")
        .and_then(|params| params.get("protocolVersion"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);

    serde_json::json!({
        "protocolVersion": asked_for,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "agent-terminal panel", "version": env!("CARGO_PKG_VERSION")},
    })
}

/// The one tool, with the shape of what it takes.
fn tools() -> serde_json::Value {
    serde_json::json!({
        "tools": [{
            "name": "panel_set",
            "description": "Publish this pane's status panel. The document is the panel document: \
                            a required \"lines\" array, and optionally \"phase\", \"attention\" \
                            (wait, block or done), \"actions\" and the rest of the contract. \
                            Publishing claims this pane under the given source until this process \
                            exits; while it is claimed, the pane's panel.json is not read and \
                            another source is refused.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "What to call whoever is publishing, shown on the panel.",
                    },
                    "document": {
                        "type": "object",
                        "description": "The panel document.",
                    },
                },
                "required": ["source", "document"],
            },
        }],
    })
}

/// Runs a tool call.
fn called(
    id: &serde_json::Value,
    message: &serde_json::Value,
    pane: &mut Option<Producer>,
) -> String {
    let params = message.get("params").unwrap_or(&serde_json::Value::Null);
    let name = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if name != "panel_set" {
        return failed(id, -32602, &format!("no tool \"{name}\""));
    }

    let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
    let Some(source) = arguments.get("source").and_then(serde_json::Value::as_str) else {
        return succeeded(id, refusal("panel_set needs a \"source\""));
    };
    let Some(document) = arguments.get("document") else {
        return succeeded(id, refusal("panel_set needs a \"document\""));
    };

    // **A failure to reach the pane is a tool error and not a protocol one**, which is the
    // distinction the protocol draws and it matters here: the agent is meant to read it, decide the
    // panel is not available and carry on working, not to treat its whole connection as broken.
    let producer = match open(pane) {
        Ok(producer) => producer,
        Err(failure) => return succeeded(id, refusal(&failure)),
    };
    match producer.publish(source, &document.to_string()) {
        Ok(Answered::Ok) => succeeded(id, published()),
        Ok(Answered::Failed(why)) => succeeded(id, refusal(&why)),
        Err(failure) => {
            // The channel is gone; the next call opens a fresh one rather than reusing a dead
            // connection — and opening a fresh one is also what claims the pane again.
            *pane = None;
            succeeded(id, refusal(&failure.to_string()))
        }
    }
}

/// The connection to this pane, opened if it is not open yet.
fn open(pane: &mut Option<Producer>) -> Result<&mut Producer, String> {
    // **No `expect` after a fallible step.** An `Option::as_mut().expect(…)` here — even one whose
    // branch cannot be reached today — is a panic in the bridge, and a panic kills the agent's whole
    // tool session over a failure the caller is meant to read as a soft tool error. `insert` returns
    // the slot it just filled, and cannot fail.
    let producer = match pane {
        Some(producer) => return Ok(producer),
        None => Producer::of_this_pane().map_err(|failure| failure.to_string())?,
    };
    Ok(pane.insert(producer))
}

fn published() -> serde_json::Value {
    serde_json::json!({"content": [{"type": "text", "text": "published"}]})
}

fn refusal(why: &str) -> serde_json::Value {
    serde_json::json!({
        "content": [{"type": "text", "text": why}],
        "isError": true,
    })
}

fn succeeded(id: &serde_json::Value, result: serde_json::Value) -> String {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()
}

fn failed(id: &serde_json::Value, code: i32, message: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message},
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(message: &str) -> serde_json::Value {
        let mut nowhere = None;
        let answered = answer(message, &mut nowhere).expect("it deserved an answer");
        serde_json::from_str(&answered).expect("the answer is JSON")
    }

    /// **A request that carries an id is always answered, whatever else is wrong with it.** A message
    /// with an id and no readable `method` used to leave through the notification door — the same `?`
    /// serving two different decisions — and the client that sent it then waited on that id for ever.
    /// Silence is the one answer the protocol cannot recover from, which is why this is asserted
    /// rather than left to the shape of the code.
    #[test]
    fn a_request_with_an_id_and_no_method_is_answered_rather_than_taken_for_a_notification() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":7}");

        assert_eq!(answered["id"], 7);
        assert_eq!(answered["error"]["code"], -32600);
    }

    /// **One byte that is not UTF-8 must not end the session.** Decoding the input as text made the
    /// reading iterator yield an error the loop could not answer, so a client that garbled a single
    /// message was left waiting for ever on every message after it too. The assertion is on the
    /// message **after** the bad one arriving, never on the error alone: a version that answered the
    /// parse error and then stopped reading would satisfy a check on the first reply.
    #[test]
    fn a_message_that_is_not_utf8_is_answered_and_the_next_one_still_is() {
        let mut input: Vec<u8> = Vec::new();
        input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");
        input.extend_from_slice(&[0xff, 0xfe, b'\n']);
        input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n");
        let mut written = Vec::new();

        bridge(&input[..], &mut written, &mut None);

        let answers: Vec<serde_json::Value> = String::from_utf8(written)
            .expect("the answers are text")
            .lines()
            .map(|line| serde_json::from_str(line).expect("each answer is JSON"))
            .collect();
        assert_eq!(answers.len(), 3, "the session ended early: {answers:?}");
        assert_eq!(answers[1]["error"]["code"], -32700);
        assert_eq!(
            answers[2]["id"], 3,
            "the message after the bad one was never answered"
        );
    }

    /// **A line over the transport bound is answered, and its tail is swallowed so the next message
    /// is still answered.** Before the cap, `read_until` grew the buffer without limit for a client
    /// that never sent a newline — a same-user process could exhaust the bridge. The assertion is on
    /// the message **after** the over-long one, never on the error alone: swallowing nothing would
    /// answer its tail as a second message.
    #[test]
    fn a_message_over_the_ceiling_is_answered_and_the_next_one_still_is() {
        let mut input: Vec<u8> = Vec::new();
        input.extend_from_slice(&vec![b'x'; MAX_REQUEST + 10]);
        input.push(b'\n');
        input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}\n");
        let mut written = Vec::new();

        bridge(&input[..], &mut written, &mut None);

        let answers: Vec<serde_json::Value> = String::from_utf8(written)
            .expect("the answers are text")
            .lines()
            .map(|line| serde_json::from_str(line).expect("each answer is JSON"))
            .collect();
        assert_eq!(
            answers.len(),
            2,
            "the session ended early or the tail was answered: {answers:?}"
        );
        assert_eq!(answers[0]["error"]["code"], -32600);
        assert_eq!(
            answers[1]["id"], 9,
            "the message after the over-long one was never answered"
        );
    }

    /// **A message of exactly the ceiling is not over it.** The delimiter used to be counted, so a
    /// message of exactly `MAX_REQUEST` bytes was flagged over and the swallow then ate the message
    /// after it — which the client waited on for ever. The assertion is that the next one arrives.
    #[test]
    fn a_message_of_exactly_the_ceiling_does_not_eat_the_next_one() {
        let mut input: Vec<u8> = Vec::new();
        input.extend_from_slice(&vec![b'x'; MAX_REQUEST]);
        input.push(b'\n');
        input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":11,\"method\":\"ping\"}\n");
        let mut written = Vec::new();

        bridge(&input[..], &mut written, &mut None);

        let answers: Vec<serde_json::Value> = String::from_utf8(written)
            .expect("the answers are text")
            .lines()
            .map(|line| serde_json::from_str(line).expect("each answer is JSON"))
            .collect();
        assert_eq!(answers.len(), 2, "the next message was eaten: {answers:?}");
        assert_eq!(
            answers[0]["error"]["code"], -32700,
            "a message of exactly the ceiling was refused as over it"
        );
        assert_eq!(answers[1]["id"], 11);
    }

    /// **A notification is acted on and never answered**, and getting this wrong is the failure that
    /// breaks a client outright: an answer to `notifications/initialized` is a message with no
    /// request behind it, and a strict client closes the connection over it.
    #[test]
    fn a_notification_is_not_answered() {
        let mut nowhere = None;

        assert!(
            answer(
                "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}",
                &mut nowhere
            )
            .is_none()
        );
    }

    #[test]
    fn the_handshake_answers_with_the_tools_capability() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\
             \"params\":{\"protocolVersion\":\"2024-11-05\"}}");

        assert_eq!(answered["result"]["protocolVersion"], "2024-11-05");
        assert!(answered["result"]["capabilities"]["tools"].is_object());
    }

    /// **This build advertises one tool**, and the assertion is on the whole list rather than on the
    /// presence of the one: advertising `split_open` before its scoping exists is exactly the
    /// mistake this pins.
    #[test]
    fn only_the_tool_this_build_serves_is_advertised() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}");

        let tools = answered["result"]["tools"]
            .as_array()
            .expect("there is a list of tools");
        let names: Vec<&str> = tools
            .iter()
            .map(|tool| tool["name"].as_str().expect("every tool is named"))
            .collect();
        assert_eq!(names, ["panel_set"]);
    }

    /// A call with nothing to publish is a **tool** error, not a protocol one: the agent reads it,
    /// decides the panel is not available and carries on working.
    #[test]
    fn a_call_missing_its_document_comes_back_as_a_tool_error() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\
             \"params\":{\"name\":\"panel_set\",\"arguments\":{\"source\":\"flow\"}}}");

        assert!(answered.get("error").is_none(), "{answered}");
        assert_eq!(answered["result"]["isError"], true);
    }

    /// A tool that does not exist is a protocol error, because the client asked for something this
    /// server never advertised.
    #[test]
    fn a_tool_that_was_never_advertised_is_a_protocol_error() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\
             \"params\":{\"name\":\"split_open\",\"arguments\":{}}}");

        assert_eq!(answered["error"]["code"], -32602);
    }

    #[test]
    fn a_line_that_is_not_json_is_answered_rather_than_swallowed() {
        let mut nowhere = None;
        let answered = answer("not json", &mut nowhere).expect("it deserved an answer");
        let answered: serde_json::Value = serde_json::from_str(&answered).expect("JSON");

        assert_eq!(answered["error"]["code"], -32700);
    }
}
