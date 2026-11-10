//! Per-console startup files for interactive shells. No user configuration is changed.
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

const BASH: &str = include_str!("../../../resources/agent-integrations/codex/shell/bash");
const ZSH: &str = include_str!("../../../resources/agent-integrations/codex/shell/zsh");
const FISH: &str = include_str!("../../../resources/agent-integrations/codex/shell/fish");

/// The program that opens a console: the shell of whoever launched the application.
///
/// **It is what a terminal does**: without this a shell opens with no prompt and no history, and
/// this project exists to look at what comes out on screen. It is still not outside input —it is
/// the user's own session—, but the day the program comes from **configuration** or from a file
/// from outside, this line becomes sensitive surface and has to be reviewed as such. That is why
/// saved labels and directories **never supply a program**: there is nothing
/// to connect here.
///
/// **It has been in a function of its own since restoring exists**, and not for pleasure: restoring
/// opens N consoles at once, and reading the environment inside each one would read it a hundred
/// and twenty-eight times in the worst case.
pub(crate) fn the_users_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| crate::FALLBACK_SHELL.to_owned())
}

pub(crate) fn helper_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.join("ivy-codex"))
}

pub(crate) struct Prepared {
    dir: PathBuf,
    file: PathBuf,
    args: Vec<String>,
    env: Vec<(&'static str, Option<OsString>)>,
}

impl Prepared {
    pub(crate) fn prepare(program: &str, helper: Option<&Path>) -> io::Result<Option<Self>> {
        Self::prepare_with_zdotdir(program, helper, std::env::var_os("ZDOTDIR"))
    }

