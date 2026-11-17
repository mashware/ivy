//! Inherited clients refuse a planted directory before opening its socket.
#![cfg(unix)]

use panel_contract::PanelDir;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::{Command, Stdio};

fn refused(dir: &Path, privacy: bool) {
    let path = dir.to_str().unwrap();
    let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"panel_get\",\"arguments\":{}}}\n";
    let mut child = Command::new(env!("CARGO_BIN_EXE_panel-mcp"))
        .env("PANEL_DIR", dir)
        .env("PANEL_ID", "test")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let replies: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies[0]["result"]["instructions"].is_string(), privacy);
    assert_eq!(replies[1]["result"]["isError"], true);
    assert_eq!(replies[1]["result"]["content"][0]["type"], "text");
    let bridge = replies[1]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(bridge.contains(path), "{bridge}");
    for args in [vec!["attach", "claude"], vec!["publish"]] {
        let mut helper = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
            .args(args)
            .env("PANEL_DIR", dir)
            .env("PANEL_ID", "test")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // The helper refuses the directory before it reads its input, so it may already be gone
        // when this write lands: a closed pipe is that refusal arriving first, not a failure. What
        // the test proves is the exit code and the message below.
        if let Err(error) = helper.stdin.take().unwrap().write_all(b"{}") {
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe, "{error}");
        }
        let output = helper.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with("ivy-session: "), "{error}");
        assert!(error.contains(path), "{error}");
        let expected = if privacy {
            "not a private directory of this user"
        } else {
            "directory is gone"
        };
        assert!(error.contains(expected), "{error}");
        assert!(bridge.contains(expected), "{bridge}");
        if !privacy {
            assert!(!error.contains("not a private directory"));
        }
        if !output.stdout.is_empty() {
            let reply: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(reply["error"], "helper_error");
            assert_eq!(
                reply["message"].as_str().unwrap(),
                error.trim_end().strip_prefix("ivy-session: ").unwrap()
            );
        }
    }
}

#[test]
fn unsafe_inherited_directories_never_reach_a_listening_socket() {
    let panel = PanelDir::create().unwrap();
    let listener = UnixListener::bind(panel.path().join(panel_contract::CONTROL_SOCKET)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let no_connection = || {
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        )
    };
    for bit in [0o040, 0o020, 0o010, 0o004, 0o002, 0o001] {
        std::fs::set_permissions(panel.path(), std::fs::Permissions::from_mode(0o700 | bit))
            .unwrap();
        refused(panel.path(), true);
        no_connection();
    }
    std::fs::set_permissions(panel.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let holder = PanelDir::create().unwrap();
    let alias = holder.path().join("alias");
    symlink(panel.path(), &alias).unwrap();
    for suffix in ["", "/", "/.", "/././"] {
        refused(Path::new(&format!("{}{suffix}", alias.display())), true);
        no_connection();
    }
    refused(&holder.path().join("gone"), false);
}

#[test]
fn genuine_inherited_directory_returns_the_document() {
    use std::io::{BufRead, BufReader};
    use std::time::{Duration, Instant};
    let panel = PanelDir::create().unwrap();
    let listener = UnixListener::bind(panel.path().join(panel_contract::CONTROL_SOCKET)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "genuine bridge never connected");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("{error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).unwrap()["tool"],
            "panel_get"
        );
        stream
            .write_all(b"{\"ok\":true,\"document\":{\"title\":\"genuine console\"}}\n")
            .unwrap();
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_panel-mcp"))
        .env("PANEL_DIR", panel.path())
        .env("PANEL_ID", panel.id())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"panel_get\",\"arguments\":{}}}\n").unwrap();
    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(output.status.success());
    let reply: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_ne!(reply["result"]["isError"], true, "{reply}");
    let document: serde_json::Value =
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(document["title"], "genuine console");
}
