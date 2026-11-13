//! What travels over the channel: newline-delimited JSON, one request per line.
//!
//! **The framing is a line and not a length prefix**, because the producer on the other side is the
//! same kind of thing that writes `panel.json` by hand: a shell script with `printf` has to be able
//! to speak this, and a length prefix takes that away for nothing in return — the document's ceiling
//! already bounds a line.
//!
//! **Cutting the lines is separated from reading the socket**, and that is what makes the connection
//! able to end. A reader that blocks inside `read` until a line is complete cannot be told its pane
//! has closed, and an idle agent holding its channel open is not an edge case here — it is the
//! ordinary state of the thing this channel exists for. So the socket is read in whatever chunks it
//! gives, [`Framing`] is fed them, and everything below is decided without any I/O at all.

use std::io::{self, Write};
use std::time::Duration;

use panel_contract::{Dropped, Patch};
use serde_json::value::RawValue;

/// The most a source name may take **on the wire**.
///
/// **It is bounded so the envelope is bounded, and the bound is what keeps a legal document from
/// being refused.** Without it, a producer with a long name and a document of exactly the ceiling
/// produced a line past [`MAX_REQUEST`], which is an `Unrecoverable`: the connection ended for a
/// publication the file would have accepted. A source is a label — flow, ci, an agent's name — and
/// two hundred and fifty-six bytes is past any of them. **Counted on the encoded form**, quotes and
/// escapes included, because that is what the line has to hold.
const MAX_SOURCE: usize = 256;

/// The most a `split_open`'s program name, and the whole of its argument list, may take **on the
/// wire**.
///
/// **They are bounded because these bytes end up in `execve`.** The transport's own ceiling would
/// allow a single argument of a quarter of a megabyte, which the kernel refuses with `E2BIG` — and
/// the caller would read that as an unexplained launch failure rather than as a request it can fix.
/// The bound is on the encoded form, like [`MAX_SOURCE`], and a value over it is malformed: the
/// connection survives and the caller can retry shorter. The numbers are generous past any real
/// command line and far below `ARG_MAX`.
const MAX_PROGRAM: usize = 1024;
const MAX_ARGS: usize = 16 * 1024;

/// Room the request's own keys and the source name may take on top of the document.
///
/// **It exists so the envelope does not count against the document's ceiling.** The ceiling that
/// matters is the contract's, and it is enforced by the parser on the document itself
/// ([`panel_contract::parse_document`]); this module only has to keep one line from reserving more
/// memory than a document plus its envelope can legitimately be. Measuring the whole line against
/// [`panel_contract::MAX_BYTES`] did the opposite: a document of exactly the ceiling —which the file
/// accepts— produced a line the channel refused, and ended the connection for it. That is the
/// two-transport drift the contract forbids, seen from the other side. One KiB is the fixed keys
/// (`tool`, `source`, `document`, punctuation) plus [`MAX_SOURCE`] many times over.
const ENVELOPE: usize = 1024;

/// The most one request line may hold.
///
/// **It is a transport bound, not the document's ceiling**, and the difference is the whole of the
/// paragraph above: the document's ceiling lives in
/// [`panel_contract::parse_document`](panel_contract::parse_document), and this number is only what
/// stops an unfinished line from reserving without limit.
pub const MAX_REQUEST: usize = panel_contract::MAX_BYTES as usize + ENVELOPE;

/// Which edge of the caller's pane a new one is opened on.
///
/// **Declared here and not imported from the tiling module**, because this crate is the wire's and
/// knows nothing visual (D3): the format names four strings and the app translates them into its own
/// `Side` at the boundary, so a rename inside the tiling cannot silently rename the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// The string the frozen format carries, or `None` for one this build does not know.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "top" => Some(Self::Top),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }

    /// The one string that names this side back to a caller.
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

/// Activity declared by the OpenCode TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Activity {
    Busy,
    Idle,
    Unknown,
}

/// Complete, pane-local OpenCode metadata. Session is inert, bounded metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSnapshot {
    pub session: String,
    pub activity: Activity,
    pub permission: bool,
    pub question: bool,
    /// The last complete claim is unconfirmed while authoritative refresh is unavailable.
    #[serde(default)]
    pub stale: bool,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentEnvelope {
    tool: String,
    v: u8,
    client: String,
    snapshot: Option<AgentSnapshot>,
}

/// A request, once it has been understood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// An independent OpenCode signal, never a panel publication.
    AgentSignal(Option<AgentSnapshot>),
    /// Publish this document on the pane this connection opened, under this source.
    ///
    /// The document travels as **its own text** and not as an interpreted value: the contract's
    /// parser is the only one that turns text into a panel, and handing it anything else would be a
    /// second interpretation of the same bytes — which is exactly how two transports of one contract
    /// start disagreeing field by field. It is the producer's own bytes, kept as a [`RawValue`], so
    /// the parser's ceiling measures what arrived and not a compact re-serialisation of it.
    PanelSet { source: String, document: String },
    /// Publish a project roadmap document. Project scope is supplied by the caller's pane; `repo`
    /// is deliberately not part of the request and cannot widen that scope.
    PlanSet { document: String },
    /// Read the published roadmap for the caller's pane project.
    PlanGet,
    /// Remove the published roadmap for the caller's pane project.
    PlanRetire,
    /// Hand back the document this pane's panel is showing.
    ///
    /// **It names nothing, and it cannot**: the connection's own endpoint already decided which pane
    /// this is, so there is no word in this transport for asking about another. **And it asks for no
    /// hold**, which is the asymmetry that makes it worth having — the producer it exists for is the
    /// one that has *not* claimed the pane: an agent that restarted, one whose context was compacted
    /// away, a second tool taking a console over.
    PanelGet,
    /// Intentionally end the session that currently holds this pane's panel.
    ///
    /// It carries no identity: the endpoint names the pane and the connection is the authority.
    PanelRetire,
    /// Change part of this pane's published document, leaving the rest of it alone.
    ///
    /// **The ops arrive read**, which is the whole shape of this verb: everything decidable without
    /// the document — an op nobody serves, a field that is not patchable, a value of the wrong kind,
    /// more ops than the ceiling allows — is decided on the connection's own thread by
    /// [`panel_contract::parse_ops_noting`], and never in a frame. What travels is the one question that
    /// needs the panel: does this key name a line there.
    ///
    /// **It carries no `source`**, unlike a publication. The hold already names its holder and a
    /// patch cannot exist without a hold, so a name here would either reach nothing — which is what
    /// `split_open`'s absent `env` is on record for refusing — or rename a holder mid-hold, which
    /// nobody asked for.
    ///
    /// **What the carried lines let fall travels beside the ops and never reaches the window**: it is
    /// read here, where the ops are, and the listener puts it on the reply once the window has taken
    /// the patch (**D58**).
    PanelPatch { ops: Patch, dropped: Dropped },
    /// Open a pane beside the caller's own, optionally running a program of the caller's choosing.
    ///
    /// **There is no environment here, and there cannot be one**: a pane's environment is exactly
    /// what [`panel_contract`] injects at launch, so `env` is not a field of this request's envelope
    /// and a value sent under that name reaches nothing. The program and its arguments are the whole
    /// of what a caller may choose.
    SplitOpen {
        side: Side,
        program: Option<String>,
        args: Vec<String>,
    },
    /// Read a pane this connection opened, the visible screen unless the scrollback is asked for.
    SplitRead { pane: String, scrollback: bool },
    /// Close a pane this connection opened.
    SplitClose { pane: String },
    /// A tool this build does not serve, carried by its name so the refusal can say which.
    NotServed(String),
}

