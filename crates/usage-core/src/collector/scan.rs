use crate::accounting::Observation;
use crate::claude::{self, Record};
use crate::confined::{self, Binding, Root, Stamp};
use crate::ledger::{Checkpoint, Error as LedgerError, Ledger};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read as _, Seek as _};
use std::path::PathBuf;

const READ_BYTES: usize = 64 * 1024;
const RECORD_BYTES: usize = 1024 * 1024;
const SLICE_BYTES: usize = 8 * 1024 * 1024;
const SLICE_ROWS: usize = 2_000;
const PREFIX_BYTES: u64 = 64 * 1024 * 1024;
const UNRESOLVED: i64 = 1;
const INCOMPLETE: i64 = 2;
const PREFIX_PENDING: i64 = 4;

type Reader = BufReader<File>;

#[derive(Debug)]
pub(super) enum Error {
    Io(io::Error),
    Ledger(LedgerError),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<LedgerError> for Error {
    fn from(error: LedgerError) -> Self {
        Self::Ledger(error)
    }
}

enum Phase {
    OldPrefix {
        hash: Sha256,
        read: u64,
    },
    Records,
    Verify {
        current: Reader,
        observed: Stamp,
        hash: Sha256,
        read: u64,
    },
    Reconcile,
    Ready,
}

pub(super) struct Resume {
    key: [u8; 32],
    original: File,
    stamp: Stamp,
    digest: Sha256,
}

pub(super) struct Scan {
    relative: PathBuf,
    provider: crate::Provider,
    codex: crate::codex::Parser,
    root_key: [u8; 32],
    source_key: [u8; 32],
    salt: [u8; 32],
    old: Option<Checkpoint>,
    phase: Option<Phase>,
    reader: Option<Reader>,
    captured: u64,
    target: u64,
    touched: u64,
    complete: u64,
    verified_replay_bytes: u64,
    replay_checked: bool,
    digest: Sha256,
    complete_hash: [u8; 32],
    line: Vec<u8>,
    oversized: bool,
    flags: i64,
    started: bool,
    verified: Option<Stamp>,
    verified_prefix: Option<(Stamp, u64, Sha256)>,
}

pub(super) enum Progress {
    Work,
    Unchanged,
    Publish,
}

impl Scan {
    pub(super) fn new(
        root: &Root,
        relative: PathBuf,
        root_key: [u8; 32],
        salt: [u8; 32],
        ledger: &Ledger,
        resume: Option<&Resume>,
        provider: crate::Provider,
    ) -> Result<Self, Error> {
        let root_key = if provider == crate::Provider::Codex {
            key(&salt, b"codex-root", &root_key)
        } else {
            root_key
        };
        let source_key = key(&salt, &root_key, relative.as_os_str().as_encoded_bytes());
        let old = ledger.checkpoint(source_key)?;
        let file = root.file(&relative)?;
        let stamp = confined::stamp(&file)?;
        let captured = stamp.len;
        let validate_old = old
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.prefix_bytes <= captured);
        let first = old.is_none();
        let target = if first {
            captured.min(PREFIX_BYTES)
        } else {
            captured
        };
        let mut scan = Self {
            relative,
            provider,
            codex: crate::codex::Parser::default(),
            root_key,
            source_key,
            salt,
            old,
            phase: Some(if validate_old {
                Phase::OldPrefix {
                    hash: Sha256::new(),
                    read: 0,
                }
            } else {
                Phase::Records
            }),
            reader: Some(BufReader::with_capacity(READ_BYTES, file)),
            captured,
            target,
            touched: 0,
            complete: 0,
            verified_replay_bytes: 0,
            replay_checked: false,
            digest: Sha256::new(),
            complete_hash: Sha256::digest([]).into(),
            line: Vec::new(),
            oversized: false,
            flags: 0,
            started: false,
            verified: None,
            verified_prefix: None,
        };
        if provider == crate::Provider::Claude
            && let Some(saved) = resume
            && let Some(old) = &scan.old
            && saved.key == scan.source_key
            && saved.stamp == stamp
            && old.coverage & PREFIX_PENDING != 0
            && old.fingerprint == Some(saved.digest.clone().finalize().into())
        {
            let reader = scan.reader.as_mut().expect("scan owns its source");
            if confined::same_file(&saved.original, reader.get_ref())? {
                reader.seek(io::SeekFrom::Start(old.prefix_bytes))?;
                scan.verified_prefix = Some((stamp, old.prefix_bytes, saved.digest.clone()));
                scan.phase = Some(Phase::Records);
                scan.touched = old.prefix_bytes;
                scan.complete = old.prefix_bytes;
                scan.complete_hash = saved.digest.clone().finalize().into();
                scan.digest = saved.digest.clone();
                scan.target = captured.min(old.prefix_bytes.saturating_add(PREFIX_BYTES));
                scan.flags = old.coverage & UNRESOLVED;
            }
        }
        Ok(scan)
    }

