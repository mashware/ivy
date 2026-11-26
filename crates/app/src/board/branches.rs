//! Optional worktree branch names, resolved away from the window thread.
use eframe::egui;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

const INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, PartialEq, Eq)]
struct Request {
    pane: usize,
    generation: u64,
    directory: PathBuf,
}
struct Observation {
    directory: PathBuf,
    generation: u64,
    branch: Option<String>,
    due: Instant,
}
#[derive(Default)]
struct Queue {
    wanted: HashMap<usize, Request>,
    pending: HashMap<usize, Request>,
    completed: HashMap<usize, (Request, Option<String>)>,
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
    fn start(ctx: egui::Context) -> Option<Self> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
        });
        let thread = shared.clone();
        std::thread::Builder::new()
            .name("board-branch-resolver".into())
            .spawn(move || {
                loop {
                    let request = {
                        let mut q = thread.queue.lock().unwrap();
                        while q.pending.is_empty() && !q.stopping {
                            // Bounded: requests wake this worker; dropping it sets stopping and notifies the same condition variable.
                            q = thread.wake.wait(q).unwrap();
                        }
                        if q.stopping {
                            return;
                        }
                        let pane = q
                            .pending
                            .keys()
                            .copied()
                            .filter(|p| q.last_pane.is_none_or(|last| *p > last))
                            .min()
                            .or_else(|| q.pending.keys().copied().min())
                            .unwrap();
                        q.last_pane = Some(pane);
                        q.pending.remove(&pane).unwrap()
                    };
                    let branch = resolve(&request.directory);
                    let mut q = thread.queue.lock().unwrap();
                    if q.stopping {
                        return;
                    }
                    if q.wanted.get(&request.pane) == Some(&request) {
                        q.completed.insert(request.pane, (request, branch));
                        ctx.request_repaint();
                    }
                }
            })
            .ok()?;
        Some(Self { shared })
    }
    fn request(&self, request: Request) {
        let mut q = self.shared.queue.lock().unwrap();
        q.completed.remove(&request.pane);
        q.wanted.insert(request.pane, request.clone());
        q.pending.insert(request.pane, request);
        self.shared.wake.notify_one();
    }
    fn forget(&self, pane: usize) {
        let mut q = self.shared.queue.lock().unwrap();
        q.wanted.remove(&pane);
        q.pending.remove(&pane);
        q.completed.remove(&pane);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let mut q = self.shared.queue.lock().unwrap();
        q.stopping = true;
        q.pending.clear();
        self.shared.wake.notify_one();
    }
}
#[derive(Default)]
pub(super) struct Branches {
    panes: HashMap<usize, Observation>,
    worker: Option<Worker>,
    generation: u64,
}
impl Branches {
    pub(super) fn refresh(
        &mut self,
        panes: Vec<(usize, PathBuf)>,
        now: Instant,
        ctx: &egui::Context,
    ) {
        self.panes.retain(|pane, _| {
            let keep = panes.iter().any(|(id, _)| id == pane);
            if !keep && let Some(worker) = &self.worker {
                worker.forget(*pane);
            }
            keep
        });
        // Update directories before collecting results: an old completion cannot attach to a new cwd.
        for (pane, directory) in panes {
            let changed = self
                .panes
                .get(&pane)
                .is_none_or(|s| s.directory != directory);
            if changed {
                if let Some(worker) = &self.worker {
                    worker.forget(pane);
                }
                self.generation = self.generation.wrapping_add(1);
                self.panes.insert(
                    pane,
                    Observation {
                        directory,
                        generation: self.generation,
                        branch: None,
                        due: now,
                    },
                );
            }
        }
        if let Some(worker) = &self.worker {
            let completed = std::mem::take(&mut worker.shared.queue.lock().unwrap().completed);
            for (request, branch) in completed.into_values() {
                if let Some(state) = self.panes.get_mut(&request.pane)
                    && state.generation == request.generation
                    && state.directory == request.directory
                {
                    state.branch = branch;
                }
            }
        }
        if self.panes.values().any(|s| now >= s.due) && self.worker.is_none() {
            self.worker = Worker::start(ctx.clone());
        }
        for (&pane, state) in &mut self.panes {
            if now >= state.due {
                state.due = now + INTERVAL;
                if let Some(worker) = &self.worker {
                    // A distinct generation also rejects an older same-directory refresh.
                    self.generation = self.generation.wrapping_add(1);
                    state.generation = self.generation;
                    worker.request(Request {
                        pane,
                        generation: state.generation,
                        directory: state.directory.clone(),
                    });
                }
            }
        }
    }
    pub(super) fn get(&self, pane: usize) -> Option<&str> {
        self.panes.get(&pane)?.branch.as_deref()
    }
    pub(super) fn next_refresh(&self, now: Instant) -> Option<Duration> {
        self.panes
            .values()
            .map(|s| s.due.saturating_duration_since(now))
            .min()
    }
}
fn resolve(directory: &Path) -> Option<String> {
    fn git(directory: &Path, args: &[&str]) -> Option<String> {
        let mut command = std::process::Command::new("git");
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
            .arg(directory)
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let out =
            crate::git_process::run(command, Duration::from_secs(2), 64 * 1024, 64 * 1024).ok()?;
        if !out.status.success() {
            return None;
        }
        let value = std::str::from_utf8(&out.stdout).ok()?.trim();
        if value.is_empty() || value.chars().any(char::is_control) {
            None
        } else {
            Some(value.to_owned())
        }
    }
    if git(directory, &["rev-parse", "--is-bare-repository"])?.as_str() != "false" {
        return None;
    }
    git(directory, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .or_else(|| git(directory, &["rev-parse", "--short", "HEAD"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }
    fn await_branch(
        branches: &mut Branches,
        panes: Vec<(usize, PathBuf)>,
        now: Instant,
        ctx: &egui::Context,
        pane: usize,
        expected: &str,
    ) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            branches.refresh(panes.clone(), now, ctx);
            if branches.get(pane) == Some(expected) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "branch {:?}, expected {expected}",
                branches.get(pane)
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn old_directory_completion_is_discarded() {
        let ctx = egui::Context::default();
        let now = Instant::now();
        let mut branches = Branches::default();
        let old = PathBuf::from("/old");
        let new = PathBuf::from("/new");
        branches.refresh(vec![(1, old.clone())], now, &ctx);
        let generation = branches.panes[&1].generation;
        branches.refresh(vec![(1, new.clone())], now, &ctx);
        let worker = branches.worker.as_ref().unwrap();
        worker.shared.queue.lock().unwrap().completed.insert(
            1,
            (
                Request {
                    pane: 1,
                    generation,
                    directory: old,
                },
                Some("obsolete".into()),
            ),
        );
        branches.refresh(vec![(1, new)], now, &ctx);
        assert_ne!(branches.get(1), Some("obsolete"));
    }

    #[test]
    fn distinct_worktrees_and_switches_refresh() {
        let root =
            std::env::temp_dir().join(format!("board-branches-{:016x}", getrandom::u64().unwrap()));
        let repo = root.join("repo");
        let worktree = root.join("worktree");
        std::fs::create_dir_all(&repo).unwrap();
        run(&repo, &["init", "-q", "-b", "first"]);
        run(
            &repo,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "--allow-empty",
                "-qm",
                "init",
            ],
        );
        run(
            &repo,
            &[
                "worktree",
                "add",
                "-qb",
                "second",
                worktree.to_str().unwrap(),
            ],
        );
        let ctx = egui::Context::default();
        let now = Instant::now();
        let mut branches = Branches::default();
        let panes = vec![(1, repo.clone()), (2, worktree.clone())];
        await_branch(&mut branches, panes.clone(), now, &ctx, 1, "first");
        await_branch(&mut branches, panes.clone(), now, &ctx, 2, "second");
        run(&repo, &["checkout", "-qb", "third"]);
        await_branch(&mut branches, panes, now + INTERVAL, &ctx, 1, "third");
        let changed = vec![(1, worktree.clone())];
        branches.refresh(changed.clone(), now + INTERVAL, &ctx);
        assert_eq!(branches.get(1), None);
        await_branch(&mut branches, changed, now + INTERVAL, &ctx, 1, "second");
        assert_eq!(branches.get(2), None);
        drop(branches);
        std::fs::remove_dir_all(root).unwrap();
    }
}
