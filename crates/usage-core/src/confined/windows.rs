use super::{Binding, Stamp};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::{Component, Path, PathBuf, Prefix};
use std::ptr::{self, null, null_mut};

use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_REPARSE_POINT,
    FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
};
use windows_sys::Win32::Foundation::{
    ERROR_NO_MORE_FILES, HANDLE, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, RtlNtStatusToDosError,
    UNICODE_STRING,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_BASIC_INFO,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_BOTH_DIR_INFO, FILE_ID_INFO,
    FILE_INFO_BY_HANDLE_CLASS, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_READ_DATA,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO, FILE_TRAVERSE,
    FILE_TYPE_DISK, FileBasicInfo, FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo,
    FileIdInfo, FileStandardInfo, GetDriveTypeW, GetFileInformationByHandleEx, GetFileType,
    SYNCHRONIZE,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED;

const PINNED_SHARE: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE;
const SHARED: u32 = PINNED_SHARE | FILE_SHARE_DELETE;
const DIR_ACCESS: u32 = FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
const MAX_DEPTH: usize = 64;

pub(crate) fn private_directory(path: &Path, create: bool) -> io::Result<()> {
    crate::control::windows::private_directory(path, create)
}

fn name_units(name: &OsStr) -> io::Result<Vec<u16>> {
    let units: Vec<_> = name.encode_wide().collect();
    if units.is_empty()
        || units.len() > 255
        || units == [u16::from(b'.')]
        || units == [u16::from(b'.'), u16::from(b'.')]
        || units
            .iter()
            .any(|&unit| matches!(unit, 0 | 0x2f | 0x3a | 0x5c))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    Ok(units)
}

fn component_name(component: Component<'_>) -> io::Result<&OsStr> {
    match component {
        Component::Normal(name) => Ok(name),
        _ => Err(io::ErrorKind::InvalidInput.into()),
    }
}

fn open_relative(
    directory: &File,
    name: &OsStr,
    access: u32,
    share: u32,
    options: u32,
) -> io::Result<File> {
    let mut units = name_units(name)?;
    let length = (units.len() * size_of::<u16>()) as u16;
    let name = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: units.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: directory.as_raw_handle(),
        ObjectName: &raw const name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        SecurityDescriptor: null(),
        SecurityQualityOfService: null(),
    };
    let mut handle: HANDLE = null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // A named child is resolved against an open directory rather than a mutable absolute path.
    let result = unsafe {
        NtCreateFile(
            &raw mut handle,
            access,
            &raw const attributes,
            &raw mut status,
            null(),
            0,
            share,
            FILE_OPEN,
            FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT | options,
            null(),
            0,
        )
    };
    if result < 0 {
        let code = unsafe { RtlNtStatusToDosError(result) };
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    if handle.is_null() {
        return Err(io::ErrorKind::InvalidData.into());
    }
    // NtCreateFile transferred a synchronous, non-inheritable handle to the caller.
    let file = unsafe { File::from_raw_handle(handle) };
    verify_plain(&file)?;
    Ok(file)
}

fn verify_plain(file: &File) -> io::Result<()> {
    if unsafe { GetFileType(file.as_raw_handle()) } != FILE_TYPE_DISK
        || file.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

struct Directories(Vec<File>);

impl Directories {
    fn last(&self) -> &File {
        self.0.last().expect("the drive root is open")
    }

    fn push(&mut self, name: &OsStr, share: u32, access: u32) -> io::Result<()> {
        if self.0.len() > MAX_DEPTH {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let child = open_relative(self.last(), name, access, share, FILE_DIRECTORY_FILE)?;
        self.0.push(child);
        Ok(())
    }
}

fn open_absolute(path: &Path, final_share: u32) -> io::Result<Directories> {
    let mut components = path.components().peekable();
    let drive = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) if drive.is_ascii_alphabetic() => {
                drive
            }
            _ => return Err(io::ErrorKind::Unsupported.into()),
        },
        _ => return Err(io::ErrorKind::InvalidInput.into()),
    };
    if components.next() != Some(Component::RootDir) || components.peek().is_none() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let root = [u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0];
    if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
        return Err(io::ErrorKind::Unsupported.into());
    }
    let drive_root = OsString::from_wide(&root[..3]);
    let base = OpenOptions::new()
        .access_mode(DIR_ACCESS)
        .share_mode(PINNED_SHARE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(drive_root)?;
    verify_plain(&base)?;

    let mut opened = Directories(vec![base]);
    while let Some(component) = components.next() {
        let final_component = components.peek().is_none();
        let (share, access) = if final_component {
            (final_share, DIR_ACCESS | FILE_LIST_DIRECTORY)
        } else {
            (PINNED_SHARE, DIR_ACCESS)
        };
        // A directory cannot be renamed while its handle omits FILE_SHARE_DELETE.
        opened.push(component_name(component)?, share, access)?;
    }
    Ok(opened)
}

