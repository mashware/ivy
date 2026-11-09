//! Explicit installation of Ivy's own OpenCode integration asset.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const ASSET: &str = include_str!("../../../resources/agent-integrations/opencode/ivy-opencode.tsx");
static SERIAL: AtomicU64 = AtomicU64::new(0);

pub(crate) fn plugin_path(config_dir: &Path) -> PathBuf {
    config_dir.join("agent-integrations/opencode/ivy-opencode.tsx")
}

/// A JSON fragment for adding to, not replacing, OpenCode's existing plugin array.
pub(crate) fn snippet(config_dir: &Path) -> String {
    serde_json::to_string(&plugin_path(config_dir).to_string_lossy().as_ref())
        .expect("path serializes")
}

/// Install or repair the bundled asset; never edit the OpenCode configuration.
pub(crate) fn install(config_dir: &Path) -> io::Result<PathBuf> {
    let path = plugin_path(config_dir);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("plugin has no parent directory"))?;
    fs::create_dir_all(parent)?;
    match fs::symlink_metadata(&path) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => {
            return Err(io::Error::other("plugin destination is not a regular file"));
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let temporary = parent.join(format!(
        ".ivy-opencode-{}-{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(ASSET.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        Ok(path.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn install_and_repair_touch_only_owned_asset() {
        let dir = std::env::temp_dir().join(format!(
            "ivy-plugin-test-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("tui.json"), "existing").unwrap();
        let path = install(&dir).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), ASSET);
        fs::write(&path, "broken").unwrap();
        install(&dir).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), ASSET);
        assert_eq!(
            fs::read_to_string(dir.join("tui.json")).unwrap(),
            "existing"
        );
        assert_eq!(
            serde_json::from_str::<String>(&snippet(&dir)).unwrap(),
            path.to_string_lossy()
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlink_destination() {
        use std::os::unix::fs::symlink;
        let dir = std::env::temp_dir().join(format!(
            "ivy-plugin-link-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let path = plugin_path(&dir);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let target = dir.join("target");
        fs::write(&target, "untouched").unwrap();
        symlink(&target, &path).unwrap();
        assert!(install(&dir).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "untouched");
        fs::remove_dir_all(dir).unwrap();
    }
}
