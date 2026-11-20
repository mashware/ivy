//! Native Windows checks run in the windows-2025 job.
#![cfg(windows)]

use panel_contract::PanelDir;
use panel_control::{Producer, endpoint};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn windows_client_rejects_wrong_server_before_sending_request_bytes() {
    let panel = PanelDir::create().unwrap();
    let listener = endpoint::bind(panel.path(), panel.id()).unwrap();
    let wrong = std::process::id().wrapping_add(1).max(1);
    let failed = Producer::of_server(panel.path(), panel.id(), wrong)
        .err()
        .unwrap();
    assert_eq!(failed.kind(), std::io::ErrorKind::PermissionDenied);
    match listener.accept() {
        Ok(mut accepted) => {
            accepted
                .set_recv_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = [0; 1];
            assert_eq!(
                accepted.read(&mut bytes).unwrap(),
                0,
                "unverified peer received request bytes"
            );
        }
        Err(problem) => {
            // A verified refusal may close the client before ConnectNamedPipe begins.
            assert_eq!(problem.raw_os_error(), Some(232), "{problem}");
        }
    }
}

#[test]
fn windows_mcp_instructions_require_expected_pid_and_panel_set_round_trips() {
    let panel = PanelDir::create().unwrap();
    let listener = endpoint::bind(panel.path(), panel.id()).unwrap();
    let worker = std::thread::spawn(move || {
        let mut stream = listener.accept().unwrap();
        stream
            .set_recv_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut byte = [0];
        loop {
            assert_eq!(stream.read(&mut byte).unwrap(), 1);
            bytes.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["tool"], "panel_set");
        assert_eq!(value["document"]["lines"][0], "native pipe round trip");
        stream.write_all(b"{\"ok\":true}\n").unwrap();
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_panel-mcp"))
        .env("PANEL_ID", panel.id())
        .env("PANEL_DIR", panel.path())
        .env("PANEL_SERVER_PID", std::process::id().to_string())
        .env_remove(panel_control::codex_route::ENV)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"panel_set\",\"arguments\":{\"source\":\"native-test\",\"document\":{\"lines\":[\"native pipe round trip\"]}}}}\n"
    ).as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    worker.join().unwrap();
    assert!(output.status.success());
    let replies: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(replies[0]["result"]["instructions"].is_string());
    assert_ne!(replies[1]["result"]["isError"], true, "{replies:?}");

    for value in [None, Some("0"), Some("oops")] {
        let mut bridge = Command::new(env!("CARGO_BIN_EXE_panel-mcp"));
        bridge
            .env("PANEL_ID", panel.id())
            .env("PANEL_DIR", panel.path())
            .env_remove(panel_control::codex_route::ENV)
            .env_remove("PANEL_SERVER_PID");
        if let Some(value) = value {
            bridge.env("PANEL_SERVER_PID", value);
        }
        let mut bridge = bridge
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        bridge.stdin.take().unwrap().write_all(concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"panel_get\",\"arguments\":{}}}\n"
        ).as_bytes()).unwrap();
        let output = bridge.wait_with_output().unwrap();
        assert!(output.status.success());
        let replies: Vec<serde_json::Value> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(
            replies[0]["result"].get("instructions").is_none(),
            "{value:?}: {replies:?}"
        );
        assert_eq!(replies[1]["result"]["isError"], true);
        assert!(
            replies[1]["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("PANEL_SERVER_PID")
        );
    }
}
