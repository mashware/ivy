use super::quota_broker::{Broker, Snapshot, Wake, WindowState};
use super::quota_setup::Preview;
use eframe::egui;
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
};

#[derive(Default)]
pub(super) struct View {
    enabled: bool,
    loaded: bool,
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
    Consent(bool),
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
        if matches!(self.broker.snapshot(), Snapshot::Disconnected) || !read_consent()? {
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

impl View {
    pub(super) fn open(&mut self, ctx: &egui::Context) {
        if self.directory.is_empty() {
            self.directory = std::env::current_dir()
                .ok()
                .and_then(|directory| directory.into_os_string().into_string().ok())
                .unwrap_or_default();
        }
        if !self.loaded && self.pending.is_none() {
            self.request(ctx, || read_consent().map(Update::Consent));
        }
        if let Some(wake) = &self.wake {
            wake.set_visible(true);
        }
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
        let ctx = ctx.clone();
        match std::thread::Builder::new()
            .name("claude-quota-setup".into())
            .spawn(move || {
                let _ = send.send(operation());
                ctx.request_repaint();
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
            Ok(Update::Consent(enabled)) => {
                self.loaded = true;
                self.enabled = enabled;
            }
            Ok(Update::Preview(preview)) => self.preview = Some(preview),
            Ok(Update::Launch(launch)) => return Some(launch),
            Err(error) => {
                self.loaded = true;
                self.error = Some(error.to_string());
            }
        }
        None
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui) -> Option<Launch> {
        let ready = self.poll();
        ui.strong("Claude Code · Subscription allowances");
        if let Some(error) = &self.error {
            ui.label(error);
        }
        if self.pending.is_some() {
            ui.label("Preparing Claude allowance settings…");
        }
        if !self.enabled {
            if ui
                .add_enabled(
                    self.pending.is_none(),
                    egui::Button::new("Enable allowance reporting"),
                )
                .clicked()
            {
                self.request(ui.ctx(), || {
                    write_consent(true).map(|()| Update::Consent(true))
                });
            }
            return None;
        }
        if ui
            .add_enabled(
                self.pending.is_none(),
                egui::Button::new("Disable allowance reporting"),
            )
            .clicked()
        {
            self.enabled = false;
            self.preview = None;
            for (_, broker) in self.brokers.drain(..) {
                broker.stop();
            }
            self.disconnected = false;
            self.request(ui.ctx(), || {
                write_consent(false).map(|()| Update::Consent(false))
            });
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
                let wake = self.wake.get_or_insert_with(|| {
                    let ctx = ui.ctx().clone();
                    Wake::new(move || ctx.request_repaint())
                });
                wake.set_visible(true);
                let wake = Arc::clone(wake);
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

fn consent_file() -> io::Result<PathBuf> {
    crate::session::directory()
        .map(|directory| directory.join("claude-allowance-reporting"))
        .ok_or_else(|| io::Error::other("Ivy's configuration directory is unavailable."))
}

fn read_consent() -> io::Result<bool> {
    match fs::symlink_metadata(consent_file()?) {
        Ok(metadata) if metadata.is_file() && metadata.len() == 0 => Ok(true),
        Ok(_) => Err(io::Error::other(
            "Ivy's Claude allowance consent file is invalid.",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn write_consent(enabled: bool) -> io::Result<()> {
    let file = consent_file()?;
    if enabled {
        if read_consent()? {
            return Ok(());
        }
        let directory = file
            .parent()
            .ok_or_else(|| io::Error::other("Ivy's configuration directory is unavailable."))?;
        fs::create_dir_all(directory)?;
        let _file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(file)?;
        return Ok(());
    }
    match fs::remove_file(file) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
