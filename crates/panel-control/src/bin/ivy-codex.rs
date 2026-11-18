use std::env;
use std::ffi::OsString;
use std::io::Write;
use std::process::Command;

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
                if i == args.len() || args[i].to_string_lossy().starts_with('-') {
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
    if matches!(classify(&original), Kind::Interactive { already: false })
        && panel_control::codex_relay::supported(&original)
    {
        match panel_control::codex_relay::run(&exe, &original) {
            Ok(status) => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    if let Some(signal) = status.signal() {
                        unsafe {
                            libc::signal(signal, libc::SIG_DFL);
                            libc::raise(signal);
                        }
                    }
                }
                std::process::exit(status.code().unwrap_or(1));
            }
            Err(failure) => {
                eprintln!(
                    "ivy-codex: shared panel connection could not start ({failure}); use --no-daemon explicitly for independent mode"
                );
                std::process::exit(1);
            }
        }
    }
    match classify(&original) {
        Kind::Interactive { already: false } => {
            eprintln!(
                "ivy-codex: this platform or invocation cannot use Ivy's shared-panel bridge; preserving the original Codex command without adding --no-daemon; local panel routing is unverified"
            );
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
