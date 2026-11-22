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
//! **It exposes the panel and split verbs the channel serves**, plus plan_set, plan_get, and
//! plan_retire for project roadmaps: `panel_set`, `panel_get`, `panel_patch`, the
//! retirement verb, and the three `split_*` that let an agent open, read and close a console of its
//! own. Each one is advertised because each one is answered; their authority lives on the window's
//! side of the channel.
//!
//! **Which pane it drives is not a parameter and cannot be one.** The two variables the program
//! inherited name its own pane's directory, the endpoint inside it is what the connection opens, and
//! no message carries an identifier for anybody to trust.

use std::io::{BufRead, Read, Write};

use panel_contract::{Attention, Mark, Style};
use panel_control::{Answered, MAX_REQUEST, Producer, Side};

/// The version answered when the client does not ask for one.
///
/// **What a client asks for is echoed back when it asks**, which is unusual enough to say why: this
/// server has no behaviour that varies with the version — it advertises the same operations
/// and forwards each call through the pane's channel — so refusing a client over a date would break
/// the connection for a difference that does not exist anywhere in this file.
const PROTOCOL_VERSION: &str = "2025-06-18";

fn main() {
    // **Opened lazily, on the first call that needs it.** A client that only lists the tools must
    // not fail because nothing is listening yet, and an agent's editor lists tools long before it
    // has anything to publish.
    let mut pane: Option<Producer> = None;
    // **The read that decides the handshake stays here**: taken further down, a unit test would
    // inherit the identity of whatever console runs the suite. That the first tool call judges the
    // variables by the same rule is D88's decision, and the binary's integration test holds both.
    let [id, dir] = panel_contract::ENV_NAMES.map(std::env::var_os);
    let route = std::env::var_os(panel_control::codex_route::ENV);
    // Instruction delivery must not depend on a socket metadata snapshot: Ivy starts its
    // child before binding the channel, while the bridge connects lazily on a later tool call.
    // Conditional guidance does not prove connectivity or claim a panel.
    let instructions = handshake_instructions(
        route.as_deref(),
        id.as_deref(),
        dir.as_deref(),
        panel_control::endpoint::BINDS && expected_server_available(),
        dir.as_deref()
            .is_some_and(|dir| std::path::Path::new(dir).is_dir()),
    );

    if let Some(route) = route {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || read_frames(std::io::stdin().lock(), sender));
        routed_loop(receiver, std::io::stdout(), &mut pane, instructions, &route);
    } else {
        bridge(
            std::io::stdin().lock(),
            std::io::stdout(),
            &mut pane,
            instructions,
        );
    }
}

