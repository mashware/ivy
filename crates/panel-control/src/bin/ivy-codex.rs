use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
enum Kind {
    Interactive { already: bool },
    Pass,
    Remote,
    Unknown,
}

fn classify(args: &[OsString]) -> Kind {
    let mut i = 0;
    let mut session = false;
    let mut already = false;
    while i < args.len() {
        let Some(arg) = args[i].to_str() else {
            return Kind::Unknown;
        };
        if arg == "--" {
            break;
        }
        if matches!(arg, "-h" | "--help" | "-V" | "--version") {
            return Kind::Pass;
        }
        if arg == "--no-daemon" {
            already = true;
            i += 1;
            continue;
        }
        let (key, inline) = arg.split_once('=').map_or((arg, false), |(k, _)| (k, true));
        if key == "--remote" {
            return Kind::Remote;
        }
        let value_long = matches!(
            key,
            "--config"
                | "--model"
                | "--profile"
                | "--cd"
                | "--image"
                | "--sandbox"
                | "--ask-for-approval"
                | "--enable"
                | "--disable"
                | "--add-dir"
                | "--local-provider"
                | "--remote-auth-token-env"
        );
        let value_short = matches!(arg, "-c" | "-m" | "-p" | "-C" | "-i" | "-s" | "-a");
        if value_long || value_short {
            if !inline {
                i += 1;
                if i == args.len() {
                    return Kind::Unknown;
                }
            }
            i += 1;
            continue;
        }
        if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 {
            let b = arg.as_bytes();
            if matches!(b[1], b'c' | b'm' | b'p' | b'C' | b'i' | b's' | b'a') {
                i += 1;
                continue;
            }
            return Kind::Unknown;
        }
        if matches!(
            arg,
            "--strict-config"
                | "--oss"
                | "--approve-for-me"
                | "--dangerously-bypass-approvals-and-sandbox"
                | "--dangerously-bypass-hook-trust"
                | "--worktree"
                | "--search"
                | "--no-alt-screen"
                | "--last"
                | "--all"
                | "--include-non-interactive"
        ) {
            i += 1;
            continue;
        }
        if arg.starts_with('-') {
            return Kind::Unknown;
        }
        if !session {
            if matches!(arg, "resume" | "fork") {
                session = true;
            } else if matches!(
                arg,
                "agents"
                    | "exec"
                    | "e"
                    | "review"
                    | "login"
                    | "logout"
                    | "mcp"
                    | "plugin"
                    | "app-server"
                    | "remote-control"
                    | "completion"
                    | "update"
                    | "doctor"
                    | "sandbox"
                    | "debug"
                    | "apply"
                    | "a"
                    | "queue"
                    | "archive"
                    | "delete"
                    | "migrate-rollouts"
                    | "migrate-nfs-db"
                    | "unarchive"
                    | "cloud"
                    | "exec-server"
                    | "features"
                    | "help"
            ) {
                return Kind::Pass;
            } else {
                // A prompt may be followed by root options, but later words are data.
                session = true;
            }
        }
        i += 1;
    }
    Kind::Interactive { already }
}

fn supports(exe: &OsStr) -> bool {
    let Ok(mut child) = Command::new(exe)
        .arg("--help")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return false;
    };
    let (tx, rx) = mpsc::channel();
    for mut stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut output = Vec::new();
            let mut buf = [0; 4096];
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let room = 65536usize.saturating_sub(output.len());
                output.extend_from_slice(&buf[..n.min(room)]);
            }
            let _ = tx.send(output);
        });
    }
    drop(tx);
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    if !status.is_some_and(|s| s.success()) {
        return false;
    }
    let mut found = false;
    for _ in 0..2 {
        if let Ok(bytes) = rx.recv_timeout(Duration::from_millis(100)) {
            found |= bytes
                .windows(b"--no-daemon".len())
                .any(|w| w == b"--no-daemon");
        }
    }
    found
}

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(exe) = args.next() else {
        eprintln!("ivy-codex: missing client executable");
        std::process::exit(2)
    };
    if let (Ok(target), Ok(this)) = (
        std::fs::canonicalize(&exe),
        env::current_exe().and_then(std::fs::canonicalize),
    ) && target == this
    {
        eprintln!("ivy-codex: client resolves to the launcher itself");
        std::process::exit(2);
    }
    let original: Vec<_> = args.collect();
    let mut inject = false;
    match classify(&original) {
        Kind::Interactive { already: false } => {
            inject = supports(&exe);
            if !inject {
                eprintln!(
                    "ivy-codex: client does not advertise --no-daemon or its capability probe failed; independent panel routing is unverified"
                );
            }
        }
        Kind::Interactive { already: true } | Kind::Pass => {}
        Kind::Remote => {
            eprintln!("ivy-codex: remote sessions cannot guarantee local Ivy panel routing")
        }
        Kind::Unknown => eprintln!(
            "ivy-codex: unrecognized or ambiguous options; independent panel routing is unverified"
        ),
    }
    let mut command = Command::new(exe);
    if inject {
        command.arg("--no-daemon");
    }
    command.args(original);
    let _ = std::io::stderr().flush();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = command.exec();
        eprintln!("ivy-codex: cannot launch client: {err}");
        std::process::exit(127);
    }
    #[cfg(not(unix))]
    {
        match command.status() {
            Ok(status) => std::process::exit(status.code().unwrap_or(1)),
            Err(err) => {
                eprintln!("ivy-codex: cannot launch client: {err}");
                std::process::exit(127);
            }
        }
    }
}
