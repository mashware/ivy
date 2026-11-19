//! Insertion-point editing for the search's ordered input steps.
use super::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Start,
    End,
    Stay,
}

impl Mode {
    pub(super) fn move_caret(&mut self, motion: Motion) {
        self.caret = match motion {
            Motion::Left => self.previous(),
            Motion::Right => self.next(),
            Motion::WordLeft => self.previous_word(),
            Motion::WordRight => self.next_word(),
            Motion::Start => 0,
            Motion::End => self.query.len(),
            Motion::Stay => self.caret,
        };
    }

    pub(super) fn insert(&mut self, text: &str) {
        let text: String = text
            .trim_end_matches(char::is_control)
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        self.query.insert_str(self.caret, &text);
        self.caret += text.len();
    }

    pub(super) fn backspace(&mut self) {
        let start = self.previous();
        self.query.drain(start..self.caret);
        self.caret = start;
    }

    pub(super) fn delete_word(&mut self) {
        let start = self.previous_word();
        self.query.drain(start..self.caret);
        self.caret = start;
    }

    pub(super) fn delete_forward(&mut self, word: bool) {
        let end = if word { self.next_word() } else { self.next() };
        self.query.drain(self.caret..end);
    }

    fn previous(&self) -> usize {
        self.query[..self.caret]
            .char_indices()
            .next_back()
            .map_or(0, |(at, _)| at)
    }

    fn next(&self) -> usize {
        self.query[self.caret..]
            .chars()
            .next()
            .map_or(self.caret, |c| self.caret + c.len_utf8())
    }

    fn previous_word(&self) -> usize {
        self.query[..self.caret]
            .trim_end()
            .trim_end_matches(|c: char| !c.is_whitespace())
            .len()
    }

    fn next_word(&self) -> usize {
        let tail = &self.query[self.caret..];
        let rest = tail
            .trim_start_matches(|c: char| !c.is_whitespace())
            .trim_start();
        self.query.len() - rest.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{OnThisConsole, Step, Steps, apply};
    use term_core::{Screen, Size};

    #[test]
    fn navigation_edits_at_the_insertion_position_and_keeps_unicode_valid() {
        let mut mode = Mode::default();
        mode.insert("A日🙂Z");
        mode.move_caret(Motion::Left);
        mode.insert("!");
        assert_eq!(mode.query, "A日🙂!Z");
        mode.backspace();
        mode.backspace();
        assert_eq!(mode.query, "A日Z");
        mode.move_caret(Motion::Left);
        mode.delete_forward(false);
        assert_eq!(mode.query, "AZ");
        mode.move_caret(Motion::Start);
        mode.backspace();
        mode.insert("x");
        mode.move_caret(Motion::End);
        mode.delete_forward(false);
        mode.insert("y");
        assert_eq!(mode.query, "xAZy");
    }

    #[test]
    fn word_edits_and_paste_use_the_caret() {
        let mut mode = Mode::default();
        mode.insert("one two three");
        mode.move_caret(Motion::WordLeft);
        mode.delete_word();
        assert_eq!(mode.query, "one three");
        mode.insert("日\n本\n");
        assert_eq!(mode.query, "one 日 本three");
        mode.move_caret(Motion::Start);
        mode.move_caret(Motion::WordRight);
        assert_eq!(&mode.query[..mode.caret], "one ");
        mode.delete_forward(true);
        assert_eq!(mode.query, "one 本three");
    }

    #[test]
    fn navigation_preserves_compiled_query_but_edits_invalidate_it() {
        let mut screen = Screen::new(Size::default());
        screen.advance(b"Ask");
        let mut console = OnThisConsole {
            open: Some(Mode::default()),
            ..Default::default()
        };
        let steps = |asked| Steps { open: true, asked };
        apply(
            &mut console,
            &mut screen,
            steps(vec![Step::Type("Ask".into())]),
        );
        console.mark(&screen, &mut screen.view());
        apply(
            &mut console,
            &mut screen,
            steps(vec![Step::Move(Motion::Left)]),
        );
        assert!(console.open.as_ref().unwrap().compiled.is_some());
        apply(
            &mut console,
            &mut screen,
            steps(vec![Step::Type("!".into())]),
        );
        let mode = console.open.as_ref().unwrap();
        assert_eq!(mode.query(), "As!k");
        assert!(mode.compiled.is_none());
    }
}
