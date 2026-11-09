//! Runs the shipped helper so inherited routing, content filtering and kernel lifetime meet.

#![cfg(target_os = "linux")]

use panel_contract::PanelDir;
use panel_control::{Channel, session};
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
