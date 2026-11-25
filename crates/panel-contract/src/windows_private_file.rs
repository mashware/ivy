//! Private runtime capability files on Windows.
//!
//! D92 excludes reparse-backed runtime storage; document-viewer filter hydration is a different job.
use std::fs::File;
use std::io;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;

use crate::windows_security::{OwnedHandle, descriptor, validate_acl, wide};

pub fn create_new(path: &Path) -> io::Result<File> {
    open_impl(path, true)
}

pub fn open(path: &Path) -> io::Result<File> {
    open_impl(path, false)
}

fn open_impl(path: &Path, create: bool) -> io::Result<File> {
    let security = if create {
        Some(descriptor(false)?)
    } else {
        None
    };
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.as_ref().map_or(null_mut(), |s| s.0),
        bInheritHandle: 0,
    };
    let name = wide(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE | READ_CONTROL | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            if create { &attributes } else { null() },
            if create { CREATE_NEW } else { OPEN_EXISTING },
            FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let handle = OwnedHandle(handle);
    let mut info = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
    if unsafe { GetFileType(handle.0) } != FILE_TYPE_DISK
        || unsafe { GetFileInformationByHandle(handle.0, &mut info) } == 0
        || info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "not a private regular file",
        ));
    }
    validate_acl(handle.0, path, false)?;
    let raw = handle.0;
    std::mem::forget(handle);
    Ok(unsafe { File::from_raw_handle(raw.cast()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Security::Authorization::*;

    #[test]
    fn private_file_round_trips_and_shares_delete() {
        let dir = crate::PanelDir::create().unwrap();
        let path = dir.path().join("route");
        let mut first = create_new(&path).unwrap();
        first.write_all(b"private").unwrap();
        assert_eq!(
            create_new(&path).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        let mut second = open(&path).unwrap();
        let mut content = String::new();
        second.read_to_string(&mut content).unwrap();
        assert_eq!(content, "private");
        std::fs::remove_file(&path).unwrap();
        first.seek(SeekFrom::Start(0)).unwrap();
        content.clear();
        first.read_to_string(&mut content).unwrap();
        assert_eq!(content, "private");
    }

    #[test]
    fn rejects_broad_and_null_dacls_without_repair() {
        for acl in [None, Some("D:(A;;FA;;;WD)")] {
            let dir = crate::PanelDir::create().unwrap();
            let path = dir.path().join("route");
            drop(create_new(&path).unwrap());
            crate::windows_directory::tests::set_test_dacl(&path, acl, true);
            assert!(open(&path).is_err());
            assert!(open(&path).is_err());
        }
    }

    #[test]
    fn refuses_missing_and_directory_paths() {
        let dir = crate::PanelDir::create().unwrap();
        assert!(open(&dir.path().join("missing")).is_err());
        assert!(open(dir.path()).is_err());
    }

    #[test]
    fn refuses_an_inherited_file_acl() {
        let dir = crate::PanelDir::create().unwrap();
        let path = dir.path().join("inherited");
        std::fs::write(&path, b"not protected").unwrap();
        assert!(open(&path).is_err());
    }

    #[test]
    fn fresh_file_has_protected_noninheriting_user_and_system_acl() {
        let dir = crate::PanelDir::create().unwrap();
        let file = create_new(&dir.path().join("route")).unwrap();
        let mut owner = null_mut();
        let mut acl = null_mut();
        let mut raw_descriptor = null_mut();
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle().cast(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                null_mut(),
                &mut acl,
                null_mut(),
                &mut raw_descriptor,
            )
        };
        assert_eq!(status, ERROR_SUCCESS);
        let security = crate::windows_security::OwnedDescriptor(raw_descriptor);
        let (_token, bytes) = crate::windows_security::token_user().unwrap();
        let user = unsafe { &*(bytes.as_ptr().cast::<TOKEN_USER>()) };
        assert!(!owner.is_null() && !acl.is_null());
        assert_ne!(unsafe { EqualSid(owner, user.User.Sid) }, 0);
        let mut control = 0;
        let mut revision = 0;
        assert_ne!(
            unsafe { GetSecurityDescriptorControl(security.0, &mut control, &mut revision) },
            0
        );
        assert_ne!(control & SE_DACL_PROTECTED, 0);
        let mut information = ACL_SIZE_INFORMATION {
            AceCount: 0,
            AclBytesInUse: 0,
            AclBytesFree: 0,
        };
        assert_ne!(
            unsafe {
                GetAclInformation(
                    acl,
                    (&mut information as *mut ACL_SIZE_INFORMATION).cast(),
                    std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                    AclSizeInformation,
                )
            },
            0
        );
        #[repr(align(4))]
        struct SystemSid([u8; 12]);
        let mut system = SystemSid([1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0]);
        let system = system.0.as_mut_ptr().cast();
        let same = unsafe { EqualSid(user.User.Sid, system) } != 0;
        assert_eq!(information.AceCount, if same { 1 } else { 2 });
        let (mut found_user, mut found_system) = (false, false);
        for index in 0..information.AceCount {
            let mut ace = null_mut();
            assert_ne!(unsafe { GetAce(acl, index, &mut ace) }, 0);
            let entry = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
            assert_eq!(entry.Header.AceType, 0);
            assert_eq!(entry.Header.AceFlags, 0);
            assert_eq!(entry.Mask, FILE_ALL_ACCESS);
            let sid = (&entry.SidStart as *const u32).cast_mut().cast();
            found_user |= unsafe { EqualSid(sid, user.User.Sid) } != 0;
            found_system |= unsafe { EqualSid(sid, system) } != 0;
        }
        assert!(found_user && found_system);
    }

    #[test]
    fn file_symlink_with_private_acl_is_rejected() {
        let dir = crate::PanelDir::create().unwrap();
        let target = dir.path().join("target");
        drop(create_new(&target).unwrap());
        let link = dir.path().join("link");
        std::os::windows::fs::symlink_file(&target, &link)
            .expect("Windows runner must permit creation of a file symlink");
        let name = wide(link.as_os_str());
        let raw = unsafe {
            CreateFileW(
                name.as_ptr(),
                READ_CONTROL | WRITE_DAC,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        assert_ne!(raw, INVALID_HANDLE_VALUE);
        let handle = OwnedHandle(raw);
        let security = descriptor(false).unwrap();
        let (mut present, mut defaulted, mut acl) = (0, 0, null_mut());
        assert_ne!(
            unsafe {
                GetSecurityDescriptorDacl(security.0, &mut present, &mut acl, &mut defaulted)
            },
            0
        );
        assert_ne!(present, 0);
        assert_eq!(
            unsafe {
                SetSecurityInfo(
                    handle.0,
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    acl,
                    null_mut(),
                )
            },
            ERROR_SUCCESS
        );
        validate_acl(handle.0, &link, false)
            .expect("symlink fixture must otherwise pass ACL validation");
        drop(handle);
        assert!(open(&link).is_err(), "accepted a private file symlink");
        std::fs::remove_file(link).unwrap();
    }
}
