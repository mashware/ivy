use crate::confined::{self, Binding};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[cfg(windows)]
pub(crate) mod windows;

const CONSENT_BYTES: u64 = 64 * 1024;
const MAX_ROOTS: usize = 64;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct Consent {
    pub enabled: bool,
    pub roots: Vec<Binding>,
}

pub(crate) struct Control {
    directory: PathBuf,
    #[cfg(windows)]
    pinned_directory: std::cell::OnceCell<windows::DirectoryPin>,
}

impl Control {
    pub(crate) fn new(directory: PathBuf) -> Self {
        #[cfg(windows)]
        let directory = windows::verbatim_path(&directory).unwrap_or_default();
        Self {
            directory,
            #[cfg(windows)]
            pinned_directory: std::cell::OnceCell::new(),
        }
    }

    fn ready(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            if self.pinned_directory.get().is_none() {
                let pin = windows::DirectoryPin::open(&self.directory)?;
                let _ = self.pinned_directory.set(pin);
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            confined::private_directory(&self.directory)
        }
    }

    fn verify_existing(&self) -> io::Result<bool> {
        #[cfg(windows)]
        if self.directory.as_os_str().is_empty() {
            return Err(io::ErrorKind::Unsupported.into());
        }
        match fs::symlink_metadata(&self.directory) {
            Ok(_) => self.ready().map(|()| true),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    pub(crate) fn read(&self) -> io::Result<Consent> {
        if !self.verify_existing()? {
            return Ok(Consent::default());
        }
        let file = match private_file(&self.directory.join("consent.json"), false, false) {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Consent::default()),
            Err(err) => return Err(err),
        };
        let mut bytes = Vec::new();
        file.take(CONSENT_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > CONSENT_BYTES {
            return Err(io::ErrorKind::InvalidData.into());
        }
        serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData.into())
    }

    pub(crate) fn pending_clear(&self) -> io::Result<bool> {
        if !self.verify_existing()? {
            return Ok(false);
        }
        #[cfg(windows)]
        if self.quarantine_exists()? {
            return Ok(true);
        }
        match private_file(&self.directory.join("clear.pending"), false, false) {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    pub(crate) fn set_enabled(&self, paths: Vec<PathBuf>) -> io::Result<()> {
        if paths.is_empty() || paths.len() > MAX_ROOTS {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let _consent = self.lock_exclusive("consent.lock")?;
        if self.pending_clear()? {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let mut roots = Vec::new();
        for path in paths {
            let binding = confined::bind(path)?;
            if !roots.iter().any(|existing: &Binding| {
                existing.device == binding.device
                    && existing.inode == binding.inode
                    && existing.file_id_high == binding.file_id_high
            }) {
                roots.push(binding);
            }
        }
        self.write_consent(&Consent {
            enabled: true,
            roots,
        })
    }

    pub(crate) fn stop(&self) -> io::Result<()> {
        if !self.verify_existing()? {
            return Ok(());
        }
        let _consent = self.lock_exclusive("consent.lock")?;
        let mut consent = self.read()?;
        consent.enabled = false;
        self.write_consent(&consent)
    }

    pub(crate) fn begin_clear(&self) -> io::Result<()> {
        let _consent = self.lock_exclusive("consent.lock")?;
        let marker = self.directory.join("clear.pending");
        #[cfg(windows)]
        self.write_atomic(&marker, b"clear\n")?;
        #[cfg(not(windows))]
        {
            let file = private_file(&marker, true, false)?;
            file.sync_all()?;
            self.sync_directory()?;
        }
        self.write_consent(&Consent::default())
    }

    pub(crate) fn complete_clear(&self) -> io::Result<()> {
        let _owner = self.owner()?;
        let _life = self.lock_exclusive("lifecycle.lock")?;
        let _consent = self.lock_exclusive("consent.lock")?;
        if !self.pending_clear()? {
            return Ok(());
        }
        self.write_consent(&Consent::default())?;
        let ledger = self.directory.join("ledger");
        #[cfg(windows)]
        let ledger = self.quarantine_ledger(&ledger)?;
        match fs::symlink_metadata(&ledger) {
            Ok(_) => {
                confined::private_directory(&ledger)?;
                for name in [
                    "usage.sqlite3",
                    "usage.sqlite3-journal",
                    "usage.sqlite3-wal",
                    "usage.sqlite3-shm",
                ] {
                    remove_file_if_present(&ledger.join(name))?;
                }
                fs::remove_dir(&ledger)?;
                self.sync_directory()?;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        remove_file_if_present(&self.directory.join("clear.pending"))?;
        self.sync_directory()
    }

    pub(crate) fn database(&self, create: bool) -> io::Result<PathBuf> {
        #[cfg(windows)]
        if create {
            self.ready()?;
        } else if !self.verify_existing()? {
            return Err(io::ErrorKind::NotFound.into());
        }
        let directory = self.directory.join("ledger");
        if !create {
            fs::symlink_metadata(&directory)?;
        }
        confined::private_directory(&directory)?;
        let path = directory.join("usage.sqlite3");
        // Opening then closing the database with a raw fd releases this process's POSIX locks.
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                #[cfg(unix)]
                validate_file(&metadata)?;
                #[cfg(not(unix))]
                {
                    let _ = metadata;
                    drop(private_file(&path, false, false)?);
                }
            }
            Err(err) if create && err.kind() == io::ErrorKind::NotFound => {
                drop(private_file(&path, true, false)?);
            }
            Err(err) => return Err(err),
        }
        #[cfg(windows)]
        for name in [
            "usage.sqlite3-journal",
            "usage.sqlite3-wal",
            "usage.sqlite3-shm",
        ] {
            match private_file(&directory.join(name), false, false) {
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(path)
    }

    pub(crate) fn approve_read(&self, expected: &Consent) -> io::Result<File> {
        let file = private_file(&self.directory.join("consent.lock"), false, false)?;
        file.try_lock_shared().map_err(lock_error)?;
        if self.pending_clear()? || self.read()? != *expected {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        Ok(file)
    }

    pub(crate) fn owner(&self) -> io::Result<File> {
        self.lock_exclusive("owner.lock")
    }

    pub(crate) fn reader(&self) -> io::Result<File> {
        // A follower never creates the DB, but an old owner may not have initialized
        // the shared lock file if it exited between acquiring ownership and opening it.
        self.ready()?;
        let file = private_file(&self.directory.join("lifecycle.lock"), true, false)?;
        file.try_lock_shared().map_err(lock_error)?;
        if self.pending_clear()? {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        Ok(file)
    }

    fn lock_exclusive(&self, name: &str) -> io::Result<File> {
        self.ready()?;
        let file = private_file(&self.directory.join(name), true, false)?;
        file.try_lock().map_err(lock_error)?;
        Ok(file)
    }

    fn write_consent(&self, consent: &Consent) -> io::Result<()> {
        let bytes = serde_json::to_vec(consent).map_err(|_| io::ErrorKind::InvalidInput)?;
        if bytes.len() as u64 > CONSENT_BYTES {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.write_atomic(&self.directory.join("consent.json"), &bytes)
    }

    fn write_atomic(&self, target: &Path, bytes: &[u8]) -> io::Result<()> {
        let temporary = self.directory.join("consent.tmp");
        let mut file = private_file(&temporary, true, true)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        #[cfg(windows)]
        windows::replace_file(&temporary, target, true)?;
        #[cfg(not(windows))]
        fs::rename(&temporary, target)?;
        self.sync_directory()
    }

    #[cfg(windows)]
    fn quarantine_exists(&self) -> io::Result<bool> {
        match fs::symlink_metadata(self.directory.join("ledger.pending-delete")) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    #[cfg(windows)]
    fn quarantine_ledger(&self, ledger: &Path) -> io::Result<PathBuf> {
        let quarantine = self.directory.join("ledger.pending-delete");
        match fs::symlink_metadata(ledger) {
            Ok(_) => {
                confined::private_directory(ledger)?;
                windows::replace_file(ledger, &quarantine, false)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Ok(quarantine)
    }

    #[cfg(not(windows))]
    fn sync_directory(&self) -> io::Result<()> {
        File::open(&self.directory)?.sync_all()
    }

    // Windows lacks a per-directory fsync. The marker and consent use
    // write-through moves. A ledger that survives interrupted deletion is
    // off the live path and independently counts as pending clear.
    #[cfg(windows)]
    fn sync_directory(&self) -> io::Result<()> {
        Ok(())
    }
}

fn lock_error(error: std::fs::TryLockError) -> io::Error {
    match error {
        std::fs::TryLockError::WouldBlock => io::ErrorKind::WouldBlock.into(),
        std::fs::TryLockError::Error(error) => error,
    }
}

fn remove_file_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(unix)]
fn private_file(path: &Path, create: bool, truncate: bool) -> io::Result<File> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    let file = OpenOptions::new()
        .read(true)
        .write(create)
        .create(create)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .mode(0o600)
        .open(path)?;
    validate_file(&file.metadata()?)?;
    if truncate {
        file.set_len(0)?;
    }
    Ok(file)
}

#[cfg(windows)]
use windows::private_file;

#[cfg(not(any(unix, windows)))]
fn private_file(_: &Path, _: bool, _: bool) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(unix)]
fn validate_file(info: &fs::Metadata) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if !info.is_file()
        || info.uid() != unsafe { libc::geteuid() }
        || info.mode() & 0o077 != 0
        || info.nlink() != 1
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}