/// What one complete line produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// Metadata traffic, including validation failures, never enters the UI ownership protocol.
    Session(Result<crate::session::Request, String>),
    /// A request to answer.
    Request(Request),
    /// The line could not be understood. The connection survives: one malformed line from an agent
    /// writing JSON by hand is not a reason to take its channel away.
    Malformed(String),
    /// A line this stream cannot be resynchronised after.
    ///
    /// **A line over the ceiling ends the connection rather than being skipped**, and that is not
    /// severity for its own sake: what was held is the *start* of somebody's line, so whatever comes
    /// next is its tail and would be read as a fresh request. There is no way back to a boundary
    /// that was never seen.
    Unrecoverable(String),
}

/// Cuts a stream of bytes into requests, holding no more than [`MAX_REQUEST`] of an unfinished one.
#[derive(Debug, Default)]
pub struct Framing {
    held: Vec<u8>,
    over: Over,
    /// How long the line being accumulated is, **carried instead of rescanned**.
    ///
    /// The first version walked the whole buffer for a newline on every `feed`. Inside the
    /// application a 4 096-byte read kept that cheap, but `Framing` is public API and a same-user
    /// caller writing one unfinished line a byte at a time made it O(n²) — a 256 KiB line reached
    /// some three times ten to the tenth comparisons and pinned the pane's thread before the ceiling
    /// ever tripped. This is the tail after the last newline, updated as the bytes arrive.
    line_len: usize,
}

/// What a framing does once the ceiling has been passed.
///
/// **Three states and not a flag, because the ceiling is only a ceiling if it is terminal.** A flag
/// cleared on the way out let the next chunk of the same over-long line look like a fresh request —
/// which is resynchronising onto a boundary that was never seen, the one thing the refusal exists to
/// prevent. Keeping it set instead would report the overrun again on every chunk. So: it is said
/// once, and after that nothing is accumulated and nothing more is said.
#[derive(Debug, Default, PartialEq, Eq)]
enum Over {
    #[default]
    Fine,
    /// Passed, and not yet told to the caller.
    Passed,
    /// Passed and told. This framing is finished.
    Told,
}

impl Framing {
    /// Adds what the socket gave, whatever shape it came in.
    pub fn feed(&mut self, chunk: &[u8]) {
        if self.over != Over::Fine {
            return;
        }
        self.held.extend_from_slice(chunk);
        // **What is measured is a single line, complete or unfinished, and never the buffer.** A
        // caller may legitimately deposit several short requests in one chunk, so measuring the
        // buffer would refuse it; measuring only the tail after the last newline — what this did
        // first — let a complete over-long line through, because its tail is empty. The length is
        // carried as the bytes arrive, so a line fed in thousands of small chunks costs what a line
        // fed in one costs.
        for byte in chunk {
            if *byte == b'\n' {
                if self.line_len > MAX_REQUEST {
                    self.over = Over::Passed;
                    self.held = Vec::new();
                    self.line_len = 0;
                    return;
                }
                self.line_len = 0;
            } else {
                self.line_len += 1;
            }
        }
        if self.line_len > MAX_REQUEST {
            self.over = Over::Passed;
            // The complete requests ahead of it go too. The connection ends on the refusal below,
            // so there is nobody left to answer them.
            self.held = Vec::new();
            self.line_len = 0;
        }
    }

    /// Takes the next complete request out of what has been fed, if there is one.
    pub fn take(&mut self) -> Option<Incoming> {
        match self.over {
            Over::Passed => {
                self.over = Over::Told;
                return Some(Incoming::Unrecoverable(
                    "the request goes over the ceiling".to_owned(),
                ));
            }
            Over::Told => return None,
            Over::Fine => {}
        }
        let end = self.held.iter().position(|byte| *byte == b'\n')?;
        let line: Vec<u8> = self.held.drain(..=end).collect();
        Some(interpret(&line))
    }
}

/// A request as it arrives: the tool, and the two fields `panel_set` needs.
///
/// **Both the source and the document are [`RawValue`]s.** The document keeps the producer's own
/// text, so the parser's ceiling is measured on what was sent and not on a compact re-serialisation
/// — the drift the review found. The source is raw for a narrower reason: **the bound is on the wire
/// bytes**, and decoding first would count a name of 256 escaped scalars as 256 bytes when it takes
/// 1 536 on the wire, pushing a ceiling document past the transport's bound.
#[derive(serde::Deserialize)]
struct Envelope<'a> {
    tool: String,
    #[serde(default, borrow)]
    source: Option<&'a RawValue>,
    #[serde(default, borrow)]
    document: Option<&'a RawValue>,
    #[serde(default, borrow)]
    repo: Option<&'a RawValue>,
    #[serde(default, borrow)]
    side: Option<&'a RawValue>,
    #[serde(default, borrow)]
    program: Option<&'a RawValue>,
    #[serde(default, borrow)]
    args: Option<&'a RawValue>,
    #[serde(default, borrow)]
    pane: Option<&'a RawValue>,
    #[serde(default, borrow)]
    scrollback: Option<&'a RawValue>,
    #[serde(default, borrow)]
    ops: Option<&'a RawValue>,
}

/// Turns one line into a request.
///
/// **Nothing here is tolerated per field, unlike the document it carries.** A panel document is
/// somebody's summary and losing it over a field nobody looks at would be the worst thing to do
/// with it; a request is an instruction, and one whose tool cannot be read has no sensible
/// interpretation at all.
fn interpret(line: &[u8]) -> Incoming {
    let Ok(text) = std::str::from_utf8(line) else {
        return Incoming::Malformed("the request is not UTF-8 text".to_owned());
    };
    // An empty line is somebody's newline, not a request. It is refused as malformed rather than
    // taken as a publication, so a stray `\n` cannot claim a pane.
    if text.trim().is_empty() {
        return Incoming::Malformed("empty request".to_owned());
    }

    let envelope: Envelope = match serde_json::from_str(text) {
        Ok(envelope) => envelope,
        Err(failure) => {
            return Incoming::Malformed(format!("the request could not be read ({failure})"));
        }
    };

    if envelope.tool == "agent_signal" {
        return agent_signal(text);
    }

    match envelope.tool.as_str() {
        "session_observe" | "session_attach" | "session_probe" | "session_update" => {
            Incoming::Session(crate::session::Request::parse(
                text.trim_end_matches('\n').as_bytes(),
            ))
        }
        "panel_set" => panel_set(&envelope),
        "plan_set" => plan_set(&envelope),
        "plan_get" => Incoming::Request(Request::PlanGet),
        "plan_retire" => Incoming::Request(Request::PlanRetire),
        "panel_get" => Incoming::Request(Request::PanelGet),
        "panel_retire" => Incoming::Request(Request::PanelRetire),
        "panel_patch" => panel_patch(&envelope),
        "split_open" => split_open(&envelope),
        "split_read" => split_read(&envelope),
        "split_close" => split_close(&envelope),
        other => Incoming::Request(Request::NotServed(other.to_owned())),
    }
}

