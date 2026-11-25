#![cfg(windows)]

use panel_control::codex_route::Registry;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Threading::*;

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

struct Descriptor(PSECURITY_DESCRIPTOR);
impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}

fn token_user() -> Vec<usize> {
    let mut raw = null_mut();
    assert_ne!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) },
        0
    );
    let token = Handle(raw);
    let mut needed = 0;
    unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut needed) };
    assert_ne!(needed, 0);
    let mut bytes = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    assert_ne!(
        unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                bytes.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        },
        0
    );
    bytes
}

fn assert_private(path: &Path, directory: bool, user: PSID) {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .unwrap();
    let handle = file.as_raw_handle().cast();
    assert_eq!(unsafe { GetFileType(handle) }, FILE_TYPE_DISK);
    let mut info = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
    assert_ne!(unsafe { GetFileInformationByHandle(handle, &mut info) }, 0);
    assert_eq!(info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT, 0);
    assert_eq!(
        info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
        directory
    );

    let (mut owner, mut acl, mut descriptor) = (null_mut(), null_mut(), null_mut());
    assert_eq!(
        unsafe {
            GetSecurityInfo(
                handle,
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                null_mut(),
                &mut acl,
                null_mut(),
                &mut descriptor,
            )
        },
        ERROR_SUCCESS
    );
    let descriptor = Descriptor(descriptor);
    assert!(!owner.is_null() && !acl.is_null(), "{}", path.display());
    assert_ne!(unsafe { EqualSid(owner, user) }, 0, "{}", path.display());
    let (mut control, mut revision) = (0, 0);
    assert_ne!(
        unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) },
        0
    );
    assert_ne!(control & SE_DACL_PROTECTED, 0, "{}", path.display());
    let mut size = ACL_SIZE_INFORMATION {
        AceCount: 0,
        AclBytesInUse: 0,
        AclBytesFree: 0,
    };
    assert_ne!(
        unsafe {
            GetAclInformation(
                acl,
                (&mut size as *mut ACL_SIZE_INFORMATION).cast(),
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
    let same = unsafe { EqualSid(user, system) } != 0;
    assert_eq!(
        size.AceCount,
        if same { 1 } else { 2 },
        "{}",
        path.display()
    );
    let flags = if directory {
        (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8
    } else {
        0
    };
    let (mut found_user, mut found_system) = (false, false);
    for index in 0..size.AceCount {
        let mut ace = null_mut();
        assert_ne!(unsafe { GetAce(acl, index, &mut ace) }, 0);
        let entry = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
        assert_eq!(entry.Header.AceType, 0);
        assert_eq!(entry.Header.AceFlags, flags, "{}", path.display());
        assert_eq!(entry.Mask, FILE_ALL_ACCESS);
        let sid = (&entry.SidStart as *const u32).cast_mut().cast();
        let is_user = unsafe { EqualSid(sid, user) } != 0;
        let is_system = unsafe { EqualSid(sid, system) } != 0;
        assert!(is_user || is_system);
        found_user |= is_user;
        found_system |= is_system;
    }
    assert!(found_user && found_system, "{}", path.display());
}

#[test]
fn every_created_registry_object_has_an_independently_verified_private_descriptor() {
    let fixture = panel_contract::PanelDir::create().unwrap();
    let registry = Registry::for_home(fixture.path()).unwrap();
    let root = fixture.path().join("ivy-panel-routes");
    let route = registry.pending().unwrap();
    let user_bytes = token_user();
    let user = unsafe { &*user_bytes.as_ptr().cast::<TOKEN_USER>() }
        .User
        .Sid;
    assert_private(&root, true, user);
    for path in [
        &root.join("registry.lock"),
        &route.with_extension("lock"),
        &route,
    ] {
        assert_private(path, false, user);
    }

    let thread = "01234567-89ab-cdef-0123-456789abcdef";
    registry
        .bind(thread, &route, "pane", fixture.path(), "old")
        .unwrap();
    registry
        .bind(thread, &route, "pane", fixture.path(), "new")
        .unwrap();
    registry.cancel_epoch("old").unwrap();
    let expected: BTreeSet<_> = [
        "registry.lock".to_owned(),
        route.file_name().unwrap().to_str().unwrap().to_owned(),
        route
            .with_extension("lock")
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned(),
        format!("thread-{thread}"),
        "cancel-6f6c64".to_owned(),
    ]
    .into_iter()
    .collect();
    let actual: BTreeSet<_> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            assert_private(&path, false, user);
            path.file_name().unwrap().to_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(actual, expected);
    fs::remove_dir_all(root).unwrap();
}
