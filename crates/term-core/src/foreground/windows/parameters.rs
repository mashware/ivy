//! Best-effort native x64 parameter inspection. Remote process memory is untrusted
//! and these observations must never confer foreground or launch authority.
use super::memory::{Memory, Stream, read_verified};
use super::{arguments, process};
use std::ffi::c_void;
use std::io;
use std::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle};
use std::path::{Path, PathBuf};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleA(name: *const u8) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn GetCurrentProcess() -> *mut c_void;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn ReadProcessMemory(
        process: *mut c_void,
        address: *const c_void,
        buffer: *mut c_void,
        len: usize,
        read: *mut usize,
    ) -> i32;
}
const QUERY: u32 = 0x1000;
const VM_READ: u32 = 0x0010;
const SYNCHRONIZE: u32 = 0x0010_0000;
// These offsets are restricted to a native AMD64 target and may change with Windows.
const PEB_PARAMETERS: usize = 0x20;
const CURRENT_DIRECTORY: usize = 0x38;
const COMMAND_LINE: usize = 0x70;

fn invalid() -> io::Error {
    io::ErrorKind::InvalidData.into()
}
fn unsupported() -> io::Error {
    io::ErrorKind::Unsupported.into()
}
fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}

unsafe fn symbol(name: &'static [u8]) -> io::Result<*mut c_void> {
    let module = unsafe { GetModuleHandleA(c"ntdll.dll".as_ptr().cast()) };
    let p = unsafe { GetProcAddress(module, name.as_ptr()) };
    if p.is_null() {
        Err(unsupported())
    } else {
        Ok(p)
    }
}
fn architecture(handle: BorrowedHandle<'_>) -> io::Result<()> {
    if !cfg!(target_arch = "x86_64") {
        return Err(unsupported());
    }
    let module = unsafe { GetModuleHandleA(c"kernel32.dll".as_ptr().cast()) };
    let p = unsafe { GetProcAddress(module, c"IsWow64Process2".as_ptr().cast()) };
    if p.is_null() {
        return Err(unsupported());
    }
    let query: unsafe extern "system" fn(*mut c_void, *mut u16, *mut u16) -> i32 =
        unsafe { std::mem::transmute(p) };
    for h in [unsafe { GetCurrentProcess() }, handle.as_raw_handle()] {
        let (mut process, mut native) = (0, 0);
        if unsafe { query(h, &mut process, &mut native) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if !supported_machine(process, native) {
            return Err(unsupported());
        }
    }
    Ok(())
}
fn supported_machine(process: u16, native: u16) -> bool {
    process == 0 && native == 0x8664
}
#[repr(C)]
struct Basic {
    reserved1: *mut c_void,
    peb: *mut c_void,
    reserved2: [*mut c_void; 2],
    pid: usize,
    parent: *mut c_void,
}
struct Reader {
    handle: OwnedHandle,
}
impl Memory for Reader {
    fn read(&self, address: usize, buffer: &mut [u8]) -> io::Result<()> {
        if address == 0 || address.checked_add(buffer.len()).is_none() {
            return Err(invalid());
        }
        let mut read = 0;
        if unsafe {
            ReadProcessMemory(
                self.handle.as_raw_handle(),
                address as _,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut read,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if read != buffer.len() {
            return Err(invalid());
        }
        Ok(())
    }
}
impl Reader {
    fn pointer(&self, address: usize) -> io::Result<usize> {
        let mut b = [0; 8];
        self.read(address, &mut b)?;
        let value = usize::try_from(u64::from_le_bytes(b)).map_err(|_| invalid())?;
        if value == 0 || !value.is_multiple_of(8) {
            return Err(invalid());
        }
        Ok(value)
    }
    fn parameters(&self) -> io::Result<usize> {
        let f: unsafe extern "system" fn(*mut c_void, u32, *mut c_void, u32, *mut u32) -> i32 = unsafe {
            std::mem::transmute(symbol(c"NtQueryInformationProcess".to_bytes_with_nul())?)
        };
        let mut basic: Basic = unsafe { std::mem::zeroed() };
        let status = unsafe {
            f(
                self.handle.as_raw_handle(),
                0,
                (&mut basic as *mut Basic).cast(),
                std::mem::size_of::<Basic>() as u32,
                std::ptr::null_mut(),
            )
        };
        if status < 0 {
            return Err(denied());
        }
        let peb = basic.peb as usize;
        if peb == 0 || !peb.is_multiple_of(8) {
            return Err(invalid());
        }
        self.pointer(peb.checked_add(PEB_PARAMETERS).ok_or_else(invalid)?)
    }
}
fn inspect<T>(
    original: BorrowedHandle<'_>,
    offset: usize,
    read: impl FnOnce(&mut Stream<'_, Reader>) -> io::Result<T>,
) -> io::Result<T> {
    let identity = process::identity(original)?;
    let executable = process::executable(original)?;
    if !process::is_alive(original) {
        return Err(denied());
    }
    architecture(original)?;
    let h = unsafe { OpenProcess(QUERY | VM_READ | SYNCHRONIZE, 0, identity.pid) };
    if h.is_null() {
        return Err(io::Error::last_os_error());
    }
    let reader = Reader {
        handle: unsafe { OwnedHandle::from_raw_handle(h) },
    };
    if process::identity(reader.handle.as_handle())? != identity
        || !process::is_alive(reader.handle.as_handle())
        || process::executable(reader.handle.as_handle())? != executable
    {
        return Err(denied());
    }
    let parameters = reader.parameters()?;
    let address = parameters.checked_add(offset).ok_or_else(invalid)?;
    let value = read_verified(&reader, address, read)?;
    if reader.parameters()? != parameters {
        return Err(invalid());
    }
    if process::identity(original)? != identity
        || !process::is_alive(original)
        || !process::is_alive(reader.handle.as_handle())
        || process::identity(reader.handle.as_handle())? != identity
        || process::executable(original)? != executable
        || process::executable(reader.handle.as_handle())? != executable
    {
        return Err(denied());
    }
    Ok(value)
}
pub(super) fn working_directory(handle: BorrowedHandle<'_>) -> io::Result<PathBuf> {
    inspect(handle, CURRENT_DIRECTORY, |stream| {
        let mut units = Vec::new();
        while let Some(unit) = arguments::Units::next_unit(stream)? {
            units.push(unit);
        }
        let text = String::from_utf16(&units).map_err(|_| invalid())?;
        let path = PathBuf::from(text);
        if !path.is_absolute() {
            return Err(invalid());
        }
        Ok(path)
    })
}
pub(super) fn matches_arguments(
    handle: BorrowedHandle<'_>,
    shapes: &[&[crate::foreground::Argument<'_>]],
) -> io::Result<bool> {
    inspect(handle, COMMAND_LINE, |stream| {
        arguments::matches(stream, shapes)
    })
}
pub(super) fn runs_script(handle: BorrowedHandle<'_>, expected: &Path) -> io::Result<bool> {
    if !expected.is_absolute() {
        return Ok(false);
    }
    let Ok(expected) = std::fs::canonicalize(expected) else {
        return Ok(false);
    };
    inspect(handle, COMMAND_LINE, |stream| {
        let Some(slot) = arguments::script_slot(stream)? else {
            return Ok(false);
        };
        let candidate = PathBuf::from(slot);
        if !candidate.is_absolute() {
            return Ok(false);
        }
        Ok(std::fs::canonicalize(candidate).is_ok_and(|path| path == expected))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_wow64_and_non_x64_layouts() {
        assert!(supported_machine(0, 0x8664));
        assert!(!supported_machine(0x14c, 0x8664));
        assert!(!supported_machine(0, 0xaa64));
    }
    #[test]
    fn current_process_directory_and_arguments_agree_with_runtime() {
        const CHILD: &str = "IVY_TEST_NATIVE_PARAMETER_CHILD";
        const MARKER: &str = "IVY_TEST_NATIVE_PARAMETER_MARKER";
        if std::env::var_os(CHILD).is_none() {
            use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir()
                .join(format!("ivy-parameters-猫-{}-{unique}", std::process::id()));
            std::fs::create_dir(&directory).unwrap();
            let marker = directory.join("checked");
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "foreground::windows::parameters::tests::current_process_directory_and_arguments_agree_with_runtime", "hola 世界", r#"a\\"b"#, ""])
                .env(CHILD, "1").env(MARKER, &marker).current_dir(&directory)
                .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                .spawn().unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("native argument fixture exceeded its deadline");
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            assert!(
                status.success(),
                "native parameter fixture failed: {status}"
            );
            assert_eq!(std::fs::read_to_string(&marker).unwrap(), "checked");
            std::fs::remove_dir_all(directory).unwrap();
            return;
        }
        let values: Vec<String> = std::env::args().skip(1).collect();
        assert!(values.iter().any(|v| v == "hola 世界"));
        assert!(values.iter().any(|v| v == r#"a\\"b"#));
        assert!(values.iter().any(|v| v.is_empty()));
        let handle = process::open(std::process::id()).unwrap();
        assert_eq!(
            working_directory(handle.as_handle()).unwrap(),
            std::env::current_dir().unwrap()
        );
        let values: Vec<String> = std::env::args().skip(1).collect();
        let shape: Vec<_> = values
            .iter()
            .map(|s| crate::foreground::Argument::Literal(s))
            .collect();
        assert!(matches_arguments(handle.as_handle(), &[&shape]).unwrap());
        std::fs::write(std::env::var_os(MARKER).unwrap(), "checked").unwrap();
    }
}