    fn prepare_with_zdotdir(
        program: &str,
        helper: Option<&Path>,
        original_zdotdir: Option<OsString>,
    ) -> io::Result<Option<Self>> {
        let shell = Path::new(program)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("");
        let (name, script) = match shell {
            "bash" => ("bashrc", BASH),
            "zsh" => (".zshenv", ZSH),
            "fish" => ("fish-init", FISH),
            _ => {
                eprintln!("Ivy: unsupported shell; automatic Codex panel routing is inactive.");
                return Ok(None);
            }
        };
        let Some(helper) = helper.filter(|p| p.is_absolute() && p.is_file()) else {
            eprintln!("Ivy: codex helper unavailable; automatic panel routing is inactive.");
            return Ok(None);
        };
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ivy-shell-{suffix}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir(&dir)?;
        let file = dir.join(name);
        if let Err(error) = std::fs::write(&file, script) {
            let _ = std::fs::remove_file(&file);
            let _ = std::fs::remove_dir(&dir);
            return Err(error);
        }
        let mut env = vec![("IVY_CODEX_HELPER", Some(helper.as_os_str().to_owned()))];
        let args = match shell {
            "bash" => vec!["--rcfile".into(), file.to_string_lossy().into_owned()],
            "zsh" => {
                let original = original_zdotdir;
                env.push((
                    "IVY_ZDOTDIR_WAS_SET",
                    Some(if original.is_some() { "1" } else { "0" }.into()),
                ));
                env.push(("IVY_ORIGINAL_ZDOTDIR", original));
                env.push(("ZDOTDIR", Some(dir.as_os_str().to_owned())));
                vec![]
            }
            "fish" => {
                env.push(("IVY_FISH_INIT", Some(file.as_os_str().to_owned())));
                vec!["--init-command".into(), "source \"$IVY_FISH_INIT\"".into()]
            }
            _ => unreachable!(),
        };
        Ok(Some(Self {
            dir,
            file,
            args,
            env,
        }))
    }
    pub(crate) fn args(&self) -> &[String] {
        &self.args
    }
    pub(crate) fn env(&self) -> Vec<(&str, Option<&OsStr>)> {
        self.env
            .iter()
            .map(|(name, value)| (*name, value.as_deref()))
            .collect()
    }
}
impl Drop for Prepared {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
        let _ = std::fs::remove_dir(&self.dir);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    fn executable(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn real_shells_use_post_configuration_path_and_remain_alive() {
        let fish = std::env::var("IVY_TEST_FISH").unwrap_or_else(|_| "/usr/bin/fish".into());
        for shell in ["/bin/bash", "/usr/bin/zsh", fish.as_str()] {
            if !Path::new(shell).exists() {
                eprintln!("shell integration test skipped: {shell} unavailable");
                continue;
            }
            let mut random = [0; 8];
            getrandom::fill(&mut random).unwrap();
            let home = std::env::temp_dir().join(format!(
                "ivy test ' $(touch sentinel) {:x}",
                u64::from_ne_bytes(random)
            ));
            let bin = home.join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            let helper = home.join("helper ' $()");
            executable(
                &helper,
                "#!/bin/sh\nprintf 'helper\\n' >> \"$LOG\"\nexec \"$@\"\n",
            );
            executable(
                &bin.join("codex"),
                "#!/bin/sh\nprintf 'client:%s:%s:%s\\n' \"$PANEL_ID\" \"$1\" \"$2\" >> \"$LOG\"\n",
            );
            let setup = "export PATH=\"$PREPEND_BIN:$PATH\"\n";
            if shell.ends_with("bash") {
                std::fs::write(home.join(".bashrc"), setup).unwrap();
            } else if shell.ends_with("zsh") {
                std::fs::write(home.join(".zshrc"), setup).unwrap();
            } else {
                std::fs::create_dir_all(home.join(".config/fish")).unwrap();
                std::fs::write(
                    home.join(".config/fish/config.fish"),
                    "set -gx PATH $PREPEND_BIN $PATH\n",
                )
                .unwrap();
            }
            let prepared = Prepared::prepare_with_zdotdir(shell, Some(&helper), None)
                .unwrap()
                .unwrap();
            let mut command = Command::new(shell);
            command
                .args(prepared.args())
                .arg("-i")
                .env("HOME", &home)
                .env("PREPEND_BIN", &bin)
                .env("LOG", home.join("log"))
                .env("PANEL_ID", "pane-a")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            for (key, value) in prepared.env() {
                match value {
                    Some(value) => {
                        command.env(key, value);
                    }
                    None => {
                        command.env_remove(key);
                    }
                }
            }
            let mut child = command.spawn().unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(
                    b"codex 'one space' 'two'\ncodex 'again' 'done'\necho STILL_ALIVE\nexit\n",
                )
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{shell}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("STILL_ALIVE"));
            assert_eq!(
                std::fs::read_to_string(home.join("log")).unwrap(),
                "helper\nclient:pane-a:one space:two\nhelper\nclient:pane-a:again:done\n",
                "{shell}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let dir = prepared.dir.clone();
            drop(prepared);
            assert!(!dir.exists());
            std::fs::remove_dir_all(home).unwrap();
        }
    }

    #[test]
    fn zsh_restores_original_directory_and_honors_changes_from_zshenv() {
        if !Path::new("/usr/bin/zsh").exists() {
            eprintln!("zsh unavailable");
            return;
        }
        for mode in ["unset", "empty", "present", "changed"] {
            let mut random = [0; 8];
            getrandom::fill(&mut random).unwrap();
            let home =
                std::env::temp_dir().join(format!("ivy-zsh-{:x}", u64::from_ne_bytes(random)));
            let original = home.join("original");
            let changed = home.join("changed");
            std::fs::create_dir_all(&original).unwrap();
            std::fs::create_dir_all(&changed).unwrap();
            let helper = home.join("helper");
            executable(&helper, "#!/bin/sh\nexec \"$@\"\n");
            let initial = if mode == "unset" || mode == "empty" {
                &home
            } else {
                &original
            };
            if mode == "changed" {
                std::fs::write(
                    initial.join(".zshenv"),
                    "export ZDOTDIR=\"$TEST_CHANGED\"\n",
                )
                .unwrap();
            }
            let expected = if mode == "changed" { &changed } else { initial };
            std::fs::write(expected.join(".zshrc"), "echo USER_RC\n").unwrap();
            let inherited = if mode == "unset" {
                None
            } else if mode == "empty" {
                Some(OsString::new())
            } else {
                Some(original.as_os_str().to_owned())
            };
            let prepared = Prepared::prepare_with_zdotdir("/usr/bin/zsh", Some(&helper), inherited)
                .unwrap()
                .unwrap();
            let mut cmd = Command::new("/usr/bin/zsh");
            cmd.arg("-i")
                .current_dir(&home)
                .env("HOME", &home)
                .env("TEST_CHANGED", &changed)
                .env("TERM", "dumb")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            for (key, value) in prepared.env() {
                if let Some(value) = value {
                    cmd.env(key, value);
                } else {
                    cmd.env_remove(key);
                }
            }
            let mut child = cmd.spawn().unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"echo RESULT:${ZDOTDIR-unset}\nexit\n")
                .unwrap();
            let output = child.wait_with_output().unwrap();
            let out = String::from_utf8_lossy(&output.stdout);
            if mode == "empty" {
                assert!(!out.contains("USER_RC"), "{mode}: {out}");
            } else {
                assert!(out.contains("USER_RC"), "{mode}: {out}");
            }
            let value = if mode == "unset" {
                "unset".to_owned()
            } else if mode == "empty" {
                String::new()
            } else {
                expected.to_string_lossy().into_owned()
            };
            assert!(out.contains(&format!("RESULT:{value}")), "{mode}: {out}");
            drop(prepared);
            std::fs::remove_dir_all(home).unwrap();
        }
    }

