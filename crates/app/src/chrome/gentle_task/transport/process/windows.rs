//! Own every descendant even if the executable that started it has already exited.
use std::io;
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use std::ptr::null;
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
};

fn owned(raw: HANDLE) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: only newly created, valid kernel handles are passed in.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

fn create_job() -> io::Result<OwnedHandle> {
    // SAFETY: null requests an unnamed job with non-inheritable default security.
    let job = owned(unsafe { CreateJobObjectW(null(), null()) })?;
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: limits is initialized and its size matches the selected information class.
    if unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(job)
}

fn resume(child: &Child) -> io::Result<()> {
    // Stable std::process retains the process handle but does not expose its primary thread.
    // CREATE_SUSPENDED prevents the launcher from running or starting descendants before assignment.
    // SAFETY: this snapshot is owned here; the thread record declares its initialized size.
    unsafe {
        let snapshot = owned(CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0))?;
        let mut record = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut entry = Thread32First(snapshot.as_raw_handle(), &mut record);
        while entry != 0 {
            if record.th32OwnerProcessID == child.id() {
                let thread = owned(OpenThread(THREAD_SUSPEND_RESUME, 0, record.th32ThreadID))?;
                if ResumeThread(thread.as_raw_handle()) == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                return Ok(());
            }
            entry = Thread32Next(snapshot.as_raw_handle(), &mut record);
        }
        Err(io::Error::last_os_error())
    }
}

pub(super) fn spawn(command: &mut Command) -> io::Result<(Child, OwnedHandle)> {
    let job = create_job()?;
    let mut child = command
        .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
        .spawn()?;
    // SAFETY: both handles are live and retained until after assignment and resume.
    let assigned = unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) };
    let result = if assigned == 0 {
        Err(io::Error::last_os_error())
    } else {
        resume(&child)
    };
    if let Err(error) = result {
        if let Err(cleanup) = super::stop_and_reap(&mut child) {
            crate::pty_output::say(&format!(
                "Gentle startup failed and the suspended process could not be retired: {cleanup}"
            ));
        }
        return Err(error);
    }
    Ok((child, job))
}

pub(super) fn terminate(job: &OwnedHandle) -> io::Result<()> {
    // SAFETY: the caller owns the job; closing its last handle is a second termination path.
    if unsafe { TerminateJobObject(job.as_raw_handle(), 1) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;
    use std::thread;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
    };

    const ROLE: &str = "IVY_TEST_GENTLE_WINDOWS_JOB_ROLE";
    const PID_FILE: &str = "IVY_TEST_GENTLE_WINDOWS_JOB_PID_FILE";

    fn fixture(role: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("windows::tests::windows_job_helper")
            .env(ROLE, role)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    #[test]
    fn windows_job_helper() {
        let Ok(role) = std::env::var(ROLE) else {
            return;
        };
        if role != "descendant" {
            let child = fixture("descendant").spawn().unwrap();
            std::fs::write(std::env::var_os(PID_FILE).unwrap(), child.id().to_string()).unwrap();
            // Release the launcher's own handle so the test observes only job ownership.
            drop(OwnedHandle::from(child));
            if role == "exit" {
                return;
            }
        }
        thread::sleep(Duration::from_secs(120));
    }

    fn exit_status(child: &mut Child) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "Windows child did not exit");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn stopping_a_job_retires_children_with_a_live_or_exited_launcher() {
        for role in ["stay", "exit"] {
            let pid_file = std::env::temp_dir()
                .join(format!("ivy-gentle-job-{}-{role}.pid", std::process::id()));
            let _ = std::fs::remove_file(&pid_file);
            let (mut launcher, job) = spawn(fixture(role).env(PID_FILE, &pid_file)).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            let pid = loop {
                if let Some(pid) = std::fs::read_to_string(&pid_file)
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    break pid;
                }
                assert!(
                    Instant::now() < deadline,
                    "launcher never published its child"
                );
                thread::sleep(Duration::from_millis(10));
            };
            std::fs::remove_file(pid_file).unwrap();
            // SAFETY: the test opens only its own helper for observation and retains the handle.
            let descendant = owned(unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) }).unwrap();
            let wait = |ms| unsafe { WaitForSingleObject(descendant.as_raw_handle(), ms) };
            if role == "exit" {
                assert!(exit_status(&mut launcher).success());
                assert_eq!(wait(0), WAIT_TIMEOUT);
                drop(job);
            } else {
                assert!(launcher.try_wait().unwrap().is_none());
                assert_eq!(wait(0), WAIT_TIMEOUT);
                terminate(&job).unwrap();
            }
            assert_eq!(wait(5000), WAIT_OBJECT_0);
            exit_status(&mut launcher);
        }
    }
}
