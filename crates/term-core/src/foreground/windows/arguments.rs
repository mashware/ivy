//! Incremental MSVC-style command-line parser. No token or payload is retained.
use crate::foreground::Argument;
use std::io;

pub(super) trait Units {
    fn next_unit(&mut self) -> io::Result<Option<u16>>;
}

struct Parser<'a, R> {
    input: &'a mut R,
}

impl<R: Units> Parser<'_, R> {
    fn next(&mut self) -> io::Result<Option<u16>> {
        self.input.next_unit()
    }
    // Returns false when the consumer rejects a prefix. The first token follows
    // the special argv[0] quoting rule documented by the Microsoft C runtime.
    fn token(
        &mut self,
        first: bool,
        mut emit: impl FnMut(u16) -> bool,
    ) -> io::Result<Option<bool>> {
        let mut ch = self.next()?;
        while matches!(ch, Some(32 | 9)) {
            ch = self.next()?;
        }
        if ch.is_none() {
            return Ok(None);
        }
        let mut quoted = false;
        loop {
            let Some(unit) = ch else {
                return Ok(Some(true));
            };
            if unit == 0 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            if !quoted && matches!(unit, 32 | 9) {
                return Ok(Some(true));
            }
            if first {
                if unit == 34 {
                    quoted = !quoted;
                } else if !emit(unit) {
                    return Ok(Some(false));
                }
                ch = self.next()?;
                continue;
            }
            if unit == 92 {
                let mut count = 1usize;
                let mut emitted = 0;
                let mut next = self.next()?;
                while next == Some(92) {
                    count += 1;
                    // Every pair emits a slash regardless of the following quote.
                    // Rejecting it now bounds lookahead for an unsupported prefix.
                    if count.is_multiple_of(2) {
                        if !emit(92) {
                            return Ok(Some(false));
                        }
                        emitted += 1;
                    }
                    next = self.next()?;
                }
                if next == Some(34) {
                    if !count.is_multiple_of(2) {
                        if !emit(34) {
                            return Ok(Some(false));
                        }
                        ch = self.next()?;
                        continue;
                    }
                    // Even runs leave the quote to the same doubled-quote rule below.
                    ch = next;
                } else {
                    for _ in emitted..count {
                        if !emit(92) {
                            return Ok(Some(false));
                        }
                    }
                    ch = next;
                    continue;
                }
            }
            if ch == Some(34) {
                if quoted {
                    let next = self.next()?;
                    if next == Some(34) {
                        if !emit(34) {
                            return Ok(Some(false));
                        }
                        ch = self.next()?;
                    } else {
                        quoted = false;
                        ch = next;
                    }
                } else {
                    quoted = true;
                    ch = self.next()?;
                }
            } else {
                if !emit(unit) {
                    return Ok(Some(false));
                }
                ch = self.next()?;
            }
        }
    }
}

