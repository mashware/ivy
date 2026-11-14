use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

const ENTRY_LIMIT: usize = 4096;
const PATH_BYTES_LIMIT: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Directory,
    File,
    Link,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) path: PathBuf,
    pub(crate) kind: EntryKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listing {
    pub(crate) entries: Vec<Entry>,
    pub(crate) bytes: usize,
}

/// Enumerates one directory without following symlinks. Cancellation is checked between entries.
pub(crate) fn read_directory(path: &Path, cancelled: impl Fn() -> bool) -> Result<Listing, String> {
    let iterator = fs::read_dir(path).map_err(|error| format!("Directory unavailable. {error}"))?;
    let mut entries = Vec::new();
    let mut bytes = 0usize;
    for item in iterator {
        if cancelled() {
            return Err("Directory loading cancelled".into());
        }
        let item = item.map_err(|error| format!("Cannot read directory entry: {error}"))?;
        if entries.len() == ENTRY_LIMIT {
            return Err("Directory exceeds explorer limit (4096 entries / 4 MiB).".into());
        }
        let entry_path = item.path();
        bytes = bytes
            .checked_add(path_payload_bytes(&entry_path))
            .ok_or_else(limit_error)?;
        if bytes > PATH_BYTES_LIMIT {
            return Err(limit_error());
        }
        let file_type = item
            .file_type()
            .map_err(|error| format!("Cannot inspect directory entry: {error}"))?;
        let kind = if file_type.is_symlink() {
            EntryKind::Link
        } else if file_type.is_dir() {
            EntryKind::Directory
        } else if file_type.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        entries.push(Entry {
            path: entry_path,
            kind,
        });
    }
    if cancelled() {
        return Err("Directory loading cancelled".into());
    }
    entries.sort_by(|left, right| {
        let left_dir = left.kind == EntryKind::Directory;
        let right_dir = right.kind == EntryKind::Directory;
        right_dir
            .cmp(&left_dir)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(Listing { entries, bytes })
}

fn limit_error() -> String {
    "Directory exceeds explorer limit (4096 entries / 4 MiB).".into()
}

#[cfg(unix)]
fn path_payload_bytes(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().len()
}

#[cfg(windows)]
fn path_payload_bytes(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().count().saturating_mul(2)
}

#[cfg(not(any(unix, windows)))]
fn path_payload_bytes(path: &Path) -> usize {
    path.as_os_str().to_string_lossy().len()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Request {
    pub(crate) pane: usize,
    pub(crate) generation: u64,
    pub(crate) epoch: u64,
    pub(crate) path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct Reply {
    pub(crate) request: Request,
    pub(crate) result: Result<Listing, String>,
}

struct Job {
    request: Request,
    cancelled: Arc<AtomicBool>,
    ctx: egui::Context,
}

type Mailbox = Arc<(Mutex<Option<Job>>, Condvar)>;

/// A single detached worker with one replaceable queued request and one result slot.
pub(crate) struct Loader {
    mailbox: Mailbox,
    result_rx: Receiver<Reply>,
    shutdown: Arc<AtomicBool>,
    current: Option<Arc<AtomicBool>>,
    worker_error: Option<String>,
    startup_reply: Option<Reply>,
}

impl Default for Loader {
    fn default() -> Self {
        Self::new_with_reader(Arc::new(|path, cancelled| read_directory(path, cancelled)))
    }
}

impl Loader {
    fn new_with_reader(
        reader: Arc<dyn Fn(&Path, &dyn Fn() -> bool) -> Result<Listing, String> + Send + Sync>,
    ) -> Self {
        let mailbox: Mailbox = Arc::new((Mutex::new(None), Condvar::new()));
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_error = spawn_worker(
            Arc::clone(&mailbox),
            result_tx,
            Arc::clone(&shutdown),
            reader,
        )
        .err()
        .map(|error| error.to_string());
        Self {
            mailbox,
            result_rx,
            shutdown,
            current: None,
            worker_error,
            startup_reply: None,
        }
    }

    pub(crate) fn submit(&mut self, request: Request, ctx: &egui::Context) {
        if let Some(current) = self.current.take() {
            current.store(true, Ordering::Release);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        self.current = Some(Arc::clone(&cancelled));
        if let Some(error) = &self.worker_error {
            self.startup_reply = Some(Reply {
                request,
                result: Err(format!("Cannot start explorer loader: {error}")),
            });
            ctx.request_repaint();
            return;
        }
        let (lock, wake) = &*self.mailbox;
        if let Ok(mut slot) = lock.lock() {
            if let Some(old) = slot.replace(Job {
                request,
                cancelled,
                ctx: ctx.clone(),
            }) {
                old.cancelled.store(true, Ordering::Release);
            }
            wake.notify_one();
        }
    }

    pub(crate) fn poll(&mut self) -> Option<Reply> {
        if self.startup_reply.is_some() {
            self.current = None;
            return self.startup_reply.take();
        }
        match self.result_rx.try_recv() {
            Ok(reply) => Some(reply),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    pub(crate) fn cancel(&mut self) {
        if let Some(current) = self.current.take() {
            current.store(true, Ordering::Release);
        }
        self.startup_reply = None;
        let (lock, _) = &*self.mailbox;
        if let Ok(mut slot) = lock.lock() {
            if let Some(job) = slot.take() {
                job.cancelled.store(true, Ordering::Release);
            }
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel();
        self.shutdown.store(true, Ordering::Release);
        self.mailbox.1.notify_one();
    }
}

fn spawn_worker(
    mailbox: Mailbox,
    result_tx: SyncSender<Reply>,
    shutdown: Arc<AtomicBool>,
    reader: Arc<dyn Fn(&Path, &dyn Fn() -> bool) -> Result<Listing, String> + Send + Sync>,
) -> Result<(), std::io::Error> {
    thread::Builder::new()
        .name("explorer-loader".into())
        .spawn(move || {
            loop {
                let job = {
                    let (lock, wake) = &*mailbox;
                    let mut slot = match lock.lock() {
                        Ok(slot) => slot,
                        Err(_) => return,
                    };
                    while slot.is_none() {
                        if shutdown.load(Ordering::Acquire) {
                            return;
                        }
                        slot = match wake.wait(slot) {
                            Ok(slot) => slot,
                            Err(_) => return,
                        };
                    }
                    slot.take().expect("worker mailbox was checked")
                };
                let token = Arc::clone(&job.cancelled);
                let cancelled = || token.load(Ordering::Acquire);
                let result = reader(&job.request.path, &cancelled);
                if token.load(Ordering::Acquire) {
                    if shutdown.load(Ordering::Acquire) {
                        return;
                    }
                    continue;
                }
                let ctx = job.ctx;
                if result_tx
                    .send(Reply {
                        request: job.request,
                        result,
                    })
                    .is_err()
                {
                    return;
                }
                ctx.request_repaint();
            }
        })
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("explorer-loader-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).expect("create fixture");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn request(path: PathBuf) -> Request {
        Request {
            pane: 7,
            generation: 3,
            epoch: 11,
            path,
        }
    }

    #[test]
    fn lists_directories_first_and_keeps_links_unfollowed() {
        let temp = TempDir::new();
        fs::create_dir(temp.0.join("z-dir")).unwrap();
        fs::write(temp.0.join("a-file"), b"x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(temp.0.join("z-dir"), temp.0.join("b-link")).unwrap();

        let listing = read_directory(&temp.0, || false).unwrap();
        assert_eq!(listing.entries[0].path.file_name().unwrap(), "z-dir");
        assert_eq!(listing.entries[0].kind, EntryKind::Directory);
        assert_eq!(listing.entries[1].path.file_name().unwrap(), "a-file");
        #[cfg(unix)]
        assert_eq!(listing.entries[2].kind, EntryKind::Link);
        assert_eq!(
            listing.bytes,
            listing
                .entries
                .iter()
                .map(|e| path_payload_bytes(&e.path))
                .sum::<usize>()
        );
    }

    #[test]
    fn rejects_the_4097th_entry_and_cancellation() {
        let temp = TempDir::new();
        for index in 0..=ENTRY_LIMIT {
            fs::write(temp.0.join(format!("{index:04}")), b"").unwrap();
        }
        assert_eq!(
            read_directory(&temp.0, || false).unwrap_err(),
            limit_error()
        );
        assert_eq!(
            read_directory(&temp.0, || true).unwrap_err(),
            "Directory loading cancelled"
        );
    }

    #[test]
    fn rejects_path_payload_over_four_mib() {
        let temp = TempDir::new();
        let mut base = temp.0.clone();
        for index in 0..5 {
            base.push(format!("{}-{index}", "x".repeat(190)));
            fs::create_dir(&base).unwrap();
        }
        for index in 0..ENTRY_LIMIT {
            fs::write(base.join(format!("{index:04}-{}", "y".repeat(250))), b"").unwrap();
        }
        assert_eq!(read_directory(&base, || false).unwrap_err(), limit_error());
    }

    #[test]
    fn loader_delivers_tagged_result_without_waiting_in_poll() {
        let temp = TempDir::new();
        fs::write(temp.0.join("file"), b"").unwrap();
        let mut loader = Loader::default();
        let ctx = egui::Context::default();
        let mut req = request(temp.0.clone());
        req.pane = 42;
        loader.submit(req.clone(), &ctx);
        let deadline = Instant::now() + Duration::from_secs(3);
        let reply = loop {
            if let Some(reply) = loader.poll() {
                break reply;
            }
            assert!(Instant::now() < deadline, "worker did not return");
            thread::yield_now();
        };
        assert_eq!(reply.request, req);
        assert_eq!(reply.result.unwrap().entries.len(), 1);
    }

    #[test]
    fn dropping_loader_does_not_wait_for_a_blocked_reader() {
        let (started_tx, started_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let release_rx = Mutex::new(Some(release_rx));
        let reader = Arc::new(move |_path: &Path, _cancelled: &dyn Fn() -> bool| {
            started_tx.send(()).unwrap();
            if let Ok(mut slot) = release_rx.lock() {
                if let Some(receiver) = slot.take() {
                    let _ = receiver.recv();
                }
            }
            Ok(Listing {
                entries: Vec::new(),
                bytes: 0,
            })
        });
        let mut loader = Loader::new_with_reader(reader);
        loader.submit(request(PathBuf::from("ignored")), &egui::Context::default());
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let start = Instant::now();
        drop(loader);
        assert!(start.elapsed() < Duration::from_millis(100));
        let _ = release_tx.send(());
    }

    #[test]
    fn worker_replaces_pending_request_and_cancels_previous_token() {
        let (started_tx, started_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let gate = Arc::new(Mutex::new(Some(release_rx)));
        let reader = Arc::new({
            let gate = Arc::clone(&gate);
            move |path: &Path, cancelled: &dyn Fn() -> bool| {
                if path == Path::new("first") {
                    started_tx.send(()).unwrap();
                    if let Ok(mut slot) = gate.lock() {
                        if let Some(receiver) = slot.take() {
                            let _ = receiver.recv();
                        }
                    }
                }
                if cancelled() {
                    return Err("cancelled".into());
                }
                Ok(Listing {
                    entries: Vec::new(),
                    bytes: 0,
                })
            }
        });
        let mut loader = Loader::new_with_reader(reader);
        let ctx = egui::Context::default();
        loader.submit(request("first".into()), &ctx);
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        loader.submit(request("second".into()), &ctx);
        // Release the blocked injected read so this single worker can consume the mailbox.
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match loader.poll() {
                Some(reply) if reply.request.path == Path::new("second") => break,
                Some(_) => {}
                None => {}
            }
            assert!(
                Instant::now() < deadline,
                "replacement request was not delivered"
            );
            thread::yield_now();
        }
    }
}