fn expected_server_available() -> bool {
    #[cfg(windows)]
    {
        panel_control::client::server_pid(std::env::var_os("PANEL_SERVER_PID").as_deref()).is_ok()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Answers messages until the input ends.
///
/// **It takes its two ends rather than reaching for the process's**, so what it does with a message
/// it cannot decode can be asserted. Without that the only way to find out was to run the binary,
/// which is exactly why the decoding bug below went in unnoticed.
fn bridge(
    mut input: impl BufRead,
    mut output: impl Write,
    pane: &mut Option<Producer>,
    instructions: Option<&str>,
) {
    // **The input is taken as bytes and decoded per line, rather than by a decoding iterator.**
    // Reading it as text makes one malformed byte end the whole session: the iterator yields an
    // error the loop cannot answer, so a client that garbled a single message would be left waiting
    // for ever on every message after it too. As bytes, the bad line is the only casualty and it
    // leaves with the parse error the protocol has for exactly this.
    let route = std::env::var_os(panel_control::codex_route::ENV);
    let mut epoch: Option<String> = None;
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
            Ok(line) => answer_routed(line, pane, instructions, route.as_deref(), &mut epoch),
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

// The input thread owns framing, including the overlong-line tail. Its bounded queue cannot
// grow with an unresponsive client, while the dispatcher remains free to revoke idle producers.
enum Frame {
    Line(Vec<u8>),
    TooLong,
    ReadError(std::io::Error),
}

fn read_frames(mut input: impl BufRead, sender: std::sync::mpsc::SyncSender<Frame>) {
    loop {
        let mut line = Vec::new();
        match input
            .by_ref()
            .take(MAX_REQUEST as u64 + 1)
            .read_until(b'\n', &mut line)
        {
            Ok(0) => return,
            Ok(_) => (),
            Err(error) => {
                let _ = sender.send(Frame::ReadError(error));
                return;
            }
        }
        let ended = line.last() == Some(&b'\n');
        if line.len() - usize::from(ended) > MAX_REQUEST {
            if sender.send(Frame::TooLong).is_err() {
                return;
            }
            if !ended && let Err(error) = swallow_to_newline(&mut input) {
                let _ = sender.send(Frame::ReadError(error));
                return;
            }
        } else if sender.send(Frame::Line(line)).is_err() {
            return;
        }
    }
}

fn invalidate_stale_route(
    route: &std::ffi::OsStr,
    pane: &mut Option<Producer>,
    epoch: &mut Option<String>,
) {
    if pane.is_none() {
        return;
    }
    let current = panel_control::codex_route::lock_route(std::path::Path::new(route));
    if !current.as_ref().is_ok_and(|guard| {
        guard.identity.active && Some(guard.identity.epoch.as_str()) == epoch.as_deref()
    }) {
        *pane = None;
        *epoch = None;
    }
}

fn routed_loop(
    receiver: std::sync::mpsc::Receiver<Frame>,
    mut output: impl Write,
    pane: &mut Option<Producer>,
    instructions: Option<&str>,
    route: &std::ffi::OsStr,
) {
    use std::sync::mpsc::RecvTimeoutError;
    let mut epoch = None;
    loop {
        invalidate_stale_route(route, pane, &mut epoch);
        let frame = if pane.is_some() {
            match receiver.recv_timeout(std::time::Duration::from_millis(150)) {
                Ok(frame) => frame,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            // Abandoned on purpose: the idle MCP dispatcher can wait indefinitely for its client; it holds no pane connection and ends when the client closes stdin or the process exits.
            match receiver.recv() {
                Ok(frame) => frame,
                Err(_) => break,
            }
        };
        let answered = match frame {
            Frame::TooLong => Some(failed(
                &serde_json::Value::Null,
                -32600,
                "the message goes over the ceiling",
            )),
            Frame::ReadError(error) => {
                eprintln!(
                    "panel-mcp: reading a message failed ({}{})",
                    error.kind(),
                    error
                        .raw_os_error()
                        .map_or(String::new(), |number| format!(", errno {number}"))
                );
                break;
            }
            Frame::Line(line) => match std::str::from_utf8(&line) {
                Ok(line) if line.trim().is_empty() => None,
                Ok(line) => answer_routed(line, pane, instructions, Some(route), &mut epoch),
                Err(error) => Some(failed(
                    &serde_json::Value::Null,
                    -32700,
                    &format!("the message is not UTF-8 ({error})"),
                )),
            },
        };
        if let Some(answered) = answered
            && (writeln!(output, "{answered}").is_err() || output.flush().is_err())
        {
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
#[cfg(test)]
fn answer(line: &str, pane: &mut Option<Producer>, instructions: Option<&str>) -> Option<String> {
    answer_routed(line, pane, instructions, None, &mut None)
}

fn answer_routed(
    line: &str,
    pane: &mut Option<Producer>,
    instructions: Option<&str>,
    route: Option<&std::ffi::OsStr>,
    epoch: &mut Option<String>,
) -> Option<String> {
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
        "initialize" => succeeded(&id, initialized(&message, instructions)),
        "tools/list" => succeeded(&id, tools()),
        "tools/call" => {
            let proof = route
                .and_then(|path| {
                    panel_control::codex_route::lock_route(std::path::Path::new(path)).ok()
                })
                .map(|guard| guard.identity.proof.clone());
            let answer = called_routed(&id, &message, pane, route, epoch);
            attach_route_proof(answer, proof.as_deref())
        }
        "ping" => succeeded(&id, serde_json::json!({})),
        other => failed(&id, -32601, &format!("no method \"{other}\"")),
    })
}

fn attach_route_proof(answer: String, proof: Option<&str>) -> String {
    let Some(proof) = proof else {
        return answer;
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&answer) else {
        return answer;
    };
    if let Some(result) = value
        .get_mut("result")
        .and_then(serde_json::Value::as_object_mut)
    {
        result.insert("_meta".into(), serde_json::json!({"ivy/routeProof": proof}));
        value.to_string()
    } else {
        answer
    }
}

/// The instructions this bridge sends with its handshake, or none.
///
/// **Only a whole identity is promised a panel** — the rule of
/// [`panel_control::client::whole_identity`], the same one the first tool call applies — so the
/// handshake and the calls never disagree about whether this is a console.
///
/// Guidance requires a supported channel and, on Windows, an inherited expected server PID.
/// Connectivity is checked lazily by the first operation.
///
/// It takes the values — and whether the channel binds — instead of reading them, so both gates
/// can be tested on any platform without `set_var`; `main` reads them, and the binary's integration
/// test covers that line.
fn handshake_instructions(
    route: Option<&std::ffi::OsStr>,
    id: Option<&std::ffi::OsStr>,
    dir: Option<&std::ffi::OsStr>,
    binds: bool,
    directory_present: bool,
) -> Option<&'static str> {
    if route.is_some() {
        return binds.then_some(ROUTED_INSTRUCTIONS);
    }
    instructions_for(id, dir, binds, directory_present)
}

const ROUTED_INSTRUCTIONS: &str = "This server may reach the Ivy console attached to this conversation. \
The route can be pending or unavailable, and a tool call reports whether it works. \
When it is available, publish task status with this server's panel_set, update it with panel_patch, \
and use attention wait, block, or done when the person needs to know. \
A subagent sharing the same console should leave its parent's panel alone.";

fn instructions_for(
    id: Option<&std::ffi::OsStr>,
    dir: Option<&std::ffi::OsStr>,
    binds: bool,
    directory_present: bool,
) -> Option<&'static str> {
    if !binds || !directory_present {
        return None;
    }
    panel_control::client::whole_identity(id, dir).map(|_| INSTRUCTIONS)
}

/// **What an agent is told when it connects inside an Ivy console**, in the harnesses that pass a
/// server's instructions on to their model. The tools alone were not enough: a harness may defer
/// them — listed by name, no schema — and an agent that never loads them publishes nothing, so the
/// person sees an empty panel. D88.
///
/// - It names only the verbs publishing needs, `panel_get` included: the `split_*` and retirement
///   descriptions already explain the rest.
/// - «This server's» is deliberate: a second panel server may be registered beside this one and
///   serve the same names.
/// - An agent working for another agent in the same console — a subagent sharing this connection,
///   or a worker process with a bridge of its own — is told to leave the panel alone: the first
///   would overwrite its parent's document through the connection that holds the pane, the second
///   would be refused at every stop or, publishing first, hold the pane against its parent (D43).
///   Only that case: an agent another agent started in a console of its own has its own identity
///   and its own panel to keep.
/// - A rendered document is passed through as printed only when it comes from a workflow the agent
///   was asked to follow, and never with buttons the agent would not run itself: a panel's
///   «actions» are commands for the person's shell, and a document printed by a script of unknown
///   origin is the untrusted panel the contract warns about.
/// - A patch addresses lines by `key`, so lines meant to be patched are published with one.
/// - A new connection holds nothing, so after a restart the document read back has to be published
///   again before any patch — only when it was the agent's own (a file producer may have painted
///   the pane meanwhile), and without a claim the dead session left, which nobody holds any more.
const INSTRUCTIONS: &str = "\
This server may reach the Ivy console named by its inherited identity. Its channel can be \
starting or unavailable. Call this server's `panel_get` to check; a successful call verifies the \
connection. When available, Ivy paints the status you publish beside that console. Do not repeat \
installation just because its panel is empty. If a tool is unavailable, explain whether the client \
needs reconnecting; never copy another console's identity or change registration automatically.

- If you work for another agent in this same console (a subagent, or a worker it started), leave \
the panel alone: it belongs to the agent the person is talking to.
- Otherwise publish with this server's `panel_set` (`source`: your name or your workflow's) when \
you start a task, before every long stretch and at every stop. The tools may be deferred: load them \
first.
- Set `attention` to `wait` when you need the person; `block` when stuck on something else; `done` \
when finished. Put the question itself on a line marked wait. Leave `attention` out while you work, \
and republish without it as soon as it stops being true.
- Give a `key` to every line you will want to change, and once you have published with `panel_set` \
update those lines with `panel_patch`. After a restart, call `panel_get` and publish that document \
again with `panel_set` before patching if it is the panel you had published, leaving out an \
`attention` that is no longer true; after a compaction, call `panel_get` before publishing again.
- When a workflow you were asked to follow renders a panel document for you (JSON with `lines`), \
pass it to `panel_set` as it was printed; if it carries `actions` you would not run yourself, \
publish it without them, because a button sends its command to the person's shell.";

/// What this server says it is, and — only where [`instructions_for`] gave some — what it asks of
/// the agent. Absent rather than empty: an empty string is still an instruction to a client.
fn initialized(message: &serde_json::Value, instructions: Option<&str>) -> serde_json::Value {
    let asked_for = message
        .get("params")
        .and_then(|params| params.get("protocolVersion"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);

    let mut answer = serde_json::json!({
        "protocolVersion": asked_for,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "ivy panel", "version": env!("CARGO_PKG_VERSION")},
    });
    if let Some(instructions) = instructions {
        answer["instructions"] = serde_json::Value::from(instructions);
    }
    answer
}

/// `a, b or c`: a closed vocabulary as the description names it.
///
/// **Written from the vocabulary's own list** rather than typed into the description, because the
/// description is the only copy of the contract some producers ever read, and a word added to the
/// contract and not to it is exactly the vocabulary-from-nowhere this description exists to end.
fn listed(words: &[&str]) -> String {
    match words.split_last() {
        Some((last, [])) => (*last).to_owned(),
        Some((last, init)) => format!("{} or {last}", init.join(", ")),
        None => String::new(),
    }
}

/// The tools this build serves, with the shape of what each takes.
fn tools() -> serde_json::Value {
    serde_json::json!({
        "tools": [
            {
                "name": "panel_set",
                "description": format!(
                    "Publish this pane's status panel. The document is the panel document: a \
                     required \"lines\" array, and optionally \"phase\", \"attention\", \
                     \"actions\" and the rest of the contract. A line is a string, or an object \
                     with \"text\" and optionally \"mark\", \"style\", \"indent\", \"key\", \
                     \"ref\" and \"link\". Three fields take a closed vocabulary: \"attention\" \
                     is {}; a line's \"mark\" is {}; its \"style\" is {}. Any other non-null \
                     value is dropped, the panel still paints, and the reply names what fell away. \
                     Publishing claims this pane under the given source until this process exits; \
                     while it is claimed, the pane's panel.json is not read and another source is \
                     refused.",
                    listed(&Attention::ALL.map(Attention::name)),
                    listed(&Mark::ALL.map(Mark::name)),
                    listed(&Style::ALL.map(Style::name)),
                ),
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
            },
            {
                "name": "panel_get",
                "description": "Read back the document this pane's status panel is showing, \
                                including one that arrived through the pane's panel.json rather than \
                                over this channel. It takes no arguments, claims nothing and changes \
                                nothing: it is how a process that restarted, or that lost the \
                                document it published, finds out what is on screen before publishing \
                                again.",
                "inputSchema": {"type": "object", "properties": {}},
            },
            {
                "name": "panel_retire",
                "description": "Intentionally end the session currently driving this pane: remove \
                                its panel and close auxiliary consoles this connection still owns, \
                                while preserving consoles the person has taken over.",
                "inputSchema": {"type": "object", "properties": {}},
            },
            {
                "name": "plan_set",
                "description": "Publish the complete roadmap for this console's project. The project is always the project of the pane this server belongs to; the optional repo argument is accepted for compatibility and ignored. The published plan takes precedence over the configured roadmap file until plan_retire removes it, and Ivy persists the published document across restarts. Item marks use the closed vocabulary done, current, pending, wait or block; other marks are dropped and named in the reply.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "document": {"type": "object", "description": "The complete plan document, with milestones and items."},
                        "repo": {"type": "string", "description": "Accepted for compatibility; project scope is always this console's project."}
                    },
                    "required": ["document"]
                }
            },
            {
                "name": "plan_get",
                "description": "Read the published roadmap document for this console's project. The optional repo argument is accepted and ignored; scope comes from this console's pane. Returns only the published plan, which takes precedence over the configured file roadmap and is persisted across Ivy restarts, or reports that none is published.",
                "inputSchema": {"type": "object", "properties": {"repo": {"type": "string"}}}
            },
            {
                "name": "plan_retire",
                "description": "Remove the published roadmap for this console's project so the configured file roadmap can show again. The optional repo argument is accepted and ignored; scope comes from this console's pane. Published plans persist across Ivy restarts until retired.",
                "inputSchema": {"type": "object", "properties": {"repo": {"type": "string"}}}
            },
            {
                "name": "panel_patch",
                "description": "Change part of this pane's status panel without republishing the \
                                rest of it. Each op names what it does: \"replace\", \"before\", \
                                \"after\" and \"delete\" address a line by the \"key\" its \
                                producer gave it, \"first\" and \"last\" put a line at either end \
                                of the body, and \"set\" changes one of \"phase\", \
                                \"attention\", \"header\" or \"stale_after_minutes\". Either the \
                                whole list applies or none of it does: a key that names no line, or \
                                two, refuses the patch and leaves the panel exactly as it was. It \
                                needs this pane claimed with panel_set first, and buttons are not \
                                patchable. A line an op carries is read like a line of panel_set's \
                                document: a mark or style outside its vocabulary is dropped and named \
                                in the reply.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "ops": {
                            "type": "array",
                            "description": "The edits, applied in order; each one sees what the \
                                            ones before it did.",
                            "items": {"type": "object"},
                        },
                    },
                    "required": ["ops"],
                },
            },
            {
                "name": "split_open",
                "description": "Open a console beside this pane and return a handle for it. The \
                                handle belongs to this connection: no other connection can use it. The \
                                keyboard stays where the person left it. A console with no \
                                addressable identity answers a null handle.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "side": {
                            "type": "string",
                            "enum": ["left", "right", "top", "bottom"],
                            "description": "Which edge of this pane the new console goes on.",
                        },
                        "program": {
                            "type": "string",
                            "description": "What to run in it; the user's shell, the same one this pane runs, if \
                                            it is left out.",
                        },
                        "args": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Arguments for that program. There is no environment: \
                                            the console gets this pane's identity and nothing else.",
                        },
                    },
                    "required": ["side"],
                },
            },
            {
                "name": "split_read",
                "description": "Read a console this process opened, by its handle. Returns what is on \
                                its screen, or the whole history if the scrollback is asked for. \
                                Refused with the reason once the person has typed into that console.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": {
                            "type": "string",
                            "description": "The handle split_open answered with.",
                        },
                        "scrollback": {
                            "type": "boolean",
                            "description": "Read the whole history instead of the visible screen. \
                                            Defaults to false.",
                        },
                    },
                    "required": ["pane"],
                },
            },
            {
                "name": "split_close",
                "description": "Close a console this process opened, by its handle. Refused for a \
                                handle this connection does not own, for a console that is its \
                                tab's only one, and for one the person has typed into.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane": {
                            "type": "string",
                            "description": "The handle split_open answered with.",
                        },
                    },
                    "required": ["pane"],
                },
            },
            {
                "name": "image_open",
                // The one guard a subagent meets (D88): nothing on the wire tells it from the agent
                // whose connection it shares, so the last sentence is the whole of it (D120).
                "description": IMAGE_OPEN,
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The image file's absolute path.",
                        },
                    },
                    "required": ["path"],
                },
            }
        ],
    })
}

