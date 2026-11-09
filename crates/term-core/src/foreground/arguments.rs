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
    let max_arguments = shapes.iter().map(|shape| shape.len()).max().unwrap_or(0);
    for index in 0..=max_arguments {
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
                        .is_some_and(|argument| complete(argument, position))
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

fn complete(argument: &Argument<'_>, position: usize) -> bool {
    match argument {
        Argument::Literal(value) => value.len() == position,
        Argument::Uuid => position == 36,
        Argument::Opaque { max_bytes } => (1..=*max_bytes).contains(&position),
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
        Argument::Opaque { max_bytes } => position < *max_bytes,
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

    fn config_shapes() -> Vec<Vec<Argument<'static>>> {
        use Argument::{Literal as L, Opaque, Uuid};
        [
            vec![L("--no-daemon"), L("-c"), Opaque { max_bytes: 8192 }],
            vec![L("--no-daemon"), L("--config"), Opaque { max_bytes: 8192 }],
            vec![
                L("--no-daemon"),
                L("-c"),
                Opaque { max_bytes: 8192 },
                L("--config"),
                Opaque { max_bytes: 8192 },
            ],
            vec![
                L("--no-daemon"),
                L("-c"),
                Opaque { max_bytes: 8192 },
                L("--config"),
                Opaque { max_bytes: 8192 },
                L("-c"),
                Opaque { max_bytes: 8192 },
                L("--config"),
                Opaque { max_bytes: 8192 },
            ],
        ]
        .into_iter()
        .flat_map(|shape| {
            let mut resume = shape.clone();
            resume.extend([L("resume"), Uuid]);
            [shape, resume]
        })
        .collect()
    }

    fn argv(arguments: &[&[u8]]) -> Vec<u8> {
        std::iter::once(b"codex".as_slice())
            .chain(arguments.iter().copied())
            .flat_map(|argument| argument.iter().copied().chain([0]))
            .collect()
    }

    #[test]
    fn opaque_values_accept_exact_aliases_mixed_pairs_and_resume_without_interpretation() {
        let shapes = config_shapes();
        let shapes: Vec<_> = shapes.iter().map(Vec::as_slice).collect();
        for arguments in [
            vec![b"--no-daemon".as_slice(), b"-c", b"model=exec"],
            vec![
                b"--no-daemon".as_slice(),
                b"--config",
                b"review app-server --remote",
            ],
            vec![
                b"--no-daemon".as_slice(),
                b"-c",
                b"first=value",
                b"--config",
                b"second=value",
            ],
            vec![
                b"--no-daemon".as_slice(),
                b"-c",
                b"1",
                b"--config",
                b"2",
                b"-c",
                b"3",
                b"--config",
                b"4",
                b"resume",
                b"11111111-1111-4111-8111-111111111111",
            ],
        ] {
            assert!(matches(&mut argv(&arguments).as_slice(), &shapes).unwrap());
        }
        let value = vec![0xff; 8192];
        assert!(
            matches(
                &mut argv(&[b"--no-daemon", b"-c", &value]).as_slice(),
                &shapes,
            )
            .unwrap()
        );
    }

    #[test]
    fn opaque_values_reject_empty_missing_truncated_and_oversized_arguments() {
        let shapes = config_shapes();
        let shapes: Vec<_> = shapes.iter().map(Vec::as_slice).collect();
        for input in [
            argv(&[b"--no-daemon", b"-c", b""]),
            argv(&[b"--no-daemon", b"-c"]),
            b"codex\0--no-daemon\0-c\0unterminated".to_vec(),
            argv(&[b"--no-daemon", b"-c", &vec![b'x'; 8193]]),
        ] {
            assert!(!matches(&mut input.as_slice(), &shapes).unwrap());
        }
    }

    #[test]
    fn config_shapes_reject_hybrid_aliases_extra_pairs_modes_and_payloads_early() {
        let shapes = config_shapes();
        let shapes: Vec<_> = shapes.iter().map(Vec::as_slice).collect();
        for mode in [
            b"exec".as_slice(),
            b"review",
            b"app-server",
            b"server",
            b"--remote",
            b"--unknown",
            b"unknown-command",
            b"-cconfig",
            b"--c",
            b"--config-extra",
        ] {
            let prefix = argv(&[b"--no-daemon", b"-c", b"model=value", mode]);
            let mut input = prefix.clone();
            input.extend_from_slice(b"private payload\0");
            let mut reader = io::Cursor::new(input);
            assert!(!matches(&mut reader, &shapes).unwrap());
            assert!(reader.position() <= prefix.len() as u64);
        }
        for arguments in [
            vec![b"--no-daemon".as_slice(), b"-cconfig", b"private payload"],
            vec![b"--no-daemon".as_slice(), b"-c", b"value", b"extra prompt"],
            vec![
                b"--no-daemon".as_slice(),
                b"-c",
                b"1",
                b"--config",
                b"2",
                b"-c",
                b"3",
                b"--config",
                b"4",
                b"-c",
                b"private payload",
            ],
            vec![
                b"--no-daemon".as_slice(),
                b"-c",
                b"value",
                b"resume",
                b"11111111-1111-4111-8111-111111111111",
                b"extra prompt",
            ],
        ] {
            let input = argv(&arguments);
            let payload = input
                .windows(7)
                .position(|part| part == b"private")
                .or_else(|| input.windows(5).position(|part| part == b"extra"))
                .unwrap();
            let mut reader = io::Cursor::new(input);
            assert!(!matches(&mut reader, &shapes).unwrap());
            assert!(reader.position() <= payload as u64 + 1);
        }
    }
}