/// The plan document stays as the exact JSON text received, just like a panel publication.
/// `repo` is accepted as an envelope field for producer parity, but scope always comes from the
/// endpoint's pane and the value is intentionally ignored.
fn plan_set(envelope: &Envelope<'_>) -> Incoming {
    let Some(document) = envelope.document else {
        return Incoming::Malformed("a plan publication carries no \"document\"".to_owned());
    };
    let _ = envelope.repo;
    Incoming::Request(Request::PlanSet {
        document: document.get().to_owned(),
    })
}

fn agent_signal(text: &str) -> Incoming {
    // Option normally accepts an absent field; require explicit null or a complete snapshot.
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(_) => return Incoming::Malformed("invalid agent signal".to_owned()),
    };
    if value.get("snapshot").is_none() {
        return Incoming::Malformed("agent signal requires snapshot".to_owned());
    }
    let envelope: AgentEnvelope = match serde_json::from_str(text) {
        Ok(envelope) => envelope,
        Err(_) => return Incoming::Malformed("invalid agent signal".to_owned()),
    };
    if envelope.tool != "agent_signal" || envelope.v != 1 || envelope.client != "opencode" {
        return Incoming::Malformed("unsupported agent signal".to_owned());
    }
    if envelope.snapshot.as_ref().is_some_and(|s| {
        s.session.is_empty() || s.session.len() > 256 || s.session.chars().any(char::is_control)
    }) {
        return Incoming::Malformed("invalid agent session".to_owned());
    }
    Incoming::Request(Request::AgentSignal(envelope.snapshot))
}

/// The publication, understood at the wire.
///
/// **The three `split_*` verbs are parsed here too, even though the window answers them**: the shape
/// is what lets a refusal name the verb, and it is what pins that `split_open` carries no `env`.
fn panel_set(envelope: &Envelope<'_>) -> Incoming {
    let Some(source) = envelope.source else {
        return Incoming::Malformed("a publication names no \"source\"".to_owned());
    };
    // **The bound is on the wire bytes and not the decoded name**, because it is the wire that has
    // to fit: a name of 256 scalars written as `\uXXXX` escapes takes 1 536 bytes and would push a
    // document at the ceiling past [`MAX_REQUEST`], which is the connection-end this bound exists to
    // prevent. It is checked before the document is even looked at, and refused as malformed so the
    // connection lives on and the producer can retry under a shorter name.
    if source.get().len() > MAX_SOURCE {
        return Incoming::Malformed(format!(
            "a publication's \"source\" is over {MAX_SOURCE} bytes"
        ));
    }
    let Ok(source) = serde_json::from_str::<String>(source.get()) else {
        return Incoming::Malformed("a publication's \"source\" is not text".to_owned());
    };
    let Some(document) = envelope.document else {
        return Incoming::Malformed("a publication carries no \"document\"".to_owned());
    };

    Incoming::Request(Request::PanelSet {
        source,
        // **The producer's own text, not a re-serialisation of it**, so the parser's ceiling is
        // measured against the bytes that arrived and the two transports cannot drift. It is paid on
        // the connection's own thread, never in the frame.
        document: document.get().to_owned(),
    })
}

/// The patch, understood at the wire.
///
/// **Its refusals are `Malformed`**, like every other request's: a patch whose ops cannot be read is
/// one line an agent got wrong, and taking its channel away for it would be the connection-ending
/// reserved for a line whose boundary was never seen. **Reaching this function at all is the part
/// that has to be bought**, and the bound below is what buys it — a line past [`MAX_REQUEST`] is
/// `Unrecoverable` before any of this runs.
fn panel_patch(envelope: &Envelope<'_>) -> Incoming {
    let Some(ops) = envelope.ops else {
        return Incoming::Malformed("a patch carries no \"ops\"".to_owned());
    };
    // **The ops are bounded on the wire for the reason `source` is, and it is the same failure.**
    // [`MAX_REQUEST`] is the document's ceiling plus an envelope sized for fixed keys and a name;
    // ops are neither, and nothing else holds them. Without this, a producer rewriting sixty keyed
    // lines of a near-ceiling document sends a line past that bound — which is `Unrecoverable`, so
    // its connection ends and its hold with it, and the panel falls back to its file. That is the
    // whole cost this verb exists to remove, charged for using it. Checked on the wire bytes, and
    // refused as malformed so the connection lives and the producer can send fewer ops.
    if ops.get().len() > panel_contract::MAX_BYTES as usize {
        return Incoming::Malformed(format!(
            "a patch's \"ops\" are over {} KiB",
            panel_contract::MAX_BYTES / 1024
        ));
    }
    let Ok(ops) = serde_json::from_str::<serde_json::Value>(ops.get()) else {
        return Incoming::Malformed("a patch's \"ops\" could not be read".to_owned());
    };
    // **Read here and not at the pane**, which is what keeps this verb's work inside the frame down
    // to the one question the document answers. The sentence is the contract's own, so a producer
    // reads the same words whichever transport ever carries these ops.
    match panel_contract::parse_ops_noting(&ops) {
        Ok((ops, dropped)) => Incoming::Request(Request::PanelPatch { ops, dropped }),
        Err(problem) => Incoming::Malformed(problem.to_string()),
    }
}

