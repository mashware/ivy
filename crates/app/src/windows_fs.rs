//! What Windows needs beyond std to tell files apart and to open one without following a link
//! (D108). Self-contained on purpose: it depends on nothing in this crate, so it can be
//! type-checked for Windows on its own where `app` cannot be cross-compiled.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::os::windows::io::AsRawHandle as _;
use std::path::Path;

use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
    GetFileInformationByHandle,
};

/// The volume serial number and the 64-bit file index: what `dev` and `ino` are on Unix. std keeps
/// them behind an unstable feature. On ReFS the 64-bit index is not guaranteed unique (D108).
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

/// Opens `path` for reading unless its last component is a symbolic link or a junction.
///
/// The first open lands on the link itself, so the refusal is about the file opened, not about a
/// look taken before. Any other reparse point (deduplication, compression, a cloud placeholder)
/// has to be opened through its owner for its content to be read, so it is opened again and kept
/// only if that second open reached the same file.
pub(crate) fn open_no_follow(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    if metadata.is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a link was not followed",
        ));
    }
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
        return Ok(file);
    }
    let through = File::open(path)?;
    if identity(&through)? != identity(&file)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the file was replaced while it was being opened",
        ));
    }
    Ok(through)
}
