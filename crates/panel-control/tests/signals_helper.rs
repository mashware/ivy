//! Shipped helper, actual authenticated native endpoint, and bounded process lifetime.
#[cfg(unix)]
use interprocess::local_socket::traits::{Listener as _, Stream as _};
#[cfg(unix)]
use interprocess::local_socket::{Listener, Stream};
use panel_contract::PanelDir;
use panel_control::endpoint;
#[cfg(windows)]
use panel_control::endpoint::{Listener, Stream};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const NULL: &[u8] =
    b"{\"tool\":\"agent_signal\",\"v\":1,\"client\":\"opencode\",\"snapshot\":null}\n";

fn spawn(panel: &PanelDir) -> Child {
    Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .arg("signals")
        .env("PANEL_ID", panel.id())
        .env("PANEL_DIR", panel.path())
        .env("PANEL_SERVER_PID", std::process::id().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}
fn wait(child: &mut Child, budget: Duration) {
    let until = Instant::now() + budget;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= until {
            let _ = child.kill();
            panic!("signals helper exceeded process deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn line(stream: &mut Stream) -> Vec<u8> {
    let mut result = Vec::new();
    loop {
        let mut byte = [0];
        assert_eq!(stream.read(&mut byte).unwrap(), 1);
        if byte[0] == b'\n' {
            return result;
        }
        result.push(byte[0]);
        assert!(result.len() <= 65536);
    }
}
fn accept(listener: Listener) -> Stream {
    #[cfg_attr(unix, allow(unused_mut))]
    let mut stream = listener.accept().unwrap();
    stream
        .set_recv_timeout(Some(Duration::from_secs(8)))
        .unwrap();
    stream
        .set_send_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    #[cfg(windows)]
    {
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&line(&mut stream)).unwrap(),
            serde_json::json!({"tool":"peer_hello","v":1})
        );
        stream
            .write_all(b"{\"ok\":true,\"peer\":{\"v\":1,\"mode\":\"unverified\"}}\n")
            .unwrap();
    }
    stream
}

#[test]
fn native_helper_preserves_refusals_and_explicit_withdrawal_on_one_connection() {
    let panel = PanelDir::create().unwrap();
    let bound = endpoint::bind(panel.path(), panel.id()).unwrap();
    let server = std::thread::spawn(move || {
        let mut stream = accept(bound);
        for reply in [
            b"{\"ok\":false,\"error\":\"try again\",\"retryable\":true}\n".as_slice(),
            b"{\"ok\":false,\"error\":\"disabled\"}\n",
            b"{\"ok\":true}\n",
        ] {
            let request = line(&mut stream);
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&request).unwrap(),
                serde_json::from_slice::<serde_json::Value>(NULL).unwrap()
            );
            stream.write_all(reply).unwrap();
        }
        assert_eq!(
            stream.read(&mut [0]).unwrap(),
            0,
            "stdin EOF must close the endpoint"
        );
    });
    let mut child = spawn(&panel);
    let mut input = child.stdin.take().unwrap();
    for _ in 0..3 {
        input.write_all(NULL).unwrap();
    }
    drop(input);
    wait(&mut child, Duration::from_secs(5));
    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        output.stdout,
        concat!(
            "{\"ok\":true,\"ready\":true,\"v\":1}\n",
            "{\"ok\":false,\"error\":\"try again\",\"retryable\":true}\n",
            "{\"ok\":false,\"error\":\"disabled\"}\n",
            "{\"ok\":true}\n",
        )
        .as_bytes()
    );
    let replies: Vec<serde_json::Value> = output
        .stdout
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|l| serde_json::from_slice(l).unwrap())
        .collect();
    assert_eq!(
        replies,
        vec![
            serde_json::json!({"ok":true,"ready":true,"v":1}),
            serde_json::json!({"ok":false,"error":"try again","retryable":true}),
            serde_json::json!({"ok":false,"error":"disabled"}),
            serde_json::json!({"ok":true})
        ]
    );
}

#[test]
fn native_helper_rejects_invalid_input_without_forwarding_any_signal() {
    for input in [
        b"{\"tool\":\"panel_retire\"}\n".to_vec(),
        NULL[..NULL.len() - 1].to_vec(),
        vec![b'x'; 65537],
        b"\xff\n".to_vec(),
    ] {
        let panel = PanelDir::create().unwrap();
        let bound = endpoint::bind(panel.path(), panel.id()).unwrap();
        let server = std::thread::spawn(move || {
            let mut stream = accept(bound);
            assert_eq!(
                stream.read(&mut [0]).unwrap(),
                0,
                "invalid input reached native endpoint"
            );
        });
        let mut child = spawn(&panel);
        let _ = child.stdin.take().unwrap().write_all(&input);
        wait(&mut child, Duration::from_secs(5));
        let output = child.wait_with_output().unwrap();
        server.join().unwrap();
        assert!(!output.status.success());
        assert_eq!(output.stdout, b"{\"ok\":true,\"ready\":true,\"v\":1}\n");
    }
}

