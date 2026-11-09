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
