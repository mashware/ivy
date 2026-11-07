//! The window-level configuration transaction and its input ownership.
use super::{document::Document, store::Store};
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
mod script;
mod timing;
mod view;
use timing::Timing;

#[derive(Default)]
pub(crate) struct Editor {
    enabled: bool,
    entry_rect: Option<egui::Rect>,
    font_rect: Option<egui::Rect>,
    script: Option<script::Script>,
    measuring: bool,
    regime: &'static str,
    font_requests: usize,
    pub(super) draft: Option<Document>,
    pub(super) store: Option<Store>,
    pub(super) message: String,
    pub(super) fonts: Option<Vec<String>>,
    pub(super) section: usize,
    blocked_frame: Option<u64>,
    held_pointer: bool,
    timing: Timing,
}

impl Editor {
    pub fn resolved(entry: super::Entry, mode: Option<&str>) -> Self {
        let measuring = matches!(entry, super::Entry::Measuring);
        Self {
            enabled: !measuring || matches!(mode, Some("closed" | "open" | "fonts")),
            measuring,
            regime: match mode {
                Some("closed") => "closed",
                Some("open") => "open",
                Some("fonts") => "fonts",
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

    pub(super) fn save(&mut self) {
        let started = Instant::now();
        if let Some(draft) = &self.draft {
            if !draft.errors().is_empty() {
                return;
            }
            if let Some(store) = &mut self.store {
                match store.save(&draft.text()) {
                    Ok(()) => {
                        self.message =
                            "Saved. Restart agent-terminal to apply these changes.".into()
                    }
                    Err(error) => self.message = error,
                }
            }
        }
        self.timing.record_work(2, started.elapsed());
    }

    /// Runs before console input. A closing frame and its held release remain owned.
    pub fn show(&mut self, ui: &mut egui::Ui, discoverer: &mut crate::fonts::Discoverer) -> bool {
        if !self.enabled {
            return false;
        }
        let started = Instant::now();
        let ctx = ui.ctx().clone();
        let frame = ctx.cumulative_frame_nr();
        let was_open = self.is_open();
        let row = egui::Rect::from_min_size(
            ui.max_rect().min,
            egui::vec2(ui.max_rect().width(), 28.0_f32.min(ui.max_rect().height())),
        );
        let over_row = ctx.input(|i| i.pointer.interact_pos().is_some_and(|p| row.contains(p)));
        let mut opening = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(row), |ui| {
            ui.horizontal(|ui| {
                let response = ui.add(egui::Button::new("Settings").sense(egui::Sense::CLICK));
                self.entry_rect = Some(response.rect);
                opening = response.clicked();
            });
        });
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

    pub fn console_area(&self, mut area: egui::Rect) -> egui::Rect {
        if self.enabled {
            area.min.y += 28.0_f32.min(area.height());
        }
        area
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
        self.timing.publish(to)
    }
}

/// All settings gestures are consumed before the terminal's event dispatcher sees them.
pub(crate) fn before_keys(
    window: &mut crate::Window,
    ui: &mut egui::Ui,
) -> Vec<crate::WindowRequest> {
    if !window.settings_editor.enabled {
        return window.send_keys_to_the_focused(ui.ctx());
    }
    let started = Instant::now();
    let owned = window.settings_editor.show(ui, &mut window.discoverer);
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
            |ui| assert!(editor.show(ui, &mut fonts)),
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
        editor.save();
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
            |ui| assert!(editor.show(ui, &mut fonts)),
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
        editor.save();
        assert!(editor.message.starts_with("Saved."));
        fs::write(&path, "size=22").unwrap();
        editor.save();
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
        editor.save();
        assert!(editor.message.contains("Restart agent-terminal"));
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
        editor.save();
        assert_eq!(fs::read_to_string(&path).unwrap(), "size=18");
        assert!(!editor.message.contains("Saved."));
        assert!(editor.is_open());
        fs::write(&path, "version=999").unwrap();
        editor.open(Some(path.clone()));
        assert!(editor.store.is_none());
        editor.draft.as_mut().unwrap().defaults();
        editor.save();
        assert_eq!(fs::read_to_string(&path).unwrap(), "version=999");
        assert!(!editor.message.contains("Saved."));
        fs::remove_file(path).unwrap();
    }
}
