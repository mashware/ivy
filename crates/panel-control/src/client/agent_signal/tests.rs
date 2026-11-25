use super::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct ScriptedIo {
    reply: &'static [u8],
    step: Duration,
    clock: Rc<Cell<Instant>>,
    waits: RefCell<Vec<Duration>>,
    written: Vec<u8>,
    flushes: usize,
    reads: usize,
}
impl ScriptedIo {
    fn new(reply: &'static [u8], step: Duration, clock: Rc<Cell<Instant>>) -> Self {
        Self {
            reply,
            step,
            clock,
            waits: RefCell::default(),
            written: Vec::new(),
            flushes: 0,
            reads: 0,
        }
    }
    fn advance(&self) {
        self.clock.set(self.clock.get() + self.step);
    }
}
impl Read for ScriptedIo {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.advance();
        self.reads += 1;
        if self.reply.is_empty() {
            return Ok(0);
        }
        buffer[0] = self.reply[0];
        self.reply = &self.reply[1..];
        Ok(1)
    }
}
impl Write for ScriptedIo {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.advance();
        self.written.push(buffer[0]);
        Ok(1)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.advance();
        self.flushes += 1;
        Ok(())
    }
}
impl TimedIo for ScriptedIo {
    fn receive_timeout(&self, remaining: Duration) -> io::Result<()> {
        self.waits.borrow_mut().push(remaining);
        Ok(())
    }
    fn send_timeout(&self, remaining: Duration) -> io::Result<()> {
        self.waits.borrow_mut().push(remaining);
        Ok(())
    }
}

#[test]
fn delayed_write_flush_and_trickled_reply_share_one_absolute_deadline() {
    let start = Instant::now();
    let clock = Rc::new(Cell::new(start));
    let mut io = ScriptedIo::new(
        b"{\"ok\":true}\n",
        Duration::from_millis(8),
        Rc::clone(&clock),
    );
    let error = exchange(&mut io, b"abc", start + Duration::from_millis(70), || {
        clock.get()
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(io.written, b"abc");
    assert_eq!(io.flushes, 1);
    assert_eq!(
        io.reads, 5,
        "the deadline must expire during the trickled reply"
    );
    assert_eq!(
        io.reply, b":true}\n",
        "a renewed budget would consume the complete reply"
    );
    assert_eq!(clock.get().duration_since(start), Duration::from_millis(72));
    assert_eq!(
        *io.waits.borrow(),
        [70, 62, 54, 46, 38, 30, 22, 14, 6].map(Duration::from_millis),
        "writes, flush and reads must receive only the same deadline's remaining budget"
    );
}

#[test]
fn deadline_is_checked_after_a_successful_final_transfer() {
    let start = Instant::now();
    let clock = Rc::new(Cell::new(start));
    let mut io = ScriptedIo::new(
        b"{\"ok\":true}\n",
        Duration::from_millis(2),
        Rc::clone(&clock),
    );
    let error = exchange(&mut io, b"", start + Duration::from_millis(25), || {
        clock.get()
    })
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(io.flushes, 1);
    assert_eq!(io.reads, 12);
    assert!(
        io.reply.is_empty(),
        "the final successful newline must cross the deadline"
    );
    assert_eq!(clock.get().duration_since(start), Duration::from_millis(26));
    assert_eq!(
        *io.waits.borrow(),
        [25, 23, 21, 19, 17, 15, 13, 11, 9, 7, 5, 3, 1].map(Duration::from_millis)
    );
}

#[test]
fn replies_preserve_explicit_retryability_and_reject_other_protocol_shapes() {
    assert_eq!(
        decode(br#"{"ok":true}"#).unwrap(),
        SignalAnswer::Accepted { ok: true }
    );
    for (line, retryable) in [
        (
            br#"{"ok":false,"error":"retry this","retryable":true}"#.as_slice(),
            true,
        ),
        (br#"{"ok":false,"error":"retry this"}"#.as_slice(), false),
    ] {
        assert_eq!(
            decode(line).unwrap(),
            SignalAnswer::Refused {
                ok: false,
                error: "retry this".into(),
                retryable
            }
        );
    }
    for line in [
        r#"{"ok":true,"error":"bad"}"#,
        r#"{"ok":false}"#,
        r#"{"ok":false,"error":"bad","retryable":"true"}"#,
        r#"{"ok":true,"pane":"other"}"#,
        r#"{"ok":true,"ok":false}"#,
    ] {
        assert!(decode(line.as_bytes()).is_err(), "accepted {line}");
    }
}

#[test]
fn helper_replies_serialize_the_exact_approved_shapes() {
    for line in [
        br#"{"ok":true}"#.as_slice(),
        br#"{"ok":false,"error":"disabled"}"#.as_slice(),
        br#"{"ok":false,"error":"busy","retryable":true}"#.as_slice(),
    ] {
        let answer = decode(line).unwrap();
        assert_eq!(serde_json::to_vec(&answer).unwrap(), line);
    }
    let explicit_false = decode(br#"{"ok":false,"error":"disabled","retryable":false}"#).unwrap();
    assert_eq!(
        serde_json::to_vec(&explicit_false).unwrap(),
        br#"{"ok":false,"error":"disabled"}"#
    );
}
