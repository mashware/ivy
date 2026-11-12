//! The producer's side of the channel.
//!
//! **It is here and not in whoever speaks MCP**, because what a request looks like on the wire is
//! the contract's business and not the vocabulary's: the day the same channel is spoken over
//! something else, that vocabulary is the piece that goes and this is the piece that stays.

use std::io::{self, Read, Write};
use std::path::Path;

use interprocess::local_socket::Stream;

use crate::endpoint;
use crate::wire::{MAX_REQUEST, Side};

/// The pane identity a program inherited, **when it is whole**: `PANEL_ID` present, non-empty and
/// text, and `PANEL_DIR` present and an absolute path.
///
/// **The one rule the MCP bridge applies, to its handshake and to its tool calls alike**, because
/// two rules in one process gave two answers: the handshake said «no console» for an empty
/// identifier while the first tool call connected with it. The session helper still reads the same
/// variables with a rule of its own. Empty counts as absent — a client configuration that
/// interpolates `${PANEL_ID:-}` produces one, and on Windows an empty identifier names a single
/// machine-wide endpoint. An identifier that is not text cannot be one this terminal generated, and
/// converting it lossily would connect to a mangled name. A relative directory cannot be one either:
/// it would resolve against the working directory and reach whatever socket sits there.
///
/// **It takes the values instead of reading them** so it can be tested without `set_var`, which
/// is undefined behaviour in a suite that launches processes in parallel.
pub fn whole_identity<'a>(
    id: Option<&'a std::ffi::OsStr>,
    dir: Option<&'a std::ffi::OsStr>,
) -> Option<(&'a str, &'a Path)> {
    let id = id?.to_str().filter(|id| !id.is_empty())?;
    let dir = Path::new(dir?);
    dir.is_absolute().then_some((id, dir))
}

/// A connection to one pane's channel.
///
/// **The hold this delivery gives a source lasts as long as this value does**, which is why it is a
/// type rather than a function per request: publishing twice through two connections would claim the
/// pane twice and be refused the second time.
pub struct Producer {
    stream: Stream,
}

