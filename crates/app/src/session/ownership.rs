//! Coordinates independent windows without transferring ownership during their lifetime.

#[cfg(not(unix))]
use std::fs::TryLockError;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::Path;

/// Holds this window's operating-system lease until it is dropped.
pub(crate) struct Ownership {
    _lease: File,
    primary: bool,
}

impl Ownership {
    /// Acquire a lease in the application's configuration directory.
    /// Any failure leaves ownership undecided; callers must disable persistence.
    pub(crate) fn acquire(config_dir: &Path) -> io::Result<Self> {
        let directory = config_dir.join("window-leases");
        super::create_directory(&directory)?;
        let coordinator = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("coordinator"))?;
        lock(&coordinator, true)?;

        let mut primary = true;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.starts_with("lease-") {
                continue;
            }
            // Unix probes need only read access, including for readable leases left by root.
            let file = OpenOptions::new()
                .read(true)
                .write(cfg!(not(unix)))
                .open(entry.path())?;
            match try_lock(&file)? {
                true => {
                    drop(file);
                    fs::remove_file(entry.path())?;
                }
                false => primary = false,
            }
        }

        let lease = loop {
            let mut random = [0u8; 16];
            getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
            let name = random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(directory.join(format!("lease-{name}")))
            {
                Ok(file) => break file,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };
        lock(&lease, true)?;
        // Dropping the coordinator releases the startup lock only after the lease is held.
        unlock(&coordinator)?;
        drop(coordinator);
        Ok(Self {
            _lease: lease,
            primary,
        })
    }

    pub(crate) fn is_primary(&self) -> bool {
        self.primary
    }
}

impl Drop for Ownership {
    fn drop(&mut self) {
        // Release the lease before closing its handle.
        let _ = unlock(&self._lease);
    }
}

// POSIX record locks belong to the process and are not inherited across fork. A crash
// therefore releases the lease even if a child has inherited its descriptor before exec.
#[cfg(unix)]
fn set_lock(file: &File, kind: libc::c_short, wait: bool) -> io::Result<bool> {
    use std::os::fd::AsRawFd;
    let mut request: libc::flock = unsafe { std::mem::zeroed() };
    request.l_type = kind;
    request.l_whence = libc::SEEK_SET as libc::c_short;
    loop {
        let result = unsafe {
            libc::fcntl(
                file.as_raw_fd(),
                if wait { libc::F_SETLKW } else { libc::F_SETLK },
                &request,
            )
        };
        if result != -1 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        if error.kind() == ErrorKind::Interrupted {
            continue;
        }
        if !wait
            && matches!(
                error.raw_os_error(),
                Some(libc::EACCES) | Some(libc::EAGAIN)
            )
        {
            return Ok(false);
        }
        return Err(error);
    }
}

#[cfg(unix)]
fn lock(file: &File, wait: bool) -> io::Result<()> {
    set_lock(file, libc::F_WRLCK as libc::c_short, wait).map(|_| ())
}
#[cfg(unix)]
fn try_lock(file: &File) -> io::Result<bool> {
    // Every live owner holds an exclusive lock, so a read lock still detects it.
    set_lock(file, libc::F_RDLCK as libc::c_short, false)
}
#[cfg(unix)]
fn unlock(file: &File) -> io::Result<()> {
    set_lock(file, libc::F_UNLCK as libc::c_short, false).map(|_| ())
}

