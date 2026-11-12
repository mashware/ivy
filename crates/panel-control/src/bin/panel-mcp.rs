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
//! **It exposes the seven verbs the channel serves**: `panel_set`, `panel_get`, `panel_patch`, the
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
/// server has no behaviour that varies with the version — it advertises the same seven operations
/// and forwards each call through the pane's channel — so refusing a client over a date would break
/// the connection for a difference that does not exist anywhere in this file.
const PROTOCOL_VERSION: &str = "2025-06-18";

fn main() {
    // **Opened lazily, on the first call that needs it.** A client that only lists the tools must
    // not fail because nothing is listening yet, and an agent's editor lists tools long before it
    // has anything to publish.
    let mut pane: Option<Producer> = None;
    // **The one read of the environment**, and it stays here: a decision taken further down would
    // inherit the identity of whatever console runs the test suite.
    let [id, dir] = panel_contract::ENV_NAMES.map(std::env::var_os);
    let inside = inside_a_console(id.as_deref(), dir.as_deref());

    bridge(
        std::io::stdin().lock(),
        std::io::stdout(),
        &mut pane,
        inside,
    );
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
    inside: bool,
) {
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
            Ok(line) => answer(line, pane, inside),
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
fn answer(line: &str, pane: &mut Option<Producer>, inside: bool) -> Option<String> {
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
        "initialize" => succeeded(&id, initialized(&message, inside)),
        "tools/list" => succeeded(&id, tools()),
        "tools/call" => called(&id, &message, pane),
        "ping" => succeeded(&id, serde_json::json!({})),
        other => failed(&id, -32601, &format!("no method \"{other}\"")),
    })
}

/// Whether this bridge runs inside an Ivy console: both identity variables present **and
/// non-empty**.
///
/// **Empty counts as absent**, which is the rule the session helper and `ivy-session` already
/// apply to the same two variables: Ivy itself always sets both or neither, but a client
/// configuration that interpolates `${PANEL_ID:-}` hands the bridge an empty value, and a bridge
/// that cannot reach a pane must not tell its agent that a panel is waiting for it.
///
/// **It takes the values instead of reading them** so it can be tested without `set_var`, which
/// is undefined behaviour in a suite that launches processes in parallel. `main` is the one place
/// that reads the environment, and no test covers that line.
fn inside_a_console(id: Option<&std::ffi::OsStr>, dir: Option<&std::ffi::OsStr>) -> bool {
    id.is_some_and(|id| !id.is_empty()) && dir.is_some_and(|dir| !dir.is_empty())
}

/// **What every agent is told when it connects inside an Ivy console**, whatever harness or
/// workflow it runs. The tools alone were not enough: a harness may defer them — listed by name,
/// no schema — and an agent that never loads them publishes nothing, so the person sees an empty
/// panel. Harnesses place these words in the agent's own instructions, the one place every agent
/// reads.
///
/// It names only the publishing verbs: the `split_*` and retirement descriptions already explain
/// the rest, and these words land in every agent's prompt. «This server's tools» is deliberate — a
/// second panel server may be registered beside this one and serve the same names.
const INSTRUCTIONS: &str = "\
This session runs in an Ivy console, and Ivy paints a status panel beside it. It shows only what \
you publish: publish nothing and the person sees an empty panel.

- Publish with this server's `panel_set` (`source`: your name or your workflow's) when you start a \
task, before every long stretch and at every stop. The tools may be deferred: load them first.
- Set `attention` to `wait` whenever you need the person, and put the question on a line marked \
`wait`; `block` when stuck on something else; `done` when finished. Leave it out while you work, \
and republish without it as soon as it stops being true.
- Between publications, update lines by `key` with `panel_patch`. After a restart or a compaction, \
call `panel_get` before publishing again.
- When a tool or workflow you run renders a panel document for you (JSON with `lines`), pass it to \
`panel_set` as it was printed.";

/// What this server says it is, and — only inside a console — what it asks of the agent.
fn initialized(message: &serde_json::Value, inside: bool) -> serde_json::Value {
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
    // Outside a console the calls are refused, so promising a panel there would be false.
    if inside {
        answer["instructions"] = serde_json::Value::from(INSTRUCTIONS);
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

/// The seven tools this build serves, with the shape of what each takes.
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
            }
        ],
    })
}

