//! The bridge as a whole program: what its handshake says depends on the environment it was
//! started with.
//!
//! **A child process, because that is the only honest way to give a test an environment.**
//! `std::env::set_var` is undefined behaviour in a suite that launches processes in parallel, and
//! the unit tests inside the binary cannot reach the one line of `main` that reads the variables.

use std::io::Write;
use std::process::{Command, Stdio};

/// Starts the bridge with exactly `env`, sends it one «initialize» request and returns the reply.
fn handshake(env: &[(&str, &str)]) -> serde_json::Value {
    exchange(env, b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n")
}

/// Starts the bridge with exactly `env`, sends it one `message` and returns the reply.
fn exchange(env: &[(&str, &str)], message: &[u8]) -> serde_json::Value {
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
        .write_all(message)
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

/// The text of a `panel_get` call's reply, which is a tool error in every case below: no pane is
/// listening at `/nonexistent`.
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

/// **The first tool call applies the handshake's rule: an empty identity is no console.** Before
/// `whole_identity`, presence was enough and the call tried a pane named `""`. The control row is
/// what keeps this honest: with a whole identity the call fails too — nobody listens at the
/// directory — but for another reason, so the assertion cannot be satisfied by any failure.
#[test]
fn a_tool_call_with_a_broken_identity_says_it_is_not_inside_a_pane() {
    const OUTSIDE: &str = "not running inside a pane";

    for (env, why) in [
        (
            &[("PANEL_ID", ""), ("PANEL_DIR", "/nonexistent/ivy-x")][..],
            "an empty identity",
        ),
        (
            &[("PANEL_ID", "a1b2c3d4"), ("PANEL_DIR", "")][..],
            "an empty directory",
        ),
        (&[("PANEL_ID", "a1b2c3d4")][..], "no directory"),
        (&[][..], "no identity"),
    ] {
        let refusal = panel_get_refusal(env);
        assert!(refusal.contains(OUTSIDE), "{why}: {refusal}");
    }

    let whole = panel_get_refusal(&[
        ("PANEL_ID", "a1b2c3d4"),
        ("PANEL_DIR", "/nonexistent/ivy-a1b2c3d4"),
    ]);
    assert!(
        !whole.contains(OUTSIDE),
        "a whole identity is inside a pane: {whole}"
    );
}
