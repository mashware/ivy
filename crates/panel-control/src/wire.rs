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

/// A request, once it has been understood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Publish this document on the pane this connection opened, under this source.
    ///
    /// The document travels as **its own text** and not as an interpreted value: the contract's
    /// parser is the only one that turns text into a panel, and handing it anything else would be a
    /// second interpretation of the same bytes — which is exactly how two transports of one contract
    /// start disagreeing field by field. It is the producer's own bytes, kept as a [`RawValue`], so
    /// the parser's ceiling measures what arrived and not a compact re-serialisation of it.
    PanelSet { source: String, document: String },
    /// A tool this build does not serve, carried by its name so the refusal can say which.
    ///
    /// **The three `split_*` verbs land here on purpose in this delivery**: their scoping arrives
    /// with the pane handles, and a verb that answered something before it could be scoped would be
    /// a promise taken back later.
    NotServed(String),
}

/// What one complete line produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
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

    match envelope.tool.as_str() {
        "panel_set" => panel_set(&envelope),
        other => Incoming::Request(Request::NotServed(other.to_owned())),
    }
}

/// The one tool this build serves.
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

/// What goes back over the channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Ok,
    Failed(String),
}

impl Reply {
    /// The literal line, terminated.
    ///
    /// **Built by hand and not serialised from a type**, which is worth a sentence because it looks
    /// like the wrong call: the shape is two fields wide, it is frozen by this delivery's contract,
    /// and the one thing that could go wrong — an error text with a quote or a newline in it — is
    /// what `serde_json`'s string escaping is asked for below. A `Serialize` derive would add a
    /// dependency feature to buy the half that is already bought.
    pub fn line(&self) -> String {
        match self {
            Self::Ok => "{\"ok\":true}\n".to_owned(),
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

    /// The three verbs of the frozen format are refused **by name** rather than read as malformed,
    /// which is what lets the refusal say what it is refusing.
    #[test]
    fn the_verbs_this_build_does_not_serve_come_back_named() {
        for tool in ["split_open", "split_read", "split_close"] {
            let incoming = one(&format!("{{\"tool\":\"{tool}\"}}\n"));

            assert!(
                matches!(&incoming, Incoming::Request(Request::NotServed(named)) if named == tool),
                "{tool} did not come back named: {incoming:?}"
            );
        }
    }

    /// **`split_open` is not served, with or without the `env` field the frozen format carries.**
    /// The whole verb comes back named, so there is no field of it — `env` included — this build
    /// could accept; and pinning that is what stops a later delivery from quietly serving the verb
    /// with an environment the pane's identity was designed to exclude.
    #[test]
    fn a_split_open_carrying_an_env_field_is_still_not_served() {
        let incoming = one(
            "{\"tool\":\"split_open\",\"side\":\"right\",\"env\":{\"PANEL_ID\":\"elsewhere\"}}\n",
        );

        assert!(
            matches!(&incoming, Incoming::Request(Request::NotServed(named)) if named == "split_open"),
            "a split_open with an env field was not refused by name: {incoming:?}"
        );
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
              {\"tool\":\"split_open\"}\n",
        );

        assert!(matches!(
            framing.take(),
            Some(Incoming::Request(Request::PanelSet { .. }))
        ));
        assert!(matches!(
            framing.take(),
            Some(Incoming::Request(Request::NotServed(_)))
        ));
        assert_eq!(framing.take(), None);
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
