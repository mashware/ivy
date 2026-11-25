use crate::windows_security::{OwnedHandle, descriptor, wide};
use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;
fn error() -> io::Error {
    io::Error::last_os_error()
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
    crate::windows_security::validate_acl(handle.0, path, true)
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
pub(crate) mod tests {
    use super::*;
    use crate::windows_security::{OwnedDescriptor, token_user};
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::Security::Authorization::*;

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

    pub(crate) fn set_test_dacl(path: &Path, sddl: Option<&str>, protected: bool) {
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