/// What `image_open` tells the agent it does: the tool's own description, worded for the agent
/// and not copied from the contract (D120).
const IMAGE_OPEN: &str = "Show an image to the person in the drawer under this console, where they can look at it, mark it up and send it back to this console as a line with the path of an annotated copy. Each console holds one image: this replaces the one this console had, unless the person has marks on it they have not sent, which refuses the call. If the person is working in this console the drawer opens on it at once and the keyboard stays where it was; otherwise nothing moves, and this console's tab and header show that a new image is waiting. PNG or JPEG, at most 32 MiB. If you work for another agent in this same console (a subagent, or a worker it started), do not call this: give the path to that agent.";

/// The file `path` names, if an agent may hand it to the window: refused with the sentence the agent
/// reads otherwise.
///
/// **A courtesy, not the enforcement** (D120): it answers at once, in the bridge's own process —
/// the agent's, or the shared daemon when Codex's calls are routed — so a stuck filesystem stalls
/// the bridge and never the window; but the window may see another file (another mount namespace, a
/// file replaced meanwhile), so its own read checks again. A path on another machine is refused
/// before anything is asked of it: asking is what reaches that host.
fn image_to_open(path: &str) -> Result<std::path::PathBuf, String> {
    if path.is_empty() {
        return Err("image_open's \"path\" is empty".to_owned());
    }
    let file = std::path::PathBuf::from(path);
    if !file.is_absolute() {
        return Err(format!(
            "image_open needs an absolute path, and {path} is relative"
        ));
    }
    if !panel_control::image::is_local(&file) {
        return Err(format!("{path} is not on this machine"));
    }
    let found = std::fs::metadata(&file).map_err(|why| format!("cannot open {path}: {why}"))?;
    if found.is_dir() {
        return Err(format!("{path} is a folder, not an image"));
    }
    if !found.is_file() {
        return Err(format!("{path} is not a regular file"));
    }
    if found.len() > panel_control::image::MAX_BYTES {
        return Err(format!(
            "{path} is over {} MiB",
            panel_control::image::MAX_BYTES / (1024 * 1024)
        ));
    }
    // The first bytes decide, not the extension (D120): an agent told «opened» for a file the
    // drawer cannot show would wait for marks that never come.
    let mut head = Vec::with_capacity(PNG_SIGNATURE.len());
    std::io::Read::read_to_end(
        &mut std::io::Read::take(
            std::fs::File::open(&file).map_err(|why| format!("cannot open {path}: {why}"))?,
            PNG_SIGNATURE.len() as u64,
        ),
        &mut head,
    )
    .map_err(|why| format!("cannot read {path}: {why}"))?;
    if !head.starts_with(PNG_SIGNATURE) && !head.starts_with(JPEG_START) {
        return Err(format!("{path} is not a PNG or JPEG image"));
    }
    Ok(file)
}

/// How every PNG file begins.
const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// How every JPEG file begins: the start-of-image marker and the first byte of the next one.
const JPEG_START: &[u8] = &[0xFF, 0xD8, 0xFF];

/// What one tool call asks for, **already read and validated**.
///
/// **It exists so the channel is opened after the arguments are understood**, and not before: a
/// malformed call is a tool error the agent reads, and finding it out must not require a pane.
enum Ask {
    Publish(String, String),
    ReadTheDocument,
    Retire,
    PlanSet(String, Option<String>),
    PlanGet(Option<String>),
    PlanRetire(Option<String>),
    Patch(String),
    Open(Side, Option<String>, Vec<String>),
    Read(String, bool),
    Close(String),
    ImageOpen(std::path::PathBuf),
}

/// What to call an empty `ok`, **decided from what was asked and never from the shape of the reply**.
///
/// **A word per verb rather than a flag per exception, and a function rather than a `let`.** The
/// first shape was a bool asking whether the call was a close, and the second verb to answer an
/// empty `ok` walked straight into it: a patch reported «published», on a channel whose entire
/// distinction is that publishing replaces the document and patching edits one line of it. An agent
/// reading that concludes it re-sent everything. It is out here so a test can reach it without a
/// pane — which is what the `let` could not offer, and is why nothing caught the first mistake.
fn what_it_did(ask: &Ask) -> &'static str {
    match ask {
        Ask::Close(_) => "closed",
        // The window's own sentence is the answer; this word is only for an empty `ok`.
        Ask::ImageOpen(_) => "opened",
        Ask::Patch(_) => "patched",
        Ask::Retire => "retired",
        Ask::PlanRetire(..) => "retired",
        Ask::PlanGet(..) => "read",
        Ask::Publish(..)
        | Ask::PlanSet(..)
        | Ask::ReadTheDocument
        | Ask::Open(..)
        | Ask::Read(..) => "published",
    }
}

