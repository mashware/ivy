#![cfg(unix)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
fn run(args: &[&str], help: &str) -> (Vec<String>, String, i32) {
    let dir = std::env::temp_dir().join(format!(
        "ivy-codex-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    let client = dir.join("client");
    fs::write(&client, "#!/bin/sh\nif [ \"$1\" = --help ]; then printf '%s' \"$HELP_TEXT\"; exit \"${HELP_STATUS:-0}\"; fi\nprintf '%s\\0' \"$PANEL_ID\" \"$PANEL_DIR\" \"$CONFIG_SENTINEL\" \"$PWD\" \"$@\" > \"$RECORD\"\nexit 37\n").unwrap();
    fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).unwrap();
    let record = dir.join("record");
    let output = Command::new(env!("CARGO_BIN_EXE_ivy-codex"))
        .arg(&client)
        .args(args)
        .env("PANEL_ID", "pane one")
        .env("PANEL_DIR", "pane directory")
        .env("CONFIG_SENTINEL", "inherited value")
        .env("RECORD", &record)
        .env("HELP_TEXT", help)
        .current_dir(&dir)
        .output()
        .unwrap();
    let result = (
        fs::read(record)
            .unwrap_or_default()
            .split(|b| *b == 0)
            .map(|v| String::from_utf8(v.to_vec()).unwrap())
            .collect(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap(),
    );
    fs::remove_dir_all(dir).unwrap();
    let (mut values, diagnostic, status): (Vec<String>, _, _) = result;
    assert_eq!(values.pop().as_deref(), Some(""));
    (values, diagnostic, status)
}
#[test]
fn interactive_inserts_and_preserves_arguments_environment_and_status() {
    for args in [
        &[][..],
        &["resume", "session one"][..],
        &["-m", "exec", "prompt"][..],
        &["--", "--remote"][..],
        &["fork", "id", "", "line one\nline two"][..],
        &["-c", "model=exec", "prompt"][..],
        &["--config=model=exec", "prompt"][..],
        &["-mexec", "prompt", "login"][..],
        &["--model=--remote", "prompt"][..],
    ] {
        let (record, _, status) = run(args, "  --no-daemon  run locally");
        assert_eq!(
            &record[..3],
            &["pane one", "pane directory", "inherited value"]
        );
        assert_eq!(record[4], "--no-daemon");
        assert_eq!(&record[5..], args);
        assert_eq!(status, 37);
    }
}
#[test]
fn passes_through_existing_admin_remote_and_unknown() {
    for args in [
        &["--no-daemon", "resume", "id"][..],
        &["exec", "hello"][..],
        &["resume", "id", "--remote", "ws://host"][..],
        &["--future-flag"][..],
        &["--model", "--remote", "ws://host"][..],
        &["do work", "--remote", "ws://host"][..],
        &["do work", "--help"][..],
    ] {
        let (record, diagnostic, _) = run(args, "--no-daemon");
        assert_eq!(&record[4..], args);
        if args.contains(&"--remote") || args.contains(&"--future-flag") {
            assert!(!diagnostic.is_empty());
        }
    }
}
#[test]
fn unsupported_client_preserves_arguments_and_reports_limitation() {
    let (record, diagnostic, _) = run(&["fork", "id"], "old client");
    assert_eq!(&record[4..], &["fork", "id"]);
    assert!(diagnostic.contains("unverified"));
}

#[test]
fn stalled_probe_falls_back_in_bounded_time() {
    let dir = std::env::temp_dir().join(format!("ivy-codex-timeout-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let client = dir.join("client");
    fs::write(
        &client,
        "#!/bin/sh\nif [ \"$1\" = --help ]; then sleep 10; fi\nprintf '%s' \"$*\" > \"$RECORD\"\n",
    )
    .unwrap();
    fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).unwrap();
    let record = dir.join("record");
    let start = std::time::Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_ivy-codex"))
        .arg(&client)
        .arg("resume")
        .env("RECORD", &record)
        .output()
        .unwrap();
    assert!(start.elapsed() < std::time::Duration::from_secs(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unverified"));
    assert_eq!(fs::read_to_string(record).unwrap(), "resume");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn refuses_recursive_executable() {
    let launcher = env!("CARGO_BIN_EXE_ivy-codex");
    let output = Command::new(launcher).arg(launcher).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("itself"));
}

#[test]
fn preserves_client_signal() {
    use std::os::unix::process::ExitStatusExt;
    let dir = std::env::temp_dir().join(format!("ivy-codex-signal-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let client = dir.join("client");
    fs::write(&client, "#!/bin/sh\nkill -TERM $$\n").unwrap();
    fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_ivy-codex"))
        .arg(&client)
        .arg("exec")
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(15));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn inherits_auth_and_mcp_environment_without_changing_configuration_or_adding_api_keys() {
    let dir = std::env::temp_dir().join(format!(
        "ivy-codex-config-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).unwrap();
    let config = dir.join("config.toml");
    let auth = dir.join("auth.json");
    let config_bytes =
        b"# existing user settings\n[mcp_servers.ivy]\nenv_vars = ['PANEL_ID', 'PANEL_DIR']\n";
    let auth_bytes = b"{\"session\":\"inherited subscription sentinel\"}\n";
    fs::write(&config, config_bytes).unwrap();
    fs::write(&auth, auth_bytes).unwrap();
    let client = dir.join("client");
    fs::write(&client, "#!/bin/sh\nif [ \"$1\" = --help ]; then echo --no-daemon; exit 0; fi\ncat \"$CODEX_HOME/config.toml\" \"$CODEX_HOME/auth.json\" > \"$READ_CONFIG\"\nprintf '%s\\0' \"$CODEX_HOME\" \"$MCP_AUTH_SENTINEL\" \"$SUBSCRIPTION_SENTINEL\" \"${OPENAI_API_KEY-unset}\" \"${ANTHROPIC_API_KEY-unset}\" \"$@\" > \"$RECORD\"\n").unwrap();
    fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).unwrap();
    let record = dir.join("record");
    let read_config = dir.join("read-config");
    let output = Command::new(env!("CARGO_BIN_EXE_ivy-codex"))
        .arg(&client)
        .args([
            "-c",
            "mcp_servers.other.command='existing command'",
            "resume",
            "session one",
        ])
        .env("CODEX_HOME", &dir)
        .env("MCP_AUTH_SENTINEL", "existing MCP authentication")
        .env("SUBSCRIPTION_SENTINEL", "existing subscription")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env("RECORD", &record)
        .env("READ_CONFIG", &read_config)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let fields = fs::read(record).unwrap();
    let fields: Vec<&[u8]> = fields.split(|b| *b == 0).collect();
    assert_eq!(fields[0], dir.as_os_str().as_encoded_bytes());
    assert_eq!(
        &fields[1..],
        &[
            b"existing MCP authentication".as_slice(),
            b"existing subscription",
            b"unset",
            b"unset",
            b"--no-daemon",
            b"-c",
            b"mcp_servers.other.command='existing command'",
            b"resume",
            b"session one",
            b"",
        ]
    );
    assert_eq!(fs::read(&config).unwrap(), config_bytes);
    assert_eq!(fs::read(&auth).unwrap(), auth_bytes);
    assert_eq!(
        fs::read(read_config).unwrap(),
        [config_bytes.as_slice(), auth_bytes.as_slice()].concat()
    );
    fs::remove_dir_all(dir).unwrap();
}