impl Producer {
    /// Opens the channel of the pane named by the two variables the program inherited.
    ///
    /// **No third variable, and no identifier on the wire.** Which pane this reaches is decided by
    /// which endpoint was opened, so there is nothing in any message for anybody to trust.
    ///
    /// **A whole identity or nothing, and never a fallback** ([`whole_identity`]). Which of the two
    /// variables names the endpoint is the platform's business ([`endpoint::name_of`]), so half an
    /// identity is a missing one on one platform and harmless on the other — and defaulting the
    /// identifier to the empty string resolved every pane on Windows to a single machine-wide name.
    ///
    /// **Two refusals, because they have two causes.** Neither variable set is a program outside
    /// any pane. Either one set but the identity not whole is a client that forwards them badly —
    /// an interpolation of an unset variable, a list of variables missing one — and saying «not in a
    /// pane» there would send whoever reads it looking in the wrong place.
    pub fn of_this_pane() -> io::Result<Self> {
        let [id, dir] = panel_contract::ENV_NAMES.map(std::env::var_os);
        let refusal = if id.is_none() && dir.is_none() {
            "this program is not running inside a pane of this terminal"
        } else {
            "PANEL_ID and PANEL_DIR are set but are not a whole pane identity (one is missing or \
             empty, the identifier is not text, or the directory is not absolute): check how your \
             client forwards them"
        };
        let (id, dir) = whole_identity(id.as_deref(), dir.as_deref())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, refusal))?;

        Self::of(dir, id)
    }

    /// The same, for a pane named directly. **What the tests use**, and what a producer that manages
    /// several panes of its own would use.
    pub fn of(dir: &Path, id: &str) -> io::Result<Self> {
        Ok(Self {
            stream: endpoint::connect(dir, id)?,
        })
    }

    /// Publishes a document, and returns what the window answered.
    ///
    /// **The document is written back out on one line, and that is this side's job rather than the
    /// producer's.** The wire is framed by newlines, and the natural thing for somebody writing JSON
    /// by hand — or for any pretty-printer — is to produce several lines; interpolating that raw
    /// split the request across frames, so the pane read a first line that was not valid JSON and
    /// then a tail of nothing. Compacting here costs one parse on the producer's own thread and
    /// makes the framing impossible to break from outside.
    ///
    /// It also means a document that is not JSON at all is refused **here, to its author**, instead
    /// of travelling to the window to come back as an answer nobody can act on.
    pub fn publish(&mut self, source: &str, document: &str) -> io::Result<Answered> {
        let document: serde_json::Value = serde_json::from_str(document).map_err(|failure| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the document is not JSON ({failure})"),
            )
        })?;
        // `Value`'s `Display` is the compact form: no newline can survive it, which is the property
        // the framing rests on.
        let escaped_source = serde_json::Value::String(source.to_owned());
        writeln!(
            self.stream,
            "{{\"tool\":\"panel_set\",\"source\":{escaped_source},\"document\":{document}}}"
        )?;
        self.stream.flush()?;

        self.read_reply()
    }

    /// Opens a console beside this pane's own, and returns the handle the window gave it.
    ///
    /// **The handle is what the rest of the verbs are named by**, and it is opaque on purpose: it
    /// belongs to this connection, and a second connection cannot use it even if it reads it.
    pub fn open_a_pane(
        &mut self,
        side: Side,
        program: Option<&str>,
        args: &[String],
    ) -> io::Result<Answered> {
        // **Built as a value and not interpolated**, so a program name with a quote in it cannot
        // break the request it travels in — the same hazard the reply's escaping answers.
        let mut request = serde_json::json!({"tool": "split_open", "side": side.name()});
        if let Some(program) = program {
            request["program"] = serde_json::Value::String(program.to_owned());
        }
        if !args.is_empty() {
            request["args"] = serde_json::json!(args);
        }
        self.ask(request)
    }

    /// Reads a console this connection opened: **the visible screen unless the scrollback is asked
    /// for**, which is the contract's default.
    pub fn read_a_pane(&mut self, pane: &str, scrollback: bool) -> io::Result<Answered> {
        self.ask(serde_json::json!({
            "tool": "split_read",
            "pane": pane,
            "scrollback": scrollback,
        }))
    }

    /// Asks for the document this pane's panel is showing.
    ///
    /// **It takes no hold and needs none**: it is for the producer that has *not* claimed the pane —
    /// one that restarted, one whose context was compacted away, a second tool taking the console
    /// over — and it answers with a document that arrived by file just as readily as with one this
    /// connection published.
    pub fn read_the_document(&mut self) -> io::Result<Answered> {
        self.ask(serde_json::json!({"tool": "panel_get"}))
    }

    /// Changes part of the document this connection published, leaving the rest of it alone.
    ///
    /// **It needs the hold that a publication takes**, unlike [`Producer::read_the_document`]: there
    /// is no state where a connection holds a pane and there is nothing to patch, so *nothing has
    /// been published yet* and *this pane's panel is its file's* are one refusal, and it names the
    /// move — publish first.
    ///
    /// **What happens here is the framing, not the reading.** The ops are checked to be JSON and
    /// compacted, for the same two reasons a document is: a list that is not JSON at all is refused
    /// to its author instead of travelling to the window, and a compact line cannot carry the newline
    /// the framing rests on. **What each op says is decided at the other end** — by
    /// `panel_contract::parse_ops_noting`, on the connection's thread inside the application — so an op
    /// nobody serves or a field that is not patchable travels and comes back as a refusal.
    pub fn patch(&mut self, ops: &str) -> io::Result<Answered> {
        let ops: serde_json::Value = serde_json::from_str(ops).map_err(|failure| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the ops are not JSON ({failure})"),
            )
        })?;
        self.ask(serde_json::json!({"tool": "panel_patch", "ops": ops}))
    }

    /// Intentionally ends the session currently holding this pane's panel.
    pub fn retire(&mut self) -> io::Result<Answered> {
        self.ask(serde_json::json!({"tool": "panel_retire"}))
    }

    /// Closes a console this connection opened.
    pub fn close_a_pane(&mut self, pane: &str) -> io::Result<Answered> {
        self.ask(serde_json::json!({"tool": "split_close", "pane": pane}))
    }

    /// Writes one request line and reads its answer, **the one place that talks to the socket**.
    ///
    /// `Value`'s `Display` is the compact form, so a request can never carry a newline and break the
    /// framing the wire rests on.
    fn ask(&mut self, request: serde_json::Value) -> io::Result<Answered> {
        writeln!(self.stream, "{request}")?;
        self.stream.flush()?;

        self.read_reply()
    }

    /// Reads one reply line, bounded the same way a request is.
    fn read_reply(&mut self) -> io::Result<Answered> {
        let mut line = Vec::new();
        let mut byte = [0_u8; 1];
        loop {
            match self.stream.read(&mut byte)? {
                0 => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "the pane closed the channel without answering",
                    ));
                }
                _ if byte[0] == b'\n' => break,
                _ => line.push(byte[0]),
            }
            if line.len() > MAX_REQUEST {
                return Err(io::Error::other("the answer goes over the ceiling"));
            }
        }

        // **The document is kept as the bytes that arrived, and every other field is not.** A
        // `serde_json::Value` cannot do it: without `preserve_order` its map is sorted, so a document
        // published as `{"updated_at":…,"phase":…,"lines":…}` came back
        // `{"lines":…,"phase":…,"updated_at":…}` — the same panel, and not the same bytes. That
        // defeats the one reason the reply carries the document unescaped, and it made this type's own
        // promise —«as the JSON text it arrived as»— false. Borrowing it as a [`RawValue`] is the same
        // answer the request side already gives for the same reason.
        let raw: Reply<'_> = serde_json::from_slice(&line)
            .map_err(|failure| io::Error::other(failure.to_string()))?;
        // **The field travels, not its name.** Looking one up by a string would put a `_ => None` arm
        // between a refusal and the caller: a typo would compile, every refusal would collapse into
        // «it refused without saying why», and the reason would be thrown away — the same silent loss
        // the branch below exists to remove.
        let text_of = |held: Option<&serde_json::value::RawValue>| -> Option<String> {
            held.and_then(|held| serde_json::from_str::<String>(held.get()).ok())
        };
        match raw.ok {
            Some(true) if raw.closed.is_some() || raw.preserved.is_some() => {
                match (raw.closed, raw.preserved) {
                    (Some(closed), Some(preserved)) => Ok(Answered::Retired { closed, preserved }),
                    _ => Ok(Answered::Failed(
                        "the pane answered retirement without both counts".to_owned(),
                    )),
                }
            }
            Some(true) => match raw.pane {
                // **Present and possibly null**: an open answers with the handle, or with a JSON null
                // where the console has no addressable identity, and those are two different
                // answers a caller can act on differently. A handle that is neither text nor null
                // would also land on `None`; the wire writes only those two (`Reply::Opened` in
                // `wire.rs`), so no separate answer is kept for it.
                Some(pane) => Ok(Answered::Opened(text_of(Some(pane)))),
                // **Before the fallback and not after it.** Everything that is not `pane` and not
                // `text` used to land on `Answered::Ok` — success carrying nothing — so a reply whose
                // whole content is the document would have read as «it worked» with the document
                // thrown away, and silently.
                None => match raw.document {
                    Some(document) => Ok(Answered::Document(document.get().to_owned())),
                    // Before the fallback for the reason the arm above gives: landing on `Ok` is a
                    // lossy publication answered as a perfect one, which is the silence this field
                    // exists to break. A note that is not text still says something fell, so it is
                    // handed over as the JSON it arrived as rather than dropped into a plain success.
                    None => match raw.dropped {
                        Some(dropped) => Ok(Answered::Lossy(
                            text_of(Some(dropped)).unwrap_or_else(|| dropped.get().to_owned()),
                        )),
                        None => match text_of(raw.text) {
                            Some(text) => Ok(Answered::Text(text)),
                            None => Ok(Answered::Ok),
                        },
                    },
                },
            },
            _ => Ok(Answered::Failed(text_of(raw.error).unwrap_or_else(|| {
                "the pane refused it without saying why".to_owned()
            }))),
        }
    }
}

