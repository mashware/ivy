use eframe::egui;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

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

/// While the chip shows Codex, it is read again this long after the last read finished (D132):
/// each read starts the official client, so the cadence is the cost a person pays for a live chip.
const CADENCE: Duration = Duration::from_secs(5 * 60);

/// When the next background read is due.
#[derive(Debug, PartialEq)]
enum Next {
    Now,
    After(Duration),
    Never,
}

/// Never while a read is in flight, so two clients are never asked at once; at once when nothing
/// has been read since enabling, so the chip does not wait five minutes for its first figure.
fn next_read(
    enabled: bool,
    shown: bool,
    pending: bool,
    last_finished: Option<Instant>,
    now: Instant,
) -> Next {
    if !enabled || !shown || pending {
        return Next::Never;
    }
    match last_finished.map(|finished| finished + CADENCE) {
        Some(due) if due > now => Next::After(due - now),
        _ => Next::Now,
    }
}

#[derive(Default)]
pub(super) struct AllowanceView {
    enabled: bool,
    pending: Option<Pending>,
    last_finished: Option<Instant>,
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

    /// What the chip reads: the 5-hour window is the one of 300 minutes and the weekly one the one
    /// of 10 080, in the first bucket that reports each; Codex labels neither.
    pub(super) fn chip_source(&self, now: i64) -> super::chip::Source {
        use super::chip::{Reading, Source, Allowance};
        if !SUPPORTED_PLATFORM {
            return Source::Unsupported;
        }
        if !self.enabled {
            return Source::Off;
        }
        let Some(observation) = &self.observation else {
            return Source::Waiting;
        };
        let find = |minutes: i64| {
            let windows = observation
                .buckets
                .iter()
                .flat_map(|bucket| [&bucket.primary, &bucket.secondary]);
            windows
                .flatten()
                .find(|window| window.minutes == Some(minutes))
                .map(|window| Allowance {
                    used: window.used,
                    resets_at: window.resets_at,
                    expired: window.resets_at.is_some_and(|reset| reset <= now),
                })
        };
        Source::Read(Reading {
            observed_at: observation.observed_at,
            five_hour: find(300),
            weekly: find(10_080),
        })
    }

    pub(super) fn measured_windows(&self) -> usize {
        self.painted_fixture_windows.unwrap_or_default()
    }

    /// Revokes the reads: closing AI usage no longer does, since the chip reads with it closed
    /// (D132, amending D129).
    pub(super) fn disable(&mut self) {
        self.enabled = false;
        self.observation = None;
        self.failure = None;
        self.last_finished = None;
        if let Some(pending) = &self.pending {
            pending.cancelled.store(true, Ordering::Relaxed);
        }
    }

    /// Starts the background read when it is due, or wakes the window when it will be: a window
    /// at rest paints nothing, so a deadline nobody arms is never reached.
    pub(super) fn tick(&mut self, ctx: &egui::Context, now: Instant, shown: bool) {
        if !SUPPORTED_PLATFORM {
            return;
        }
        let pending = self.pending.is_some();
        match next_read(self.enabled, shown, pending, self.last_finished, now) {
            Next::Now => self.refresh(ctx),
            Next::After(wait) => ctx.request_repaint_after(wait),
            Next::Never => {}
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
            // A thread that cannot start counts as a finished read, or the cadence would retry it
            // on every frame.
            self.last_finished = Some(Instant::now());
        }
    }

