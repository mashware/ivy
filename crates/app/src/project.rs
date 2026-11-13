//! Transient project identity, resolved outside the window thread.
use eframe::egui;
use std::{collections::HashMap, path::PathBuf, time::Instant};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Repository(PathBuf),
    Folder(PathBuf),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub(crate) key: Key,
    pub(crate) name: String,
    pub(crate) path: PathBuf,
}

#[derive(Default)]
pub(crate) struct Projects {
    panes: HashMap<usize, Observation>,
    worker: Option<Worker>,
}
struct Observation {
    directory: Option<PathBuf>,
    generation: u64,
    identity: Option<Identity>,
    requested: bool,
    retry_at: Instant,
    last_error: Option<String>,
}

impl Projects {
    pub(crate) fn identity(&self, pane: usize) -> Option<&Identity> {
        self.panes.get(&pane)?.identity.as_ref()
    }
}

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
const RETRY: Duration = Duration::from_secs(5);
#[derive(Clone, Debug, PartialEq, Eq)]
struct Request {
    pane: usize,
    generation: u64,
    directory: PathBuf,
}
struct Completion {
    request: Request,
    result: Result<Identity, String>,
}
#[derive(Default)]
struct Queue {
    wanted: HashMap<usize, Request>,
    pending: HashMap<usize, Request>,
    completed: HashMap<usize, Completion>,
    stopping: bool,
    last_pane: Option<usize>,
}
struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
}
struct Worker {
    shared: Arc<Shared>,
}
impl Worker {
    fn start(ctx: egui::Context) -> Result<Self, String> {
        Self::start_with(ctx, resolve)
    }
    fn start_with(
        ctx: egui::Context,
        resolver: impl Fn(&std::path::Path) -> Result<Identity, String>
        + Send
        + std::panic::RefUnwindSafe
        + 'static,
    ) -> Result<Self, String> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
        });
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("project-resolver".into())
            .spawn(move || {
                loop {
                    let request = {
                        let mut queue = thread_shared.queue.lock().unwrap();
                        while queue.pending.is_empty() && !queue.stopping {
                            // Bounded: new work notifies this condition; dropping the worker sets
                            // stopping under the same lock and notifies, so teardown wakes it too.
                            queue = thread_shared.wake.wait(queue).unwrap();
                        }
                        if queue.stopping {
                            return;
                        }
                        let pane = queue
                            .pending
                            .keys()
                            .copied()
                            .filter(|pane| queue.last_pane.is_none_or(|last| *pane > last))
                            .min()
                            .or_else(|| queue.pending.keys().copied().min())
                            .unwrap();
                        queue.last_pane = Some(pane);
                        queue.pending.remove(&pane).unwrap()
                    };
                    let result = std::panic::catch_unwind(|| resolver(&request.directory))
                        .unwrap_or_else(|_| Err("Project resolver failed unexpectedly".into()));
                    let mut queue = thread_shared.queue.lock().unwrap();
                    if queue.stopping {
                        return;
                    }
                    if queue.wanted.get(&request.pane) == Some(&request) {
                        let repaint = result.is_ok();
                        queue
                            .completed
                            .insert(request.pane, Completion { request, result });
                        if repaint {
                            ctx.request_repaint();
                        }
                    }
                }
            })
            .map_err(|e| format!("Cannot start project resolver: {e}"))?;
        Ok(Self { shared })
    }
    fn request(&self, request: Request) {
        let mut queue = self.shared.queue.lock().unwrap();
        queue.completed.remove(&request.pane);
        queue.wanted.insert(request.pane, request.clone());
        queue.pending.insert(request.pane, request);
        self.shared.wake.notify_one();
    }
    fn forget(&self, pane: usize) {
        let mut queue = self.shared.queue.lock().unwrap();
        queue.wanted.remove(&pane);
        queue.pending.remove(&pane);
        queue.completed.remove(&pane);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let mut queue = self.shared.queue.lock().unwrap();
        queue.stopping = true;
        queue.pending.clear();
        self.shared.wake.notify_one();
        // A bounded running Git operation may finish; window teardown never joins it.
    }
}
impl Projects {
    pub(crate) fn observe(
        &mut self,
        pane: usize,
        directory: Option<PathBuf>,
        now: Instant,
        ctx: &egui::Context,
    ) {
        let state = self.panes.entry(pane).or_insert_with(|| Observation {
            directory: None,
            generation: 0,
            identity: None,
            requested: false,
            retry_at: now,
            last_error: None,
        });
        if state.directory != directory {
            state.directory = directory;
            state.generation = state.generation.wrapping_add(1);
            state.identity = None;
            state.requested = false;
            state.retry_at = now;
            state.last_error = None;
            if let Some(worker) = &self.worker {
                worker.forget(pane);
            }
        }
        let Some(directory) = state.directory.clone() else {
            return;
        };
        if state.identity.is_some() || state.requested || now < state.retry_at {
            return;
        }
        if self.worker.is_none() {
            match Worker::start(ctx.clone()) {
                Ok(worker) => self.worker = Some(worker),
                Err(error) => {
                    record_error(state, error);
                    state.retry_at = now + RETRY;
                    return;
                }
            }
        }
        state.requested = true;
        self.worker.as_ref().unwrap().request(Request {
            pane,
            generation: state.generation,
            directory,
        });
    }
    pub(crate) fn poll(&mut self) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let completed = std::mem::take(&mut worker.shared.queue.lock().unwrap().completed);
        let mut changed = false;
        for completion in completed.into_values() {
            let request = completion.request;
            let Some(state) = self.panes.get_mut(&request.pane) else {
                continue;
            };
            if state.generation != request.generation
                || state.directory.as_ref() != Some(&request.directory)
            {
                continue;
            }
            state.requested = false;
            match completion.result {
                Ok(identity) => {
                    changed |= state.identity.as_ref() != Some(&identity);
                    state.identity = Some(identity);
                    state.last_error = None;
                }
                Err(error) => {
                    record_error(state, error);
                    state.retry_at = Instant::now() + RETRY;
                }
            }
        }
        changed
    }
    pub(crate) fn retain(&mut self, live: &[usize]) {
        self.panes.retain(|pane, _| {
            let keep = live.contains(pane);
            if !keep && let Some(worker) = &self.worker {
                worker.forget(*pane);
            }
            keep
        });
    }
}
fn record_error(state: &mut Observation, error: String) {
    let error: String = error
        .lines()
        .next()
        .unwrap_or("Unknown discovery error")
        .chars()
        .filter(|c| !c.is_control())
        .take(240)
        .collect();
    if state.last_error.as_ref() != Some(&error) {
        eprintln!("Project identity unavailable: {error}");
    }
    state.last_error = Some(error);
}

