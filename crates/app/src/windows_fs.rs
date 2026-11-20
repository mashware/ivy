//! The two facts about an open file that std's Windows `MetadataExt` keeps behind an unstable
//! feature (windows_by_handle, D108). Self-contained on purpose: it depends on nothing in this
//! crate, so it can be type-checked for Windows on its own where `app` cannot be cross-compiled.

use std::fs::File;
use std::io;
use std::os::windows::io::AsRawHandle as _;

use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_TAG_INFO, FileAttributeTagInfo,
    GetFileInformationByHandle, GetFileInformationByHandleEx,
};

/// The bit a reparse tag carries when it names another file: symbolic links and junctions. The same
/// rule std's is_symlink applies on Windows.
const NAME_SURROGATE: u32 = 0x2000_0000;

/// Which file this is: its volume serial number and its 64-bit file index, the pair Windows
/// promises unique while the file is open — what `dev` and `ino` are on Unix.
pub(crate) fn identity(file: &File) -> io::Result<(u64, u64)> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle is owned by `file` and outlives the call; the out-pointer is a live local.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    Ok((u64::from(information.dwVolumeSerialNumber), index))
}

/// Whether the file was opened on a link rather than through it: only meaningful for a handle
/// opened with `FILE_FLAG_OPEN_REPARSE_POINT`, which leaves the link itself as the open file.
pub(crate) fn is_name_surrogate(file: &File) -> io::Result<bool> {
    let mut information = FILE_ATTRIBUTE_TAG_INFO::default();
    // SAFETY: as above; the size passed is the size of the buffer the pointer names.
    let answered = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileAttributeTagInfo,
            (&raw mut information).cast(),
            std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if answered == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(information.ReparseTag & NAME_SURROGATE != 0)
}
