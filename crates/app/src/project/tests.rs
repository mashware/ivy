use super::*;
use std::{
    fs,
    path::Path,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ivy-project-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn git(path: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn checkout_nested_directory_and_worktree_share_identity() {
    let temp = Temp::new();
    let main = temp.0.join("main");
    fs::create_dir(&main).unwrap();
    git(&main, &["init", "-q"]);
    git(
        &main,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "initial",
        ],
    );
    let linked = temp.0.join("linked");
    git(
        &main,
        &["worktree", "add", "-qb", "linked", linked.to_str().unwrap()],
    );
    let nested = main.join("nested");
    fs::create_dir(&nested).unwrap();
    let identity = resolve(&main).unwrap();
    assert_eq!(identity.name, "main");
    assert_eq!(identity, resolve(&nested).unwrap());
    assert_eq!(identity, resolve(&linked).unwrap());
    let second = temp.0.join("second");
    fs::create_dir(&second).unwrap();
    git(&second, &["init", "-q"]);
    assert_ne!(identity.key, resolve(&second).unwrap().key);
}
#[test]
fn bare_and_custom_git_directories_name_from_shared_identity() {
    let temp = Temp::new();
    let bare = temp.0.join("bare.git");
    git(&temp.0, &["init", "--bare", "-q", bare.to_str().unwrap()]);
    assert_eq!(resolve(&bare).unwrap().name, "bare.git");
    let dot_git = temp.0.join(".git");
    git(
        &temp.0,
        &["init", "--bare", "-q", dot_git.to_str().unwrap()],
    );
    assert_eq!(resolve(&dot_git).unwrap().name, ".git");
    fs::remove_dir_all(dot_git).unwrap();
    let checkout = temp.0.join("checkout");
    let metadata = temp.0.join("metadata");
    git(
        &temp.0,
        &[
            "init",
            "-q",
            "--separate-git-dir",
            metadata.to_str().unwrap(),
            checkout.to_str().unwrap(),
        ],
    );
    let identity = resolve(&checkout).unwrap();
    assert_eq!(identity.name, "metadata");
    assert_eq!(identity.path, metadata.canonicalize().unwrap());
}
#[test]
fn non_git_is_exact_folder_but_git_failures_stay_neutral() {
    let temp = Temp::new();
    let nested = temp.0.join("nested");
    fs::create_dir(&nested).unwrap();
    let identity = resolve(&temp.0).unwrap();
    assert!(matches!(identity.key, Key::Folder(_)));
    assert_eq!(identity, resolve(&temp.0).unwrap());
    assert_ne!(identity.key, resolve(&nested).unwrap().key);
    assert!(resolve_with_git(&temp.0, std::ffi::OsStr::new("/missing/ivy-project-git")).is_err());
    assert!(!non_repository(
        b"fatal: detected dubious ownership in repository"
    ));
    assert!(!non_repository(b"fatal: not a git repository: broken.git"));
    assert!(resolve(&temp.0.join("missing")).is_err());
    fs::write(nested.join(".git"), "gitdir: /missing/ivy-metadata\n").unwrap();
    assert!(
        resolve(&nested).is_err(),
        "broken Git metadata must stay neutral"
    );
}
fn state(directory: &Path, generation: u64) -> Observation {
    Observation {
        directory: Some(directory.into()),
        generation,
        identity: None,
        requested: true,
        retry_at: Instant::now(),
        last_error: None,
    }
}
fn request(directory: &Path, generation: u64) -> Request {
    Request {
        pane: 1,
        generation,
        directory: directory.into(),
    }
}
fn inert_worker() -> Worker {
    Worker {
        shared: Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
        }),
    }
}
#[test]
fn requests_coalesce_and_stale_completions_cannot_replace_latest_source() {
    let temp = Temp::new();
    let worker = inert_worker();
    let a = request(&temp.0.join("a"), 1);
    let b = request(&temp.0.join("b"), 2);
    let c = request(&temp.0, 3);
    worker.request(a.clone());
    worker.request(b);
    worker.request(c.clone());
    {
        let queue = worker.shared.queue.lock().unwrap();
        assert_eq!(queue.pending.len(), 1);
        assert_eq!(queue.pending[&1], c);
    }
    let mut projects = Projects::default();
    projects.panes.insert(1, state(&temp.0, 3));
    projects.worker = Some(worker);
    let identity = resolve(&temp.0).unwrap();
    projects
        .worker
        .as_ref()
        .unwrap()
        .shared
        .queue
        .lock()
        .unwrap()
        .completed
        .insert(
            1,
            Completion {
                request: a,
                result: Ok(identity.clone()),
            },
        );
    assert!(!projects.poll());
    assert!(projects.identity(1).is_none());
    projects
        .worker
        .as_ref()
        .unwrap()
        .shared
        .queue
        .lock()
        .unwrap()
        .completed
        .insert(
            1,
            Completion {
                request: c,
                result: Ok(identity.clone()),
            },
        );
    assert!(projects.poll());
    assert_eq!(projects.identity(1), Some(&identity));
    projects.retain(&[]);
    let queue = projects
        .worker
        .as_ref()
        .unwrap()
        .shared
        .queue
        .lock()
        .unwrap();
    assert!(queue.wanted.is_empty() && queue.pending.is_empty() && queue.completed.is_empty());
    assert!(projects.identity(1).is_none());
}
#[test]
fn absent_metadata_is_lazy_and_changed_source_clears_badge() {
    let temp = Temp::new();
    let ctx = egui::Context::default();
    let now = Instant::now();
    let mut projects = Projects::default();
    projects.observe(1, None, now, &ctx);
    assert!(projects.worker.is_none());
    projects.panes.insert(1, state(&temp.0, 1));
    projects.panes.get_mut(&1).unwrap().identity = Some(resolve(&temp.0).unwrap());
    projects.observe(1, None, now, &ctx);
    assert!(projects.identity(1).is_none());
    assert!(projects.worker.is_none());
}
#[test]
fn errors_retry_only_on_later_observations_after_five_seconds() {
    let temp = Temp::new();
    let ctx = egui::Context::default();
    let now = Instant::now();
    let mut projects = Projects::default();
    projects.panes.insert(1, state(&temp.0, 1));
    projects.worker = Some(inert_worker());
    projects
        .worker
        .as_ref()
        .unwrap()
        .shared
        .queue
        .lock()
        .unwrap()
        .completed
        .insert(
            1,
            Completion {
                request: request(&temp.0, 1),
                result: Err("Git timed out".into()),
            },
        );
    assert!(!projects.poll());
    projects.observe(1, Some(temp.0.clone()), now, &ctx);
    assert!(
        projects
            .worker
            .as_ref()
            .unwrap()
            .shared
            .queue
            .lock()
            .unwrap()
            .pending
            .is_empty()
    );
    projects.observe(1, Some(temp.0.clone()), Instant::now() + RETRY, &ctx);
    assert_eq!(
        projects
            .worker
            .as_ref()
            .unwrap()
            .shared
            .queue
            .lock()
            .unwrap()
            .pending
            .len(),
        1
    );
}

