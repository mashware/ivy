#![cfg(test)]
use super::*;
use crate::{Pty, Size};

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn powershell() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    format!(r"{root}\System32\WindowsPowerShell\v1.0\powershell.exe")
}

fn directory() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ivy-foreground-{}-{unique}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    path
}

fn escaped(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn launch(command: &str) -> (Pty, Receiver<Vec<u8>>) {
    Pty::spawn(
        &powershell(),
        &["-NoLogo", "-NoProfile", "-Command", command],
        Size::default(),
    )
    .unwrap()
}

fn wait_until(pty: &mut Pty, output: &Receiver<Vec<u8>>, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut seen = String::new();
    let mut answered = 0;
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "fixture did not reach its barrier"
        );
        if let Ok(bytes) = output.recv_timeout(Duration::from_millis(50)) {
            seen.push_str(&String::from_utf8_lossy(&bytes));
            for _ in answered..seen.matches("\x1b[6n").count() {
                let _ = pty.write(b"\x1b[1;1R");
            }
            answered = seen.matches("\x1b[6n").count();
        }
    }
}

fn console(base: &std::path::Path, name: &str) -> (Pty, Receiver<Vec<u8>>) {
    let marker = base.join(format!("{name}.ready"));
    let release = base.join(format!("{name}.release"));
    let command = format!(
        "$deadline = [DateTime]::UtcNow.AddSeconds(60); [IO.File]::WriteAllText('{}','ready'); while (!(Test-Path -LiteralPath '{}') -and [DateTime]::UtcNow -lt $deadline) {{ Start-Sleep -Milliseconds 25 }}",
        escaped(&marker),
        escaped(&release)
    );
    let (mut pty, output) = launch(&command);
    wait_until(&mut pty, &output, || {
        std::fs::read_to_string(&marker).is_ok_and(|v| v == "ready")
    });
    (pty, output)
}

