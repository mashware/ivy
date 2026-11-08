//! Keep only the last explicit title event, without putting a lock in the painter's accessor.

use alacritty_terminal::event::{Event, EventListener};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub(super) struct Listener {
    // Outer None means no update; Some(None) explicitly clears the previously observed title.
    pending: Arc<Mutex<Option<Option<String>>>>,
}

impl Listener {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Screen, Size};

    fn screen() -> Screen {
        Screen::new(Size { rows: 8, cols: 40 })
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