pub(crate) fn resolve(directory: &std::path::Path) -> Result<Identity, String> {
    resolve_with_git(directory, std::ffi::OsStr::new("git"))
}
fn resolve_with_git(
    directory: &std::path::Path,
    git: &std::ffi::OsStr,
) -> Result<Identity, String> {
    let directory = directory
        .canonicalize()
        .map_err(|e| format!("Cannot resolve console directory: {e}"))?;
    if !directory.is_dir() {
        return Err("Console directory is not a folder".into());
    }
    let mut command = std::process::Command::new(git);
    // Agent sessions commonly inherit GIT_DIR/GIT_WORK_TREE. None may redirect discovery.
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    command
        .args(["--no-pager", "-c", "core.fsmonitor=false", "-C"])
        .arg(&directory)
        .args([
            "rev-parse",
            "--is-bare-repository",
            "--path-format=absolute",
            "--git-common-dir",
        ])
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let output = crate::git_process::run(command, Duration::from_secs(2), 64 * 1024, 64 * 1024)?;
    if !output.status.success() {
        if non_repository(&output.stderr) {
            return Ok(Identity {
                key: Key::Folder(directory.clone()),
                name: basename(&directory),
                path: directory,
            });
        }
        return Err(format!(
            "Git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let separator = output
        .stdout
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or("Git returned incomplete repository metadata")?;
    let (bare, raw) = output.stdout.split_at(separator);
    if bare != b"true" && bare != b"false" {
        return Err("Git returned invalid repository metadata".into());
    }
    let raw = &raw[1..];
    let raw = raw.strip_suffix(b"\n").unwrap_or(raw);
    if raw.is_empty() {
        return Err("Git returned an empty common directory".into());
    }
    let path = git_path(raw)?
        .canonicalize()
        .map_err(|e| format!("Cannot resolve Git common directory: {e}"))?;
    let naming_path = if bare == b"false" && path.file_name().is_some_and(|name| name == ".git") {
        path.parent().unwrap_or(&path)
    } else {
        &path
    };
    Ok(Identity {
        key: Key::Repository(path.clone()),
        name: basename(naming_path),
        path,
    })
}

#[cfg(unix)]
fn git_path(raw: &[u8]) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(std::ffi::OsString::from_vec(raw.to_vec()).into())
}
#[cfg(not(unix))]
fn git_path(raw: &[u8]) -> Result<PathBuf, String> {
    std::str::from_utf8(raw)
        .map(PathBuf::from)
        .map_err(|_| "Git path is not UTF-8".into())
}

fn basename(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
fn non_repository(error: &[u8]) -> bool {
    error.starts_with(b"fatal: not a git repository (or any of the parent directories): .git")
        || error.starts_with(b"fatal: not a git repository (or any parent up to mount point ")
}

#[cfg(test)]
mod tests;
