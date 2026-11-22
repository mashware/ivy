//! The file the captured stream lands in, and the rules that keep it bounded and findable.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

/// `ivy.log`, written from its end and moved aside once it holds the cap.
///
/// Before each write the file this sink holds is compared with the file at the name, by identity
/// and not by a count of its own bytes: every window of Ivy is a process of its own (the lease in
/// `session/ownership.rs`) and they all append to the same name. A sink whose file another window
/// moved aside, or somebody deleted, follows the name instead of writing where nobody looks.
pub(super) struct RotatingSink {
    path: PathBuf,
    file: Option<File>,
    cap: u64,
}

impl RotatingSink {
    /// Opens `path` for appending, creating it owner-only on Unix.
    pub(super) fn open(path: PathBuf, cap: u64) -> std::io::Result<Self> {
        let file = append(&path)?;
        Ok(Self {
            path,
            file: Some(file),
            cap,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Writes `bytes` whole into the file at the name, moving it aside first when it already holds
    /// the cap. The check comes before the write, so the write that crosses the cap lands whole.
    pub(super) fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if !self.holds_the_name() {
            self.file = append(&self.path).ok();
        }
        let full = match &self.file {
            Some(file) => file.metadata().map_or(true, |now| now.len() >= self.cap),
            None => true,
        };
        if full {
            self.rotate()?;
        }
        let file = match &mut self.file {
            Some(file) => file,
            None => self.file.insert(append(&self.path)?),
        };
        file.write_all(bytes)
    }

    fn holds_the_name(&self) -> bool {
        let Some(file) = &self.file else {
            return false;
        };
        matches!(
            (identity(file), identity_at(&self.path)),
            (Ok(held), Ok(named)) if held == named
        )
    }

    /// Moves the full file aside. When that cannot be done — another program holds the file, the
    /// directory turned read-only — the file is emptied instead and says so on its first line:
    /// writing on past the cap would let a failing console fill the disk, and stopping would leave
    /// a log that says nothing about why.
    ///
    /// Two windows that find the file full in the same instant can both rename: the second moves
    /// the first one's fresh file over the copy. Accepted: it takes two processes crossing the cap
    /// within the same few microseconds, and costs one copy, never the current file.
    fn rotate(&mut self) -> std::io::Result<()> {
        self.file = None;
        let moved = std::fs::rename(&self.path, rotated(&self.path));
        self.file = append(&self.path).ok();
        if let (Err(why), Some(file)) = (moved, &mut self.file) {
            // Windows append handles lack the right to truncate, even with write(true).
            // Keep appending through the original handle so other windows cannot overwrite us.
            OpenOptions::new().write(true).open(&self.path)?.set_len(0)?;
            writeln!(
                file,
                "ivy: could not move the full log aside ({why}); it was started again"
            )?;
        }
        Ok(())
    }
}

/// Where a full file goes: the same name with `.1`, replacing the one there before.
pub(super) fn rotated(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

/// Opens for appending. What lands here is whatever the window wrote to standard error — paths,
/// addresses, a program's error — so on Unix nobody but its owner can read it.
pub(super) fn append(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

#[cfg(unix)]
fn identity(file: &File) -> std::io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(unix)]
fn identity_at(path: &Path) -> std::io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path)?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn identity(file: &File) -> std::io::Result<(u64, u64)> {
    crate::windows_fs::identity(file)
}

#[cfg(windows)]
fn identity_at(path: &Path) -> std::io::Result<(u64, u64)> {
    crate::windows_fs::identity(&File::open(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ivy-log-sink-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_write_that_reaches_the_cap_moves_the_file_aside_before_the_next_one() {
        let dir = scratch("cap");
        let path = dir.join("ivy.log");
        let mut sink = RotatingSink::open(path.clone(), 10).unwrap();
        sink.write(b"0123456").unwrap();
        sink.write(b"789").unwrap();
        assert!(!rotated(&path).exists(), "under the cap nothing rotates");
        sink.write(b"abc").unwrap();
        assert_eq!(std::fs::read(rotated(&path)).unwrap(), b"0123456789");
        assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        sink.write(b"defghijk").unwrap();
        sink.write(b"Z").unwrap();
        assert_eq!(
            std::fs::read(rotated(&path)).unwrap(),
            b"abcdefghijk",
            "the previous copy is replaced, and a chunk is never split"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"Z");
        let files = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(files, 2, "never more than the file and one previous copy");
    }

    #[test]
    fn a_file_left_at_the_cap_by_the_last_run_is_rotated_before_this_runs_first_byte() {
        let dir = scratch("startup");
        let path = dir.join("ivy.log");
        std::fs::write(&path, b"last run!!").unwrap();
        let mut sink = RotatingSink::open(path.clone(), 10).unwrap();
        sink.write(b"new").unwrap();
        assert_eq!(std::fs::read(rotated(&path)).unwrap(), b"last run!!");
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn a_file_under_the_cap_is_continued_where_the_last_run_left_it() {
        let dir = scratch("continue");
        let path = dir.join("ivy.log");
        std::fs::write(&path, b"old ").unwrap();
        let mut sink = RotatingSink::open(path.clone(), 10).unwrap();
        sink.write(b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"old new");
        assert!(!rotated(&path).exists());
    }

    #[test]
    fn two_windows_sharing_one_file_rotate_it_once_and_lose_nothing() {
        let dir = scratch("two");
        let path = dir.join("ivy.log");
        let mut first = RotatingSink::open(path.clone(), 10).unwrap();
        let mut second = RotatingSink::open(path.clone(), 10).unwrap();
        first.write(b"aaaaaa").unwrap();
        second.write(b"bbbbbb").unwrap();
        first.write(b"A").unwrap();
        // The second still holds the file the first moved aside; it must follow the name, not
        // rename the first's fresh file over the copy that holds its own six bytes.
        second.write(b"B").unwrap();
        assert_eq!(std::fs::read(rotated(&path)).unwrap(), b"aaaaaabbbbbb");
        assert_eq!(std::fs::read(&path).unwrap(), b"AB");
    }

    #[test]
    fn a_log_deleted_while_the_window_runs_comes_back_at_the_next_line() {
        let dir = scratch("deleted");
        let path = dir.join("ivy.log");
        let mut sink = RotatingSink::open(path.clone(), 10).unwrap();
        sink.write(b"before").unwrap();
        std::fs::remove_file(&path).unwrap();
        sink.write(b"after").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"after");
    }

    #[test]
    fn a_file_that_cannot_be_moved_aside_starts_again_and_says_why() {
        let dir = scratch("stuck");
        let path = dir.join("ivy.log");
        let mut sink = RotatingSink::open(path.clone(), 4).unwrap();
        sink.write(b"1234").unwrap();
        // A directory where the copy should go makes every rename fail, on every platform.
        std::fs::create_dir(rotated(&path)).unwrap();
        sink.write(b"5").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with("ivy: could not move the full log aside ("),
            "{text}"
        );
        assert!(text.ends_with("it was started again\n5"), "{text}");
        assert!(!text.contains("1234"), "the full file was emptied: {text}");
    }

    #[cfg(unix)]
    #[test]
    fn both_files_are_readable_by_their_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("mode");
        let path = dir.join("ivy.log");
        let mut sink = RotatingSink::open(path.clone(), 4).unwrap();
        sink.write(b"1234").unwrap();
        sink.write(b"5").unwrap();
        for file in [&path, &rotated(&path)] {
            let mode = std::fs::metadata(file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{}", file.display());
        }
    }
}
