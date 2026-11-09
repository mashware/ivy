//! Coordinates independent windows without transferring ownership during their lifetime.

use std::fs::{self, File, OpenOptions, TryLockError};
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
        coordinator.lock()?;

        let mut primary = true;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.starts_with("lease-") {
                continue;
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(entry.path())?;
            match file.try_lock() {
                Ok(()) => {
                    drop(file);
                    fs::remove_file(entry.path())?;
                }
                Err(TryLockError::WouldBlock) => primary = false,
                Err(TryLockError::Error(error)) => return Err(error),
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
        lease.lock()?;
        // Dropping the coordinator releases the startup lock only after the lease is held.
        coordinator.unlock()?;
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
        // A concurrent fork can inherit this open-file description until exec, despite CLOEXEC.
        // Explicit unlock ends the window lease now rather than when that child's duplicate closes.
        let _ = self._lease.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::process::{Child, ChildStdin, Command, Stdio};

    #[test]
    fn closing_owner_releases_an_inherited_open_file_description() {
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).unwrap();
        let directory = std::env::temp_dir().join(format!(
            "ivy-ownership-inherited-{:x}",
            u64::from_le_bytes(random)
        ));
        let owner = Ownership::acquire(&directory).unwrap();
        let inherited = owner._lease.try_clone().unwrap();
        drop(owner);
        let replacement = Ownership::acquire(&directory).unwrap();
        assert!(
            replacement.is_primary(),
            "a duplicate inherited during fork must not keep a closed window alive"
        );
        drop(replacement);
        drop(inherited);
        std::fs::remove_dir_all(directory).unwrap();
    }

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

    struct Window {
        child: Child,
        input: ChildStdin,
        output: BufReader<std::process::ChildStdout>,
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
