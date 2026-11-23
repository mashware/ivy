//! Native attribution for the persistent shared server, not for shells or the remote TUI.

use std::collections::BTreeMap;
use std::ffi::{CString, OsString};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

type Disclaim = unsafe extern "C" fn(*mut libc::posix_spawnattr_t, bool) -> libc::c_int;

struct Attributes(libc::posix_spawnattr_t);

impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: only successfully initialized attributes are owned here.
        unsafe { libc::posix_spawnattr_destroy(&mut self.0) };
    }
}

struct Prepared {
    attributes: Attributes,
    executables: Vec<CString>,
    _arguments: Vec<CString>,
    _environment: Vec<CString>,
    argv: Vec<*mut libc::c_char>,
    envp: Vec<*mut libc::c_char>,
}

// SAFETY: the arrays point only into owned, immutable CString buffers. Moving their
// owner does not move those buffers. The fork child's hook only reads this state;
// attribute destruction happens in the parent after spawn has finished.
unsafe impl Send for Prepared {}
unsafe impl Sync for Prepared {}

impl Prepared {
    fn new(command: &Command) -> io::Result<Self> {
        // Resolve and allocate before fork: the hook must not acquire loader or allocator locks.
        let symbol = unsafe {
            libc::dlsym(
                libc::RTLD_DEFAULT,
                c"responsibility_spawnattrs_setdisclaim".as_ptr(),
            )
        };
        if symbol.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "macOS cannot separate the shared Codex service from Ivy's application lifetime",
            ));
        }
        // SAFETY: this is the libSystem spawn-attribute SPI used by LLVM and Chromium.
        let disclaim: Disclaim = unsafe { std::mem::transmute(symbol) };
        let mut raw = std::ptr::null_mut();
        errno(unsafe { libc::posix_spawnattr_init(&mut raw) })?;
        let mut attributes = Attributes(raw);
        errno(unsafe {
            libc::posix_spawnattr_setflags(&mut attributes.0, libc::POSIX_SPAWN_SETEXEC as i16)
        })?;
        errno(unsafe { disclaim(&mut attributes.0, true) })?;

        let arguments: Vec<_> = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|value| CString::new(value.as_bytes()))
            .collect::<Result<_, _>>()
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        // This private caller uses inherited environment plus explicit overrides/removals,
        // never env_clear. Command's overrides are not installed in environ before pre_exec.
        let mut environment: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
        for (key, value) in command.get_envs() {
            match value {
                Some(value) => {
                    environment.insert(key.to_owned(), value.to_owned());
                }
                None => {
                    environment.remove(key);
                }
            }
        }
        let executables = executable_candidates(command, &environment)?
            .into_iter()
            .map(|path| {
                CString::new(path.as_os_str().as_bytes())
                    .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
            })
            .collect::<io::Result<_>>()?;
        let environment: Vec<_> = environment
            .into_iter()
            .map(|(key, value)| {
                let mut entry = key.as_bytes().to_vec();
                entry.push(b'=');
                entry.extend_from_slice(value.as_bytes());
                CString::new(entry).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
            })
            .collect::<io::Result<_>>()?;
        let pointers = |strings: &Vec<CString>| {
            strings
                .iter()
                .map(|value| value.as_ptr().cast_mut())
                .chain(std::iter::once(std::ptr::null_mut()))
                .collect()
        };
        Ok(Self {
            attributes,
            executables,
            argv: pointers(&arguments),
            envp: pointers(&environment),
            _arguments: arguments,
            _environment: environment,
        })
    }

    fn exec(&self) -> io::Result<()> {
        // SETEXEC keeps std's child PID, prepared stdio/cwd/signals and error pipe.
        // Successful replacement never returns. Nothing in this hook allocates or locks.
        let mut last = libc::ENOENT;
        let mut denied = false;
        for executable in &self.executables {
            let result = unsafe {
                libc::posix_spawn(
                    std::ptr::null_mut(),
                    executable.as_ptr(),
                    std::ptr::null(),
                    &self.attributes.0,
                    self.argv.as_ptr(),
                    self.envp.as_ptr(),
                )
            };
            match result {
                libc::EACCES => denied = true,
                libc::ENOENT | libc::ENOTDIR => {}
                other => {
                    return Err(io::Error::from_raw_os_error(if other == 0 {
                        libc::EIO
                    } else {
                        other
                    }));
                }
            }
            last = result;
        }
        Err(io::Error::from_raw_os_error(if denied {
            libc::EACCES
        } else {
            last
        }))
    }
}

fn executable_candidates(
    command: &Command,
    environment: &BTreeMap<OsString, OsString>,
) -> io::Result<Vec<PathBuf>> {
    let program = Path::new(command.get_program());
    if program.is_absolute() {
        return Ok(vec![program.to_owned()]);
    }
    let mut directory = std::env::current_dir()?;
    if let Some(cwd) = command.get_current_dir() {
        directory = directory.join(cwd);
    }
    if program.as_os_str().as_bytes().contains(&b'/') {
        return Ok(vec![directory.join(program)]);
    }
    // posix_spawnp reads PATH through getenv, which may lock after fork. Prepare
    // absolute candidates here; let exec, not mode bits, decide access to each one.
    let path = environment
        .get(std::ffi::OsStr::new("PATH"))
        .map(OsString::as_os_str)
        .unwrap_or(std::ffi::OsStr::new("/usr/bin:/bin"));
    Ok(std::env::split_paths(path)
        .map(|entry| directory.join(entry).join(program))
        .collect())
}

fn errno(result: libc::c_int) -> io::Result<()> {
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(result))
    }
}

