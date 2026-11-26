//! Relay Claude's cancellation to the original statusline and its ordinary shell children.

use std::io;
use std::mem::MaybeUninit;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicI32, Ordering};

static DELEGATE_GROUP: AtomicI32 = AtomicI32::new(0);
const CANCELLATION: [libc::c_int; 3] = [libc::SIGTERM, libc::SIGINT, libc::SIGHUP];

extern "C" fn cancel(signal: libc::c_int) {
    let group = DELEGATE_GROUP.load(Ordering::Relaxed);
    // SAFETY: kill and raise are async-signal-safe. SA_RESETHAND makes the queued
    // signal terminate the wrapper normally when this handler returns.
    unsafe {
        if group > 0 {
            libc::kill(-group, signal);
        }
        libc::raise(signal);
    }
}

pub(super) fn spawn(mut child: Command) -> io::Result<Child> {
    let blocked = Blocked::install()?;
    let child_mask = blocked.previous;
    let parent = std::process::id();
    child.process_group(0);
    // SAFETY: the child callback uses only native calls before exec. The parent comparison
    // covers termination between fork and setting the immediate child's death signal.
    unsafe {
        child.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) < 0
                || libc::sigprocmask(libc::SIG_SETMASK, &child_mask, std::ptr::null_mut()) < 0
            {
                return Err(io::Error::last_os_error());
            }
            if libc::getppid() as u32 != parent {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            Ok(())
        });
    }
    let child = child.spawn()?;
    DELEGATE_GROUP.store(child.id() as i32, Ordering::Relaxed);
    drop(blocked);
    Ok(child)
}

pub(super) fn wait(child: &mut Child) -> io::Result<ExitStatus> {
    loop {
        let mut status = MaybeUninit::<libc::siginfo_t>::uninit();
        // SAFETY: use the owned child's ID, and keep it waitable until the signal handler
        // no longer targets its process group. The output is not inspected.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                status.as_mut_ptr(),
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if result == 0 {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            DELEGATE_GROUP.store(0, Ordering::Relaxed);
            return Err(error);
        }
    }
    DELEGATE_GROUP.store(0, Ordering::Relaxed);
    // Bounded: waitid without WNOHANG observed the exit and WNOWAIT left it ready for reaping.
    child.wait()
}

struct Blocked {
    previous: libc::sigset_t,
}

impl Blocked {
    fn install() -> io::Result<Self> {
        // SAFETY: sigemptyset initializes each mask before it is used. pthread_sigmask
        // initializes the previous mask on success, with all storage owned here.
        unsafe {
            let mut mask: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut mask);
            for signal in CANCELLATION {
                libc::sigaddset(&mut mask, signal);
            }
            let mut previous: libc::sigset_t = std::mem::zeroed();
            let result = libc::pthread_sigmask(libc::SIG_BLOCK, &mask, &mut previous);
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result));
            }
            let blocked = Self { previous };
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = cancel as *const () as usize;
            action.sa_flags = libc::SA_RESETHAND;
            action.sa_mask = mask;
            for signal in CANCELLATION {
                if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(blocked)
        }
    }
}

impl Drop for Blocked {
    fn drop(&mut self) {
        // SAFETY: restore the initialized mask on the same thread that blocked it.
        unsafe {
            libc::pthread_sigmask(libc::SIG_SETMASK, &self.previous, std::ptr::null_mut());
        }
    }
}