/// Runs a tool call.
fn called_routed(
    id: &serde_json::Value,
    message: &serde_json::Value,
    pane: &mut Option<Producer>,
    route: Option<&std::ffi::OsStr>,
    epoch: &mut Option<String>,
) -> String {
    let params = message.get("params").unwrap_or(&serde_json::Value::Null);
    let name = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    // **A tool this server never advertised is a protocol error**, and it is checked before the pane
    // is even reached: a call for something nobody serves must not open the channel to find that out.
    if !matches!(
        name,
        "panel_set"
            | "panel_get"
            | "panel_retire"
            | "plan_set"
            | "plan_get"
            | "plan_retire"
            | "panel_patch"
            | "split_open"
            | "split_read"
            | "split_close"
            | "image_open"
    ) {
        return failed(id, -32602, &format!("no tool \"{name}\""));
    }

    // **What the call asks for, read and validated before the pane is reached.** A malformed argument
    // is a tool error the agent reads, and finding that out must not need a channel — which is also
    // what lets the refusals of a missing or unknown field be asserted without a pane to reach.
    let ask = match name {
        "panel_set" => {
            let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let Some(source) = arguments.get("source").and_then(serde_json::Value::as_str) else {
                return succeeded(id, refusal("panel_set needs a \"source\""));
            };
            let Some(document) = arguments.get("document") else {
                return succeeded(id, refusal("panel_set needs a \"document\""));
            };
            Ask::Publish(source.to_owned(), document.to_string())
        }
        // **It takes nothing, so there is nothing to validate.** Any arguments an agent sends are
        // ignored rather than refused: refusing a field that reaches nothing would be inventing a
        // shape this verb does not have.
        "panel_get" => Ask::ReadTheDocument,
        // Like `panel_get`, it takes no arguments; unknown arguments reach nothing and are ignored.
        "panel_retire" => Ask::Retire,
        "plan_set" => {
            let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let Some(document) = arguments.get("document") else {
                return succeeded(id, refusal("plan_set needs a \"document\""));
            };
            let repo = match arguments.get("repo") {
                None => None,
                Some(serde_json::Value::String(repo)) => Some(repo.clone()),
                Some(_) => return succeeded(id, refusal("plan_set's \"repo\" is not text")),
            };
            Ask::PlanSet(document.to_string(), repo)
        }
        "plan_get" | "plan_retire" => {
            let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let repo = match arguments.get("repo") {
                None => None,
                Some(serde_json::Value::String(repo)) => Some(repo.clone()),
                Some(_) => return succeeded(id, refusal("the optional \"repo\" is not text")),
            };
            if name == "plan_get" {
                Ask::PlanGet(repo)
            } else {
                Ask::PlanRetire(repo)
            }
        }
        "panel_patch" => {
            let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            // **Its presence and its shape are checked here, and what each op says is not**: the
            // contract's own reader is the only party that decides that, and it answers in the
            // sentences a producer reads on every transport.
            let Some(ops) = arguments.get("ops").filter(|ops| ops.is_array()) else {
                return succeeded(id, refusal("panel_patch needs an \"ops\" list"));
            };
            Ask::Patch(ops.to_string())
        }
        "split_open" => {
            let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let Some(side) = arguments.get("side").and_then(serde_json::Value::as_str) else {
                return succeeded(id, refusal("split_open needs a \"side\""));
            };
            let Some(side) = Side::parse(side) else {
                return succeeded(id, refusal("split_open names no side this build knows"));
            };
            let program = match arguments.get("program") {
                None => None,
                Some(serde_json::Value::String(text)) => Some(text.clone()),
                // **A present value of the wrong type is refused by name**, never defaulted: an
                // agent that sent a number here would otherwise have its program silently dropped.
                Some(_) => {
                    return succeeded(id, refusal("split_open's \"program\" is not text"));
                }
            };
            let args = match arguments.get("args") {
                None => Vec::new(),
                Some(serde_json::Value::Array(items)) => {
                    let mut args = Vec::with_capacity(items.len());
                    for item in items {
                        let Some(text) = item.as_str() else {
                            return succeeded(
                                id,
                                refusal("split_open's \"args\" is not a list of text"),
                            );
                        };
                        args.push(text.to_owned());
                    }
                    args
                }
                Some(_) => {
                    return succeeded(id, refusal("split_open's \"args\" is not a list of text"));
                }
            };
            Ask::Open(side, program, args)
        }
        "split_read" => {
            let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let Some(handle) = arguments.get("pane").and_then(serde_json::Value::as_str) else {
                return succeeded(id, refusal("split_read needs a \"pane\""));
            };
            let scrollback = match arguments.get("scrollback") {
                None => false,
                Some(serde_json::Value::Bool(flag)) => *flag,
                Some(_) => {
                    return succeeded(
                        id,
                        refusal("split_read's \"scrollback\" is not true or false"),
                    );
                }
            };
            Ask::Read(handle.to_owned(), scrollback)
        }
        "image_open" => {
            let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let path = match arguments.get("path") {
                None => return succeeded(id, refusal("image_open needs a \"path\"")),
                Some(serde_json::Value::String(path)) => path,
                Some(_) => return succeeded(id, refusal("image_open's \"path\" is not text")),
            };
            match image_to_open(path) {
                Ok(path) => Ask::ImageOpen(path),
                Err(why) => return succeeded(id, refusal(&why)),
            }
        }
        // `split_close`, the only name left after the check above.
        _ => {
            let arguments = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let Some(handle) = arguments.get("pane").and_then(serde_json::Value::as_str) else {
                return succeeded(id, refusal("split_close needs a \"pane\""));
            };
            Ask::Close(handle.to_owned())
        }
    };

    // **A failure to reach the pane is a tool error and not a protocol one**, which is the
    // distinction the protocol draws and it matters here: the agent is meant to read it, decide the
    // panel is not available and carry on working, not to treat its whole connection as broken.
    // Keep the route lock until the producer operation has returned. A stale or missing
    // capability never falls back to the process environment.
    let guard = if let Some(route) = route {
        match panel_control::codex_route::lock_route(std::path::Path::new(route)) {
            Ok(guard) if guard.identity.active => Some(guard),
            Ok(_) => {
                *pane = None;
                *epoch = None;
                return succeeded(id, refusal("panel route is inactive"));
            }
            Err(error) => {
                *pane = None;
                *epoch = None;
                return succeeded(id, refusal(&format!("panel route unavailable: {error}")));
            }
        }
    } else {
        None
    };
    if let Some(guard) = &guard {
        if epoch.as_deref() != Some(guard.identity.epoch.as_str()) {
            *pane = None;
            *epoch = Some(guard.identity.epoch.clone());
        }
        if pane.is_none() {
            match Producer::of(&guard.identity.pane_dir, &guard.identity.pane_id) {
                Ok(producer) => *pane = Some(producer),
                Err(error) => return succeeded(id, refusal(&error.to_string())),
            }
        }
    }
    let producer = match open(pane) {
        Ok(producer) => producer,
        Err(failure) => return succeeded(id, refusal(&failure)),
    };

    let did = what_it_did(&ask);
    let answer = match ask {
        Ask::Publish(source, document) => producer.publish(&source, &document),
        Ask::ReadTheDocument => producer.read_the_document(),
        Ask::Retire => producer.retire(),
        Ask::PlanSet(document, repo) => producer.publish_plan(&document, repo.as_deref()),
        Ask::PlanGet(repo) => producer.read_plan(repo.as_deref()),
        Ask::PlanRetire(repo) => producer.retire_plan(repo.as_deref()),
        Ask::Patch(ops) => producer.patch(&ops),
        Ask::Open(side, program, args) => producer.open_a_pane(side, program.as_deref(), &args),
        Ask::Read(handle, scrollback) => producer.read_a_pane(&handle, scrollback),
        Ask::Close(handle) => producer.close_a_pane(&handle),
        Ask::ImageOpen(path) => producer.open_an_image(&path),
    };

    match answer {
        Ok(answered) => succeeded(id, result_of(did, answered)),
        Err(failure) => {
            // The channel is gone; the next call opens a fresh one rather than reusing a dead
            // connection — and opening a fresh one is also what claims the pane again.
            *pane = None;
            succeeded(id, refusal(&failure.to_string()))
        }
    }
}