#[test]
fn windows_foreground_root_and_other_console_and_revocation() {
    let base = directory();
    let (first, _first_output) = console(&base, "first");
    let (second, _second_output) = console(&base, "second");
    let first_pid = first.child.as_ref().unwrap().process_id().unwrap();
    let second_pid = second.child.as_ref().unwrap().process_id().unwrap();
    let scope = first.foreground_scope().unwrap();
    assert_eq!(scope.classify_ancestry(first_pid), Ancestry::Inside);
    assert_eq!(scope.classify_ancestry(second_pid), Ancestry::Outside);
    let observation = scope.observe_process(first_pid).unwrap();
    let birth = observation.identity().birth;
    assert!(matches!(scope.inspect(first_pid, birth), Err(error)
        if error.kind() == std::io::ErrorKind::Unsupported));
    assert!(
        matches!(scope.inspect_descendant(first_pid, birth), Err(error)
        if error.kind() == std::io::ErrorKind::Unsupported)
    );
    assert_eq!(observation.identity().pid, first_pid);
    assert!(observation.executable().unwrap().is_absolute());
    assert!(observation.is_alive());
    drop(first);
    assert!(!scope.is_open());
    assert_eq!(scope.classify_ancestry(first_pid), Ancestry::Unknown);
    assert!(observation.executable().is_err());
    std::fs::write(base.join("second.release"), "go").unwrap();
    drop(second);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn windows_foreground_scope_moved_to_worker_observes_root_exit() {
    let base = directory();
    let (mut pty, output) = console(&base, "root");
    let pid = pty.child.as_ref().unwrap().process_id().unwrap();
    let scope = scope(&pty).unwrap();
    let observation = scope.observe_process(pid).unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        receive.recv_timeout(Duration::from_secs(30)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while scope.is_open() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!scope.is_open());
        assert_eq!(scope.classify_ancestry(pid), Ancestry::Unknown);
        assert!(observation.executable().is_err());
        assert!(observation.lifetime().is_err());
    });
    std::fs::write(base.join("root.release"), "go").unwrap();
    send.send(()).unwrap();
    wait_until(&mut pty, &output, || worker.is_finished());
    worker.join().unwrap();
    drop(pty);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn windows_foreground_descendant_lifetime_signals_exit_259() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    let base = directory();
    let marker = base.join("child.ready");
    let release = base.join("child.release");
    let script = base.join("child.ps1");
    std::fs::write(&script, format!("$deadline = [DateTime]::UtcNow.AddSeconds(60); [IO.File]::WriteAllText('{}', $PID.ToString()); while (!(Test-Path -LiteralPath '{}') -and [DateTime]::UtcNow -lt $deadline) {{ Start-Sleep -Milliseconds 25 }}; exit 259", escaped(&marker), escaped(&release))).unwrap();
    let command = format!(
        "$s = New-Object System.Diagnostics.ProcessStartInfo; $s.FileName = '{}'; $s.Arguments = '-NoProfile -File \"{}\"'; $s.UseShellExecute = $false; [void][System.Diagnostics.Process]::Start($s); Start-Sleep 30",
        powershell().replace('\'', "''"),
        escaped(&script)
    );
    let (mut pty, output) = launch(&command);
    let scope = scope(&pty).unwrap();
    let mut pid = None;
    wait_until(&mut pty, &output, || {
        pid = std::fs::read_to_string(&marker)
            .ok()
            .and_then(|s| s.parse::<u32>().ok());
        pid.is_some()
    });
    let observation = scope.observe_process(pid.unwrap()).unwrap();
    let lifetime = observation.lifetime().unwrap();
    std::fs::write(&release, "go").unwrap();
    // SAFETY: the owned process handle remains open for this bounded wait.
    assert_eq!(
        unsafe { WaitForSingleObject(lifetime.as_raw_handle(), 30_000) },
        WAIT_OBJECT_0
    );
    let mut exit_code = 0;
    // SAFETY: the process handle is open and exit_code is writable.
    assert_ne!(
        unsafe { GetExitCodeProcess(lifetime.as_raw_handle(), &mut exit_code) },
        0
    );
    assert_eq!(exit_code, 259);
    assert!(scope.is_open());
    assert!(!observation.is_alive());
    assert!(observation.executable().is_err());
    drop(pty);
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn windows_foreground_nested_descendant_is_inside() {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let powershell = format!(r"{root}\System32\WindowsPowerShell\v1.0\powershell.exe");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("ivy-chain-{}-{unique}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let marker = base.join("child.txt");
    let script = base.join("middle.ps1");
    let escaped = |path: &std::path::Path| path.to_string_lossy().replace('\'', "''");
    std::fs::write(&script, format!(
        "$s = New-Object System.Diagnostics.ProcessStartInfo; $s.FileName = '{}'; $s.Arguments = '-NoProfile -Command Start-Sleep 30'; $s.UseShellExecute = $false; $p = [System.Diagnostics.Process]::Start($s); [IO.File]::WriteAllText('{}', $p.Id.ToString()); Start-Sleep 30",
        powershell.replace('\'', "''"), escaped(&marker))).unwrap();
    let command = format!(
        "$s = New-Object System.Diagnostics.ProcessStartInfo; $s.FileName = '{}'; $s.Arguments = '-NoProfile -File \"{}\"'; $s.UseShellExecute = $false; [void][System.Diagnostics.Process]::Start($s); Start-Sleep 30",
        powershell.replace('\'', "''"),
        escaped(&script)
    );
    let (mut pty, output) = Pty::spawn(
        &powershell,
        &["-NoProfile", "-Command", &command],
        Size::default(),
    )
    .unwrap();
    let scope = scope(&pty).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut seen = String::new();
    let mut answered = 0;
    let pid = loop {
        if let Ok(value) = std::fs::read_to_string(&marker)
            && let Ok(pid) = value.parse::<u32>()
        {
            break pid;
        }
        assert!(
            Instant::now() < deadline,
            "nested process did not publish its PID"
        );
        if let Ok(bytes) = output.recv_timeout(Duration::from_millis(50)) {
            seen.push_str(&String::from_utf8_lossy(&bytes));
            for _ in answered..seen.matches("\x1b[6n").count() {
                pty.write(b"\x1b[1;1R").unwrap();
            }
            answered = seen.matches("\x1b[6n").count();
        }
    };
    assert_eq!(scope.classify_ancestry(pid), Ancestry::Inside);
    drop(pty);
    let _ = std::fs::remove_dir_all(base);
}
