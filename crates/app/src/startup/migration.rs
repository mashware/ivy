//! One-time namespace migration before preferences or recovery are read (D87).
use std::{io, path::Path};

pub(super) fn prepare(measuring: bool) -> io::Result<()> {
    if !measuring {
        let config = std::env::var_os("XDG_CONFIG_HOME");
        let home = std::env::var_os("HOME");
        if let Some(directory) = crate::session::directory_from(config.as_deref(), home.as_deref())
        {
            migrate(&directory)?;
        }
    }
    Ok(())
}

fn migrate(destination: &Path) -> io::Result<()> {
    // Any existing entry wins, including a symlink or an unreadable entry. `exists()` would
    // mistake a dangling symlink for an absent destination and permit replacing it.
    match std::fs::symlink_metadata(destination) {
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let legacy = destination.with_file_name("agent-terminal");
    match std::fs::symlink_metadata(&legacy) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return Err(io::Error::other(
                "ivy: previous configuration is not a directory",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    }
    std::fs::rename(legacy, destination).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("ivy: could not migrate configuration: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("ivy-migration-{:016x}", getrandom::u64().unwrap()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn seed(&self, name: &str, contents: &[u8]) {
            let path = self.0.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn preferences_layout_and_other_data_move_without_rewriting() {
        let dir = Directory::new();
        for name in ["config", "layout.json", "recovery.json", "extra/state"] {
            dir.seed(&format!("agent-terminal/{name}"), b"saved data\0\xff");
        }
        let destination = dir.0.join("ivy");
        migrate(&destination).unwrap();
        for name in ["config", "layout.json", "recovery.json", "extra/state"] {
            assert_eq!(
                std::fs::read(destination.join(name)).unwrap(),
                b"saved data\0\xff"
            );
        }
        assert!(!dir.0.join("agent-terminal").exists());
        migrate(&destination).unwrap();
    }

    #[test]
    fn existing_ivy_data_wins_without_merging() {
        let dir = Directory::new();
        dir.seed("agent-terminal/config", b"old");
        dir.seed("agent-terminal/layout.json", b"old layout");
        dir.seed("ivy/config", b"current");
        migrate(&dir.0.join("ivy")).unwrap();
        assert_eq!(std::fs::read(dir.0.join("ivy/config")).unwrap(), b"current");
        assert!(!dir.0.join("ivy/layout.json").exists());
        assert_eq!(
            std::fs::read(dir.0.join("agent-terminal/config")).unwrap(),
            b"old"
        );
    }

    #[test]
    fn first_launch_creates_nothing_and_an_invalid_legacy_entry_fails() {
        let dir = Directory::new();
        migrate(&dir.0.join("ivy")).unwrap();
        assert!(!dir.0.join("ivy").exists());
        dir.seed("agent-terminal", b"not a directory");
        assert!(migrate(&dir.0.join("ivy")).is_err());
        assert!(!dir.0.join("ivy").exists());
    }

    #[cfg(unix)]
    #[test]
    fn dangling_destination_is_not_replaced() {
        let dir = Directory::new();
        dir.seed("agent-terminal/config", b"old");
        std::os::unix::fs::symlink("missing", dir.0.join("ivy")).unwrap();
        migrate(&dir.0.join("ivy")).unwrap();
        assert!(dir.0.join("ivy").is_symlink());
        assert!(dir.0.join("agent-terminal/config").is_file());
    }
}