/// `split_open`: the side is required, the program and its arguments are not, and **there is no
/// `env`** — the envelope has no such field, so a value sent under that name is never read.
fn split_open(envelope: &Envelope<'_>) -> Incoming {
    let Some(side) = envelope.side else {
        return Incoming::Malformed("a split_open names no \"side\"".to_owned());
    };
    let Ok(side) = serde_json::from_str::<String>(side.get()) else {
        return Incoming::Malformed("a split_open's \"side\" is not text".to_owned());
    };
    let Some(side) = Side::parse(&side) else {
        return Incoming::Malformed("a split_open names no side this build knows".to_owned());
    };

    let program = match envelope.program {
        None => None,
        Some(raw) => {
            // Measured before it is decoded, like the source: it is the wire that has to fit.
            if raw.get().len() > MAX_PROGRAM {
                return Incoming::Malformed(format!(
                    "a split_open's \"program\" is over {MAX_PROGRAM} bytes"
                ));
            }
            match serde_json::from_str::<String>(raw.get()) {
                Ok(program) => Some(program),
                Err(_) => {
                    return Incoming::Malformed(
                        "a split_open's \"program\" is not text".to_owned(),
                    );
                }
            }
        }
    };
    let args = match envelope.args {
        None => Vec::new(),
        Some(raw) => {
            if raw.get().len() > MAX_ARGS {
                return Incoming::Malformed(format!(
                    "a split_open's \"args\" is over {MAX_ARGS} bytes"
                ));
            }
            match serde_json::from_str::<Vec<String>>(raw.get()) {
                Ok(args) => args,
                Err(_) => {
                    return Incoming::Malformed(
                        "a split_open's \"args\" is not a list of text".to_owned(),
                    );
                }
            }
        }
    };

    Incoming::Request(Request::SplitOpen {
        side,
        program,
        args,
    })
}

/// `split_read`: the handle is required, and `scrollback` defaults to the visible screen.
fn split_read(envelope: &Envelope<'_>) -> Incoming {
    let Some(pane) = envelope.pane else {
        return Incoming::Malformed("a split_read names no \"pane\"".to_owned());
    };
    let Ok(pane) = serde_json::from_str::<String>(pane.get()) else {
        return Incoming::Malformed("a split_read's \"pane\" is not text".to_owned());
    };
    let scrollback = match envelope.scrollback {
        None => false,
        Some(raw) => match serde_json::from_str::<bool>(raw.get()) {
            Ok(scrollback) => scrollback,
            Err(_) => {
                return Incoming::Malformed(
                    "a split_read's \"scrollback\" is not true or false".to_owned(),
                );
            }
        },
    };

    Incoming::Request(Request::SplitRead { pane, scrollback })
}

/// `split_close`: the handle, and nothing else.
fn split_close(envelope: &Envelope<'_>) -> Incoming {
    let Some(pane) = envelope.pane else {
        return Incoming::Malformed("a split_close names no \"pane\"".to_owned());
    };
    let Ok(pane) = serde_json::from_str::<String>(pane.get()) else {
        return Incoming::Malformed("a split_close's \"pane\" is not text".to_owned());
    };

    Incoming::Request(Request::SplitClose { pane })
}

/// What goes back over the channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Ok,
    /// Taken, **and some of what was sent fell away on the way in**: the sentence naming it, as the
    /// contract's reader wrote it. Still a success — nothing was refused and the panel paints — which
    /// is why it is an `ok` with one more field and not a failure (**D58**).
    Lossy(String),
    /// A session was retired; auxiliary consoles were either detached or preserved after takeover.
    Retired {
        closed: usize,
        preserved: usize,
    },
    /// A pane was opened; the handle is `None` where the pane has no addressable identity.
    Opened(Option<String>),
    /// What a pane showed.
    Text(String),
    /// The document a pane's panel is showing, **already JSON**.
    ///
    /// **It is the one arm that does not escape its payload**, and that is a decision rather than an
    /// oversight. A document inside [`Reply::Text`] would have every quote in it escaped, so one near
    /// [`panel_contract::MAX_BYTES`] would roughly double and the client would refuse its own answer
    /// for being over [`MAX_REQUEST`] — a document the file transport accepts, unreadable over the
    /// channel, which is the two-transport drift the contract forbids seen from the reply side. It
    /// would also give `text` two meanings, since that one is a console's screen.
    ///
    /// **Its content may come only from the contract crate's own writer**, whose promise of one
    /// compact line with no newline anywhere is what keeps this interpolation safe on a
    /// newline-framed channel.
    Document(String),
    Failed(String),
    /// A temporary failure specific to agent signal delivery.
    Retryable(String),
}

impl Reply {
    /// The literal line, terminated.
    ///
    /// **Built by hand and not serialised from a type**, which is worth a sentence because it looks
    /// like the wrong call: the shape is three fields wide at most, it is frozen by this delivery's
    /// contract, and the one thing that could go wrong — an error text, a handle or a read carrying
    /// a quote or a newline — is what `serde_json`'s string escaping is asked for below. A
    /// `Serialize` derive would add a dependency feature to buy the half that is already bought.
    pub fn line(&self) -> String {
        match self {
            Self::Ok => "{\"ok\":true}\n".to_owned(),
            Self::Lossy(dropped) => {
                let escaped = serde_json::Value::String(dropped.clone());
                format!("{{\"ok\":true,\"dropped\":{escaped}}}\n")
            }
            Self::Retired { closed, preserved } => {
                format!("{{\"ok\":true,\"closed\":{closed},\"preserved\":{preserved}}}\n")
            }
            Self::Opened(pane) => match pane {
                Some(handle) => {
                    let escaped = serde_json::Value::String(handle.clone());
                    format!("{{\"ok\":true,\"pane\":{escaped}}}\n")
                }
                None => "{\"ok\":true,\"pane\":null}\n".to_owned(),
            },
            Self::Text(text) => {
                let escaped = serde_json::Value::String(text.clone());
                format!("{{\"ok\":true,\"text\":{escaped}}}\n")
            }
            // **Interpolated raw**, for the reason the variant states. What guarantees the framing
            // here is not this line but its only source: the contract's writer emits one compact line
            // and puts every string of the document through the same escaping the arm above uses.
            Self::Document(document) => format!("{{\"ok\":true,\"document\":{document}}}\n"),
            Self::Retryable(reason) => {
                let escaped = serde_json::Value::String(reason.clone());
                format!("{{\"ok\":false,\"error\":{escaped},\"retryable\":true}}\n")
            }
            Self::Failed(reason) => {
                let escaped = serde_json::Value::String(reason.clone());
                format!("{{\"ok\":false,\"error\":{escaped}}}\n")
            }
        }
    }
}

/// How long a reply is given to reach a client that is not reading it.
///
/// **It is the socket's own send timeout, not a sleep loop.** A client that is not draining its end
/// for five seconds has stopped reading, and cutting it is then the honest answer; the stream has
/// this value as its send timeout, so `write_all` returns rather than blocking for ever. That is
/// what replaced the hand-rolled write loop an earlier version carried here.
pub(crate) const REPLY_DEADLINE: Duration = Duration::from_secs(5);

