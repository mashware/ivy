//! The window-level configuration transaction and its input ownership.
use super::{document::Document, store::Store};
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
mod probe;
pub(crate) mod runtime;
mod script;
mod timing;
pub(crate) mod view;
use timing::Timing;

#[derive(Default)]
pub(crate) struct Editor {
    enabled: bool,
    entry_rect: Option<egui::Rect>,
    font_rect: Option<egui::Rect>,
    script: Option<script::Script>,
    probe: Option<probe::Probe>,
    measuring: bool,
    regime: &'static str,
    font_requests: usize,
    pub(super) preview: view::typography::PreviewState,
    pub(super) draft: Option<Document>,
    pub(super) store: Option<Store>,
    pub(super) message: String,
    pub(super) fonts: Option<Vec<String>>,
    pub(super) section: usize,
    pub(super) picker_open: bool,
    pub(super) picker_query: String,
    pub(super) picker_filter: usize,
    blocked_frame: Option<u64>,
    held_pointer: bool,
    timing: Timing,
    application: runtime::Application,
    pub(super) saved_startup_restore: Option<super::StartupRestore>,
}

impl Editor {
    pub(crate) fn open_from_chrome(&mut self) {
        if self.enabled && !self.is_open() {
            self.open(super::path());
        }
    }
    /// Open the same editor on its keyboard bindings page.
    pub(crate) fn open_shortcuts_from_chrome(&mut self) {
        self.open_from_chrome();
        self.section = 2;
    }
    pub fn resolved(entry: super::Entry, mode: Option<&str>) -> Self {
        let measuring = matches!(entry, super::Entry::Measuring);
        Self {
            enabled: !measuring || matches!(mode, Some("closed" | "open" | "fonts" | "apply")),
            measuring,
            probe: (measuring && mode == Some("apply")).then(probe::Probe::default),
            regime: match mode {
                Some("closed") => "closed",
                Some("open") => "open",
                Some("fonts") => "fonts",
                Some("apply") => "apply",
                _ => "historical",
            },
            script: if measuring {
                script::Script::for_mode(mode)
            } else {
                None
            },
            ..Self::default()
        }
    }

    fn open(&mut self, path: Option<PathBuf>) {
        let started = Instant::now();
        self.message.clear();
        self.preview = Default::default();
        self.picker_open = false;
        self.picker_query.clear();
        self.picker_filter = 0;
        self.fonts = None;
        self.store = None;
        match path
            .ok_or_else(|| "The configuration directory is unavailable.".to_owned())
            .and_then(Store::open)
        {
            Ok((store, text)) => {
                if let Ok((_, warnings)) = super::interpret(&text) {
                    self.message = warnings
                        .kept()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n");
                }
                self.store = Some(store);
                self.draft = Some(Document::new(text));
            }
            Err(error) => {
                self.message = error;
                self.draft = Some(Document::new(String::new()));
            }
        }
        self.timing.record_work(2, started.elapsed());
    }

    pub(super) fn save(&mut self, fonts: &mut crate::fonts::Discoverer) -> bool {
        runtime::save(self, fonts)
    }

    pub(super) fn save_in_frame(
        &mut self,
        fonts: &mut crate::fonts::Discoverer,
        ctx: &egui::Context,
    ) -> bool {
        let frame = ctx.cumulative_frame_nr();
        if let Some((saved_frame, outcome)) = self.application.saved_frame
            && saved_frame == frame
        {
            return outcome;
        }
        let outcome = self.save(fonts);
        self.application.saved_frame = Some((frame, outcome));
        outcome
    }

    pub(crate) fn take_saved_startup_restore(&mut self) -> Option<super::StartupRestore> {
        self.saved_startup_restore.take()
    }

    pub(crate) fn remember_family(&mut self, settings: &super::Settings) {
        self.application.family = settings.font.clone();
    }

