use super::{Ancestry, ProcessIdentity, Scope, process};
use std::io;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, OwnedHandle};

#[repr(C)]
struct BasicInfo {
    reserved1: *mut std::ffi::c_void,
    peb: *mut std::ffi::c_void,
    reserved2: [*mut std::ffi::c_void; 2],
    pid: usize,
    parent: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut std::ffi::c_void;
    fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
}
type Query = unsafe extern "system" fn(
    *mut std::ffi::c_void,
    u32,
    *mut std::ffi::c_void,
    u32,
    *mut u32,
) -> i32;

fn query() -> io::Result<Query> {
    let dll: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
    // SAFETY: the DLL name is NUL-terminated and remains live for the call.
    let module = unsafe { GetModuleHandleW(dll.as_ptr()) };
    if module.is_null() {
        return Err(io::ErrorKind::Unsupported.into());
    }
    // SAFETY: ntdll is already loaded and the symbol name is NUL-terminated.
    let address = unsafe { GetProcAddress(module, c"NtQueryInformationProcess".as_ptr().cast()) };
    if address.is_null() {
        return Err(io::ErrorKind::Unsupported.into());
    }
    // SAFETY: this symbol has the documented NtQueryInformationProcess ABI.
    Ok(unsafe { std::mem::transmute::<*mut std::ffi::c_void, Query>(address) })
}

pub(super) fn sequence(handle: BorrowedHandle<'_>) -> io::Result<u64> {
    let query = query()?;
    let mut sequence = 0u64;
    let mut written = 0u32;
    // SAFETY: class92 returns one ULONG64; the exact process handle stays borrowed.
    let status = unsafe {
        query(
            handle.as_raw_handle(),
            92,
            (&mut sequence as *mut u64).cast(),
            std::mem::size_of::<u64>() as u32,
            &mut written,
        )
    };
    if status < 0 {
        return Err(io::ErrorKind::Unsupported.into());
    }
    if written != std::mem::size_of::<u64>() as u32 || sequence == 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(sequence)
}

pub(super) fn parent(handle: BorrowedHandle<'_>) -> io::Result<u32> {
    let query = query()?;
    // SAFETY: zero is a valid initial value for every field of this output structure.
    let mut info: BasicInfo = unsafe { std::mem::zeroed() };
    // SAFETY: the process handle stays borrowed and info is writable for its declared size.
    let status = unsafe {
        query(
            handle.as_raw_handle(),
            0,
            (&mut info as *mut BasicInfo).cast(),
            std::mem::size_of::<BasicInfo>() as u32,
            std::ptr::null_mut(),
        )
    };
    if status < 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    u32::try_from(info.parent).map_err(|_| io::ErrorKind::InvalidData.into())
}

fn chain_identity(id: ProcessIdentity, sequence: u64) -> super::chain::Identity {
    super::chain::Identity {
        pid: id.pid,
        birth: id.birth,
        sequence,
    }
}

pub(super) fn classify(scope: &Scope, first: OwnedHandle) -> Ancestry {
    use super::chain::{Outcome, walk};
    if !scope.is_open() {
        return Ancestry::Unknown;
    }
    let Some(root_sequence) = scope.root_sequence else {
        return Ancestry::Unknown;
    };
    let outcome = walk(
        first,
        chain_identity(scope.root_identity, root_sequence),
        scope
            .anchor_identity
            .zip(scope.anchor_sequence)
            .map(|(id, sequence)| chain_identity(id, sequence)),
        |handle| {
            if !process::is_alive(handle.as_handle()) {
                return None;
            }
            process::identity(handle.as_handle()).ok().and_then(|id| {
                sequence(handle.as_handle())
                    .ok()
                    .map(|sequence| chain_identity(id, sequence))
            })
        },
        |handle| parent(handle.as_handle()).ok(),
        |pid| process::open(pid).ok(),
    );
    if !scope.is_open() {
        return Ancestry::Unknown;
    }
    if outcome == Outcome::Outside
        && let (Some(anchor), Some(id)) = (&scope.anchor, scope.anchor_identity)
        && (!process::is_alive(anchor.as_handle())
            || process::identity(anchor.as_handle()).ok() != Some(id)
            || parent(scope.root.as_handle()).ok() != Some(id.pid))
    {
        return Ancestry::Unknown;
    }
    if !scope.is_open() {
        return Ancestry::Unknown;
    }
    match outcome {
        Outcome::Inside => Ancestry::Inside,
        Outcome::Outside => Ancestry::Outside,
        Outcome::Unknown => Ancestry::Unknown,
    }
}

#[cfg(test)]
pub(super) fn telemetry_sequence(handle: BorrowedHandle<'_>) -> u64 {
    #[repr(C)]
    struct Telemetry {
        header_size: u32,
        pid: u32,
        start_key: u64,
        create_time: u64,
        interrupt_time: u64,
        unbiased_time: u64,
        sequence: u64,
        remaining: [u64; 8192],
    }
    // Only this fixture's known test process is queried. Variable strings are never decoded/logged.
    let mut telemetry: Telemetry = unsafe { std::mem::zeroed() };
    let mut written = 0;
    // SAFETY: the whole aligned output buffer is writable; class64's documented header is fixed.
    let status = unsafe {
        query().unwrap()(
            handle.as_raw_handle(),
            64,
            (&mut telemetry as *mut Telemetry).cast(),
            std::mem::size_of::<Telemetry>() as u32,
            &mut written,
        )
    };
    assert_eq!(status, 0);
    assert!(written >= 48 && telemetry.header_size >= 48);
    assert_eq!(telemetry.pid, process::identity(handle).unwrap().pid);
    assert_eq!(
        telemetry.create_time,
        process::identity(handle).unwrap().birth
    );
    assert!(telemetry.interrupt_time > 0);
    telemetry.sequence
}