/// Answers a request.
///
/// **One line, terminated and flushed**, so a client that reads by lines is never left waiting on a
/// reply that was written but not closed.
///
/// **`write_all` and no loop of its own for a full send buffer.** The stream is given
/// [`REPLY_DEADLINE`] as its send timeout where it is armed, so a full buffer comes back as an error
/// after that deadline instead of blocking; the loop that used to be here was a second way of
/// spelling the socket's own timeout, and a second deadline to keep in step.
pub fn write_reply(writer: &mut impl Write, reply: &Reply) -> io::Result<()> {
    writer.write_all(reply.line().as_bytes())?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A writer that takes one byte at a time.
    ///
    /// **What it ties down is that the whole line goes out even when one `write` does not take it
    /// all** — the partial-write case `write_all` exists for. A full send buffer is the socket's own
    /// timeout now, set where the stream is armed, and this double cannot be that timeout.
    struct OneByteAtATime {
        taken: Vec<u8>,
    }

    impl Write for OneByteAtATime {
        fn write(&mut self, from: &[u8]) -> io::Result<usize> {
            self.taken.push(from[0]);
            Ok(1)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// **A reply arrives whole even when the far end takes it a byte at a time.** The assertion is on
    /// **the bytes that landed**, never on the call returning `Ok`: a version that gave up after the
    /// first chunk would satisfy the latter.
    #[test]
    fn a_reply_reaches_an_end_that_takes_one_byte_at_a_time() {
        let reply = Reply::Failed("held by \"flow\"".to_owned());
        let mut client = OneByteAtATime { taken: Vec::new() };

        write_reply(&mut client, &reply).expect("the reply was written");

        assert_eq!(
            String::from_utf8(client.taken).expect("the reply is text"),
            reply.line()
        );
    }

    /// Both counts are fields of a successful retirement, not prose a client has to parse.
    #[test]
    fn a_retirement_reply_carries_closed_and_preserved_counts() {
        assert_eq!(
            Reply::Retired {
                closed: 2,
                preserved: 1,
            }
            .line(),
            "{\"ok\":true,\"closed\":2,\"preserved\":1}\n"
        );
    }

    /// The sentence carries quotes, so an unescaped interpolation would end the string early.
    #[test]
    fn a_lossy_reply_is_an_ok_that_carries_what_fell_away_escaped() {
        assert_eq!(
            Reply::Lossy("1 line carries an unknown mark \"doing\"".to_owned()).line(),
            "{\"ok\":true,\"dropped\":\"1 line carries an unknown mark \\\"doing\\\"\"}\n"
        );
    }

    fn one(text: &str) -> Incoming {
        let mut framing = Framing::default();
        framing.feed(text.as_bytes());
        framing.take().expect("a complete line was fed")
    }

    #[test]
    fn a_publication_carries_its_source_and_its_document() {
        let incoming =
            one("{\"tool\":\"panel_set\",\"source\":\"flow\",\"document\":{\"lines\":[\"a\"]}}\n");

        let Incoming::Request(Request::PanelSet { source, document }) = incoming else {
            panic!("not a publication: {incoming:?}");
        };
        assert_eq!(source, "flow");
        assert_eq!(
            panel_contract::parse_document(&document)
                .expect("the document survives the round trip")
                .lines
                .len(),
            1
        );
    }

    #[test]
    fn plan_requests_keep_the_document_text_and_ignore_repo_for_scope() {
        let raw = r#"{ "title" : "Ivy", "milestones": [] }"#;
        let incoming = one(&format!(
            "{{\"tool\":\"plan_set\",\"repo\":\"/other/repo\",\"document\":{raw}}}\n"
        ));
        assert_eq!(
            incoming,
            Incoming::Request(Request::PlanSet {
                document: raw.to_owned()
            })
        );
        assert_eq!(
            one("{\"tool\":\"plan_get\",\"repo\":\"/other\"}\n"),
            Incoming::Request(Request::PlanGet)
        );
        assert_eq!(
            one("{\"tool\":\"plan_retire\"}\n"),
            Incoming::Request(Request::PlanRetire)
        );
        assert!(matches!(
            one("{\"tool\":\"plan_set\"}\n"),
            Incoming::Malformed(_)
        ));
    }

    #[test]
    fn plan_documents_at_the_contract_ceiling_fit_inside_the_wire_envelope() {
        let raw = format!("\"{}\"", "x".repeat(panel_contract::MAX_BYTES as usize - 2));
        assert_eq!(raw.len(), panel_contract::MAX_BYTES as usize);
        let line = format!("{{\"tool\":\"plan_set\",\"document\":{raw}}}\n");
        assert!(line.len() - 1 <= MAX_REQUEST);
        let Incoming::Request(Request::PlanSet { document }) = one(&line) else {
            panic!("a plan at the contract ceiling was refused");
        };
        assert_eq!(document, raw);

        let raw = format!("\"{}\"", "x".repeat(panel_contract::MAX_BYTES as usize - 1));
        let line = format!("{{\"tool\":\"plan_set\",\"document\":{raw}}}\n");
        assert!(line.len() - 1 <= MAX_REQUEST);
        assert!(matches!(
            one(&line),
            Incoming::Request(Request::PlanSet { .. })
        ));
    }

    /// **A tool nobody serves still comes back named**, which is what lets the refusal say what it is
    /// refusing. The three verbs of the frozen format are no longer among them: they are understood
    /// here and answered by the window.
    #[test]
    fn an_unknown_tool_comes_back_named() {
        let incoming = one("{\"tool\":\"panel_dance\"}\n");

        assert!(
            matches!(&incoming, Incoming::Request(Request::NotServed(named)) if named == "panel_dance"),
            "an unknown tool did not come back named: {incoming:?}"
        );
    }

    /// The three verbs of the frozen format are read as requests, each with its own fields, and
    /// `scrollback` defaults to the visible screen.
    #[test]
    fn the_three_split_verbs_are_understood() {
        let Incoming::Request(Request::SplitOpen {
            side,
            program,
            args,
        }) = one(
            "{\"tool\":\"split_open\",\"side\":\"right\",\"program\":\"cargo\",\"args\":[\"test\"]}\n",
        )
        else {
            panic!("split_open was not understood");
        };
        assert_eq!(side, Side::Right);
        assert_eq!(program.as_deref(), Some("cargo"));
        assert_eq!(args, ["test"]);

        assert_eq!(
            one("{\"tool\":\"split_read\",\"pane\":\"h\"}\n"),
            Incoming::Request(Request::SplitRead {
                pane: "h".to_owned(),
                scrollback: false,
            }),
            "scrollback did not default to the visible screen"
        );
        assert_eq!(
            one("{\"tool\":\"split_read\",\"pane\":\"h\",\"scrollback\":true}\n"),
            Incoming::Request(Request::SplitRead {
                pane: "h".to_owned(),
                scrollback: true,
            })
        );
        assert_eq!(
            one("{\"tool\":\"split_close\",\"pane\":\"h\"}\n"),
            Incoming::Request(Request::SplitClose {
                pane: "h".to_owned(),
            })
        );
    }

    /// **`split_open` carries no `env`, and a value sent under that name reaches nothing.** The
    /// envelope has no such field, so the request that arrives is the same one as without it — which
    /// is what stops the verb from ever accepting an environment the pane's identity was designed to
    /// exclude.
    #[test]
    fn a_split_open_carrying_an_env_field_does_not_carry_it_anywhere() {
        let with = one(
            "{\"tool\":\"split_open\",\"side\":\"right\",\"env\":{\"PANEL_ID\":\"elsewhere\"}}\n",
        );
        let without = one("{\"tool\":\"split_open\",\"side\":\"right\"}\n");

        assert_eq!(with, without, "the env field changed the request");
        assert!(
            matches!(with, Incoming::Request(Request::SplitOpen { .. })),
            "split_open was not understood: {with:?}"
        );
    }

    /// A missing or unknown side, and an `args` that is not a list of text, are refused as malformed
    /// rather than opening a pane the wrong way; the connection lives, as for every malformed line.
    #[test]
    fn a_split_open_with_a_field_it_cannot_read_is_malformed() {
        assert!(matches!(
            one("{\"tool\":\"split_open\"}\n"),
            Incoming::Malformed(_)
        ));
        assert!(matches!(
            one("{\"tool\":\"split_open\",\"side\":\"diagonal\"}\n"),
            Incoming::Malformed(_)
        ));
        assert!(matches!(
            one("{\"tool\":\"split_open\",\"side\":\"right\",\"args\":\"cargo test\"}\n"),
            Incoming::Malformed(_)
        ));
    }

    /// The two payload replies are one escaped line each, and a pane with no addressable identity
    /// answers a JSON null for its handle rather than inventing one.
    #[test]
    fn a_handle_and_a_read_travel_as_escaped_payloads() {
        let opened = Reply::Opened(Some("a\"b".to_owned())).line();
        assert_eq!(opened.matches('\n').count(), 1, "not one line: {opened:?}");
        let parsed: serde_json::Value =
            serde_json::from_str(opened.trim_end()).expect("the reply is valid JSON");
        assert_eq!(parsed["ok"], serde_json::Value::Bool(true));
        assert_eq!(parsed["pane"], "a\"b");

        assert_eq!(Reply::Opened(None).line(), "{\"ok\":true,\"pane\":null}\n");

        let text = Reply::Text("line one\nline two".to_owned()).line();
        assert_eq!(
            text.matches('\n').count(),
            1,
            "a read produced two lines: {text:?}"
        );
        let parsed: serde_json::Value =
            serde_json::from_str(text.trim_end()).expect("the reply is valid JSON");
        assert_eq!(parsed["text"], "line one\nline two");
    }

    #[test]
    fn a_line_that_is_not_understood_does_not_end_the_connection() {
        assert!(matches!(one("not json\n"), Incoming::Malformed(_)));
        assert!(matches!(one("{}\n"), Incoming::Malformed(_)));
        assert!(matches!(
            one("{\"tool\":\"panel_set\"}\n"),
            Incoming::Malformed(_)
        ));
        assert!(matches!(one("\n"), Incoming::Malformed(_)));
    }

    /// **A patch is read at the wire, and everything it can refuse there refuses the line and not the
    /// connection.**
    ///
    /// The three shapes this covers are the three doors: no `ops` at all, an `ops` that is not a
    /// list, and an `ops` whose contents the contract refuses. The last is the one worth pinning —
    /// it is the contract's reader running on the connection's own thread, which is what keeps that
    /// work out of the frame — and the refusal comes back carrying the contract's own sentence,
    /// **not wrapped in a second one**.
    #[test]
    fn a_patch_the_wire_can_refuse_refuses_the_line_and_not_the_connection() {
        assert!(matches!(
            one("{\"tool\":\"panel_patch\"}\n"),
            Incoming::Malformed(_)
        ));
        assert!(matches!(
            one("{\"tool\":\"panel_patch\",\"ops\":\"replace\"}\n"),
            Incoming::Malformed(_)
        ));

        let Incoming::Malformed(why) = one(
            "{\"tool\":\"panel_patch\",\"ops\":[{\"op\":\"set\",\"field\":\"actions\",\"value\":null}]}\n",
        ) else {
            panic!("a field that is not patchable is refused at the wire");
        };
        assert_eq!(
            why,
            "\"actions\" is not patchable: a button reaches a shell, so it is only ever set with the whole document",
            "the contract's sentence reached the producer wrapped in something else"
        );

        // **Ops past the document's ceiling are refused, and the connection survives it.** Sized to
        // land in the window where that is decidable at all: the envelope is `ENVELOPE` bytes wide,
        // so ops a little over `MAX_BYTES` still make a line under `MAX_REQUEST` and reach this
        // function. Without the bound they would be read, and the same patch a hundred bytes longer
        // would pass the line ceiling too and be `Unrecoverable` — the connection ended, and the
        // producer's hold with it, for a patch doing exactly what this verb is for.
        let over = format!(
            "{{\"tool\":\"panel_patch\",\"ops\":[{{\"op\":\"last\",\"line\":\"{}\"}}]}}\n",
            "f".repeat(panel_contract::MAX_BYTES as usize + 100)
        );
        assert!(
            over.len() < MAX_REQUEST,
            "the line has to reach the verb for this to be about the verb: {} bytes",
            over.len()
        );
        let mut framing = Framing::default();
        framing.feed(over.as_bytes());
        let Some(Incoming::Malformed(why)) = framing.take() else {
            panic!("a patch whose ops are past the ceiling did not come back refused");
        };
        assert!(why.contains("over 256 KiB"), "{why}");

        // And a patch the wire has nothing against becomes a request, with its ops already read.
        let Incoming::Request(Request::PanelPatch { ops, .. }) =
            one("{\"tool\":\"panel_patch\",\"ops\":[{\"op\":\"delete\",\"key\":\"one\"}]}\n")
        else {
            panic!("a patch the wire serves becomes a request");
        };
        assert_eq!(ops.len(), 1);
    }

    /// **A request is only taken out once its line is complete**, which is the property the socket
    /// read depends on: a chunk that cuts a request in half must not produce half a request.
    #[test]
    fn a_request_split_across_chunks_is_taken_out_whole_and_only_then() {
        let whole = "{\"tool\":\"panel_set\",\"source\":\"flow\",\"document\":{\"lines\":[]}}\n";
        let (first, second) = whole.split_at(20);
        let mut framing = Framing::default();

        framing.feed(first.as_bytes());
        assert_eq!(framing.take(), None, "half a line produced a request");

        framing.feed(second.as_bytes());
        assert!(matches!(
            framing.take(),
            Some(Incoming::Request(Request::PanelSet { .. }))
        ));
        assert_eq!(framing.take(), None);
    }

    /// Two requests arriving in one chunk are two requests, which is what a source publishing twice
    /// in a row produces.
    #[test]
    fn several_requests_in_one_chunk_come_out_one_by_one() {
        let mut framing = Framing::default();
        framing.feed(
            b"{\"tool\":\"panel_set\",\"source\":\"a\",\"document\":{\"lines\":[]}}\n\
              {\"tool\":\"split_close\",\"pane\":\"h\"}\n",
        );

        assert!(matches!(
            framing.take(),
            Some(Incoming::Request(Request::PanelSet { .. }))
        ));
        assert!(matches!(
            framing.take(),
            Some(Incoming::Request(Request::SplitClose { .. }))
        ));
        assert_eq!(framing.take(), None);
    }

    /// Retirement is a connection-scoped instruction with no payload of its own.
    #[test]
    fn panel_retire_is_read_as_the_zero_argument_request() {
        let mut framing = Framing::default();
        framing.feed(b"{\"tool\":\"panel_retire\"}\n");

        assert_eq!(
            framing.take(),
            Some(Incoming::Request(Request::PanelRetire))
        );
    }

    /// **The ceiling is what stops a line from being held, so the assertion is on what is held** and
    /// not only on the verdict. Asserting the verdict alone passes with no ceiling at all: an
    /// unbounded accumulation would reach the same «too big» conclusion after having kept every
    /// byte of it, which is the one thing the ceiling exists to prevent.
    ///
    /// **Fed in production-sized chunks.** Feeding `MAX_REQUEST / 2` — what this did first — left the
    /// overshoot exactly as big as a socket read, which is how the ceiling's own tests missed a
    /// complete over-long line for a whole round.
    #[test]
    fn a_request_over_the_ceiling_is_not_held_whole() {
        let mut framing = Framing::default();

        for _ in 0..(MAX_REQUEST / 4096 + 2) {
            framing.feed(&vec![b'x'; 4096]);
            assert!(
                framing.held.len() <= MAX_REQUEST,
                "it held {} bytes of an unfinished line",
                framing.held.len()
            );
            if framing.over != Over::Fine {
                break;
            }
        }

        assert!(matches!(framing.take(), Some(Incoming::Unrecoverable(_))));
    }

    /// **A complete over-long line is caught even in a single feed.** Measuring only the tail after
    /// the last newline returned zero for a line that arrived already terminated, so `take` drained
    /// it whole and parsed it at any size. Inside the application a 4096-byte read kept the overshoot
    /// at 4095 bytes by coincidence; `Framing` is public API, so any other caller had no ceiling at
    /// all.
    #[test]
    fn a_complete_line_over_the_ceiling_is_caught_in_one_feed() {
        let mut framing = Framing::default();
        let mut line = vec![b'x'; MAX_REQUEST + 1];
        line.push(b'\n');

        framing.feed(&line);

        assert!(
            framing.held.len() <= MAX_REQUEST,
            "it held the whole over-long line"
        );
        assert!(matches!(framing.take(), Some(Incoming::Unrecoverable(_))));
    }

    /// **The envelope does not count against the document's ceiling.** A document of exactly the
    /// contract's ceiling — which the file accepts — produces a line a few dozen bytes longer, and
    /// the channel used to refuse it and end the connection for it. The document's own ceiling is the
    /// parser's now, so the transport has to let this line through.
    #[test]
    fn the_envelope_does_not_count_against_the_document_s_ceiling() {
        let document = "x".repeat(panel_contract::MAX_BYTES as usize);
        let line =
            format!("{{\"tool\":\"panel_set\",\"source\":\"flow\",\"document\":\"{document}\"}}\n");
        let mut framing = Framing::default();

        framing.feed(line.as_bytes());

        assert!(
            matches!(
                framing.take(),
                Some(Incoming::Request(Request::PanelSet { .. }))
            ),
            "the transport refused a document the file would accept"
        );
    }

    /// **A source longer than the envelope allows is refused as malformed, and the connection
    /// lives.** Without the bound, the same publication with a document at the ceiling would put the
    /// line past `MAX_REQUEST`, which is an `Unrecoverable` — the connection would end for a
    /// publication the file accepts. The second half asserts the bound actually fits: a source at the
    /// bound plus a ceiling document is still a `Request`, not an over-long line. The refusal is on
    /// the **wire width**, quotes included, so a name is sized at the bound minus the quotes.
    #[test]
    fn a_source_over_the_bound_is_refused_without_ending_the_connection() {
        let source = "s".repeat(MAX_SOURCE); // plus the quotes, over the wire bound
        let over = format!(
            "{{\"tool\":\"panel_set\",\"source\":\"{source}\",\"document\":{{\"lines\":[]}}}}\n"
        );
        let mut framing = Framing::default();
        framing.feed(over.as_bytes());
        assert!(
            matches!(framing.take(), Some(Incoming::Malformed(_))),
            "a source over the bound was not refused as malformed"
        );
        // **And the connection lives**: what follows is framed as a fresh request, which an
        // `Unrecoverable` would not be.
        framing.feed(b"{\"tool\":\"panel_set\",\"source\":\"a\",\"document\":{\"lines\":[]}}\n");
        assert!(
            matches!(
                framing.take(),
                Some(Incoming::Request(Request::PanelSet { .. }))
            ),
            "the refusal ended the connection"
        );

        let source = "s".repeat(MAX_SOURCE - 2); // plus the quotes, exactly at the bound
        let document = "x".repeat(panel_contract::MAX_BYTES as usize);
        let line = format!(
            "{{\"tool\":\"panel_set\",\"source\":\"{source}\",\"document\":\"{document}\"}}\n"
        );
        let mut framing = Framing::default();
        framing.feed(line.as_bytes());
        assert!(
            matches!(
                framing.take(),
                Some(Incoming::Request(Request::PanelSet { .. }))
            ),
            "a source at the bound pushed a legal document past the transport"
        );
    }

    /// **A line fed one byte at a time is framed exactly like one fed in a chunk.** The carried
    /// length is what makes the ceiling cost the same whatever the write size; this is what catches
    /// it drifting from the buffer across many feeds.
    #[test]
    fn a_line_fed_one_byte_at_a_time_is_taken_out_whole() {
        let whole = "{\"tool\":\"panel_set\",\"source\":\"flow\",\"document\":{\"lines\":[]}}\n";
        let mut framing = Framing::default();

        for byte in whole.as_bytes() {
            framing.feed(&[*byte]);
        }

        assert!(matches!(
            framing.take(),
            Some(Incoming::Request(Request::PanelSet { .. }))
        ));
        assert_eq!(framing.take(), None);
    }

    /// **The document travels as the bytes the producer wrote, so the ceiling is measured on those
    /// and not on a compact re-serialisation.** A pretty-printed document over the ceiling compacts
    /// under it — the two transports drifted there — and the parser now refuses it because the raw
    /// text is what reaches it. The premise is asserted too: the compact form really is under, which
    /// is what made the old path accept it.
    #[test]
    fn a_pretty_document_over_the_ceiling_is_not_compacted_under_it() {
        let between = " ".repeat(panel_contract::MAX_BYTES as usize);
        let document = format!("{{{} \"lines\":[\"a\"]}}", between);
        let line =
            format!("{{\"tool\":\"panel_set\",\"source\":\"flow\",\"document\":{document}}}\n");
        let mut framing = Framing::default();
        framing.feed(line.as_bytes());

        let Some(Incoming::Request(Request::PanelSet { document, .. })) = framing.take() else {
            panic!("the request was not understood");
        };
        let compact = serde_json::from_str::<serde_json::Value>(&document)
            .expect("it is JSON")
            .to_string();
        assert!(
            compact.len() <= panel_contract::MAX_BYTES as usize,
            "the premise failed: the compact form was already over the ceiling"
        );
        assert!(
            document.len() > panel_contract::MAX_BYTES as usize,
            "the raw document was not kept"
        );
        assert!(
            matches!(
                panel_contract::parse_document(&document),
                Err(panel_contract::Problem::TooBig)
            ),
            "the parser accepted a document whose bytes go over the ceiling"
        );
    }

    /// **One complete request must not switch the ceiling off for everything after it.** Asking
    /// whether a newline existed anywhere did exactly that: a legal request followed by an endless
    /// line was held whole, so a producer could reserve as much of the window's memory as it liked
    /// by sending one valid publication first.
    #[test]
    fn a_legal_request_does_not_lift_the_ceiling_for_what_follows_it() {
        let mut framing = Framing::default();
        framing.feed(b"{\"tool\":\"panel_set\",\"source\":\"a\",\"document\":{\"lines\":[]}}\n");

        for _ in 0..(MAX_REQUEST / 4096 + 2) {
            framing.feed(&vec![b'x'; 4096]);
            assert!(
                framing.held.len() <= MAX_REQUEST,
                "it held {} bytes after a complete request",
                framing.held.len()
            );
        }
    }

    /// **The refusal is terminal.** Clearing the state on the way out let the tail of the over-long
    /// line be read as a fresh request — resynchronising onto a boundary nobody ever saw, which is
    /// the whole reason the connection ends instead of skipping the line.
    #[test]
    fn once_the_ceiling_is_passed_nothing_else_is_read() {
        let mut framing = Framing::default();
        framing.feed(&vec![b'x'; MAX_REQUEST + 1]);
        assert!(matches!(framing.take(), Some(Incoming::Unrecoverable(_))));

        framing.feed(b"\n{\"tool\":\"panel_set\",\"source\":\"a\",\"document\":{\"lines\":[]}}\n");

        assert_eq!(framing.take(), None, "the tail was read as a fresh request");
        assert!(framing.held.is_empty());
    }

    /// A connection sending a thousand ordinary requests accumulates nothing: the ceiling is about
    /// one unfinished line, and a version that measured the connection's whole traffic would kill
    /// the longest-lived agent instead of the abusive one.
    #[test]
    fn a_long_conversation_of_small_requests_never_reaches_the_ceiling() {
        let mut framing = Framing::default();
        let request = "{\"tool\":\"panel_set\",\"source\":\"a\",\"document\":{\"lines\":[]}}\n";

        for _ in 0..(MAX_REQUEST / request.len() + 10) {
            framing.feed(request.as_bytes());
            assert!(matches!(
                framing.take(),
                Some(Incoming::Request(Request::PanelSet { .. }))
            ));
        }

        assert!(framing.held.is_empty());
    }

    /// A reason with a quote and a line break in it must not be able to produce two lines or to
    /// close the JSON early — which is the one thing hand-building this line could get wrong.
    #[test]
    fn a_refusal_carrying_quotes_stays_one_line_of_json() {
        let line = Reply::Failed("held by \"ci\"\nand not by you".to_owned()).line();

        assert_eq!(line.matches('\n').count(), 1, "not one line: {line:?}");
        let parsed: serde_json::Value =
            serde_json::from_str(line.trim_end()).expect("the refusal is valid JSON");
        assert_eq!(parsed["ok"], serde_json::Value::Bool(false));
        assert_eq!(parsed["error"], "held by \"ci\"\nand not by you");
    }

    #[test]
    fn a_reply_is_one_terminated_line() {
        let mut written = Vec::new();
        write_reply(&mut written, &Reply::Ok).expect("write the reply");

        assert_eq!(String::from_utf8(written).expect("text"), "{\"ok\":true}\n");
    }
}

#[cfg(test)]
mod agent_signal_tests {
    use super::*;

    #[test]
    fn retryable_reply_is_escaped_and_legacy_failure_shape_is_unchanged() {
        let reply: serde_json::Value =
            serde_json::from_str(&Reply::Retryable("busy \"now\"".into()).line()).unwrap();
        assert_eq!(
            reply,
            serde_json::json!({"ok": false, "error": "busy \"now\"", "retryable": true})
        );
        let legacy: serde_json::Value =
            serde_json::from_str(&Reply::Failed("busy".into()).line()).unwrap();
        assert_eq!(legacy, serde_json::json!({"ok": false, "error": "busy"}));
    }

    #[test]
    fn accepts_only_complete_strict_opencode_v1_snapshots() {
        let valid = r#"{"tool":"agent_signal","v":1,"client":"opencode","snapshot":{"session":"ses_root","activity":"busy","permission":false,"question":true}}"#;
        assert!(matches!(
            interpret(valid.as_bytes()),
            Incoming::Request(Request::AgentSignal(Some(_)))
        ));
        assert!(matches!(
            interpret(br#"{"tool":"agent_signal","v":1,"client":"opencode","snapshot":null}"#),
            Incoming::Request(Request::AgentSignal(None))
        ));
        assert!(
            matches!(interpret(valid.as_bytes()), Incoming::Request(Request::AgentSignal(Some(snapshot))) if !snapshot.stale)
        );
        let stale = valid.replace("\"question\":true", "\"question\":true,\"stale\":true");
        assert!(
            matches!(interpret(stale.as_bytes()), Incoming::Request(Request::AgentSignal(Some(snapshot))) if snapshot.stale && snapshot.question)
        );
        let invalid = [
            valid.replace("\"question\":true", "\"question\":true,\"stale\":1"),
            valid.replace("opencode", "codex"),
            valid.replace("\"v\":1", "\"v\":2"),
            valid.replace("\"busy\"", "\"working\""),
            valid.replace("\"question\":true", "\"question\":1"),
            valid.replace("\"session\":\"ses_root\",", ""),
            valid.replace("\"question\":true", "\"question\":true,\"pane\":\"other\""),
            valid.replace("\"v\":1", "\"v\":1,\"pane\":\"other\""),
            valid.replace("ses_root", &"x".repeat(257)),
            valid.replace("ses_root", ""),
            r#"{"tool":"agent_signal","v":1,"client":"opencode"}"#.to_owned(),
        ];
        for line in invalid {
            assert!(
                matches!(interpret(line.as_bytes()), Incoming::Malformed(_)),
                "accepted {line}"
            );
        }
    }
}
