#![cfg(windows)]

use panel_control::codex_route::{Registry, lock_route};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

const THREAD: &str = "01234567-89ab-cdef-0123-456789abcdef";

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let path = std::env::temp_dir().join(format!(
            "ivy-route-sharing-{}-{:032x}",
            std::process::id(),
            u128::from_le_bytes(random)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn assert_no_temporary_files(&self) {
        for entry in fs::read_dir(self.0.join("ivy-panel-routes")).unwrap() {
            let name = entry.unwrap().file_name();
            let name = name.to_string_lossy();
            assert!(!name.starts_with("write-"), "leftover temporary: {name}");
            assert!(!name.starts_with("mapping-"), "leftover temporary: {name}");
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn deny_delete_sharing(path: &Path) -> File {
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(path)
        .unwrap()
}

#[test]
fn route_replacement_with_external_reader_is_atomic_or_preserves_previous_state() {
    let fixture = Fixture::new();
    let registry = Registry::for_home(&fixture.0).unwrap();
    let route = registry.pending().unwrap();
    registry
        .bind(THREAD, &route, "pane", &fixture.0, "old")
        .unwrap();
    let before = fs::read(&route).unwrap();
    let mut reader = deny_delete_sharing(&route);

    let result = registry.bind(THREAD, &route, "pane", &fixture.0, "new");
    if result.is_ok() {
        let current = lock_route(&route).unwrap();
        assert!(current.identity.active);
        assert_eq!(current.identity.epoch, "new");
        assert_eq!(current.identity.pane_id, "pane");
        assert_eq!(current.identity.pane_dir, fixture.0);
        let previous: serde_json::Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(current.identity.proof, previous["proof"].as_str().unwrap());
    } else {
        assert_eq!(fs::read(&route).unwrap(), before);
    }
    assert_eq!(registry.route_for(THREAD).unwrap(), Some(route.clone()));
    let mut held = Vec::new();
    reader.read_to_end(&mut held).unwrap();
    assert_eq!(held, before);
    fixture.assert_no_temporary_files();

    drop(reader);
    registry
        .bind(THREAD, &route, "pane", &fixture.0, "new")
        .unwrap();
    assert_eq!(lock_route(&route).unwrap().identity.epoch, "new");
}

#[test]
fn mapping_replacement_with_external_reader_is_atomic_or_restores_displaced_routes() {
    let fixture = Fixture::new();
    let registry = Registry::for_home(&fixture.0).unwrap();
    let old = registry.pending().unwrap();
    let new = registry.pending().unwrap();
    registry
        .bind(THREAD, &old, "pane", &fixture.0, "old")
        .unwrap();
    let old_before = fs::read(&old).unwrap();
    let new_before = fs::read(&new).unwrap();
    let mapping = fixture
        .0
        .join("ivy-panel-routes")
        .join(format!("thread-{THREAD}"));
    let mapping_before = fs::read(&mapping).unwrap();
    let mut reader = deny_delete_sharing(&mapping);

    let result = registry.bind(THREAD, &new, "pane", &fixture.0, "new");
    if result.is_ok() {
        assert!(!lock_route(&old).unwrap().identity.active);
        let current = lock_route(&new).unwrap();
        assert!(current.identity.active);
        assert_eq!(current.identity.epoch, "new");
        assert_eq!(current.identity.pane_id, "pane");
        assert_eq!(current.identity.pane_dir, fixture.0);
        let previous: serde_json::Value = serde_json::from_slice(&new_before).unwrap();
        assert_eq!(current.identity.proof, previous["proof"].as_str().unwrap());
        drop(current);
        assert_eq!(registry.route_for(THREAD).unwrap(), Some(new.clone()));
    } else {
        assert_eq!(fs::read(&old).unwrap(), old_before);
        assert_eq!(fs::read(&new).unwrap(), new_before);
        assert_eq!(fs::read(&mapping).unwrap(), mapping_before);
        assert_eq!(registry.route_for(THREAD).unwrap(), Some(old));
    }
    let mut held = Vec::new();
    reader.read_to_end(&mut held).unwrap();
    assert_eq!(held, mapping_before);
    fixture.assert_no_temporary_files();

    drop(reader);
    registry
        .bind(THREAD, &new, "pane", &fixture.0, "new")
        .unwrap();
    assert!(lock_route(&new).unwrap().identity.active);
    assert_eq!(registry.route_for(THREAD).unwrap(), Some(new));
}
