use std::ffi::{OsStr, c_void};
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Component, Path, PathBuf, Prefix};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, ERROR_SUCCESS, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE,
    LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, EqualSid,
    GetAce, GetSecurityDescriptorControl, GetTokenInformation, INHERIT_ONLY_ACE,
    OBJECT_INHERIT_ACE, OWNER_SECURITY_INFORMATION, PSID, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateDirectoryW, CreateFileW, FILE_ALL_ACCESS,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAG_WRITE_THROUGH,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TYPE_DISK,
    GetDriveTypeW, GetFileInformationByHandle, GetFileType, MOVEFILE_REPLACE_EXISTING,
    MOVEFILE_WRITE_THROUGH, MoveFileExW, OPEN_ALWAYS, OPEN_EXISTING, READ_CONTROL,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;

struct LocalMemory(*mut c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // LocalFree also accepts a null pointer.
        unsafe { LocalFree(self.0) };
    }
}

struct User {
    token_info: Vec<usize>,
}

impl User {
    fn current() -> io::Result<Self> {
        let mut token = null_mut();
        // The process pseudo-handle does not require cleanup.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = unsafe { OwnedHandle::from_raw_handle(token) };
        let mut size = 0;
        unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut size) };
        if (size as usize) < size_of::<TOKEN_USER>() {
            return Err(io::Error::last_os_error());
        }
        let mut token_info = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        // The buffer is aligned for TOKEN_USER and retains the storage for its SID.
        if unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                token_info.as_mut_ptr().cast(),
                size,
                &mut size,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { token_info })
    }

    fn sid(&self) -> PSID {
        unsafe { (*self.token_info.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }

    fn descriptor(&self, directory: bool) -> io::Result<LocalMemory> {
        let mut sid = null_mut();
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut sid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let sid = LocalMemory(sid.cast());
        let sid = sid.0.cast::<u16>();
        let len = (0..)
            .find(|&index| unsafe { *sid.add(index) } == 0)
            .expect("Windows SID strings are nul terminated");
        let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid, len) });
        let inherit = if directory { "OICI" } else { "" };
        let sddl = format!("O:{sid}D:P(A;{inherit};FA;;;{sid})");
        let mut descriptor = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(OsStr::new(&sddl))?.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(LocalMemory(descriptor))
    }
}

fn attributes(descriptor: &LocalMemory) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    }
}

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut encoded: Vec<_> = value.encode_wide().collect();
    if encoded.contains(&0) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    encoded.push(0);
    Ok(encoded)
}

fn open(
    path: &Path,
    access: u32,
    share: u32,
    flags: u32,
    security: Option<&SECURITY_ATTRIBUTES>,
) -> io::Result<File> {
    let handle = unsafe {
        CreateFileW(
            wide(path.as_os_str())?.as_ptr(),
            access,
            share,
            security.map_or(null(), std::ptr::from_ref),
            if security.is_some() {
                OPEN_ALWAYS
            } else {
                OPEN_EXISTING
            },
            flags | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_handle(handle) })
    }
}

