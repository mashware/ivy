//! Re-exec gate: no user code runs until the parent has settled scope placement.
use crate::{Pty, Result, Size};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::os::unix::{
    fs::DirBuilderExt,
    net::{UnixListener, UnixStream},
    process::CommandExt,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

const MODE: &str = "--ivy-private-scope-helper-v1";
const STARTUP: Duration = Duration::from_secs(1);
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Controlled placement outcomes; available only to hosts' development tests.
#[cfg(any(test, feature = "testing"))]
#[derive(Clone, Copy, Debug)]
pub enum TestScopePlacement {
    /// Release the actual helper without involving a systemd manager.
    Ready,
    /// Reject placement before release and exercise the direct fallback.
    Denied,
    /// Keep the actual gate closed until the one-second startup deadline.
    Stalled,
}

struct Gate {
    directory: PathBuf,
    listener: UnixListener,
}
impl Gate {
    fn new() -> io::Result<Self> {
        for _ in 0..32 {
            let mut random = [0u8; 16];
            std::fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
            let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
            let directory = std::env::temp_dir().join(format!("ivy-scope-{token}"));
            match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => match UnixListener::bind(directory.join("gate")) {
                    Ok(listener) => {
                        let gate = Self {
                            directory,
                            listener,
                        };
                        gate.listener.set_nonblocking(true)?;
                        return Ok(gate);
                    }
                    Err(error) => {
                        let _ = std::fs::remove_dir(&directory);
                        return Err(error);
                    }
                },
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::other("could not reserve launcher socket"))
    }
    fn connect(&self, deadline: Instant) -> io::Result<UnixStream> {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let left = remaining(deadline)?;
                    stream.set_read_timeout(Some(left))?;
                    stream.set_write_timeout(Some(left))?;
                    return Ok(stream);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                    // Bounded by the shared startup deadline.
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(e) => return Err(e),
            }
        }
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.directory.join("gate"));
        let _ = std::fs::remove_dir(&self.directory);
    }
}
fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "launcher deadline expired"))
}

impl Pty {
    /// Opts into Linux isolation using a host executable supporting [`crate::dispatch_scope_helper`].
    /// Run this complete operation on a launch worker; manager setup shares a one-second deadline.
    pub fn spawn_with_launcher_in(
        launcher: &Path,
        identity: &str,
        program: &str,
        args: &[&str],
        size: Size,
        env: &[(&str, Option<&OsStr>)],
        directory: Option<&Path>,
    ) -> Result<(Self, Receiver<Vec<u8>>)> {
        Self::spawn_gated_in(
            launcher,
            identity,
            program,
            args,
            size,
            env,
            directory,
            |pid, name, identity, deadline| {
                crate::scope::place(pid, name, identity, deadline).map(Some)
            },
        )
    }

    /// Exercises the real helper protocol with controlled manager outcomes in integration tests.
    #[cfg(any(test, feature = "testing"))]
    #[allow(clippy::too_many_arguments)] // Keep the production launch arguments intact in tests.
    pub fn spawn_with_test_launcher_in(
        launcher: &Path,
        identity: &str,
        program: &str,
        args: &[&str],
        size: Size,
        env: &[(&str, Option<&OsStr>)],
        directory: Option<&Path>,
        placement: TestScopePlacement,
    ) -> Result<(Self, Receiver<Vec<u8>>)> {
        Self::spawn_gated_in(
            launcher,
            identity,
            program,
            args,
            size,
            env,
            directory,
            move |_, _, _, deadline| match placement {
                TestScopePlacement::Ready => Ok(None),
                TestScopePlacement::Denied => Err("injected manager denial".into()),
                TestScopePlacement::Stalled => {
                    if let Some(left) = deadline.checked_duration_since(Instant::now()) {
                        // Bounded by the same production startup deadline; no user work is released.
                        std::thread::sleep(left);
                    }
                    Err("injected manager timeout".into())
                }
            },
        )
    }

