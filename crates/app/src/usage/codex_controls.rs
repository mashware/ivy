//! Explicit folder selection and consent for importing local Codex history.

use std::path::PathBuf;

use chrono::Local;
use eframe::egui;
use usage_core::{Service, Snapshot};

#[derive(Default)]
pub(super) struct Controls {
    roots: Vec<PathBuf>,
    input: String,
}

impl Controls {
    pub(super) fn paint(
        &mut self,
        ui: &mut egui::Ui,
        service: &Service,
        snapshot: Option<&Snapshot>,
        can_enable: bool,
    ) -> bool {
        ui.separator();
        ui.strong("Codex · Local, unassigned history");
        let codex_enabled = snapshot.is_some_and(|s| s.codex_enabled);
        if let Some(snapshot) = snapshot {
            ui.label(if codex_enabled {
                "Collection enabled."
            } else {
                "Collection is off."
            });
            if let Some(success) = snapshot
                .codex_last_success_at_ms
                .and_then(chrono::DateTime::from_timestamp_millis)
            {
                ui.label(format!(
                    "Last successful collection: {}",
                    success.with_timezone(&Local).format("%Y-%m-%d %H:%M %Z")
                ));
            }
            if !snapshot.codex_roots.is_empty() {
                ui.collapsing(
                    format!("Codex folders ({})", snapshot.codex_roots.len()),
                    |ui| {
                        for root in &snapshot.codex_roots {
                            ui.small(root.display().to_string());
                        }
                    },
                );
            }
        }
        if can_enable && !codex_enabled {
            ui.label("Choose Codex sessions or archived_sessions folders.");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.input)
                        .hint_text("Absolute folder path")
                        .desired_width(300.0),
                );
                let path = PathBuf::from(self.input.trim());
                if ui
                    .add_enabled(
                        path.is_absolute() && !self.roots.contains(&path) && self.roots.len() < 64,
                        egui::Button::new("Add folder"),
                    )
                    .clicked()
                {
                    self.roots.push(path);
                    self.input.clear();
                }
            });
            if let Some(home) = std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .or_else(|| std::env::home_dir().map(|path| path.join(".codex")))
            {
                ui.horizontal_wrapped(|ui| {
                    for (folder, label) in [
                        ("sessions", "Add Codex sessions"),
                        ("archived_sessions", "Add archived sessions"),
                    ] {
                        let path = home.join(folder);
                        if ui
                            .add_enabled(
                                !self.roots.contains(&path) && self.roots.len() < 64,
                                egui::Button::new(label),
                            )
                            .clicked()
                        {
                            self.roots.push(path);
                        }
                    }
                });
            }
            let mut remove = None;
            for (index, root) in self.roots.iter().enumerate() {
                ui.push_id(("codex-folder", index), |ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("Remove").clicked() {
                            remove = Some(index);
                        }
                        ui.label(root.display().to_string());
                    });
                });
            }
            if let Some(index) = remove {
                self.roots.remove(index);
            }
            ui.small("Enabling Codex upgrades the local usage database. Older Ivy versions cannot open the upgraded history.");
            if ui
                .add_enabled(
                    !self.roots.is_empty(),
                    egui::Button::new("Enable Codex collection"),
                )
                .clicked()
                && service.enable_codex(self.roots.clone())
            {
                self.roots.clear();
            }
        }
        codex_enabled
    }
}
