//! Shared Windows owner and private-DACL policy for pane directories and route files.
//!
//! D7 and D92 require privacy at creation, rather than a permissive interval followed by
//! tightening. Directory inheritance and ordinary capability-file access use the same principals.

use std::ffi::OsStr;
use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Threading::*;

fn error() -> io::Error {
    io::Error::last_os_error()
}
pub(crate) fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

pub(crate) struct OwnedHandle(pub(crate) HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub(crate) struct OwnedDescriptor(pub(crate) PSECURITY_DESCRIPTOR);
impl Drop for OwnedDescriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

pub(crate) fn token_user() -> io::Result<(OwnedHandle, Vec<usize>)> {
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

pub(crate) fn descriptor(children: bool) -> io::Result<OwnedDescriptor> {
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

pub(crate) fn validate_acl(handle: HANDLE, path: &Path, directory: bool) -> io::Result<()> {
    let (token, token_bytes) = token_user()?;
    let user = unsafe { &*(token_bytes.as_ptr().cast::<TOKEN_USER>()) };
    let mut owner = null_mut();
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    let result = unsafe {
        GetSecurityInfo(
            handle,
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
    let expected_flags = if directory {
        (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8
    } else {
        0
    };
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
