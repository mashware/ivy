//! Snapshot checking and replacement of the human-owned settings file.
use super::{MAX_BYTES, interpret};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Store {
    path: PathBuf,
    original: Option<Vec<u8>>,
}

fn snapshot(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if metadata.file_type().is_symlink() {
        return Err(
            "Saving requires a regular config file; edit the symbolic link target by hand.".into(),
        );
    }
    if !metadata.is_file() {
        return Err("The configuration path is not a regular file.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("The configuration exceeds 64 KiB.".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
    interpret(text).map_err(|e| e.to_string())?;
    Ok(Some(bytes))
}

impl Store {
    pub fn open(path: PathBuf) -> Result<(Self, String), String> {
        let original = snapshot(&path)?;
        let text = original
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        Ok((Self { path, original }, text))
    }

    pub fn save(&mut self, text: &str) -> Result<(), String> {
        self.save_replacing(text, |from, to| fs::rename(from, to))
    }

    fn save_replacing(
        &mut self,
        text: &str,
        replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<(), String> {
        if text.len() > MAX_BYTES {
            return Err("The configuration exceeds 64 KiB.".into());
        }
        interpret(text).map_err(|e| e.to_string())?;
        if snapshot(&self.path)? != self.original {
            return Err(
                "The configuration changed outside Settings. Cancel and reopen to reload it."
                    .into(),
            );
        }
        let parent = self
            .path
            .parent()
            .ok_or("The configuration has no directory.")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".config-{}-{serial}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            if let Ok(metadata) = fs::metadata(&self.path) {
                file.set_permissions(metadata.permissions())?;
            }
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            // Recheck after preparation: a hand editor may have saved while the temporary file was written.
            if snapshot(&self.path).map_err(std::io::Error::other)? != self.original {
                return Err(std::io::Error::other(
                    "The configuration changed outside Settings. Cancel and reopen to reload it.",
                ));
            }
            replace(&temporary, &self.path)
        })();
        drop(file);
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error.to_string());
        }
        self.original = Some(text.as_bytes().to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "terminal-settings-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn save_detects_external_creation_edit_and_deletion() {
        let dir = dir();
        let path = dir.join("config");
        let (mut store, _) = Store::open(path.clone()).unwrap();
        fs::write(&path, "size=19").unwrap();
        assert!(store.save("size=20").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "size=19");
        let (mut store, _) = Store::open(path.clone()).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(store.save("size=20").is_err());
        assert!(!path.exists());
        fs::write(&path, "size=18").unwrap();
        let (mut store, _) = Store::open(path.clone()).unwrap();
        fs::write(&path, "size=21").unwrap();
        assert!(store.save("size=20").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "size=21");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn absent_file_save_and_failure_keep_the_previous_bytes() {
        let dir = dir();
        let path = dir.join("new/config");
        let (mut store, text) = Store::open(path.clone()).unwrap();
        assert!(text.is_empty());
        assert!(!path.parent().unwrap().exists());
        store.save("size=18").unwrap();
        assert_eq!(super::super::read(&path).unwrap().0.font_size, Some(18.0));
        assert!(store.save(&"x".repeat(MAX_BYTES + 1)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "size=18");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_replacement_preserves_the_file_and_cleans_the_temporary() {
        let dir = dir();
        let path = dir.join("config");
        fs::write(&path, "# original\nsize=14").unwrap();
        let (mut store, _) = Store::open(path.clone()).unwrap();
        let error = store
            .save_replacing("size=20", |temporary, destination| {
                assert_eq!(fs::read_to_string(temporary).unwrap(), "size=20");
                assert_eq!(
                    fs::read_to_string(destination).unwrap(),
                    "# original\nsize=14"
                );
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "replacement denied",
                ))
            })
            .unwrap_err();
        assert!(error.contains("replacement denied"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "# original\nsize=14");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        store.save("size=18").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "size=18");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_whole_files_are_never_overwritten() {
        let dir = dir();
        let path = dir.join("config");
        for bytes in [
            b"version=999".as_slice(),
            &[255],
            &vec![b'x'; MAX_BYTES + 1],
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(Store::open(path.clone()).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symbolic_link_save_is_refused_and_permissions_survive() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = dir();
        let path = dir.join("config");
        let target = dir.join("target");
        fs::write(&target, "size=18").unwrap();
        symlink(&target, &path).unwrap();
        assert!(Store::open(path.clone()).is_err());
        assert!(path.is_symlink());
        fs::remove_file(&path).unwrap();
        fs::write(&path, "size=14").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let (mut store, _) = Store::open(path.clone()).unwrap();
        store.save("size=20").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