fn executable_token<R: Units>(parser: &mut Parser<'_, R>) -> io::Result<bool> {
    let mut argv0 = 0;
    let mut high = false;
    let mut malformed = false;
    let first = parser.token(true, |unit| {
        argv0 += 1;
        if high {
            malformed = !(0xdc00..=0xdfff).contains(&unit);
            high = false;
        } else if (0xd800..=0xdbff).contains(&unit) {
            high = true;
        } else if (0xdc00..=0xdfff).contains(&unit) {
            malformed = true;
        }
        !malformed && argv0 <= 4096
    })?;
    if malformed || high {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(first == Some(true) && argv0 > 0)
}

pub(super) fn matches(reader: &mut impl Units, shapes: &[&[Argument<'_>]]) -> io::Result<bool> {
    let mut parser = Parser { input: reader };
    if !executable_token(&mut parser)? {
        return Ok(false);
    }
    let mut candidates: Vec<_> = shapes.to_vec();
    let mut index = 0;
    loop {
        let mut high = None;
        let mut byte_pos = 0usize;
        let result = parser.token(false, |unit| {
            // Decode UTF-16 incrementally, keeping at most one surrogate. Opaque is
            // measured in UTF-8 bytes, as in the existing Unix matcher.
            let scalar = if let Some(h) = high.take() {
                if !(0xdc00..=0xdfff).contains(&unit) {
                    high = Some(0);
                    return false;
                }
                char::from_u32(0x10000 + (((h as u32 - 0xd800) << 10) | (unit as u32 - 0xdc00)))
            } else if (0xd800..=0xdbff).contains(&unit) {
                high = Some(unit);
                return true;
            } else {
                char::from_u32(unit as u32)
            };
            let Some(scalar) = scalar else {
                high = Some(0);
                return false;
            };
            let mut bytes = [0; 4];
            for &byte in scalar.encode_utf8(&mut bytes).as_bytes() {
                candidates.retain(|shape| {
                    shape
                        .get(index)
                        .is_some_and(|arg| accepts(arg, byte_pos, byte))
                });
                byte_pos += 1;
                if candidates.is_empty() {
                    return false;
                }
            }
            true
        })?;
        if high.is_some() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        match result {
            None => return Ok(candidates.iter().any(|shape| shape.len() == index)),
            Some(false) => return Ok(false),
            Some(true) => {
                candidates
                    .retain(|shape| shape.get(index).is_some_and(|arg| complete(arg, byte_pos)));
                if candidates.is_empty() {
                    return Ok(false);
                }
                index += 1;
            }
        }
    }
}

fn accepts(argument: &Argument<'_>, position: usize, byte: u8) -> bool {
    match argument {
        Argument::Literal(value) => value.as_bytes().get(position) == Some(&byte),
        Argument::Uuid => match position {
            8 | 13 | 18 | 23 => byte == b'-',
            0..=35 => byte.is_ascii_hexdigit(),
            _ => false,
        },
        Argument::Opaque { max_bytes } => position < *max_bytes,
    }
}
fn complete(argument: &Argument<'_>, position: usize) -> bool {
    match argument {
        Argument::Literal(value) => position == value.len(),
        Argument::Uuid => position == 36,
        Argument::Opaque { .. } => position > 0,
    }
}

pub(super) fn script_slot(reader: &mut impl Units) -> io::Result<Option<String>> {
    let mut parser = Parser { input: reader };
    if !executable_token(&mut parser)? {
        return Ok(None);
    }
    let mut units = Vec::new();
    if parser.token(false, |unit| {
        units.push(unit);
        units.len() <= 4096
    })? != Some(true)
    {
        return Ok(None);
    }
    String::from_utf16(&units)
        .map(Some)
        .map_err(|_| io::ErrorKind::InvalidData.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Source {
        data: Vec<u16>,
        pos: usize,
    }
    impl Units for Source {
        fn next_unit(&mut self) -> io::Result<Option<u16>> {
            let v = self.data.get(self.pos).copied();
            self.pos += usize::from(v.is_some());
            Ok(v)
        }
    }
    fn source(s: &str) -> Source {
        Source {
            data: s.encode_utf16().collect(),
            pos: 0,
        }
    }
    #[test]
    fn quotes_unicode_empty_and_backslashes() {
        let mut s = source("\"C:\\Program Files\\x.exe\" \"hola 世界\" \"\" a\\\\\\\"b");
        let shape = [
            Argument::Literal("hola 世界"),
            Argument::Literal(""),
            Argument::Literal("a\\\"b"),
        ];
        assert!(matches(&mut s, &[&shape]).unwrap());
    }
    #[test]
    fn rejects_before_payload() {
        let mut s = source("x --bad secret-payload");
        let shape = [
            Argument::Literal("--good"),
            Argument::Opaque { max_bytes: 100 },
        ];
        assert!(!matches(&mut s, &[&shape]).unwrap());
        assert!(s.pos < "x --bad secret-payload".encode_utf16().count());
    }
    #[test]
    fn rejected_backslash_run_stops_before_the_rest_of_the_argument() {
        let mut input = source(&format!("x {}private", "\\".repeat(1000)));
        assert!(!matches(&mut input, &[&[Argument::Literal("--good")]]).unwrap());
        assert!(
            input.pos <= 4,
            "unsupported slash prefix read {} units",
            input.pos
        );
    }
    #[test]
    fn even_backslashes_preserve_doubled_quotes_inside_quoted_argument() {
        let mut input = source(r#"x "a\\""b""#);
        assert!(matches(&mut input, &[&[Argument::Literal("a\\\"b")]]).unwrap());
    }
    #[test]
    fn reads_only_the_script_slot() {
        let mut input = source("x \"C:\\some path\\app.js\" later-secret");
        assert_eq!(
            script_slot(&mut input).unwrap().as_deref(),
            Some("C:\\some path\\app.js")
        );
        assert!(input.pos < input.data.len());
    }
    #[test]
    fn uuid_requires_exact_hyphens_and_hex_digits() {
        let shape = [Argument::Uuid];
        assert!(
            matches(
                &mut source("x 12345678-abcd-1234-ABCD-123456789abc"),
                &[&shape]
            )
            .unwrap()
        );
        assert!(
            !matches(
                &mut source("x 12345678_abcd-1234-ABCD-123456789abc"),
                &[&shape]
            )
            .unwrap()
        );
    }
    #[test]
    fn invalid_utf16_is_an_error() {
        let mut input = Source {
            data: vec![b'x' as u16, b' ' as u16, 0xd800],
            pos: 0,
        };
        assert_eq!(
            matches(&mut input, &[&[Argument::Opaque { max_bytes: 8 }]])
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn malformed_executable_is_rejected_by_both_invocation_readers() {
        for prefix in [vec![0xd800], vec![0xdc00], vec![0xd800, u16::from(b'x')]] {
            let mut data = prefix;
            data.push(u16::from(b' '));
            data.extend(r"C:\valid\app.js".encode_utf16());
            let mut input = Source {
                data: data.clone(),
                pos: 0,
            };
            assert_eq!(
                script_slot(&mut input).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
            let mut input = Source { data, pos: 0 };
            assert_eq!(
                matches(&mut input, &[&[Argument::Literal(r"C:\valid\app.js")]])
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
        let mut input = source("x😀 C:\\valid\\app.js");
        assert_eq!(
            script_slot(&mut input).unwrap().as_deref(),
            Some(r"C:\valid\app.js")
        );
    }
    #[test]
    fn opaque_counts_utf8_and_requires_nonempty() {
        let shape = [Argument::Opaque { max_bytes: 3 }];
        assert!(matches(&mut source("x 猫"), &[&shape]).unwrap());
        assert!(!matches(&mut source("x 猫a"), &[&shape]).unwrap());
        assert!(!matches(&mut source("x \"\""), &[&shape]).unwrap());
    }
}
