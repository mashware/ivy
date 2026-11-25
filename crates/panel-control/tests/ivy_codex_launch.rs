mod native_shell_entry {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut random = [0; 8];
            getrandom::fill(&mut random).unwrap();
            let root = std::env::temp_dir()
                .join(format!("ivy-native-entry-{:x}", u64::from_ne_bytes(random)));
            std::fs::create_dir(&root).unwrap();
            let code = r#"fn main() {
                use std::io::Write;
                let args: Vec<_> = std::env::args().skip(1).collect();
                std::fs::write(std::env::var_os("IVY_NATIVE_RECORD").unwrap(), format!("{args:?}")).unwrap();
                if let Some(path) = std::env::var_os("IVY_NATIVE_CALLS") {
                    let mut calls = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
                    writeln!(calls, "{args:?}").unwrap();
                }
                if let Some(path) = std::env::var_os("IVY_NATIVE_IDENTITY") {
                    std::fs::write(path, std::env::current_exe().unwrap().to_str().unwrap()).unwrap();
                }
                std::process::exit(std::env::var("IVY_NATIVE_STATUS").unwrap().parse().unwrap());
            }"#;
            std::fs::write(root.join("client.rs"), code).unwrap();
            assert!(
                Command::new("rustc")
                    .arg("--crate-name=ivy_native_fixture")
                    .arg(root.join("client.rs"))
                    .arg("-o")
                    .arg(root.join("codex.exe"))
                    .status()
                    .unwrap()
                    .success()
            );
            std::fs::copy(env!("CARGO_BIN_EXE_ivy-codex"), root.join("ivy-codex.exe")).unwrap();
            std::fs::copy(
                env!("CARGO_BIN_EXE_ivy-codex"),
                root.join("ivy-codex-shell.exe"),
            )
            .unwrap();
            Self(root)
        }
        fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
            let mut command = Command::new(program);
            command
                .env("IVY_NATIVE_RECORD", self.0.join("record"))
                .env("IVY_NATIVE_CALLS", self.0.join("calls"))
                .env("IVY_NATIVE_STATUS", "37")
                .env("PATH", &self.0)
                .env("IVY_CODEX_HELPER", self.0.join("ivy-codex.exe"))
                .env("IVY_TEST_ROOT", &self.0)
                .env("XDG_CACHE_HOME", &self.0)
                .env("XDG_CONFIG_HOME", &self.0)
                .env("XDG_DATA_HOME", &self.0)
                .env("TERM", "dumb");
            command
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn powershell_programs() -> Vec<(PathBuf, u32)> {
        #[cfg(windows)]
        {
            let legacy = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            assert!(
                legacy.is_file(),
                "Windows PowerShell is required: {legacy:?}"
            );
            let pwsh = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|dir| dir.join("pwsh.exe"))
                .find(|path| path.is_file())
                .expect("PowerShell 7 is required on Windows; install pwsh and add it to PATH");
            vec![(legacy, 5), (pwsh, 7)]
        }
        #[cfg(not(windows))]
        {
            std::env::var_os("IVY_TEST_PWSH")
                .map(|path| (PathBuf::from(path), 7))
                .into_iter()
                .collect()
        }
    }

    fn output(mut command: Command, root: &Path) -> String {
        let out = root.join("stdout");
        let err = root.join("stderr");
        command
            .stdout(std::fs::File::create(&out).unwrap())
            .stderr(std::fs::File::create(&err).unwrap())
            .stdin(Stdio::null());
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("native invocation timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let text = std::fs::read_to_string(out).unwrap();
        assert!(
            status.success(),
            "{text}\n{}",
            std::fs::read_to_string(err).unwrap()
        );
        text
    }

    #[test]
    fn native_shell_entry_resolves_current_path_and_keeps_explicit_mode() {
        let fixture = Fixture::new();
        let shell = fixture.0.join("ivy-codex-shell.exe");
        let args = ["-V", "-i", "image one", "", "--long", "quote\"here"];
        let result = fixture.command(&shell).args(args).status().unwrap();
        assert_eq!(result.code(), Some(37));
        let expected = format!("{:?}", args);
        assert_eq!(
            std::fs::read_to_string(fixture.0.join("record")).unwrap(),
            expected
        );
        let other = fixture.0.join("new path");
        std::fs::create_dir(&other).unwrap();
        std::fs::rename(fixture.0.join("codex.exe"), other.join("codex.exe")).unwrap();
        // A batch candidate in the earlier directory must lose to an exe in the later one.
        std::fs::write(fixture.0.join("codex.cmd"), "not the executable").unwrap();
        let path = std::env::join_paths([&fixture.0, &other]).unwrap();
        let result = fixture
            .command(&shell)
            .env("PATH", path)
            .env("IVY_NATIVE_STATUS", "0")
            .args(args)
            .status()
            .unwrap();
        assert_eq!(result.code(), Some(0));
        let result = fixture
            .command(fixture.0.join("ivy-codex.exe"))
            .arg(other.join("codex.exe"))
            .args(args)
            .status()
            .unwrap();
        assert_eq!(result.code(), Some(37));
        let result = fixture
            .command(&shell)
            .env("PATH", "relative-only")
            .current_dir(&other)
            .args(args)
            .status()
            .unwrap();
        assert_eq!(result.code(), Some(127));
    }

    #[test]
    fn native_shell_entry_preserves_native_powershell_status_and_arguments() {
        for (program, major) in powershell_programs() {
            for batch in if cfg!(windows) {
                vec![false, true]
            } else {
                vec![false]
            } {
                let fixture = Fixture::new();
                let target = if batch {
                    std::fs::rename(fixture.0.join("codex.exe"), fixture.0.join("client.exe"))
                        .unwrap();
                    std::fs::write(
                        fixture.0.join("codex.cmd"),
                        "@echo off\r\n\"%~dp0client.exe\" %*\r\nexit /b %errorlevel%\r\n",
                    )
                    .unwrap();
                    "client.exe"
                } else {
                    "codex.exe"
                };
                std::fs::write(fixture.0.join("codex.ps1"), "throw \"wrong launcher\"").unwrap();
                let script = include_str!(
                    "../../../resources/agent-integrations/codex/shell/powershell.ps1"
                );
                let body = r#"
$alias = Get-Command codex
if ($alias.CommandType -ne 'Alias' -or $alias.Definition -ne (Join-Path $env:IVY_TEST_ROOT 'ivy-codex-shell.exe')) {
    throw 'native alias target missing'
}
'VERSION:' + $PSVersionTable.PSVersion.Major
$nativeArgs = @('-V', '-i', 'image one', '', '--long', 'trailing\')
if ($env:IVY_NATIVE_TARGET -eq 'codex.exe') {
    $nativeArgs += 'amp&pipe|caret^percent%dollar$paren()semi;angle<>'
}
& (Join-Path $env:IVY_TEST_ROOT $env:IVY_NATIVE_TARGET) @nativeArgs
'DIRECT:' + $? + ':' + $LASTEXITCODE
$direct = [IO.File]::ReadAllText($env:IVY_NATIVE_RECORD)
codex @nativeArgs
'WRAPPED:' + $? + ':' + $LASTEXITCODE
$wrapped = [IO.File]::ReadAllText($env:IVY_NATIVE_RECORD)
if ($direct -ne $wrapped) { throw "argument mismatch: $direct / $wrapped" }
'ARGS_EQUAL'
$env:IVY_NATIVE_STATUS = '0'
codex @nativeArgs
'SUCCESS:' + $? + ':' + $LASTEXITCODE
if ($PSVersionTable.PSVersion.Major -ge 7) {
    Invoke-Expression 'codex @nativeArgs && Write-Output SUCCESS_CHAIN'
    $env:IVY_NATIVE_STATUS = '37'
    Invoke-Expression 'codex @nativeArgs && Write-Output WRONG_FAILURE_CHAIN'
    'CHAINS_CHECKED'
}
exit 0
"#;
                let mut command = fixture.command(&program);
                command.env("IVY_NATIVE_TARGET", target);
                command.args([
                    "-NoLogo",
                    "-NoProfile",
                    "-Command",
                    &format!("{script}\n{body}"),
                ]);
                let out = output(command, &fixture.0);
                assert!(out.contains(&format!("VERSION:{major}")), "{out}");
                assert!(out.contains("DIRECT:False:37"), "{out}");
                assert!(out.contains("WRAPPED:False:37"), "{out}");
                assert!(
                    out.contains("SUCCESS:True:0") && out.contains("ARGS_EQUAL"),
                    "{out}"
                );
                assert!(!out.contains("WRONG_FAILURE_CHAIN"), "{out}");
                if major >= 7 {
                    assert!(out.contains("CHAINS_CHECKED"), "{out}");
                    assert!(out.contains("SUCCESS_CHAIN"), "{out}");
                }
                let mut expected = vec!["-V", "-i", "image one", "", "--long", "trailing\\"];
                if major == 5 {
                    expected.retain(|arg| !arg.is_empty());
                }
                if !batch {
                    expected.push("amp&pipe|caret^percent%dollar$paren()semi;angle<>");
                }
                assert_eq!(
                    std::fs::read_to_string(fixture.0.join("record")).unwrap(),
                    format!("{expected:?}"),
                    "{program:?}, batch={batch}"
                );
            }
        }
    }

    #[test]
    fn native_shell_entry_resolves_path_changes_after_powershell_installs_the_alias() {
        for (program, _) in powershell_programs() {
            let fixture = Fixture::new();
            for name in ["first", "second"] {
                std::fs::create_dir(fixture.0.join(name)).unwrap();
            }
            std::fs::rename(
                fixture.0.join("codex.exe"),
                fixture.0.join("first/codex.exe"),
            )
            .unwrap();
            std::fs::copy(
                fixture.0.join("first/codex.exe"),
                fixture.0.join("second/codex.exe"),
            )
            .unwrap();
            let script =
                include_str!("../../../resources/agent-integrations/codex/shell/powershell.ps1");
            let body = r#"
$alias = Get-Command codex
if ($alias.CommandType -ne 'Alias' -or $alias.Definition -ne (Join-Path $env:IVY_TEST_ROOT 'ivy-codex-shell.exe')) {
    throw 'native alias target missing'
}
foreach ($name in @('first', 'second')) {
    $env:PATH = Join-Path $env:IVY_TEST_ROOT $name
    $env:IVY_NATIVE_RECORD = Join-Path $env:IVY_TEST_ROOT "$name-args"
    $env:IVY_NATIVE_IDENTITY = Join-Path $env:IVY_TEST_ROOT "$name-identity"
    codex -V "image $name"
    if (-not $? -or $LASTEXITCODE -ne 0) { throw "client $name failed" }
}
'PATH_CHANGES_CHECKED'
exit 0
"#;
            let mut command = fixture.command(&program);
            command.env("IVY_NATIVE_STATUS", "0").args([
                "-NoLogo",
                "-NoProfile",
                "-Command",
                &format!("{script}\n{body}"),
            ]);
            assert!(output(command, &fixture.0).contains("PATH_CHANGES_CHECKED"));
            for name in ["first", "second"] {
                let identity =
                    std::fs::read_to_string(fixture.0.join(format!("{name}-identity"))).unwrap();
                assert_eq!(
                    std::fs::canonicalize(identity).unwrap(),
                    std::fs::canonicalize(fixture.0.join(name).join("codex.exe")).unwrap()
                );
                assert_eq!(
                    std::fs::read_to_string(fixture.0.join(format!("{name}-args"))).unwrap(),
                    format!("{:?}", ["-V", &format!("image {name}")])
                );
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn unsupported_windows_relay_explains_why_and_preserves_the_client_invocation() {
        let fixture = Fixture::new();
        let args = ["prompt with \"quotes\"", "--", "", "trailing\\", "a&b"];
        let result = fixture
            .command(fixture.0.join("ivy-codex.exe"))
            .arg(fixture.0.join("codex.exe"))
            .args(args)
            .env("CODEX_HOME", &fixture.0)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(37));
        assert_eq!(
            std::fs::read_to_string(fixture.0.join("record")).unwrap(),
            format!("{args:?}")
        );
        assert_eq!(
            std::fs::read_to_string(fixture.0.join("calls")).unwrap(),
            format!("{args:?}\n"),
            "the unsupported path must invoke only the original client, without capability or daemon probes"
        );
        assert_eq!(
            String::from_utf8(result.stderr)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [
                "ivy-codex: shared panel routing is not supported on Windows: Ivy cannot yet verify the owning console for a shared conversation; preserving the original Codex command without adding --no-daemon; local panel routing is unverified"
            ]
        );
        assert!(!fixture.0.join("ivy-panel-routes").exists());

        for args in [
            vec!["--no-daemon", "hello"],
            vec!["--help"],
            vec!["app-server", "daemon", "status"],
        ] {
            std::fs::remove_file(fixture.0.join("calls")).unwrap();
            let result = fixture
                .command(fixture.0.join("ivy-codex.exe"))
                .arg(fixture.0.join("codex.exe"))
                .args(&args)
                .output()
                .unwrap();
            assert_eq!(result.status.code(), Some(37));
            assert!(
                result.stderr.is_empty(),
                "bypass invocation gained a routing warning"
            );
            assert_eq!(
                std::fs::read_to_string(fixture.0.join("calls")).unwrap(),
                format!("{args:?}\n")
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_shell_entry_refuses_unsafe_batch_arguments_visibly() {
        let fixture = Fixture::new();
        std::fs::rename(fixture.0.join("codex.exe"), fixture.0.join("client.exe")).unwrap();
        std::fs::write(
            fixture.0.join("codex.cmd"),
            "@echo off\r\n\"%~dp0client.exe\" %*\r\nexit /b %errorlevel%\r\n",
        )
        .unwrap();
        for argument in ["line\nbreak", "line\rbreak"] {
            let result = fixture
                .command(fixture.0.join("ivy-codex-shell.exe"))
                .args(["-V", argument])
                .output()
                .unwrap();
            assert!(!result.status.success(), "unsafe batch argument accepted");
            assert!(!result.stderr.is_empty(), "batch failure was silent");
            assert!(!fixture.0.join("record").exists(), "client was started");
        }
    }
}

#[cfg(unix)]
mod tests {
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
    if [ -n "${IVY_NATIVE_RESPONSIBILITY_RECORD-}" ]; then
        /usr/bin/python3 -c 'import ctypes,json,os,sys; query=ctypes.CDLL(None).responsibility_get_pid_responsible_for_pid; query.argtypes=[ctypes.c_int]; query.restype=ctypes.c_int; json.dump({"daemon_pid":os.getppid(),"responsible":query(os.getpid())},open(sys.argv[1],"w"))' "$IVY_NATIVE_RESPONSIBILITY_RECORD" || exit 92
    fi
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

    // macOS responsibility is kernel state; the Python supplied with Xcode queries it
    // in the actual daemon-start fixture, so bypassing the launch correction fails this test.
    #[cfg(target_os = "macos")]
    #[test]
    fn ordinary_daemon_start_separates_native_responsibility_but_explicit_admin_keeps_it() {
        let client = Client::new();
        let record = client.dir.join("native-responsibility.json");
        for (args, separate) in [
            (&["app-server", "daemon", "start"][..], false),
            (&[][..], true),
        ] {
            let output = client
                .command(args, REMOTE_HELP)
                .env("IVY_NATIVE_RESPONSIBILITY_RECORD", &record)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(if separate { 37 } else { 0 }),
                "{}",
                diagnostic(&output)
            );
            let captured: serde_json::Value =
                serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
            if separate {
                assert_eq!(captured["responsible"], captured["daemon_pid"]);
            } else {
                assert_ne!(
                    captured["responsible"], captured["daemon_pid"],
                    "control already disclaimed; native regression oracle is unavailable"
                );
            }
        }
        assert_eq!(client.fields("daemon-env"), ["unset", "unset"]);
        assert_eq!(client.calls(), "daemon\nhelp\ndaemon\n");
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
}

#[cfg(windows)]
mod tests {
    #[test]
    #[ignore = "unix-only: requires POSIX shell executables, signal behavior, and executable permission bits"]
    fn explicit_independent_admin_remote_and_unknown_preserve_arguments_environment_and_status() {}

    #[test]
    #[ignore = "unix-only: requires POSIX shell executables, signal behavior, and executable permission bits"]
    fn shared_launch_uses_capabilities_with_arbitrary_cli_and_server_version_labels() {}

    #[test]
    #[ignore = "unix-only: requires POSIX shell executables, signal behavior, and executable permission bits"]
    fn missing_remote_capabilities_never_start_an_independent_session() {}

    #[test]
    #[ignore = "unix-only: requires POSIX shell executables, signal behavior, and executable permission bits"]
    fn failed_shared_setup_never_starts_an_independent_session() {}

    #[test]
    #[ignore = "unix-only: requires POSIX shell executables, signal behavior, and executable permission bits"]
    fn stalled_capability_probe_fails_in_bounded_time_without_starting_a_session() {}

    #[test]
    #[ignore = "unix-only: requires POSIX shell executables, signal behavior, and executable permission bits"]
    fn refuses_recursive_executable() {}

    #[test]
    #[ignore = "unix-only: requires POSIX shell executables, signal behavior, and executable permission bits"]
    fn preserves_client_signal() {}

    #[test]
    #[ignore = "unix-only: requires POSIX shell executables, signal behavior, and executable permission bits"]
    fn inherits_auth_and_mcp_environment_without_changing_configuration_or_adding_api_keys() {}
}
