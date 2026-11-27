use super::quota_broker::{Broker, Snapshot, Wake, WindowState};
use super::quota_setup::Preview;
use eframe::egui;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Default)]
pub(super) struct QuotaView {
    enabled: Arc<AtomicBool>,
    directory: String,
    wake: Option<Arc<Wake>>,
    pending: Option<mpsc::Receiver<io::Result<Update>>>,
    launching: bool,
    preview: Option<Preview>,
    brokers: Vec<(usize, Arc<Broker>)>,
    disconnected: bool,
    error: Option<String>,
}

enum Update {
    Preview(Preview),
    Launch(Launch),
}

pub(super) struct Launch {
    pub(super) overlay: String,
    pub(super) gate: Startup,
}

pub(crate) struct Startup {
    preview: Preview,
    broker: Arc<Broker>,
    overlay: Arc<Overlay>,
    enabled: Arc<AtomicBool>,
}

struct Overlay {
    path: PathBuf,
    _directory: panel_contract::PanelDir,
}

pub(crate) struct Lease {
    broker: Arc<Broker>,
    _overlay: Arc<Overlay>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.broker.stop();
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

impl Startup {
    pub(crate) fn endpoint(&self) -> &Path {
        self.broker.endpoint()
    }

    pub(crate) fn lease(&self) -> Lease {
        Lease {
            broker: Arc::clone(&self.broker),
            _overlay: Arc::clone(&self.overlay),
        }
    }

    pub(crate) fn verify(&self) -> io::Result<()> {
        self.preview.recheck()?;
        if matches!(self.broker.snapshot(), Snapshot::Disconnected)
            || !self.enabled.load(Ordering::Acquire)
        {
            return Err(io::Error::other("Claude allowance reporting was disabled."));
        }
        Ok(())
    }

    pub(crate) fn bind(self, pty: &term_core::Pty) {
        if pty
            .foreground_scope()
            .and_then(|scope| self.broker.attach(scope, self.preview.program))
            .is_err()
        {
            self.broker.startup_failed();
        }
    }

    pub(crate) fn fail(&self) {
        self.broker.startup_failed();
    }
}

impl QuotaView {
    #[cfg(test)]
    pub(super) fn install_fixture(&mut self, broker: Arc<Broker>, wake: Arc<Wake>) {
        self.enabled.store(true, Ordering::Release);
        self.wake = Some(wake);
        self.install(0, broker);
    }

    pub(super) fn open(&mut self, ctx: &egui::Context) {
        if self.directory.is_empty() {
            self.directory = std::env::current_dir()
                .ok()
                .and_then(|directory| directory.into_os_string().into_string().ok())
                .unwrap_or_default();
        }
        self.wake_for(ctx).set_visible(true);
    }

    fn wake_for(&mut self, ctx: &egui::Context) -> Arc<Wake> {
        Arc::clone(self.wake.get_or_insert_with(|| {
            let ctx = ctx.clone();
            Wake::new(move || ctx.request_repaint())
        }))
    }

    pub(super) fn close(&mut self) {
        if self.launching {
            self.pending = None;
            self.launching = false;
        }
        if let Some(wake) = &self.wake {
            wake.set_visible(false);
        }
    }

