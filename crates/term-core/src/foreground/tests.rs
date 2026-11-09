use crate::{Pty, Size};
use std::sync::mpsc::Receiver;
use std::time::Duration;

fn console(script: &str) -> (Pty, Receiver<Vec<u8>>, u32) {
    let (pty, output) = Pty::spawn("/usr/bin/python3", &["-c", script], Size::default()).unwrap();
    let mut bytes = Vec::new();
    while !bytes.contains(&b'\n') {
        bytes.extend(output.recv_timeout(Duration::from_secs(5)).unwrap());
    }
    let pid = String::from_utf8(bytes).unwrap().trim().parse().unwrap();
    (pty, output, pid)
}

const LIVE: &str = "import os; print(os.getpid(), flush=True); input()";

#[test]
fn child_exit_status_distinguishes_success_from_failed_resume() {
    for (code, success) in [(0, true), (7, false)] {
        let (mut pty, output) =
            Pty::spawn("/bin/sh", &["-c", &format!("exit {code}")], Size::default()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while pty.exit_success().is_none() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(pty.exit_success(), Some(success));
        drop(output);
    }
}

#[test]
fn foreground_scope_is_owned_send_and_ends_with_its_console() {
    let (pty, _output, pid) = console(LIVE);
    let scope = pty.foreground_scope().unwrap();
    let birth = super::linux::stat(pid).unwrap().birth;
    let scope = std::thread::spawn(move || {
        let chain = scope.inspect(pid, birth).unwrap();
        assert_eq!(chain.processes.last().unwrap().pid, pid);
        assert!(
            scope.inspect(pid, birth + 1).is_err(),
            "reused PID birth is rejected"
        );
        scope
    })
    .join()
    .unwrap();
    drop(pty);
    assert!(!scope.is_open());
    assert!(scope.inspect(pid, birth).is_err());
}

#[test]
fn foreground_scope_rejects_the_other_console_and_background_groups() {
    let (pty, _output, pid) = console(LIVE);
    let (other, _other_output, other_pid) = console(LIVE);
    let scope = pty.foreground_scope().unwrap();
    assert!(
        scope
            .inspect(other_pid, super::linux::stat(other_pid).unwrap().birth)
            .is_err()
    );
    assert!(
        other
            .foreground_scope()
            .unwrap()
            .inspect(pid, super::linux::stat(pid).unwrap().birth)
            .is_err()
    );
    let (background, _background_output, background_pid) = console(
        "import os, time; p=os.fork();\nif p==0:\n os.setpgid(0,0); print(os.getpid(),flush=True); time.sleep(30)\nelse:\n os.waitpid(p,0)",
    );
    assert!(
        background
            .foreground_scope()
            .unwrap()
            .inspect(
                background_pid,
                super::linux::stat(background_pid).unwrap().birth
            )
            .is_err()
    );
    // SAFETY: this process was created by this test; its isolated background group otherwise survives HUP.
    unsafe { libc::kill(background_pid as i32, libc::SIGKILL) };
}

#[test]
fn nested_descendants_are_exposed_and_root_exit_is_native() {
    use std::os::fd::AsRawFd;
    let (mut pty, _output, pid) = console(
        "import os; p=os.fork();\nif p==0:\n print(os.getpid(),flush=True); input()\nelse:\n os.waitpid(p,0)",
    );
    let scope = pty.foreground_scope().unwrap();
    let chain = scope
        .inspect(pid, super::linux::stat(pid).unwrap().birth)
        .unwrap();
    assert_eq!(chain.processes.len(), 2);
    assert_ne!(chain.processes[0].pid, chain.processes[1].pid);
    let lifetime = chain.processes[1].lifetime().unwrap();
    // SAFETY: F_GETFD reads flags of an owned descriptor.
    assert_ne!(
        unsafe { libc::fcntl(lifetime.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
    pty.write(b"\n").unwrap();
    let mut poll = libc::pollfd {
        fd: lifetime.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: a single valid owned pidfd; the test waits at most five seconds for child exit.
    assert_eq!(unsafe { libc::poll(&mut poll, 1, 5000) }, 1);
    assert!(scope.inspect(pid, chain.processes[1].birth).is_err());
}
