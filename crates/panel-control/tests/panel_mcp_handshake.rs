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

/// **Inside a console the agent is asked to publish; with no identity, or a broken one, it is
/// not.** The directory is never opened by a handshake, so any absolute path stands for one here.
#[test]
fn the_bridge_asks_for_a_panel_only_when_started_inside_a_console() {
    let dir = nowhere("ivy-a1b2c3d4");
    let dir = dir.as_str();
    let inside = handshake(&[("PANEL_ID", "a1b2c3d4"), ("PANEL_DIR", dir)]);
    let instructions = inside["result"].get("instructions");
    if panel_control::endpoint::BINDS {
        let text = instructions
            .and_then(serde_json::Value::as_str)
            .expect("inside a console the handshake carries instructions");
        assert!(text.contains("`panel_set`"), "{text}");
    } else {
        assert!(
            instructions.is_none(),
            "no channel binds here yet: {inside}"
        );
    }

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
