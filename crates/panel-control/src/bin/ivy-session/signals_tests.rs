use super::*;
use std::io::{BufReader, Cursor};

const VALID: &str = r#"{"tool":"agent_signal","v":1,"client":"opencode","snapshot":{"session":"ses_root","activity":"busy","permission":false,"question":true}}"#;

#[test]
fn only_exact_opencode_envelopes_become_typed_metadata() {
    let snapshot = parse(VALID.as_bytes()).unwrap().unwrap();
    assert_eq!(snapshot.session, "ses_root");
    assert!(snapshot.question && !snapshot.stale);
    assert_eq!(
        parse(br#"{"tool":"agent_signal","v":1,"client":"opencode","snapshot":null}"#).unwrap(),
        None
    );
    let mut invalid = vec![
        VALID.replace("agent_signal", "panel_set"),
        VALID.replace("opencode", "codex"),
        VALID.replace("\"v\":1", "\"v\":2"),
        VALID.replace("busy", "working"),
        VALID.replace("\"question\":true", "\"question\":1"),
        VALID.replace("\"v\":1", "\"v\":1,\"extra\":false"),
        VALID.replace("\"question\":true", "\"question\":true,\"extra\":false"),
        VALID.replace("ses_root", ""),
        VALID.replace("ses_root", "x\\ny"),
        VALID.replace("ses_root", &"é".repeat(129)),
        r#"{"tool":"agent_signal","v":1,"client":"opencode"}"#.into(),
        r#"{"tool":"agent_signal","v":1,"client":"opencode","snapshot":null,"snapshot":null}"#
            .into(),
    ];
    invalid.push(String::from_utf8_lossy(b"\xff").into());
    for line in invalid {
        assert!(parse(line.as_bytes()).is_err(), "accepted {line}");
    }
}

#[test]
fn chunk_boundaries_ceiling_and_eof_do_not_create_requests() {
    let text = format!(
        "{VALID}\n{{\"tool\":\"agent_signal\",\"v\":1,\"client\":\"opencode\",\"snapshot\":null}}\n"
    );
    let mut input = BufReader::with_capacity(3, Cursor::new(text));
    assert_eq!(frame(&mut input).unwrap().unwrap(), VALID.as_bytes());
    assert_eq!(parse(&frame(&mut input).unwrap().unwrap()).unwrap(), None);
    assert!(frame(&mut input).unwrap().is_none());
    assert!(frame(&mut Cursor::new(VALID)).is_err());
    assert!(frame(&mut Cursor::new(vec![b'x'; MAX_FRAME + 1])).is_err());
    let mut exact = vec![b' '; MAX_FRAME];
    exact.push(b'\n');
    assert_eq!(
        frame(&mut Cursor::new(exact)).unwrap().unwrap().len(),
        MAX_FRAME
    );
    let mut oversized = vec![b' '; MAX_FRAME + 1];
    oversized.push(b'\n');
    assert!(frame(&mut Cursor::new(oversized)).is_err());
    assert!(parse(&[0xff]).is_err());
}
