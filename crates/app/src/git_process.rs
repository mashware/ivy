//! Bounded Git subprocess execution with raw output and no UI-thread waiting.
use std::io::{ErrorKind, Read};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub(crate) fn run(
    mut command: Command,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<std::process::Output, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start Git: {e}"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let (sender, receiver) = mpsc::channel();
    for (index, reader, limit) in [
        (0, Box::new(stdout) as Box<dyn Read + Send>, stdout_limit),
        (1, Box::new(stderr) as Box<dyn Read + Send>, stderr_limit),
    ] {
        let sender = sender.clone();
        if let Err(e) = thread::Builder::new()
            .name("git-output-reader".into())
            .spawn(move || {
                let _ = sender.send((index, read_bounded(reader, limit)));
            })
        {
            stop_child(&mut child);
            return Err(format!("Cannot start Git output reader: {e}"));
        }
    }
    drop(sender);
    let start = Instant::now();
    let mut output = None;
    let mut errors = None;
    let mut status = None;
    loop {
        while let Ok((index, result)) = receiver.try_recv() {
            match result {
                Ok(bytes) if index == 0 => output = Some(bytes),
                Ok(bytes) => errors = Some(bytes),
                Err(e) => {
                    stop_child(&mut child);
                    return Err(format!("Git output error: {e}"));
                }
            }
        }
        if status.is_none() {
            status = match child.try_wait() {
                Ok(status) => status,
                Err(e) => {
                    stop_child(&mut child);
                    return Err(format!("Cannot wait for Git: {e}"));
                }
            };
        }
        if let (Some(status), Some(stdout), Some(stderr)) = (&status, &output, &errors) {
            return Ok(std::process::Output {
                status: *status,
                stdout: stdout.clone(),
                stderr: stderr.clone(),
            });
        }
        if start.elapsed() >= timeout {
            stop_child(&mut child);
            return Err("Git timed out".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

// D57: never wait without a deadline, even after kill. If the OS does not reap
// promptly, dropping the handle deliberately abandons that child rather than hanging.
fn stop_child(child: &mut std::process::Child) {
    let _ = child.kill();
    let deadline = Instant::now() + Duration::from_millis(100);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => thread::sleep(Duration::from_millis(5)),
        }
    }
}

fn read_bounded(reader: impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "content exceeds the size limit",
        ));
    }
    Ok(bytes)
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetConsoleWindow() -> *mut std::ffi::c_void;
    }

    #[test]
    fn background_runner_does_not_allocate_a_console() {
        const CASE: &str =
            "git_process::windows_tests::background_runner_does_not_allocate_a_console";
        const MARKER: &str = "git-child-without-console";
        match std::env::var("IVY_GIT_CONSOLE_TEST").as_deref() {
            Ok("witness") => {
                assert!(unsafe { GetConsoleWindow() }.is_null());
                println!("{MARKER}");
            }
            Ok("host") => {
                assert!(unsafe { GetConsoleWindow() }.is_null());
                let mut command = Command::new(std::env::current_exe().unwrap());
                command
                    .args([CASE, "--exact", "--nocapture"])
                    .env("IVY_GIT_CONSOLE_TEST", "witness")
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let output = run(command, Duration::from_secs(15), 65536, 65536).unwrap();
                assert!(output.status.success(), "{:?}", output);
                assert!(String::from_utf8_lossy(&output.stdout).contains(MARKER));
            }
            _ => {
                let mut host = Command::new(std::env::current_exe().unwrap());
                host.args([CASE, "--exact", "--nocapture"])
                    .env("IVY_GIT_CONSOLE_TEST", "host")
                    .creation_flags(0x0000_0008) // DETACHED_PROCESS
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                let mut host = host.spawn().unwrap();
                let deadline = Instant::now() + Duration::from_secs(20);
                loop {
                    if let Some(status) = host.try_wait().unwrap() {
                        assert!(status.success());
                        break;
                    }
                    if Instant::now() >= deadline {
                        stop_child(&mut host);
                        panic!("detached host timed out");
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}
