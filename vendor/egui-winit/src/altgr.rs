use super::egui;

/// The system's Ctrl+Alt character owns the gesture before clipboard and key shortcuts.
pub(super) fn consume(
    pressed: bool,
    modifiers: egui::Modifiers,
    system_text: Option<&str>,
    events: &mut Vec<egui::Event>,
) -> bool {
    if !pressed || !modifiers.ctrl || !modifiers.alt || modifiers.mac_cmd {
        return false;
    }
    // KeyEvent::text removes Ctrl and can fabricate a plain letter on US layouts.
    // The modifier supplement instead carries the actual Windows character message.
    let Some(text) =
        system_text.filter(|text| !text.is_empty() && text.chars().all(|ch| !ch.is_control()))
    else {
        return false;
    };
    events.push(egui::Event::Text(text.to_owned()));
    true
}
