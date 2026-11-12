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
    collections::BTreeMap,
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
    pub plan: Option<Plan>,
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
                let (events_tx, events_rx) = mpsc::channel();
                let watcher = notify::recommended_watcher(move |event| {
                    let _ = events_tx.send(event);
                });
                let mut watcher = watcher.ok();
                if let Some(w) = watcher.as_mut() {
                    use notify::Watcher;
                    let mut parents = std::collections::BTreeSet::new();
                    for path in config.values() {
                        if let Some(parent) = path.parent() {
                            parents.insert(parent.to_path_buf());
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
                    let mut notified = false;
                    while events_rx.try_recv().is_ok() {
                        notified = true;
                    }
                    let mut changed = false;
                    for (name, path) in &config {
                        let sig = fs::metadata(path)
                            .ok()
                            .map(|m| (m.len(), m.modified().ok()));
                        if notified || signatures.get(name) != Some(&sig) {
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

fn load(name: &str, path: &Path) -> Snapshot {
    let identity = path
        .parent()
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
        fs::File::open(path)
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
            plan: Some(plan),
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

/// Read task lists as inert text. A heading groups milestones; top-level tasks start milestones
/// and indented task items become their deliverables.
fn markdown_plan(text: &str) -> Option<Plan> {
    let mut title = None;
    let mut milestones: Vec<Milestone> = Vec::new();
    let mut current: Option<usize> = None;
    let mut group: Option<usize> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        let heading_depth = trimmed.bytes().take_while(|b| *b == b'#').count();
        if heading_depth > 0 && trimmed.as_bytes().get(heading_depth) == Some(&b' ') {
            let value = trimmed[heading_depth..].trim();
            if !value.is_empty() {
                if heading_depth == 1 && title.is_none() {
                    title = Some(value.to_owned());
                } else if heading_depth >= 2 {
                    let index = milestones.len();
                    milestones.push(Milestone {
                        id: None,
                        title: value.to_owned(),
                        done_when: None,
                        depends_on: Vec::new(),
                        items: Vec::new(),
                    });
                    current = Some(index);
                    group = Some(index);
                }
            }
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let body = line.trim_start();
        let Some((checked, rest)) = task(body) else {
            continue;
        };
        let item = Item {
            id: None,
            text: rest.to_owned(),
            mark: Some(if checked { Mark::Done } else { Mark::Pending }),
            reference: None,
            link: None,
        };
        if indent == 0 && group.is_none() {
            let index = milestones.len();
            milestones.push(Milestone {
                id: None,
                title: rest.to_owned(),
                done_when: None,
                depends_on: Vec::new(),
                items: vec![item],
            });
            current = Some(index);
        } else if let Some(index) = group.or(current) {
            milestones[index].items.push(item);
        }
    }
    milestones.retain(|milestone| !milestone.items.is_empty());
    if milestones.is_empty() {
        return None;
    }
    let mut anchored = false;
    for milestone in &mut milestones {
        if !anchored
            && milestone
                .items
                .iter()
                .any(|item| item.mark != Some(Mark::Done))
        {
            let item = milestone
                .items
                .iter_mut()
                .find(|item| item.mark != Some(Mark::Done))
                .unwrap();
            item.mark = Some(Mark::Current);
            anchored = true;
        }
    }
    Some(Plan {
        title,
        source: None,
        milestones,
        outside: Vec::new(),
    })
}

fn task(line: &str) -> Option<(bool, &str)> {
    let marker = line.find("[")?;
    if marker > 5 || !line[..marker].trim_end().ends_with(['-', '*', '+']) {
        return None;
    }
    let bytes = line.as_bytes();
    if bytes.get(marker + 2) != Some(&b']')
        || !matches!(bytes.get(marker + 1), Some(b' ' | b'x' | b'X'))
    {
        return None;
    }
    let rest = line.get(marker + 3..)?.trim();
    let checked = matches!(bytes.get(marker + 1), Some(b'x' | b'X'));
    Some((checked, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_tasks_form_milestones_and_heading_groups() {
        let plan = markdown_plan(
            "# Roadmap\n\n- [x] First\n  - [x] done\n  - [ ] still doing\n- [ ] Second\n\n## Later\n- [ ] One\n- [ ] Two\n",
        )
        .unwrap();
        assert_eq!(plan.title.as_deref(), Some("Roadmap"));
        assert_eq!(plan.milestones.len(), 3);
        assert_eq!(plan.milestones[0].title, "First");
        assert_eq!(plan.milestones[0].items.len(), 3);
        assert_eq!(plan.milestones[0].items[2].mark, Some(Mark::Current));
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
}
