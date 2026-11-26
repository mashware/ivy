use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct Binding {
    pub path: PathBuf,
    pub device: u64,
    pub inode: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_id_high: Option<u64>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Stamp {
    pub(crate) len: u64,
    times: [i64; 4],
}

impl Stamp {
    pub(crate) fn unchanged_or_appended(self, after: Self) -> bool {
        after.len > self.len || after == self
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod native {
    use super::{Binding, Stamp};
    use std::ffi::{CStr, CString, OsStr, OsString};
    use std::fs::File;
    use std::io;
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::os::unix::fs::MetadataExt;
    use std::path::{Component, Path, PathBuf};

    const DIR_FLAGS: libc::c_int =
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC;
    const FILE_FLAGS: libc::c_int =
        libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC;

    fn name_bytes(name: &OsStr) -> io::Result<CString> {
        CString::new(name.as_bytes()).map_err(|_| io::ErrorKind::InvalidInput.into())
    }

    fn open_at(parent: BorrowedFd<'_>, name: &OsStr, flags: libc::c_int) -> io::Result<File> {
        let name = name_bytes(name)?;
        // The name is a single path component, and the descriptor remains borrowed.
        let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn fs_root() -> io::Result<File> {
        let fd = unsafe { libc::open(c"/".as_ptr(), DIR_FLAGS) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }

    fn normal(component: Component<'_>) -> io::Result<&OsStr> {
        match component {
            Component::Normal(name) => Ok(name),
            _ => Err(io::ErrorKind::InvalidInput.into()),
        }
    }

    fn open_absolute(path: &Path, private: bool, create: bool) -> io::Result<File> {
        let mut components = path.components().peekable();
        if components.next() != Some(Component::RootDir) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut directory = fs_root()?;
        let uid = unsafe { libc::geteuid() };
        let filesystem_owner = directory.metadata()?.uid();
        while let Some(component) = components.next() {
            let name = normal(component)?;
            directory = match open_at(directory.as_fd(), name, DIR_FLAGS) {
                Err(err) if create && err.kind() == io::ErrorKind::NotFound => {
                    let name_buf = name_bytes(name)?;
                    let created =
                        unsafe { libc::mkdirat(directory.as_raw_fd(), name_buf.as_ptr(), 0o700) };
                    if created < 0
                        && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists
                    {
                        return Err(io::Error::last_os_error());
                    }
                    open_at(directory.as_fd(), name, DIR_FLAGS)?
                }
                result => result?,
            };
            if private {
                let metadata = directory.metadata()?;
                let mode = metadata.mode();
                let private_leaf = components.peek().is_none();
                let safe = if private_leaf {
                    metadata.uid() == uid && mode & 0o077 == 0
                } else {
                    (metadata.uid() == uid || metadata.uid() == filesystem_owner)
                        && (mode & 0o022 == 0 || mode & 0o1000 != 0)
                };
                if !safe {
                    return Err(io::ErrorKind::PermissionDenied.into());
                }
            }
        }
        if path == Path::new("/") {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(directory)
    }

    pub(crate) fn private_directory(path: &Path, create: bool) -> io::Result<()> {
        open_absolute(path, true, create).map(drop)
    }

    pub(crate) fn bind(path: PathBuf) -> io::Result<Binding> {
        let directory = open_absolute(&path, false, false)?;
        let metadata = directory.metadata()?;
        open_source(directory.as_fd(), Path::new("."))?;
        Ok(Binding {
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
            file_id_high: None,
        })
    }

    pub(crate) struct Root {
        directory: File,
        binding: Binding,
    }

    impl Root {
        pub(crate) fn open(binding: Binding) -> io::Result<Self> {
            let directory = open_absolute(&binding.path, false, false)?;
            let metadata = directory.metadata()?;
            if metadata.dev() != binding.device || metadata.ino() != binding.inode {
                return Err(io::ErrorKind::PermissionDenied.into());
            }
            Ok(Self { directory, binding })
        }

        pub(crate) fn verify(&self) -> io::Result<()> {
            Self::open(self.binding.clone()).map(drop)
        }

        pub(crate) fn file(&self, path: &Path) -> io::Result<File> {
            if path.as_os_str().is_empty()
                || !path
                    .components()
                    .all(|item| matches!(item, Component::Normal(_)))
            {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            let file = open_source(self.directory.as_fd(), path)?;
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.nlink() != 1 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            Ok(file)
        }

        pub(crate) fn discover(&self) -> io::Result<Discovery> {
            Ok(Discovery {
                stack: vec![(Reader::open(self.directory.as_fd())?, PathBuf::new())],
                partial: false,
            })
        }
    }

    #[cfg(target_os = "linux")]
    fn open_source(root: BorrowedFd<'_>, path: &Path) -> io::Result<File> {
        let path = name_bytes(path.as_os_str())?;
        // The kernel enforces containment in one lookup, including a concurrent rename.
        let mut how: libc::open_how = unsafe { std::mem::zeroed() };
        how.flags = FILE_FLAGS as u64;
        how.resolve = libc::RESOLVE_BENEATH | libc::RESOLVE_NO_SYMLINKS | libc::RESOLVE_NO_XDEV;
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                root.as_raw_fd(),
                path.as_ptr(),
                &raw const how,
                std::mem::size_of::<libc::open_how>(),
            )
        };
        if fd < 0 {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(libc::ENOSYS) {
                Err(io::ErrorKind::Unsupported.into())
            } else {
                Err(error)
            };
        }
        Ok(unsafe { File::from_raw_fd(fd as libc::c_int) })
    }

    #[cfg(target_os = "macos")]
    fn open_source(root: BorrowedFd<'_>, path: &Path) -> io::Result<File> {
        // Darwin rejects O_NOFOLLOW and O_NOFOLLOW_ANY together.
        let flags = (FILE_FLAGS & !libc::O_NOFOLLOW) | libc::O_NOFOLLOW_ANY;
        open_at(root, path.as_os_str(), flags)
    }

    pub(crate) fn same_file(left: &File, right: &File) -> io::Result<bool> {
        let left = left.metadata()?;
        let right = right.metadata()?;
        Ok(left.dev() == right.dev() && left.ino() == right.ino())
    }

    pub(crate) fn stamp(file: &File) -> io::Result<Stamp> {
        let metadata = file.metadata()?;
        Ok(Stamp {
            len: metadata.len(),
            times: [
                metadata.mtime(),
                metadata.mtime_nsec(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ],
        })
    }

    pub(crate) struct Discovery {
        stack: Vec<(Reader, PathBuf)>,
        pub partial: bool,
    }

    impl Discovery {
        // Limit both traversal work per tick and simultaneously open descriptors.
        pub(crate) fn next(&mut self, mut steps: usize) -> Option<PathBuf> {
            while steps != 0 {
                let (reader, directory) = self.stack.last_mut()?;
                steps -= 1;
                let Some((name, kind)) = reader.next() else {
                    self.partial |= reader.failed;
                    self.stack.pop();
                    continue;
                };
                let path = directory.join(&name);
                if kind == libc::DT_DIR || kind == libc::DT_UNKNOWN {
                    match open_at(reader.descriptor(), &name, DIR_FLAGS) {
                        Ok(child) if self.stack.len() < 64 => {
                            match Reader::open(child.as_fd()) {
                                Ok(reader) => self.stack.push((reader, path)),
                                Err(_) => self.partial = true,
                            }
                            continue;
                        }
                        Ok(_) => {
                            self.partial = true;
                            continue;
                        }
                        Err(err)
                            if kind == libc::DT_UNKNOWN
                                && err.raw_os_error() == Some(libc::ENOTDIR) => {}
                        Err(_) => {
                            self.partial = true;
                            continue;
                        }
                    }
                }
                if crate::confined::is_history_candidate(&path) {
                    if kind == libc::DT_REG || kind == libc::DT_UNKNOWN {
                        return Some(path);
                    }
                    self.partial = true;
                }
            }
            None
        }

        pub(crate) fn finished(&self) -> bool {
            self.stack.is_empty()
        }
    }

    struct Reader {
        pointer: std::ptr::NonNull<libc::DIR>,
        failed: bool,
    }

    impl Reader {
        fn open(fd: BorrowedFd<'_>) -> io::Result<Self> {
            // Open a separate descriptor to give each sweep its own directory offset.
            let handle = open_at(fd, OsStr::new("."), DIR_FLAGS)?;
            let fd = OwnedFd::from(handle);
            let pointer = unsafe { libc::fdopendir(fd.as_raw_fd()) };
            let pointer = std::ptr::NonNull::new(pointer).ok_or_else(io::Error::last_os_error)?;
            let _ = fd.into_raw_fd();
            Ok(Self {
                pointer,
                failed: false,
            })
        }

        fn descriptor(&self) -> BorrowedFd<'_> {
            let raw = unsafe { libc::dirfd(self.pointer.as_ptr()) };
            unsafe { BorrowedFd::borrow_raw(raw) }
        }

        fn next(&mut self) -> Option<(OsString, u8)> {
            loop {
                set_errno(0);
                let item = unsafe { libc::readdir(self.pointer.as_ptr()).as_ref() };
                let Some(item) = item else {
                    self.failed = io::Error::last_os_error().raw_os_error() != Some(0);
                    return None;
                };
                let name = unsafe { CStr::from_ptr(item.d_name.as_ptr()) }.to_bytes();
                if name != b"." && name != b".." {
                    return Some((OsString::from_vec(name.to_vec()), item.d_type));
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn set_errno(value: libc::c_int) {
        unsafe { *libc::__errno_location() = value };
    }

    #[cfg(target_os = "macos")]
    fn set_errno(value: libc::c_int) {
        unsafe { *libc::__error() = value };
    }

    impl Drop for Reader {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.pointer.as_ptr()) };
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn replacement_and_links_do_not_inherit_access() -> io::Result<()> {
            let tmp = std::env::temp_dir()
                .canonicalize()?
                .join(format!("ivy-root-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&tmp);
            std::fs::create_dir(&tmp)?;
            let root = tmp.join("approved");
            std::fs::create_dir(&root)?;
            std::fs::write(tmp.join("outside.jsonl"), "unapproved")?;
            std::os::unix::fs::symlink(tmp.join("outside.jsonl"), root.join("link.jsonl"))?;
            let binding = bind(root.clone())?;
            let open = Root::open(binding.clone())?;
            assert!(open.file(Path::new("link.jsonl")).is_err());
            std::fs::rename(&root, tmp.join("old"))?;
            std::fs::create_dir(&root)?;
            assert!(Root::open(binding).is_err());
            assert!(open.verify().is_err());
            std::fs::remove_dir_all(&tmp)
        }
    }
}

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod native {
    use super::{Binding, Stamp};
    use std::fs::File;
    use std::io;
    use std::path::{Path, PathBuf};

    pub(crate) fn private_directory(_: &Path, _: bool) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub(crate) fn bind(_: PathBuf) -> io::Result<Binding> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub(crate) fn same_file(_: &File, _: &File) -> io::Result<bool> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub(crate) struct Root;

    impl Root {
        pub(crate) fn open(_: Binding) -> io::Result<Self> {
            Err(io::ErrorKind::Unsupported.into())
        }

        pub(crate) fn verify(&self) -> io::Result<()> {
            Err(io::ErrorKind::Unsupported.into())
        }

        pub(crate) fn file(&self, _: &Path) -> io::Result<File> {
            Err(io::ErrorKind::Unsupported.into())
        }

        pub(crate) fn discover(&self) -> io::Result<Discovery> {
            Err(io::ErrorKind::Unsupported.into())
        }
    }

    pub(crate) fn stamp(_: &File) -> io::Result<Stamp> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub(crate) struct Discovery {
        pub partial: bool,
    }

    impl Discovery {
        pub(crate) fn next(&mut self, _: usize) -> Option<PathBuf> {
            None
        }

        pub(crate) fn finished(&self) -> bool {
            true
        }
    }
}

#[cfg(not(windows))]
pub(crate) use native::*;
#[cfg(windows)]
pub(crate) use windows::{Discovery, Root, bind, private_directory, same_file, stamp};

pub(crate) fn supported() -> bool {
    cfg!(any(target_os = "linux", target_os = "macos", windows))
}

fn is_history_candidate(path: &std::path::Path) -> bool {
    let plain = path.extension().is_some_and(|extension| {
        if cfg!(windows) {
            extension.eq_ignore_ascii_case("jsonl")
        } else {
            extension == "jsonl"
        }
    });
    plain
        || path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl.zst"))
}
