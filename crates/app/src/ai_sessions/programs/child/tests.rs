#![cfg(test)]

use super::*;
use panel_contract::PanelDir;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

static SERIAL: Mutex<()> = Mutex::new(());

struct Fixture {
    directory: PanelDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            directory: PanelDir::create().unwrap(),
        }
    }
    fn script(&self, body: &str) -> PathBuf {
        let path = self.directory.path().join("volta");
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for file in ["volta", "pid", "started"] {
            let _ = std::fs::remove_file(self.directory.path().join(file));
        }
    }
}

fn wait_for_cleanup() {
    let deadline = Instant::now() + Duration::from_secs(2);
    while RESERVED.load(Ordering::Acquire) != 0 {
        reap();
        assert!(
            Instant::now() < deadline,
            "resolver cleanup did not release its process slot"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn volta_stdout_and_success_are_collected_under_one_deadline() {
    let _serial = SERIAL.lock().unwrap();
    let fixture = Fixture::new();
    let volta = fixture.script("printf '/bin/true\\n'\n");
    let resolved =
        super::super::volta_which(&volta, "codex", std::ffi::OsStr::new("/usr/bin:/bin")).unwrap();
    assert_eq!(resolved, std::fs::canonicalize("/bin/true").unwrap());
    wait_for_cleanup();
}

#[test]
fn volta_timeout_does_not_wait_after_kill_and_the_child_is_reaped() {
    let _serial = SERIAL.lock().unwrap();
    let fixture = Fixture::new();
    let pid = fixture.directory.path().join("pid");
    let body = format!(
        "printf '%s' \"$$\" > '{}'\nexec /bin/sleep 5\n",
        pid.display()
    );
    let volta = fixture.script(&body);
    let started = Instant::now();
    let result = super::super::volta_which(&volta, "codex", std::ffi::OsStr::new("/usr/bin:/bin"));
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    assert!(
        started.elapsed() < Duration::from_millis(1300),
        "the resolver waited beyond its exchange deadline to reap"
    );
    let pid: i32 = std::fs::read_to_string(pid).unwrap().parse().unwrap();
    wait_for_cleanup();
    let mut status = 0;
    // SAFETY: WNOHANG observes only this test's known child and cannot block.
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn volta_descendant_holding_stdout_cannot_extend_the_exchange_deadline() {
    let _serial = SERIAL.lock().unwrap();
    let fixture = Fixture::new();
    let volta = fixture.script("/bin/sleep 2 &\nprintf '/bin/true\\n'\nexit 0\n");
    let started = Instant::now();
    let result = super::super::volta_which(&volta, "codex", std::ffi::OsStr::new("/usr/bin:/bin"));
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
    assert!(
        started.elapsed() < Duration::from_millis(1300),
        "stdout waited for the descendant instead of the shared deadline"
    );
    wait_for_cleanup();
}

#[test]
fn resolver_cleanup_exhaustion_rejects_before_spawning_another_child() {
    let _serial = SERIAL.lock().unwrap();
    wait_for_cleanup();
    let slots: Vec<_> = (0..MAX_CHILDREN)
        .map(|_| Slot::reserve().unwrap())
        .collect();
    let fixture = Fixture::new();
    let marker = fixture.directory.path().join("started");
    let volta = fixture.script(&format!(
        "printf started > '{}'\nprintf '/bin/true\\n'\n",
        marker.display()
    ));
    let result = super::super::volta_which(&volta, "codex", std::ffi::OsStr::new("/usr/bin:/bin"));
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::WouldBlock);
    assert!(
        !marker.exists(),
        "the resolver spawned before reserving cleanup capacity"
    );
    drop(slots);
    assert_eq!(RESERVED.load(Ordering::Acquire), 0);
}
