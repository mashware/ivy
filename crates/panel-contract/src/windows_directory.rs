//! Windows creation and validation for private pane directories.
//!
//! The ACL is applied in `CreateDirectoryW`, so no permissive directory is visible between
//! creation and tightening. Inherited paths are opened without following reparse points and their
//! owner, protected DACL, type, and exact allow entries are checked through that handle.

use std::ffi::OsStr;
use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Threading::*;

fn error() -> io::Error {
    io::Error::last_os_error()
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct OwnedDescriptor(PSECURITY_DESCRIPTOR);
impl Drop for OwnedDescriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn token_user() -> io::Result<(OwnedHandle, Vec<usize>)> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(error());
    }
    let token = OwnedHandle(token);
    let mut needed = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut needed);
    }
    let mut bytes = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            bytes.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(error());
    }
    Ok((token, bytes))
}

fn descriptor(children: bool) -> io::Result<OwnedDescriptor> {
    let (_token, user) = token_user()?;
    let token_user = unsafe { &*(user.as_ptr().cast::<TOKEN_USER>()) };
    let mut sid = null_mut();
    if unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut sid) } == 0 {
        return Err(error());
    }
    let mut length = 0;
    unsafe {
        while *sid.add(length) != 0 {
            length += 1;
        }
    }
    let user_sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid, length) });
    unsafe {
        LocalFree(sid.cast());
    }
    let inherit = if children { "OICI" } else { "" };
    let system_ace = if user_sid == "S-1-5-18" {
        String::new()
    } else {
        format!("(A;{inherit};FA;;;SY)")
    };
    let sddl: Vec<u16> = format!("O:{user_sid}D:P(A;{inherit};FA;;;{user_sid}){system_ace}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut output = null_mut();
    let mut size = 0;
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut output,
            &mut size,
        )
    } == 0
    {
        return Err(error());
    }
    Ok(OwnedDescriptor(output))
}

pub fn create(path: &Path) -> io::Result<()> {
    let security = descriptor(true)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security.0,
        bInheritHandle: 0,
    };
    let path = wide(path.as_os_str());
    if unsafe { CreateDirectoryW(path.as_ptr(), &attributes) } == 0 {
        return Err(error());
    }
    Ok(())
}

pub fn validate(path: &Path) -> io::Result<()> {
    let path_wide = no_follow_path(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            READ_CONTROL | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(error());
    }
    let handle = OwnedHandle(handle);
    let mut info = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
    if unsafe { GetFileInformationByHandle(handle.0, &mut info) } == 0 {
        return Err(error());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a private directory of this user", path.display()),
        ));
    }
    let (token, token_bytes) = token_user()?;
    let user = unsafe { &*(token_bytes.as_ptr().cast::<TOKEN_USER>()) };
    let mut owner = null_mut();
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    let result = unsafe {
        GetSecurityInfo(
            handle.0,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if result != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    let descriptor = OwnedDescriptor(descriptor);
    if owner.is_null() || dacl.is_null() || unsafe { EqualSid(owner, user.User.Sid) } == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a private directory of this user", path.display()),
        ));
    }
    let mut control = 0;
    let mut revision = 0;
    if unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) } == 0
        || control & SE_DACL_PROTECTED == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a private directory of this user", path.display()),
        ));
    }
    let mut acl_info = ACL_SIZE_INFORMATION {
        AceCount: 0,
        AclBytesInUse: 0,
        AclBytesFree: 0,
    };
    if unsafe {
        GetAclInformation(
            dacl,
            (&mut acl_info as *mut ACL_SIZE_INFORMATION).cast(),
            std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
    {
        return Err(error());
    }
    #[repr(align(4))]
    struct AlignedSid([u8; 12]);
    let mut system_sid = AlignedSid([1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0]);
    let system_sid_ptr = system_sid.0.as_mut_ptr().cast::<c_void>();
    // S-1-5-18 is the fixed LocalSystem SID representation.
    let system_is_user = unsafe { EqualSid(user.User.Sid, system_sid_ptr) } != 0;
    if acl_info.AceCount != if system_is_user { 1 } else { 2 } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a private directory of this user", path.display()),
        ));
    }
    let expected_flags = (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8;
    let mut user_found = false;
    let mut system_found = false;
    for index in 0..acl_info.AceCount {
        let mut ace = null_mut();
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 {
            return Err(error());
        }
        let header = unsafe { &*(ace.cast::<ACE_HEADER>()) };
        if header.AceType != 0 || header.AceFlags != expected_flags {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not a private directory of this user", path.display()),
            ));
        }
        let allow = unsafe { &*(ace.cast::<ACCESS_ALLOWED_ACE>()) };
        if allow.Mask != FILE_ALL_ACCESS {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not a private directory of this user", path.display()),
            ));
        }
        let sid_ptr = (&allow.SidStart as *const u32).cast_mut().cast::<c_void>();
        if unsafe { EqualSid(sid_ptr, user.User.Sid) } != 0 {
            user_found = true;
        } else if unsafe { EqualSid(sid_ptr, system_sid_ptr) } != 0 {
            system_found = true;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} is not a private directory of this user", path.display()),
            ));
        }
    }
    let _ = token;
    if !user_found || (!system_is_user && !system_found) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is not a private directory of this user", path.display()),
        ));
    }
    Ok(())
}