#[test]
fn busy_worker_keeps_latest_request_and_does_not_publish_closed_panes() {
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = Mutex::new(release_rx);
    let worker = Worker::start_with(egui::Context::default(), move |path| {
        started_tx.send(path.to_owned()).unwrap();
        release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        Ok(Identity {
            key: Key::Folder(path.into()),
            name: basename(path),
            path: path.into(),
        })
    })
    .unwrap();
    let a = request(Path::new("a"), 1);
    let b = request(Path::new("b"), 2);
    let c = request(Path::new("c"), 3);
    worker.request(a);
    assert_eq!(
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        PathBuf::from("a")
    );
    worker.request(b);
    worker.request(c.clone());
    release_tx.send(()).unwrap();
    assert_eq!(
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        c.directory
    );
    assert!(worker.shared.queue.lock().unwrap().completed.is_empty());
    worker.forget(1);
    release_tx.send(()).unwrap();
    // A following job is a barrier proving the closed pane's job has completed.
    let mut barrier = request(Path::new("barrier"), 1);
    barrier.pane = 2;
    worker.request(barrier);
    assert_eq!(
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        PathBuf::from("barrier")
    );
    assert!(
        !worker
            .shared
            .queue
            .lock()
            .unwrap()
            .completed
            .contains_key(&1)
    );
    release_tx.send(()).unwrap();
}

#[cfg(unix)]
#[test]
fn non_utf8_repository_paths_group_checkout_subdirectory_and_worktree() {
    use std::os::unix::ffi::OsStringExt;
    let temp = Temp::new();
    let main = temp
        .0
        .join(std::ffi::OsString::from_vec(b"project-\xff".to_vec()));
    fs::create_dir(&main).unwrap();
    git(&main, &["init", "-q"]);
    git(
        &main,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "initial",
        ],
    );
    let nested = main.join("nested");
    fs::create_dir(&nested).unwrap();
    let linked = temp
        .0
        .join(std::ffi::OsString::from_vec(b"linked-\xfe".to_vec()));
    let output = Command::new("git")
        .arg("-C")
        .arg(&main)
        .args(["worktree", "add", "-qb", "linked"])
        .arg(&linked)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let identity = resolve(&main).unwrap();
    assert_eq!(
        identity.key,
        Key::Repository(main.join(".git").canonicalize().unwrap())
    );
    assert_eq!(identity.path, main.join(".git").canonicalize().unwrap());
    assert_eq!(identity.name, "project-\u{fffd}");
    assert_eq!(identity, resolve(&nested).unwrap());
    assert_eq!(identity, resolve(&linked).unwrap());
}
