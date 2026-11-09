#![cfg(test)]
use super::*;

const UUID: &str = "11111111-1111-4111-8111-111111111111";

#[test]
fn exact_ids_are_client_typed_and_never_shell_text() {
    for client in [Client::Codex, Client::Claude] {
        assert!(Identity::new(client, UUID).is_ok());
        for bad in [
            "--last",
            "$(touch /tmp/no)",
            "ses_anything",
            "../conversation",
            "11111111-1111-4111-8111-11111111111g",
        ] {
            assert!(Identity::new(client, bad).is_err());
        }
    }
    assert!(Identity::new(Client::Opencode, "ses_Az019").is_ok());
    for bad in ["ses_", "ses_a;b", "ses_hello world", UUID] {
        assert!(Identity::new(Client::Opencode, bad).is_err());
    }
    assert!(Identity::new(Client::Opencode, format!("ses_{}", "a".repeat(125))).is_err());
    assert!(
        serde_json::from_value::<Identity>(
            serde_json::json!({"client":"codex","session_id":"--last"})
        )
        .is_err()
    );
}

#[test]
fn literal_requests_round_trip_and_unknown_authority_is_refused() {
    let requests = [
        Request::Observe {
            client: Client::Codex,
            session_id: UUID.into(),
            cwd: "/tmp".into(),
            event: "SessionStart".into(),
            source: "resume".into(),
        },
        Request::Attach {
            client: Client::Claude,
        },
        Request::Attach {
            client: Client::Opencode,
        },
        Request::Probe {
            generation: "opaque".into(),
        },
        Request::Update {
            generation: "opaque".into(),
            sequence: 1,
            session_id: Some(UUID.into()),
            cwd: None,
            challenge: None,
        },
        Request::Update {
            generation: "opaque".into(),
            sequence: 2,
            session_id: None,
            cwd: None,
            challenge: Some("close-challenge".into()),
        },
    ];
    for request in requests {
        let line = request.line().unwrap();
        assert_eq!(Request::parse(&line).unwrap(), request);
        for authority in [
            "pid", "pane_id", "PANEL_ID", "exec", "argv", "env", "prompt",
        ] {
            let mut value = serde_json::to_value(&request).unwrap();
            value[authority] = serde_json::json!("untrusted");
            assert!(
                Request::parse(&serde_json::to_vec(&value).unwrap()).is_err(),
                "accepted {authority}"
            );
        }
    }
}

