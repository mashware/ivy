use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Map, Value};

const SETTINGS_LIMIT: u64 = 1024 * 1024;
const VERSION: &str = "2.1.332 (Claude Code)";

#[derive(PartialEq, Eq)]
pub(super) struct Preview {
    pub(super) directory: PathBuf,
    pub(super) program: PathBuf,
    pub(super) path: OsString,
    helper: PathBuf,
    statusline: Option<Map<String, Value>>,
    program_identity: (u64, u64, u64, i64, i64),
}

impl Preview {
    pub(super) fn read(directory: &Path) -> io::Result<Self> {
        let directory = directory.canonicalize()?;
        if !directory.is_dir() {
            return Err(io::Error::other("Choose a local directory."));
        }
        let resolved =
            crate::client_attestation::resolve_launch(panel_control::session::Client::Claude)
                .map_err(|_| {
                    io::Error::other("No supported native Claude Code executable was found.")
                })?;
        if !resolved.prefix_args.is_empty() {
            return Err(io::Error::other(
                "This Claude Code launcher is unsupported.",
            ));
        }
        let helper = crate::resume::helper_path()
            .ok_or_else(|| io::Error::other("The Ivy session helper is unavailable."))?;
        let statusline = effective_statusline(&directory)?;
        let program_identity = identity(&resolved.program)?;
        verify_version(&resolved.program, &resolved.path)?;
        if identity(&resolved.program)? != program_identity {
            return Err(io::Error::other(
                "Claude Code changed during setup. Preview it again.",
            ));
        }
        Ok(Self {
            directory,
            program: resolved.program,
            path: resolved.path,
            helper,
            statusline,
            program_identity,
        })
    }

    pub(super) fn command(&self) -> Option<&str> {
        self.statusline.as_ref()?.get("command")?.as_str()
    }

    pub(super) fn options(&self) -> impl Iterator<Item = (&'static str, &Value)> {
        [
            ("Padding", "padding"),
            ("Refresh interval (seconds)", "refreshInterval"),
            ("Hide Vim mode indicator", "hideVimModeIndicator"),
        ]
        .into_iter()
        .filter_map(|(label, option)| Some((label, self.statusline.as_ref()?.get(option)?)))
    }

    pub(super) fn overlay(&self) -> io::Result<String> {
        let helper = self
            .helper
            .to_str()
            .ok_or_else(|| io::Error::other("Ivy helper path is not UTF-8."))?;
        let mut command = format!("exec {} quota-statusline", shell_word(helper));
        if let Some(original) = self.command() {
            command.push(' ');
            command.push_str(&shell_word(original));
        }
        Ok(serde_json::json!({
            "statusLine": { "type": "command", "command": command }
        })
        .to_string())
    }

    pub(super) fn recheck(&self) -> io::Result<()> {
        if identity(&self.program)? != self.program_identity
            || effective_statusline(&self.directory)? != self.statusline
            || crate::resume::helper_path().as_ref() != Some(&self.helper)
        {
            return Err(io::Error::other(
                "Claude Code or its settings changed. Preview the launch again.",
            ));
        }
        Ok(())
    }
}

fn shell_word(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn identity(path: &Path) -> io::Result<(u64, u64, u64, i64, i64)> {
    let metadata = fs::metadata(path)?;
    Ok((
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    ))
}

fn verify_version(program: &Path, path: &OsStr) -> io::Result<()> {
    let mut command = Command::new(program);
    command
        .arg("--version")
        .env("PATH", path)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped());
    let output = crate::git_process::run(command, Duration::from_secs(2), 256, 1024)
        .map_err(|_| io::Error::other("Claude Code did not report its version."))?;
    if output.status.success()
        && std::str::from_utf8(&output.stdout).is_ok_and(|version| version.trim() == VERSION)
    {
        return Ok(());
    }
    Err(io::Error::other(
        "This Claude Code version has not been verified for allowance reporting.",
    ))
}