#[test]
fn parent_eof_during_a_stalled_reply_exits_within_the_native_exchange_bound() {
    let panel = PanelDir::create().unwrap();
    let bound = endpoint::bind(panel.path(), panel.id()).unwrap();
    let server = std::thread::spawn(move || {
        let mut stream = accept(bound);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&line(&mut stream)).unwrap(),
            serde_json::from_slice::<serde_json::Value>(NULL).unwrap()
        );
        assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    });
    let mut child = spawn(&panel);
    child.stdin.take().unwrap().write_all(NULL).unwrap();
    let start = Instant::now();
    wait(&mut child, Duration::from_secs(7));
    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(!output.status.success());
    assert!(start.elapsed() < Duration::from_secs(7));
    assert_eq!(output.stdout, b"{\"ok\":true,\"ready\":true,\"v\":1}\n");
}

#[cfg(windows)]
#[test]
fn native_helper_wrong_pid_sends_zero_bytes_before_failed_readiness() {
    let panel = PanelDir::create().unwrap();
    let bound = endpoint::bind(panel.path(), panel.id()).unwrap();
    let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);
    let (observed_sender, observed_receiver) = std::sync::mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        ready_sender.send(()).unwrap();
        let observed = match bound.accept() {
            Ok(mut stream) => {
                stream
                    .set_recv_timeout(Some(Duration::from_secs(4)))
                    .unwrap();
                stream.read(&mut [0])
            }
            // The verified client may close before ConnectNamedPipe completes.
            Err(error) if error.raw_os_error() == Some(232) => Ok(0),
            Err(error) => Err(error),
        };
        observed_sender.send(observed).unwrap();
    });
    ready_receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
        .arg("signals")
        .env("PANEL_ID", panel.id())
        .env("PANEL_DIR", panel.path())
        .env(
            "PANEL_SERVER_PID",
            std::process::id().wrapping_add(1).max(1).to_string(),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let held_input = child.stdin.take().unwrap();
    // Observe while the child is live: accepting only after exit loses buffered peer_hello bytes.
    let observed = observed_receiver.recv_timeout(Duration::from_secs(5));
    wait(&mut child, Duration::from_secs(3));
    drop(held_input);
    let output = child.wait_with_output().unwrap();
    let received = observed
        .expect("native accept/read did not finish within its deadline")
        .unwrap();
    server.join().unwrap();
    assert_eq!(
        received, 0,
        "wrong-PID helper sent bytes before authenticating its server"
    );
    assert!(!output.status.success());
    let ready: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(ready["ok"], false);
    assert_eq!(ready["ready"], true);
}

#[test]
fn startup_refuses_incomplete_identity_and_a_non_directory_without_reading_stdin() {
    let panel = PanelDir::create().unwrap();
    let file = panel.path().join("not-a-directory");
    std::fs::write(&file, b"inert").unwrap();
    for dir in [std::path::Path::new("relative"), file.as_path()] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
            .arg("signals")
            .env("PANEL_ID", panel.id())
            .env("PANEL_DIR", dir)
            .env("PANEL_SERVER_PID", std::process::id().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let held_input = child.stdin.take().unwrap();
        wait(&mut child, Duration::from_secs(2));
        drop(held_input);
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        let ready: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(ready["ok"], false);
        assert_eq!(ready["ready"], true);
        assert_eq!(ready["retryable"], false);
    }
}

#[test]
fn signals_accepts_no_endpoint_or_program_arguments() {
    let panel = PanelDir::create().unwrap();
    for extra in ["--endpoint", "opencode", "sh"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ivy-session"))
            .args(["signals", extra])
            .env("PANEL_ID", panel.id())
            .env("PANEL_DIR", panel.path())
            .env("PANEL_SERVER_PID", std::process::id().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let held_input = child.stdin.take().unwrap();
        wait(&mut child, Duration::from_secs(2));
        drop(held_input);
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("\"ready\":true"));
    }
}

#[test]
fn native_trickled_reply_does_not_renew_the_absolute_exchange_budget() {
    let panel = PanelDir::create().unwrap();
    let bound = endpoint::bind(panel.path(), panel.id()).unwrap();
    let server = std::thread::spawn(move || {
        let mut stream = accept(bound);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&line(&mut stream)).unwrap(),
            serde_json::from_slice::<serde_json::Value>(NULL).unwrap()
        );
        for byte in b"{\"ok\":true}\n" {
            std::thread::sleep(Duration::from_millis(550));
            if stream.write_all(&[*byte]).is_err() {
                return;
            }
        }
    });
    let mut child = spawn(&panel);
    child.stdin.take().unwrap().write_all(NULL).unwrap();
    let start = Instant::now();
    wait(&mut child, Duration::from_secs(7));
    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    assert!(
        !output.status.success(),
        "a renewed per-byte budget accepted the late reply"
    );
    assert!(start.elapsed() < Duration::from_millis(7500));
    assert_eq!(output.stdout, b"{\"ok\":true,\"ready\":true,\"v\":1}\n");
}
