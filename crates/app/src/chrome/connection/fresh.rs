use crate::launch::fresh::{Client, Selection};
use eframe::egui;

#[derive(Default, Debug)]
pub(super) struct Form {
    selection: Selection,
    pub(super) request: Option<Selection>,
}

pub(super) fn show(form: &mut Form, ui: &mut egui::Ui) {
    ui.separator();
    let section = egui::CollapsingHeader::new("Open a new agent console")
        .default_open(false)
        .show(ui, |ui| fields(form, ui));
    reveal_on_focus(section.header_response);
}

fn reveal_on_focus(response: egui::Response) -> egui::Response {
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    response
}

fn fields(form: &mut Form, ui: &mut egui::Ui) {
    ui.label("Live hook updates require an agent opened here. Agents started in a shell can still use MCP.");
    ui.horizontal(|ui| {
        reveal_on_focus(ui.selectable_value(&mut form.selection.client, Client::Claude, "Claude"));
        reveal_on_focus(ui.selectable_value(
            &mut form.selection.client,
            Client::Codex,
            "Codex local",
        ));
        reveal_on_focus(ui.selectable_value(
            &mut form.selection.client,
            Client::OpenCode,
            "OpenCode",
        ));
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
        reveal_on_focus(ui.text_edit_singleline(path));
    }
    ui.label("Working directory");
    reveal_on_focus(ui.text_edit_singleline(&mut form.selection.directory));
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
    if reveal_on_focus(ui.button("Open agent console")).clicked() {
        form.request = Some(form.selection.clone());
    }
}
