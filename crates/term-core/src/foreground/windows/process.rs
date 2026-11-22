use super::ProcessIdentity;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::{AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::path::PathBuf;
use windows_sys::Win32::Foundation::{FILETIME, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{
    GetProcessId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, WaitForSingleObject,
};

pub(super) fn open(pid: u32) -> io::Result<OwnedHandle> {
    // SAFETY: OpenProcess takes only values and returns a new owned handle.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | 0x0010_0000, 0, pid) };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful OpenProcess returns a uniquely owned handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}

pub(super) fn identity(handle: BorrowedHandle<'_>) -> io::Result<ProcessIdentity> {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: the borrowed handle remains valid across both queries.
    let pid = unsafe { GetProcessId(handle.as_raw_handle()) };
    if pid == 0
        || unsafe {
            GetProcessTimes(
                handle.as_raw_handle(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(ProcessIdentity {
        pid,
        birth: (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
    })
}

pub(super) fn is_alive(handle: BorrowedHandle<'_>) -> bool {
    // SAFETY: the process handle remains valid for this nonblocking wait.
    unsafe { WaitForSingleObject(handle.as_raw_handle(), 0) == WAIT_TIMEOUT }
}

pub(super) fn executable(handle: BorrowedHandle<'_>) -> io::Result<PathBuf> {
    let mut buf = vec![0u16; 32768];
    let mut size = buf.len() as u32;
    // SAFETY: the buffer has size writable UTF-16 units and the handle is borrowed.
    if unsafe { QueryFullProcessImageNameW(handle.as_raw_handle(), 0, buf.as_mut_ptr(), &mut size) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    buf.truncate(size as usize);
    let path = PathBuf::from(std::ffi::OsString::from_wide(&buf));
    if !path.is_absolute() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(path)
}