fn effective_statusline(directory: &Path) -> io::Result<Option<Map<String, Value>>> {
    let profile = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|home| home.join(".claude")))
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::other("Claude Code's settings directory is unavailable."))?;

    for entry in ["managed-settings.json", "managed-settings.d"] {
        let path = Path::new("/etc/claude-code").join(entry);
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(io::Error::other(
                    "Managed Claude Code installations are unavailable for allowance reporting.",
                ));
            }
            Err(error) => return Err(error),
        }
    }

    statusline_for_profile(directory, &profile)
}

fn statusline_for_profile(
    directory: &Path,
    profile: &Path,
) -> io::Result<Option<Map<String, Value>>> {
    let user_settings = profile.join("settings.json");
    let project = directory.join(".claude");
    let local = project.join("settings.local.json");
    // The installed client chose the main checkout's local command over the linked checkout's.
    let root_local =
        project_local_root(directory)?.map(|root| root.join(".claude/settings.local.json"));
    let mut statusline = read_statusline(&user_settings)?;
    let project_settings = project.join("settings.json");
    for source in [
        Some(&project_settings),
        Some(&local),
        root_local.as_ref().filter(|path| **path != local),
    ]
    .into_iter()
    .flatten()
    .filter(|path| **path != user_settings)
    {
        if let Some(project) = read_statusline(source)? {
            statusline.get_or_insert_with(Map::new).extend(project);
        }
    }
    if let Some(statusline) = &statusline
        && (statusline.get("type").and_then(Value::as_str) != Some("command")
            || !statusline
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty() && !value.contains('\0')))
    {
        return Err(io::Error::other(
            "Claude Code has an unsupported statusline command.",
        ));
    }
    Ok(statusline)
}

