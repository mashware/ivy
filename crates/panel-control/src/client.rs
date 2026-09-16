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
    /// **Both variables are required, and neither has a fallback.** Which of the two names the
    /// endpoint is the platform's business ([`endpoint::name_of`]), so a missing one is a missing
    /// identity on one platform and harmless on the other — and defaulting the identifier to the
    /// empty string resolved every pane on Windows to a single machine-wide name. The pane sets the
    /// two together or removes them together, which is what makes half an identity impossible on the
    /// other side too.
    pub fn of_this_pane() -> io::Result<Self> {
        let outside = || {
            io::Error::new(
                io::ErrorKind::NotFound,
                "this program is not running inside a pane of this terminal",
            )
        };
        let dir = std::env::var_os(panel_contract::ENV_NAMES[1]).ok_or_else(outside)?;
        let id = std::env::var_os(panel_contract::ENV_NAMES[0]).ok_or_else(outside)?;

        Self::of(Path::new(&dir), &id.to_string_lossy())
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

        let raw: serde_json::Value = serde_json::from_slice(&line)
            .map_err(|failure| io::Error::other(failure.to_string()))?;
        match raw.get("ok").and_then(serde_json::Value::as_bool) {
            Some(true) => match raw.get("pane") {
                // **Present and possibly null**: an open answers with the handle, or with a JSON null
                // where the console has no addressable identity, and those are two different
                // answers a caller can act on differently.
                Some(pane) => Ok(Answered::Opened(pane.as_str().map(str::to_owned))),
                None => match raw.get("text").and_then(serde_json::Value::as_str) {
                    Some(text) => Ok(Answered::Text(text.to_owned())),
                    None => Ok(Answered::Ok),
                },
            },
            _ => Ok(Answered::Failed(
                raw.get("error")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("the pane refused it without saying why")
                    .to_owned(),
            )),
        }
    }
}

/// What a pane answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    /// It was applied, and the reply carried nothing else.
    Ok,
    /// A console was opened; the handle is `None` where it has no addressable identity.
    Opened(Option<String>),
    /// What a console showed.
    Text(String),
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listener::{Answer, Channel, Delivery, What};
    use panel_contract::PanelDir;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc::{Receiver, sync_channel};
    use std::time::Duration;

    fn a_pane() -> (PanelDir, Channel, Receiver<Delivery>) {
        let dir = PanelDir::create().expect("create the panel directory");
        let (sender, inbox) = sync_channel(8);
        let nobody = Arc::new(AtomicUsize::new(0));
        let listening = Channel::open(
            dir.path(),
            dir.id(),
            sender,
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