/// One reply line, **with the document kept as the bytes it arrived as**.
///
/// Every JSON-valued field is borrowed raw for the reason the request's own envelope states: this crate does not
/// re-serialise somebody else's JSON, because re-serialising is what silently reordered a document's
/// keys on the way back to its producer.
#[derive(serde::Deserialize)]
struct Reply<'a> {
    ok: Option<bool>,
    #[serde(default)]
    closed: Option<usize>,
    #[serde(default)]
    preserved: Option<usize>,
    /// **A null `pane` is an answer of its own** — the console was opened with no addressable
    /// identity — so it must not be read as an absent field, which is what `Option`'s own
    /// `Deserialize` does with a null. Hence [`present`].
    #[serde(default, borrow, deserialize_with = "present")]
    pane: Option<&'a serde_json::value::RawValue>,
    #[serde(default, borrow)]
    text: Option<&'a serde_json::value::RawValue>,
    #[serde(default, borrow)]
    document: Option<&'a serde_json::value::RawValue>,
    #[serde(default, borrow)]
    dropped: Option<&'a serde_json::value::RawValue>,
    #[serde(default, borrow)]
    error: Option<&'a serde_json::value::RawValue>,
}

/// `Some` for a field that is there, **null included** — serde only calls this for a present field,
/// and `default` supplies the `None` for an absent one.
fn present<'de, D>(field: D) -> Result<Option<&'de serde_json::value::RawValue>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(field).map(Some)
}

