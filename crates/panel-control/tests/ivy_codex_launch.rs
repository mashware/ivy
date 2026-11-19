#![cfg(unix)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const REMOTE_HELP: &str = "Options:\n  --remote <ADDR>\n  --remote-auth-token-env <ENV>\n";

struct Client {
    dir: PathBuf,
    executable: PathBuf,
}
impl Client {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ivy-codex-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        let dir = fs::canonicalize(dir).unwrap();
        let executable = dir.join("client");
        fs::write(&executable, r#"#!/bin/sh
if [ "$1" = --version ]; then
    echo version >> "$CALLS"
    exit 91
fi
if [ "$1" = --help ]; then
    echo help >> "$CALLS"
    if [ "$STALL_HELP" = yes ]; then exec sleep 10; fi
    printf '%s' "$HELP_TEXT"
    exit 0
fi
if [ "$1 $2 $3" = 'app-server daemon start' ]; then
    echo daemon >> "$CALLS"
    printf '%s\0' "${PANEL_ID-unset}" "${PANEL_DIR-unset}" > "$DAEMON_ENV"
    printf '%s' "$DAEMON_JSON"
    exit 0
fi
printf '%s\0' "$PANEL_ID" "$PANEL_DIR" "$CONFIG_SENTINEL" "$PWD" "$CODEX_HOME" "$MCP_AUTH_SENTINEL" "$SUBSCRIPTION_SENTINEL" "${OPENAI_API_KEY-unset}" "${ANTHROPIC_API_KEY-unset}" "${IVY_CODEX_RELAY_AUTH-unset}" "$@" > "$RECORD"
if [ -f "$CODEX_HOME/config.toml" ]; then
    cat "$CODEX_HOME/config.toml" "$CODEX_HOME/auth.json" > "$READ_CONFIG"
fi
exit 37
"#).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir, executable }
    }
    fn command(&self, args: &[&str], help: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ivy-codex"));
        command
            .arg(&self.executable)
            .args(args)
            .env("PANEL_ID", "pane one")
            .env("PANEL_DIR", self.dir.join("pane directory"))
            .env("CONFIG_SENTINEL", "inherited value")
            .env("CODEX_HOME", &self.dir)
            .env("MCP_AUTH_SENTINEL", "existing MCP authentication")
            .env("SUBSCRIPTION_SENTINEL", "existing subscription")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("IVY_CODEX_RELAY_AUTH")
            .env_remove("STALL_HELP")
            .env("RECORD", self.dir.join("record"))
            .env("CALLS", self.dir.join("calls"))
            .env("DAEMON_ENV", self.dir.join("daemon-env"))
            .env("READ_CONFIG", self.dir.join("read-config"))
            .env("HELP_TEXT", help)
            .env("DAEMON_JSON", r#"{"socketPath":"/unused-test-socket"}"#)
            .current_dir(&self.dir);
        command
    }
    fn fields(&self, name: &str) -> Vec<String> {
        let data = fs::read(self.dir.join(name)).unwrap();
        let mut fields: Vec<_> = data
            .split(|b| *b == 0)
            .map(|v| String::from_utf8(v.to_vec()).unwrap())
            .collect();
        assert_eq!(fields.pop().as_deref(), Some(""));
        fields
    }
    fn calls(&self) -> String {
        fs::read_to_string(self.dir.join("calls")).unwrap_or_default()
    }
    fn assert_environment(&self, record: &[String]) {
        assert_eq!(record[0], "pane one");
        assert_eq!(record[1], self.dir.join("pane directory").to_str().unwrap());
        assert_eq!(record[2], "inherited value");
        assert_eq!(record[3], self.dir.to_str().unwrap());
        assert_eq!(record[4], self.dir.to_str().unwrap());
        assert_eq!(
            &record[5..9],
            &[
                "existing MCP authentication",
                "existing subscription",
                "unset",
                "unset"
            ]
        );
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
fn diagnostic(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn explicit_independent_admin_remote_and_unknown_preserve_arguments_environment_and_status() {
    for args in [
        &["--no-daemon", "resume", "session one"][..],
        &["exec", "hello"][..],
        &["login"][..],
        &["mcp", "list"][..],
        &["app-server", "daemon", "status"][..],
        &["resume", "id", "--remote", "ws://host"][..],
        &["--future-flag"][..],
        &["--model", "--remote", "ws://host"][..],
        &["do work", "--remote", "ws://host"][..],
        &["-c", "model=exec", "prompt"][..],
        &["--config=model=exec", "prompt"][..],
    ] {
        let client = Client::new();
        let output = client.command(args, REMOTE_HELP).output().unwrap();
        let record = client.fields("record");
        client.assert_environment(&record);
        assert_eq!(&record[10..], args);
        assert_eq!(output.status.code(), Some(37));
        assert!(
            client.calls().is_empty(),
            "passthrough must not probe or start a daemon"
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn shared_launch_uses_capabilities_with_arbitrary_cli_and_server_version_labels() {
    // Representative labels establish version independence, not a compatibility whitelist.
    for (cli_version, server_version) in [
        ("0.170.0", Some("0.170.0")),
        ("9999.42.0-future", Some("future-managed-server")),
        ("unversioned-build", None),
    ] {
        for args in [
            &[][..],
            &["resume", "session one"][..],
            &["-m", "exec", "prompt"][..],
            &["--", "--remote"][..],
            &["fork", "id", "", "line one\nline two"][..],
            &["-mexec", "prompt", "login"][..],
            &["--model=--remote", "prompt"][..],
        ] {
            let client = Client::new();
            let help = format!("Codex {cli_version}\n{REMOTE_HELP}");
            let mut daemon = serde_json::json!({"socketPath":"/unused-test-socket"});
            if let Some(version) = server_version {
                daemon["appServerVersion"] = version.into();
            }
            let output = client
                .command(args, &help)
                .env("DAEMON_JSON", daemon.to_string())
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(37), "{}", diagnostic(&output));
            let record = client.fields("record");
            client.assert_environment(&record);
            assert_eq!(record[10], "--remote");
            assert!(record[11].starts_with("ws://127.0.0.1:"));
            assert_eq!(record[12], "--remote-auth-token-env");
            assert_eq!(record[13], "IVY_CODEX_RELAY_AUTH");
            assert_ne!(record[9], "unset");
            assert!(!record[9].is_empty());
            assert_eq!(&record[14..], args);
            assert!(!record[10..].iter().any(|arg| arg == "--no-daemon"));
            assert_eq!(client.calls(), "help\ndaemon\n");
            assert_eq!(client.fields("daemon-env"), ["unset", "unset"]);
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn missing_remote_capabilities_never_start_an_independent_session() {
    for help in [
        "old client --no-daemon",
        "--remote <ADDR>",
        "--remote-auth-token-env <ENV>",
        "--remote-future --remote-auth-token-env",
    ] {
        let client = Client::new();
        let output = client.command(&["fork", "id"], help).output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(diagnostic(&output).contains("does not advertise"));
        assert!(!client.dir.join("record").exists());
        assert_eq!(client.calls(), "help\n");
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn failed_shared_setup_never_starts_an_independent_session() {
    for response in ["not-json", "{}", r#"{"socketPath":null}"#] {
        let client = Client::new();
        let output = client
            .command(&[], REMOTE_HELP)
            .env("DAEMON_JSON", response)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(diagnostic(&output).contains("shared panel connection could not start"));
        assert!(!client.dir.join("record").exists());
        assert_eq!(client.calls(), "help\ndaemon\n");
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn stalled_capability_probe_fails_in_bounded_time_without_starting_a_session() {
    let client = Client::new();
    let start = std::time::Instant::now();
    let output = client
        .command(&["resume"], REMOTE_HELP)
        .env("STALL_HELP", "yes")
        .output()
        .unwrap();
    assert!(start.elapsed() < std::time::Duration::from_secs(4));
    assert_eq!(output.status.code(), Some(1));
    assert!(diagnostic(&output).contains("timed out"));
    assert!(!client.dir.join("record").exists());
    assert_eq!(client.calls(), "help\n");
}

#[test]
fn refuses_recursive_executable() {
    let launcher = env!("CARGO_BIN_EXE_ivy-codex");
    let client = Client::new();
    let output = Command::new(launcher)
        .arg(launcher)
        .env("CODEX_HOME", &client.dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(diagnostic(&output).contains("itself"));
}

#[test]
fn preserves_client_signal() {
    use std::os::unix::process::ExitStatusExt;
    let client = Client::new();
    fs::write(&client.executable, "#!/bin/sh\nkill -TERM $$\n").unwrap();
    let status = client.command(&["exec"], REMOTE_HELP).status().unwrap();
    assert_eq!(status.signal(), Some(15));
}

#[test]
fn inherits_auth_and_mcp_environment_without_changing_configuration_or_adding_api_keys() {
    let client = Client::new();
    let config = client.dir.join("config.toml");
    let auth = client.dir.join("auth.json");
    let config_bytes =
        b"# existing user settings\n[mcp_servers.ivy]\nenv_vars = ['PANEL_ID', 'PANEL_DIR']\n";
    let auth_bytes = b"{\"session\":\"inherited subscription sentinel\"}\n";
    fs::write(&config, config_bytes).unwrap();
    fs::write(&auth, auth_bytes).unwrap();
    let args = [
        "--no-daemon",
        "-c",
        "mcp_servers.other.command='existing command'",
        "resume",
        "session one",
    ];
    let output = client.command(&args, REMOTE_HELP).output().unwrap();
    assert_eq!(output.status.code(), Some(37), "{}", diagnostic(&output));
    let record = client.fields("record");
    client.assert_environment(&record);
    assert_eq!(&record[10..], args);
    assert_eq!(fs::read(&config).unwrap(), config_bytes);
    assert_eq!(fs::read(&auth).unwrap(), auth_bytes);
    assert_eq!(
        fs::read(client.dir.join("read-config")).unwrap(),
        [config_bytes.as_slice(), auth_bytes.as_slice()].concat()
    );
    assert!(client.calls().is_empty());
}
