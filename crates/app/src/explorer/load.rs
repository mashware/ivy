use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use super::pattern::Matcher;

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Listing {
    pub(crate) entries: Vec<Entry>,
    pub(crate) bytes: usize,
    pub(crate) root: Option<PathBuf>,
    pub(crate) selected: Option<PathBuf>,
    pub(crate) notice: Option<String>,
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
    Ok(Listing {
        entries,
        bytes,
        ..Default::default()
    })
}

fn reveal_path(path: &Path, cancelled: &dyn Fn() -> bool) -> Result<Listing, String> {
    let metadata = fs::metadata(path).map_err(|error| format!("Path unavailable: {error}"))?;
    let (root, selected) = if metadata.is_dir() {
        (path.to_path_buf(), None)
    } else if metadata.is_file() {
        (
            path.parent()
                .ok_or("This file has no parent directory.")?
                .to_path_buf(),
            Some(path.to_path_buf()),
        )
    } else {
        return Err("This path is not a file or directory.".into());
    };
    let mut listing = read_directory(&root, cancelled)?;
    listing.root = Some(root);
    listing.selected = selected;
    Ok(listing)
}

fn search_names(
    root: &Path,
    matcher: &Matcher,
    cancelled: &dyn Fn() -> bool,
) -> Result<Listing, String> {
    let mut pending = std::collections::VecDeque::from([root.to_path_buf()]);
    let mut listing = Listing::default();
    let mut visited = 0usize;
    'scan: while let Some(directory) = pending.pop_front() {
        if cancelled() {
            return Err("Directory loading cancelled".into());
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if directory == root => {
                return Err(format!("Directory unavailable. {error}"));
            }
            Err(_) => {
                listing
                    .notice
                    .get_or_insert_with(|| "Some folders could not be searched.".into());
                continue;
            }
        };
        for item in entries {
            if cancelled() {
                return Err("Directory loading cancelled".into());
            }
            if visited == 100_000 {
                listing.notice =
                    Some("Search stopped after 100000 entries; choose a smaller folder.".into());
                break 'scan;
            }
            visited += 1;
            let entry = match item.and_then(|item| item.file_type().map(|kind| (item.path(), kind)))
            {
                Ok((path, kind)) => Entry {
                    path,
                    kind: if kind.is_symlink() {
                        EntryKind::Link
                    } else if kind.is_dir() {
                        EntryKind::Directory
                    } else if kind.is_file() {
                        EntryKind::File
                    } else {
                        EntryKind::Other
                    },
                },
                Err(_) => {
                    listing
                        .notice
                        .get_or_insert_with(|| "Some entries could not be searched.".into());
                    continue;
                }
            };
            if entry.kind == EntryKind::Directory {
                if pending.len() < ENTRY_LIMIT {
                    pending.push_back(entry.path.clone());
                } else {
                    listing.notice =
                        Some("Search folder limit reached; narrow the search location.".into());
                }
            }
            if entry
                .path
                .file_name()
                .is_some_and(|name| matcher.matches(&name.to_string_lossy()))
            {
                let bytes = path_payload_bytes(&entry.path);
                if listing.entries.len() == ENTRY_LIMIT
                    || listing.bytes.saturating_add(bytes) > PATH_BYTES_LIMIT
                {
                    listing.notice = Some("Search result limit reached; narrow the name.".into());
                    break 'scan;
                }
                listing.bytes += bytes;
                listing.entries.push(entry);
            }
        }
    }
    listing.entries.sort_by(|a, b| {
        (b.kind == EntryKind::Directory)
            .cmp(&(a.kind == EntryKind::Directory))
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(listing)
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
    pub(crate) query: String,
    /// The query is a pattern: part of the key, so a list found as text is never shown for the
    /// same query read as a pattern.
    pub(crate) regex: bool,
    pub(crate) reveal: bool,
}

#[derive(Debug)]
pub(crate) struct Reply {
    pub(crate) request: Request,
    pub(crate) result: Result<Listing, String>,
}

struct Job {
    request: Request,
    /// The name search's query, compiled once on the interface's side. Beside the request rather
    /// than in it: the request is the cache's key, and a compiled pattern has no equality.
    matcher: Option<Arc<Matcher>>,
    cancelled: Arc<AtomicBool>,
    ctx: egui::Context,
}

type Reader = Arc<dyn Fn(&Path, &dyn Fn() -> bool) -> Result<Listing, String> + Send + Sync>;

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
    fn new_with_reader(reader: Reader) -> Self {
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
        self.submit_with(request, None, ctx);
    }

    /// Submits a name search, which the worker runs with `matcher` instead of listing a folder.
    pub(crate) fn search(&mut self, request: Request, matcher: Arc<Matcher>, ctx: &egui::Context) {
        self.submit_with(request, Some(matcher), ctx);
    }

    fn submit_with(
        &mut self,
        request: Request,
        matcher: Option<Arc<Matcher>>,
        ctx: &egui::Context,
    ) {
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
                matcher,
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
        self.result_rx.try_recv().ok()
    }

    pub(crate) fn cancel(&mut self) {
        if let Some(current) = self.current.take() {
            current.store(true, Ordering::Release);
        }
        self.startup_reply = None;
        let (lock, _) = &*self.mailbox;
        if let Ok(mut slot) = lock.lock()
            && let Some(job) = slot.take()
        {
            job.cancelled.store(true, Ordering::Release);
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.cancel();
        self.mailbox.1.notify_one();
    }
}

fn spawn_worker(
    mailbox: Mailbox,
    result_tx: SyncSender<Reply>,
    shutdown: Arc<AtomicBool>,
    reader: Reader,
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
                        // Bounded: requests notify this condition; dropping Loader sets shutdown
                        // before taking the mailbox lock, then notifies after the waiter releases it.
                        slot = match wake.wait(slot) {
                            Ok(slot) => slot,
                            Err(_) => return,
                        };
                    }
                    slot.take().expect("worker mailbox was checked")
                };
                let token = Arc::clone(&job.cancelled);
                let cancelled = || token.load(Ordering::Acquire);
                let result = if job.request.reveal {
                    reveal_path(&job.request.path, &cancelled)
                } else if let Some(matcher) = &job.matcher {
                    search_names(&job.request.path, matcher, &cancelled)
                } else {
                    reader(&job.request.path, &cancelled)
                };
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
            query: String::new(),
            regex: false,
            reveal: false,
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
        for index in 0..ENTRY_LIMIT {
            fs::write(temp.0.join(format!("{index:04}")), b"").unwrap();
        }
        assert_eq!(
            read_directory(&temp.0, || false).unwrap().entries.len(),
            ENTRY_LIMIT
        );
        fs::write(temp.0.join("overflow"), b"").unwrap();
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
    fn dropping_an_idle_loader_releases_its_worker() {
        struct Released(mpsc::Sender<()>);
        impl Drop for Released {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let (released_tx, released_rx) = mpsc::channel();
        let released = Released(released_tx);
        let loader = Loader::new_with_reader(Arc::new(move |_, _| {
            let _ = &released;
            unreachable!("an idle loader receives no directory requests")
        }));
        drop(loader);
        released_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("idle worker should release its reader after shutdown");
    }

    #[test]
    fn dropping_loader_does_not_wait_for_a_blocked_reader() {
        let (started_tx, started_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let release_rx = Mutex::new(Some(release_rx));
        let reader = Arc::new(move |_path: &Path, _cancelled: &dyn Fn() -> bool| {
            started_tx.send(()).unwrap();
            if let Ok(mut slot) = release_rx.lock()
                && let Some(receiver) = slot.take()
            {
                let _ = receiver.recv();
            }
            Ok(Listing {
                entries: Vec::new(),
                bytes: 0,
                ..Default::default()
            })
        });
        let mut loader = Loader::new_with_reader(reader);
        loader.submit(request(PathBuf::from("ignored")), &egui::Context::default());
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let (dropped_tx, dropped_rx) = mpsc::sync_channel(1);
        let dropper = thread::spawn(move || {
            drop(loader);
            let _ = dropped_tx.send(());
        });
        let dropped_before_release = dropped_rx.recv_timeout(Duration::from_secs(2)).is_ok();
        let _ = release_tx.send(());
        dropper.join().unwrap();
        assert!(
            dropped_before_release,
            "drop waited for the blocked filesystem read"
        );
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
                    if let Ok(mut slot) = gate.lock()
                        && let Some(receiver) = slot.take()
                    {
                        let _ = receiver.recv();
                    }
                }
                if cancelled() {
                    return Err("cancelled".into());
                }
                Ok(Listing {
                    entries: Vec::new(),
                    bytes: 0,
                    ..Default::default()
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
    #[test]
    fn blocked_filesystem_reader_leaves_frames_and_console_input_progressing() {
        use crate::tests::{one_frame_in, window_of_tabs};
        let temp = TempDir::new();
        let marker = temp.0.join("console-input");
        let command = format!(
            "cd '{}' && exec cat > '{}'",
            temp.0.display(),
            marker.display()
        );
        let mut window = window_of_tabs([[command.as_str(), "sleep 30"]]);
        crate::project_view::configure(&mut window, crate::settings::Entry::Normal, None);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        let deadline = Instant::now() + Duration::from_secs(5);
        let pane = window.active().panes()[0].name;
        while window.projects.directory(pane) != Some(temp.0.as_path()) {
            one_frame_in(&mut window, &ctx, area, vec![]);
            assert!(Instant::now() < deadline, "native directory unavailable");
            thread::sleep(Duration::from_millis(10));
        }
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let gate = Mutex::new(Some(release_rx));
        window.explorer.loader = Loader::new_with_reader(Arc::new(move |_, _| {
            let _ = started_tx.send(());
            if let Some(receiver) = gate.lock().unwrap().take() {
                let _ = receiver.recv();
            }
            Ok(Listing {
                entries: Vec::new(),
                bytes: 0,
                ..Default::default()
            })
        }));
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
        one_frame_in(&mut window, &ctx, area, vec![]);
        started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let before = ctx.cumulative_frame_nr();
        let pos = egui::pos2(400.0, 450.0);
        for pressed in [true, false] {
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("WHILE_BLOCKED\n".into())],
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while fs::read(&marker).unwrap_or_default() != b"WHILE_BLOCKED\n" {
            one_frame_in(&mut window, &ctx, area, vec![]);
            assert!(
                Instant::now() < deadline,
                "console input stalled behind filesystem read"
            );
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ctx.cumulative_frame_nr() > before);
        assert!(window.explorer.loading.is_some());
        release_tx.send(()).unwrap();
    }
    fn text(query: &str) -> Matcher {
        Matcher::new(query, false).unwrap()
    }

    #[test]
    fn a_name_search_with_a_pattern_lists_only_the_names_it_matches() {
        let temp = TempDir::new();
        fs::create_dir_all(temp.0.join("src")).unwrap();
        fs::write(temp.0.join("src/main.rs"), "").unwrap();
        fs::write(temp.0.join("src/domain.rs"), "").unwrap();
        fs::write(temp.0.join("main.rsx"), "").unwrap();
        let pattern = Matcher::new(r"^main\.rs$", true).unwrap();
        let found = search_names(&temp.0, &pattern, &|| false).unwrap();
        assert_eq!(
            found
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            vec![temp.0.join("src/main.rs")]
        );
        let as_text = search_names(&temp.0, &text("main.rs"), &|| false).unwrap();
        assert_eq!(
            as_text.entries.len(),
            3,
            "as text, «main.rs» is a piece of «domain.rs» and «main.rsx» too"
        );
    }

    #[test]
    fn name_search_walks_nested_folders_without_following_directory_links() {
        let temp = TempDir::new();
        fs::create_dir_all(temp.0.join("nested/NeedleFolder")).unwrap();
        fs::write(temp.0.join("nested/NeedleFile.rs"), "content").unwrap();
        fs::write(temp.0.join("unrelated.rs"), "needle").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&temp.0, temp.0.join("nested/loop")).unwrap();
        let found = search_names(&temp.0, &text("nEeDlE"), &|| false).unwrap();
        assert_eq!(found.entries.len(), 2);
        assert_eq!(found.entries[0].kind, EntryKind::Directory);
        assert!(found.entries.iter().all(|entry| {
            entry
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("Needle")
        }));
        assert!(search_names(&temp.0, &text("needle"), &|| true).is_err());
    }

    #[test]
    fn name_search_says_when_its_result_limit_is_reached() {
        let temp = TempDir::new();
        for index in 0..=ENTRY_LIMIT {
            fs::write(temp.0.join(format!("match-{index}")), "").unwrap();
        }
        let found = search_names(&temp.0, &text("match"), &|| false).unwrap();
        assert_eq!(found.entries.len(), ENTRY_LIMIT);
        assert!(found.notice.unwrap().contains("limit"));
    }

    #[test]
    fn path_reveal_distinguishes_a_directory_file_and_missing_path() {
        let temp = TempDir::new();
        fs::create_dir(temp.0.join("child")).unwrap();
        let file = temp.0.join("file.rs");
        fs::write(&file, "source").unwrap();
        let found = reveal_path(&file, &|| false).unwrap();
        assert_eq!(found.root, Some(temp.0.clone()));
        assert_eq!(found.selected, Some(file));
        let folder = reveal_path(&temp.0.join("child"), &|| false).unwrap();
        assert_eq!(folder.root, Some(temp.0.join("child")));
        assert!(folder.selected.is_none());
        assert!(reveal_path(&temp.0.join("missing"), &|| false).is_err());
    }
}