    pub(super) fn step(&mut self, root: &Root, ledger: &mut Ledger) -> Result<Progress, Error> {
        root.verify()?;
        let phase = self.phase.take().expect("one worker owns each scan");
        self.phase = Some(match phase {
            Phase::OldPrefix { mut hash, mut read } => {
                let old = self.old.as_ref().expect("old prefix requires a checkpoint");
                let reader = self
                    .reader
                    .as_mut()
                    .expect("records follow prefix verification");
                if !hash_slice(reader, &mut hash, &mut read, old.prefix_bytes)? {
                    Phase::OldPrefix { hash, read }
                } else if old.fingerprint == Some(hash.clone().finalize().into()) {
                    self.touched = old.prefix_bytes;
                    self.complete = old.prefix_bytes;
                    self.complete_hash = hash.clone().finalize().into();
                    self.digest = hash;
                    self.target = self
                        .captured
                        .min(old.prefix_bytes.saturating_add(PREFIX_BYTES));
                    self.flags = old.coverage & UNRESOLVED;
                    if self.touched == self.captured && old.coverage & PREFIX_PENDING == 0 {
                        let current = root.file(&self.relative)?;
                        if !confined::same_file(reader.get_ref(), &current)? {
                            return Err(io::ErrorKind::InvalidData.into_io());
                        }
                        return Ok(Progress::Unchanged);
                    }
                    if self.provider == crate::Provider::Codex {
                        reader.seek(io::SeekFrom::Start(0))?;
                        self.verified_replay_bytes = old.prefix_bytes;
                        self.touched = 0;
                        self.complete = 0;
                        self.complete_hash = Sha256::digest([]).into();
                        self.digest = Sha256::new();
                        self.flags = 0;
                    }
                    Phase::Records
                } else {
                    reader.seek(io::SeekFrom::Start(0))?;
                    Phase::Records
                }
            }
            Phase::Records => {
                if !self.started {
                    ledger.begin(self.source_key)?;
                    self.started = true;
                }
                let (done, rows) = self.read_rows()?;
                ledger.stage(&rows)?;
                if done {
                    let original = self.reader.as_ref().expect("records have an open file");
                    let current = root.file(&self.relative)?;
                    if !confined::same_file(original.get_ref(), &current)? {
                        return Err(io::ErrorKind::InvalidData.into_io());
                    }
                    if self.complete < self.captured {
                        if self.touched == self.captured {
                            self.flags |= INCOMPLETE;
                        } else {
                            self.flags |= PREFIX_PENDING;
                        }
                    }
                    let observed = confined::stamp(&current)?;
                    let mut current = BufReader::with_capacity(READ_BYTES, current);
                    // Retain proof across bounded publications only for the same,
                    // unchanged open file. Growth or any metadata change requires
                    // a new full verification pass.
                    let (read, hash) = match &self.verified_prefix {
                        Some((stamp, offset, digest)) if *stamp == observed => {
                            current.seek(io::SeekFrom::Start(*offset))?;
                            (*offset, digest.clone())
                        }
                        _ => (0, Sha256::new()),
                    };
                    Phase::Verify {
                        current,
                        observed,
                        hash,
                        read,
                    }
                } else {
                    Phase::Records
                }
            }
            Phase::Verify {
                mut current,
                observed,
                mut hash,
                mut read,
            } => {
                if !hash_slice(&mut current, &mut hash, &mut read, self.complete)? {
                    Phase::Verify {
                        current,
                        observed,
                        hash,
                        read,
                    }
                } else {
                    let fresh = root.file(&self.relative)?;
                    let after = confined::stamp(&fresh)?;
                    let original = self
                        .reader
                        .as_ref()
                        .expect("verification requires open file");
                    if !confined::same_file(original.get_ref(), &fresh)?
                        || after.len < self.complete
                        || !observed.unchanged_or_appended(after)
                        || (self.verified_prefix.is_some() && observed != after)
                        || self.complete_hash != <[u8; 32]>::from(hash.finalize())
                    {
                        return Err(io::ErrorKind::InvalidData.into_io());
                    }
                    self.verified = Some(after);
                    Phase::Reconcile
                }
            }
            Phase::Reconcile => {
                if ledger.reconcile_slice()? {
                    Phase::Ready
                } else {
                    Phase::Reconcile
                }
            }
            Phase::Ready => Phase::Ready,
        });
        Ok(if matches!(self.phase, Some(Phase::Ready)) {
            Progress::Publish
        } else {
            Progress::Work
        })
    }

