//! Runs the shipped helper so inherited routing, content filtering and kernel lifetime meet.

#![cfg(target_os = "linux")]

use panel_contract::PanelDir;
use panel_control::{Channel, Reply, session};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::sync_channel;
use std::time::Duration;

#[test]
fn helper_reports_only_allowed_metadata_with_its_actual_kernel_identity() {
    let dir = PanelDir::create().unwrap();
    let (sender, inbox) = sync_channel(0);
    let (observed, peers) = sync_channel(2);
    let _channel = Channel::open_with_services(
        dir.path(),
        dir.id(),
        sender,
        None,
        Some(Arc::new(move |peer, request| {
            assert!(peer.is_alive());
            observed.send((peer, request)).unwrap();
            serde_json::json!({"ok":true})
        })),
        Arc::new(|| panic!("helper must not wake the window")),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .args(["hook", "codex"])
        .env("PANEL_DIR", dir.path())
        .env("PANEL_ID", dir.id())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let actual_pid = child.id();
    let hook = serde_json::json!({
        "session_id":"11111111-1111-4111-8111-111111111111", "cwd":"/tmp",
        "hook_event_name":"SessionStart", "source":"resume", "pid":1,
        "prompt":"PRIVATE PROMPT", "transcript_path":"PRIVATE HISTORY", "argv":["sh","-c","forbidden"]
    });
    child
        .stdin
        .take()
        .unwrap()
        .write_all(hook.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["ok"],
        true
    );
    let (peer, request) = peers.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        peer.pid(),
        actual_pid,
        "payload PID replaced kernel identity"
    );
    assert!(
        !peer.is_alive(),
        "socket peer lifetime survived actual helper exit"
    );
    let forwarded = serde_json::to_string(&request).unwrap();
    for forbidden in ["PRIVATE", "prompt", "transcript_path", "argv", "pid"] {
        assert!(!forwarded.contains(forbidden));
    }
    assert!(inbox.try_recv().is_err());
    assert!(output.stdout.len() <= session::MAX_REPLY + 1);
}

#[test]
fn absent_inherited_identity_is_a_distinct_bounded_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .args(["attach", "claude"])
        .env_remove("PANEL_DIR")
        .env_remove("PANEL_ID")
        .env("PANEL_ENDPOINT", "/tmp/not-authority")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["error"],
        "outside_ivy"
    );
    assert!(output.stdout.len() <= session::MAX_REPLY + 1);
}

#[cfg(unix)]
#[test]
fn invalid_whole_identity_exits_before_reading_stdin_or_connecting() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::thread;
    use std::time::{Duration, Instant};

    let bad_id = OsString::from_vec(vec![b'p', b'a', b'n', b'e', 0xff]);
    for (id, dir) in [
        (OsString::from("pane"), OsString::from("relative")),
        (bad_id, std::env::temp_dir().into_os_string()),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
            .args(["hook", "codex"])
            .env("PANEL_ID", id)
            .env("PANEL_DIR", dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let _held_open_stdin = child.stdin.take().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert_eq!(status.code(), Some(1));
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                panic!("ivy-session read stdin before rejecting its identity");
            }
            thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.stdout, b"{\"ok\":false,\"error\":\"outside_ivy\"}\n");
        assert!(output.stderr.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn session_exchange_rejects_invalid_identity_before_connecting() {
    const CHILD: &str = "IVY_TEST_DIRECT_EXCHANGE_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let request = session::Request::Attach {
            client: session::Client::Claude,
        };
        assert_eq!(
            session::exchange(&request).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        return;
    }

    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    for (id, dir) in [
        (OsString::from("pane"), OsString::from("relative")),
        (
            OsString::from_vec(vec![b'p', b'a', b'n', b'e', 0xff]),
            std::env::temp_dir().into_os_string(),
        ),
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session_exchange_rejects_invalid_identity_before_connecting",
            ])
            .env(CHILD, "1")
            .env("PANEL_ID", id)
            .env("PANEL_DIR", dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn publish_cli_rejects_invalid_identity_and_bad_input_before_connect() {
    let output = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .arg("publish")
        .env_remove("PANEL_DIR")
        .env_remove("PANEL_ID")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = output;
    child.stdin.take().unwrap().write_all(b"not json").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"{\"ok\":false,\"error\":\"outside_ivy\"}\n");
    assert!(output.stderr.is_empty());

    let dir = PanelDir::create().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .arg("publish")
        .env("PANEL_DIR", dir.path())
        .env("PANEL_ID", dir.id())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut child = output;
    child.stdin.take().unwrap().write_all(b"[]").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be a JSON object"));
}

#[test]
fn publish_cli_maps_unheld_and_policy_refusal_to_the_declared_output() {
    let dir = PanelDir::create().unwrap();
    let (sender, inbox) = sync_channel(1);
    let (document_sender, document_receiver) = sync_channel(1);
    let callback = Arc::new(move |_peer, _connection, document| {
        document_sender.send(document).unwrap();
        Reply::PublishUnheld
    });
    let _channel = Channel::open_with_authorization_services(
        dir.path(),
        dir.id(),
        sender,
        None,
        None,
        Some(panel_control::listener::PublicationServices::new(callback)),
        Arc::new(|| {}),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .arg("publish")
        .env("PANEL_DIR", dir.path())
        .env("PANEL_ID", dir.id())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\n  \"lines\": [\"x\"]\n}\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "{\"ok\":false,\"code\":\"unheld\",\"message\":\"this panel has no holder\"}\n"
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        document_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap(),
        "{\"lines\":[\"x\"]}"
    );
    assert!(inbox.try_recv().is_err());

    let dir = PanelDir::create().unwrap();
    let (sender, _inbox) = sync_channel(1);
    let callback = Arc::new(|_peer, _connection, _document| Reply::PublishRefused {
        code: "unauthorized".into(),
        message: "publisher does not descend from this panel's holding client".into(),
    });
    let _channel = Channel::open_with_authorization_services(
        dir.path(),
        dir.id(),
        sender,
        None,
        None,
        Some(panel_control::listener::PublicationServices::new(callback)),
        Arc::new(|| {}),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .arg("publish")
        .env("PANEL_DIR", dir.path())
        .env("PANEL_ID", dir.id())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"lines\":[]}")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "ivy-session: publisher does not descend from this panel's holding client\n"
    );
}

#[test]
fn publish_cli_rejects_oversized_stdin_before_endpoint_connect() {
    let dir = PanelDir::create().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .arg("publish")
        .env("PANEL_DIR", dir.path())
        .env("PANEL_ID", dir.id())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // No endpoint is bound: an attempted connection would exit 1 instead of the input error.
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&vec![b' '; panel_contract::MAX_BYTES as usize + 1])
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "ivy-session: session metadata goes over the ceiling\n"
    );
}
