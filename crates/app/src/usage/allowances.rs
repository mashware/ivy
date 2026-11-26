use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod native;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod wire;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use wire::bind_observation;

const SUPPORTED_PLATFORM: bool = cfg!(all(target_os = "linux", target_arch = "x86_64"));

#[derive(Clone, Debug, Eq, PartialEq)]
struct Account {
    session: String,
    user: String,
    workspace: String,
    label: String,
}

impl Account {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn matches(&self, other: &Self) -> bool {
        self.session == other.session
            && self.user == other.user
            && self.workspace == other.workspace
    }
}

#[derive(Debug)]
struct Observation {
    account: Account,
    observed_at: i64,
    buckets: Vec<Bucket>,
}

#[derive(Debug)]
struct Bucket {
    label: String,
    primary: Option<AllowanceWindow>,
    secondary: Option<AllowanceWindow>,
}

#[derive(Debug)]
struct AllowanceWindow {
    used: Option<f64>,
    minutes: Option<i64>,
    resets_at: Option<i64>,
}

#[derive(Debug)]
enum Failure {
    Unsupported,
    Unavailable,
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    SameAccount(Account),
}

type Reply = Result<Observation, Failure>;

struct Pending {
    result: mpsc::Receiver<Reply>,
    cancelled: Arc<AtomicBool>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub(super) struct AllowanceView {
    enabled: bool,
    pending: Option<Pending>,
    observation: Option<Observation>,
    failure: Option<Failure>,
    painted_fixture_windows: Option<usize>,
}

impl AllowanceView {
    pub(super) fn prepare_measurement(&mut self) {
        let now = chrono::Utc::now().timestamp();
        self.painted_fixture_windows = Some(0);
        self.observation = Some(Observation {
            account: Account {
                session: "fixture-session".into(),
                user: "fixture-user".into(),
                workspace: "fixture-workspace".into(),
                label: "fixture@example.invalid · Example workspace".into(),
            },
            observed_at: now,
            buckets: vec![Bucket {
                label: "codex".into(),
                primary: Some(AllowanceWindow {
                    used: Some(20.0),
                    minutes: Some(300),
                    resets_at: Some(now + 3600),
                }),
                secondary: Some(AllowanceWindow {
                    used: Some(67.0),
                    minutes: Some(10080),
                    resets_at: Some(now + 7200),
                }),
            }],
        });
    }

    pub(super) fn measured_windows(&self) -> usize {
        self.painted_fixture_windows.unwrap_or_default()
    }

    pub(super) fn close(&mut self) {
        self.enabled = false;
        self.observation = None;
        self.failure = None;
        if let Some(pending) = &self.pending {
            pending.cancelled.store(true, Ordering::Relaxed);
        }
    }

    fn refresh(&mut self, context: &egui::Context) {
        if !self.enabled || !SUPPORTED_PLATFORM || self.pending.is_some() {
            return;
        }
        self.failure = None;
        let (sender, result) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = cancelled.clone();
        let ctx = context.clone();
        if std::thread::Builder::new()
            .name("codex-allowances".into())
            .spawn(move || {
                #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
                let reply = native::read(cancel);
                #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
                let reply = {
                    let _ = cancel;
                    Err(Failure::Unsupported)
                };
                let _ = sender.send(reply);
                ctx.request_repaint();
            })
            .is_ok()
        {
            self.pending = Some(Pending { result, cancelled });
        } else {
            self.install(Err(Failure::Unavailable));
        }
    }

    pub(super) fn polling(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn poll(&mut self) {
        let Some(pending) = &self.pending else { return };
        let reply = match pending.result.try_recv() {
            Ok(reply) => reply,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(Failure::Unavailable),
        };
        if !pending.cancelled.load(Ordering::Relaxed) && self.enabled {
            self.install(reply);
        }
        self.pending = None;
    }

    fn install(&mut self, reply: Reply) {
        self.failure = match reply {
            Ok(observation) => {
                self.observation = Some(observation);
                None
            }
            Err(failure) => {
                let same_context = match &failure {
                    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
                    Failure::SameAccount(account) => self
                        .observation
                        .as_ref()
                        .is_some_and(|observation| account.matches(&observation.account)),
                    _ => false,
                };
                if !same_context {
                    self.observation = None;
                }
                Some(failure)
            }
        };
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui) {
        ui.strong("Codex · Official allowances");
        if let Some(maximum) = &mut self.painted_fixture_windows {
            if let Some(observation) = &self.observation {
                *maximum = (*maximum).max(observation.show(ui));
            }
            return;
        }
        if !SUPPORTED_PLATFORM {
            ui.label("Official allowance reads are unsupported on this platform.");
            return;
        }
        if self.enabled {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.pending.is_none(),
                        egui::Button::new("Refresh allowances"),
                    )
                    .clicked()
                {
                    self.refresh(ui.ctx());
                }
                if ui.button("Disable allowance reads").clicked() {
                    self.close();
                }
            });
        } else {
            ui.label("Allow Ivy to ask the installed Codex client for the account’s allowance.");
            if ui.button("Enable allowance reads").clicked() {
                self.enabled = true;
            }
        }
        if !self.enabled {
            return;
        }
        if self.pending.is_some() {
            ui.label("Checking Codex account and allowances…");
            return;
        }
        if let Some(failure) = &self.failure {
            ui.label(match failure {
                Failure::Unsupported => {
                    "This Codex installation does not support verified allowance reads."
                }
                _ if self.observation.is_some() => {
                    "Stale · Could not refresh the last successful reading."
                }
                _ => "Allowances unavailable for this reading.",
            });
        }
        if let Some(observation) = &self.observation {
            observation.show(ui);
        }
    }
}