fn verify_type(file: &File, directory: bool) -> io::Result<()> {
    let handle = file.as_raw_handle();
    if unsafe { GetFileType(handle) } != FILE_TYPE_DISK {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || (!directory && info.nNumberOfLinks != 1)
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

fn verify_permissions(file: &File, user: &User, directory: bool) -> io::Result<()> {
    let mut owner = null_mut();
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let descriptor = LocalMemory(descriptor);
    if owner.is_null() || dacl.is_null() || unsafe { EqualSid(owner, user.sid()) } == 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let mut control = 0;
    let mut revision = 0;
    if unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SQLite derives journal permissions from the leaf directory. Regular files can
    // therefore carry their single ACE either explicitly or through inheritance.
    if (directory && control & SE_DACL_PROTECTED == 0) || unsafe { (*dacl).AceCount } != 1 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let mut ace = null_mut();
    if unsafe { GetAce(dacl, 0, &mut ace) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let header = unsafe { &*ace.cast::<ACE_HEADER>() };
    const ACCESS_ALLOWED: u8 = 0;
    let inherit = (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8;
    if header.AceType != ACCESS_ALLOWED
        || usize::from(header.AceSize) < size_of::<ACCESS_ALLOWED_ACE>()
        || header.AceFlags & INHERIT_ONLY_ACE as u8 != 0
        || (directory && header.AceFlags & inherit != inherit)
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let ace = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    let sid = std::ptr::from_ref(&ace.SidStart).cast_mut().cast();
    if ace.Mask != FILE_ALL_ACCESS || unsafe { EqualSid(sid, user.sid()) } == 0 {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

/// Absolute cache operations retain delete-denying directory handles while
/// they use the path. No ancestor reparse point can redirect SQLite's opens.
pub(super) struct DirectoryPin {
    _directories: Vec<File>,
}

fn drive_and_components(path: &Path) -> io::Result<(String, Vec<&OsStr>)> {
    let mut parts = path.components();
    let letter = match parts.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return Err(io::ErrorKind::Unsupported.into()),
        },
        _ => return Err(io::ErrorKind::InvalidInput.into()),
    };
    if parts.next() != Some(Component::RootDir) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut names = Vec::new();
    for part in parts {
        let Component::Normal(name) = part else {
            return Err(io::ErrorKind::InvalidInput.into());
        };
        // Extended names would otherwise allow alternate data streams and separators.
        if name.encode_wide().any(|c| matches!(c, 0 | 47 | 58 | 92)) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        names.push(name);
    }
    if names.is_empty() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok((format!("{}:\\", char::from(letter)), names))
}

pub(super) fn verbatim_path(path: &Path) -> io::Result<PathBuf> {
    let (drive, names) = drive_and_components(path)?;
    require_fixed_drive(&drive)?;
    let mut path = PathBuf::from(format!("\\\\?\\{drive}"));
    path.extend(names);
    Ok(path)
}

fn require_fixed_drive(drive: &str) -> io::Result<()> {
    if unsafe { GetDriveTypeW(wide(OsStr::new(drive))?.as_ptr()) } == DRIVE_FIXED {
        Ok(())
    } else {
        Err(io::ErrorKind::Unsupported.into())
    }
}

impl DirectoryPin {
    pub(super) fn open(path: &Path, create: bool) -> io::Result<Self> {
        let (drive, names) = drive_and_components(path)?;
        require_fixed_drive(&drive)?;
        let mut ancestor = PathBuf::from(format!("\\\\?\\{drive}"));
        let mut directories = vec![open_directory(&ancestor)?];
        let user = User::current()?;
        let depth = names.len();
        for (index, name) in names.into_iter().enumerate() {
            ancestor.push(name);
            let leaf = index + 1 == depth;
            let file = match open_directory(&ancestor) {
                Err(error) if create && leaf && error.kind() == io::ErrorKind::NotFound => {
                    let security = user.descriptor(true)?;
                    let result = unsafe {
                        CreateDirectoryW(
                            wide(ancestor.as_os_str())?.as_ptr(),
                            &attributes(&security),
                        )
                    };
                    if result == 0 {
                        let error = io::Error::last_os_error();
                        if error.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32) {
                            return Err(error);
                        }
                    }
                    open_directory(&ancestor)?
                }
                result => result?,
            };
            if leaf {
                verify_permissions(&file, &user, true)?;
            }
            directories.push(file);
        }
        Ok(Self {
            _directories: directories,
        })
    }
}

fn open_directory(path: &Path) -> io::Result<File> {
    let file = open(
        path,
        FILE_READ_ATTRIBUTES | READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        FILE_FLAG_BACKUP_SEMANTICS,
        None,
    )?;
    verify_type(&file, true)?;
    Ok(file)
}

pub(crate) fn private_directory(path: &Path, create: bool) -> io::Result<()> {
    DirectoryPin::open(path, create).map(drop)
}

pub(super) fn private_file(path: &Path, create: bool, truncate: bool) -> io::Result<File> {
    let user = User::current()?;
    let descriptor = if create {
        Some(user.descriptor(false)?)
    } else {
        None
    };
    let security = descriptor.as_ref().map(attributes);
    let access = GENERIC_READ | READ_CONTROL | if create { GENERIC_WRITE } else { 0 };
    let file = open(
        path,
        access,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        FILE_ATTRIBUTE_NORMAL | if create { FILE_FLAG_WRITE_THROUGH } else { 0 },
        security.as_ref(),
    )?;
    verify_type(&file, false)?;
    verify_permissions(&file, &user, false)?;
    if truncate {
        file.set_len(0)?;
    }
    Ok(file)
}

pub(super) fn replace_file(from: &Path, to: &Path, replace: bool) -> io::Result<()> {
    let from = wide(from.as_os_str())?;
    let to = wide(to.as_os_str())?;
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), flags) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::Control;
    use crate::ledger::Ledger;
    use std::fs;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let mut random = [0u8; 8];
            getrandom::fill(&mut random).unwrap();
            let id = u64::from_ne_bytes(random);
            let path = std::env::temp_dir().join(format!("ivy-usage-acl-{id:016x}"));
            fs::create_dir(&path).unwrap();
            Self(verbatim_path(&path).unwrap())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn cache_creation_refuses_unprotected_acl_and_multiply_linked_files() {
        let fixture = Fixture::new();
        let ordinary = fixture.0.join("ordinary");
        fs::create_dir(&ordinary).unwrap();
        assert_eq!(
            private_directory(&ordinary, false).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );

        let cache = fixture.0.join("private");
        let pin = DirectoryPin::open(&cache, true).unwrap();
        let landmark = cache.join("owned");
        drop(private_file(&landmark, true, false).unwrap());
        let alias = cache.join("alias");
        fs::hard_link(&landmark, &alias).unwrap();
        assert_eq!(
            private_file(&landmark, true, true).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );

        let replacement = fixture.0.join("replacement");
        assert!(fs::rename(&fixture.0, replacement).is_err());
        drop(pin);
    }

    #[test]
    fn clear_waits_for_readers_and_recovers_quarantine_without_marker() {
        let fixture = Fixture::new();
        let first = Control::new(fixture.0.join("store"));
        let writer = first.owner().unwrap();
        let database = first.database(true).unwrap();
        let ledger = Ledger::open_writer(&database).unwrap();
        let store_id = ledger.info().unwrap().store_id;
        drop(ledger);
        drop(writer);

        let follower = Control::new(fixture.0.join("store"));
        let lease = follower.reader(false).unwrap();
        let ledger = Ledger::open_reader(&follower.database(false).unwrap()).unwrap();
        assert_eq!(ledger.info().unwrap().store_id, store_id);
        drop(ledger);
        first.begin_clear().unwrap();
        assert!(first.pending_clear().unwrap());
        assert_eq!(
            first.complete_clear().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(lease);

        // Model a completed directory move followed by an interrupted delete.
        let store = fixture.0.join("store");
        let quarantine = store.join("ledger.pending-delete");
        replace_file(database.parent().unwrap(), &quarantine, false).unwrap();
        fs::remove_file(store.join("clear.pending")).unwrap();
        assert!(follower.pending_clear().unwrap());
        follower.complete_clear().unwrap();
        assert!(!first.pending_clear().unwrap());
        assert!(!database.exists());
        assert!(!quarantine.exists());
        assert!(!first.read().unwrap().enabled);
    }
}
