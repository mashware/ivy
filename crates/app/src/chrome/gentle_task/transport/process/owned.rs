#[cfg(windows)]
use super::windows;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

pub(super) struct Owned {
    pub(super) child: Child,
    #[cfg(windows)]
    pub(super) job: std::os::windows::io::OwnedHandle,
}

impl Owned {
    /// The PID stays reserved until retirement can signal the client's whole process group.
    #[cfg(unix)]
    pub(super) fn exited(&self) -> std::io::Result<bool> {
        // SAFETY: waitid writes a siginfo record and WNOWAIT preserves our child.
        unsafe {
            let mut info: libc::siginfo_t = std::mem::zeroed();
            let result = libc::waitid(
                libc::P_PID,
                self.child.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            );
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    return Ok(false);
                }
                return Err(error);
            }
            Ok(info.si_pid() != 0)
        }
    }

    #[cfg(windows)]
    pub(super) fn exited(&mut self) -> std::io::Result<bool> {
        self.child.try_wait().map(|status| status.is_some())
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let pid = self.child.id() as i32;
            // The official launcher forwards termination to Pi. Sending it directly to both
            // would deliver two signals and force Pi to skip graceful shutdown.
            // SAFETY: the unreaped PID belongs to our launcher.
            let result = unsafe { libc::kill(pid, libc::SIGTERM) };
            report_signal_error(result);
            // A launcher can exit before the client or its pipes. Keep its PID reserved
            // through the grace period, then reach any surviving group members.
            thread::sleep(Duration::from_millis(750));
            // SAFETY: this child has not been reaped, so its process group ID cannot be recycled.
            let result = unsafe { libc::kill(-pid, libc::SIGKILL) };
            report_signal_error(result);
        }
        #[cfg(windows)]
        if let Err(error) = windows::terminate(&self.job) {
            crate::pty_output::say(&format!("Could not terminate the Gentle task job: {error}"));
        }
        if let Err(error) = stop_and_reap(&mut self.child) {
            crate::pty_output::say(&format!(
                "Gentle task cleanup could not confirm exit: {error}"
            ));
        }
    }
}

#[cfg(unix)]
fn report_signal_error(result: libc::c_int) {
    if result == -1 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            crate::pty_output::say(&format!(
                "Could not signal the Gentle task process: {error}"
            ));
        }
    }
}

pub(super) fn stop_and_reap(child: &mut Child) -> std::io::Result<()> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    let kill = child.kill();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(kill.err().unwrap_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "the client did not exit before its retirement deadline",
                )
            }));
        }
        thread::sleep(Duration::from_millis(10));
    }
}
