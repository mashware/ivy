//! Bounded, inert Markdown fallback for project roadmaps.
//!
//! Reading and Git identity resolution happen on a worker. The window only drains completed
//! snapshots, so opening a large or temporarily missing file never blocks a frame.

use eframe::egui;
use panel_contract::{
    Mark,
    plan::{Item, Milestone, Plan},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

const MAX_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    pub name: String,
    pub identity: Option<crate::project::Identity>,
    pub plan: Option<Arc<Plan>>,
    pub error: Option<String>,
    pub path: PathBuf,
}

/// The latest file-fed plans, keyed by the user-facing settings name.
pub(crate) struct Reader {
    latest: Arc<Mutex<BTreeMap<String, Snapshot>>>,
    stop: Option<mpsc::Sender<()>>,
}

impl Reader {
    pub(crate) fn new(config: BTreeMap<String, PathBuf>, ctx: egui::Context) -> Self {
        let latest = Arc::new(Mutex::new(BTreeMap::new()));
        let (stop, stopped) = mpsc::channel();
        let worker_latest = latest.clone();
        let _ = std::thread::Builder::new()
            .name("roadmap-files".into())
            .spawn(move || {
                let aliases = watch_aliases(&config);
                let dirty = Arc::new(Mutex::new(BTreeSet::new()));
                let notified = dirty.clone();
                let watcher = notify::recommended_watcher(move |event| {
                    record_changes(event, &aliases, &mut notified.lock().unwrap());
                });
                let mut watcher = watcher.ok();
                if let Some(w) = watcher.as_mut() {
                    use notify::Watcher;
                    let mut parents = std::collections::BTreeSet::new();
                    for path in config.values() {
                        if let Some(parent) = path.parent() {
                            parents.insert(if parent.as_os_str().is_empty() {
                                PathBuf::from(".")
                            } else {
                                parent.to_path_buf()
                            });
                        }
                    }
                    for parent in parents {
                        let _ = w.watch(&parent, notify::RecursiveMode::NonRecursive);
                    }
                }
                let mut signatures =
                    BTreeMap::<String, Option<(u64, Option<std::time::SystemTime>)>>::new();
                loop {
                    if stopped.try_recv().is_ok() {
                        break;
                    }
                    let notified = std::mem::take(&mut *dirty.lock().unwrap());
                    let mut changed = false;
                    for (name, path) in &config {
                        let sig = fs::metadata(path)
                            .ok()
                            .map(|m| (m.len(), m.modified().ok()));
                        if notified.contains(name) || signatures.get(name) != Some(&sig) {
                            signatures.insert(name.clone(), sig);
                            let snapshot = load(name, path);
                            worker_latest.lock().unwrap().insert(name.clone(), snapshot);
                            changed = true;
                        }
                    }
                    // Directory notifications catch atomic replacement; the bounded polling pass
                    // also handles missed events and missing-file creation.
                    if changed {
                        ctx.request_repaint();
                    }
                    if stopped.recv_timeout(Duration::from_millis(350)).is_ok() {
                        break;
                    }
                }
                drop(watcher);
            });
        Self {
            latest,
            stop: Some(stop),
        }
    }