pub(crate) fn open_directory(path: &Path) -> io::Result<File> {
    let mut opened = open_absolute(path, SHARED)?;
    opened
        .0
        .pop()
        .ok_or_else(|| io::ErrorKind::InvalidData.into())
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Identity {
    volume: u64,
    low: u64,
    high: u64,
}

fn identity(file: &File) -> io::Result<Identity> {
    let info: FILE_ID_INFO = file_information(file, FileIdInfo)?;
    let [low, high] = std::array::from_fn(|index| {
        let start = index * size_of::<u64>();
        let bytes = info.FileId.Identifier[start..start + size_of::<u64>()]
            .try_into()
            .expect("FILE_ID_128 contains two u64 halves");
        u64::from_le_bytes(bytes)
    });
    Ok(Identity {
        volume: info.VolumeSerialNumber,
        low,
        high,
    })
}

pub(crate) fn bind(path: PathBuf) -> io::Result<Binding> {
    let directory = open_directory(&path)?;
    let actual = identity(&directory)?;
    Ok(Binding {
        path,
        device: actual.volume,
        inode: actual.low,
        file_id_high: Some(actual.high),
    })
}

pub(crate) struct Root {
    binding: Binding,
}

impl Root {
    pub(crate) fn open(binding: Binding) -> io::Result<Self> {
        let root = Self { binding };
        root.verify()?;
        Ok(root)
    }

    fn open_bound(&self, share: u32) -> io::Result<Directories> {
        let opened = open_absolute(&self.binding.path, share)?;
        let actual = identity(opened.last())?;
        if self.binding.device != actual.volume
            || self.binding.inode != actual.low
            || self.binding.file_id_high != Some(actual.high)
        {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(opened)
    }

    pub(crate) fn verify(&self) -> io::Result<()> {
        self.open_bound(SHARED).map(drop)
    }

    pub(crate) fn file(&self, path: &Path) -> io::Result<File> {
        let mut chain = self.open_bound(PINNED_SHARE)?;
        let mut components = path.components().peekable();
        while let Some(component) = components.next() {
            let name = component_name(component)?;
            if components.peek().is_some() {
                chain.push(name, PINNED_SHARE, DIR_ACCESS)?;
                continue;
            }
            let file = open_relative(
                chain.last(),
                name,
                FILE_READ_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                SHARED,
                FILE_NON_DIRECTORY_FILE,
            )?;
            let info: FILE_STANDARD_INFO = file_information(&file, FileStandardInfo)?;
            if info.Directory || info.DeletePending || info.NumberOfLinks != 1 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            return Ok(file);
        }
        Err(io::ErrorKind::InvalidInput.into())
    }

    pub(crate) fn discover(&self) -> io::Result<Discovery> {
        let mut opened = self.open_bound(SHARED)?;
        let directory = opened.0.pop().expect("the bound root is open");
        Ok(Discovery {
            root: Root {
                binding: self.binding.clone(),
            },
            stack: vec![(Reader::new(directory), PathBuf::new())],
            partial: false,
        })
    }
}

pub(crate) fn same_file(left: &File, right: &File) -> io::Result<bool> {
    Ok(identity(left)? == identity(right)?)
}

pub(crate) fn stamp(file: &File) -> io::Result<Stamp> {
    let standard: FILE_STANDARD_INFO = file_information(file, FileStandardInfo)?;
    let basic: FILE_BASIC_INFO = file_information(file, FileBasicInfo)?;
    Ok(Stamp {
        len: u64::try_from(standard.EndOfFile).map_err(|_| io::ErrorKind::InvalidData)?,
        times: [basic.LastWriteTime, basic.ChangeTime, basic.CreationTime, 0],
    })
}

fn file_information<T: Default>(file: &File, class: FILE_INFO_BY_HANDLE_CLASS) -> io::Result<T> {
    let mut data = T::default();
    // Each caller pairs the Win32 information class with its matching output structure.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            class,
            (&raw mut data).cast(),
            size_of::<T>() as u32,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(data)
}

const DIRECTORY_BUFFER_BYTES: usize = 8 * 1024;

#[repr(align(8))]
struct DirectoryBuffer([u8; DIRECTORY_BUFFER_BYTES]);

struct Reader {
    directory: File,
    buffer: Box<DirectoryBuffer>,
    next_offset: Option<usize>,
    first: bool,
}

struct Entry {
    name: OsString,
    attributes: u32,
}

impl Reader {
    fn new(directory: File) -> Self {
        Self {
            directory,
            buffer: Box::new(DirectoryBuffer([0; DIRECTORY_BUFFER_BYTES])),
            next_offset: None,
            first: true,
        }
    }

    fn refill(&mut self) -> io::Result<bool> {
        let class = if self.first {
            FileIdBothDirectoryRestartInfo
        } else {
            FileIdBothDirectoryInfo
        };
        self.buffer.0.fill(0);
        let ok = unsafe {
            GetFileInformationByHandleEx(
                self.directory.as_raw_handle(),
                class,
                self.buffer.0.as_mut_ptr().cast(),
                DIRECTORY_BUFFER_BYTES as u32,
            )
        };
        if ok == 0 {
            let error = io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                Ok(false)
            } else {
                Err(error)
            };
        }
        self.first = false;
        self.next_offset = Some(0);
        Ok(true)
    }

    fn next(&mut self) -> io::Result<Option<Entry>> {
        if self.next_offset.is_none() && !self.refill()? {
            return Ok(None);
        }
        let offset = self.next_offset.expect("a refill initializes the offset");
        let start = offset + offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
        if offset + size_of::<FILE_ID_BOTH_DIR_INFO>() > DIRECTORY_BUFFER_BYTES {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let fixed = unsafe {
            ptr::read_unaligned(
                self.buffer
                    .0
                    .as_ptr()
                    .add(offset)
                    .cast::<FILE_ID_BOTH_DIR_INFO>(),
            )
        };
        let end = start
            .checked_add(fixed.FileNameLength as usize)
            .ok_or(io::ErrorKind::InvalidData)?;
        let next_delta = fixed.NextEntryOffset as usize;
        let record_end = if next_delta == 0 {
            DIRECTORY_BUFFER_BYTES
        } else {
            offset
                .checked_add(next_delta)
                .ok_or(io::ErrorKind::InvalidData)?
        };
        if start == end
            || end > record_end
            || record_end > DIRECTORY_BUFFER_BYTES
            || end % 2 != start % 2
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let wide: Vec<_> = self.buffer.0[start..end]
            .chunks_exact(2)
            .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
            .collect();
        self.next_offset = (next_delta != 0).then_some(record_end);
        Ok(Some(Entry {
            name: OsString::from_wide(&wide),
            attributes: fixed.FileAttributes,
        }))
    }
}

pub(crate) struct Discovery {
    root: Root,
    stack: Vec<(Reader, PathBuf)>,
    pub(crate) partial: bool,
}

impl Discovery {
    fn pin_directory(root: &Root, path: &Path, current: &File) -> io::Result<Directories> {
        let mut chain = root.open_bound(PINNED_SHARE)?;
        for component in path.components() {
            chain.push(component_name(component)?, PINNED_SHARE, DIR_ACCESS)?;
        }
        if !same_file(chain.last(), current)? {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        Ok(chain)
    }

    pub(crate) fn next(&mut self, steps: usize) -> Option<PathBuf> {
        for _ in 0..steps {
            let depth = self.stack.len();
            let (reader, directory) = self.stack.last_mut()?;
            // Buffered names have already been read while the containing chain was pinned.
            let mut pin = None;
            if reader.next_offset.is_none() {
                match Self::pin_directory(&self.root, directory, &reader.directory) {
                    Ok(chain) => pin = Some(chain),
                    Err(_) => {
                        self.partial = true;
                        self.stack.pop();
                        continue;
                    }
                }
            }
            let item = match reader.next() {
                Ok(Some(item)) => item,
                Ok(None) => {
                    self.stack.pop();
                    continue;
                }
                Err(_) => {
                    self.partial = true;
                    self.stack.pop();
                    continue;
                }
            };
            if item.name == "." || item.name == ".." {
                continue;
            }
            if item.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                self.partial = true;
                continue;
            }
            let path = directory.join(&item.name);
            if item.attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                if depth == MAX_DEPTH {
                    self.partial = true;
                    continue;
                }
                if pin.is_none() {
                    match Self::pin_directory(&self.root, directory, &reader.directory) {
                        Ok(chain) => pin = Some(chain),
                        Err(_) => {
                            self.partial = true;
                            self.stack.pop();
                            continue;
                        }
                    }
                }
                let chain = pin
                    .as_ref()
                    .expect("directory traversal has a pinned parent");
                match open_relative(
                    chain.last(),
                    &item.name,
                    DIR_ACCESS | FILE_LIST_DIRECTORY,
                    SHARED,
                    FILE_DIRECTORY_FILE,
                ) {
                    Ok(directory) => self.stack.push((Reader::new(directory), path)),
                    Err(_) => self.partial = true,
                }
            } else if crate::confined::is_history_candidate(&path) {
                return Some(path);
            }
        }
        None
    }

    pub(crate) fn finished(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Read;
    use windows_sys::Win32::Foundation::GENERIC_WRITE;
    use windows_sys::Win32::System::IO::DeviceIoControl;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> io::Result<Self> {
            let mut suffix = [0_u8; 8];
            getrandom::fill(&mut suffix)
                .map_err(|_| io::Error::other("fixture nonce unavailable"))?;
            let path = std::env::temp_dir().canonicalize()?.join(format!(
                "ivy-usage-confined-{:016x}",
                u64::from_le_bytes(suffix)
            ));
            fs::create_dir(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn junction(link: &Path, target: &Path) -> io::Result<()> {
        const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00a4;
        const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xa000_0003;

        fs::create_dir(link)?;
        let file = OpenOptions::new()
            .access_mode(GENERIC_WRITE)
            .share_mode(SHARED)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(link)?;
        let canonical: Vec<_> = target.canonicalize()?.as_os_str().encode_wide().collect();
        let verbatim: Vec<_> = r"\\?\".encode_utf16().collect();
        let print = canonical
            .strip_prefix(verbatim.as_slice())
            .unwrap_or(&canonical);
        let substitute: Vec<_> = r"\??\"
            .encode_utf16()
            .chain(print.iter().copied())
            .collect();
        let print_offset = (substitute.len() as u16 + 1) * 2;
        let print_length = print.len() as u16 * 2;
        let mut bytes = IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes().to_vec();
        // REPARSE_DATA_BUFFER.MountPointReparseBuffer stores byte offsets into its UTF-16 tail.
        for field in [
            8 + print_offset + print_length + 2,
            0,
            0,
            print_offset - 2,
            print_offset,
            print_length,
        ] {
            bytes.extend_from_slice(&field.to_le_bytes());
        }
        for unit in substitute
            .into_iter()
            .chain([0])
            .chain(print.iter().copied())
            .chain([0])
        {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let mut written = 0;
        let ok = unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                FSCTL_SET_REPARSE_POINT,
                bytes.as_ptr().cast(),
                bytes.len() as u32,
                null_mut(),
                0,
                &raw mut written,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    #[test]
    fn reads_nested_history_but_excludes_links_and_alternate_streams() -> io::Result<()> {
        let fixture = Fixture::new()?;
        let root_path = fixture.0.join("approved");
        let subdir = root_path.join("nested");
        let outside = fixture.0.join("outside");
        fs::create_dir_all(&subdir)?;
        fs::create_dir(&outside)?;
        fs::write(subdir.join("turn.jsonl"), "approved")?;
        fs::write(outside.join("secret.jsonl"), "unapproved")?;
        let link = root_path.join("junction");
        junction(&link, &outside)?;
        assert!(bind(link).is_err());

        let root = Root::open(bind(root_path.clone())?)?;
        let mut discovery = root.discover()?;
        let mut files = Vec::new();
        for _ in 0..100 {
            files.extend(discovery.next(1));
            if discovery.finished() {
                break;
            }
        }
        assert!(discovery.finished());
        assert!(discovery.partial);
        assert_eq!(files, [PathBuf::from("nested").join("turn.jsonl")]);
        let mut contents = String::new();
        root.file(&files[0])?.read_to_string(&mut contents)?;
        assert_eq!(contents, "approved");
        assert!(
            root.file(&Path::new("junction").join("secret.jsonl"))
                .is_err()
        );
        assert!(root.file(Path::new(r"..\outside\secret.jsonl")).is_err());
        assert!(root.file(Path::new(r"nested\turn.jsonl:secret")).is_err());

        fs::hard_link(
            outside.join("secret.jsonl"),
            root_path.join("hard-link.jsonl"),
        )?;
        assert!(root.file(Path::new("hard-link.jsonl")).is_err());
        assert!(root.file(&subdir.join("turn.jsonl")).is_err());
        Ok(())
    }

    #[test]
    fn pins_ancestors_during_open_and_denies_the_replaced_root() -> io::Result<()> {
        let fixture = Fixture::new()?;
        let parent = fixture.0.join("group");
        let selected = parent.join("approved");
        fs::create_dir_all(&selected)?;
        fs::write(selected.join("turn.jsonl"), "old")?;
        let root = Root::open(bind(selected.clone())?)?;
        let moved = fixture.0.join("moved");
        let pin = root.open_bound(PINNED_SHARE)?;
        assert!(fs::rename(&parent, &moved).is_err());
        drop(pin);

        fs::rename(&parent, moved)?;
        fs::create_dir_all(&selected)?;
        fs::write(selected.join("turn.jsonl"), "unapproved")?;
        assert!(root.verify().is_err());
        assert!(root.file(Path::new("turn.jsonl")).is_err());
        assert!(root.discover().is_err());
        Ok(())
    }
}
