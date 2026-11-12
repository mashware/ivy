//! The bridge as a whole program: what its handshake says depends on the environment it was
//! started with.
//!
//! **A child process, because that is the only honest way to give a test an environment.**
//! `std::env::set_var` is undefined behaviour in a suite that launches processes in parallel, and
//! the unit tests inside the binary cannot reach the one line of `main` that reads the variables.

use std::io::Write;
use std::process::{Command, Stdio};

/// Starts the bridge with exactly `env`, sends it one `initialize` and returns the reply.
fn handshake(env: &[(&str, &str)]) -> serde_json::Value {
    let mut bridge = Command::new(env!("CARGO_BIN_EXE_panel-mcp"))
        .env_clear()
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the bridge starts");
    bridge
        .stdin
        .take()
        .expect("its input is piped")
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n")
        .expect("the message is written");
    // Dropping the input ends it, so the bridge answers and exits.
    let output = bridge.wait_with_output().expect("the bridge exits");
    serde_json::from_slice(&output.stdout).expect("one JSON reply")
}

/// **Inside a console the agent is asked to publish; with no identity, or half of one, it is
/// not.** The directory is never opened by a handshake, so any path stands for one here.
#[test]
fn the_bridge_asks_for_a_panel_only_when_started_inside_a_console() {
    let inside = handshake(&[
        ("PANEL_ID", "a1b2c3d4"),
        ("PANEL_DIR", "/nonexistent/ivy-a1b2c3d4"),
    ]);
    let instructions = inside["result"].get("instructions");
    if cfg!(windows) {
        assert!(
            instructions.is_none(),
            "Windows has no channel yet: {inside}"
        );
    } else {
        let text = instructions
            .and_then(serde_json::Value::as_str)
            .expect("inside a console the handshake carries instructions");
        assert!(text.contains("`panel_set`"), "{text}");
    }

    for (env, why) in [
        (&[][..], "no identity"),
        (
            &[("PANEL_ID", ""), ("PANEL_DIR", "/nonexistent/ivy-x")][..],
            "an empty identity",
        ),
        (
            &[("PANEL_DIR", "/nonexistent/ivy-x")][..],
            "half an identity",
        ),
    ] {
        let reply = handshake(env);
        assert!(
            reply["result"].get("instructions").is_none(),
            "{why}: {reply}"
        );
    }
}