    fn read_rows(&mut self) -> Result<(bool, Vec<Observation>), Error> {
        let mut bytes = 0;
        let mut rows_read = 0;
        let mut rows = Vec::new();
        let reader = self.reader.as_mut().expect("records have an open file");
        while bytes < SLICE_BYTES && rows_read < SLICE_ROWS && self.touched < self.captured {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                return Err(io::ErrorKind::UnexpectedEof.into_io());
            }
            let limit = ((self.captured - self.touched).min((SLICE_BYTES - bytes) as u64)) as usize;
            let available = &available[..available.len().min(limit)];
            let newline = available.iter().position(|byte| *byte == b'\n');
            let used = newline.map_or(available.len(), |index| index + 1);
            self.digest.update(&available[..used]);
            self.touched += used as u64;
            bytes += used;
            if !self.oversized {
                let content = newline.unwrap_or(used);
                if self.line.len() + content > RECORD_BYTES {
                    self.oversized = true;
                    self.line.clear();
                } else {
                    self.line.extend_from_slice(&available[..content]);
                }
            }
            reader.consume(used);
            if newline.is_none() {
                continue;
            }
            rows_read += 1;
            self.complete = self.touched;
            self.complete_hash = self.digest.clone().finalize().into();
            if self.verified_replay_bytes != 0
                && !self.replay_checked
                && self.complete >= self.verified_replay_bytes
            {
                if self.complete != self.verified_replay_bytes
                    || self
                        .old
                        .as_ref()
                        .and_then(|checkpoint| checkpoint.fingerprint)
                        != Some(self.complete_hash)
                {
                    return Err(io::ErrorKind::InvalidData.into_io());
                }
                self.replay_checked = true;
            }
            if self.oversized {
                self.codex.invalidate_baseline();
                self.flags |= UNRESOLVED;
            } else {
                let record = match self.provider {
                    crate::Provider::Claude => {
                        claude::parse(&self.line, self.source_key, &self.salt)
                    }
                    crate::Provider::Codex => {
                        self.codex.parse(&self.line, self.source_key, &self.salt)
                    }
                };
                match record {
                    Record::Ignored => {}
                    Record::Unresolved => self.flags |= UNRESOLVED,
                    Record::Observation(row) if self.complete > self.verified_replay_bytes => {
                        rows.push(row)
                    }
                    Record::Observation(_) => {}
                }
            }
            self.oversized = false;
            self.line.clear();
            if self.complete >= self.target {
                return Ok((true, rows));
            }
        }
        if self.touched == self.captured && self.verified_replay_bytes != 0 && !self.replay_checked
        {
            return Err(io::ErrorKind::InvalidData.into_io());
        }
        Ok((self.touched == self.captured, rows))
    }

    pub(super) fn publish(
        self,
        root: &Root,
        ledger: &mut Ledger,
    ) -> Result<(bool, Option<Resume>), Error> {
        root.verify()?;
        let fresh = root.file(&self.relative)?;
        let original = self
            .reader
            .as_ref()
            .expect("published scans retain the open file");
        let metadata = confined::stamp(&fresh)?;
        let verified = self
            .verified
            .as_ref()
            .expect("publish follows prefix verification");
        if !confined::same_file(original.get_ref(), &fresh)?
            || metadata.len < self.complete
            || !verified.unchanged_or_appended(metadata)
            || (self.verified_prefix.is_some() && *verified != metadata)
        {
            return Err(io::ErrorKind::InvalidData.into_io());
        }
        ledger.publish(Checkpoint {
            provider: self.provider,
            root_key: self.root_key,
            fingerprint: Some(self.complete_hash),
            prefix_bytes: self.complete,
            coverage: self.flags,
        })?;
        let pending = self.flags & PREFIX_PENDING != 0;
        let resume =
            (pending && *verified == metadata && self.complete == self.touched).then_some(Resume {
                key: self.source_key,
                original: fresh,
                stamp: metadata,
                digest: self.digest,
            });
        Ok((pending || metadata.len > self.captured, resume))
    }
}

