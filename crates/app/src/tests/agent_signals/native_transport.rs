//! Native negotiation, backpressure and client handoff over the real PTY boundary.
use super::*;

#[test]
fn deferred_capability_reply_is_one_value_and_disable_withdraws_it() {
    let mut pane = pane_with("sleep 8", 0);
    configure(&mut pane);
    for _ in 0..100 {
        assert!(!pane.signals.take_capability_reply(true, false));
    }
    assert!(pane.signals.take_capability_reply(false, true));
    assert!(!pane.signals.take_capability_reply(false, true));
    assert!(!pane.signals.take_capability_reply(true, false));
    pane.signals.configure(Default::default(), &mut pane.screen);
    assert!(!pane.signals.take_capability_reply(false, true));
}

#[cfg(unix)]
#[test]
fn automatic_capability_does_not_refuse_human_input_when_writer_is_backpressured() {
    let mut pane = pane_with("stty raw -echo && printf ready && exec sleep 8", 0);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !pane.screen.to_text().contains("ready") {
        pane.pump(enabled());
        assert!(
            Instant::now() < deadline,
            "raw fixture did not become ready"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    pane.pty.write(&vec![b'x'; 512 * 1024]).unwrap();
    for _ in 0..100 {
        pane.screen.advance(b"\x1b]7501;?\x07");
        pane.pump(enabled());
    }
    let backlog = pane
        .pty
        .backlog()
        .expect("the raw program never reads its input");
    assert!(
        backlog.bytes > 256 * 1024,
        "fixture did not hold enough input to reject an extra write"
    );
    assert_eq!(
        backlog.refusals, 0,
        "automatic metadata falsely reported a refused user keystroke"
    );
    assert!(!backlog.refused);
}

#[cfg(unix)]
#[test]
fn healthy_native_client_receives_the_exact_capability_response() {
    let path = std::env::temp_dir().join(format!(
        "ivy-capability-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let response = b"\x1b]7501;?\x07";
    let quoted = path.to_string_lossy().replace('\'', "'\\''");
    let script = format!(
        "stty raw -echo && printf '\\033]7501;?\\007' && dd bs=1 count={} of='{}' 2>/dev/null && exec sleep 8",
        response.len(),
        quoted
    );
    let mut pane = pane_with(&script, 0);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        pane.pump(enabled());
        if std::fs::read(&path).ok().as_deref() == Some(response) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "client never received the expected capability bytes"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(std::fs::read(&path).unwrap(), response);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_fresh_native_client_can_replace_a_disconnected_tui_but_a_clear_cannot() {
    let mut pane = pane_with("sleep 8", 0);
    configure(&mut pane);
    let document = publish_flow(&mut pane, "block");
    let now = Instant::now();
    pane.signals
        .receive(81, Some(snapshot(false, true)), now)
        .unwrap();
    pane.signals.closed(81, now);
    native(&mut pane, "state=clear", now);
    assert!(pane.signals.view(now).unwrap().waiting);
    assert!(!pane.signals.view(now).unwrap().backed);
    pane.screen.advance(b"\x1b]7501;?\x07");
    let query = pane.signals.sync_native(&mut pane.screen, now);
    assert!(
        pane.signals.take_capability_reply(query, true),
        "a stale disconnected reporter blocked the next client's negotiation"
    );
    assert_eq!(
        pane.signals.view(now).unwrap().client,
        Client::OpenCode,
        "a probe alone withdrew an unconfirmed question"
    );
    native(&mut pane, "state=working:app=claude-code", now);
    let view = pane.signals.view(now).unwrap();
    assert_eq!(view.client, Client::Claude);
    assert!(view.backed && view.busy && !view.waiting);
    assert_eq!(
        pane.effective_attention(SystemTime::now()).unwrap().class,
        Attention::Block
    );
    still_flow_owned(&mut pane, &document);
}
