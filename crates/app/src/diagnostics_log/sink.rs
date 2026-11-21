//! The file the captured stream lands in, and the one rule that keeps it bounded.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

/// `ivy.log`, written from its end and moved aside once it holds the cap.
///
/// The size is read from the file before each write, never counted here: every window of Ivy is a
/// process of its own (the lease in `session/ownership.rs`) and they all append to the same file,
/// so a count of one process's own bytes would let the file grow past the cap and would rename a
/// file another process had just started.
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

    /// Writes `bytes` whole into the current file, moving it aside first when it already holds the
    /// cap. A chunk is never split across the two files, so either can pass the cap by one write.
    pub(super) fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let full = match &self.file {
            Some(file) => file.metadata()?.len() >= self.cap,
            None => true,
        };
        if full {
            self.rotate();
        }
        let file = match &mut self.file {
            Some(file) => file,
            None => self.file.insert(append(&self.path)?),
        };
        file.write_all(bytes)
    }

    /// Moves a full file aside, unless another window already did: then the name holds a fresh
    /// file, and renaming it would delete the copy that window just made. A rename that fails —
    /// the file deleted under us, another program holding it — leaves writing to go on past the
    /// cap rather than stop: a log that stops says nothing about why.
    fn rotate(&mut self) {
        self.file = None;
        let fresh = std::fs::metadata(&self.path).is_ok_and(|now| now.len() < self.cap);
        if !fresh {
            let _ = std::fs::rename(&self.path, rotated(&self.path));
        }
        self.file = append(&self.path).ok();
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
    fn two_windows_writing_one_file_keep_it_under_the_cap_and_lose_nothing() {
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
    fn a_rotation_that_cannot_rename_keeps_writing_rather_than_stopping() {
        let dir = scratch("stuck");
        let path = dir.join("ivy.log");
        let mut sink = RotatingSink::open(path.clone(), 4).unwrap();
        sink.write(b"1234").unwrap();
        // A directory where the copy should go makes every rename fail, on every platform.
        std::fs::create_dir(rotated(&path)).unwrap();
        sink.write(b"5").unwrap();
        sink.write(b"6").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"123456");
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