    fn request(
        &mut self,
        ctx: &egui::Context,
        operation: impl FnOnce() -> io::Result<Update> + Send + 'static,
    ) {
        let (send, receive) = mpsc::channel();
        let wake = self.wake_for(ctx);
        match std::thread::Builder::new()
            .name("claude-quota-setup".into())
            .spawn(move || {
                if send.send(operation()).is_ok() {
                    wake.changed();
                }
            }) {
            Ok(_) => {
                self.error = None;
                self.pending = Some(receive);
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn poll(&mut self) -> Option<Launch> {
        let result = match self.pending.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(io::Error::other("Claude allowance setup stopped."))
            }
        };
        self.pending = None;
        self.launching = false;
        match result {
            Ok(Update::Preview(preview)) => self.preview = Some(preview),
            Ok(Update::Launch(launch)) => return Some(launch),
            Err(error) => {
                self.error = Some(error.to_string());
            }
        }
        None
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui) -> Option<Launch> {
        let ready = self.poll();
        ui.strong("Claude Code · Subscription allowances");
        if let Some(error) = &self.error {
            ui.label(format!("Claude allowance unavailable · {error}"));
        }
        if self.pending.is_some() {
            ui.label("Preparing Claude allowance settings…");
        }
        if !self.enabled.load(Ordering::Acquire) {
            ui.small("For unmanaged Claude profiles. Claude Code’s /status shows managed settings under Setting sources.");
            if ui
                .button("Enable for this unmanaged Claude profile")
                .clicked()
            {
                self.enabled.store(true, Ordering::Release);
                ui.ctx().request_repaint();
            }
            return None;
        }
        if ui.button("Disable allowance reporting").clicked() {
            self.disable();
            return None;
        }
        if let Some(wake) = &self.wake {
            wake.begin_frame();
        }
        self.paint_snapshots(ui);
        if let Some(preview) = &self.preview {
            ui.label(format!("Launch in {}", preview.directory.display()));
            match preview.command() {
                Some(command) => {
                    ui.label("Existing statusline command for this launch");
                    ui.code(command);
                    for (name, value) in preview.options() {
                        ui.small(format!("{name} · {value}"));
                    }
                    ui.small("Edits to the statusline apply on relaunch. Preview it again before using allowance reporting.");
                }
                None => {
                    ui.small("Claude has no custom statusline for this launch. A custom statusline can hide some of its built-in footer hints.");
                }
            }
            if ui
                .button("Launch Claude with allowance reporting")
                .clicked()
            {
                let preview = self.preview.take().expect("preview displayed");
                let wake = self.wake_for(ui.ctx());
                wake.set_visible(true);
                let enabled = Arc::clone(&self.enabled);
                self.request(ui.ctx(), move || {
                    let directory = panel_contract::PanelDir::create()?;
                    let path = directory.path().join("claude-allowance-settings.json");
                    let overlay = Arc::new(Overlay {
                        path: path.clone(),
                        _directory: directory,
                    });
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(&path)?;
                    file.write_all(preview.overlay()?.as_bytes())?;
                    let overlay_path = path.into_os_string().into_string().map_err(|_| {
                        io::Error::other("The Ivy quota settings path is not UTF-8.")
                    })?;
                    let broker = Arc::new(Broker::prepare(wake)?);
                    Ok(Update::Launch(Launch {
                        overlay: overlay_path,
                        gate: Startup {
                            preview,
                            broker,
                            overlay,
                            enabled,
                        },
                    }))
                });
                self.launching = self.pending.is_some();
            }
            if ui.button("Cancel preview").clicked() {
                self.preview = None;
            }
        } else {
            ui.add_enabled(
                self.pending.is_none(),
                egui::TextEdit::singleline(&mut self.directory)
                    .hint_text("Absolute launch directory"),
            );
            if ui
                .add_enabled(
                    self.pending.is_none(),
                    egui::Button::new("Preview Claude launch"),
                )
                .clicked()
            {
                let directory = PathBuf::from(&self.directory);
                self.request(ui.ctx(), move || {
                    if !directory.is_absolute() {
                        return Err(io::Error::other("Choose an absolute launch directory."));
                    }
                    Preview::read(&directory).map(Update::Preview)
                });
            }
        }
        ready
    }

    fn disable(&mut self) {
        self.enabled.store(false, Ordering::Release);
        self.pending = None;
        self.launching = false;
        self.preview = None;
        for (_, broker) in self.brokers.drain(..) {
            broker.stop();
        }
        self.disconnected = false;
    }

    /// Called on every frame before the chip reads the launches: clearing the pending wake is what
    /// lets the next change schedule its own repaint.
    pub(super) fn chip_frame(&self, shown: bool) {
        if let Some(wake) = &self.wake {
            let shown = shown && self.enabled.load(Ordering::Acquire);
            wake.set_chip(shown);
            if shown {
                wake.begin_frame();
            }
        }
    }

    /// What the chip reads: the most recently updated launch's snapshot, since several launches can
    /// report and none of them is verified to be the account the person means.
    pub(super) fn chip_source(&self) -> super::chip::Source {
        use super::chip::{Allowance, Reading, Source};
        if !self.enabled.load(Ordering::Acquire) {
            return Source::Off;
        }
        let window = |state: WindowState| match state {
            WindowState::Absent => None,
            WindowState::Expired => Some(Allowance {
                used: None,
                resets_at: None,
                expired: true,
            }),
            WindowState::Reported(window) => Some(Allowance {
                used: window.used_percentage,
                resets_at: window.resets_at,
                expired: false,
            }),
        };
        let latest = self
            .brokers
            .iter()
            .filter_map(|(_, broker)| match broker.snapshot() {
                Snapshot::Observed {
                    received_at,
                    five_hour,
                    seven_day,
                } => Some(Reading {
                    observed_at: received_at,
                    five_hour: window(five_hour),
                    weekly: window(seven_day),
                }),
                _ => None,
            })
            .max_by_key(|reading| reading.observed_at);
        latest.map_or(Source::Waiting, Source::Read)
    }

    pub(super) fn install(&mut self, name: usize, broker: Arc<Broker>) {
        self.brokers.push((name, broker));
    }

    pub(super) fn failed(&mut self, message: String) {
        self.error = Some(message);
    }

    fn paint_snapshots(&mut self, ui: &mut egui::Ui) {
        self.brokers.retain(|(name, broker)| {
            let snapshot = broker.snapshot();
            if matches!(snapshot, Snapshot::Disconnected) {
                self.disconnected = true;
                return false;
            }
            ui.label(format!("Claude launch {}", name + 1));
            match snapshot {
                Snapshot::Waiting => {
                    ui.small("Waiting for Claude statusline");
                }
                Snapshot::ProducerError => {
                    ui.label("Claude allowance producer error");
                }
                Snapshot::Observed {
                    received_at,
                    five_hour,
                    seven_day,
                } => {
                    ui.small("Producer snapshot · account unverified");
                    if let Some(time) = chrono::DateTime::from_timestamp(received_at, 0) {
                        ui.small(format!(
                            "Observed {}",
                            time.with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M:%S %Z")
                        ));
                    }
                    if matches!(five_hour, WindowState::Absent)
                        && matches!(seven_day, WindowState::Absent)
                    {
                        ui.small("Claude reported no windows yet");
                    } else {
                        paint_window(ui, "5 hours", five_hour);
                        paint_window(ui, "7 days", seven_day);
                    }
                }
                Snapshot::Disconnected => unreachable!(),
            }
            true
        });
        if self.disconnected {
            ui.small("A Claude allowance producer disconnected.");
        }
    }
}

impl Launch {
    pub(super) fn program(&self) -> &Path {
        &self.gate.preview.program
    }