    /// Runs before console input. A closing frame and its held release remain owned.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        discoverer: &mut crate::fonts::Discoverer,
        gear: egui::Rect,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        let started = Instant::now();
        let ctx = ui.ctx().clone();
        let frame = ctx.cumulative_frame_nr();
        let was_open = self.is_open();
        let over_row = ctx.input(|i| i.pointer.interact_pos().is_some_and(|p| gear.contains(p)));
        let mut opening = false;
        if gear.is_positive() {
            let response = ui
                .interact(gear, ui.id().with("settings-gear"), egui::Sense::CLICK)
                .on_hover_text("Settings");
            self.entry_rect = Some(gear);
            opening = response.clicked();
            let painter = ui.painter_at(gear);
            painter.rect_filled(gear, 0.0, egui::Color32::from_rgb(35, 41, 52));
            let ink = if opening || response.hovered() {
                egui::Color32::from_rgb(173, 195, 255)
            } else {
                egui::Color32::from_rgb(153, 163, 182)
            };
            let radius = (gear.width().min(gear.height()) * 0.24).min(8.0);
            let stroke = egui::Stroke::new(1.6, ink);
            painter.circle_stroke(gear.center(), radius * 0.7, stroke);
            painter.circle_stroke(gear.center(), radius * 0.26, stroke);
            for tooth in 0..8 {
                let direction = egui::vec2(
                    (tooth as f32 * std::f32::consts::TAU / 8.0).cos(),
                    (tooth as f32 * std::f32::consts::TAU / 8.0).sin(),
                );
                painter.line_segment(
                    [
                        gear.center() + direction * radius * 0.7,
                        gear.center() + direction * radius,
                    ],
                    stroke,
                );
            }
        }
        self.timing.record_work(0, started.elapsed());
        if opening {
            if self.measuring {
                self.draft = Some(Document::new(String::new()));
                self.message = "Measurement: configuration I/O is disabled.".into();
            } else {
                self.open(super::path());
            }
        }
        let row_gesture = over_row
            && ctx.input(|i| {
                i.pointer.any_pressed()
                    || i.pointer.any_released()
                    || i.smooth_scroll_delta != egui::Vec2::ZERO
            });
        let owned = was_open
            || opening
            || row_gesture
            || self.held_pointer
            || self.blocked_frame == Some(frame);
        if self.draft.is_some() {
            let started = Instant::now();
            let child_before = self.timing.work;
            view::show(self, &ctx, discoverer);
            let children = self.timing.work.saturating_sub(child_before);
            self.timing
                .record_work(1, started.elapsed().saturating_sub(children));
        }
        if owned {
            self.blocked_frame = Some(frame);
            self.held_pointer = ctx.input(|i| i.focused && i.pointer.any_down());
        }
        owned
    }

    #[cfg(test)]
    pub(crate) fn entry_rect(&self) -> egui::Rect {
        self.entry_rect.expect("the entry was painted")
    }

    pub fn is_open(&self) -> bool {
        self.draft.is_some()
    }

    pub(crate) fn chrome_visible(&self) -> bool {
        self.enabled
    }

    pub(crate) fn record_geometry(&mut self, cost: Duration) {
        self.timing.record_work(7, cost);
    }

    pub fn close_frame(&mut self) {
        self.timing.close_frame();
    }
    pub fn take_cost(&mut self) -> Duration {
        std::mem::take(&mut self.timing.work)
    }
    pub fn publish(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        writeln!(
            to,
            "settings UI: visible={}, measurement={}",
            self.enabled, self.measuring
        )?;
        writeln!(
            to,
            "settings chrome: {}",
            if self.enabled {
                "compact-tab-strip"
            } else {
                "historical"
            }
        )?;
        if let Some(probe) = &self.probe {
            probe.publish(to)?;
        }
        writeln!(
            to,
            "settings mode: {}",
            if !self.enabled {
                "historical"
            } else if !self.measuring {
                "interactive"
            } else {
                self.regime
            }
        )?;
        writeln!(
            to,
            "settings installed families: {}",
            self.fonts.as_ref().map_or(0, Vec::len)
        )?;
        writeln!(to, "settings font requests: {}", self.font_requests)?;
        runtime::publish(&self.application, to)?;
        self.timing.publish(to)
    }
}

/// All settings gestures are consumed before the terminal's event dispatcher sees them.
pub(crate) fn before_keys(
    window: &mut crate::Window,
    ui: &mut egui::Ui,
    metrics: &crate::painter::CellMetrics,
) -> Vec<crate::WindowRequest> {
    if !window.settings_editor.enabled {
        return window.send_keys_to_the_focused(ui.ctx());
    }
    let started = Instant::now();
    let gear = crate::tab_bar::geometry(window, ui.max_rect(), metrics)
        .gear
        .expect("enabled settings has a gear");
    let owned = window
        .settings_editor
        .show(ui, &mut window.discoverer, gear);
    let measured = window.settings_editor.timing.work;
    if owned {
        let ctx = ui.ctx();
        window.modifiers = ctx.input(|i| {
            if i.focused {
                i.modifiers
            } else {
                egui::Modifiers::NONE
            }
        });
        window
            .header_input
            .block_for_settings(ctx.cumulative_frame_nr());
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                matches!(
                    e,
                    egui::Event::ModifiersChanged(_)
                        | egui::Event::WindowFocused(_)
                        | egui::Event::PointerButton { pressed: false, .. }
                )
            })
        });
    }
    window
        .settings_editor
        .timing
        .record_work(4, started.elapsed().saturating_sub(measured));
    window.timed_separately += window.settings_editor.take_cost();
    if owned {
        Vec::new()
    } else {
        window.send_keys_to_the_focused(ui.ctx())
    }
}