pub(super) fn spawn(mut command: Command) -> io::Result<Child> {
    let prepared = Prepared::new(&command)?;
    // SAFETY: preparation owns all storage and attributes before fork; the hook only
    // performs the native exec replacement and constructs an errno on failure.
    unsafe {
        command.pre_exec(move || prepared.exec());
    }
    command.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    const PROBE: &str = "IVY_NATIVE_DAEMON_PROBE";

    #[test]
    fn native_probe() {
        let Some(path) = std::env::var_os(PROBE) else {
            return;
        };
        let symbol = unsafe {
            libc::dlsym(
                libc::RTLD_DEFAULT,
                c"responsibility_get_pid_responsible_for_pid".as_ptr(),
            )
        };
        assert!(
            !symbol.is_null(),
            "native responsibility query is unavailable"
        );
        let query: unsafe extern "C" fn(libc::pid_t) -> libc::pid_t =
            unsafe { std::mem::transmute(symbol) };
        let responsible = unsafe { query(libc::getpid()) };
        let mut input = Vec::new();
        use std::io::Read;
        std::io::stdin().read_to_end(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "daemon command must not inherit terminal input"
        );
        let record = serde_json::json!({
            "pid": std::process::id(), "responsible": responsible,
            "kept": std::env::var("IVY_PROBE_KEPT").unwrap(),
            "removed": std::env::var_os("PATH"),
            "cwd": std::env::current_dir().unwrap(),
        });
        std::fs::write(path, record.to_string()).unwrap();
        println!("native daemon output");
        std::process::exit(37);
    }

    fn command(path: &std::path::Path) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "codex_relay::macos_daemon::tests::native_probe",
                "--nocapture",
            ])
            .env(PROBE, path)
            .env("IVY_PROBE_KEPT", "value with spaces")
            .env_remove("PATH")
            .current_dir(path.parent().unwrap())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn finish(mut child: Child) -> std::process::Output {
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                let _ = child.kill();
                panic!("native probe did not finish within its deadline");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    }

    #[test]
    fn daemon_start_changes_effective_responsibility_without_changing_io_environment_or_status() {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let directory = std::env::temp_dir().join(format!("ivy-native-daemon-{:x?}", random));
        std::fs::create_dir(&directory).unwrap();
        let directory = std::fs::canonicalize(directory).unwrap();
        let normal = directory.join("normal.json");
        let corrected = directory.join("corrected.json");
        for (path, separate) in [(&normal, false), (&corrected, true)] {
            let mut cmd = command(path);
            let child = if separate {
                spawn(cmd).unwrap()
            } else {
                cmd.spawn().unwrap()
            };
            let output = finish(child);
            assert_eq!(
                output.status.code(),
                Some(37),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("native daemon output"));
        }
        let read = |path: PathBuf| {
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap()).unwrap()
        };
        let normal = read(normal);
        let corrected = read(corrected);
        // Keep the original launcher alive: orphan auto-disclamation must not hide the mutation.
        assert_ne!(
            normal["responsible"], normal["pid"],
            "control already disclaimed; regression oracle cannot distinguish the launch"
        );
        assert_eq!(corrected["responsible"], corrected["pid"]);
        assert_eq!(corrected["kept"], "value with spaces");
        assert_eq!(corrected["removed"], serde_json::Value::Null);
        assert_eq!(corrected["cwd"], serde_json::json!(directory));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_executable_reports_spawn_failure() {
        let missing =
            std::env::temp_dir().join(format!("ivy-absent-executable-{}", std::process::id()));
        let failure = spawn(Command::new(missing)).unwrap_err();
        assert_eq!(failure.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn native_path_search_skips_an_inaccessible_candidate_and_keeps_the_later_programs_io() {
        // Root may execute the first file through its group execute bit, so it cannot
        // supply the EACCES control this macOS PATH regression needs.
        if unsafe { libc::geteuid() } == 0 {
            eprintln!("not executed: native PATH EACCES witness requires a non-root user");
            return;
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let directory = std::env::temp_dir().join(format!("ivy-native-path-{:x?}", random));
        let denied = directory.join("denied");
        let usable = directory.join("usable");
        std::fs::create_dir_all(&denied).unwrap();
        std::fs::create_dir(&usable).unwrap();
        let program = "codex probe";
        std::fs::write(denied.join(program), "#!/bin/sh\nprintf 'wrong program'\n").unwrap();
        // The fixture belongs to us: owner execute is denied even though the file
        // has a group execute bit. Checking whether any execute bit exists is wrong.
        std::fs::set_permissions(denied.join(program), std::fs::Permissions::from_mode(0o610))
            .unwrap();
        std::fs::write(
            usable.join(program),
            "#!/bin/sh\nprintf 'usable:%s' \"$1\"\nprintf 'native diagnostic' >&2\nexit 37\n",
        )
        .unwrap();
        std::fs::set_permissions(usable.join(program), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let path = std::env::join_paths([&denied, &usable]).unwrap();
        let make_command = || {
            let mut command = Command::new(program);
            command
                .arg("argument with spaces")
                .env("PATH", &path)
                .current_dir(&directory)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            command
        };
        // Establish the ordinary Command behavior before exercising the adapter.
        let control = finish(make_command().spawn().unwrap());
        assert_eq!(control.status.code(), Some(37));
        assert_eq!(control.stdout, b"usable:argument with spaces");
        assert_eq!(control.stderr, b"native diagnostic");
        let corrected = finish(spawn(make_command()).unwrap());
        assert_eq!(corrected.status.code(), control.status.code());
        assert_eq!(corrected.stdout, control.stdout);
        assert_eq!(corrected.stderr, control.stderr);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