/// What one tool call asks for, **already read and validated**.
///
/// **It exists so the channel is opened after the arguments are understood**, and not before: a
/// malformed call is a tool error the agent reads, and finding it out must not require a pane.
enum Ask {
    Publish(String, String),
    ReadTheDocument,
    Retire,
    Patch(String),
    Open(Side, Option<String>, Vec<String>),
    Read(String, bool),
    Close(String),
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
        Ask::Patch(_) => "patched",
        Ask::Retire => "retired",
        Ask::Publish(..) | Ask::ReadTheDocument | Ask::Open(..) | Ask::Read(..) => "published",
    }
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
    // **A tool this server never advertised is a protocol error**, and it is checked before the pane
    // is even reached: a call for something nobody serves must not open the channel to find that out.
    if !matches!(
        name,
        "panel_set"
            | "panel_get"
            | "panel_retire"
            | "panel_patch"
            | "split_open"
            | "split_read"
            | "split_close"
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
    let producer = match open(pane) {
        Ok(producer) => producer,
        Err(failure) => return succeeded(id, refusal(&failure)),
    };

    let did = what_it_did(&ask);
    let answer = match ask {
        Ask::Publish(source, document) => producer.publish(&source, &document),
        Ask::ReadTheDocument => producer.read_the_document(),
        Ask::Retire => producer.retire(),
        Ask::Patch(ops) => producer.patch(&ops),
        Ask::Open(side, program, args) => producer.open_a_pane(side, program.as_deref(), &args),
        Ask::Read(handle, scrollback) => producer.read_a_pane(&handle, scrollback),
        Ask::Close(handle) => producer.close_a_pane(&handle),
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
/// **Out of [`called`] so a test reaches it without a pane**, for the reason [`what_it_did`] gives:
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

    fn ask(message: &str) -> serde_json::Value {
        let mut nowhere = None;
        let answered = answer(message, &mut nowhere, false).expect("it deserved an answer");
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

        bridge(&input[..], &mut written, &mut None, false);

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

        bridge(&input[..], &mut written, &mut None, false);

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

        bridge(&input[..], &mut written, &mut None, false);

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
                false
            )
            .is_none()
        );
    }

    /// **Only a whole identity is a console.** Each row breaks one half, absent or empty; a version
    /// that checked one variable, or presence only, passes the first row and fails one of the rest.
    #[test]
    fn a_console_is_both_variables_present_and_non_empty() {
        use std::ffi::OsStr;
        let id = Some(OsStr::new("a1b2c3d4"));
        let dir = Some(OsStr::new("/run/user/1000/ivy/a1b2c3d4"));
        let empty = Some(OsStr::new(""));

        assert!(inside_a_console(id, dir));
        assert!(!inside_a_console(None, dir), "no identity");
        assert!(!inside_a_console(id, None), "no directory");
        assert!(!inside_a_console(empty, dir), "an empty identity");
        assert!(!inside_a_console(id, empty), "an empty directory");
    }

    /// **The handshake carries the instructions inside a console and nothing outside**, asserted
    /// through `answer` so the flag is proven to travel from the session down to the reply, not
    /// only to work when `initialized` is called by hand.
    #[test]
    fn the_handshake_asks_the_agent_to_publish_only_inside_a_console() {
        let initialize = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}";
        let reply = |inside| -> serde_json::Value {
            let answered = answer(initialize, &mut None, inside).expect("answered");
            serde_json::from_str(&answered).expect("the answer is JSON")
        };

        assert_eq!(reply(true)["result"]["instructions"], INSTRUCTIONS);
        assert!(
            reply(false)["result"].get("instructions").is_none(),
            "outside a console the key is absent, not empty"
        );
    }

    /// The backticked `snake_case` spans of the instructions: the tool names they mention. Every
    /// verb this bridge serves has an underscore, which is what lets the extraction tell a tool from
    /// a field such as `attention` or `key`.
    fn tools_the_instructions_name() -> Vec<&'static str> {
        INSTRUCTIONS
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|span| {
                span.contains('_') && span.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            })
            .collect()
    }

    /// **The instructions never name a tool this bridge does not serve.** The served names come
    /// from `tools/list` itself, not from a copy, so a verb renamed there or one added to the text
    /// — `plan_set` is served by another terminal's bridge, not this one — fails here.
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

    /// **Each attention the contract accepts is named, backticked**, from the contract's own list,
    /// so a word added to or renamed in the vocabulary is not left out of what agents are told.
    #[test]
    fn the_instructions_name_every_attention_the_contract_accepts() {
        for attention in Attention::ALL.map(Attention::name) {
            assert!(
                INSTRUCTIONS.contains(&format!("`{attention}`")),
                "{attention} is not in the instructions"
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

    /// **This build advertises the seven verbs it serves**, and the assertion is on the whole list rather
    /// than on the presence of one: a tool advertised before it is answered is a promise taken back
    /// later, and an answered tool left unadvertised is one no client will ever call.
    #[test]
    fn the_seven_verbs_this_build_serves_are_advertised() {
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
                "panel_patch",
                "split_open",
                "split_read",
                "split_close"
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
            "its \"style\" is normal, dim, title, accent, ok, warn or error.",
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
        let answered = answer("not json", &mut nowhere, false).expect("it deserved an answer");
        let answered: serde_json::Value = serde_json::from_str(&answered).expect("JSON");

        assert_eq!(answered["error"]["code"], -32700);
    }
}