pub(crate) fn inject(window: &mut crate::Window, raw: &mut egui::RawInput) {
    let Some(script) = &mut window.settings_editor.script else {
        return;
    };
    let started = Instant::now();
    raw.events.extend(script.next(
        window.settings_editor.entry_rect,
        window.settings_editor.font_rect,
    ));
    window.spent_injecting += started.elapsed();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    #[test]
    fn startup_save_and_cancel_keep_the_saved_choice() {
        let path = std::env::temp_dir().join(format!(
            "startup-editor-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "# keep\nstartup.restore = layout\n").unwrap();
        let mut editor = Editor::resolved(super::super::Entry::Normal, None);
        let mut fonts = crate::fonts::Discoverer::default();
        editor.open(Some(path.clone()));
        editor
            .draft
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|f| f.key == "startup.restore")
            .unwrap()
            .value = "directories".into();
        assert!(editor.save(&mut fonts));
        assert_eq!(
            editor.take_saved_startup_restore(),
            Some(super::super::StartupRestore::Directories)
        );
        assert_eq!(
            super::super::interpret(&fs::read_to_string(&path).unwrap())
                .unwrap()
                .0
                .startup_restore,
            super::super::StartupRestore::Directories
        );
        editor.draft = None;
        editor.store = None;
        editor.open(Some(path.clone()));
        editor
            .draft
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|f| f.key == "startup.restore")
            .unwrap()
            .value = "none".into();
        let ctx = egui::Context::default();
        ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                editor.show(ui, &mut fonts, ui.max_rect());
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(editor.take_saved_startup_restore(), None);
        assert_eq!(
            super::super::interpret(&fs::read_to_string(&path).unwrap())
                .unwrap()
                .0
                .startup_restore,
            super::super::StartupRestore::Directories
        );
        fs::remove_file(path).unwrap();
    }

    struct RestorePolicyFixture {
        directory: PathBuf,
        snapshot: PathBuf,
        config: PathBuf,
    }

    impl RestorePolicyFixture {
        fn new(mode: super::super::StartupRestore) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "settings-restore-policy-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).unwrap();
            let fixture = Self {
                snapshot: directory.join("layout.json"),
                config: directory.join("config"),
                directory,
            };
            let value = match mode {
                super::super::StartupRestore::None => "none",
                super::super::StartupRestore::Layout => "layout",
                super::super::StartupRestore::Directories => "directories",
            };
            fs::write(&fixture.config, format!("startup.restore = {value}\n")).unwrap();
            fixture
        }

        fn seed(&self, version: u64) -> Vec<u8> {
            let document = crate::session::StoredDocument {
                v: version,
                tabs: vec![crate::session::StoredTab {
                    label: None,
                    root: crate::session::StoredNode::Leaf(crate::session::StoredLeaf {
                        panel_width: 37,
                        folded: true,
                        label: Some("Saved console".into()),
                        cwd: Some(self.directory.clone()),
                    }),
                }],
            };
            let bytes = crate::session::serialize(&document).unwrap().into_bytes();
            fs::write(&self.snapshot, &bytes).unwrap();
            bytes
        }

        fn open(&self, mode: super::super::StartupRestore) -> crate::Window {
            let mut persistence = crate::session::Persistence::at(Some(self.snapshot.clone()));
            let (opening, state) = crate::restoration::prepare(
                &mut persistence,
                Some(&self.snapshot),
                mode,
                false,
                None,
                None,
            );
            let mut window =
                crate::Window::opening(&Default::default(), opening, "/bin/sh", None, persistence)
                    .unwrap();
            window.restoration = state;
            window
        }

        fn save_mode(&self, window: &mut crate::Window, mode: &str) {
            window.settings_editor.open(Some(self.config.clone()));
            window
                .settings_editor
                .draft
                .as_mut()
                .unwrap()
                .fields
                .iter_mut()
                .find(|field| field.key == "startup.restore")
                .unwrap()
                .value = mode.into();
            assert!(window.settings_editor.save(&mut window.discoverer));
            assert!(window.settings_editor.message.starts_with("Saved."));
        }

        fn queue_expired_change(window: &mut crate::Window) {
            window.active_mut().panes_mut()[0].label = Some("Changed console".into());
            window.check_whether_the_layout_moved();
            assert!(window.persistence.save_due.is_some());
            window.persistence.save_due = Some(Instant::now() - Duration::from_secs(1));
        }
    }

    impl Drop for RestorePolicyFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn saved_none_cancels_an_expired_layout_write_before_the_next_frame() {
        let fixture = RestorePolicyFixture::new(super::super::StartupRestore::Directories);
        let original = fixture.seed(crate::session::VERSION);
        let mut window = fixture.open(super::super::StartupRestore::Directories);
        RestorePolicyFixture::queue_expired_change(&mut window);
        fixture.save_mode(&mut window, "none");

        window.save_if_due();

        assert_eq!(fs::read(&fixture.snapshot).unwrap(), original);
        assert!(window.persistence.save_due.is_none());
        assert!(!window.restoration.can_save());
    }

    #[test]
    fn saved_none_preserves_the_snapshot_when_the_window_closes_immediately() {
        let fixture = RestorePolicyFixture::new(super::super::StartupRestore::Directories);
        let original = fixture.seed(crate::session::VERSION);
        let mut window = fixture.open(super::super::StartupRestore::Directories);
        RestorePolicyFixture::queue_expired_change(&mut window);
        fixture.save_mode(&mut window, "none");

        crate::restoration::capture_on_exit(&mut window);
        window.check_whether_the_layout_moved();
        window.save_now();

        assert_eq!(fs::read(&fixture.snapshot).unwrap(), original);
        assert!(!window.restoration.can_save());
        assert!(window.persistence.save_due.is_none());
    }

    #[test]
    fn saved_layout_removes_cached_directories_from_the_next_snapshot() {
        let fixture = RestorePolicyFixture::new(super::super::StartupRestore::Directories);
        fixture.seed(crate::session::VERSION);
        let mut window = fixture.open(super::super::StartupRestore::Directories);
        assert!(window.active().panes()[0].restoration.directory.is_some());
        RestorePolicyFixture::queue_expired_change(&mut window);
        fixture.save_mode(&mut window, "layout");

        window.save_if_due();
        window.check_whether_the_layout_moved();
        window.save_now();

        let saved = crate::session::read(&fixture.snapshot).unwrap();
        let crate::session::StoredNode::Leaf(leaf) = &saved.tabs()[0].root else {
            panic!("expected the saved console");
        };
        assert_eq!(leaf.label.as_deref(), Some("Changed console"));
        assert!(leaf.cwd.is_none());
        assert!(window.active().panes()[0].restoration.directory.is_none());
    }

    #[test]
    fn enabling_layout_after_disabled_startup_respects_a_newer_snapshot_version() {
        let fixture = RestorePolicyFixture::new(super::super::StartupRestore::None);
        let original = fixture.seed(99);
        let mut window = fixture.open(super::super::StartupRestore::None);
        assert!(window.persistence.load_time().is_none());
        fixture.save_mode(&mut window, "layout");

        window.save_if_due();
        window.active_mut().panes_mut()[0].label = Some("Changed fresh console".into());
        window.check_whether_the_layout_moved();
        window.save_now();

        assert!(window.restoration.can_save());
        assert!(window.persistence.load_time().is_some());
        assert_eq!(fs::read(&fixture.snapshot).unwrap(), original);
    }

    #[test]
    fn opening_and_cancelling_never_write_or_enumerate_fonts() {
        let path = std::env::temp_dir().join(format!(
            "settings-editor-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "# keep\nsize=18").unwrap();
        let mut editor = Editor::resolved(super::super::Entry::Normal, None);
        editor.open(Some(path.clone()));
        editor.draft.as_mut().unwrap().defaults();
        let ctx = egui::Context::default();
        let mut fonts = crate::fonts::Discoverer::default();
        let output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| assert!(editor.show(ui, &mut fonts, ui.max_rect())),
        );
        output.drop_without_applying_deltas();
        assert!(!editor.is_open());
        assert!(editor.fonts.is_none());
        assert_eq!(fs::read_to_string(&path).unwrap(), "# keep\nsize=18");
        editor.close_frame();
        let mut report = Vec::new();
        editor.publish(&mut report).unwrap();
        assert!(
            String::from_utf8(report)
                .unwrap()
                .contains("settings font enumeration: frames=0")
        );
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn save_reports_conflicts_and_never_claims_success() {
        let path = std::env::temp_dir().join(format!(
            "settings-editor-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "size=18").unwrap();
        let mut editor = Editor::resolved(super::super::Entry::Normal, None);
        editor.open(Some(path.clone()));
        editor
            .draft
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|f| f.key == "size")
            .unwrap()
            .value = "20".into();
        fs::write(&path, "size=19").unwrap();
        editor.save(&mut crate::fonts::Discoverer::default());
        assert!(editor.message.contains("changed outside"));
        assert!(!editor.message.contains("Saved."));
        assert!(editor.is_open());
        assert_eq!(fs::read_to_string(&path).unwrap(), "size=19");
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn closing_and_reopening_discards_edits_and_reads_a_fresh_external_snapshot() {
        let path = std::env::temp_dir().join(format!(
            "settings-reopen-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "size=18").unwrap();
        let mut editor = Editor::resolved(super::super::Entry::Normal, None);
        editor.open(Some(path.clone()));
        editor
            .draft
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|f| f.key == "size")
            .unwrap()
            .value = "20".into();
        editor.fonts = Some(vec!["old snapshot".into()]);
        let ctx = egui::Context::default();
        let mut fonts = crate::fonts::Discoverer::default();
        ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| assert!(editor.show(ui, &mut fonts, ui.max_rect())),
        )
        .drop_without_applying_deltas();
        assert!(editor.draft.is_none() && editor.store.is_none() && editor.fonts.is_none());
        assert_eq!(fs::read_to_string(&path).unwrap(), "size=18");
        fs::write(&path, "size=21").unwrap();
        editor.open(Some(path.clone()));
        assert_eq!(
            editor
                .draft
                .as_ref()
                .unwrap()
                .fields
                .iter()
                .find(|f| f.key == "size")
                .unwrap()
                .value,
            "21"
        );
        assert!(editor.message.is_empty() && editor.fonts.is_none() && editor.store.is_some());
        editor.save(&mut crate::fonts::Discoverer::default());
        assert!(editor.message.starts_with("Saved."));
        fs::write(&path, "size=22").unwrap();
        editor.save(&mut crate::fonts::Discoverer::default());
        assert!(editor.message.contains("changed outside"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn defaults_are_staged_then_saved_for_a_fresh_startup() {
        let path = std::env::temp_dir().join(format!(
            "settings-defaults-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let source = "# retain\nversion=1\nfuture=yes\nsize=18\ntheme=tokyonight\nforeground=#abcdef\nfont=Custom\ncursor.blink=on\nkey.copy=none\nkey.new-tab=none\n";
        fs::write(&path, source).unwrap();
        let mut editor = Editor::resolved(super::super::Entry::Normal, None);
        editor.open(Some(path.clone()));
        editor.draft.as_mut().unwrap().defaults();
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        editor.save(&mut crate::fonts::Discoverer::default());
        assert!(editor.message.contains("Applying settings"));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# retain\nversion=1\nfuture=yes\n"
        );
        assert_eq!(
            super::super::read(&path).unwrap().0,
            super::super::Settings::default()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_draft_and_unsupported_file_cannot_be_saved() {
        let path = std::env::temp_dir().join(format!(
            "settings-invalid-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "size=18").unwrap();
        let mut editor = Editor::resolved(super::super::Entry::Normal, None);
        editor.open(Some(path.clone()));
        editor
            .draft
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|f| f.key == "size")
            .unwrap()
            .value = "97".into();
        editor.save(&mut crate::fonts::Discoverer::default());
        assert_eq!(fs::read_to_string(&path).unwrap(), "size=18");
        assert!(!editor.message.contains("Saved."));
        assert!(editor.is_open());
        fs::write(&path, "version=999").unwrap();
        editor.open(Some(path.clone()));
        assert!(editor.store.is_none());
        editor.draft.as_mut().unwrap().defaults();
        editor.save(&mut crate::fonts::Discoverer::default());
        assert_eq!(fs::read_to_string(&path).unwrap(), "version=999");
        assert!(!editor.message.contains("Saved."));
        fs::remove_file(path).unwrap();
    }
}