    #[test]
    fn cleanup_never_removes_unknown_contents() {
        let prepared = Prepared::prepare_with_zdotdir("bash", Some(Path::new("/bin/sh")), None)
            .unwrap()
            .unwrap();
        let dir = prepared.dir.clone();
        std::fs::write(dir.join("foreign"), "kept").unwrap();
        drop(prepared);
        assert_eq!(
            std::fs::read_to_string(dir.join("foreign")).unwrap(),
            "kept"
        );
        std::fs::remove_file(dir.join("foreign")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn user_commands_win_with_diagnostic() {
        let fish = std::env::var("IVY_TEST_FISH").unwrap_or_else(|_| "/usr/bin/fish".into());
        for shell in ["/bin/bash", "/usr/bin/zsh", fish.as_str()] {
            if !Path::new(shell).exists() {
                eprintln!("shell unavailable: {shell}");
                continue;
            }
            let mut random = [0; 8];
            getrandom::fill(&mut random).unwrap();
            let home =
                std::env::temp_dir().join(format!("ivy-conflict-{:x}", u64::from_ne_bytes(random)));
            std::fs::create_dir_all(&home).unwrap();
            if shell.ends_with("bash") {
                std::fs::write(home.join(".bashrc"), "alias codex='echo USER_COMMAND'\n").unwrap();
            } else if shell.ends_with("zsh") {
                std::fs::write(home.join(".zshrc"), "alias codex='echo USER_COMMAND'\n").unwrap();
            } else {
                std::fs::create_dir_all(home.join(".config/fish")).unwrap();
                std::fs::write(
                    home.join(".config/fish/config.fish"),
                    "function codex; echo USER_COMMAND; end\n",
                )
                .unwrap();
            }
            let prepared = Prepared::prepare_with_zdotdir(shell, Some(Path::new("/bin/sh")), None)
                .unwrap()
                .unwrap();
            let mut command = Command::new(shell);
            command
                .args(prepared.args())
                .arg("-i")
                .env("HOME", &home)
                .env("TERM", "dumb")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            for (key, value) in prepared.env() {
                if let Some(value) = value {
                    command.env(key, value);
                } else {
                    command.env_remove(key);
                }
            }
            let mut child = command.spawn().unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"codex\nexit\n")
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("USER_COMMAND"),
                "{shell}"
            );
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("existing codex alias or function preserved"),
                "{shell}"
            );
            drop(prepared);
            std::fs::remove_dir_all(home).unwrap();
        }
    }

    #[test]
    fn zsh_disabling_rcs_does_not_install_codex_function() {
        if !Path::new("/usr/bin/zsh").exists() {
            eprintln!("zsh unavailable");
            return;
        }
        let mut random = [0; 8];
        getrandom::fill(&mut random).unwrap();
        let home =
            std::env::temp_dir().join(format!("ivy-zsh-rcs-{:x}", u64::from_ne_bytes(random)));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".zshenv"), "unsetopt rcs\n").unwrap();
        let prepared =
            Prepared::prepare_with_zdotdir("/usr/bin/zsh", Some(Path::new("/bin/sh")), None)
                .unwrap()
                .unwrap();
        let mut command = Command::new("/usr/bin/zsh");
        command
            .arg("-i")
            .current_dir(&home)
            .env("HOME", &home)
            .env("TERM", "dumb")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in prepared.env() {
            if let Some(value) = value {
                command.env(key, value);
            } else {
                command.env_remove(key);
            }
        }
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                b"(( ${+functions[codex]} )) && echo INSTALLED || echo NOT_INSTALLED\nexit\n",
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("NOT_INSTALLED"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("zsh startup files disabled"));
        drop(prepared);
        std::fs::remove_dir_all(home).unwrap();
    }
}
