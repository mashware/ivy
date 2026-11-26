use crate::launch::fresh::{Client, Selection};
use eframe::egui;

#[derive(Default, Debug)]
pub(super) struct Form {
    selection: Selection,
    pub(super) request: Option<Selection>,
}

pub(super) fn show(form: &mut Form, ui: &mut egui::Ui) {
    ui.separator();
    ui.strong("Open a new agent console");
    ui.label("Live hook updates require an agent opened here. Agents started in a shell can still use MCP.");
    ui.horizontal(|ui| {
        ui.selectable_value(&mut form.selection.client, Client::Claude, "Claude");
        ui.selectable_value(&mut form.selection.client, Client::Codex, "Codex local");
        ui.selectable_value(&mut form.selection.client, Client::OpenCode, "OpenCode");
    });
    for (label, path) in [
        "Claude executable",
        "Codex executable",
        "OpenCode executable",
    ]
    .into_iter()
    .zip(&mut form.selection.images)
    {
        ui.label(label);
        ui.text_edit_singleline(path);
    }
    ui.label("Working directory");
    ui.text_edit_singleline(&mut form.selection.directory);
    let index = match form.selection.client {
        Client::Claude => 0,
        Client::Codex => 1,
        Client::OpenCode => 2,
    };
    ui.label(format!("Executable: {}", form.selection.images[index]));
    ui.label(format!("Arguments: {:?}", form.selection.client.args()));
    ui.label(format!("Directory: {}", form.selection.directory));
    if form.selection.client == Client::Codex {
        ui.label("Starts a new local Codex session with --no-daemon. This launch does not share the conversation with mobile.");
    }
    if ui.button("Open agent console").clicked() {
        form.request = Some(form.selection.clone());
    }
}
