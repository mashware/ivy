use super::*;
use std::ffi::OsString;
use std::path::Component;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Client {
    #[default]
    Claude,
    Codex,
    OpenCode,
}

impl Client {
    pub(crate) fn args(self) -> &'static [&'static str] {
        match self {
            Self::Codex => &["--no-daemon"],
            _ => &[],
        }
    }
}

#[derive(Clone, Default, Debug)]
pub(crate) struct Selection {
    pub client: Client,
    pub images: [String; 3],
    pub directory: String,
}

impl Selection {
    fn selected(&self) -> &str {
        &self.images[match self.client {
            Client::Claude => 0,
            Client::Codex => 1,
            Client::OpenCode => 2,
        }]
    }
}

fn local_absolute(path: &Path) -> bool {
    use std::path::Prefix;
    matches!(path.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        && path.is_absolute()
}

fn validate_image(path: &Path) -> term_core::Result<PathBuf> {
    if !local_absolute(path)
        || !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        return Err("Select an absolute local native .exe file.".into());
    }
    let canonical = std::fs::canonicalize(path)?;
    let name = canonical
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "cmd.exe"
            | "powershell.exe"
            | "pwsh.exe"
            | "python.exe"
            | "pythonw.exe"
            | "node.exe"
            | "wscript.exe"
            | "cscript.exe"
            | "bash.exe"
            | "sh.exe"
            | "deno.exe"
            | "bun.exe"
    ) {
        return Err("Interpreter executables are not supported as agent roots.".into());
    }
    if !local_absolute(&canonical) || !canonical.is_file() {
        return Err("The selected executable is not a local file.".into());
    }
    Ok(canonical)
}

pub(crate) fn start(
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    reads: crate::control::ReadActivity,
    selection: Selection,
    size: Size,
    env: Vec<(String, Option<OsString>)>,
) -> term_core::Result<(Process, Receiver<Vec<u8>>)> {
    start_prepared_worker(wake, reads, move || {
        let program = selection.selected().to_owned();
        let operation = || -> term_core::Result<Ready> {
            let selected = validate_image(Path::new(selection.selected()))?;
            let mut images = Vec::new();
            for image in selection.images.iter().filter(|s| !s.trim().is_empty()) {
                images.push(validate_image(Path::new(image))?);
            }
            let directory = Path::new(&selection.directory);
            if !local_absolute(directory) {
                return Err("Select an absolute local working directory.".into());
            }
            let canonical_directory = std::fs::canonicalize(directory)?;
            if !local_absolute(&canonical_directory) || !canonical_directory.is_dir() {
                return Err("The working directory is not a local directory.".into());
            }
            if selection.client == Client::Codex
                && !panel_control::codex_relay::supports_local_launch(selected.as_os_str())?
            {
                return Err("This Codex executable does not advertise --no-daemon.".into());
            }
            let program = selection.selected();
            let borrowed: Vec<_> = env
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_deref()))
                .collect();
            let spawned = Pty::spawn_with_exact_cwd(
                program,
                selection.client.args(),
                size,
                &borrowed,
                directory,
            )?;
            let invocation = spawned
                .0
                .foreground_scope()?
                .fresh_invocation(&selected, &images)?;
            let mut ready = prepare(spawned);
            ready.invocation = Some(invocation);
            Ok(ready)
        };
        operation().map_err(|source| -> term_core::Error {
            Box::new(StartupFailure {
                program,
                reason: source.to_string(),
                source,
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_clients_supply_only_the_disclosed_fixed_arguments() {
        assert_eq!(Client::Claude.args(), &[] as &[&str]);
        assert_eq!(Client::OpenCode.args(), &[] as &[&str]);
        assert_eq!(Client::Codex.args(), &["--no-daemon"]);
    }

    #[test]
    fn local_paths_reject_remote_and_device_names() {
        assert!(local_absolute(Path::new(r"C:\work\client.exe")));
        for path in [
            r"relative.exe",
            r"\\server\share\client.exe",
            r"\\?\UNC\server\share\client.exe",
            r"\\.\device",
        ] {
            assert!(!local_absolute(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn script_and_interpreter_roots_are_refused() {
        for path in [
            r"C:\work\agent.cmd",
            r"C:\work\agent.ps1",
            r"C:\work\agent.bat",
        ] {
            assert!(validate_image(Path::new(path)).is_err());
        }
        let system = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap();
        assert!(validate_image(&system.join("System32").join("cmd.exe")).is_err());
    }
}