fn no_follow_path(path: &OsStr) -> Vec<u16> {
    let mut units: Vec<u16> = path.encode_wide().collect();
    loop {
        let len = units.len();
        if len > 3 && (units[len - 1] == b'\\' as u16 || units[len - 1] == b'/' as u16) {
            units.pop();
        } else if len > 3
            && units[len - 1] == b'.' as u16
            && (units[len - 2] == b'\\' as u16 || units[len - 2] == b'/' as u16)
        {
            units.truncate(len - 2);
        } else {
            break;
        }
    }
    units.push(0);
    units
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;

    #[test]
    fn fresh_panel_directory_has_the_required_owner_and_protected_acl() {
        let panel = crate::PanelDir::create().unwrap();
        validate(panel.path()).unwrap();
    }

    #[test]
    fn inherited_file_and_missing_path_are_rejected() {
        let panel = crate::PanelDir::create().unwrap();
        let file = panel.path().join("ordinary-file");
        std::fs::write(&file, "x").unwrap();
        assert!(validate(&file).is_err());
        assert!(validate(&panel.path().join("missing")).is_err());
    }

    #[test]
    fn inherited_reparse_point_is_rejected_even_with_terminal_dot_or_separator() {
        let panel = crate::PanelDir::create().unwrap();
        let target = panel.path().with_extension("target");
        std::fs::create_dir(&target).unwrap();
        let alias = panel.path().with_extension("alias");
        let junction = format!("mklink /J \"{}\" \"{}\"", alias.display(), target.display());
        // `raw_arg`, not `arg`: std escapes the inner quotes as `\"`, which cmd.exe does not
        // understand, and mklink then refuses the path as malformed syntax.
        let status = std::process::Command::new("cmd.exe")
            .args(["/d", "/c"])
            .raw_arg(&junction)
            .status()
            .unwrap();
        assert!(
            status.success(),
            "could not create junction fixture: {junction}"
        );
        for suffix in ["", "\\", "\\.", "\\.\\.\\"] {
            let candidate = std::path::PathBuf::from(format!("{}{suffix}", alias.display()));
            assert!(
                validate(&candidate).is_err(),
                "accepted reparse path {}",
                candidate.display()
            );
        }
        let _ = std::fs::remove_dir(&alias);
        let _ = std::fs::remove_dir(&target);
    }

    #[test]
    fn null_and_broad_dacls_are_rejected() {
        let panel = crate::PanelDir::create().unwrap();
        set_test_dacl(panel.path(), None, true);
        assert!(validate(panel.path()).is_err(), "accepted a null DACL");
        let panel = crate::PanelDir::create().unwrap();
        set_test_dacl(panel.path(), Some("D:(A;;FA;;;WD)"), true);
        assert!(validate(panel.path()).is_err(), "accepted a broad DACL");
    }

    #[test]
    fn otherwise_exact_but_unprotected_directory_dacl_is_rejected() {
        let panel = crate::PanelDir::create().unwrap();
        let (_token, user) = token_user().unwrap();
        let user = unsafe { &*(user.as_ptr().cast::<TOKEN_USER>()) };
        let mut sid = null_mut();
        assert_ne!(
            unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) },
            0
        );
        let mut length = 0;
        unsafe {
            while *sid.add(length) != 0 {
                length += 1;
            }
        }
        let user_sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid, length) });
        unsafe {
            LocalFree(sid.cast());
        }
        let system_ace = if user_sid == "S-1-5-18" {
            String::new()
        } else {
            "(A;OICI;FA;;;SY)".to_owned()
        };
        let sddl = format!("D:(A;OICI;FA;;;{user_sid}){system_ace}");
        set_test_dacl(panel.path(), Some(&sddl), false);
        assert!(
            validate(panel.path()).is_err(),
            "accepted the right allow ACEs on an unprotected DACL"
        );
    }

    fn set_test_dacl(path: &Path, sddl: Option<&str>, protected: bool) {
        let path_wide = wide(path.as_os_str());
        let handle = unsafe {
            CreateFileW(
                path_wide.as_ptr(),
                READ_CONTROL | WRITE_DAC,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                null_mut(),
            )
        };
        assert_ne!(handle, INVALID_HANDLE_VALUE);
        let handle = OwnedHandle(handle);
        let descriptor = if let Some(sddl) = sddl {
            let descriptor_sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
            let mut descriptor = null_mut();
            let mut size = 0;
            assert_ne!(
                unsafe {
                    ConvertStringSecurityDescriptorToSecurityDescriptorW(
                        descriptor_sddl.as_ptr(),
                        SDDL_REVISION_1,
                        &mut descriptor,
                        &mut size,
                    )
                },
                0
            );
            Some(OwnedDescriptor(descriptor))
        } else {
            None
        };
        let acl = if let Some(descriptor) = &descriptor {
            let mut present = 0;
            let mut dacl_defaulted = 0;
            let mut dacl = null_mut();
            assert_ne!(
                unsafe {
                    GetSecurityDescriptorDacl(
                        descriptor.0,
                        &mut present,
                        &mut dacl,
                        &mut dacl_defaulted,
                    )
                },
                0
            );
            assert_ne!(present, 0);
            dacl
        } else {
            null_mut()
        };
        let protection = if protected {
            PROTECTED_DACL_SECURITY_INFORMATION
        } else {
            UNPROTECTED_DACL_SECURITY_INFORMATION
        };
        let result = unsafe {
            SetSecurityInfo(
                handle.0,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | protection,
                null_mut(),
                null_mut(),
                acl,
                null_mut(),
            )
        };
        assert_eq!(result, ERROR_SUCCESS);
    }
}
