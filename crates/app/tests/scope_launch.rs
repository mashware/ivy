#[cfg(target_os = "linux")]
use std::ffi::OsStr;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};
#[cfg(target_os = "linux")]
use term_core::{Pty, Size, TestScopePlacement};

#[cfg(target_os = "linux")]
fn launcher() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_app"))
}
#[cfg(target_os = "linux")]
fn read(output: &std::sync::mpsc::Receiver<Vec<u8>>, marker: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut text = Vec::new();
    while Instant::now() < deadline {
        if let Ok(chunk) = output.recv_timeout(Duration::from_millis(20)) {
            text.extend(chunk);
        }
        if String::from_utf8_lossy(&text).contains(marker) {
            return String::from_utf8(text).unwrap();
        }
    }
    panic!("missing {marker:?}: {}", String::from_utf8_lossy(&text));
}

#[cfg(target_os = "linux")]
#[test]
fn real_ivy_helper_preserves_argv_environment_directory_and_controlling_terminal() {
    let root = std::env::temp_dir();
    let (mut pty, output) = Pty::spawn_with_test_launcher_in(launcher(), "fixture", "/bin/sh",
        &["-c", "test -t 0 && test -t 1 && printf 'RESULT:%s:%s:%s:%s:END' \"$0\" \"$1\" \"$EXACT\" \"$PWD\"", "a literal;$arg", "mañana"],
        Size::default(), &[("EXACT", Some(OsStr::new("spaces;$literal")))], Some(&root), TestScopePlacement::Ready).unwrap();
    let text = read(&output, ":END");
    assert!(
        text.contains(&format!(
            "RESULT:a literal;$arg:mañana:spaces;$literal:{}:END",
            root.display()
        )),
        "{text}"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while pty.exit_success().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(pty.exit_success(), Some(true));
}

#[cfg(target_os = "linux")]
#[test]
fn real_helper_returns_failed_exec_instead_of_reporting_successful_launch() {
    let error = Pty::spawn_with_test_launcher_in(
        launcher(),
        "fixture",
        "/definitely/no/ivy-program",
        &[],
        Size::default(),
        &[],
        None,
        TestScopePlacement::Ready,
    )
    .err()
    .expect("exec must fail");
    assert!(error.to_string().contains("No such file"), "{error}");
}

#[cfg(target_os = "linux")]
#[test]
fn failed_and_stalled_manager_preserve_argv_environment_cwd_exactly_once() {
    let root = std::env::temp_dir();
    for placement in [TestScopePlacement::Denied, TestScopePlacement::Stalled] {
        let before = Instant::now();
        let (mut pty, output) = Pty::spawn_with_test_launcher_in(
            launcher(),
            "fixture",
            "/bin/sh",
            &[
                "-c",
                "printf 'ONLY-ONCE:%s:%s:%s:%s:END' \"$0\" \"$1\" \"$EXACT\" \"$PWD\"",
                "literal;$argv0",
                "mañana spaces",
            ],
            Size::default(),
            &[("EXACT", Some(OsStr::new("spaces;$literal")))],
            Some(&root),
            placement,
        )
        .unwrap();
        let mut text = read(&output, ":END");
        let deadline = Instant::now() + Duration::from_secs(3);
        while pty.exit_success().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        while let Ok(bytes) = output.try_recv() {
            text.push_str(&String::from_utf8_lossy(&bytes));
        }
        assert_eq!(text.matches("ONLY-ONCE").count(), 1, "{text}");
        assert!(
            text.contains(&format!(
                "ONLY-ONCE:literal;$argv0:mañana spaces:spaces;$literal:{}:END",
                root.display()
            )),
            "{text}"
        );
        assert!(before.elapsed() < Duration::from_secs(3));
        assert_eq!(pty.exit_success(), Some(true));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn real_helper_honors_path_override_and_relative_executable() {
    use std::os::unix::fs::PermissionsExt;
    let root = std::env::temp_dir().join(format!("ivy-helper-path-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let program = root.join("fixture");
    std::fs::write(&program, "#!/bin/sh\nprintf 'PATH-OK:%s:END' \"$1\"\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    for name in ["fixture", "./fixture"] {
        let (_pty, output) = Pty::spawn_with_test_launcher_in(
            launcher(),
            "fixture",
            name,
            &["literal argument"],
            Size::default(),
            &[("PATH", Some(root.as_os_str()))],
            Some(&root),
            TestScopePlacement::Ready,
        )
        .unwrap();
        assert!(read(&output, ":END").contains("PATH-OK:literal argument:END"));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "requires a real systemd user manager; run explicitly with the session bus"]
fn real_systemd_scopes_separate_roots_and_immediate_descendants_from_the_gui() {
    let launch = || {
        Pty::spawn_with_launcher_in(
            launcher(),
            "integration",
            "/bin/sh",
            &["-c", "sleep 30 & printf '%s %s\\n' $$ $!; wait"],
            Size::default(),
            &[],
            None,
        )
        .unwrap()
    };
    let (_first, a) = launch();
    let (_second, b) = launch();
    let get = |output| {
        let text = read(output, "\n");
        let pids: Vec<u32> = text
            .split_whitespace()
            .map(|p| p.parse().unwrap())
            .collect();
        assert_eq!(pids.len(), 2, "{text}");
        let group = std::fs::read_to_string(format!("/proc/{}/cgroup", pids[0])).unwrap();
        let descendant = std::fs::read_to_string(format!("/proc/{}/cgroup", pids[1])).unwrap();
        assert!(group.contains("/app.slice/ivy-console-"), "{group}");
        assert_eq!(group, descendant);
        group
    };
    let first = get(&a);
    let second = get(&b);
    assert_ne!(first, second);
    let parent = std::fs::read_to_string("/proc/self/cgroup").unwrap();
    assert_ne!(parent, first);
    assert_ne!(parent, second);
}

#[test]
#[ignore = "requires an owned systemd test scope with memory-controller delegation"]
fn bounded_kernel_oom_is_attributed_without_touching_other_scopes() {
    let (mut victim, output) = Pty::spawn_with_launcher_in(launcher(), "bounded-oom-test", "/usr/bin/python3",
        &["-c", "import os; print('READY:'+str(os.getpid()), flush=True); input(); x=bytearray(64*1024*1024); print('unexpected survival', flush=True)"],
        Size::default(), &[], None).unwrap();
    let text = read(&output, "\n");
    let pid: u32 = text.trim().strip_prefix("READY:").unwrap().parse().unwrap();
    let group = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
    let name = group.trim().rsplit('/').next().unwrap();
    assert!(
        name.starts_with("ivy-console-") && name.ends_with(".scope"),
        "{group}"
    );
    assert!(
        name.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
    );
    // The limit belongs only to this test's freshly-created leaf scope, never to the user slice.
    let result = std::process::Command::new("systemctl")
        .args([
            "--user",
            "set-property",
            "--runtime",
            name,
            "MemoryMax=32M",
            "MemorySwapMax=0",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let (mut sibling, _) = Pty::spawn_with_launcher_in(
        launcher(),
        "oom-sibling",
        "/bin/sleep",
        &["30"],
        Size::default(),
        &[],
        None,
    )
    .unwrap();
    victim.write(b"allocate\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while victim.termination() != term_core::Termination::Oom && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(victim.termination(), term_core::Termination::Oom);
    assert_eq!(
        sibling.exit_success(),
        None,
        "unrelated console was terminated"
    );
}

#[test]
#[ignore = "managed pressure is allowed only inside the explicitly marked disposable QEMU guest"]
fn real_systemd_oomd_kills_only_the_busy_console_in_a_disposable_guest() {
    let product = std::fs::read_to_string("/sys/class/dmi/id/product_name").unwrap();
    assert_eq!(
        product.trim(),
        "Ivy124Disposable",
        "never generate managed pressure on the host"
    );
    let ctl = |properties: &[&str]| {
        let result = std::process::Command::new("systemctl")
            .arg("--user")
            .args(properties)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    };
    ctl(&[
        "set-property",
        "--runtime",
        "app.slice",
        "ManagedOOMMemoryPressure=kill",
        "ManagedOOMMemoryPressureLimit=10%",
        "ManagedOOMMemoryPressureDurationSec=2s",
    ]);
    let (mut victim, output) = Pty::spawn_with_launcher_in(launcher(), "managed-oom-test", "/usr/bin/python3",
        &["-c", "import os; print('READY:'+str(os.getpid()), flush=True); input()\nwhile True:\n x=bytearray(96*1024*1024)\n del x"],
        Size::default(), &[], None).unwrap();
    let text = read(&output, "\n");
    let pid: u32 = text.trim().strip_prefix("READY:").unwrap().parse().unwrap();
    let group = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap();
    let path = group.trim().strip_prefix("0::").unwrap();
    let name = path.rsplit('/').next().unwrap();
    assert!(
        name.starts_with("ivy-console-") && name.ends_with(".scope"),
        "{group}"
    );
    ctl(&[
        "set-property",
        "--runtime",
        name,
        "MemoryHigh=16M",
        "MemoryMax=160M",
        "MemorySwapMax=0",
    ]);
    let (mut sibling, _) = Pty::spawn_with_launcher_in(
        launcher(),
        "managed-oom-sibling",
        "/bin/sleep",
        &["90"],
        Size::default(),
        &[],
        None,
    )
    .unwrap();
    let events = format!("/sys/fs/cgroup{path}/memory.events");
    let mut kernel_kills = 0;
    victim.write(b"allocate\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(50);
    while victim.termination() != term_core::Termination::Oom && Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(&events)
            && let Some(count) = text.lines().find_map(|line| line.strip_prefix("oom_kill "))
        {
            kernel_kills = kernel_kills.max(count.parse::<u64>().unwrap());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let journal = std::process::Command::new("sudo")
        .args([
            "-n",
            "journalctl",
            "-u",
            "systemd-oomd",
            "-b",
            "--no-pager",
            "-o",
            "cat",
        ])
        .output()
        .unwrap();
    let journal = String::from_utf8_lossy(&journal.stdout);
    let selected: Vec<_> = journal.lines().filter(|line| line.contains(name)).collect();
    println!("systemd-oomd evidence: {}", selected.join("\n"));
    println!(
        "kernel oom_kill counter: {kernel_kills}; sibling exit: {:?}",
        sibling.exit_success()
    );
    ctl(&[
        "set-property",
        "--runtime",
        "app.slice",
        "ManagedOOMMemoryPressure=auto",
    ]);
    assert_eq!(
        victim.termination(),
        term_core::Termination::Oom,
        "{journal}"
    );
    assert_eq!(
        kernel_kills, 0,
        "this scenario must prove userspace oomd rather than kernel OOM"
    );
    assert!(
        !selected.is_empty(),
        "oomd did not identify the test scope: {journal}"
    );
    assert_eq!(sibling.exit_success(), None, "unrelated console died");
}

#[cfg(target_os = "linux")]
#[test]
fn real_helper_preserves_resize_input_session_and_root_close() {
    let (mut pty, output) = Pty::spawn_with_test_launcher_in(
        launcher(), "pty-semantics", "/usr/bin/python3",
        &["-u", "-c", "import os,fcntl,termios,struct\nprint('ROOT:%d:%d:%d:READY' % (os.getpid(),os.getpgrp(),os.getsid(0)),flush=True)\nline=input()\nrows,cols,_,_=struct.unpack('HHHH',fcntl.ioctl(0,termios.TIOCGWINSZ,b'\\0'*8))\nprint('INPUT:%s:SIZE:%d:%d:END' % (line,rows,cols),flush=True)\ninput()"],
        Size::default(), &[], None, TestScopePlacement::Ready).unwrap();
    let ready = read(&output, ":READY");
    let root = ready
        .split("ROOT:")
        .nth(1)
        .unwrap()
        .split(":READY")
        .next()
        .unwrap();
    let ids: Vec<u32> = root
        .split(':')
        .map(|value| value.parse().unwrap())
        .collect();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], ids[1], "root no longer leads its process group");
    assert_eq!(ids[0], ids[2], "root no longer owns its terminal session");
    pty.resize(Size { rows: 37, cols: 93 }).unwrap();
    pty.write(b"literal input;$value\n").unwrap();
    assert!(read(&output, ":END").contains("INPUT:literal input;$value:SIZE:37:93:END"));
    drop(pty);
    let deadline = Instant::now() + Duration::from_secs(3);
    while unsafe { libc::kill(ids[0] as i32, 0) } == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        unsafe { libc::kill(ids[0] as i32, 0) },
        -1,
        "closing left the root alive"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn denied_scope_fallback_runs_once_with_closed_or_stalled_stderr() {
    use std::io::{Read, Write};
    const CHILD: &str = "IVY_SCOPE_STDERR_FIXTURE";
    if let Ok(mode) = std::env::var(CHILD) {
        // The parent settles the pipe state before allowing the launch to reach its diagnostic.
        let mut release = [0];
        std::io::stdin().read_exact(&mut release).unwrap();
        if mode == "stalled" {
            let flags = unsafe { libc::fcntl(2, libc::F_GETFL) };
            assert!(flags >= 0);
            assert_eq!(
                unsafe { libc::fcntl(2, libc::F_SETFL, flags | libc::O_NONBLOCK) },
                0
            );
            let filler = [b'x'; 8192];
            loop {
                let written = unsafe { libc::write(2, filler.as_ptr().cast(), filler.len()) };
                if written < 0 {
                    assert_eq!(
                        std::io::Error::last_os_error().kind(),
                        std::io::ErrorKind::WouldBlock
                    );
                    break;
                }
            }
            assert_eq!(unsafe { libc::fcntl(2, libc::F_SETFL, flags) }, 0);
        }
        // The first report may occupy the writer and the second its one queue slot.
        // A third fallback must also finish without waiting for either.
        for _ in 0..3 {
            let (mut pty, output) = Pty::spawn_with_test_launcher_in(
                launcher(),
                "stderr-fixture",
                "/bin/sh",
                &["-c", "printf 'ONLY-ONCE:END'"],
                Size::default(),
                &[],
                None,
                TestScopePlacement::Denied,
            )
            .unwrap();
            let mut text = read(&output, ":END");
            let deadline = Instant::now() + Duration::from_secs(2);
            while pty.exit_success().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(2));
            }
            while let Ok(bytes) = output.try_recv() {
                text.push_str(&String::from_utf8_lossy(&bytes));
            }
            assert_eq!(pty.exit_success(), Some(true));
            assert_eq!(text.matches("ONLY-ONCE").count(), 1, "{text}");
        }
        return;
    }
    for mode in ["closed", "stalled"] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "denied_scope_fallback_runs_once_with_closed_or_stalled_stderr",
                "--nocapture",
            ])
            .env(CHILD, mode)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        if mode == "closed" {
            drop(child.stderr.take());
        }
        child.stdin.take().unwrap().write_all(&[1]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "{mode} stderr prevented fallback: {status}"
                );
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("{mode} stderr stalled the scope fallback");
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

macro_rules! windows_unix_omission {
    ($name:ident, $reason:literal) => {
        #[cfg(windows)]
        #[test]
        #[ignore = $reason]
        fn $name() {}
    };
}

#[cfg(windows)]
windows_unix_omission!(
    real_ivy_helper_preserves_argv_environment_directory_and_controlling_terminal,
    "unix-only: invokes /bin/sh and verifies Unix controlling-terminal behavior"
);
windows_unix_omission!(
    real_helper_returns_failed_exec_instead_of_reporting_successful_launch,
    "unix-only: requires Unix exec failure and ENOENT reporting from the Linux test launcher"
);
windows_unix_omission!(
    failed_and_stalled_manager_preserve_argv_environment_cwd_exactly_once,
    "unix-only: invokes /bin/sh and exercises Linux process-manager stderr backpressure"
);
windows_unix_omission!(
    real_helper_honors_path_override_and_relative_executable,
    "unix-only: creates executable POSIX shell fixtures and verifies Unix PATH lookup"
);
windows_unix_omission!(
    real_systemd_scopes_separate_roots_and_immediate_descendants_from_the_gui,
    "unix-only: requires Linux systemd user scopes and process ancestry"
);
windows_unix_omission!(
    bounded_kernel_oom_is_attributed_without_touching_other_scopes,
    "unix-only: requires delegated Linux cgroup memory-controller scopes"
);
windows_unix_omission!(
    real_systemd_oomd_kills_only_the_busy_console_in_a_disposable_guest,
    "unix-only: requires systemd-oomd and a disposable Linux guest"
);
windows_unix_omission!(
    real_helper_preserves_resize_input_session_and_root_close,
    "unix-only: verifies Linux process groups, sessions and TIOCGWINSZ terminal ioctl"
);
windows_unix_omission!(
    denied_scope_fallback_runs_once_with_closed_or_stalled_stderr,
    "unix-only: exercises Linux pipe closure and fcntl nonblocking stderr behavior"
);
