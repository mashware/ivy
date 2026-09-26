//! #159, below the crate: a raw pseudoterminal, a writer blocked on the master, the child killed —
//! and then, one at a time, the things that might wake the writer.
//!
//! `cargo run --release --example writer_wake -p term-core -- <poke>`
//! poke: none | flush-out | flush-in | flush-both | winsz | close-master | diag | hold
//!
//! Linux only: it is the Linux behaviour it measures, and `libc` is not a dependency elsewhere.

fn main() {
    #[cfg(target_os = "linux")]
    probe::main();
    #[cfg(not(target_os = "linux"))]
    eprintln!("this probe measures Linux's pseudoterminal and does nothing elsewhere");
}

#[cfg(target_os = "linux")]
mod probe {
    use std::os::fd::FromRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    pub fn main() {
        let poke = std::env::args().nth(1).unwrap_or_else(|| "none".into());
        let mut master = 0;
        let mut slave = 0;
        unsafe {
            assert_eq!(
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null()
                ),
                0
            );
            let mut t: libc::termios = std::mem::zeroed();
            libc::tcgetattr(slave, &mut t);
            libc::cfmakeraw(&mut t);
            libc::tcsetattr(slave, libc::TCSANOW, &t);
        }
        let dup = |fd| unsafe { Stdio::from_raw_fd(libc::dup(fd)) };
        let mut cmd = Command::new("sleep");
        cmd.arg("3600")
            .stdin(dup(slave))
            .stdout(dup(slave))
            .stderr(dup(slave));
        unsafe {
            cmd.pre_exec(move || {
                libc::setsid();
                libc::ioctl(0, libc::TIOCSCTTY, 0);
                Ok(())
            });
        }
        let mut child = cmd.spawn().expect("spawn");
        // The `Command` owns the parent's copies of the slave it was handed: they close with it.
        drop(cmd);
        unsafe { libc::close(slave) };

        let written = Arc::new(AtomicIsize::new(0));
        let last = Arc::new(AtomicIsize::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let (w, l, d) = (Arc::clone(&written), Arc::clone(&last), Arc::clone(&done));
        let wfd = unsafe { libc::dup(master) };
        thread::spawn(move || {
            let buf = vec![b'x'; 64 * 1024];
            loop {
                let n = unsafe { libc::write(wfd, buf.as_ptr().cast(), buf.len()) };
                l.store(
                    if n < 0 {
                        -(std::io::Error::last_os_error().raw_os_error().unwrap() as isize)
                    } else {
                        n
                    },
                    Ordering::SeqCst,
                );
                if n <= 0 {
                    break;
                }
                w.fetch_add(n, Ordering::SeqCst);
            }
            d.store(true, Ordering::SeqCst);
        });
        thread::sleep(Duration::from_millis(300));
        println!(
            "blocked after {} bytes, done={}",
            written.load(Ordering::SeqCst),
            done.load(Ordering::SeqCst)
        );
        child.kill().unwrap();
        child.wait().unwrap();
        thread::sleep(Duration::from_millis(1000));
        println!(
            "child reaped +1s: done={} last={} written={}",
            done.load(Ordering::SeqCst),
            last.load(Ordering::SeqCst),
            written.load(Ordering::SeqCst)
        );
        let t0 = Instant::now();
        unsafe {
            match poke.as_str() {
                "flush-out" => {
                    libc::tcflush(master, libc::TCOFLUSH);
                }
                "flush-in" => {
                    libc::tcflush(master, libc::TCIFLUSH);
                }
                "flush-both" => {
                    libc::tcflush(master, libc::TCIOFLUSH);
                }
                "winsz" => {
                    let ws = libc::winsize {
                        ws_row: 30,
                        ws_col: 90,
                        ws_xpixel: 0,
                        ws_ypixel: 0,
                    };
                    libc::ioctl(master, libc::TIOCSWINSZ, &ws);
                }
                "close-master" => {
                    libc::close(master);
                }
                "hold" => {
                    let mut buf = [0u8; 64];
                    libc::ptsname_r(master, buf.as_mut_ptr().cast(), buf.len());
                    println!(
                        "holding, pts={} pid={}",
                        String::from_utf8_lossy(&buf).trim_end_matches('\0'),
                        std::process::id()
                    );
                    thread::sleep(Duration::from_secs(20));
                }
                "diag" => {
                    let mut p = libc::pollfd {
                        fd: master,
                        events: libc::POLLOUT | libc::POLLIN,
                        revents: 0,
                    };
                    libc::poll(&mut p, 1, 0);
                    println!(
                        "poll revents={:#x} (POLLOUT={:#x} POLLHUP={:#x} POLLERR={:#x})",
                        p.revents,
                        libc::POLLOUT,
                        libc::POLLHUP,
                        libc::POLLERR
                    );
                    let mut buf = [0u8; 64];
                    let r = libc::ptsname_r(master, buf.as_mut_ptr().cast(), buf.len());
                    println!("ptsname r={r} {}", String::from_utf8_lossy(&buf));
                    let fl = libc::fcntl(master, libc::F_GETFL);
                    libc::fcntl(master, libc::F_SETFL, fl | libc::O_NONBLOCK);
                    let n = libc::write(master, b"y".as_ptr().cast(), 1);
                    println!(
                        "nonblocking write -> {n} {}",
                        std::io::Error::last_os_error()
                    );
                    let mut rb = [0u8; 16];
                    let n = libc::read(master, rb.as_mut_ptr().cast(), rb.len());
                    println!(
                        "nonblocking read -> {n} {}",
                        std::io::Error::last_os_error()
                    );
                    libc::fcntl(master, libc::F_SETFL, fl);
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(500));
        println!(
            "poke {poke} +500ms: done={} last={} written={} ({:?})",
            done.load(Ordering::SeqCst),
            last.load(Ordering::SeqCst),
            written.load(Ordering::SeqCst),
            t0.elapsed()
        );
    }
}