    /// Drain the current snapshots without doing filesystem or Git work on the caller's thread.
    pub(crate) fn snapshots(&mut self) -> Vec<Snapshot> {
        self.latest.lock().unwrap().values().cloned().collect()
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

// Aliases are resolved once on the worker: notify may return the canonical parent path.
fn watch_aliases(config: &BTreeMap<String, PathBuf>) -> BTreeMap<PathBuf, BTreeSet<String>> {
    let directory = std::env::current_dir().ok();
    let mut aliases: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for (name, path) in config {
        let absolute = if path.is_absolute() {
            path.clone()
        } else {
            directory
                .as_ref()
                .map_or_else(|| path.clone(), |directory| directory.join(path))
        };
        aliases
            .entry(absolute.clone())
            .or_default()
            .insert(name.clone());
        if let (Some(parent), Some(file)) = (absolute.parent(), absolute.file_name())
            && let Ok(parent) = parent.canonicalize()
        {
            aliases
                .entry(parent.join(file))
                .or_default()
                .insert(name.clone());
        }
    }
    aliases
}

fn record_changes(
    event: notify::Result<notify::Event>,
    aliases: &BTreeMap<PathBuf, BTreeSet<String>>,
    dirty: &mut BTreeSet<String>,
) {
    match event {
        Ok(event) => {
            for path in event.paths {
                if let Some(names) = aliases.get(&path) {
                    dirty.extend(names.iter().cloned());
                }
            }
        }
        Err(_) => dirty.extend(aliases.values().flatten().cloned()),
    }
}

fn load(name: &str, path: &Path) -> Snapshot {
    let identity = path
        .parent()
        .map(|p| {
            if p.as_os_str().is_empty() {
                Path::new(".")
            } else {
                p
            }
        })
        .and_then(|parent| crate::project::resolve(parent).ok())
        .map(|mut id| {
            id.name = name.to_owned();
            id
        });
    let result = (|| {
        let metadata =
            fs::metadata(path).map_err(|e| format!("roadmap file is unavailable: {e}"))?;
        if !metadata.is_file() {
            return Err("roadmap path is not a file".into());
        }
        if metadata.len() > MAX_BYTES {
            return Err(format!("roadmap file exceeds {} KiB", MAX_BYTES / 1024));
        }
        use std::io::Read;
        let mut bytes = Vec::with_capacity(MAX_BYTES as usize + 1);
        let mut open = fs::OpenOptions::new();
        open.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // A path can be replaced between metadata and open. Never let a FIFO stall this worker.
            open.custom_flags(libc::O_NONBLOCK);
        }
        open.open(path)
            .map_err(|e| format!("roadmap file could not be read: {e}"))?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("roadmap file could not be read: {e}"))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(format!("roadmap file exceeds {} KiB", MAX_BYTES / 1024));
        }
        let text =
            std::str::from_utf8(&bytes).map_err(|_| "roadmap file is not UTF-8".to_owned())?;
        markdown_plan(text).ok_or_else(|| "roadmap file has no task items".to_owned())
    })();
    match result {
        Ok(plan) => Snapshot {
            name: name.to_owned(),
            identity,
            plan: Some(Arc::new(plan)),
            error: None,
            path: path.to_path_buf(),
        },
        Err(error) => Snapshot {
            name: name.to_owned(),
            identity,
            plan: None,
            error: Some(error),
            path: path.to_path_buf(),
        },
    }
}