#[cfg(not(unix))]
fn lock(file: &File, _wait: bool) -> io::Result<()> {
    file.lock()
}
#[cfg(not(unix))]
fn try_lock(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(TryLockError::WouldBlock) => Ok(false),
        Err(TryLockError::Error(error)) => Err(error),
    }
}
#[cfg(not(unix))]
fn unlock(file: &File) -> io::Result<()> {
    file.unlock()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::process::{Child, ChildStdin, Command, Stdio};

    #[test]
    fn ownership_child() {
        let Ok(directory) = std::env::var("IVY_OWNERSHIP_TEST_DIRECTORY") else {
            return;
        };
        let mut gate = [0];
        io::stdin().read_exact(&mut gate).unwrap();
        let ownership = Ownership::acquire(Path::new(&directory)).unwrap();
        println!("OWNERSHIP_ROLE={}", ownership.is_primary());
        io::stdout().flush().unwrap();
        io::stdin().read_exact(&mut gate).unwrap();
    }

    crate::windows_unix_omission!(
        fork_owner_child,
        "unix-only: forks a child process to verify owner lease lifetime"
    );

    #[cfg(unix)]
    #[test]

    fn fork_owner_child() {
        let Ok(directory) = std::env::var("IVY_FORK_TEST_DIRECTORY") else {
            return;
        };
        let owner = Ownership::acquire(Path::new(&directory)).unwrap();
        assert!(owner.is_primary());
        // The child performs only async-signal-safe operations before it exits.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            unsafe {
                libc::pause();
                libc::_exit(0)
            }
        }
        println!("FORK_CHILD={pid}");
        io::stdout().flush().unwrap();
        let mut gate = [0];
        let _ = io::stdin().read_exact(&mut gate);
    }

    crate::windows_unix_omission!(
        crashed_owner_releases_lease_while_fork_child_lives,
        "unix-only: forks a child and signals its parent to test owner lease release"
    );

    #[cfg(unix)]
    #[test]

    fn crashed_owner_releases_lease_while_fork_child_lives() {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let directory = std::env::temp_dir().join(format!("ivy-ownership-fork-{:?}", random));
        let mut owner = Command::new(std::env::current_exe().unwrap())
            .args(["fork_owner_child", "--nocapture"])
            .env("IVY_FORK_TEST_DIRECTORY", &directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(owner.stdout.take().unwrap());
        let child_pid: libc::pid_t = loop {
            let mut line = String::new();
            assert_ne!(output.read_line(&mut line).unwrap(), 0);
            if let Some(pid) = line.split("FORK_CHILD=").nth(1) {
                break pid.trim().parse().unwrap();
            }
        };
        owner.kill().unwrap();
        owner.wait().unwrap();
        let mut replacement = Window::spawn(&directory);
        replacement.start();
        let primary = replacement.role();
        unsafe {
            libc::kill(child_pid, libc::SIGKILL);
        }
        replacement.close();
        fs::remove_dir_all(directory).unwrap();
        assert!(
            primary,
            "a forked child must not retain its crashed parent's lease"
        );
    }

    struct Window {
        child: Child,
        input: ChildStdin,
        output: BufReader<std::process::ChildStdout>,
    }

    impl Drop for Window {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    impl Window {
        fn spawn(directory: &Path) -> Self {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["ownership_child", "--nocapture"])
                .env("IVY_OWNERSHIP_TEST_DIRECTORY", directory)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let input = child.stdin.take().unwrap();
            let output = BufReader::new(child.stdout.take().unwrap());
            Self {
                child,
                input,
                output,
            }
        }

        fn start(&mut self) {
            self.input.write_all(b"x").unwrap();
        }

        fn role(&mut self) -> bool {
            loop {
                let mut line = String::new();
                assert_ne!(self.output.read_line(&mut line).unwrap(), 0, "child exited");
                if let Some(role) = line.split("OWNERSHIP_ROLE=").nth(1) {
                    return role.trim() == "true";
                }
            }
        }

        fn close(mut self) {
            self.input.write_all(b"x").unwrap();
            assert!(self.child.wait().unwrap().success());
        }

        fn crash(mut self) {
            self.child.kill().unwrap();
            self.child.wait().unwrap();
        }
    }

    crate::windows_unix_omission!(
        readable_non_writable_stale_lease_does_not_block_startup,
        "unix-only: POSIX permissions and process-owned record locks"
    );

    #[cfg(unix)]
    #[test]
    fn readable_non_writable_stale_lease_does_not_block_startup() {
        use std::os::unix::fs::PermissionsExt;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let directory = std::env::temp_dir().join(format!("ivy-stale-lease-{:?}", random));
        let leases = directory.join("window-leases");
        fs::create_dir_all(&leases).unwrap();
        let stale = leases.join("lease-stale");
        fs::write(&stale, b"").unwrap();
        fs::set_permissions(&stale, fs::Permissions::from_mode(0o444)).unwrap();

        let result = Ownership::acquire(&directory);
        let removed = !stale.exists();
        fs::remove_dir_all(&directory).unwrap();
        assert!(result.unwrap().is_primary());
        assert!(removed, "an unlocked obsolete lease must be cleaned up");
    }

    crate::windows_unix_omission!(
        readable_non_writable_live_lease_is_preserved_until_owner_exits,
        "unix-only: POSIX permissions and process-owned record locks"
    );

    #[cfg(unix)]
    #[test]
    fn readable_non_writable_live_lease_is_preserved_until_owner_exits() {
        use std::os::unix::fs::PermissionsExt;
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let directory = std::env::temp_dir().join(format!("ivy-live-lease-{:?}", random));
        fs::create_dir(&directory).unwrap();
        let mut primary = Window::spawn(&directory);
        primary.start();
        assert!(primary.role());
        let lease = fs::read_dir(directory.join("window-leases"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("lease-")
            })
            .unwrap();
        fs::set_permissions(&lease, fs::Permissions::from_mode(0o444)).unwrap();

        let mut secondary = Window::spawn(&directory);
        secondary.start();
        assert!(
            !secondary.role(),
            "a read-only lease still has a live exclusive owner"
        );
        assert!(lease.exists(), "a live lease must not be removed");
        assert_eq!(
            fs::metadata(&lease).unwrap().permissions().mode() & 0o777,
            0o444
        );
        secondary.close();
        primary.close();

        let mut replacement = Window::spawn(&directory);
        replacement.start();
        assert!(replacement.role());
        assert!(
            !lease.exists(),
            "the same lease becomes removable after its owner exits"
        );
        replacement.close();
        fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn independent_processes_keep_roles_until_all_exit() {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let directory = std::env::temp_dir().join(format!("ivy-ownership-{:?}", random));
        fs::create_dir(&directory).unwrap();

        let mut first = Window::spawn(&directory);
        let mut second = Window::spawn(&directory);
        first.start();
        second.start();
        let first_role = first.role();
        let second_role = second.role();
        assert_ne!(first_role, second_role);
        let (primary, secondary) = if first_role {
            (first, second)
        } else {
            (second, first)
        };
        primary.close();
        let mut third = Window::spawn(&directory);
        third.start();
        assert!(!third.role());
        secondary.close();
        third.crash();

        let mut primary = Window::spawn(&directory);
        primary.start();
        assert!(primary.role());
        let mut secondary = Window::spawn(&directory);
        secondary.start();
        assert!(!secondary.role());
        secondary.close();
        let mut another = Window::spawn(&directory);
        another.start();
        assert!(!another.role());
        primary.crash();
        another.close();
        let mut last = Window::spawn(&directory);
        last.start();
        assert!(last.role());
        last.close();
        fs::remove_dir_all(directory).unwrap();
    }
}