    pub(super) fn polling(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn poll(&mut self, now: Instant) {
        let Some(pending) = &self.pending else { return };
        let reply = match pending.result.try_recv() {
            Ok(reply) => reply,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(Failure::Unavailable),
        };
        let current = !pending.cancelled.load(Ordering::Relaxed) && self.enabled;
        if current {
            self.install(reply);
        }
        // A revoked read does not count, so enabling again reads at once.
        self.last_finished = current.then_some(now);
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
                    self.disable();
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
    fn disabling_rejects_a_reply_queued_before_enabling_again() {
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
        view.disable();
        assert!(cancelled.load(Ordering::Relaxed));
        view.enabled = true;
        view.poll(Instant::now());
        assert!(view.observation.is_none());
        assert!(!view.polling());
        assert_eq!(view.last_finished, None, "a revoked read is not a finished one");
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

/// What the chip is handed, on every platform: where reads exist, the windows found by length;
/// where they do not, «unsupported», which is what the popover turns into its sentence.
#[cfg(test)]
mod chip_tests {
    use super::super::chip::{Reading, Source, Allowance};
    use super::*;

    fn window(used: f64, minutes: i64, resets_at: i64) -> Option<AllowanceWindow> {
        Some(AllowanceWindow {
            used: Some(used),
            minutes: Some(minutes),
            resets_at: Some(resets_at),
        })
    }

    /// AC6: the next read is five minutes after the last one finished — not after it started —
    /// never while one is in flight, and only while enabled and on the chip.
    #[test]
    fn codex_is_read_five_minutes_after_the_last_read_finished_and_never_twice_at_once() {
        let start = Instant::now();
        let finished = Some(start);
        let at = |seconds| start + Duration::from_secs(seconds);
        assert_eq!(next_read(true, true, false, None, start), Next::Now);
        assert_eq!(
            next_read(true, true, false, finished, at(299)),
            Next::After(Duration::from_secs(1))
        );
        assert_eq!(next_read(true, true, false, finished, at(300)), Next::Now);
        assert_eq!(next_read(true, true, true, finished, at(900)), Next::Never);
        assert_eq!(next_read(true, true, true, None, start), Next::Never);
        assert_eq!(next_read(false, true, false, finished, at(900)), Next::Never);
        assert_eq!(next_read(true, false, false, finished, at(900)), Next::Never);
    }

    /// The clock starts when a read ends: the instant `poll` takes the reply.
    #[test]
    fn a_read_counts_as_finished_when_its_reply_is_taken() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut view = AllowanceView {
            enabled: true,
            pending: Some(Pending {
                result: receiver,
                cancelled: Arc::new(AtomicBool::new(false)),
            }),
            ..AllowanceView::default()
        };
        let later = Instant::now() + Duration::from_secs(7);
        view.poll(later);
        assert_eq!(view.last_finished, None, "no reply yet");
        sender.send(Err(Failure::Unavailable)).unwrap();
        view.poll(later);
        assert_eq!(view.last_finished, Some(later));
        assert!(!view.polling());
    }

    /// AC6: closing AI usage leaves the reads enabled and what they read in place; disabling them
    /// revokes both at once.
    #[test]
    fn closing_ai_usage_keeps_codex_reads_and_disabling_revokes_them() {
        let ctx = egui::Context::default();
        let mut view = super::super::View::default();
        view.allowances.enabled = true;
        view.allowances.observation = Some(Observation {
            account: Account {
                session: "s".into(),
                user: "u".into(),
                workspace: "w".into(),
                label: "label".into(),
            },
            observed_at: 1,
            buckets: Vec::new(),
        });
        view.open = true;
        view.close(&ctx);
        assert!(view.allowances.enabled);
        assert!(view.allowances.observation.is_some());
        view.allowances.disable();
        assert!(!view.allowances.enabled);
        assert!(view.allowances.observation.is_none());
    }

    #[test]
    fn the_chip_takes_the_three_hundred_minute_window_as_five_hours() {
        let mut view = AllowanceView {
            enabled: true,
            ..AllowanceView::default()
        };
        let enabled = view.chip_source(100);
        view.observation = Some(Observation {
            account: Account {
                session: "s".into(),
                user: "u".into(),
                workspace: "w".into(),
                label: "label".into(),
            },
            observed_at: 90,
            buckets: vec![
                Bucket {
                    label: "other".into(),
                    primary: window(1.0, 60, 500),
                    secondary: None,
                },
                Bucket {
                    label: "codex".into(),
                    primary: window(10_080.0, 10_080, 50),
                    secondary: window(42.0, 300, 400),
                },
            ],
        });
        let read = view.chip_source(100);
        view.enabled = false;
        let off = view.chip_source(100);
        if !SUPPORTED_PLATFORM {
            for source in [enabled, read, off] {
                assert_eq!(source, Source::Unsupported);
            }
            return;
        }
        assert_eq!(enabled, Source::Waiting);
        assert_eq!(off, Source::Off);
        assert_eq!(
            read,
            Source::Read(Reading {
                observed_at: 90,
                five_hour: Some(Allowance {
                    used: Some(42.0),
                    resets_at: Some(400),
                    expired: false,
                }),
                weekly: Some(Allowance {
                    used: Some(10_080.0),
                    resets_at: Some(50),
                    expired: true,
                }),
            })
        );
    }
}