fn hash_slice(
    reader: &mut Reader,
    hash: &mut Sha256,
    read: &mut u64,
    end: u64,
) -> io::Result<bool> {
    let stop = end.min(read.saturating_add(SLICE_BYTES as u64));
    let mut buffer = [0; READ_BYTES];
    while *read < stop {
        let length = (stop - *read).min(READ_BYTES as u64) as usize;
        let count = reader.read(&mut buffer[..length])?;
        if count == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        hash.update(&buffer[..count]);
        *read += count as u64;
    }
    Ok(*read == end)
}

fn key(salt: &[u8; 32], context: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Hmac::<Sha256>::new_from_slice(salt).expect("HMAC accepts a 32-byte key");
    hash.update(b"ivy/usage/sources/v1");
    hash.update(&(context.len() as u64).to_le_bytes());
    hash.update(context);
    hash.update(bytes);
    hash.finalize().into_bytes().into()
}

pub(super) fn root_key(root: &Binding, salt: &[u8; 32]) -> [u8; 32] {
    let mut context = [0; 24];
    context[..8].copy_from_slice(&root.device.to_le_bytes());
    context[8..16].copy_from_slice(&root.inode.to_le_bytes());
    let size = if let Some(high) = root.file_id_high {
        context[16..].copy_from_slice(&high.to_le_bytes());
        24
    } else {
        16
    };
    key(
        salt,
        &context[..size],
        root.path.as_os_str().as_encoded_bytes(),
    )
}

trait KindError {
    fn into_io(self) -> Error;
}

