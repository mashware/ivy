use eframe::egui;

// Exercise the vendored dispatch itself, including its event output, on every platform.
#[path = "../../../vendor/egui-winit/src/altgr.rs"]
mod altgr;

fn chord() -> egui::Modifiers {
    egui::Modifiers {
        ctrl: true,
        command: true,
        alt: true,
        ..Default::default()
    }
}

#[test]
fn windows_altgr_system_character_reaches_the_terminal_once_without_a_shortcut() {
    for text in ["@", "{", "€", "é", "a", "AA"] {
        let mut events = Vec::new();
        assert!(altgr::consume(true, chord(), Some(text), &mut events));
        assert_eq!(events, vec![egui::Event::Text(text.to_owned())]);
        let bytes: Vec<_> = events
            .iter()
            .filter_map(|event| app::keyboard::bytes(event, app::keyboard::CursorKeys::Normal))
            .flat_map(|bytes| bytes.into_owned())
            .collect();
        assert_eq!(bytes, text.as_bytes());
    }
}

#[test]
fn windows_altgr_fallback_text_cannot_turn_a_us_shortcut_into_typing() {
    // US Ctrl+Alt+A may have KeyEvent::text="a", while its actual system text is absent
    // or a control character. Only that actual system text reaches this dispatch.
    for system_text in [
        None,
        Some(""),
        Some("\u{1}"),
        Some("\u{1b}"),
        Some("a\n"),
        Some("\u{85}"),
    ] {
        let mut events = Vec::new();
        assert!(!altgr::consume(true, chord(), system_text, &mut events));
        assert!(events.is_empty());
    }
}

#[test]
fn windows_altgr_does_not_consume_release_or_other_modifier_combinations() {
    let mut combinations = vec![egui::Modifiers::default()];
    for change in [0, 1, 2] {
        let mut modifiers = chord();
        match change {
            0 => modifiers.ctrl = false,
            1 => modifiers.alt = false,
            _ => modifiers.mac_cmd = true,
        }
        combinations.push(modifiers);
    }
    for modifiers in combinations {
        let mut events = Vec::new();
        assert!(!altgr::consume(true, modifiers, Some("@"), &mut events));
        assert!(events.is_empty());
    }
    let mut events = Vec::new();
    assert!(!altgr::consume(false, chord(), Some("@"), &mut events));
    assert!(events.is_empty());
}

#[test]
fn windows_altgr_keeps_shift_caps_and_composed_system_text() {
    let mut modifiers = chord();
    modifiers.shift = true;
    let mut events = Vec::new();
    assert!(altgr::consume(true, modifiers, Some("É"), &mut events));
    assert!(altgr::consume(true, modifiers, Some("É"), &mut events));
    assert_eq!(
        events,
        vec![egui::Event::Text("É".into()), egui::Event::Text("É".into())]
    );
}

#[test]
fn windows_altgr_character_is_inserted_once_in_a_focused_editor_with_modifiers_held() {
    let context = egui::Context::default();
    let editor = egui::Id::new("windows-altgr-editor");
    let mut text = String::new();
    let output = context.run_ui(egui::RawInput::default(), |ui| {
        ui.add(egui::TextEdit::singleline(&mut text).id(editor))
            .request_focus();
    });
    output.drop_without_applying_deltas();
    let mut events = vec![egui::Event::ModifiersChanged(chord())];
    altgr::consume(true, chord(), Some("@"), &mut events);
    let input = egui::RawInput {
        events,
        ..Default::default()
    };
    let output = context.run_ui(input, |ui| {
        assert!(ui.input(|input| input.modifiers.ctrl && input.modifiers.alt));
        assert!(
            ui.add(egui::TextEdit::singleline(&mut text).id(editor))
                .has_focus()
        );
    });
    output.drop_without_applying_deltas();
    assert_eq!(text, "@");
}
