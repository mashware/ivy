//! The bridge as a whole program: what its handshake says depends on the environment it was
//! started with.
//!
//! **A child process, because that is the only honest way to give a test an environment.**
//! `std::env::set_var` is undefined behaviour in a suite that launches processes in parallel, and
//! the unit tests inside the binary cannot reach the one line of `main` that reads the variables.

use std::io::Write;
use std::process::{Command, Stdio};

/// Starts the bridge with `env` as its pane identity (see [`exchange`]), sends it one «initialize»
/// request and returns the reply.
fn handshake(env: &[(&str, &str)]) -> serde_json::Value {
    exchange(
        env,
        b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n",
    )
}

/// Starts the bridge with the suite's environment minus the pane identity, plus `env`, sends it
/// one `message` and returns the reply. Only the two identity variables are removed: whatever runs
/// the suite may itself be an Ivy console, and the rest of the environment is what a process needs
/// to start at all on some platforms.
fn exchange(env: &[(&str, &str)], message: &[u8]) -> serde_json::Value {
    let mut bridge = Command::new(env!("CARGO_BIN_EXE_panel-mcp"));
    for name in panel_contract::ENV_NAMES {
        bridge.env_remove(name);
    }
    let mut bridge = bridge
        .env_remove(panel_control::codex_route::ENV)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the bridge starts");
    bridge
        .stdin
        .take()
        .expect("its input is piped")
        .write_all(message)
        .expect("the message is written");
    // Dropping the input ends it, so the bridge answers and exits.
    let output = bridge.wait_with_output().expect("the bridge exits");
    // A bridge that answered and then crashed would still have written its reply.
    assert!(output.status.success(), "the bridge failed: {output:?}");
    serde_json::from_slice(&output.stdout).expect("one JSON reply")
}

/// An absolute directory nobody listens at, written the way this platform writes one:
/// `/nonexistent/…` is not absolute on Windows, which wants a drive.
fn nowhere(name: &str) -> String {
    if cfg!(windows) {
        format!(r"C:\nonexistent\{name}")
    } else {
        format!("/nonexistent/{name}")
    }
}

/// A whole identity with a gone directory does not get guidance; tool calls explain the failure.
#[test]
fn the_bridge_gives_conditional_guidance_only_for_a_whole_console_identity() {
    let dir = nowhere("ivy-a1b2c3d4");
    let dir = dir.as_str();
    let outside = handshake(&[("PANEL_ID", "a1b2c3d4"), ("PANEL_DIR", dir)]);
    assert!(
        outside["result"].get("instructions").is_none(),
        "a missing pane directory is not live: {outside}"
    );

    for (env, why) in [
        (&[][..], "no identity"),
        (
            &[("PANEL_ID", ""), ("PANEL_DIR", dir)][..],
            "an empty identity",
        ),
        (&[("PANEL_DIR", dir)][..], "a directory with no identifier"),
        (
            &[("PANEL_ID", "a1b2c3d4")][..],
            "an identifier with no directory",
        ),
        (
            &[("PANEL_ID", "a1b2c3d4"), ("PANEL_DIR", "ivy-a1b2c3d4")][..],
            "a relative directory",
        ),
    ] {
        let reply = handshake(env);
        assert!(
            reply["result"]["capabilities"].is_object(),
            "{why}: still a handshake: {reply}"
        );
        assert!(
            reply["result"].get("instructions").is_none(),
            "{why}: {reply}"
        );
    }
}

#[cfg(unix)]
#[test]
fn conditional_instructions_neither_probe_nor_promise_a_socket() {
    use std::os::unix::net::UnixListener;

    let panel = panel_contract::PanelDir::create().unwrap();
    let socket_path = panel.path().join(panel_contract::CONTROL_SOCKET);
    let listener = UnixListener::bind(&socket_path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let dir = panel.path().to_str().unwrap();
    let bound = handshake(&[("PANEL_ID", panel.id()), ("PANEL_DIR", dir)]);
    assert!(bound["result"].get("instructions").is_some(), "{bound}");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "delivering guidance connected"
    );
    drop(listener);

    std::fs::remove_file(&socket_path).unwrap();
    std::fs::write(&socket_path, b"regular file").unwrap();
    let regular = handshake(&[("PANEL_ID", panel.id()), ("PANEL_DIR", dir)]);
    assert_eq!(
        regular["result"]["instructions"],
        bound["result"]["instructions"]
    );
    assert!(
        panel_get_refusal(&[("PANEL_ID", panel.id()), ("PANEL_DIR", dir)])
            .contains("could not be reached")
    );
}

/// The text of a `panel_get` call's reply, which is a tool error in every case below: outside a
/// pane, a broken identity, or a whole one with nobody listening at the directory — or, where no
/// channel binds, a platform that has none.
fn panel_get_refusal(env: &[(&str, &str)]) -> String {
    let reply = exchange(
        env,
        b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\
          \"params\":{\"name\":\"panel_get\",\"arguments\":{}}}\n",
    );
    assert_eq!(reply["result"]["isError"], true, "{reply}");
    reply["result"]["content"][0]["text"]
        .as_str()
        .expect("the refusal is text")
        .to_owned()
}

