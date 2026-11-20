//! Project-shared inert plans. Parsing and persistence stay on background threads.
use crate::project::{Identity, Key};
use panel_contract::plan::Plan;
use panel_control::{Answer, What};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    time::SystemTime,
};

mod disk;

pub(crate) struct Published {
    pub(crate) plan: Plan,
    pub(crate) document: String,
    pub(crate) published_at: SystemTime,
}

type Bank = Arc<Mutex<HashMap<Key, Arc<Published>>>>;
#[cfg(windows)]
type SnapshotReaders = Arc<
    Mutex<
        HashMap<
            usize,
            (
                Result<Identity, String>,
                panel_control::windows_services::PlanPublisher,
            ),
        >,
    >,
>;
struct Job {
    identity: Identity,
    what: What,
    answer: SyncSender<Answer>,
    #[cfg(windows)]
    _release: Option<panel_control::windows_services::ReleaseToken>,
}
#[derive(Default)]
pub(crate) struct Store {
    bank: Bank,
    sender: Option<SyncSender<Job>>,
    #[cfg(windows)]
    snapshots: SnapshotReaders,
}

impl Store {
    pub(crate) fn at(
        directory: Option<PathBuf>,
        writable: bool,
        ctx: eframe::egui::Context,
    ) -> Self {
        let bank = Bank::default();
        let shared = bank.clone();
        #[cfg(windows)]
        let snapshots = SnapshotReaders::default();
        #[cfg(windows)]
        let snapshot_writers = snapshots.clone();
        let (sender, receiver) = mpsc::sync_channel::<Job>(8);
        let spawned = std::thread::Builder::new()
            .name("project-plans".into())
            .spawn(move || {
                let mut disk = disk::Disk::new(directory, writable);
                let restored = disk.restore();
                *shared.lock().unwrap() = restored;
                #[cfg(windows)]
                publish_all_snapshots(&shared, &snapshot_writers);
                ctx.request_repaint();
                #[allow(unused_mut)]
                // Bounded: dropping the Store drops its sole sender; after queued jobs drain,
                // `recv` returns disconnected. The worker is not joined, so the UI does not wait.
                while let Ok(mut job) = receiver.recv() {
                    let answer = apply(&shared, &mut disk, &job.identity, job.what);
                    #[cfg(windows)]
                    publish_matching_snapshots(&shared, &snapshot_writers, &job.identity.key);
                    #[cfg(windows)]
                    drop(job._release.take());
                    let _ = job.answer.send(answer);
                    ctx.request_repaint();
                }
            });
        if let Err(error) = spawned {
            eprintln!("Project plans unavailable: {error}");
            return Self::default();
        }
        Self {
            bank,
            sender: Some(sender),
            #[cfg(windows)]
            snapshots,
        }
    }

    pub(crate) fn published(&self, key: &Key) -> Option<Arc<Published>> {
        self.bank.lock().unwrap().get(key).cloned()
    }

    #[cfg(windows)]
    fn install_snapshot(
        &self,
        pane: usize,
        identity: Result<Identity, String>,
        publisher: panel_control::windows_services::PlanPublisher,
    ) {
        let mut snapshots = self
            .snapshots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Lock subscribers before reading the bank. A concurrent commit updates the bank, releases
        // that lock, then takes the subscriber lock to fan out; either this read sees the commit or
        // the fanout sees the newly installed subscriber.
        let current = snapshot_value(&self.bank, &identity);
        publish_snapshot(&publisher, current);
        snapshots.insert(pane, (identity, publisher));
    }

    #[cfg(windows)]
    fn retain_snapshots(&self, living: &[usize]) {
        self.snapshots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|pane, (_, publisher)| living.contains(pane) && publisher.is_alive());
    }

    pub(crate) fn request(
        &mut self,
        identity: Result<Identity, String>,
        what: What,
        answer: SyncSender<Answer>,
        ctx: &eframe::egui::Context,
        #[cfg(windows)] mut release: Option<panel_control::windows_services::ReleaseToken>,
    ) {
        let identity = match identity {
            Ok(identity) => identity,
            Err(error) => {
                #[cfg(windows)]
                drop(release.take());
                let _ = answer.send(Answer::Refused(error));
                return;
            }
        };
        if self.sender.is_none() {
            *self = Self::at(None, false, ctx.clone());
        }
        let Some(sender) = &self.sender else {
            #[cfg(windows)]
            drop(release.take());
            let _ = answer.send(Answer::Refused("project plan worker is unavailable".into()));
            return;
        };
        if let Err(error) = sender.try_send(Job {
            identity,
            what,
            answer,
            #[cfg(windows)]
            _release: release,
        }) {
            #[allow(unused_mut)]
            let mut job = match error {
                mpsc::TrySendError::Full(job) | mpsc::TrySendError::Disconnected(job) => job,
            };
            #[cfg(windows)]
            drop(job._release.take());
            let _ = job.answer.send(Answer::Refused(
                "project plan worker is busy or unavailable".into(),
            ));
        }
    }
}