    #[allow(clippy::too_many_arguments)] // One implementation shares the existing spawn contract.
    fn spawn_gated_in(
        launcher: &Path,
        identity: &str,
        program: &str,
        args: &[&str],
        size: Size,
        env: &[(&str, Option<&OsStr>)],
        directory: Option<&Path>,
        place: impl FnOnce(u32, &str, &str, Instant) -> Result<Option<crate::scope::Scope>>,
    ) -> Result<(Self, Receiver<Vec<u8>>)> {
        if !launcher.is_file() {
            return Self::spawn_with_env_in(program, args, size, env, directory);
        }
        if program.trim().is_empty() {
            return Err("no program to launch: the name is empty".into());
        }
        let deadline = Instant::now() + STARTUP;
        let gate = match Gate::new() {
            Ok(gate) => gate,
            Err(_) => return Self::spawn_with_env_in(program, args, size, env, directory),
        };
        // The current public spawn interface uses UTF-8 program names; the private argv preserves
        // OS strings for socket paths. Use a temporary owned command adapter for those entries.
        let mut helper_args = vec![
            OsString::from(MODE),
            gate.directory.join("gate").into_os_string(),
            OsString::from(program),
        ];
        helper_args.extend(args.iter().map(OsString::from));
        let (mut pty, output) =
            Self::spawn_os_with_env_in(launcher.as_os_str(), &helper_args, size, env, directory)?;
        let mut released = false;
        let handshake = (|| -> Result<_> {
            let mut stream = gate.connect(deadline)?;
            let mut ready = [0u8; 1];
            stream.read_exact(&mut ready)?;
            if ready != [1] {
                return Err("unexpected launcher handshake".into());
            }
            let pid = pty
                .child
                .as_ref()
                .and_then(|c| c.process_id())
                .ok_or("launcher has no process ID")?;
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let namespace = gate.directory.file_name().unwrap().to_string_lossy();
            let name = format!(
                "ivy-console-{}-{sequence}.scope",
                namespace.trim_start_matches("ivy-scope-")
            );
            let scope = place(pid, &name, identity, deadline)?;
            stream.set_read_timeout(Some(remaining(deadline)?))?;
            stream.set_write_timeout(Some(remaining(deadline)?))?;
            released = true;
            stream.write_all(&[2])?;
            let mut reply = [0u8; 5];
            match stream.read(&mut reply[..1])? {
                0 => Ok(scope), // CLOEXEC closes only after the authenticated release stage.
                _ if reply[0] == 3 => {
                    stream.read_exact(&mut reply[1..])?;
                    Err(io::Error::from_raw_os_error(i32::from_ne_bytes(
                        reply[1..].try_into().unwrap(),
                    ))
                    .into())
                }
                _ => Err("unexpected launcher exec response".into()),
            }
        })();
        match handshake {
            Ok(scope) => {
                pty.scope_stop = scope.map(|scope| scope.observe(pty.termination.clone()));
                Ok((pty, output))
            }
            Err(error) => {
                if !released {
                    eprintln!("Ivy console isolation unavailable: {error}");
                }
                // Before release a failed manager transaction cannot have run user code. Kill this
                // gated PID before direct fallback, so delayed manager work can never move user work.
                // After release, retrying could execute user code twice: preserve the launch error.
                drop(pty);
                drop(output);
                if released {
                    Err(error)
                } else {
                    Self::spawn_with_env_in(program, args, size, env, directory)
                }
            }
        }
    }
}

pub(crate) fn dispatch() -> Option<i32> {
    let mut args = std::env::args_os();
    args.next();
    if args.next().as_deref() != Some(OsStr::new(MODE)) {
        return None;
    }
    Some(helper(args).unwrap_or(126))
}
fn helper(mut args: impl Iterator<Item = OsString>) -> io::Result<i32> {
    let socket = args
        .next()
        .ok_or_else(|| io::Error::other("missing gate"))?;
    let program = args
        .next()
        .ok_or_else(|| io::Error::other("missing program"))?;
    let arguments: Vec<_> = args.collect();
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(STARTUP))?;
    stream.set_write_timeout(Some(STARTUP))?;
    // UnixStream is CLOEXEC by construction; successful exec is the sole successful EOF stage.
    stream.write_all(&[1])?;
    let mut release = [0u8; 1];
    stream.read_exact(&mut release)?;
    if release != [2] {
        return Err(io::Error::other("invalid gate release"));
    }
    let error = match resolve(&program) {
        Ok(resolved) => std::process::Command::new(resolved)
            .arg0(&program)
            .args(arguments)
            .exec(),
        Err(error) => error,
    };
    let mut reply = [3u8; 5];
    reply[1..].copy_from_slice(&error.raw_os_error().unwrap_or(libc::EIO).to_ne_bytes());
    stream.write_all(&reply)?;
    Ok(127)
}
// portable-pty 0.9 resolves bare relative names against each PATH entry and explicit ./ or ../
// against the selected cwd; no libc default PATH when PATH was removed, and argv[0] stays original.
fn resolve(program: &OsStr) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let executable = |path: &Path| {
        let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        !path.is_dir() && unsafe { libc::access(c.as_ptr(), libc::X_OK) == 0 }
    };
    let path = Path::new(program);
    if path.is_absolute()
        || program.as_bytes().starts_with(b"./")
        || program.as_bytes().starts_with(b"../")
        || program == "."
        || program == ".."
    {
        if executable(path) {
            return Ok(path.to_path_buf());
        }
        return Err(io::Error::from_raw_os_error(if path.exists() {
            libc::EACCES
        } else {
            libc::ENOENT
        }));
    }
    let mut denied = false;
    if let Some(path) = std::env::var_os("PATH") {
        for entry in std::env::split_paths(&path) {
            let candidate = std::env::current_dir()?.join(entry).join(program);
            if executable(&candidate) {
                return Ok(candidate);
            }
            denied |= candidate.exists();
        }
    }
    Err(io::Error::from_raw_os_error(if denied {
        libc::EACCES
    } else {
        libc::ENOENT
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn gate_is_private_and_removes_its_endpoint_on_drop() {
        let gate = Gate::new().unwrap();
        let directory = gate.directory.clone();
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(directory.join("gate").exists());
        drop(gate);
        assert!(!directory.exists());
    }
    #[test]
    fn absent_helper_connection_obeys_startup_deadline() {
        let gate = Gate::new().unwrap();
        let started = Instant::now();
        assert!(gate.connect(started + Duration::from_millis(10)).is_err());
        assert!(started.elapsed() < STARTUP);
    }
    #[test]
    fn explicit_executable_resolution_preserves_relative_path_and_refuses_directories() {
        assert_eq!(
            resolve(OsStr::new("/bin/sh")).unwrap(),
            Path::new("/bin/sh")
        );
        assert_eq!(
            resolve(OsStr::new(".")).unwrap_err().raw_os_error(),
            Some(libc::EACCES)
        );
        assert_eq!(
            resolve(OsStr::new("/ivy-nonexistent-124-program"))
                .unwrap_err()
                .raw_os_error(),
            Some(libc::ENOENT)
        );
    }
}
