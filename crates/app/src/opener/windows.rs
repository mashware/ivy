//! The Windows desktop, through the shell's own calls: no command line exists for `cmd` to read
//! `& ^ % !` in, and no `explorer /select,` argument for a comma in a name to split.
use super::Call;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows_sys::Win32::UI::Shell::Common::ITEMIDLIST;
use windows_sys::Win32::UI::Shell::{
    ILCreateFromPathW, ILFree, SEE_MASK_FLAG_NO_UI, SEE_MASK_INVOKEIDLIST, SEE_MASK_NOASYNC,
    SHELLEXECUTEINFOW, SHOpenFolderAndSelectItems, ShellExecuteExW,
};

/// Show normally, named here rather than pulling in the window-messaging bindings for one number.
const SHOW_NORMAL: i32 = 1;

pub(super) fn run(call: &Call) -> Result<(), String> {
    // The shell's calls need COM on the calling thread; this thread is the click's own.
    // SAFETY: the reserved pointer is null, as the call requires.
    let com = unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    };
    let done = match call {
        Call::Open(path) => open(path),
        Call::Reveal(path) => reveal(path),
    };
    if com >= 0 {
        // SAFETY: balanced with the successful initialisation above, on the same thread.
        unsafe { CoUninitialize() };
    }
    done
}

/// The shell's item for `path`: it binds the file that exists, rather than a string the shell
/// would search for.
struct Item(*mut ITEMIDLIST);

impl Item {
    fn of(path: &Path) -> Result<Self, String> {
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string that outlives the call.
        let list = unsafe { ILCreateFromPathW(wide.as_ptr()) };
        if list.is_null() {
            Err("the shell does not know this path".into())
        } else {
            Ok(Self(list))
        }
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        // SAFETY: the list came from `ILCreateFromPathW` and is freed once.
        unsafe { ILFree(self.0) };
    }
}

fn open(path: &Path) -> Result<(), String> {
    let item = Item::of(path)?;
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpIDList: item.0.cast(),
        nShow: SHOW_NORMAL,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised and its item list outlives the call.
    if unsafe { ShellExecuteExW(&mut info) } != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error().to_string())
    }
}

fn reveal(path: &Path) -> Result<(), String> {
    let item = Item::of(path)?;
    // A whole item with no children selects that item in its folder's window.
    // SAFETY: the item list is valid for the call; no children are passed.
    let result = unsafe { SHOpenFolderAndSelectItems(item.0, 0, std::ptr::null(), 0) };
    if result >= 0 {
        Ok(())
    } else {
        Err(format!("the shell answered {result:#x}"))
    }
}
