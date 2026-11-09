use super::super::Argument;
use std::io::{self, Read};

pub(super) fn matches(reader: &mut impl Read, shapes: &[&[Argument<'_>]]) -> io::Result<bool> {
    // argv0 is executable identity. Read it independently so no buffer prefetches a later payload.
    for _ in 0..4097 {
        match byte(reader)? {
            Some(0) => return remaining(reader, shapes),
            Some(_) => {}
            None => return Ok(false),
        }
    }
    Ok(false)
}

fn remaining(reader: &mut impl Read, shapes: &[&[Argument<'_>]]) -> io::Result<bool> {
    let mut candidates: Vec<_> = shapes.iter().map(|shape| shape.to_vec()).collect();
    for index in 0..4 {
        let mut position = 0;
        loop {
            let next = byte(reader)?;
            if position == 0 && next.is_none() {
                return Ok(candidates.iter().any(|shape| shape.len() == index));
            }
            let Some(next) = next else {
                return Ok(false);
            };
            if next == 0 {
                candidates.retain(|shape| {
                    shape
                        .get(index)
                        .is_some_and(|argument| length(argument) == position)
                });
                if candidates.is_empty() {
                    return Ok(false);
                }
                break;
            }
            candidates.retain(|shape| {
                shape
                    .get(index)
                    .is_some_and(|argument| accepts(argument, position, next))
            });
            if candidates.is_empty() {
                return Ok(false);
            }
            position += 1;
        }
    }
    Ok(false)
}

fn length(argument: &Argument<'_>) -> usize {
    match argument {
        Argument::Literal(value) => value.len(),
        Argument::Uuid => 36,
    }
}

fn accepts(argument: &Argument<'_>, position: usize, byte: u8) -> bool {
    match argument {
        Argument::Literal(value) => value.as_bytes().get(position) == Some(&byte),
        Argument::Uuid => {
            position < 36
                && if matches!(position, 8 | 13 | 18 | 23) {
                    byte == b'-'
                } else {
                    byte.is_ascii_hexdigit()
                }
        }
    }
}

fn byte(reader: &mut impl Read) -> io::Result<Option<u8>> {
    let mut byte = [0];
    match reader.read(&mut byte) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(byte[0])),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shapes() -> Vec<Vec<Argument<'static>>> {
        vec![
            vec![],
            vec![Argument::Literal("--no-daemon")],
            vec![
                Argument::Literal("resume"),
                Argument::Literal("--no-daemon"),
                Argument::Uuid,
            ],
        ]
    }

    #[test]
    fn mode_prefix_stops_before_headless_payload_and_unknown_flags() {
        let shapes = shapes();
        let shapes: Vec<_> = shapes.iter().map(Vec::as_slice).collect();
        for input in [
            b"codex\0exec\0private prompt\0".as_slice(),
            b"codex\0--unknown\0private prompt\0".as_slice(),
        ] {
            let mut reader = std::io::Cursor::new(input);
            assert!(!matches(&mut reader, &shapes).unwrap());
            assert!(
                reader.position() < input.iter().position(|byte| *byte == b'p').unwrap() as u64
            );
        }
        let mut reader =
            b"codex\0resume\0--no-daemon\x0011111111-1111-4111-8111-111111111111\0".as_slice();
        assert!(matches(&mut reader, &shapes).unwrap());
    }
}