impl Observation {
    fn show(&self, ui: &mut egui::Ui) -> usize {
        let now = chrono::Utc::now().timestamp();
        let mut painted = 0;
        ui.label(&self.account.label);
        if let Some(observed) = chrono::DateTime::from_timestamp(self.observed_at, 0) {
            ui.small(format!(
                "Last successful reading: {}",
                observed
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S %Z")
            ));
        }
        if self.buckets.is_empty() {
            ui.label("No allowance buckets reported.");
        }
        for bucket in &self.buckets {
            ui.strong(&bucket.label);
            if bucket.primary.is_none() && bucket.secondary.is_none() {
                ui.label("No windows reported.");
            }
            for (label, window) in [
                ("Primary", &bucket.primary),
                ("Secondary", &bucket.secondary),
            ] {
                if let Some(window) = window {
                    window.show(ui, label, now);
                    painted += 1;
                }
            }
        }
        painted
    }
}

impl AllowanceWindow {
    fn show(&self, ui: &mut egui::Ui, label: &str, now: i64) {
        let used = self
            .used
            .map_or_else(|| "usage unreported".into(), |used| format!("{used}% used"));
        let duration = self.minutes.map_or_else(
            || "duration unreported".into(),
            |minutes| format!("{minutes} min"),
        );
        let reset = match self
            .resets_at
            .and_then(|reset| chrono::DateTime::from_timestamp(reset, 0))
        {
            Some(reset) if reset.timestamp() <= now => "expired".into(),
            Some(reset) => {
                ui.ctx()
                    .request_repaint_after(Duration::from_secs(reset.timestamp().abs_diff(now)));
                format!(
                    "resets {}",
                    reset
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S %Z")
                )
            }
            None => "reset unreported".into(),
        };
        ui.label(format!("{label} · {used} · {duration} · {reset}"));
    }
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::*;

    fn account(user: &str) -> Account {
        Account {
            session: user.to_owned(),
            user: user.to_owned(),
            workspace: "shared".into(),
            label: "Same workspace".into(),
        }
    }

    fn observation(user: &str) -> Observation {
        Observation {
            account: account(user),
            observed_at: 42,
            buckets: Vec::new(),
        }
    }

    #[test]
    fn same_account_failure_retains_the_original_timestamp() {
        let mut view = AllowanceView::default();
        view.install(Ok(observation("a")));
        view.install(Err(Failure::SameAccount(account("a"))));
        assert_eq!(view.observation.as_ref().unwrap().observed_at, 42);
        assert!(view.failure.is_some());
        view.install(Err(Failure::SameAccount(account("b"))));
        assert!(view.observation.is_none());
    }

    #[test]
    fn close_rejects_a_reply_queued_before_reopening() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut view = AllowanceView {
            enabled: true,
            pending: Some(Pending {
                result: receiver,
                cancelled: cancelled.clone(),
            }),
            ..AllowanceView::default()
        };
        sender.send(Ok(observation("a"))).unwrap();
        view.close();
        assert!(cancelled.load(Ordering::Relaxed));
        view.enabled = true;
        view.poll();
        assert!(view.observation.is_none());
        assert!(!view.polling());
    }

    #[test]
    fn paint_identifies_stale_and_expired_results_separately() {
        let mut view = AllowanceView {
            enabled: true,
            ..AllowanceView::default()
        };
        let mut snapshot = observation("a");
        snapshot.buckets.push(Bucket {
            label: "Fixture quota".into(),
            primary: Some(AllowanceWindow {
                used: Some(0.0),
                minutes: Some(300),
                resets_at: Some(1),
            }),
            secondary: None,
        });
        view.install(Ok(snapshot));
        view.install(Err(Failure::SameAccount(account("a"))));
        let mut output = egui::Context::default().run_ui(Default::default(), |ui| view.show(ui));
        output.textures_delta.clear();
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&"Same workspace"));
        assert!(labels.contains(&"Stale · Could not refresh the last successful reading."));
        assert!(labels.contains(&"Primary · 0% used · 300 min · expired"));
    }
}