#[test]
fn metadata_fields_and_complete_request_are_bounded() {
    for bad in ["relative", "/tmp/\ncommand", "/tmp/\0nul"] {
        assert!(validate_cwd(bad).is_err());
    }
    assert!(validate_cwd(&format!("/{}", "a".repeat(4095))).is_ok());
    assert!(validate_cwd(&format!("/{}", "a".repeat(4096))).is_err());
    let over = format!(
        "{{\"tool\":\"session_probe\",\"generation\":\"g\"}}{}",
        " ".repeat(MAX_REQUEST)
    );
    assert!(Request::parse(over.as_bytes()).is_err());
    assert!(
        Request::Attach {
            client: Client::Codex
        }
        .validate()
        .is_err()
    );
    assert!(
        Request::Probe {
            generation: "g; exec".into()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn hook_discards_content_and_update_refuses_it() {
    let hook = serde_json::json!({
        "session_id":UUID,"cwd":"/tmp","hook_event_name":"SessionStart","source":"resume",
        "transcript_path":"/private/transcript", "prompt":"private prompt", "pid":1234,
        "model_output":{"secret":"do not forward"}
    });
    let request = hook_request(&serde_json::to_vec(&hook).unwrap()).unwrap();
    let encoded = String::from_utf8(request.line().unwrap()).unwrap();
    for secret in [
        "transcript",
        "private prompt",
        "pid",
        "model_output",
        "secret",
    ] {
        assert!(!encoded.contains(secret));
    }
    assert!(hook_request(&vec![b' '; MAX_HOOK_INPUT + 1]).is_err());
    assert!(
        update_request(
            "g".into(),
            br#"{"sequence":1,"session_id":null,"prompt":"no"}"#
        )
        .is_err()
    );
}

#[cfg(target_os = "linux")]
mod sockets {
    use super::*;
    use crate::{Answer, Channel, What};
    use panel_contract::PanelDir;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::sync_channel;
    use std::time::{Duration, Instant};

    fn send(socket: &mut BufReader<UnixStream>, value: serde_json::Value) -> serde_json::Value {
        writeln!(socket.get_mut(), "{value}").unwrap();
        let mut line = String::new();
        assert!(socket.read_line(&mut line).unwrap() > 0);
        serde_json::from_str(&line).unwrap()
    }

    fn connect(dir: &PanelDir) -> BufReader<UnixStream> {
        let stream = UnixStream::connect(dir.path().join(panel_contract::CONTROL_SOCKET)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        BufReader::new(stream)
    }

    #[test]
    fn real_session_socket_bypasses_full_ui_inbox_and_cannot_spoof_peer() {
        let dir = PanelDir::create().unwrap();
        let (sender, inbox) = sync_channel(0);
        let (observed, peers) = sync_channel(4);
        let wake_count = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&wake_count);
        let channel = Channel::open_with_services(
            dir.path(),
            dir.id(),
            sender,
            None,
            Some(Arc::new(move |peer, request| {
                observed.send((peer, request)).unwrap();
                serde_json::json!({"ok":true,"challenge":null})
            })),
            Arc::new(move || {
                counting.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();
        let mut socket = connect(&dir);
        assert_eq!(
            send(
                &mut socket,
                serde_json::json!({"tool":"session_probe","generation":"g"})
            )["ok"],
            true
        );
        let (peer, request) = peers.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(peer.pid(), std::process::id());
        assert!(peer.birth() > 0 && peer.is_alive());
        assert!(matches!(request, Request::Probe { .. }));
        assert_eq!(
            send(
                &mut socket,
                serde_json::json!({"tool":"session_probe","generation":"g","pid":1})
            )["ok"],
            false
        );
        assert!(
            peers.try_recv().is_err(),
            "forged authority reached service"
        );
        drop(socket);
        let deadline = Instant::now() + Duration::from_secs(2);
        while channel.alive() > 1 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(
            channel.alive(),
            1,
            "helper close tried to enqueue a panel release"
        );
        assert!(inbox.try_recv().is_err());
        assert_eq!(wake_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn session_helper_cannot_release_or_replace_existing_panel_connection() {
        let dir = PanelDir::create().unwrap();
        let (sender, inbox) = sync_channel(4);
        let wake_count = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&wake_count);
        let channel = Channel::open_with_services(
            dir.path(),
            dir.id(),
            sender,
            None,
            Some(Arc::new(|_, _| serde_json::json!({"ok":true}))),
            Arc::new(move || {
                counting.fetch_add(1, Ordering::Relaxed);
            }),
        )
        .unwrap();
        let mut panel = connect(&dir);
        writeln!(
            panel.get_mut(),
            "{}",
            serde_json::json!({"tool":"panel_set","source":"owner","document":{"lines":["held"]}})
        )
        .unwrap();
        let held = inbox.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(held.what, What::Publish { .. }));
        let owner = held.connection;
        held.answer.send(Answer::Taken).unwrap();
        let mut line = String::new();
        panel.read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).unwrap()["ok"],
            true
        );
        let before = wake_count.load(Ordering::Relaxed);
        let mut helper = connect(&dir);
        assert_eq!(
            send(
                &mut helper,
                serde_json::json!({"tool":"session_attach","client":"claude"})
            )["ok"],
            true
        );
        drop(helper);
        let deadline = Instant::now() + Duration::from_secs(2);
        while channel.alive() > 2 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(channel.alive(), 2);
        assert!(inbox.try_recv().is_err());
        assert_eq!(wake_count.load(Ordering::Relaxed), before);
        writeln!(
            panel.get_mut(),
            "{}",
            serde_json::json!({"tool":"panel_retire"})
        )
        .unwrap();
        let retire = inbox.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(retire.connection, owner);
        assert!(matches!(retire.what, What::Retire));
        retire
            .answer
            .send(Answer::Retired {
                closed: 0,
                preserved: 0,
            })
            .unwrap();
    }
}