/// What a pane answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    /// It was applied, and the reply carried nothing else.
    Ok,
    /// It was applied, and **part of what was sent fell away on the way in**: the sentence naming
    /// what, in the contract's words.
    Lossy(String),
    /// A session was retired, with its auxiliary-console outcome.
    Retired {
        closed: usize,
        preserved: usize,
    },
    /// A console was opened; the handle is `None` where it has no addressable identity.
    Opened(Option<String>),
    /// What a console showed.
    Text(String),
    /// The document a console's panel is showing, **as the bytes it arrived as** — not a
    /// re-serialisation of them, which sorts the keys and hands the producer a document it did not
    /// publish.
    Document(String),
    Failed(String),
}

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_reply_reader_ignores_retryable_extension() {
        let parsed: super::Reply<'_> =
            serde_json::from_str(r#"{"ok":false,"error":"busy","retryable":true}"#).unwrap();
        assert_eq!(parsed.ok, Some(false));
        assert_eq!(parsed.error.unwrap().get(), "\"busy\"");
    }

    /// **Only a whole identity is one.** Each row breaks one half — absent, empty, or an identifier
    /// that is not text; a version that checked one variable, or presence only, passes the first
    /// row and fails one of the rest.
    #[test]
    fn an_identity_is_whole_only_with_both_halves_present_non_empty_and_the_id_text() {
        use std::ffi::OsStr;
        let id = Some(OsStr::new("a1b2c3d4"));
        let dir = Some(OsStr::new("/run/user/1000/ivy-a1b2c3d4"));
        let empty = Some(OsStr::new(""));

        assert_eq!(
            super::whole_identity(id, dir),
            Some(("a1b2c3d4", Path::new("/run/user/1000/ivy-a1b2c3d4")))
        );
        assert_eq!(super::whole_identity(None, dir), None, "no identity");
        assert_eq!(super::whole_identity(id, None), None, "no directory");
        assert_eq!(super::whole_identity(empty, dir), None, "an empty identity");
        assert_eq!(super::whole_identity(id, empty), None, "an empty directory");
        assert_eq!(
            super::whole_identity(id, Some(OsStr::new("ivy-a1b2c3d4"))),
            None,
            "a relative directory"
        );
        // The readers destructure `ENV_NAMES` as `[id, dir]`; swapping the list would hand the
        // directory in as the identifier and still compile.
        assert_eq!(panel_contract::ENV_NAMES, ["PANEL_ID", "PANEL_DIR"]);
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let not_text = Some(OsStr::from_bytes(b"a1\xffb2"));
            assert_eq!(
                super::whole_identity(not_text, dir),
                None,
                "an identifier that is not text"
            );
        }
    }

    use super::*;
    use crate::listener::{Answer, Channel, Delivery, What};
    use panel_contract::PanelDir;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc::{Receiver, sync_channel};
    use std::time::Duration;

    fn a_pane() -> (PanelDir, Channel, Receiver<Delivery>) {
        a_pane_reading(Arc::new(|| Err("nothing has been published".to_owned())))
    }

    fn a_pane_reading(
        read_document: crate::listener::ReadDocument,
    ) -> (PanelDir, Channel, Receiver<Delivery>) {
        let dir = PanelDir::create().expect("create the panel directory");
        let (sender, inbox) = sync_channel(8);
        let nobody = Arc::new(AtomicUsize::new(0));
        let listening = Channel::open_with_reader(
            dir.path(),
            dir.id(),
            sender,
            read_document,
            Arc::new(move || {
                nobody.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }),
        )
        .expect("open the channel");

        (dir, listening, inbox)
    }

    /// The two halves of this crate speak to each other, which is the only thing that proves the
    /// wire format is one format and not two written from the same paragraph.
    #[test]
    fn a_producer_publishes_and_reads_what_the_window_decided() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");

        let window = std::thread::spawn(move || {
            let delivery = inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the window is handed the document");
            let What::Publish { source, panel } = delivery.what else {
                panic!("the window was handed something else");
            };
            let _ = delivery.answer.send(Answer::Refused(format!(
                "held by \"{source}\", {} line(s)",
                panel.lines.len()
            )));
        });

        let answered = producer
            .publish("flow", "{\"lines\":[\"one\",\"two\"]}")
            .expect("publish");

        window.join().expect("the window thread finished");
        assert_eq!(
            answered,
            Answered::Failed("held by \"flow\", 2 line(s)".to_owned())
        );
    }

    /// **A lossy publication reads as lossy, and a clean one as plain success**, both over a real
    /// listener. The assertion is on the variant and its sentence and never on «it is ok»: the decode
    /// falls through to `Answered::Ok` when it recognises no field, which is exactly what reading the
    /// new field too late would do.
    #[test]
    fn a_producer_is_told_what_fell_away_and_nothing_when_nothing_did() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");

        let window = std::thread::spawn(move || {
            for _ in 0..2 {
                let delivery = inbox
                    .recv_timeout(Duration::from_secs(5))
                    .expect("the window is handed the document");
                let _ = delivery.answer.send(Answer::Taken);
            }
        });

        let lossy = producer
            .publish("flow", r#"{"attention":"waiting","lines":["one"]}"#)
            .expect("publish");
        let clean = producer
            .publish("flow", r#"{"attention":"wait","lines":["one"]}"#)
            .expect("publish");

        window.join().expect("the window thread finished");
        assert_eq!(
            lossy,
            Answered::Lossy(r#"the document carries an unknown attention "waiting""#.to_owned())
        );
        assert_eq!(clean, Answered::Ok);
    }

    /// **What comes back is the bytes the pane sent, not a re-serialisation of them.**
    ///
    /// The assertion is on the **order of the keys**, and that is the whole point: any reading that
    /// goes through a `serde_json::Value` returns the same panel with its fields sorted, so a test
    /// that parsed the answer and compared documents would pass on exactly the version this guards
    /// against. Published as `updated_at`, `phase`, `lines`; sorted it would be `lines`, `phase`,
    /// `updated_at`.
    #[test]
    fn a_document_comes_back_in_the_order_the_pane_sent_it() {
        let sent = "{\"updated_at\":\"x\",\"phase\":\"build\",\"lines\":[\"one\"]}";
        let (dir, _channel, _inbox) = a_pane_reading(Arc::new(move || Ok(sent.to_owned())));
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");

        let answered = producer.read_the_document().expect("read the document");

        assert_eq!(answered, Answered::Document(sent.to_owned()));
    }

    /// **A document written the way a person writes JSON — over several lines — must still arrive as
    /// one request.** Interpolating it raw split the request across frames, so the pane read a first
    /// line that was not valid JSON and then a tail of nothing, and the producer got a puzzling
    /// refusal instead of a panel. The assertion is on the panel that **reached the window**, never
    /// on the bytes written: a check on the request text would pass on a version that broke the
    /// framing somewhere further along.
    #[test]
    fn a_document_written_across_several_lines_still_arrives_as_one_request() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");

        let window = std::thread::spawn(move || {
            let delivery = inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the window is handed the document");
            let What::Publish { panel, .. } = delivery.what else {
                panic!("the window was handed something else");
            };
            let _ = delivery.answer.send(Answer::Taken);
            panel
        });

        let answered = producer
            .publish(
                "flow",
                "{\n  \"phase\": \"build\",\n  \"lines\": [\n    \"one\",\n    \"two\"\n  ]\n}",
            )
            .expect("publish");

        assert_eq!(answered, Answered::Ok);
        let panel = window.join().expect("the window thread finished");
        assert_eq!(panel.lines.len(), 2);
        assert_eq!(panel.phase.as_deref(), Some("build"));
    }

    /// The producer sends the zero-argument request and retains both result counts.
    #[test]
    fn a_producer_retires_and_reads_the_auxiliary_console_outcome() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");
        let window = std::thread::spawn(move || {
            let delivery = inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the retirement");
            assert!(matches!(delivery.what, What::Retire));
            let _ = delivery.answer.send(Answer::Retired {
                closed: 2,
                preserved: 1,
            });
        });

        assert_eq!(
            producer.retire().expect("retire"),
            Answered::Retired {
                closed: 2,
                preserved: 1,
            }
        );
        window.join().expect("the window thread finished");
    }

    /// **The three `split_*` verbs have a client half too, and their payloads survive the round trip.**
    /// The window here answers by hand: what is asserted is that each request arrives as the right
    /// `What`, and that a handle and a read come back to the caller that asked.
    #[test]
    fn the_split_verbs_travel_and_their_payloads_come_back() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");
        let window = std::thread::spawn(move || {
            let open = inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the open");
            let What::Open {
                side,
                program,
                args,
            } = open.what
            else {
                panic!("the window was handed something else");
            };
            assert_eq!(side, Side::Right);
            assert_eq!(program.as_deref(), Some("cargo"));
            assert_eq!(args, ["test"]);
            let _ = open.answer.send(Answer::Opened(Some("h-1".to_owned())));

            let read = inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the read");
            let What::Read { pane, scrollback } = read.what else {
                panic!("the window was handed something else");
            };
            assert_eq!(pane, "h-1");
            assert!(scrollback);
            let _ = read.answer.send(Answer::Text("line one".to_owned()));

            let close = inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the close");
            assert!(matches!(close.what, What::Close { .. }));
            let _ = close.answer.send(Answer::Taken);
        });

        assert_eq!(
            producer
                .open_a_pane(Side::Right, Some("cargo"), &["test".to_owned()])
                .expect("open"),
            Answered::Opened(Some("h-1".to_owned()))
        );
        assert_eq!(
            producer.read_a_pane("h-1", true).expect("read"),
            Answered::Text("line one".to_owned())
        );
        assert_eq!(producer.close_a_pane("h-1").expect("close"), Answered::Ok);

        window.join().expect("the window thread finished");
    }

    /// **An open whose console has no addressable identity reads as an open, not as a publication.**
    /// The wire carries it as `"pane":null`, and a field read through `Option`'s own `Deserialize`
    /// takes a JSON null for an absent one — so the reply used to fall all the way through to
    /// `Answered::Ok`, which the MCP bridge words as «published». The window answers over a real
    /// listener, so the null is the one the wire writes, not one typed into the test.
    #[test]
    fn an_open_with_no_addressable_identity_comes_back_as_an_open() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");
        let window = std::thread::spawn(move || {
            let open = inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the open");
            let asked_to_open = matches!(open.what, What::Open { .. });
            let _ = open.answer.send(Answer::Opened(None));
            asked_to_open
        });

        let answered = producer.open_a_pane(Side::Right, None, &[]).expect("open");

        assert!(
            window.join().expect("the window thread finished"),
            "the window was handed something other than an open"
        );
        assert_eq!(answered, Answered::Opened(None));
    }

    /// A document that is not JSON is refused **to its author, here**, instead of travelling to the
    /// window and coming back as an answer nobody can act on.
    #[test]
    fn a_document_that_is_not_json_never_leaves_the_producer() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");

        let refused = producer.publish("flow", "{not json");

        assert_eq!(
            refused.expect_err("it was refused").kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(
            inbox.recv_timeout(Duration::from_millis(200)).is_err(),
            "it reached the window anyway"
        );
    }

    /// Ops that are not JSON are refused **to their author, here**, exactly as a document that is not
    /// JSON is: a list nobody can read must not travel to the window and come back as a refusal its
    /// sender has to guess the origin of.
    #[test]
    fn ops_that_are_not_json_never_leave_the_producer() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");

        let refused = producer.patch("[{op: delete}");

        assert_eq!(
            refused.expect_err("it was refused").kind(),
            io::ErrorKind::InvalidInput
        );
        // **This second half is the one that names the test, and on its own it proves nothing**: a
        // request that DID leave would be refused at the wire, on the connection's own thread, and
        // reach no delivery either. What makes it worth keeping is the pane's own answer: a producer
        // that had written would have one waiting, and one that stopped at home has nothing to read.
        assert!(
            inbox.recv_timeout(Duration::from_millis(200)).is_err(),
            "the pane was handed something to answer"
        );
        // And the connection is still usable afterwards, which is the point of refusing at home
        // rather than letting the wire do it: a producer that got its own argument wrong keeps its
        // hold and can send the corrected ops down the same socket.
        assert!(
            producer.patch(r#"[{"op":"last","line":"after"}]"#).is_ok(),
            "a bad argument took the connection with it"
        );
    }

    /// A source with a quote in it cannot break the request it travels in, which is the same hazard
    /// the reply's escaping answers on the other side.
    #[test]
    fn a_source_carrying_quotes_does_not_break_the_request() {
        let (dir, _channel, inbox) = a_pane();
        let mut producer = Producer::of(dir.path(), dir.id()).expect("open the channel");

        let window = std::thread::spawn(move || {
            let delivery = inbox
                .recv_timeout(Duration::from_secs(5))
                .expect("the window is handed the document");
            let What::Publish { source, .. } = delivery.what else {
                panic!("the window was handed something else");
            };
            let _ = delivery.answer.send(Answer::Taken);
            source
        });

        let answered = producer
            .publish("say \"hello\"", "{\"lines\":[]}")
            .expect("publish");

        assert_eq!(answered, Answered::Ok);
        assert_eq!(
            window.join().expect("the window thread finished"),
            "say \"hello\""
        );
    }
}