impl KindError for io::ErrorKind {
    fn into_io(self) -> Error {
        Error::Io(self.into())
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;
    use std::fs;

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch() -> Cleanup {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "ivy-usage-prefix-{:016x}",
            u64::from_le_bytes(nonce)
        ));
        fs::create_dir(&path).unwrap();
        Cleanup(path)
    }

    #[test]
    fn codex_replay_rejects_a_rewrite_that_removes_the_verified_boundary() {
        let scratch = scratch();
        let path = &scratch.0;
        let relative = PathBuf::from("rollout-test.jsonl");
        let source = path.join(&relative);
        let before = b"first\n";
        fs::write(&source, b"first\nsecond\n").unwrap();
        let root = Root::open(confined::bind(path.clone()).unwrap()).unwrap();
        let salt = [7; 32];
        let root_key = [9; 32];
        let codex_root_key = key(&salt, b"codex-root", &root_key);
        let source_key = key(
            &salt,
            &codex_root_key,
            relative.as_os_str().as_encoded_bytes(),
        );
        let mut ledger = Ledger::open_writer(&path.join("ledger.sqlite3")).unwrap();
        ledger.upgrade_for_codex().unwrap();
        ledger.begin(source_key).unwrap();
        assert!(ledger.reconcile_slice().unwrap());
        ledger
            .publish(Checkpoint {
                provider: crate::Provider::Codex,
                root_key: codex_root_key,
                fingerprint: Some(Sha256::digest(before).into()),
                prefix_bytes: before.len() as u64,
                coverage: PREFIX_PENDING,
            })
            .unwrap();
        let mut scan = Scan::new(
            &root,
            relative,
            root_key,
            salt,
            &ledger,
            None,
            crate::Provider::Codex,
        )
        .unwrap();
        assert!(matches!(scan.step(&root, &mut ledger), Ok(Progress::Work)));
        fs::write(&source, b"first-second\n").unwrap();
        assert!(
            matches!(scan.step(&root, &mut ledger), Err(Error::Io(e)) if e.kind() == io::ErrorKind::InvalidData)
        );
    }

    #[test]
    fn verified_prefix_is_reused_only_for_the_same_unchanged_open_file() {
        let scratch = scratch();
        let path = scratch.0.clone();
        let relative = PathBuf::from("source.jsonl");
        let source = path.join(&relative);
        let before = b"first\n";
        fs::write(&source, b"first\nsecond\n").unwrap();
        let root = Root::open(confined::bind(path.clone()).unwrap()).unwrap();
        let salt = [7; 32];
        let root_key = [9; 32];
        let source_key = key(&salt, &root_key, relative.as_os_str().as_encoded_bytes());
        let original = root.file(&relative).unwrap();
        let stamp = confined::stamp(&original).unwrap();
        let mut digest = Sha256::new();
        digest.update(before);
        let resume = Resume {
            key: source_key,
            original,
            stamp,
            digest: digest.clone(),
        };
        let mut ledger = Ledger::open_writer(&path.join("ledger.sqlite3")).unwrap();
        ledger.begin(source_key).unwrap();
        assert!(ledger.reconcile_slice().unwrap());
        ledger
            .publish(Checkpoint {
                provider: crate::Provider::Claude,
                root_key,
                fingerprint: Some(digest.finalize().into()),
                prefix_bytes: before.len() as u64,
                coverage: PREFIX_PENDING,
            })
            .unwrap();

        let scan = Scan::new(
            &root,
            relative.clone(),
            root_key,
            salt,
            &ledger,
            Some(&resume),
            crate::Provider::Claude,
        )
        .unwrap();
        assert_eq!(scan.touched, before.len() as u64);

        fs::write(&source, b"first\nsecond\nthird\n").unwrap();
        let scan = Scan::new(
            &root,
            relative.clone(),
            root_key,
            salt,
            &ledger,
            Some(&resume),
            crate::Provider::Claude,
        )
        .unwrap();
        assert!(matches!(scan.phase, Some(Phase::OldPrefix { read: 0, .. })));

        fs::rename(&source, path.join("previous.jsonl")).unwrap();
        fs::write(&source, b"first\nsecond\n").unwrap();
        let scan = Scan::new(
            &root,
            relative,
            root_key,
            salt,
            &ledger,
            Some(&resume),
            crate::Provider::Claude,
        )
        .unwrap();
        assert!(matches!(scan.phase, Some(Phase::OldPrefix { read: 0, .. })));
    }
}
