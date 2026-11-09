//! Recovery setup is explicit; only native verified observations count as a connected reporter.
use eframe::egui;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Status {
    enabled: bool,
    connected: usize,
    recoverable: usize,
    unsupported: usize,
}

pub(super) fn status(window: &crate::Window) -> Status {
    let mut result = Status {
        enabled: window.restoration.captures_sessions(),
        ..Default::default()
    };
    for pane in window.all_panes() {
        match pane.restoration.session_status {
            Some(crate::ai_sessions::Status::Registered) => {
                result.connected += 1;
                result.recoverable += usize::from(
                    !pane.restoration.failed
                        && !pane.restoration.resuming
                        && pane.restoration.resume.is_some()
                        && pane.restoration.directory.is_some(),
                );
            }
            Some(crate::ai_sessions::Status::Unsupported) => result.unsupported += 1,
            _ => {}
        }
    }
    result
}

fn resources() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let parent = executable.parent()?;
    [
        parent.join("session-recovery"),
        parent.parent()?.join("Resources/session-recovery"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../integrations/session-recovery"),
    ]
    .into_iter()
    .find(|root| root.join("README.md").is_file())
}

pub(super) fn show(status: Status, copied: &mut Option<&'static str>, ui: &mut egui::Ui) {
    ui.separator();
    ui.strong("Recover AI sessions");
    ui.label(if status.enabled { "Session recovery is enabled for this window." } else { "Choose Layout, directories and AI sessions in Settings to record this window. Additional windows do not record recovery." });
    ui.label(format!(
        "Verified reporters: {} · Sessions with a usable directory: {}",
        status.connected, status.recoverable
    ));
    if status.unsupported > 0 {
        ui.label(format!(
            "Native verification is unavailable in {} consoles.",
            status.unsupported
        ));
    }
    ui.label("A reported session verifies this console's client; finding a helper or configuring a plugin alone does not prove registration.");
    ui.weak("Codex 0.167.0 restores its last hook-reported conversation. Switching without sending a message can leave an earlier conversation recorded.");
    if !cfg!(target_os = "linux") {
        ui.label("Verified AI capture is currently available on Linux. This host can still recover layout and directories.");
    }
    if let Some(helper) = crate::resume::helper_path() {
        super::copyable(
            ui,
            &helper.to_string_lossy(),
            "Copy recovery helper path",
            copied,
        );
    } else {
        ui.label("Recovery helper not found beside Ivy. Build or install the complete Ivy package before configuring a client.");
    }
    if let Some(root) = resources() {
        super::copyable(
            ui,
            &root.to_string_lossy(),
            "Copy recovery adapters path",
            copied,
        );
        let readme = root.join("README.md");
        ui.label("The adapter README contains reviewed setup for Codex hooks, Claude Mods and the OpenCode TUI plugin. Preserve existing hooks and plugins; no automatic client edits are made.");
        super::copy_button(ui, "Copy recovery setup instructions", copied, || {
            let path = serde_json::to_string(&readme.to_string_lossy()).unwrap_or_default();
            format!(
                "Set up Ivy session recovery for this client using the local README at {path}. Read it first and preserve existing hooks, plugins, authentication and HUD. Ivy supplies IVY_SESSION_HELPER as an absolute helper location and PANEL_ID/PANEL_DIR for this console; never persist their values. Install only the matching official client adapter explicitly. Verify a report in Ivy's Connect agents guide before claiming success. Codex uses last hook-reported identity and can differ from the visible conversation after an idle switch. Do not send a prompt, use provider APIs, inject keys, bypass permissions or read transcripts/history for this setup."
            )
        });
    } else {
        ui.label("Recovery adapters not found. Install the complete Ivy package.");
    }
}
