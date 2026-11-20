//! A small tokenizer for the code view: comments, strings, numbers, keywords, type names and
//! attributes, by language family. It runs on the loader thread; the frame only paints its runs.
//!
//! The owner asked for simple colouring (#159), so this is a scanner, not a grammar: a crate
//! with real grammars (`syntect`, `tree-sitter`) was set aside for its size and for bringing colour
//! themes of its own. What it gets wrong is a colour: every line keeps the file's whole text, and a
//! line too long to lay out is cut only where it is painted.
mod families;

pub(crate) use families::Family;

/// A line longer than this is painted plain and cut, with a mark: a minified file is one line of
/// megabytes, and laying that out would stall the frame.
pub(crate) const LINE_LIMIT: usize = 16 * 1024;

// Spelled as escapes: the repository's comment walkers read this file too, and a literal block
// opener or a lone quote character in the source would read to them as the start of one.
const BLOCK_OPEN: &str = "/\x2a";
const BLOCK_CLOSE: &str = "\x2a/";
const QUOTE: u8 = 0x22;
const APOSTROPHE: u8 = 0x27;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Token {
    Text,
    Comment,
    Keyword,
    String,
    Number,
    Type,
    Meta,
}

/// One line of the file: its whole text, how much of it is painted, and where each token of the
/// painted part begins, in bytes.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub(crate) text: String,
    pub(crate) runs: Vec<(usize, Token)>,
    /// The bytes painted: the whole line, or its first [`LINE_LIMIT`] cut on a character.
    pub(crate) shown: usize,
}

impl Line {
    pub(crate) fn truncated(&self) -> bool {
        self.shown < self.text.len()
    }
}

/// What a line leaves open for the next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    Nothing,
    Comment { depth: u32 },
    Text(Close),
}

/// How an open string ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Close {
    Quote(u8),
    Triple(u8),
    /// A Rust raw string: `"` followed by this many `#`.
    Raw(usize),
}

pub(crate) fn scan(text: &str, family: Family) -> Vec<Line> {
    let mut open = Open::Nothing;
    let text = text.strip_suffix('\n').unwrap_or(text);
    text.split('\n')
        .map(|raw| {
            let raw = raw.strip_suffix('\r').unwrap_or(raw);
            if raw.len() > LINE_LIMIT {
                open = Open::Nothing;
                let mut end = LINE_LIMIT;
                while !raw.is_char_boundary(end) {
                    end -= 1;
                }
                return Line {
                    text: raw.to_owned(),
                    runs: vec![(0, Token::Text)],
                    shown: end,
                };
            }
            let mut runs = Vec::new();
            if family == Family::Plain {
                runs.push((0, Token::Text));
            } else {
                open = line(raw.as_bytes(), family, open, &mut runs);
            }
            Line {
                text: raw.to_owned(),
                runs,
                shown: raw.len(),
            }
        })
        .collect()
}

fn mark(runs: &mut Vec<(usize, Token)>, at: usize, token: Token) {
    match runs.last() {
        Some((_, last)) if *last == token => {}
        Some((start, _)) if *start == at => {
            runs.pop();
            mark(runs, at, token);
        }
        _ => runs.push((at, token)),
    }
}

fn word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Scan one line, starting with what the previous one left open, and say what this one leaves.
fn line(bytes: &[u8], family: Family, mut open: Open, runs: &mut Vec<(usize, Token)>) -> Open {
    let mut i = 0;
    let at = |i: usize, s: &str| bytes[i..].starts_with(s.as_bytes());
    while i < bytes.len() {
        match open {
            Open::Comment { depth } => {
                mark(runs, i, Token::Comment);
                if at(i, BLOCK_CLOSE) {
                    i += 2;
                    open = if depth > 1 {
                        Open::Comment { depth: depth - 1 }
                    } else {
                        Open::Nothing
                    };
                } else if family == Family::Rust && at(i, BLOCK_OPEN) {
                    i += 2;
                    open = Open::Comment { depth: depth + 1 };
                } else {
                    i += 1;
                }
                continue;
            }
            Open::Text(close) => {
                mark(runs, i, Token::String);
                let (ended, next) = string_end(bytes, i, close);
                i = next;
                if ended {
                    open = Open::Nothing;
                    string_key(bytes, i, family, runs);
                }
                continue;
            }
            Open::Nothing => {}
        }
        let b = bytes[i];
        if line_comment(bytes, i, family) {
            mark(runs, i, Token::Comment);
            return Open::Nothing;
        }
        if at(i, BLOCK_OPEN)
            && matches!(
                family,
                Family::Rust | Family::CLike | Family::Css | Family::Php | Family::Sql
            )
        {
            open = Open::Comment { depth: 1 };
            mark(runs, i, Token::Comment);
            i += 2;
            continue;
        }
        if let Some((close, skip)) = string_start(bytes, i, family) {
            mark(runs, i, Token::String);
            let (ended, next) = string_end(bytes, i + skip, close);
            i = next;
            if ended {
                string_key(bytes, i, family, runs);
            } else if carries(close, family) {
                open = Open::Text(close);
            }
            continue;
        }
        if b == b'#' && family == Family::Rust && (at(i, "#[") || at(i, "#![")) {
            mark(runs, i, Token::Meta);
            i = bracket_end(bytes, i);
            continue;
        }
        if b == b'#' && family == Family::CLike && bytes[..i].iter().all(u8::is_ascii_whitespace) {
            mark(runs, i, Token::Meta);
            return Open::Nothing;
        }
        let sigil = b == b'@' || (b == b'$' && matches!(family, Family::Php | Family::Hash));
        if sigil
            && bytes
                .get(i + 1)
                .is_some_and(|n| word_byte(*n) || *n == b'{')
        {
            mark(runs, i, Token::Meta);
            i += 1;
            while i < bytes.len() && (word_byte(bytes[i]) || bytes[i] == b'{' || bytes[i] == b'}') {
                i += 1;
            }
            continue;
        }
        let after_word = i > 0 && word_byte(bytes[i - 1]);
        if b.is_ascii_digit() && !after_word {
            mark(runs, i, Token::Number);
            while i < bytes.len() && (word_byte(bytes[i]) || bytes[i] == b'.') {
                i += 1;
            }
            continue;
        }
        if (b.is_ascii_alphabetic() || b == b'_') && !after_word {
            let start = i;
            while i < bytes.len() && word_byte(bytes[i]) {
                i += 1;
            }
            let word = std::str::from_utf8(&bytes[start..i]).unwrap_or_default();
            let token = if family.is_keyword(word) {
                Token::Keyword
            } else if family.capital_is_type() && b.is_ascii_uppercase() {
                Token::Type
            } else {
                Token::Text
            };
            mark(runs, start, token);
            continue;
        }
        mark(runs, i, Token::Text);
        i += 1;
    }
    open
}

