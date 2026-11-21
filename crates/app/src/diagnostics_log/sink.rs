//! The file the captured stream lands in, and the one rule that keeps it bounded.

use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

/// `ivy.log`, written from its end, with its size counted so the cap holds while the window runs.
pub(super) struct RotatingSink {
    path: PathBuf,
    file: Option<File>,
    written: u64,
    cap: u64,
}

impl RotatingSink {
    /// Opens `path` for appending, creating it owner-only on Unix. The count starts at the size
    /// already there, so a file left at the cap by the last run is rotated before this run's first
    /// byte by the same check every write makes.
    pub(super) fn open(path: PathBuf, cap: u64) -> std::io::Result<Self> {
        let file = append(&path)?;
        let written = file.metadata()?.len();
        Ok(Self {
            path,
            file: Some(file),
            written,
            cap,
        })
    }

    /// Writes `bytes` whole into the current file, rotating first when it already holds the cap.
    /// A chunk is never split across the two files.
    pub(super) fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if self.written >= self.cap {
            self.rotate()?;
        }
        let file = match &mut self.file {
            Some(file) => file,
            None => self.file.insert(append(&self.path)?),
        };
        file.write_all(bytes)?;
        self.written += bytes.len() as u64;
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        // Closed before the rename: Windows refuses to move a file this process still holds open.
        self.file = None;
        std::fs::rename(&self.path, rotated(&self.path))?;
        self.written = 0;
        self.file = Some(append(&self.path)?);
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