/// What the pane answered, as the tool result an agent reads.
///
/// **Out of [`called_routed`] so a test reaches it without a pane**, for the reason [`what_it_did`] gives:
/// every arm here is only walked once a channel exists, which is how a verb once shipped answering
/// the wrong word.
fn result_of(did: &str, answered: Answered) -> serde_json::Value {
    match answered {
        // **Three verbs answer with the same empty `ok`**, so the word comes from what was asked and
        // never from the shape of the reply.
        Answered::Ok => text(did),
        // **Still the verb's word, and then what fell away**: it was applied, so an agent reading only
        // the start of the answer is not misled, and one reading the rest learns what to correct.
        Answered::Lossy(dropped) => text(&format!("{did}; {dropped}")),
        Answered::Retired { closed, preserved } => text(&retirement_result(closed, preserved)),
        // **The handle is the whole answer to an open**, and a null one is not an error: the console
        // exists and is usable, it is only this channel that cannot address it.
        Answered::Opened(Some(handle)) => text(&handle),
        Answered::Opened(None) => text("opened, with no addressable identity"),
        Answered::Text(what) => text(&what),
        // **The document goes back as the JSON it is**, not wrapped in a sentence about it: what the
        // caller wants is something it can read, edit and publish again.
        Answered::Document(document) => text(&document),
        Answered::Failed(why) => refusal(&why),
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

/// One line of text as a tool's result, which is what every verb that is not an error answers with.
fn text(what: &str) -> serde_json::Value {
    serde_json::json!({"content": [{"type": "text", "text": what}]})
}

fn retirement_result(closed: usize, preserved: usize) -> String {
    format!(
        "retired; closed {closed} auxiliary console(s); preserved {preserved} taken-over console(s)"
    )
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

    #[test]
    fn pending_capability_attests_without_disclosing_its_path_and_invalid_cannot_attest() {
        let home = panel_contract::PanelDir::create().unwrap();
        let registry = panel_control::codex_route::Registry::for_home(home.path()).unwrap();
        let route = registry.pending().unwrap();
        let proof = panel_control::codex_route::lock_route(&route)
            .unwrap()
            .identity
            .proof;
        let message =
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"panel_get"}}"#;
        let mut pane = None;
        let mut epoch = None;
        let result = answer_routed(
            message,
            &mut pane,
            None,
            Some(route.as_os_str()),
            &mut epoch,
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["result"]["_meta"]["ivy/routeProof"], proof);
        assert!(result.contains("inactive"));
        assert!(!result.contains(route.to_str().unwrap()));
        std::fs::remove_file(&route).unwrap();
        let missing = answer_routed(
            message,
            &mut pane,
            None,
            Some(route.as_os_str()),
            &mut epoch,
        )
        .unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&missing).unwrap()["result"]
                .get("_meta")
                .is_none()
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "unix-only: drives the Unix socket channel; the Windows pipe has its own witnesses"]
    fn idle_revocation_disconnects_the_producer_without_another_tool_call() {}

    #[cfg(unix)]
    #[test]
    fn idle_revocation_disconnects_the_producer_without_another_tool_call() {
        use panel_contract::PanelDir;
        use panel_control::{Answer, Channel, Producer, What, codex_route::Registry};
        use std::sync::{Arc, mpsc::sync_channel};
        use std::time::Duration;
        struct SignalOutput(std::sync::mpsc::SyncSender<()>);
        impl Write for SignalOutput {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                Ok(data.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.0.send(()).unwrap();
                Ok(())
            }
        }
        let home = PanelDir::create().unwrap();
        let dir = PanelDir::create().unwrap();
        let (tx, inbox) = sync_channel(8);
        let _channel = Channel::open_with_reader(
            dir.path(),
            dir.id(),
            tx,
            Arc::new(|| Ok(r#"{"lines":[]}"#.into())),
            Arc::new(|| {}),
        )
        .unwrap();
        let registry = Registry::for_home(home.path()).unwrap();
        let route = registry.pending().unwrap();
        registry
            .bind(
                "01234567-89ab-cdef-0123-456789abcdef",
                &route,
                dir.id(),
                dir.path(),
                "one",
            )
            .unwrap();
        let (frames, receiver) = sync_channel(1);
        let (flush, flushed) = sync_channel(0);
        let route_for_thread = route.clone();
        let worker = std::thread::spawn(move || {
            let mut pane = None;
            routed_loop(
                receiver,
                SignalOutput(flush),
                &mut pane,
                None,
                route_for_thread.as_os_str(),
            );
        });
        frames
            .send(Frame::Line(
                br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"panel_set","arguments":{"source":"old","document":{"lines":["old"]}}}}"#
                    .to_vec(),
            ))
            .unwrap();
        let initial = inbox.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(initial.what, What::Publish { .. }));
        initial.answer.send(Answer::Taken).unwrap();
        flushed.recv_timeout(Duration::from_secs(5)).unwrap();
        registry.revoke(&route, "one").unwrap();
        let closed = inbox.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(closed.what, What::Closed));
        let mut replacement = Producer::of(dir.path(), dir.id()).unwrap();
        let publish =
            std::thread::spawn(move || replacement.publish("next", r#"{"lines":["new"]}"#));
        let message = inbox.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(matches!(message.what, What::Publish { .. }));
        message.answer.send(Answer::Taken).unwrap();
        assert!(matches!(
            publish.join().unwrap().unwrap(),
            panel_control::Answered::Ok
        ));
        drop(frames);
        worker.join().unwrap();
    }

    #[test]
    fn routed_reader_keeps_boundaries_after_an_overlong_frame() {
        let mut input = vec![b'x'; MAX_REQUEST + 50];
        input.extend_from_slice(b"\nnext\n");
        let (sender, receiver) = std::sync::mpsc::sync_channel(4);
        read_frames(std::io::Cursor::new(input), sender);
        assert!(matches!(receiver.recv().unwrap(), Frame::TooLong));
        match receiver.recv().unwrap() {
            Frame::Line(bytes) => assert_eq!(bytes, b"next\n"),
            _ => panic!("expected next frame"),
        }
    }

    #[test]
    fn routed_handshake_does_not_claim_a_connection_or_inherit_stale_pane_identity() {
        let routed = handshake_instructions(
            Some(std::ffi::OsStr::new("/route")),
            None,
            None,
            true,
            false,
        )
        .unwrap();
        assert!(routed.contains("pending or unavailable"));
        assert!(!routed.contains("This session runs in an Ivy console"));
        assert_eq!(handshake_instructions(None, None, None, true, false), None);
        assert_eq!(
            handshake_instructions(
                Some(std::ffi::OsStr::new("/route")),
                None,
                None,
                false,
                false
            ),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "unix-only: drives the Unix socket channel; the Windows pipe has its own witnesses"]
    fn route_reassociation_moves_an_existing_bridge_between_real_panes() {}

    #[cfg(unix)]
    #[test]
    fn route_reassociation_moves_an_existing_bridge_between_real_panes() {
        use panel_contract::PanelDir;
        use panel_control::{Channel, codex_route::Registry};
        use std::sync::{Arc, mpsc::sync_channel};
        let home = PanelDir::create().unwrap();
        let first = PanelDir::create().unwrap();
        let second = PanelDir::create().unwrap();
        let (tx1, rx1) = sync_channel(8);
        let (tx2, rx2) = sync_channel(8);
        let _first_channel = Channel::open_with_reader(
            first.path(),
            first.id(),
            tx1,
            Arc::new(|| Ok(r#"{"lines":["first"]}"#.into())),
            Arc::new(|| {}),
        )
        .unwrap();
        let _second_channel = Channel::open_with_reader(
            second.path(),
            second.id(),
            tx2,
            Arc::new(|| Ok(r#"{"lines":["second"]}"#.into())),
            Arc::new(|| {}),
        )
        .unwrap();
        let registry = Registry::for_home(home.path()).unwrap();
        let route = registry.pending().unwrap();
        let thread = "01234567-89ab-cdef-0123-456789abcdef";
        let mut pane = None;
        let mut epoch = None;
        let message =
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"panel_get"}}"#;
        registry
            .bind(thread, &route, first.id(), first.path(), "one")
            .unwrap();
        let first_result = answer_routed(
            message,
            &mut pane,
            None,
            Some(route.as_os_str()),
            &mut epoch,
        )
        .unwrap();
        assert!(first_result.contains("first"));
        let window_first = std::thread::spawn(move || {
            let open = rx1.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
            assert!(matches!(open.what, panel_control::What::Open { .. }));
            open.answer
                .send(panel_control::Answer::Opened(Some("old-handle".into())))
                .unwrap();
            let closed = rx1.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
            assert!(matches!(closed.what, panel_control::What::Closed));
        });
        let open = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"split_open","arguments":{"side":"right"}}}"#;
        assert!(
            answer_routed(open, &mut pane, None, Some(route.as_os_str()), &mut epoch)
                .unwrap()
                .contains("old-handle")
        );
        registry
            .bind(thread, &route, second.id(), second.path(), "two")
            .unwrap();
        registry.revoke(&route, "one").unwrap();
        let second_result = answer_routed(
            message,
            &mut pane,
            None,
            Some(route.as_os_str()),
            &mut epoch,
        )
        .unwrap();
        assert!(second_result.contains("second"));
        assert!(!second_result.contains("first"));
        window_first.join().unwrap();
        let window_second = std::thread::spawn(move || {
            let read = rx2.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
            assert!(matches!(read.what, panel_control::What::Read { .. }));
            read.answer
                .send(panel_control::Answer::Refused("unknown handle".into()))
                .unwrap();
        });
        let read = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"split_read","arguments":{"pane":"old-handle"}}}"#;
        assert!(
            answer_routed(read, &mut pane, None, Some(route.as_os_str()), &mut epoch)
                .unwrap()
                .contains("unknown handle")
        );
        window_second.join().unwrap();
        registry.revoke(&route, "two").unwrap();
        let revoked = answer_routed(
            message,
            &mut pane,
            None,
            Some(route.as_os_str()),
            &mut epoch,
        )
        .unwrap();
        assert!(revoked.contains("inactive"));
        assert!(pane.is_none());
        std::fs::remove_file(&route).unwrap();
        let missing = answer_routed(
            message,
            &mut pane,
            None,
            Some(route.as_os_str()),
            &mut epoch,
        )
        .unwrap();
        assert!(missing.contains("unavailable"));
    }

    fn ask(message: &str) -> serde_json::Value {
        let mut nowhere = None;
        let answered = answer(message, &mut nowhere, None).expect("it deserved an answer");
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

        bridge(&input[..], &mut written, &mut None, None);

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

        bridge(&input[..], &mut written, &mut None, None);

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

        bridge(&input[..], &mut written, &mut None, None);

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
                &mut nowhere,
                None
            )
            .is_none()
        );
    }

    /// **The instructions follow `whole_identity` and `BINDS`, and nothing else.** The identity
    /// rule has its own table beside it; this holds what `instructions_for` adds — a whole identity
    /// gets the text where the channel binds, a broken one never does — with one broken row per
    /// half, so a handshake that judged only one variable fails here.
    #[test]
    fn instructions_are_given_only_for_a_whole_identity() {
        use std::ffi::OsStr;
        let absolute = if cfg!(windows) {
            r"C:\run\ivy-a1b2c3d4"
        } else {
            "/run/user/1000/ivy-a1b2c3d4"
        };
        let id = Some(OsStr::new("a1b2c3d4"));
        let dir = Some(OsStr::new(absolute));

        assert_eq!(instructions_for(id, dir, true, true), Some(INSTRUCTIONS));
        assert_eq!(
            instructions_for(id, dir, false, true),
            None,
            "a platform where the channel does not bind"
        );
        assert_eq!(instructions_for(id, dir, true, false), None, "no directory");
        assert_eq!(
            instructions_for(None, dir, true, true),
            None,
            "no identifier"
        );
        assert_eq!(instructions_for(id, None, true, true), None, "no directory");
    }

    /// **The handshake carries the instructions it was given, and is otherwise the same reply**,
    /// driven through `bridge` so the value is proven to travel from the session down to the
    /// reply. Comparing the two replies whole is what catches a handshake that, inside a console,
    /// answered with the instructions and lost the rest.
    #[test]
    fn the_handshake_asks_the_agent_to_publish_only_when_given_instructions() {
        let reply = |instructions| -> serde_json::Value {
            let mut written = Vec::new();
            bridge(
                &b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n"[..],
                &mut written,
                &mut None,
                instructions,
            );
            serde_json::from_slice(&written).expect("the answer is JSON")
        };

        let mut inside = reply(Some(INSTRUCTIONS))["result"].clone();
        let outside = reply(None)["result"].clone();
        assert_eq!(inside["instructions"], INSTRUCTIONS);
        assert!(
            outside.get("capabilities").is_some() && outside.get("instructions").is_none(),
            "outside a console the reply is a handshake and the key is absent, not empty: {outside}"
        );
        inside
            .as_object_mut()
            .expect("the result is an object")
            .remove("instructions");
        assert_eq!(inside, outside, "the instructions are the only difference");
    }

    /// Every snake-case word of the instructions, backticked or not: the tool names they mention.
    /// Every verb this bridge serves has an underscore, which is what tells a tool from a field such
    /// as `attention` or `key`.
    fn tools_the_instructions_name() -> Vec<&'static str> {
        INSTRUCTIONS
            .split(|c: char| !(c.is_ascii_lowercase() || c == '_'))
            .filter(|word| word.contains('_') && !word.starts_with('_') && !word.ends_with('_'))
            .collect()
    }

    /// **The instructions never name a tool this bridge does not serve.** The served names come
    /// from `tools/list` itself, not from a copy, so a verb renamed there or one added to the text
    /// — «plan_set» is served by another terminal's bridge, not this one — fails here.
    #[test]
    fn the_instructions_name_only_tools_this_bridge_serves() {
        let listed = ask("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}");
        let served: Vec<&str> = listed["result"]["tools"]
            .as_array()
            .expect("there is a list of tools")
            .iter()
            .map(|tool| tool["name"].as_str().expect("every tool is named"))
            .collect();
        let named = tools_the_instructions_name();

        for verb in ["panel_set", "panel_get", "panel_patch"] {
            assert!(named.contains(&verb), "the instructions never name {verb}");
        }
        for name in &named {
            assert!(served.contains(name), "{name} is named but not served");
        }
    }

    /// **The attention clause names each attention the contract accepts**, from the contract's own
    /// list. Asserted on the clause and not the whole text: `wait` is also a line mark, and the
    /// sentence about marks would supply a word the clause had lost.
    #[test]
    fn the_attention_clause_names_every_attention_the_contract_accepts() {
        let clause = INSTRUCTIONS
            .split_once("Set `attention` to ")
            .and_then(|(_, rest)| rest.split_once("Put the question"))
            .expect("the instructions have an attention clause")
            .0;
        for attention in Attention::ALL.map(Attention::name) {
            assert!(
                clause.contains(&format!("`{attention}`")),
                "{attention} is not in the attention clause"
            );
        }
    }

    #[test]
    fn the_handshake_answers_with_the_tools_capability() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\
             \"params\":{\"protocolVersion\":\"2024-11-05\"}}");

        assert_eq!(answered["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(answered["result"]["serverInfo"]["name"], "ivy panel");
        assert!(answered["result"]["capabilities"]["tools"].is_object());
    }

    /// **Three verbs answer with the same empty `ok`, and each says which one it was.**
    ///
    /// The word is what an agent reads back, and on this channel publishing and patching are the two
    /// things it must not confuse: one replaced the whole document, the other changed a line of it.
    /// Asserted here rather than through a call because reaching the reply needs a pane, which is
    /// exactly why the first version of this — a bool for the close and «published» for everything
    /// else — shipped saying «published» to a patch.
    #[test]
    fn each_verb_that_answers_an_empty_ok_says_which_one_it_was() {
        assert_eq!(what_it_did(&Ask::Patch(String::new())), "patched");
        assert_eq!(what_it_did(&Ask::Close(String::new())), "closed");
        assert_eq!(
            what_it_did(&Ask::Publish(String::new(), String::new())),
            "published"
        );
    }

    /// **This build advertises every verb it serves**, and the assertion is on the whole list rather
    /// than on the presence of one: a tool advertised before it is answered is a promise taken back
    /// later, and an answered tool left unadvertised is one no client will ever call.
    #[test]
    fn every_served_verb_is_advertised() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}");

        let tools = answered["result"]["tools"]
            .as_array()
            .expect("there is a list of tools");
        let names: Vec<&str> = tools
            .iter()
            .map(|tool| tool["name"].as_str().expect("every tool is named"))
            .collect();
        assert_eq!(
            names,
            [
                "panel_set",
                "panel_get",
                "panel_retire",
                "plan_set",
                "plan_get",
                "plan_retire",
                "panel_patch",
                "split_open",
                "split_read",
                "split_close",
                "image_open"
            ]
        );
        let retire = tools
            .iter()
            .find(|tool| tool["name"] == "panel_retire")
            .expect("retirement is advertised");
        assert_eq!(
            retire["inputSchema"],
            serde_json::json!({"type": "object", "properties": {}})
        );
    }

    /// **`image_open` is advertised as the contract writes it** (D120): its schema, its whole
    /// description, and the clause that keeps a subagent off it — which lives in the description
    /// and not in the handshake, whose instructions name only what publishing needs (D88).
    #[test]
    fn image_open_is_advertised_as_the_contract_writes_it_and_tells_a_subagent_to_leave_it_alone() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}");
        let tool = answered["result"]["tools"]
            .as_array()
            .expect("there is a list of tools")
            .iter()
            .find(|tool| tool["name"] == "image_open")
            .expect("image_open is advertised")
            .clone();
        assert_eq!(
            tool,
            serde_json::json!({
                "name": "image_open",
                "description": "Show an image to the person in the drawer under this console, where they can look at it, mark it up and send it back to this console as a line with the path of an annotated copy. Each console holds one image: this replaces the one this console had, unless the person has marks on it they have not sent, which refuses the call. If the person is working in this console the drawer opens on it at once and the keyboard stays where it was; otherwise nothing moves, and this console's tab and header show that a new image is waiting. PNG or JPEG, at most 32 MiB. If you work for another agent in this same console (a subagent, or a worker it started), do not call this: give the path to that agent.",
                "inputSchema": {
                    "type": "object",
                    "properties": {"path": {"type": "string", "description": "The image file's absolute path."}},
                    "required": ["path"]
                }
            })
        );
        let described = tool["description"].as_str().unwrap();
        assert!(described.contains("If you work for another agent in this same console"));
        assert!(described.contains("do not call this"));
        assert!(!INSTRUCTIONS.contains("image_open"));
    }

    /// The agent's call for `path`, answered with no pane to reach: every answer below is the
    /// bridge's own, given before any channel is opened.
    fn open_an_image(path: serde_json::Value) -> serde_json::Value {
        let call = serde_json::json!({
            "jsonrpc": "2.0", "id": 9, "method": "tools/call",
            "params": {"name": "image_open", "arguments": {"path": path}},
        });
        ask(&call.to_string())["result"].clone()
    }

    /// **The bridge answers a file problem at once, as an error the agent reads, and the pane is
    /// never reached** (D120): each refusal names the file and what is wrong with it.
    #[test]
    fn an_image_the_window_could_not_show_is_refused_by_the_bridge_with_its_reason() {
        let folder = std::env::temp_dir().join(format!("ivy-image-open-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let missing = folder.join("missing.png");
        let oversize = folder.join("huge.png");
        std::fs::File::create(&oversize)
            .unwrap()
            .set_len(panel_control::image::MAX_BYTES + 1)
            .unwrap();
        let shown = |path: &std::path::Path| path.to_str().unwrap().to_owned();
        let refused = |result: serde_json::Value| {
            assert_eq!(result["isError"], true, "{result}");
            result["content"][0]["text"].as_str().unwrap().to_owned()
        };
        assert_eq!(
            refused(open_an_image(serde_json::json!("shot.png"))),
            "image_open needs an absolute path, and shot.png is relative"
        );
        assert_eq!(
            refused(open_an_image(serde_json::json!(shown(&folder)))),
            format!("{} is a folder, not an image", shown(&folder))
        );
        let gone = refused(open_an_image(serde_json::json!(shown(&missing))));
        assert!(
            gone.starts_with(&format!("cannot open {}: ", shown(&missing))),
            "{gone}"
        );
        assert_eq!(
            refused(open_an_image(serde_json::json!(shown(&oversize)))),
            format!("{} is over 32 MiB", shown(&oversize))
        );
        // Not an image by its first bytes, whatever its name: text, a JPEG cut before its third
        // byte, a PNG signature missing its last byte, and nothing at all.
        for (name, bytes) in [
            ("notes.png", b"not an image at all".as_slice()),
            ("cut.jpg", &[0xFF, 0xD8]),
            ("cut.png", &PNG_SIGNATURE[..7]),
            ("empty.png", &[]),
        ] {
            let file = folder.join(name);
            std::fs::write(&file, bytes).unwrap();
            assert_eq!(
                refused(open_an_image(serde_json::json!(shown(&file)))),
                format!("{} is not a PNG or JPEG image", shown(&file))
            );
        }
        assert_eq!(
            refused(open_an_image(serde_json::json!(""))),
            "image_open's \"path\" is empty"
        );
        assert_eq!(
            refused(open_an_image(serde_json::json!(7))),
            "image_open's \"path\" is not text"
        );
        assert_eq!(
            refused(open_an_image(serde_json::Value::Null)),
            "image_open's \"path\" is not text"
        );
        let call = "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"tools/call\",\
             \"params\":{\"name\":\"image_open\",\"arguments\":{}}}";
        assert_eq!(
            refused(ask(call)["result"].clone()),
            "image_open needs a \"path\""
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// **A PNG or a JPEG passes the bridge by its first bytes alone**, whatever its name: the
    /// signature is what the bridge reads, and the decode stays the window's.
    #[test]
    fn an_image_that_begins_as_a_png_or_a_jpeg_passes_the_bridge() {
        let folder = std::env::temp_dir().join(format!("ivy-image-head-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(b"rest");
        for (name, bytes) in [
            ("shot", png.as_slice()),
            ("photo.txt", &[0xFF, 0xD8, 0xFF]),
            ("photo.jpg", &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]),
        ] {
            let file = folder.join(name);
            std::fs::write(&file, bytes).unwrap();
            assert_eq!(image_to_open(file.to_str().unwrap()), Ok(file.clone()), "{name}");
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Something that is neither a file nor a folder — here a FIFO, which would hang a reader — is
    /// refused by name.
    #[cfg(unix)]
    #[test]
    fn an_image_that_is_not_a_regular_file_is_refused_by_the_bridge() {
        let fifo = std::env::temp_dir().join(format!("ivy-image-fifo-{}", std::process::id()));
        let _ = std::fs::remove_file(&fifo);
        let name = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: `name` is a valid NUL-terminated path that outlives the call.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let result = open_an_image(serde_json::json!(fifo.to_str().unwrap()));
        let _ = std::fs::remove_file(&fifo);
        assert_eq!(result["isError"], true);
        assert_eq!(
            result["content"][0]["text"],
            format!("{} is not a regular file", fifo.to_str().unwrap())
        );
    }

    /// **A path on another machine is refused by its text, before the file is asked for**, which is
    /// what would reach that host (D120): a refusal that came from asking would read «cannot open …»
    /// or name what kind of file it found instead.
    #[cfg(windows)]
    #[test]
    fn an_image_on_another_machine_is_refused_by_the_bridge_before_it_is_asked_for() {
        for remote in [
            r"\\host\share\a.png",
            r"\\?\UNC\host\share\a.png",
            r"\\?\unc\host\share\a.png",
            r"\\?\GLOBALROOT\Device\Mup\host\share\a.png",
            r"\\.\PhysicalDrive0",
        ] {
            let result = open_an_image(serde_json::json!(remote));
            assert_eq!(result["isError"], true, "{remote}");
            assert_eq!(
                result["content"][0]["text"],
                format!("{remote} is not on this machine")
            );
        }
    }

    /// The window's sentence is what the agent reads: the bridge adds no word of its own.
    #[test]
    fn an_image_opened_answers_with_the_windows_sentence() {
        let said = "opened; the person is not looking at this console's drawer, so its tab and header mark a new image until they do";
        let result = result_of(
            what_it_did(&Ask::ImageOpen(std::env::temp_dir())),
            Answered::Text(said.to_owned()),
        );
        assert_eq!(result["content"][0]["text"], said);
        assert!(result.get("isError").is_none());
    }

    /// **An open answers with its handle, or says it has none** — never with the verb's word. An open
    /// is asked under the same word as a publication, so a handle-less open that reached the empty
    /// `ok` would read «published».
    #[test]
    fn an_open_answers_with_its_handle_or_says_it_has_none() {
        let said = |answered| result_of("published", answered)["content"][0]["text"].clone();

        assert_eq!(said(Answered::Opened(Some("h-1".to_owned()))), "h-1");
        assert_eq!(
            said(Answered::Opened(None)),
            "opened, with no addressable identity"
        );
    }

    /// **The verb's word first, then what fell away**, for both verbs that can lose a word — and a
    /// clean answer is the bare word, so the two can never be confused.
    #[test]
    fn a_lossy_answer_is_the_verbs_word_and_then_what_fell_away() {
        let said = |did, answered| result_of(did, answered)["content"][0]["text"].clone();

        assert_eq!(
            said(
                "published",
                Answered::Lossy(r#"4 lines carry an unknown mark "doing""#.to_owned())
            ),
            r#"published; 4 lines carry an unknown mark "doing""#
        );
        assert_eq!(
            said(
                "patched",
                Answered::Lossy("1 line carries an unknown mark 42".to_owned())
            ),
            "patched; 1 line carries an unknown mark 42"
        );
        assert_eq!(said("published", Answered::Ok), "published");
        assert!(
            result_of(
                "published",
                Answered::Lossy("1 line carries an unknown mark 42".to_owned())
            )
            .get("isError")
            .is_none(),
            "a lossy publication is still a success"
        );
    }

    /// **The description names each vocabulary under its own field**, asserted clause by clause:
    /// three of the words belong to two vocabularies, so a check word by word over the whole text
    /// passes with any of them missing from one list.
    #[test]
    fn the_publishing_description_names_each_vocabulary_under_its_own_field() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}");
        let described = |tool: &str| -> String {
            answered["result"]["tools"]
                .as_array()
                .expect("there is a list of tools")
                .iter()
                .find(|described| described["name"] == tool)
                .and_then(|described| described["description"].as_str())
                .expect("the tool is described")
                .to_owned()
        };
        let publishing = described("panel_set");

        for clause in [
            "\"attention\" is wait, block or done;",
            "a line's \"mark\" is done, current, pending, wait, block or info;",
            "its \"style\" is normal, dim, title, accent, ok, warn, error or section.",
            "Any other non-null value is dropped",
        ] {
            assert!(
                publishing.contains(clause),
                "panel_set's description does not say {clause:?}: {publishing}"
            );
        }
        assert!(
            described("panel_patch").contains("named in the reply"),
            "panel_patch's description does not say a dropped word is named"
        );
    }

    #[test]
    fn roadmap_tool_schemas_and_descriptions_pin_the_cross_repository_contract() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}");
        let tools = answered["result"]["tools"].as_array().unwrap();
        let tool = |name: &str| tools.iter().find(|tool| tool["name"] == name).unwrap();
        assert_eq!(
            tool("plan_set")["inputSchema"]["required"],
            serde_json::json!(["document"])
        );
        assert_eq!(
            tool("plan_set")["inputSchema"]["properties"]["repo"]["type"],
            "string"
        );
        for (name, clauses) in [
            (
                "plan_set",
                vec![
                    "this console's project",
                    "ignored",
                    "takes precedence",
                    "persists",
                    "done, current, pending, wait or block",
                ],
            ),
            (
                "plan_get",
                vec!["this console's pane", "takes precedence", "persisted"],
            ),
            (
                "plan_retire",
                vec!["this console's pane", "configured file roadmap", "persist"],
            ),
        ] {
            let description = tool(name)["description"].as_str().unwrap();
            for clause in clauses {
                assert!(
                    description.contains(clause),
                    "{name} misses {clause:?}: {description}"
                );
            }
        }
    }

    #[test]
    fn roadmap_tools_are_dispatched_and_validate_required_documents_before_connecting() {
        let missing = ask("{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\
             \"params\":{\"name\":\"plan_set\",\"arguments\":{\"repo\":\"/ignored\"}}}");
        assert_eq!(missing["result"]["isError"], true);
        assert!(
            missing["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("document")
        );
        let dispatched = ask("{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\
             \"params\":{\"name\":\"plan_set\",\"arguments\":{\"document\":{\"milestones\":[]},\"repo\":\"/ignored\"}}}");
        assert!(
            dispatched.get("error").is_none(),
            "plan_set was advertised but not dispatched: {dispatched}"
        );
        for name in ["plan_get", "plan_retire"] {
            let dispatched = ask(&format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{{\"name\":\"{name}\",\"arguments\":{{\"repo\":\"/ignored\"}}}}}}"
            ));
            assert!(
                dispatched.get("error").is_none(),
                "{name} was advertised but not dispatched: {dispatched}"
            );
        }
    }

    /// The MCP result is a literal cross-repository contract, including its count labels.
    #[test]
    fn retirement_result_names_both_outcomes_exactly() {
        assert_eq!(
            retirement_result(2, 1),
            "retired; closed 2 auxiliary console(s); preserved 1 taken-over console(s)"
        );
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

    /// **And a patch whose `ops` are missing or are not a list is refused here, without a pane.**
    ///
    /// The point is the second half: finding out that an argument is wrong must not need a channel,
    /// so this runs with no pane at all and still gets an answer. What each op *says* is not judged
    /// here — that is the contract's, at the other end — so a well-shaped list of nonsense is not in
    /// this test, and would not be refused by this code.
    #[test]
    fn a_patch_whose_ops_are_missing_or_not_a_list_comes_back_as_a_tool_error() {
        for arguments in [
            "{}",
            "{\"ops\":{\"op\":\"delete\"}}",
            "{\"ops\":\"delete\"}",
        ] {
            let answered = ask(&format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\
                 \"params\":{{\"name\":\"panel_patch\",\"arguments\":{arguments}}}}}"
            ));

            assert!(answered.get("error").is_none(), "{answered}");
            assert_eq!(
                answered["result"]["isError"], true,
                "a patch with arguments {arguments} was not refused"
            );
        }
    }

    /// A tool that does not exist is a protocol error, because the client asked for something this
    /// server never advertised.
    #[test]
    fn a_tool_that_was_never_advertised_is_a_protocol_error() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\
             \"params\":{\"name\":\"split_dance\",\"arguments\":{}}}");

        assert_eq!(answered["error"]["code"], -32602);
    }

    /// **A `split_open` with no side is a tool error and not a broken session**, the same shape as a
    /// publication with no document: the agent reads it and carries on.
    #[test]
    fn a_split_open_missing_its_side_comes_back_as_a_tool_error() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\
             \"params\":{\"name\":\"split_open\",\"arguments\":{}}}");

        assert!(answered.get("error").is_none(), "{answered}");
        assert_eq!(answered["result"]["isError"], true);
    }

    /// **Every verb is dispatched and not just advertised**: a side this build does not know is
    /// refused by name, and it is refused before the pane is even reached — which is why the refusal
    /// arrives with no channel open in this test.
    #[test]
    fn an_unknown_side_is_refused_by_name_without_reaching_the_pane() {
        let answered = ask("{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\
             \"params\":{\"name\":\"split_open\",\"arguments\":{\"side\":\"diagonal\"}}}");

        assert_eq!(answered["result"]["isError"], true);
        assert!(
            answered["result"]["content"][0]["text"]
                .as_str()
                .is_some_and(|text| text.contains("no side")),
            "{answered}"
        );
    }

    #[test]
    fn a_line_that_is_not_json_is_answered_rather_than_swallowed() {
        let mut nowhere = None;
        let answered = answer("not json", &mut nowhere, None).expect("it deserved an answer");
        let answered: serde_json::Value = serde_json::from_str(&answered).expect("JSON");

        assert_eq!(answered["error"]["code"], -32700);
    }
}