fn project_local_root(directory: &Path) -> io::Result<Option<PathBuf>> {
    let mut checkout = None;
    for ancestor in directory.ancestors() {
        if ancestor.join(".git").try_exists()? {
            checkout = Some(ancestor);
            break;
        }
    }
    let Some(checkout) = checkout else {
        return Ok(None);
    };
    let identity = crate::project::resolve(directory)
        .map_err(|_| io::Error::other("Could not resolve the Claude project settings scope."))?;
    let crate::project::Key::Repository(common) = identity.key else {
        return Err(io::Error::other(
            "Could not resolve the Claude project settings scope.",
        ));
    };
    let Some(main) = common
        .parent()
        .filter(|_| common.file_name().is_some_and(|name| name == ".git"))
    else {
        return Err(io::Error::other(
            "This Git repository uses an unverified Claude settings scope.",
        ));
    };
    if std::env::home_dir().as_deref() == Some(main)
        || !owned(main)?
        || !owned(&common)?
        || !owned(checkout)?
        || !owned(&checkout.join(".git"))?
    {
        return Ok(None);
    }
    let config = main.join(".claude");
    match fs::symlink_metadata(&config) {
        Ok(_) if !owned(&config)? => return Ok(None),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(Some(main.to_owned()))
}

fn owned(path: &Path) -> io::Result<bool> {
    // SAFETY: querying the process's effective user has no preconditions.
    Ok(fs::symlink_metadata(path)?.uid() == unsafe { libc::geteuid() })
}

fn read_statusline(path: &Path) -> io::Result<Option<Map<String, Value>>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) if file.metadata()?.is_file() => file,
        Ok(_) => {
            return Err(io::Error::other(
                "Claude Code settings are not a regular file.",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    let settings = read_object(file)?;
    if settings.get("disableAllHooks").and_then(Value::as_bool) == Some(true)
        || settings
            .get("allowManagedHooksOnly")
            .and_then(Value::as_bool)
            == Some(true)
    {
        return Err(io::Error::other(
            "Claude Code policy prevents an Ivy statusline.",
        ));
    }
    let Some(statusline) = settings.get("statusLine") else {
        return Ok(None);
    };
    let Some(statusline) = statusline.as_object() else {
        return Err(io::Error::other(
            "Claude Code has an unsupported statusline configuration.",
        ));
    };
    Ok(Some(statusline.clone()))
}

fn read_object(file: File) -> io::Result<Map<String, Value>> {
    let mut data = Vec::new();
    file.take(SETTINGS_LIMIT + 1).read_to_end(&mut data)?;
    if data.len() as u64 > SETTINGS_LIMIT {
        return Err(io::Error::other(
            "Claude Code settings exceed the setup size limit.",
        ));
    }
    serde_json::from_slice(&data)
        .map_err(|_| io::Error::other("Claude Code settings are not a supported JSON object."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_uses_project_precedence_and_keeps_inherited_options() {
        let directory = panel_contract::PanelDir::create().unwrap();
        let profile = directory.path().join(".claude");
        let project = directory.path().join("project");
        let project_settings = project.join(".claude");
        fs::create_dir_all(&profile).unwrap();
        fs::create_dir_all(&project_settings).unwrap();
        let templates = directory.path().join("empty-templates");
        fs::create_dir(&templates).unwrap();
        // Give this precedence fixture its own scope, independent of ancestor Git markers.
        let mut git = Command::new("git");
        for (key, _) in std::env::vars_os() {
            if key
                .to_string_lossy()
                .to_ascii_uppercase()
                .starts_with("GIT_")
            {
                git.env_remove(key);
            }
        }
        let initialized = git
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .arg("-C")
            .arg(&project)
            .args(["init", "--quiet", "--initial-branch=main", "--template"])
            .arg(&templates)
            .output()
            .unwrap();
        assert!(
            initialized.status.success(),
            "{}",
            String::from_utf8_lossy(&initialized.stderr)
        );
        fs::write(
            profile.join("settings.json"),
            r#"{"statusLine":{"type":"command","command":"user","refreshInterval":1}}"#,
        )
        .unwrap();
        fs::write(
            project_settings.join("settings.json"),
            r#"{"statusLine":{"type":"command","command":"project","padding":2}}"#,
        )
        .unwrap();
        fs::write(
            project_settings.join("settings.local.json"),
            r#"{"statusLine":{"type":"command","command":"local"}}"#,
        )
        .unwrap();
        let result = statusline_for_profile(&project, &profile).unwrap().unwrap();
        assert_eq!(result["command"], "local");
        assert_eq!(result["refreshInterval"], 1);
        assert_eq!(result["padding"], 2);
        fs::remove_dir_all(project).unwrap();
        fs::remove_dir_all(profile).unwrap();
        fs::remove_dir(templates).unwrap();
    }

    #[test]
    fn command_is_one_shell_argument_even_with_embedded_quotes_and_lines() {
        let command = "printf '%s\\n' \"$(echo never run)\"\nnext";
        let output = Command::new("/bin/sh")
            .args(["-c", &format!("printf '%s' {}", shell_word(command))])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, command.as_bytes());
    }

    #[test]
    fn linked_checkout_preserves_the_main_checkouts_local_statusline() {
        let directory = panel_contract::PanelDir::create().unwrap();
        let main = directory.path().join("main");
        let linked = directory.path().join("linked");
        fs::create_dir(&main).unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .arg("-C")
                .arg(&main)
                .args([
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        git(&["commit", "--allow-empty", "--quiet", "-m", "Fixture"]);
        git(&[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "linked",
            linked.to_str().unwrap(),
        ]);
        for (checkout, command) in [(&main, "main statusline"), (&linked, "linked statusline")] {
            fs::create_dir(checkout.join(".claude")).unwrap();
            fs::write(
                checkout.join(".claude/settings.local.json"),
                serde_json::json!({"statusLine":{"type":"command","command":command}}).to_string(),
            )
            .unwrap();
        }
        let statusline = statusline_for_profile(&linked, &directory.path().join("profile"))
            .unwrap()
            .unwrap();
        assert_eq!(statusline["command"], "main statusline");
        fs::remove_dir_all(linked).unwrap();
        fs::remove_dir_all(main).unwrap();
    }
}
