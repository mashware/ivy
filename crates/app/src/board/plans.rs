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
struct Job {
    identity: Identity,
    what: What,
    answer: SyncSender<Answer>,
}
#[derive(Default)]
pub(crate) struct Store {
    bank: Bank,
    sender: Option<SyncSender<Job>>,
}

impl Store {
    pub(crate) fn at(
        directory: Option<PathBuf>,
        writable: bool,
        ctx: eframe::egui::Context,
    ) -> Self {
        let bank = Bank::default();
        let shared = bank.clone();
        let (sender, receiver) = mpsc::sync_channel::<Job>(8);
        let spawned = std::thread::Builder::new()
            .name("project-plans".into())
            .spawn(move || {
                let mut disk = disk::Disk::new(directory, writable);
                let restored = disk.restore();
                *shared.lock().unwrap() = restored;
                ctx.request_repaint();
                // Bounded: dropping the Store drops its sole sender; after queued jobs drain,
                // `recv` returns disconnected. The worker is not joined, so the UI does not wait.
                while let Ok(job) = receiver.recv() {
                    let answer = apply(&shared, &mut disk, &job.identity, job.what);
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
        }
    }

    pub(crate) fn published(&self, key: &Key) -> Option<Arc<Published>> {
        self.bank.lock().unwrap().get(key).cloned()
    }

    pub(crate) fn request(
        &mut self,
        identity: Result<Identity, String>,
        what: What,
        answer: SyncSender<Answer>,
        ctx: &eframe::egui::Context,
    ) {
        let identity = match identity {
            Ok(identity) => identity,
            Err(error) => {
                let _ = answer.send(Answer::Refused(error));
                return;
            }
        };
        if self.sender.is_none() {
            *self = Self::at(None, false, ctx.clone());
        }
        let Some(sender) = &self.sender else {
            let _ = answer.send(Answer::Refused("project plan worker is unavailable".into()));
            return;
        };
        if let Err(error) = sender.try_send(Job {
            identity,
            what,
            answer,
        }) {
            let job = match error {
                mpsc::TrySendError::Full(job) | mpsc::TrySendError::Disconnected(job) => job,
            };
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
    window
        .board
        .plans
        .request(identity, delivery.what, delivery.answer, ctx);
    None
}

pub(crate) fn refresh_readers(window: &mut crate::Window) {
    let bank = window.board.plans.bank.clone();
    for pane in window.all_panes_mut() {
        let identity = from_badge(pane.project.as_ref());
        let Some(control) = pane.control.as_mut() else {
            continue;
        };
        if control.plan_identity.as_ref() == Some(&identity) {
            continue;
        }
        control.plan_identity = Some(identity.clone());
        let bank = bank.clone();
        let read: panel_control::listener::ReadPlan = Arc::new(move || {
            let identity = match &identity {
                Ok(identity) => identity,
                Err(why) => return panel_control::Reply::Failed(why.clone()),
            };
            let published = bank.lock().unwrap().get(&identity.key).cloned();
            match published {
                Some(plan) => panel_control::Reply::Document(plan.document.clone()),
                None => {
                    panel_control::Reply::Text(format!("no plan published for {}", identity.name))
                }
            }
        });
        control.install_plan_reader(read);
    }
}

#[cfg(test)]
mod tests;
