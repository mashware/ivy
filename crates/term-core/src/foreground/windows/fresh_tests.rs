#![cfg(test)]

use super::*;
use crate::{Pty, Size};
use std::os::windows::io::AsHandle;
use std::path::Path;
use std::time::{Duration, Instant};

#[test]
fn fresh_root_rejects_wrong_image_foreign_root_and_console_close() {
    let _inspection_guard = super::inspection::TEST_LOCK.lock().unwrap();
    let exe = std::env::current_exe().unwrap();
    let root = std::env::var("SystemRoot").unwrap();
    let shell = Path::new(&root).join(r"System32\cmd.exe");
    let shell_str = shell.to_str().unwrap();
    let (first, _) = Pty::spawn(shell_str, &["/K"], Size::default()).unwrap();
    let (second, _) = Pty::spawn(shell_str, &["/K"], Size::default()).unwrap();
    let first_scope = scope(&first).unwrap();
    assert!(first_scope.fresh_invocation(&exe, &[]).is_err());
    let invocation = first_scope.fresh_invocation(&shell, &[]).unwrap();
    let witness = invocation.attest(first_scope.root.as_handle()).unwrap();
    let again = invocation.attest(first_scope.root.as_handle()).unwrap();
    assert!(witness.same_invocation(&again));
    let foreign = scope(&second).unwrap();
    assert!(invocation.attest(foreign.root.as_handle()).is_err());
    assert!(witness.current());
    drop(first);
    assert!(!witness.current());
}

#[test]
fn fresh_witness_expires_when_root_exits() {
    let _inspection_guard = super::inspection::TEST_LOCK.lock().unwrap();
    let root = std::env::var("SystemRoot").unwrap();
    let powershell = Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let (pty, _output) = Pty::spawn(
        powershell.to_str().unwrap(),
        &["-NoProfile", "-Command", "Start-Sleep 3"],
        Size::default(),
    )
    .unwrap();
    let scope = scope(&pty).unwrap();
    let invocation = scope.fresh_invocation(&powershell, &[]).unwrap();
    let witness = invocation.attest(scope.root.as_handle()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while witness.current() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!witness.current());
}

#[test]
fn fresh_rejects_recognized_nested_process_and_helper_exit_keeps_witness() {
    let _inspection_guard = super::inspection::TEST_LOCK.lock().unwrap();
    let root = std::env::var("SystemRoot").unwrap();
    let powershell = Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let shell = Path::new(&root).join(r"System32\cmd.exe");
    let marker = std::env::temp_dir().join(format!(
        "ivy-fresh-{}-{:?}.txt",
        std::process::id(),
        std::thread::current().id()
    ));
    let script = format!(
        "$p = Start-Process -FilePath '{}' -ArgumentList '/c ping -n 5 127.0.0.1 >nul' -PassThru; [IO.File]::WriteAllText('{}', $p.Id); $p.WaitForExit(); Start-Sleep 30",
        shell.display(),
        marker.display()
    );
    let (pty, _output) = Pty::spawn(
        powershell.to_str().unwrap(),
        &["-NoProfile", "-Command", &script],
        Size::default(),
    )
    .unwrap();
    let scope = scope(&pty).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let pid = loop {
        if let Ok(value) = std::fs::read_to_string(&marker) {
            if let Ok(pid) = value.parse() {
                break pid;
            }
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    let helper = process::open(pid).unwrap();
    let permissive = scope.fresh_invocation(&powershell, &[]).unwrap();
    let witness = permissive.attest(helper.as_handle()).unwrap();
    let restricted = scope.fresh_invocation(&powershell, &[shell]).unwrap();
    assert!(restricted.attest(helper.as_handle()).is_err());
    while process::is_alive(helper.as_handle()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!process::is_alive(helper.as_handle()));
    assert!(witness.current());
    drop(pty);
    assert!(!witness.current());
    let _ = std::fs::remove_file(marker);
}