fn line_comment(bytes: &[u8], i: usize, family: Family) -> bool {
    let at = |s: &str| bytes[i..].starts_with(s.as_bytes());
    match family {
        Family::Rust | Family::CLike => at("//"),
        Family::Php => at("//") || (at("#") && !at("#[")),
        Family::Hash => at("#"),
        Family::Sql => at("--"),
        // CSS has block comments only: a `//` there is part of a `url(https://…)`.
        Family::Css | Family::Json | Family::Plain => false,
    }
}

/// The string a quote at `i` opens in this family, and how many bytes its opening takes.
fn string_start(bytes: &[u8], i: usize, family: Family) -> Option<(Close, usize)> {
    if family == Family::Plain {
        return None;
    }
    let b = bytes[i];
    let at = |s: &str| bytes[i..].starts_with(s.as_bytes());
    // A raw string, with the byte or C prefix it may carry: `r"…"`, `br#"…"#`, `cr"…"`.
    let prefix = usize::from(matches!(b, b'b' | b'c') && bytes.get(i + 1) == Some(&b'r'));
    if family == Family::Rust && bytes[i + prefix] == b'r' && (i == 0 || !word_byte(bytes[i - 1])) {
        let hashes = bytes[i + prefix + 1..]
            .iter()
            .take_while(|c| **c == b'#')
            .count();
        if bytes.get(i + prefix + 1 + hashes) == Some(&QUOTE) {
            return Some((Close::Raw(hashes), prefix + hashes + 2));
        }
    }
    if family == Family::Hash && (at("\"\"\"") || at("'''")) {
        return Some((Close::Triple(b), 3));
    }
    match b {
        QUOTE => Some((Close::Quote(b), 1)),
        APOSTROPHE if family == Family::Rust => {
            // A char literal, not a lifetime: `'x'` or an escape.
            let escaped = bytes.get(i + 1) == Some(&b'\\');
            let single = bytes.get(i + 2) == Some(&APOSTROPHE);
            (escaped || single).then_some((Close::Quote(b), 1))
        }
        APOSTROPHE if family != Family::Json => Some((Close::Quote(b), 1)),
        b'`' if family == Family::CLike => Some((Close::Quote(b), 1)),
        _ => None,
    }
}

/// Where a string that started before `i` ends: whether it closed on this line, and the byte after.
fn string_end(bytes: &[u8], mut i: usize, close: Close) -> (bool, usize) {
    while i < bytes.len() {
        match close {
            Close::Raw(hashes) => {
                if bytes[i] == QUOTE
                    && bytes[i + 1..].iter().take_while(|c| **c == b'#').count() >= hashes
                {
                    return (true, i + 1 + hashes);
                }
            }
            Close::Triple(q) => {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i..].starts_with(&[q, q, q]) {
                    return (true, i + 3);
                }
            }
            Close::Quote(q) => {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == q {
                    return (true, i + 1);
                }
            }
        }
        i += 1;
    }
    (false, bytes.len())
}

/// Whether a string left open at the end of a line goes on into the next.
fn carries(close: Close, family: Family) -> bool {
    match close {
        Close::Raw(_) | Close::Triple(_) => true,
        Close::Quote(b'`') => true,
        Close::Quote(QUOTE) => family == Family::Rust,
        Close::Quote(_) => false,
    }
}

/// In JSON a string followed by `:` is a key: it is coloured as one.
fn string_key(bytes: &[u8], end: usize, family: Family, runs: &mut [(usize, Token)]) {
    if family != Family::Json {
        return;
    }
    let colon = bytes[end..]
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| *b == b':');
    if colon && let Some(last) = runs.last_mut() {
        last.1 = Token::Keyword;
    }
}

/// The byte after the `]` that closes the attribute starting at `i`, or the end of the line.
fn bracket_end(bytes: &[u8], i: usize) -> usize {
    let mut depth = 0;
    for (offset, b) in bytes[i..].iter().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return i + offset + 1;
                }
            }
            _ => {}
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests;
