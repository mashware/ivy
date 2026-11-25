//! Per-console startup files for interactive shells. No user configuration is changed.
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

mod powershell {
    use super::Prepared;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use std::path::Path;

    const SCRIPT: &str =
        include_str!("../../../resources/agent-integrations/codex/shell/powershell.ps1");

    pub(super) fn prepare(helper: Option<&Path>) -> Prepared {
        let helper = helper.filter(|path| path.is_absolute() && path.is_file());
        // D124: inline startup leaves the user's profile execution policy intact.
        let bytes: Vec<u8> = SCRIPT.encode_utf16().flat_map(u16::to_le_bytes).collect();
        Prepared {
            dir: None,
            file: None,
            args: vec![
                "-NoExit".into(),
                "-EncodedCommand".into(),
                STANDARD.encode(bytes),
            ],
            env: vec![(
                "IVY_CODEX_HELPER",
                helper.map(|path| path.as_os_str().to_owned()),
            )],
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn windows_powershell_preparation_recognises_executables_and_keeps_profiles_enabled() {
            for shell in [
                "pwsh.exe",
                "powershell.exe",
                r"C:\Program Files\PowerShell\7\PWSH.EXE",
            ] {
                let prepared = Prepared::prepare(shell, None).unwrap().unwrap();
                assert_eq!(&prepared.args()[..2], ["-NoExit", "-EncodedCommand"]);
                let bytes = STANDARD.decode(&prepared.args()[2]).unwrap();
                let decoded = String::from_utf16(
                    &bytes
                        .chunks_exact(2)
                        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                        .collect::<Vec<_>>(),
                )
                .unwrap();
                assert_eq!(decoded, SCRIPT);
                assert!(prepared.args()[2].len() + shell.len() + 64 < 32_767);
                assert!(prepared.dir.is_none() && prepared.file.is_none());
                assert_eq!(prepared.env(), [("IVY_CODEX_HELPER", None)]);
            }
            let exe = std::env::current_exe().unwrap();
            let prepared = Prepared::prepare("powershell.exe", Some(&exe))
                .unwrap()
                .unwrap();
            assert_eq!(
                prepared.env(),
                [("IVY_CODEX_HELPER", Some(exe.as_os_str()))]
            );
            let prepared = Prepared::prepare("bash.exe", Some(&exe)).unwrap().unwrap();
            assert_eq!(prepared.args()[0], "--rcfile");
            let expected = if cfg!(windows) {
                "ivy-codex.exe"
            } else {
                "ivy-codex"
            };
            assert_eq!(
                super::super::helper_path().unwrap().file_name().unwrap(),
                expected
            );
        }

        #[cfg(windows)]
        #[test]
        #[ignore = "requires the bundled ConPTY pair beside the test binary"]
        fn windows_powershell_prepared_startup_reports_cwd_through_bundled_conpty() {
            use std::time::{Duration, Instant};
            let exe = std::env::current_exe().unwrap();
            for name in ["conpty.dll", "OpenConsole.exe"] {
                assert!(exe.with_file_name(name).is_file(), "missing {name}");
            }
            for program in programs() {
                let root = Temp::new();
                let directory = root.0.join("changed folder");
                std::fs::create_dir(&directory).unwrap();
                let prepared = Prepared::prepare(program.to_str().unwrap(), None)
                    .unwrap()
                    .unwrap();
                let args = prepared
                    .args()
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                let mut env = prepared.env();
                env.push(("IVY_TEST_ROOT", Some(directory.as_os_str())));
                let (mut pty, output) = term_core::Pty::spawn_with_env(
                    program.to_str().unwrap(),
                    &args,
                    term_core::Size::default(),
                    &env,
                )
                .unwrap();
                pty.write(b"Set-Location -LiteralPath $env:IVY_TEST_ROOT\r")
                    .unwrap();
                let mut screen = term_core::Screen::new(term_core::Size::default());
                let deadline = Instant::now() + Duration::from_secs(30);
                let mut seen = String::new();
                let mut answered = 0;
                while screen.working_directory() != Some(directory.as_path()) {
                    let left = deadline.saturating_duration_since(Instant::now());
                    assert!(
                        !left.is_zero(),
                        "no directory report from {program:?}: {seen:?}"
                    );
                    if let Ok(bytes) = output.recv_timeout(left) {
                        screen.advance(&bytes);
                        seen.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    let asked = seen.matches("\x1b[6n").count();
                    for _ in answered..asked {
                        pty.write(b"\x1b[1;1R").unwrap();
                    }
                    answered = answered.max(asked);
                }
            }
        }

        fn programs() -> Vec<PathBuf> {
            #[cfg(windows)]
            {
                let mut paths = vec![
                    PathBuf::from(super::super::windows_cmd(
                        &super::super::WindowsSession::read(),
                    ))
                    .parent()
                    .unwrap()
                    .join("WindowsPowerShell/v1.0/powershell.exe"),
                ];
                let pwsh = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                    .map(|directory| directory.join("pwsh.exe"))
                    .find(|path| path.is_file())
                    .expect("PowerShell 7 is required on Windows; install pwsh and add it to PATH");
                paths.push(pwsh);
                assert!(
                    paths.iter().all(|path| path.is_file()),
                    "Windows PowerShell is required: {paths:?}"
                );
                paths
            }
            #[cfg(not(windows))]
            {
                std::env::var_os("IVY_TEST_PWSH")
                    .map(PathBuf::from)
                    .into_iter()
                    .collect()
            }
        }

        struct Temp(PathBuf);
        impl Temp {
            fn new() -> Self {
                let mut id = [0; 8];
                getrandom::fill(&mut id).unwrap();
                let path = std::env::temp_dir()
                    .join(format!("ivy-powershell-{:x}", u64::from_ne_bytes(id)));
                std::fs::create_dir(&path).unwrap();
                Self(path)
            }
        }
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        fn native_client(root: &Path, name: &str) -> PathBuf {
            let source = root.join("client.rs");
            std::fs::write(
                &source,
                r#"fn main() {
                    let args: Vec<_> = std::env::args().skip(1).collect();
                    println!("NATIVE_CLIENT:{args:?}");
                    std::fs::write(std::path::PathBuf::from(std::env::var_os("IVY_TEST_ROOT").unwrap()).join("native-called"), "called").unwrap();
                }"#,
            )
            .unwrap();
            let target = root.join(name);
            assert!(
                std::process::Command::new("rustc")
                    .arg("--crate-name=ivy_powershell_fixture")
                    .arg(&source)
                    .arg("-o")
                    .arg(&target)
                    .status()
                    .unwrap()
                    .success()
            );
            target
        }

        fn run(
            program: &Path,
            prelude: &str,
            body: &str,
            helper: Option<&Path>,
            root: &Path,
        ) -> (String, String) {
            use std::process::{Command, Stdio};
            use std::time::{Duration, Instant};
            let script = format!("{prelude}\n{SCRIPT}\n{body}");
            let encoded = STANDARD.encode(
                script
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
            );
            let out = root.join("stdout");
            let err = root.join("stderr");
            let mut command = Command::new(program);
            command
                .args(["-NoLogo", "-NoProfile", "-EncodedCommand", &encoded])
                .env("XDG_CACHE_HOME", root)
                .env("XDG_CONFIG_HOME", root)
                .env("XDG_DATA_HOME", root)
                .env("IVY_TEST_ROOT", root)
                .env("TERM", "dumb")
                .stdout(std::fs::File::create(&out).unwrap())
                .stderr(std::fs::File::create(&err).unwrap())
                .stdin(Stdio::null());
            if let Some(helper) = helper {
                command.env("IVY_CODEX_HELPER", helper);
            } else {
                command.env_remove("IVY_CODEX_HELPER");
            }
            let mut child = command.spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "PowerShell did not complete: {}",
                        std::fs::read_to_string(&err).unwrap()
                    );
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            let out = String::from_utf8_lossy(&std::fs::read(out).unwrap()).into_owned();
            let err = String::from_utf8_lossy(&std::fs::read(err).unwrap()).into_owned();
            assert!(status.success(), "{program:?}: {out}\n{err}");
            (out, err)
        }

        #[test]
        fn windows_powershell_reports_encoded_directories_without_a_helper_and_clears_other_providers()
         {
            for program in programs() {
                let root = Temp::new();
                let dir = root.0.join("work % # café");
                std::fs::create_dir(&dir).unwrap();
                let (out, err) = run(
                    &program,
                    "function prompt { 'CUSTOM_PROMPT' }",
                    r#"
Set-Location -LiteralPath (Join-Path $env:IVY_TEST_ROOT 'work % # café')
prompt
'WRAPPED:' + [bool](Get-Command codex -CommandType Alias,Function -ErrorAction Ignore)
Set-Location Env:
prompt
"#,
                    None,
                    &root.0,
                );
                assert!(out.contains("%20%25%20%23%20caf%C3%A9"), "{out}");
                let mut screen = term_core::Screen::new(term_core::Size::default());
                let first_end = out.find("CUSTOM_PROMPT").unwrap();
                screen.advance(&out.as_bytes()[..first_end]);
                assert_eq!(screen.working_directory(), Some(dir.as_path()), "{out}");
                screen.advance(&out.as_bytes()[first_end..]);
                assert_eq!(screen.working_directory(), None, "{out}");
                assert!(out.contains("WRAPPED:False"), "{out}");
                assert!(err.is_empty(), "{err}");
            }
        }

        #[test]
        fn windows_powershell_preserves_prompt_error_status_and_user_codex_commands() {
            for program in programs() {
                for definition in [
                    "function codex { 'USER_CODEX:' + ($args -join ':') }",
                    "function user_codex { 'USER_CODEX:' + ($args -join ':') }; Set-Alias codex user_codex",
                ] {
                    let root = Temp::new();
                    native_client(&root.0, "ivy-codex-shell.exe");
                    let helper = root.0.join("ivy-codex.exe");
                    std::fs::write(&helper, "unused helper").unwrap();
                    let prelude =
                        format!("function prompt {{ \"STATUS:$?:$LASTEXITCODE\" }}; {definition}");
                    let (out, err) = run(
                        &program,
                        &prelude,
                        r#"
$global:LASTEXITCODE = 37
Write-Error 'expected failure' -ErrorAction SilentlyContinue
prompt
codex -V 'image one'
"#,
                        Some(&helper),
                        &root.0,
                    );
                    assert!(out.contains("STATUS:False:37"), "{out}");
                    assert!(out.contains("USER_CODEX:-V:image one"), "{out}");
                    assert!(!root.0.join("native-called").exists(), "{out}");
                    assert!(
                        err.contains("existing codex alias or function preserved"),
                        "{err}"
                    );
                }
            }
        }

        #[test]
        fn windows_powershell_missing_native_sibling_leaves_the_path_client_callable() {
            for program in programs() {
                let root = Temp::new();
                native_client(&root.0, if cfg!(windows) { "codex.exe" } else { "codex" });
                let helper = root.0.join("ivy-codex.exe");
                std::fs::write(&helper, "unused helper").unwrap();
                let (out, err) = run(
                    &program,
                    r#"
function prompt { 'CUSTOM_PROMPT' }
$env:PATH = $env:IVY_TEST_ROOT
"#,
                    r#"
'WRAPPED:' + [bool](Get-Command codex -CommandType Alias,Function -ErrorAction Ignore)
codex -V 'image one'
prompt
"#,
                    Some(&helper),
                    &root.0,
                );
                assert!(out.contains("WRAPPED:False"), "{out}");
                assert!(
                    out.contains(r#"NATIVE_CLIENT:["-V", "image one"]"#),
                    "{out}"
                );
                assert!(root.0.join("native-called").is_file(), "{out}");
                assert!(
                    out.contains("\x1b]7;file://") && out.contains("CUSTOM_PROMPT"),
                    "{out}"
                );
                assert!(err.contains("native Codex launcher unavailable"), "{err}");
            }
        }

        #[test]
        fn windows_powershell_formats_unc_provider_paths_without_network_access() {
            for program in programs() {
                let root = Temp::new();
                let (out, err) = run(
                    &program,
                    "function prompt { 'CUSTOM_PROMPT' }",
                    r#"
function global:Get-Location {
    [pscustomobject]@{
        Provider = [pscustomobject]@{ Name = 'FileSystem' }
        ProviderPath = '\\ivy-remote\share %\café#'
    }
}
prompt
"#,
                    None,
                    &root.0,
                );
                assert_eq!(
                    out,
                    "\x1b]7;file://ivy-remote/share%20%25/caf%C3%A9%23\x1b\\CUSTOM_PROMPT\n"
                        .replace('\n', if cfg!(windows) { "\r\n" } else { "\n" })
                );
                let mut screen = term_core::Screen::new(term_core::Size::default());
                screen.advance(b"\x1b]7;file:///before\x1b\\");
                assert!(screen.working_directory().is_some());
                screen.advance(out.as_bytes());
                assert_eq!(screen.working_directory(), None);
                assert!(err.is_empty(), "{err}");
            }
        }
    }
}

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
    #[cfg(test)]
    if let Some(shell) = TEST_SHELL.with(|shell| shell.borrow().clone()) {
        return shell;
    }
    // Resolved once per process: on Windows it searches the disk, and a console opens on the
    // interface thread.
    static SHELL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SHELL
        .get_or_init(|| {
            #[cfg(windows)]
            return windows_shell(&WindowsSession::read(), Path::is_file);
            #[cfg(not(windows))]
            std::env::var("SHELL").unwrap_or_else(|_| fallback_shell().to_owned())
        })
        .clone()
}

