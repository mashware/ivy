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

pub(super) fn parent(handle: BorrowedHandle<'_>) -> io::Result<u32> {
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
    let query: Query = unsafe { std::mem::transmute(address) };
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

fn chain_identity(id: ProcessIdentity) -> super::chain::Identity {
    super::chain::Identity {
        pid: id.pid,
        birth: id.birth,
    }
}

pub(super) fn classify(scope: &Scope, first: OwnedHandle) -> Ancestry {
    use super::chain::{Outcome, walk};
    if !scope.is_open() {
        return Ancestry::Unknown;
    }
    let outcome = walk(
        first,
        chain_identity(scope.root_identity),
        scope.anchor_identity.map(chain_identity),
        |handle| {
            if !process::is_alive(handle.as_handle()) {
                return None;
            }
            process::identity(handle.as_handle())
                .ok()
                .map(chain_identity)
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