    pub(super) fn directory(&self) -> &Path {
        &self.gate.preview.directory
    }

    pub(super) fn path(&self) -> &std::ffi::OsStr {
        &self.gate.preview.path
    }

    pub(super) fn broker(&self) -> &Arc<Broker> {
        &self.gate.broker
    }
}

fn paint_window(ui: &mut egui::Ui, label: &str, state: WindowState) {
    let value = match state {
        WindowState::Absent => "Unavailable".to_owned(),
        WindowState::Expired => "Expired".to_owned(),
        WindowState::Reported(window) => {
            let percent = window.used_percentage.map_or_else(
                || "Usage unavailable".to_owned(),
                |percent| format!("{percent:.1}% used"),
            );
            let reset = window
                .resets_at
                .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
                .map_or_else(
                    || "Reset unavailable".to_owned(),
                    |time| {
                        format!(
                            "Resets {}",
                            time.with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M %Z")
                        )
                    },
                );
            format!("{percent} · {reset}")
        }
    };
    ui.label(format!("{label} · {value}"));
}

#[cfg(target_os = "linux")]
pub(super) fn launch_in_window(window: &mut crate::Window, ctx: &egui::Context, launch: Launch) {
    let started = std::time::Instant::now();
    let result = (|| -> term_core::Result<_> {
        let program = launch
            .program()
            .to_str()
            .ok_or("Claude Code executable path is not UTF-8")?
            .to_owned();
        let directory = launch.directory().to_owned();
        let path = launch.path().to_owned();
        let broker = std::sync::Arc::clone(launch.broker());
        let name = window.next_pane_name;
        window.next_pane_name += 1;
        let tab_name = window.next_name;
        window.next_name += 1;
        let pane = crate::opening::launch_claude_quota(
            ctx,
            crate::opening::Launch {
                program: &program,
                args: &["--settings".into(), launch.overlay],
                directory: Some(&directory),
                path: Some(&path),
            },
            crate::panel_or_none(panel_contract::PanelDir::create()),
            name,
            window.control.read_activity(),
            launch.gate,
        )?;
        let layout = crate::OpeningLayout {
            columns: 1,
            rows: 1,
        };
        let tab = crate::Tab::with_panes(vec![pane], &layout, tab_name);
        window.usage.quota.install(name, broker);
        Ok(tab)
    })();
    match result {
        Ok(tab) => {
            window.usage.close(ctx);
            window.bring_to_front(Ok(tab), ctx);
        }
        Err(error) => window.usage.quota.failed(error.to_string()),
    }
    window.timed_separately += window.usage.timing.record(0, started.elapsed());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn opening_and_closing_the_view_controls_its_quota_wakes() {
        let repaints = Arc::new(AtomicUsize::new(0));
        let callback = Arc::clone(&repaints);
        let wake = Wake::new(move || {
            callback.fetch_add(1, Ordering::Relaxed);
        });
        let mut view = QuotaView {
            wake: Some(Arc::clone(&wake)),
            ..QuotaView::default()
        };
        let ctx = egui::Context::default();
        view.open(&ctx);
        wake.changed();
        wake.changed();
        assert_eq!(repaints.load(Ordering::Relaxed), 1);
        view.close();
        wake.changed();
        assert_eq!(repaints.load(Ordering::Relaxed), 1);
        view.open(&ctx);
        wake.changed();
        assert_eq!(repaints.load(Ordering::Relaxed), 2);
    }

    /// Several launches can report; the chip shows the one heard from last, and nothing once the
    /// reporting is disabled.
    #[test]
    fn the_chip_reads_the_most_recently_updated_launch() {
        use super::super::chip::Source;
        use panel_control::quota::Window;
        let wake = Wake::new(|| {});
        let observed = |received_at, used| Snapshot::Observed {
            received_at,
            five_hour: WindowState::Reported(Window {
                used_percentage: Some(used),
                resets_at: Some(i64::MAX / 2),
            }),
            seven_day: WindowState::Expired,
        };
        let mut view = QuotaView::default();
        assert_eq!(view.chip_source(), Source::Off);
        let waiting = Arc::new(Broker::prepare(Arc::clone(&wake)).unwrap());
        view.install_fixture(Arc::clone(&waiting), Arc::clone(&wake));
        assert_eq!(view.chip_source(), Source::Waiting);
        for (received_at, used) in [(200, 20.0), (300, 30.0), (100, 10.0)] {
            let broker = Arc::new(Broker::prepare(Arc::clone(&wake)).unwrap());
            broker.observe_for_test(observed(received_at, used));
            view.install(1, broker);
        }
        let Source::Read(reading) = view.chip_source() else {
            panic!("a reading was expected");
        };
        assert_eq!(reading.observed_at, 300);
        assert_eq!(reading.five_hour.unwrap().used, Some(30.0));
        assert!(reading.weekly.unwrap().expired);
        view.disable();
        assert_eq!(view.chip_source(), Source::Off);
    }

    #[test]
    fn consent_is_window_local_and_disable_revokes_the_shared_launch_flag() {
        let mut first = QuotaView::default();
        let second = QuotaView::default();
        first.enabled.store(true, Ordering::Release);
        let launch_flag = Arc::clone(&first.enabled);
        assert!(!second.enabled.load(Ordering::Acquire));
        assert!(launch_flag.load(Ordering::Acquire));
        first.disable();
        assert!(!launch_flag.load(Ordering::Acquire));
    }
}