/// **The first tool call applies the handshake's rule, and says which of two things went wrong.**
/// Nothing carried by either variable — absent or empty — is a program outside any pane; a broken
/// identity is a client forwarding the variables badly, and is told so. Before `whole_identity`,
/// presence was enough and an empty identifier went through to the endpoint. The control row keeps
/// this honest: with a whole identity the call fails too — nobody listens at the directory, or no
/// channel binds on this platform — with neither identity refusal and saying which, so the
/// assertions cannot be satisfied by any failure. A pair handed in with its halves swapped gets the
/// broken refusal: an identifier is not an absolute directory.
#[test]
fn a_tool_call_tells_a_missing_identity_from_a_broken_one() {
    const OUTSIDE: &str = "not running inside a pane";
    const BROKEN: &str = "not a whole pane identity";
    let dir = nowhere("ivy-x");
    let dir = dir.as_str();

    for (env, why) in [
        (&[][..], "nothing set"),
        (&[("PANEL_ID", ""), ("PANEL_DIR", "")][..], "both set empty"),
    ] {
        let refusal = panel_get_refusal(env);
        assert!(refusal.contains(OUTSIDE), "{why}: {refusal}");
    }
    for (env, why) in [
        (
            &[("PANEL_ID", ""), ("PANEL_DIR", dir)][..],
            "an empty identity",
        ),
        (
            &[("PANEL_ID", "a1b2c3d4"), ("PANEL_DIR", "")][..],
            "an empty directory",
        ),
        (&[("PANEL_ID", "a1b2c3d4")][..], "no directory"),
        (&[("PANEL_DIR", dir)][..], "a directory with no identifier"),
        (
            &[("PANEL_ID", dir), ("PANEL_DIR", "a1b2c3d4")][..],
            "the halves swapped",
        ),
        (
            &[("PANEL_ID", "a1b2c3d4"), ("PANEL_DIR", "ivy-a1b2c3d4")][..],
            "a relative directory",
        ),
    ] {
        let refusal = panel_get_refusal(env);
        assert!(refusal.contains(BROKEN), "{why}: {refusal}");
    }

    let unreachable = nowhere("ivy-a1b2c3d4");
    let whole = panel_get_refusal(&[("PANEL_ID", "a1b2c3d4"), ("PANEL_DIR", &unreachable)]);
    assert!(
        !whole.contains(OUTSIDE) && !whole.contains(BROKEN),
        "a whole identity is inside a pane: {whole}"
    );
    if panel_control::endpoint::BINDS {
        assert!(
            whole.contains("could not be reached") && whole.contains(&unreachable),
            "an unreachable console is named: {whole}"
        );
    } else {
        assert!(
            whole.contains("no control channel"),
            "where nothing binds, the platform is named, not the console: {whole}"
        );
    }
}

/// The child replies before binding is permitted, so this exercises ordering without sleeps.
#[cfg(unix)]
#[test]
fn an_early_bridge_keeps_publication_guidance_when_the_channel_binds_later() {
    use std::io::{BufRead, BufReader};
    use std::sync::{Arc, mpsc};
    use std::time::Duration;

    struct Bridge {
        child: std::process::Child,
        input: Option<std::process::ChildStdin>,
        replies: mpsc::Receiver<serde_json::Value>,
        reader: Option<std::thread::JoinHandle<()>>,
    }
    impl Bridge {
        fn exchange(&mut self, message: serde_json::Value) -> serde_json::Value {
            let input = self.input.as_mut().unwrap();
            writeln!(input, "{message}").unwrap();
            input.flush().unwrap();
            self.replies
                .recv_timeout(Duration::from_secs(5))
                .expect("bounded bridge reply")
        }
    }
    impl Drop for Bridge {
        fn drop(&mut self) {
            self.input.take();
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Some(reader) = self.reader.take() {
                reader.join().unwrap();
            }
        }
    }
    let panel = panel_contract::PanelDir::create().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_panel-mcp"))
        .env_remove(panel_control::codex_route::ENV)
        .env("PANEL_ID", panel.id())
        .env("PANEL_DIR", panel.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, replies) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let value = serde_json::from_str(&line).expect("bridge JSON");
            if send.send(value).is_err() {
                break;
            }
        }
    });
    let mut bridge = Bridge {
        child,
        input: Some(input),
        replies,
        reader: Some(reader),
    };
    let early = bridge.exchange(serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"}));
    assert!(!panel.path().join(panel_contract::CONTROL_SOCKET).exists());
    let (sender, _inbox) = mpsc::sync_channel(8);
    let channel = panel_control::Channel::open_with_reader(
        panel.path(),
        panel.id(),
        sender,
        Arc::new(|| Ok(r#"{"lines":["late channel ready"]}"#.into())),
        Arc::new(|| {}),
    )
    .unwrap();
    let later = bridge.exchange(serde_json::json!({"jsonrpc":"2.0","id":2,"method":"initialize"}));
    let read = bridge.exchange(serde_json::json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"panel_get","arguments":{}}}));
    assert_ne!(read["result"]["isError"], true, "{read}");
    assert!(read.to_string().contains("late channel ready"), "{read}");
    for reply in [early, later] {
        let instructions = reply["result"]["instructions"]
            .as_str()
            .expect("startup must not lose publication guidance");
        assert!(instructions.contains("panel_set"));
        assert!(instructions.contains("panel_get"));
        assert!(
            !instructions.contains("This session runs in an Ivy console"),
            "do not promise that the channel is live"
        );
    }
    drop(bridge);
    drop(channel);
}
