//! Keep explicit titles and bounded, audited terminal protocol replies.

use alacritty_terminal::event::{Event, EventListener};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub(super) struct Listener {
    // Outer None means no update; Some(None) explicitly clears the previously observed title.
    pending: Arc<Mutex<Option<Option<String>>>>,
    replies: Arc<Mutex<Option<ReplyBatch>>>,
}

// Program output is an automatic producer, unlike keystrokes. Bound its temporary replies too.
const REPLY_LIMIT: usize = 16 * 1024;

#[derive(Default)]
pub(super) struct ReplyBatch {
    pub(super) bytes: Vec<u8>,
    pub(super) error: Option<&'static str>,
}

impl Listener {
    pub(super) fn begin_replies(&self) {
        *self.replies.lock().expect("terminal reply slot poisoned") = Some(ReplyBatch::default());
    }

    pub(super) fn end_replies(&self) -> ReplyBatch {
        self.replies
            .lock()
            .expect("terminal reply slot poisoned")
            .take()
            .expect("terminal reply collection was started")
    }

    pub(super) fn take(&self) -> Option<Option<String>> {
        self.pending
            .lock()
            .expect("terminal title slot poisoned")
            .take()
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let update = match event {
            Event::PtyWrite(text) => {
                let mut slot = self.replies.lock().expect("terminal reply slot poisoned");
                if let Some(batch) = slot.as_mut() {
                    if !safe_reply(&text) {
                        batch.error = Some("terminal engine generated an unsupported query reply");
                    } else if text.len() > REPLY_LIMIT.saturating_sub(batch.bytes.len()) {
                        batch.error = Some("too many terminal query replies in one output batch");
                    } else if batch.error.is_none() {
                        batch.bytes.extend_from_slice(text.as_bytes());
                    }
                }
                return;
            }
            Event::Title(title) => {
                let valid = !title.chars().any(char::is_control) && title.chars().count() <= 128;
                let trimmed = title.trim();
                (valid && !trimmed.is_empty()).then(|| trimmed.to_owned())
            }
            Event::ResetTitle => None,
            _ => return,
        };
        *self.pending.lock().expect("terminal title slot poisoned") = Some(update);
    }
}

