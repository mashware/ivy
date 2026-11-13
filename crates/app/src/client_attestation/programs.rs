use panel_control::session::Client;
use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
#[cfg(target_os = "linux")]
use std::sync::Arc;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
mod child;

#[cfg(target_os = "linux")]
pub(crate) fn reap_children() {
    child::reap();
}
#[cfg(target_os = "linux")]
pub(crate) fn pending_children() -> (Vec<Arc<OwnedFd>>, bool) {
    child::watched()
}

pub(crate) struct ResolvedLaunch {
    pub(crate) program: PathBuf,
    pub(crate) prefix_args: Vec<String>,
    pub(crate) path: OsString,
}

pub(crate) struct Program {
    pub(crate) client: Client,
    pub(crate) native: PathBuf,
    pub(crate) wrapper: Option<(PathBuf, PathBuf)>,
    #[cfg(test)]
    pub(super) fixture: bool,
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "macos")]
pub(crate) fn resolve_launch(client: Client) -> io::Result<ResolvedLaunch> {
    // macOS recognition is limited to absolute PATH entries and the official Codex
    // package's documented wrapper topology. Volta shims are refused here.
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

pub(crate) fn discover() -> Vec<Program> {
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

pub(crate) fn supplied(programs: Vec<(Client, PathBuf)>) -> Vec<Program> {
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
        #[cfg(target_os = "macos")]
        return Err(unsupported());
        #[cfg(target_os = "linux")]
        {
            let volta =
                std::fs::canonicalize(candidate.parent().ok_or_else(unsupported)?.join("volta"))?;
            if !is_native(&volta)? {
                return Err(unsupported());
            }
            return volta_which(&volta, name, path);
        }
    }
    Ok(canonical)
}

#[cfg(target_os = "linux")]
fn volta_which(volta: &Path, name: &str, path: &OsStr) -> io::Result<PathBuf> {
    let deadline = Instant::now() + Duration::from_millis(750);
    let mut command = Command::new(volta);
    command
        .args(["which", name])
        .env("PATH", path)
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut running = child::Running::start(&mut command)?;
    let mut output = Vec::new();
    let mut status = None;
    let mut ended = false;
    loop {
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if status.is_none() {
            status = running.child.as_mut().unwrap().try_wait()?;
            if status.is_some_and(|status| !status.success()) {
                return Err(unsupported());
            }
        }
        if !ended {
            let mut bytes = [0; 512];
            match running.stdout.read(&mut bytes) {
                Ok(0) => ended = true,
                Ok(count) => {
                    output.extend_from_slice(&bytes[..count]);
                    if output.len() > 4096 {
                        return Err(unsupported());
                    }
                    continue;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        if ended && status.is_some() {
            break;
        }
        let mut watched = [
            libc::pollfd {
                fd: if ended {
                    -1
                } else {
                    running.stdout.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if status.is_none() {
                    running.lifetime.as_ref().unwrap().as_raw_fd()
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let timeout = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(750) as i32;
        // SAFETY: the pipe and pidfd remain owned by running; the shared deadline bounds poll.
        if unsafe { libc::poll(watched.as_mut_ptr(), watched.len() as libc::nfds_t, timeout) } < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
    let output = std::str::from_utf8(&output).map_err(|_| unsupported())?;
    if !Path::new(output.trim()).is_absolute() {
        return Err(unsupported());
    }
    let path = std::fs::canonicalize(output.trim())?;
    if Instant::now() >= deadline {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(path)
}

fn is_native(path: &Path) -> io::Result<bool> {
    let mut header = [0; 8];
    std::fs::File::open(path)?.read_exact(&mut header)?;
    #[cfg(target_os = "linux")]
    {
        Ok(&header[..4] == b"\x7fELF")
    }
    #[cfg(target_os = "macos")]
    {
        // Require a well-formed native 64-bit Mach-O slice. A magic prefix alone would
        // also accept 32-bit or foreign-architecture files as client roots.
        const X86_64: u32 = 0x0100_0007;
        const ARM64: u32 = 0x0100_000c;
        let wanted = match std::env::consts::ARCH {
            "x86_64" => X86_64,
            "aarch64" => ARM64,
            _ => return Ok(false),
        };
        let u32_at = |bytes: &[u8], little: bool| {
            let array: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
            Some(if little {
                u32::from_le_bytes(array)
            } else {
                u32::from_be_bytes(array)
            })
        };
        let magic = &header[..4];
        if magic == [0xcf, 0xfa, 0xed, 0xfe] || magic == [0xfe, 0xed, 0xfa, 0xcf] {
            let little = magic[0] == 0xcf;
            return Ok(u32_at(&header[4..8], little) == Some(wanted));
        }
        // Universal 32-bit or 64-bit headers. Limit slice count and inspect every
        // architecture record without trusting offsets or loading unbounded data.
        let (little, wide) = match magic {
            [0xca, 0xfe, 0xba, 0xbe] => (false, false),
            [0xbe, 0xba, 0xfe, 0xca] => (true, false),
            [0xca, 0xfe, 0xba, 0xbf] => (false, true),
            [0xbf, 0xba, 0xfe, 0xca] => (true, true),
            _ => return Ok(false),
        };
        let count = u32_at(&header[4..8], little).unwrap_or(0);
        if count == 0 || count > 64 {
            return Ok(false);
        }
        let record = if wide { 32 } else { 20 };
        let mut file = std::fs::File::open(path)?;
        use std::io::{Seek, SeekFrom};
        for index in 0..count {
            let offset = 8 + u64::from(index) * record;
            file.seek(SeekFrom::Start(offset))?;
            let mut cpu = [0; 4];
            file.read_exact(&mut cpu)?;
            if u32_at(&cpu, little) == Some(wanted) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = header;
        Ok(false)
    }
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
    #[cfg(target_os = "linux")]
    let (package, target) = match std::env::consts::ARCH {
        "x86_64" => ("codex-linux-x64", "x86_64-unknown-linux-musl"),
        "aarch64" => ("codex-linux-arm64", "aarch64-unknown-linux-musl"),
        _ => return None,
    };
    #[cfg(target_os = "macos")]
    let (package, target) = match std::env::consts::ARCH {
        "x86_64" => ("codex-darwin-x64", "x86_64-apple-darwin"),
        "aarch64" => ("codex-darwin-arm64", "aarch64-apple-darwin"),
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