/// The program that is launched when the first one cannot start: one every machine of the platform
/// has. Where it is also what was tried first, the launch does not retry it.
pub(crate) fn fallback_shell() -> &'static str {
    #[cfg(windows)]
    {
        static CMD: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        CMD.get_or_init(|| windows_cmd(&WindowsSession::read()))
    }
    #[cfg(not(windows))]
    "/bin/sh"
}

/// What a Windows session says about its shells, read from its environment.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Default)]
struct WindowsSession {
    shell: Option<OsString>,
    path: Option<OsString>,
    comspec: Option<OsString>,
    system_root: Option<OsString>,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl WindowsSession {
    fn read() -> Self {
        Self {
            shell: std::env::var_os("SHELL"),
            path: std::env::var_os("PATH"),
            comspec: std::env::var_os("ComSpec"),
            system_root: std::env::var_os("SystemRoot"),
        }
    }
}

/// Windows has no `/bin/sh`, and the shell variable a Git Bash or MSYS session exports is a path in
/// that environment's own namespace (`/usr/bin/bash`) that Windows cannot launch, so it counts only
/// as an absolute Windows path to a file. Otherwise the newer PowerShell when it is installed, else
/// the one every Windows ships, else `cmd.exe`, as Windows Terminal picks. Looked up on the
/// session's own search path, never in configuration, and only in absolute directories so the
/// folder a console starts in cannot plant one.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_shell(session: &WindowsSession, is_file: impl Fn(&Path) -> bool) -> String {
    let chosen = session
        .shell
        .as_deref()
        .map(Path::new)
        .filter(|shell| shell.is_absolute() && is_file(shell))
        .map(Path::to_path_buf);
    chosen
        .or_else(|| {
            ["pwsh.exe", "powershell.exe"].into_iter().find_map(|name| {
                session
                    .path
                    .iter()
                    .flat_map(std::env::split_paths)
                    .filter(|directory| directory.is_absolute())
                    .map(|directory| directory.join(name))
                    .find(|candidate| is_file(candidate))
            })
        })
        .map_or_else(
            || windows_cmd(session),
            |found| found.to_string_lossy().into_owned(),
        )
}

