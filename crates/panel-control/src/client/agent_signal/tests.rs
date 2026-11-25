use super::*;
use std::cell::RefCell;

struct Slow {
    reply: &'static [u8],
    delay: Duration,
    waits: RefCell<Vec<Duration>>,
    written: Vec<u8>,
}
impl Read for Slow {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        std::thread::sleep(self.delay);
        if self.reply.is_empty() {
            return Ok(0);
        }
        buffer[0] = self.reply[0];
        self.reply = &self.reply[1..];
        Ok(1)
    }
}
impl Write for Slow {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        std::thread::sleep(self.delay);
        self.written.push(buffer[0]);
        Ok(1)
    }
    fn flush(&mut self) -> io::Result<()> {
        std::thread::sleep(self.delay);
        Ok(())
    }
}
impl TimedIo for Slow {
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
    let mut io = Slow {
        reply: b"{\"ok\":true}\n",
        delay: Duration::from_millis(8),
        waits: RefCell::default(),
        written: Vec::new(),
    };
    let start = Instant::now();
    let error = exchange(&mut io, b"abc", start + Duration::from_millis(70)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(io.written, b"abc");
    assert!(
        io.reply.len() > 1,
        "the trickled reply must not finish on renewed budgets"
    );
    assert!(start.elapsed() < Duration::from_millis(250));
    let waits = io.waits.borrow();
    assert!(waits.len() >= 5, "exercise writes, flush and reads");
    assert!(
        waits.windows(2).all(|w| w[1] < w[0]),
        "timeouts must decrease: {waits:?}"
    );
}

#[test]
fn deadline_is_checked_after_a_successful_final_transfer() {
    let mut io = Slow {
        reply: b"\n",
        delay: Duration::from_millis(20),
        waits: RefCell::default(),
        written: Vec::new(),
    };
    assert_eq!(
        exchange(&mut io, b"", Instant::now() + Duration::from_millis(30))
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
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