fn apply(bank: &Bank, disk: &mut disk::Disk, identity: &Identity, what: What) -> Answer {
    let name = &identity.name;
    match what {
        What::PlanSet {
            document,
            plan,
            dropped,
        } => {
            let published_at = SystemTime::now();
            let saved = disk.save(identity, &document, published_at);
            bank.lock().unwrap().insert(
                identity.key.clone(),
                Arc::new(Published {
                    plan,
                    document,
                    published_at,
                }),
            );
            let mut message = format!("published for {name}");
            if let Some(note) = dropped.sentence() {
                message.push_str(&format!("; {note}"));
            }
            if let Err(reason) = saved {
                message.push_str(&format!(" (not saved: {reason})"));
            }
            Answer::Text(message)
        }
        What::PlanGet => match bank.lock().unwrap().get(&identity.key) {
            Some(plan) => Answer::Document(plan.document.clone()),
            None => Answer::Text(format!("no plan published for {name}")),
        },
        What::PlanRetire => {
            if !bank.lock().unwrap().contains_key(&identity.key) {
                return Answer::Text(format!("no plan published for {name}"));
            }
            if let Err(reason) = disk.retire(identity) {
                return Answer::Refused(reason);
            }
            bank.lock().unwrap().remove(&identity.key);
            Answer::Text(format!("retired for {name}"))
        }
        _ => Answer::Refused("that request is not a project plan request".into()),
    }
}

pub(crate) fn identity(window: &crate::Window, origin: usize) -> Result<Identity, String> {
    from_badge(
        window
            .all_panes()
            .find(|p| p.name == origin)
            .and_then(|p| p.project.as_ref()),
    )
}

fn from_badge(badge: Option<&crate::project_view::Badge>) -> Result<Identity, String> {
    let badge = badge.ok_or("this console's project is not known yet")?;
    match &badge.key {
        Key::Repository(path) => Ok(Identity {
            key: badge.key.clone(),
            name: badge.name.clone(),
            path: path.clone(),
        }),
        Key::Folder(_) => Err("this console is not in a Git repository".into()),
    }
}

pub(crate) fn dispatch(
    window: &mut crate::Window,
    origin: usize,
    delivery: panel_control::Delivery,
    ctx: &eframe::egui::Context,
) -> Option<panel_control::Delivery> {
    if !matches!(
        delivery.what,
        What::PlanSet { .. } | What::PlanGet | What::PlanRetire
    ) {
        return Some(delivery);
    }
    let identity = identity(window, origin);
    window.board.plans.request(
        identity,
        delivery.what,
        delivery.answer,
        ctx,
        #[cfg(windows)]
        delivery.release,
    );
    None
}

pub(crate) fn refresh_readers(window: &mut crate::Window) {
    #[cfg(not(windows))]
    let bank = window.board.plans.bank.clone();
    #[cfg(windows)]
    let living = window.all_panes().map(|pane| pane.name).collect::<Vec<_>>();
    #[cfg(windows)]
    window.board.plans.retain_snapshots(&living);
    #[cfg(windows)]
    let mut registrations = Vec::new();
    for pane in window.all_panes_mut() {
        let identity = from_badge(pane.project.as_ref());
        let Some(control) = pane.control.as_mut() else {
            continue;
        };
        #[cfg(not(windows))]
        if control.plan_identity.as_ref() == Some(&identity) {
            continue;
        }
        control.plan_identity = Some(identity.clone());
        #[cfg(windows)]
        {
            registrations.push((
                pane.name,
                identity.clone(),
                control.windows_plan_publisher(),
            ));
            continue;
        }
        #[cfg(not(windows))]
        {
            let bank = bank.clone();
            let read: panel_control::listener::ReadPlan = Arc::new(move || {
                let identity = match &identity {
                    Ok(identity) => identity,
                    Err(why) => return panel_control::Reply::Failed(why.clone()),
                };
                let published = bank.lock().unwrap().get(&identity.key).cloned();
                match published {
                    Some(plan) => panel_control::Reply::Document(plan.document.clone()),
                    None => panel_control::Reply::Text(format!(
                        "no plan published for {}",
                        identity.name
                    )),
                }
            });
            #[cfg(not(windows))]
            control.install_plan_reader(read);
        }
    }
    #[cfg(windows)]
    for (pane, identity, publisher) in registrations {
        window
            .board
            .plans
            .install_snapshot(pane, identity, publisher);
    }
}

#[cfg(windows)]
fn snapshot_value(bank: &Bank, identity: &Result<Identity, String>) -> Answer {
    match identity {
        Ok(identity) => match bank
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&identity.key)
            .cloned()
        {
            Some(plan) => Answer::Document(plan.document.clone()),
            None => Answer::Text(format!("no plan published for {}", identity.name)),
        },
        Err(why) => Answer::Refused(why.clone()),
    }
}

#[cfg(windows)]
fn publish_snapshot(publisher: &panel_control::windows_services::PlanPublisher, answer: Answer) {
    let result = match answer {
        Answer::Document(document) => publisher.publish_document(document),
        Answer::Text(text) => publisher.publish_text(text),
        Answer::Refused(message) => publisher.publish_failed(message),
        _ => return,
    };
    if let Err(problem) = result {
        eprintln!("Windows plan snapshot was not updated: {problem}");
    }
}

#[cfg(windows)]
fn publish_matching_snapshots(bank: &Bank, snapshots: &SnapshotReaders, key: &Key) {
    let matches = snapshots
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .values()
        .filter(|(identity, _)| identity.as_ref().is_ok_and(|identity| &identity.key == key))
        .cloned()
        .collect::<Vec<_>>();
    for (identity, publisher) in matches {
        publish_snapshot(&publisher, snapshot_value(bank, &identity));
    }
}

#[cfg(windows)]
fn publish_all_snapshots(bank: &Bank, snapshots: &SnapshotReaders) {
    let all = snapshots
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for (identity, publisher) in all {
        publish_snapshot(&publisher, snapshot_value(bank, &identity));
    }
}

#[cfg(test)]
mod tests;