/// `cmd.exe` by an absolute path: a bare name would be searched for again by the launcher, current
/// directory and relative entries included.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_cmd(session: &WindowsSession) -> String {
    let absolute = |value: &Option<OsString>| {
        value
            .as_deref()
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    absolute(&session.comspec)
        .or_else(|| {
            absolute(&session.system_root).map(|root| root.join("System32").join("cmd.exe"))
        })
        .map_or_else(
            || r"C:\Windows\System32\cmd.exe".to_owned(),
            |path| path.to_string_lossy().into_owned(),
        )
}

#[cfg(test)]
thread_local! {
    static TEST_SHELL: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Keeps input-routing fixtures independent of user startup files without changing process env.
#[cfg(test)]
pub(crate) struct TestShell(Option<String>);
#[cfg(test)]
impl TestShell {
    pub(crate) fn use_program(program: &str) -> Self {
        Self(TEST_SHELL.with(|shell| shell.replace(Some(program.to_owned()))))
    }
}
#[cfg(test)]
impl Drop for TestShell {
    fn drop(&mut self) {
        TEST_SHELL.with(|shell| *shell.borrow_mut() = self.0.take());
    }
}

pub(crate) fn helper_path() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|p| {
        p.join(if cfg!(windows) {
            "ivy-codex.exe"
        } else {
            "ivy-codex"
        })
    })
}