/// The Markdown parser owns task recognition, nesting, and inert inline text.
fn markdown_plan(text: &str) -> Option<Plan> {
    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
    struct Task {
        parent: Option<usize>,
        group: Option<usize>,
        checked: Option<bool>,
        text: String,
    }
    let mut tasks: Vec<Task> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut groups: Vec<String> = Vec::new();
    let mut group = None;
    let mut title = None;
    let mut heading: Option<(HeadingLevel, String)> = None;
    for event in Parser::new_ext(text, Options::ENABLE_TASKLISTS) {
        match event {
            Event::Start(Tag::Heading { level, .. }) if stack.is_empty() => {
                heading = Some((level, String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = heading.take() {
                    if level == HeadingLevel::H1 && title.is_none() {
                        title = Some(text);
                    } else {
                        group = Some(groups.len());
                        groups.push(text);
                    }
                }
            }
            Event::Start(Tag::Item) => {
                tasks.push(Task {
                    parent: stack.last().copied(),
                    group,
                    checked: None,
                    text: String::new(),
                });
                stack.push(tasks.len() - 1);
            }
            Event::End(TagEnd::Item) => {
                stack.pop();
            }
            Event::TaskListMarker(checked) => {
                if let Some(&at) = stack.last() {
                    tasks[at].checked = Some(checked);
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, heading)) = &mut heading {
                    heading.push_str(&text);
                } else if let Some(&at) = stack.last() {
                    tasks[at].text.push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak | Event::End(TagEnd::Paragraph) => {
                if let Some((_, heading)) = &mut heading {
                    heading.push(' ');
                } else if let Some(&at) = stack.last() {
                    tasks[at].text.push(' ');
                }
            }
            _ => {}
        }
    }
    let mut milestones: Vec<Milestone> = Vec::new();
    let mut group_to_milestone = BTreeMap::new();
    let mut task_to_milestone = BTreeMap::new();
    let mut current = false;
    for (at, task) in tasks.iter().enumerate() {
        let Some(checked) = task.checked else {
            continue;
        };
        let milestone = if let Some(group) = task.group {
            *group_to_milestone.entry(group).or_insert_with(|| {
                milestones.push(Milestone {
                    title: groups[group].clone(),
                    ..Default::default()
                });
                milestones.len() - 1
            })
        } else {
            let mut parent = task.parent;
            let mut ancestor = None;
            while let Some(at) = parent {
                if let Some(milestone) = task_to_milestone.get(&at) {
                    ancestor = Some(*milestone);
                    break;
                }
                parent = tasks[at].parent;
            }
            ancestor.unwrap_or_else(|| {
                milestones.push(Milestone {
                    title: task.text.trim().into(),
                    ..Default::default()
                });
                milestones.len() - 1
            })
        };
        task_to_milestone.insert(at, milestone);
        let mark = if checked {
            Mark::Done
        } else if !current {
            current = true;
            Mark::Current
        } else {
            Mark::Pending
        };
        milestones[milestone].items.push(Item {
            text: task.text.trim().into(),
            mark: Some(mark),
            ..Default::default()
        });
    }
    (!milestones.is_empty()).then_some(Plan {
        title,
        milestones,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let tick = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir =
                std::env::temp_dir().join(format!("ivy-roadmap-{}-{tick}", std::process::id()));
            fs::create_dir(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn markdown_tasks_form_milestones_and_heading_groups() {
        let plan = markdown_plan(
            "# Roadmap\n\n```md\n- [ ] sample, not a task\n```\n- [x] First\n  - [x] done\n  - [ ] still doing\n- [ ] Second\n\n## Later\n- [ ] One\n- [ ] Two\n",
        )
        .unwrap();
        assert_eq!(plan.title.as_deref(), Some("Roadmap"));
        assert_eq!(plan.milestones.len(), 3);
        assert_eq!(plan.milestones[0].title, "First");
        assert_eq!(plan.milestones[0].items.len(), 3);
        assert_eq!(plan.milestones[0].items[2].mark, Some(Mark::Current));
        assert_eq!(plan.milestones[0].items[2].text, "still doing");
        assert_eq!(plan.milestones[1].items[0].mark, Some(Mark::Pending));
        assert_eq!(plan.milestones[2].title, "Later");
        assert_eq!(plan.milestones[2].items.len(), 2);
    }

    #[test]
    fn only_real_markdown_tasks_count_and_all_done_has_no_current_item() {
        assert!(markdown_plan("# Empty\n- prose\n* [x] finished\n").is_some());
        assert!(markdown_plan("# Empty\n- prose\n").is_none());
        let plan = markdown_plan("- [x] done\n").unwrap();
        assert_eq!(plan.milestones[0].items[0].mark, Some(Mark::Done));
    }

    #[test]
    fn nested_markdown_tasks_count_but_code_examples_do_not() {
        let plan = markdown_plan("- [x] Milestone\n    - [ ] Nested\n\n    code\n\n~~~~~~~~md\n- [ ] example\n~~~\n- [ ] still code\n~~~~~~~~\n").unwrap();
        assert_eq!(plan.milestones.len(), 1);
        assert_eq!(plan.milestones[0].items.len(), 2);
        assert_eq!(plan.milestones[0].items[1].text, "Nested");
        assert_eq!(plan.milestones[0].items[1].mark, Some(Mark::Current));
    }

    #[test]
    fn markdown_events_handle_ordered_tasks_and_strip_inline_markup() {
        let plan = markdown_plan(
            "## **Phase**\n1. [ ] Write **code** and [review](https://example.com)\n2. [x] Done\n",
        )
        .unwrap();
        assert_eq!(plan.milestones[0].title, "Phase");
        assert_eq!(plan.milestones[0].items[0].text, "Write code and review");
        assert_eq!(plan.milestones[0].items[0].mark, Some(Mark::Current));
        assert!(plan.milestones[0].items[0].link.is_none());
    }

    #[test]
    fn missing_taskless_and_oversized_files_have_diagnostics() {
        let dir = TempDir::new();
        let missing = dir.0.join("missing.md");
        assert!(
            load("demo", &missing)
                .error
                .unwrap()
                .contains("unavailable")
        );
        let path = dir.0.join("empty.md");
        fs::write(&path, "# Roadmap\nSome prose, no tasks\n").unwrap();
        assert!(load("demo", &path).error.unwrap().contains("no task items"));
        fs::write(&path, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        assert!(load("demo", &path).error.unwrap().contains("exceeds"));
    }

    #[test]
    fn filesystem_event_storm_is_bounded_by_configured_files() {
        let first = PathBuf::from("/project/roadmap.md");
        let second = PathBuf::from("/other/roadmap.md");
        let aliases = BTreeMap::from([
            (first.clone(), BTreeSet::from(["first".into()])),
            (second, BTreeSet::from(["second".into()])),
        ]);
        let mut dirty = BTreeSet::new();
        for _ in 0..10000 {
            record_changes(
                Ok(notify::Event::new(notify::EventKind::Any).add_path(first.clone())),
                &aliases,
                &mut dirty,
            );
        }
        record_changes(
            Ok(notify::Event::new(notify::EventKind::Any)
                .add_path("/project/unrelated.log".into())),
            &aliases,
            &mut dirty,
        );
        assert_eq!(dirty, BTreeSet::from(["first".into()]));
        record_changes(
            Err(notify::Error::generic("events lost")),
            &aliases,
            &mut dirty,
        );
        assert_eq!(dirty.len(), 2);
    }

    #[test]
    fn reader_reloads_atomic_replacement_deletion_and_reappearance() {
        let dir = TempDir::new();
        let path = dir.0.join("roadmap.md");
        let replacement = dir.0.join("replacement.md");
        fs::write(&path, "# First\n- [ ] alpha\n").unwrap();
        let mut config = BTreeMap::new();
        config.insert("demo".into(), path.clone());
        let ctx = egui::Context::default();
        let mut reader = Reader::new(config, ctx);
        let wait_for = |reader: &mut Reader, expected: Option<&str>, error: bool| {
            let until = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                let snapshots = reader.snapshots();
                if let Some(snapshot) = snapshots.iter().find(|s| s.name == "demo")
                    && (expected.is_some_and(|text| {
                        snapshot.plan.as_ref().is_some_and(|p| {
                            p.milestones
                                .iter()
                                .flat_map(|m| &m.items)
                                .any(|i| i.text == text)
                        })
                    }) || (expected.is_none() && error && snapshot.plan.is_none()))
                {
                    return snapshot.clone();
                }
                assert!(
                    std::time::Instant::now() < until,
                    "reader did not reach expected snapshot"
                );
                std::thread::sleep(Duration::from_millis(25));
            }
        };
        assert!(wait_for(&mut reader, Some("alpha"), false).plan.is_some());

        fs::write(&replacement, "# Second\n- [ ] beta\n").unwrap();
        fs::rename(&replacement, &path).unwrap();
        assert!(wait_for(&mut reader, Some("beta"), false).plan.is_some());

        fs::remove_file(&path).unwrap();
        assert!(
            wait_for(&mut reader, None, true)
                .error
                .unwrap()
                .contains("unavailable")
        );

        fs::write(&path, "# Third\n- [ ] gamma\n").unwrap();
        assert!(wait_for(&mut reader, Some("gamma"), false).plan.is_some());
    }
}
