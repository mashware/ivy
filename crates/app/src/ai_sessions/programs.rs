use panel_control::session::Client;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) struct ResolvedLaunch {
    pub(crate) program: PathBuf,
    pub(crate) prefix_args: Vec<String>,
    pub(crate) path: OsString,
}

pub(super) struct Program {
    pub(super) client: Client,
    pub(super) native: PathBuf,
    pub(super) wrapper: Option<(PathBuf, PathBuf)>,
    #[cfg(test)]
    pub(super) fixture: bool,
}

pub(crate) fn resolve_launch(client: Client) -> io::Result<ResolvedLaunch> {
    let path = trusted_path(std::env::var_os("PATH").as_deref().unwrap_or_default())?;
    let entry = resolve(client.name(), &path)?;
    if is_native(&entry)? {
        return Ok(ResolvedLaunch {
            program: entry,
            prefix_args: Vec::new(),
            path,
        });
    }
    if client != Client::Codex || !is_codex_script(&entry) {
        return Err(unsupported());
    }
    let node = resolve("node", &path)?;
    if !is_native(&node)? {
        return Err(unsupported());
    }
    Ok(ResolvedLaunch {
        program: node,
        prefix_args: vec![entry.to_str().ok_or_else(unsupported)?.to_owned()],
        path,
    })
}

pub(super) fn discover() -> Vec<Program> {
    [Client::Codex, Client::Claude, Client::Opencode]
        .into_iter()
        .filter_map(|client| {
            let launch = resolve_launch(client).ok()?;
            if launch.prefix_args.is_empty() {
                Some(Program {
                    client,
                    native: launch.program,
                    wrapper: None,
                    #[cfg(test)]
                    fixture: false,
                })
            } else {
                let script = PathBuf::from(&launch.prefix_args[0]);
                Some(Program {
                    client,
                    native: codex_binary(&script)?,
                    wrapper: Some((launch.program, script)),
                    #[cfg(test)]
                    fixture: false,
                })
            }
        })
        .collect()
}

pub(super) fn supplied(programs: Vec<(Client, PathBuf)>) -> Vec<Program> {
    programs
        .into_iter()
        .filter_map(|(client, path)| {
            let path = std::fs::canonicalize(path).ok()?;
            is_native(&path).ok()?.then_some(Program {
                client,
                native: path,
                wrapper: None,
                #[cfg(test)]
                fixture: true,
            })
        })
        .collect()
}

fn trusted_path(path: &OsStr) -> io::Result<OsString> {
    std::env::join_paths(std::env::split_paths(path).filter(|path| path.is_absolute()))
        .map_err(io::Error::other)
}

fn resolve(name: &str, path: &OsStr) -> io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let candidate = std::env::split_paths(path)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            candidate
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "client executable is unavailable")
        })?;
    let canonical = std::fs::canonicalize(&candidate)?;
    if canonical.file_name() == Some(OsStr::new("volta-shim")) {
        let volta =
            std::fs::canonicalize(candidate.parent().ok_or_else(unsupported)?.join("volta"))?;
        if !is_native(&volta)? {
            return Err(unsupported());
        }
        return volta_which(&volta, name, path);
    }
    Ok(canonical)
}

fn volta_which(volta: &Path, name: &str, path: &OsStr) -> io::Result<PathBuf> {
    let mut child = Command::new(volta)
        .args(["which", name])
        .env("PATH", path)
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_millis(750);
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                return Err(unsupported());
            }
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::ErrorKind::TimedOut.into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut output = String::new();
    child
        .stdout
        .take()
        .ok_or_else(unsupported)?
        .take(4097)
        .read_to_string(&mut output)?;
    if output.len() > 4096 || !Path::new(output.trim()).is_absolute() {
        return Err(unsupported());
    }
    std::fs::canonicalize(output.trim())
}

fn is_native(path: &Path) -> io::Result<bool> {
    let mut magic = [0; 4];
    std::fs::File::open(path)?.read_exact(&mut magic)?;
    Ok(&magic == b"\x7fELF")
}

fn is_codex_script(path: &Path) -> bool {
    if path.file_name() != Some(OsStr::new("codex.js")) {
        return false;
    }
    let Some(root) = path.parent().and_then(Path::parent) else {
        return false;
    };
    let read = || -> Option<serde_json::Value> {
        let mut json = String::new();
        std::fs::File::open(root.join("package.json"))
            .ok()?
            .take(65537)
            .read_to_string(&mut json)
            .ok()?;
        if json.len() > 65536 {
            return None;
        }
        serde_json::from_str(&json).ok()
    };
    read().is_some_and(|json| {
        json["name"] == "@openai/codex" && json["bin"]["codex"] == "bin/codex.js"
    })
}

fn codex_binary(script: &Path) -> Option<PathBuf> {
    let root = script.parent()?.parent()?;
    let (package, target) = match std::env::consts::ARCH {
        "x86_64" => ("codex-linux-x64", "x86_64-unknown-linux-musl"),
        "aarch64" => ("codex-linux-arm64", "aarch64-unknown-linux-musl"),
        _ => return None,
    };
    let relative = PathBuf::from("vendor").join(target).join("bin/codex");
    [
        root.join("node_modules/@openai")
            .join(package)
            .join(&relative),
        root.parent()?.join(package).join(&relative),
        root.join(&relative),
    ]
    .into_iter()
    .find_map(|path| {
        std::fs::canonicalize(path)
            .ok()
            .filter(|path| is_native(path).unwrap_or(false))
    })
}

fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "unsupported client executable topology",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trusted_path_discards_empty_and_relative_entries() {
        assert_eq!(
            trusted_path(OsStr::new(".:/usr/bin::relative:/bin")).unwrap(),
            "/usr/bin:/bin"
        );
    }
}