pub(crate) struct Prepared {
    dir: Option<PathBuf>,
    file: Option<PathBuf>,
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
        let stem = program
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let shell = stem.strip_suffix(".exe").unwrap_or(&stem);
        if matches!(shell, "pwsh" | "powershell") {
            return Ok(Some(powershell::prepare(helper)));
        }
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
        let dir = panel_contract::base_dir().join(format!("ivy-shell-{suffix}"));
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
            dir: Some(dir),
            file: Some(file),
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
        if let Some(file) = &self.file {
            let _ = std::fs::remove_file(file);
        }
        if let Some(dir) = &self.dir {
            let _ = std::fs::remove_dir(dir);
        }
    }
}

#[cfg(test)]
mod default_shell_tests {
    use super::*;

    fn path_of(directories: &[&Path]) -> OsString {
        std::env::join_paths(directories).unwrap()
    }

    fn session(shell: Option<&str>, path: &OsStr) -> WindowsSession {
        let root = std::env::temp_dir().join("Windows");
        WindowsSession {
            shell: shell.map(OsString::from),
            path: Some(path.to_owned()),
            comspec: None,
            system_root: Some(root.into_os_string()),
        }
    }

    fn system_cmd() -> String {
        let root = std::env::temp_dir().join("Windows");
        root.join("System32")
            .join("cmd.exe")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn windows_startup_prefers_pwsh_then_windows_powershell_then_cmd() {
        let base = std::env::temp_dir();
        let (first, second) = (base.join("first"), base.join("second"));
        let path = path_of(&[&first, &second]);
        let pwsh = second.join("pwsh.exe");
        let legacy = first.join("powershell.exe");
        let s = session(None, &path);
        // pwsh wins even when Windows PowerShell sits in an earlier directory.
        let both = windows_shell(&s, |p| p == pwsh || p == legacy);
        assert_eq!(both, pwsh.to_string_lossy());
        assert_eq!(windows_shell(&s, |p| p == legacy), legacy.to_string_lossy());
        assert_eq!(windows_shell(&s, |_| false), system_cmd());
    }

    #[test]
    fn windows_startup_ignores_relative_path_entries() {
        let path = path_of(&[Path::new("planted")]);
        assert_eq!(windows_shell(&session(None, &path), |_| true), system_cmd());
    }

    #[test]
    fn windows_startup_takes_cmd_by_absolute_path_only() {
        let path = path_of(&[]);
        let comspec = std::env::temp_dir().join("cmd.exe");
        let mut s = session(None, &path);
        s.comspec = Some(comspec.clone().into_os_string());
        assert_eq!(windows_cmd(&s), comspec.to_string_lossy());
        s.comspec = Some("cmd.exe".into());
        assert_eq!(windows_cmd(&s), system_cmd());
        s.system_root = Some("Windows".into());
        assert_eq!(windows_cmd(&s), r"C:\Windows\System32\cmd.exe");
    }

    #[test]
    fn windows_startup_keeps_a_launchable_shell_and_skips_a_unix_one() {
        let pwsh = std::env::temp_dir().join("pwsh.exe");
        let path = path_of(&[&std::env::temp_dir()]);
        let bash = std::env::temp_dir().join("bash.exe");
        let named = session(Some(&bash.to_string_lossy()), &path);
        assert_eq!(windows_shell(&named, |_| true), bash.to_string_lossy());
        let msys = session(Some("/usr/bin/bash"), &path);
        assert_eq!(windows_shell(&msys, |p| p == pwsh), pwsh.to_string_lossy());
        let missing = session(Some(&bash.to_string_lossy()), &path);
        assert_eq!(
            windows_shell(&missing, |p| p == pwsh),
            pwsh.to_string_lossy()
        );
    }

    /// Runs one command in a console of `program` and waits for its result, not its echo: ConPTY
    /// writes its own sequences before any program does, so bytes alone prove nothing.
    #[cfg(windows)]
    fn answers(program: &str) {
        let (mut pty, output) =
            term_core::Pty::spawn(program, &[], term_core::Size::default()).unwrap();
        let name = program.to_ascii_lowercase();
        let command: &[u8] = if name.ends_with("cmd.exe") {
            b"echo IVY^_OK\r"
        } else if name.ends_with("pwsh.exe") || name.ends_with("powershell.exe") {
            b"Write-Output ('IVY' + '_OK')\r"
        } else {
            b"echo IVY\"\"_OK\r"
        };
        pty.write(command).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let (mut seen, mut answered) = (String::new(), 0);
        while !seen.contains("IVY_OK") {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(!left.is_zero(), "{program} never ran the command: {seen:?}");
            if let Ok(bytes) = output.recv_timeout(left) {
                seen.push_str(&String::from_utf8_lossy(&bytes));
            }
            // ConPTY asks where the cursor is and waits for the answer, which the window's
            // screen gives in the app; here nobody else would.
            let asked = seen.matches("\x1b[6n").count();
            for _ in answered..asked {
                pty.write(b"\x1b[1;1R").unwrap();
            }
            answered = answered.max(asked);
        }
    }

    /// The reported failure was this spawn: the default program and the fallback both run.
    #[cfg(windows)]
    #[test]
    fn windows_startup_default_and_fallback_run_a_command() {
        answers(&the_users_shell());
        answers(fallback_shell());
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
    fn git_bash_routes_the_native_launcher_instead_of_the_extensionless_npm_shim() {
        let mut random = [0; 8];
        getrandom::fill(&mut random).unwrap();
        let home = std::env::temp_dir().join(format!("ivy-msys-{:x}", u64::from_ne_bytes(random)));
        let bin = home.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(
            home.join(".bashrc"),
            "export PATH=\"$PREPEND_BIN:$PATH\"\nOSTYPE=msys\n",
        )
        .unwrap();
        let helper = home.join("helper");
        executable(&helper, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
        executable(&bin.join("codex"), "#!/bin/sh\necho WRONG_SHIM\n");
        for extension in ["cmd", "exe"] {
            executable(
                &bin.join(format!("codex.{extension}")),
                "#!/bin/sh\nexit 0\n",
            );
            let prepared = Prepared::prepare("bash.exe", Some(&helper))
                .unwrap()
                .unwrap();
            let mut command = Command::new("/bin/bash");
            command
                .args(prepared.args())
                .args(["-i", "-c", "codex resume 'session one'"])
                .env("HOME", &home)
                .env("PREPEND_BIN", &bin)
                .env("TERM", "dumb");
            for (key, value) in prepared.env() {
                command.env(key, value.unwrap());
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                format!(
                    "{}\nresume\nsession one\n",
                    bin.join(format!("codex.{extension}")).display()
                )
            );
        }
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn relative_tmpdir_uses_absolute_shared_base_and_warns_once() {
        const CHILD: &str = "IVY_TEST_RELATIVE_TMPDIR_CHILD";
        if std::env::var_os(CHILD).is_some() {
            for _ in 0..3 {
                assert_eq!(panel_contract::base_dir(), PathBuf::from("/tmp"));
            }
            let prepared = Prepared::prepare_with_zdotdir("bash", Some(Path::new("/bin/sh")), None)
                .unwrap()
                .unwrap();
            assert!(
                prepared.dir.as_ref().unwrap().starts_with("/tmp"),
                "{}",
                prepared.dir.as_ref().unwrap().display()
            );
            return;
        }

        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "shell_integration::tests::relative_tmpdir_uses_absolute_shared_base_and_warns_once",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("TMPDIR", "relative-temp")
            .env_remove("XDG_RUNTIME_DIR")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            stderr
                .matches("TMPDIR set but not an absolute path")
                .count(),
            1,
            "{stderr}"
        );
        assert!(!stderr.contains("XDG_RUNTIME_DIR set"), "{stderr}");
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
                    b"codex 'one space' 'two'\ncodex 'again' 'done'\ncommand codex bypass command\n\"$HOME/bin/codex\" bypass path\necho STILL_ALIVE\nexit\n",
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
                "helper\nclient:pane-a:one space:two\nhelper\nclient:pane-a:again:done\nclient:pane-a:bypass:command\nclient:pane-a:bypass:path\n",
                "{shell}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let dir = prepared.dir.clone().unwrap();
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
        let dir = prepared.dir.clone().unwrap();
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
            for user_definition in [
                "alias codex='echo USER_COMMAND'\n",
                "codex() { echo USER_COMMAND; }\n",
            ] {
                let mut random = [0; 8];
                getrandom::fill(&mut random).unwrap();
                let home = std::env::temp_dir()
                    .join(format!("ivy-conflict-{:x}", u64::from_ne_bytes(random)));
                std::fs::create_dir_all(&home).unwrap();
                if shell.ends_with("bash") {
                    std::fs::write(home.join(".bashrc"), user_definition).unwrap();
                } else if shell.ends_with("zsh") {
                    std::fs::write(home.join(".zshrc"), user_definition).unwrap();
                } else {
                    std::fs::create_dir_all(home.join(".config/fish")).unwrap();
                    std::fs::write(
                        home.join(".config/fish/config.fish"),
                        "function codex; echo USER_COMMAND; end\n",
                    )
                    .unwrap();
                }
                let prepared =
                    Prepared::prepare_with_zdotdir(shell, Some(Path::new("/bin/sh")), None)
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

    #[test]
    fn fish_preserves_newlines_in_executable_path_and_arguments() {
        let fish = std::env::var("IVY_TEST_FISH").unwrap_or_else(|_| "/usr/bin/fish".into());
        if !Path::new(&fish).exists() {
            eprintln!("fish integration test skipped: {fish} unavailable");
            return;
        }
        let mut random = [0; 8];
        getrandom::fill(&mut random).unwrap();
        let home =
            std::env::temp_dir().join(format!("ivy-fish-newline-{:x}", u64::from_ne_bytes(random)));
        let bin = home.join("bin\n`touch sentinel`$(touch sentinel)");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(home.join(".config/fish")).unwrap();
        std::fs::write(
            home.join(".config/fish/config.fish"),
            "set -gx PATH $PREPEND_BIN $PATH\n",
        )
        .unwrap();
        let helper = home.join("helper");
        executable(
            &helper,
            "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$HELPER_LOG\"\nexec \"$@\"\n",
        );
        executable(
            &bin.join("codex"),
            "#!/bin/sh\nprintf '%s\\0' \"$#\" \"$@\" > \"$CLIENT_LOG\"\n",
        );
        let prepared = Prepared::prepare_with_zdotdir(&fish, Some(&helper), None)
            .unwrap()
            .unwrap();
        let mut command = Command::new(&fish);
        command
            .args(prepared.args())
            .arg("-i")
            .current_dir(&home)
            .env("HOME", &home)
            .env("PREPEND_BIN", &bin)
            .env("HELPER_LOG", home.join("helper-log"))
            .env("CLIENT_LOG", home.join("client-log"))
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
            .write_all(b"codex '' 'line\nbreak' 'space value'\nexit\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut expected = bin.join("codex").as_os_str().as_encoded_bytes().to_vec();
        expected.extend_from_slice(b"\0\0line\nbreak\0space value\0");
        assert_eq!(std::fs::read(home.join("helper-log")).unwrap(), expected);
        assert_eq!(
            std::fs::read(home.join("client-log")).unwrap(),
            b"3\0\0line\nbreak\0space value\0"
        );
        assert!(!home.join("sentinel").exists());
        drop(prepared);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn missing_helper_at_preparation_or_runtime_keeps_real_shells_usable() {
        let fish = std::env::var("IVY_TEST_FISH").unwrap_or_else(|_| "/usr/bin/fish".into());
        for shell in ["/bin/bash", "/usr/bin/zsh", fish.as_str()] {
            if !Path::new(shell).exists() {
                eprintln!("shell integration test skipped: {shell} unavailable");
                continue;
            }
            for removed_after_preparation in [false, true] {
                let mut random = [0; 8];
                getrandom::fill(&mut random).unwrap();
                let home = std::env::temp_dir().join(format!(
                    "ivy-missing-helper-{:x}",
                    u64::from_ne_bytes(random)
                ));
                let bin = home.join("bin");
                std::fs::create_dir_all(&bin).unwrap();
                let (rc, contents) = if shell.ends_with("bash") {
                    (home.join(".bashrc"), "export PATH=\"$PREPEND_BIN:$PATH\"\n")
                } else if shell.ends_with("zsh") {
                    (home.join(".zshrc"), "export PATH=\"$PREPEND_BIN:$PATH\"\n")
                } else {
                    std::fs::create_dir_all(home.join(".config/fish")).unwrap();
                    (
                        home.join(".config/fish/config.fish"),
                        "set -gx PATH $PREPEND_BIN $PATH\n",
                    )
                };
                std::fs::write(&rc, contents).unwrap();
                let helper = home.join("helper");
                if removed_after_preparation {
                    executable(&helper, "#!/bin/sh\nexit 99\n");
                }
                let prepared = Prepared::prepare_with_zdotdir(shell, Some(&helper), None).unwrap();
                assert_eq!(prepared.is_some(), removed_after_preparation);
                if removed_after_preparation {
                    std::fs::remove_file(&helper).unwrap();
                }
                executable(
                    &bin.join("codex"),
                    "#!/bin/sh\nprintf '%s\\0' \"$PANEL_ID\" \"$PANEL_DIR\" \"$@\" > \"$LOG\"\nexit 37\n",
                );
                let mut command = Command::new(shell);
                if let Some(prepared) = &prepared {
                    command.args(prepared.args());
                    for (key, value) in prepared.env() {
                        if let Some(value) = value {
                            command.env(key, value);
                        } else {
                            command.env_remove(key);
                        }
                    }
                } else {
                    command.env_remove("ZDOTDIR");
                }
                command
                    .arg("-i")
                    .current_dir(&home)
                    .env("HOME", &home)
                    .env("PREPEND_BIN", &bin)
                    .env("LOG", home.join("log"))
                    .env("PANEL_ID", "fallback pane")
                    .env("PANEL_DIR", "fallback directory")
                    .env("TERM", "dumb")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let mut child = command.spawn().unwrap();
                let script = if shell.ends_with("fish") {
                    b"codex resume 'session one' ''\necho CLIENT_STATUS:$status\necho STILL_ALIVE\nexit\n".as_slice()
                } else {
                    b"codex resume 'session one' ''\necho CLIENT_STATUS:$?\necho STILL_ALIVE\nexit\n"
                        .as_slice()
                };
                child.stdin.take().unwrap().write_all(script).unwrap();
                let output = child.wait_with_output().unwrap();
                let out = String::from_utf8_lossy(&output.stdout);
                let err = String::from_utf8_lossy(&output.stderr);
                assert!(output.status.success(), "{shell}: {err}");
                assert!(out.contains("CLIENT_STATUS:37"), "{shell}: {out}, {err}");
                assert!(out.contains("STILL_ALIVE"), "{shell}: {out}, {err}");
                assert_eq!(
                    std::fs::read(home.join("log")).unwrap(),
                    b"fallback pane\0fallback directory\0resume\0session one\0\0"
                );
                if removed_after_preparation {
                    assert!(err.contains("codex helper unavailable"), "{shell}: {err}");
                }
                assert_eq!(std::fs::read(&rc).unwrap(), contents.as_bytes());
                drop(prepared);
                std::fs::remove_dir_all(home).unwrap();
            }
        }
    }

    #[test]
    fn unsupported_shell_reports_limitation_and_still_runs_commands() {
        const CHILD: &str = "IVY_TEST_UNSUPPORTED_SHELL_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let prepared =
                Prepared::prepare_with_zdotdir("/bin/sh", Some(Path::new("/bin/sh")), None)
                    .unwrap();
            assert!(prepared.is_none());
            let mut command = Command::new("/bin/sh");
            command
                .arg("-i")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = command.spawn().unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"printf '%s\\n' SHELL_USABLE\nexit 0\n")
                .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success());
            assert!(String::from_utf8_lossy(&output.stdout).contains("SHELL_USABLE"));
            return;
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shell_integration::tests::unsupported_shell_reports_limitation_and_still_runs_commands", "--nocapture"])
            .env(CHILD, "1")
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("unsupported shell; automatic Codex panel routing is inactive.")
        );
    }
}

#[cfg(all(test, windows))]
mod tests {
    crate::windows_unix_omission!(
        git_bash_routes_the_native_launcher_instead_of_the_extensionless_npm_shim,
        "unix-only: exercises the MSYS lookup branch using executable POSIX shell fixtures"
    );
    crate::windows_unix_omission!(
        relative_tmpdir_uses_absolute_shared_base_and_warns_once,
        "unix-only: requires executable POSIX shell integration with bash and /bin/sh"
    );
    crate::windows_unix_omission!(
        real_shells_use_post_configuration_path_and_remain_alive,
        "unix-only: starts bash, zsh and fish with POSIX startup files and executable shell hooks"
    );
    crate::windows_unix_omission!(
        zsh_restores_original_directory_and_honors_changes_from_zshenv,
        "unix-only: invokes zsh and relies on its POSIX startup configuration"
    );
    crate::windows_unix_omission!(
        cleanup_never_removes_unknown_contents,
        "unix-only: creates an executable POSIX shell helper using Unix permissions"
    );
    crate::windows_unix_omission!(
        user_commands_win_with_diagnostic,
        "unix-only: creates executable POSIX shell wrappers for command resolution"
    );
    crate::windows_unix_omission!(
        zsh_disabling_rcs_does_not_install_codex_function,
        "unix-only: invokes zsh and inspects its POSIX startup behavior"
    );
    crate::windows_unix_omission!(
        fish_preserves_newlines_in_executable_path_and_arguments,
        "unix-only: invokes fish and creates executable POSIX shell wrappers"
    );
    crate::windows_unix_omission!(
        missing_helper_at_preparation_or_runtime_keeps_real_shells_usable,
        "unix-only: invokes bash, zsh and fish with POSIX helper scripts"
    );
    crate::windows_unix_omission!(
        unsupported_shell_reports_limitation_and_still_runs_commands,
        "unix-only: invokes /bin/sh with POSIX commands"
    );
}