// Audit the shape, not arbitrary text: a future engine must not turn a title into shell input.
fn safe_reply(text: &str) -> bool {
    let Some(body) = text.strip_prefix("\x1b[") else {
        return false;
    };
    fn numbers(text: &str, count: usize) -> bool {
        text.split(';').count() == count
            && text
                .split(';')
                .all(|n| !n.is_empty() && n.len() <= 10 && n.bytes().all(|b| b.is_ascii_digit()))
    }
    if matches!(body, "0n" | "?6c") {
        return true;
    }
    if let Some(args) = body.strip_suffix('R') {
        return numbers(args, 2);
    }
    if let Some(version) = body.strip_prefix(">0;").and_then(|s| s.strip_suffix(";1c")) {
        return numbers(version, 1);
    }
    if let Some(size) = body.strip_prefix("8;").and_then(|s| s.strip_suffix('t')) {
        return numbers(size, 2);
    }
    if let Some(mode) = body.strip_prefix('?').and_then(|s| s.strip_suffix('u')) {
        return numbers(mode, 1);
    }
    if let Some(mode) = body.strip_suffix("$y") {
        return numbers(mode.strip_prefix('?').unwrap_or(mode), 2);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Screen, Size};

    fn screen() -> Screen {
        Screen::new(Size { rows: 8, cols: 40 })
    }

    #[test]
    fn query_replies_preserve_order_across_every_input_cut_and_current_dimensions() {
        let input = b"\x1b[3;7H\x1b[5n\x1b[6n\x1b[c\x1b[>c\x1b[18t\x1b[?25$p\x1b[4$p";
        let expected = b"\x1b[0n\x1b[3;7R\x1b[?6c\x1b[>0;2600;1c\x1b[8;8;40t\x1b[?25;1$y\x1b[4;2$y";
        for cut in 0..=input.len() {
            let mut screen = screen();
            let mut got = Vec::new();
            for chunk in [&input[..cut], &input[cut..]] {
                screen
                    .advance_with_replies(chunk, |reply| {
                        got.extend_from_slice(reply);
                        Ok(())
                    })
                    .unwrap();
            }
            assert_eq!(got, expected);
            screen.resize(Size { rows: 9, cols: 51 });
            got.clear();
            screen
                .advance_with_replies(b"\x1b[18t", |reply| {
                    got.extend_from_slice(reply);
                    Ok(())
                })
                .unwrap();
            assert_eq!(got, b"\x1b[8;9;51t");
        }
    }

    #[test]
    fn query_replies_cannot_reflect_program_strings_or_cross_screens() {
        let mut first = screen();
        let mut second = screen();
        let mut replies = Vec::new();
        first
            .advance_with_replies(
                b"\x1b]2;echo injected\x07\x1b[21t\x1b]52;c;?\x07\x1b]10;?\x07\x1b[14t\x05",
                |reply| {
                    replies.extend_from_slice(reply);
                    Ok(())
                },
            )
            .unwrap();
        assert!(replies.is_empty());
        assert_eq!(first.terminal_title(), Some("echo injected"));
        second
            .advance_with_replies(b"\x1b[6n", |reply| {
                replies.extend_from_slice(reply);
                Ok(())
            })
            .unwrap();
        assert_eq!(replies, b"\x1b[1;1R");
        for text in [
            "echo injected\n",
            "\x1b]ltext\x1b\\",
            "\x1b[1;2R\n",
            "\x1b[1;echoR",
            "\x1b[1;;2R",
        ] {
            assert!(!safe_reply(text), "untrusted reply: {text:?}");
        }
        let listener = Listener::default();
        listener.begin_replies();
        listener.send_event(Event::PtyWrite("echo injected\n".into()));
        let batch = listener.end_replies();
        assert!(batch.error.is_some());
        assert!(batch.bytes.is_empty());
        assert!(safe_reply("\x1b[?0u"));
    }

    #[test]
    fn query_reply_flood_is_bounded_and_failure_does_not_replay_later() {
        let mut screen = screen();
        let flood = b"\x1b[5n".repeat(REPLY_LIMIT / 4 + 1);
        let mut wrote = false;
        assert!(
            screen
                .advance_with_replies(&flood, |_| {
                    wrote = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!wrote);
        assert_eq!(
            screen
                .advance_with_replies(b"\x1b[6n", |_| Err("writer refused".into()))
                .unwrap_err()
                .to_string(),
            "writer refused"
        );
        screen
            .advance_with_replies(b"ordinary text", |_| panic!("a failed reply was replayed"))
            .unwrap();
        screen.advance(b"\x1b[5n");
        screen
            .advance_with_replies(b"", |_| panic!("passive parsing retained a reply"))
            .unwrap();
        let mut recovered = Vec::new();
        screen
            .advance_with_replies(b"\x1b[5n", |reply| {
                recovered.extend_from_slice(reply);
                Ok(())
            })
            .unwrap();
        assert_eq!(recovered, b"\x1b[0n");
        let listener = Listener::default();
        listener.begin_replies();
        for _ in 0..REPLY_LIMIT {
            listener.send_event(Event::PtyWrite("\x1b[0n".into()));
        }
        let batch = listener.end_replies();
        assert!(batch.bytes.len() <= REPLY_LIMIT);
        assert!(batch.error.is_some());
    }

    #[test]
    fn terminal_titles_follow_osc_zero_and_two_across_every_chunk_boundary() {
        for bytes in [b"\x1b]0;Review\x07".as_slice(), b"\x1b]2;Review\x1b\\"] {
            for cut in 0..=bytes.len() {
                let mut screen = screen();
                screen.advance(&bytes[..cut]);
                screen.advance(&bytes[cut..]);
                assert_eq!(screen.terminal_title(), Some("Review"));
                screen.advance(b"a printed session name is not metadata");
                assert_eq!(screen.terminal_title(), Some("Review"));
                screen.advance(b"\x1b]2;Ship next\x07");
                assert_eq!(screen.terminal_title(), Some("Ship next"));
            }
        }
    }

    #[test]
    fn terminal_titles_clear_on_empty_invalid_or_explicit_reset_updates() {
        let mut screen = screen();
        for title in [String::new(), "   ".into(), "x".repeat(129)] {
            screen.advance(b"\x1b]2;Review\x07");
            assert_eq!(screen.terminal_title(), Some("Review"));
            screen.advance(format!("\x1b]2;{title}\x07").as_bytes());
            assert_eq!(screen.terminal_title(), None);
        }
        // The engine owns the title stack: popping its saved default emits ResetTitle.
        let mut screen = self::screen();
        screen.advance(b"\x1b[22;2t\x1b]2;Review\x07");
        assert_eq!(screen.terminal_title(), Some("Review"));
        screen.advance(b"\x1b[23;2t");
        assert_eq!(screen.terminal_title(), None);
    }

    #[test]
    fn terminal_titles_coalesce_and_drain_each_chunk_including_clears() {
        for (bytes, expected) in [
            (b"\x1b]2;Old\x07\x1b]2;\x07".as_slice(), None),
            (b"\x1b[22;2t\x1b]2;Old\x07\x1b[23;2t", None),
            (b"\x1b[22;2t\x1b[23;2t\x1b]2;New\x07", Some("New")),
        ] {
            let mut screen = screen();
            screen.advance(bytes);
            assert_eq!(screen.terminal_title(), expected);
            screen.advance(b"ordinary output");
            assert_eq!(screen.terminal_title(), expected);
        }
    }

    #[test]
    fn terminal_titles_bound_unicode_scalars_and_keep_client_formatting() {
        let mut screen = screen();
        let boundary = "界".repeat(128);
        screen.advance(format!("\x1b]2;{boundary}\x07").as_bytes());
        assert_eq!(screen.terminal_title(), Some(boundary.as_str()));
        screen.advance(format!("\x1b]2;{boundary}界\x07").as_bytes());
        assert_eq!(screen.terminal_title(), None);
        screen.advance(b"\x1b]2;  OC | Review  \x07");
        assert_eq!(screen.terminal_title(), Some("OC | Review"));
    }

    #[test]
    fn terminal_title_listener_rejects_controls_without_replaying_its_slot() {
        let listener = Listener::default();
        listener.send_event(Event::Title("Review".into()));
        listener.send_event(Event::Title("bad\u{0085}title".into()));
        assert_eq!(listener.take(), Some(None));
        assert_eq!(listener.take(), None);
        listener.send_event(Event::Title("New".into()));
        listener.send_event(Event::Bell);
        assert_eq!(listener.take(), Some(Some("New".into())));
        assert_eq!(listener.take(), None);
    }

    #[test]
    fn terminal_titles_belong_to_their_screen_and_are_not_restored() {
        fn assert_send<T: Send>() {}
        assert_send::<Screen>();
        let mut first = screen();
        let mut second = screen();
        first.advance(b"\x1b]2;First\x07");
        second.advance(b"\x1b]2;Second\x07");
        assert_eq!(first.terminal_title(), Some("First"));
        assert_eq!(second.terminal_title(), Some("Second"));
        assert_eq!(screen().terminal_title(), None);
    }
}
