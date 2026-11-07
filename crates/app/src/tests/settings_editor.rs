//! Exercise Settings through real application frames, not hand-built field assertions.
#[cfg(test)]
mod tests {
    use super::super::*;
    fn click(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    #[test]
    fn replacing_settings_restores_defaults_and_their_effects() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of(1280.0);
        let source = "size=22\ncursor.blink=off\nkey.copy=none\nbackground=#010203\n";
        let mut configured = settings::interpret(source).unwrap().0;
        configured.shortcuts = keyboard::ALL.iter().map(|a| (*a, None)).collect();
        configured.apply_to(&mut window);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let configured_columns = window.active().panes()[0].measured.cols;
        assert!(!window.copy_enabled);
        assert_eq!(window.blink.ceiling, Some(false));
        assert_eq!(window.theme.background(), egui::Color32::from_rgb(1, 2, 3));
        assert!(
            keyboard::ALL
                .iter()
                .all(|a| window.keymap.chord_for(*a).is_none())
        );

        settings::Settings::default().apply_to(&mut window);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        assert_eq!(window.font_size, DEFAULT_FONT_SIZE);
        assert!(window.active().panes()[0].measured.cols > configured_columns);
        assert!(window.copy_enabled);
        assert_eq!(window.blink.ceiling, None);
        assert_eq!(
            window.theme.background(),
            theme::Theme::default().background()
        );
        let defaults = keyboard::Keymap::default();
        for action in keyboard::ALL {
            assert_eq!(window.keymap.chord_for(action), defaults.chord_for(action));
        }
        // The restored table must route a real chord, not merely contain the factory entry.
        one_frame_in(&mut window, &ctx, area, chord_with(egui::Key::T));
        assert_eq!(window.tabs.len(), 2);
    }
    #[test]
    fn global_entry_is_available_and_typing_stays_out_of_the_console() {
        for tabs in [1, 2] {
            let mut window = window_of_tabs((0..tabs).map(|_| ["sleep 30", "sleep 30"]));
            window.settings_editor =
                settings::editor::Editor::resolved(settings::Entry::Measuring, Some("closed"));
            let ctx = egui::Context::default();
            let area = area_of(1280.0);
            one_frame_in(&mut window, &ctx, area, Vec::new());
            let at = window.settings_editor.entry_rect().center();
            one_frame_in(&mut window, &ctx, area, click(at));
            assert!(
                window.settings_editor.is_open(),
                "entry did not open Settings with {tabs} tabs"
            );
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![
                    egui::Event::Text("must stay in settings".into()),
                    egui::Event::Paste("clipboard".into()),
                ],
            );
            assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
            let escape = egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![escape, egui::Event::Text("closing-frame".into())],
            );
            assert!(!window.settings_editor.is_open());
            assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Text("back to console".into())],
            );
            assert!(
                window
                    .active_mut()
                    .panes_mut()
                    .iter()
                    .any(|p| p.keystrokes == b"back to console")
            );
        }
    }
    #[test]
    fn an_open_editor_does_not_stop_output_in_any_tab() {
        let mut window = window_of_tabs([
            ["read -r line; printf visible; sleep 30", "sleep 30"],
            ["read -r line; printf hidden; sleep 30", "sleep 30"],
        ]);
        window.settings_editor =
            settings::editor::Editor::resolved(settings::Entry::Measuring, Some("closed"));
        let ctx = egui::Context::default();
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let at = window.settings_editor.entry_rect().center();
        one_frame_in(&mut window, &ctx, area, click(at));
        assert!(window.settings_editor.is_open());
        assert!(
            window
                .tabs
                .iter()
                .all(|t| !t.panes()[0].screen.to_text().contains("visible"))
        );
        for tab in &mut window.tabs {
            tab.panes_mut()[0].pty.write(b"go\n").unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline
            && (!window.tabs[0].panes()[0]
                .screen
                .to_text()
                .contains("visible")
                || !window.tabs[1].panes()[0]
                    .screen
                    .to_text()
                    .contains("hidden"))
        {
            window.pump_them_all();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            window.tabs[0].panes()[0]
                .screen
                .to_text()
                .contains("visible")
        );
        assert!(
            window.tabs[1].panes()[0]
                .screen
                .to_text()
                .contains("hidden")
        );
        assert!(window.settings_editor.is_open());
    }
    #[test]
    fn scripted_measurements_witness_opening_and_lazy_font_enumeration() {
        for mode in ["closed", "open", "fonts"] {
            let mut window = window_with("sleep 30", "sleep 30");
            window.settings_editor =
                settings::editor::Editor::resolved(settings::Entry::Measuring, Some(mode));
            let ctx = egui::Context::default();
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0));
            for _ in 0..6 {
                let mut raw = egui::RawInput::default();
                settings::editor::inject(&mut window, &mut raw);
                one_frame_in(&mut window, &ctx, area, raw.events);
            }
            assert_eq!(window.settings_editor.is_open(), mode != "closed");
            window.settings_editor.close_frame();
            let mut output = Vec::new();
            window.settings_editor.publish(&mut output).unwrap();
            let report = String::from_utf8(output).unwrap();
            let expected = if mode == "fonts" { 1 } else { 0 };
            assert!(
                report.contains(&format!("settings font enumeration: frames={expected},")),
                "{mode}: {report}"
            );
            assert!(!report.contains("settings entry: frames=0"));
            assert!(
                report.contains(&format!(
                    "settings font requests: {}",
                    if mode == "fonts" { 2 } else { 0 }
                )),
                "{mode}: {report}"
            );
            assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
        }
    }

    #[test]
    fn the_closed_entry_does_not_steal_tab_or_enter_from_the_program() {
        let mut window = window_with("sleep 30", "sleep 30");
        window.settings_editor =
            settings::editor::Editor::resolved(settings::Entry::Measuring, Some("closed"));
        let ctx = egui::Context::default();
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        for key in [egui::Key::Tab, egui::Key::Enter] {
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(!window.settings_editor.is_open());
            assert_eq!(
                window.active().panes()[0].keystrokes,
                if key == egui::Key::Tab { b"\t" } else { b"\r" }
            );
        }
    }
    #[test]
    fn closing_keeps_a_held_pointer_and_its_release_out_of_console_input() {
        let mut window = window_with("sleep 30", "sleep 30");
        window.settings_editor =
            settings::editor::Editor::resolved(settings::Entry::Measuring, Some("closed"));
        let ctx = egui::Context::default();
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let at = window.settings_editor.entry_rect().center();
        one_frame_in(&mut window, &ctx, area, click(at));
        let pressed = egui::Event::PointerButton {
            pos: egui::pos2(400.0, 200.0),
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        };
        one_frame_in(&mut window, &ctx, area, vec![pressed]);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!window.settings_editor.is_open());
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("held".into())],
        );
        assert!(window.active().panes()[0].keystrokes.is_empty());
        let released = egui::Event::PointerButton {
            pos: egui::pos2(400.0, 200.0),
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        };
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![released, egui::Event::Text("release".into())],
        );
        assert!(window.active().panes()[0].keystrokes.is_empty());
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("after".into())],
        );
        assert_eq!(window.active().panes()[0].keystrokes, b"after");
    }

    #[test]
    fn a_repeated_closing_pass_cannot_deliver_the_same_frames_text() {
        let mut window = window_with("sleep 30", "sleep 30");
        window.settings_editor =
            settings::editor::Editor::resolved(settings::Entry::Measuring, Some("closed"));
        let ctx = egui::Context::default();
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let at = window.settings_editor.entry_rect().center();
        one_frame_in(&mut window, &ctx, area, click(at));
        let mut frame = eframe::Frame::_new_kittest();
        let raw = egui::RawInput {
            screen_rect: Some(area),
            events: vec![
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Text("must not escape".into()),
            ],
            ..Default::default()
        };
        let mut passes = 0;
        ctx.run_ui(raw, |ui| {
            passes += 1;
            eframe::App::ui(&mut window, ui, &mut frame);
            if passes == 1 {
                ctx.request_discard("exercise a repeated closing pass");
            }
        })
        .drop_without_applying_deltas();
        assert_eq!(passes, 2);
        assert!(!window.settings_editor.is_open());
        assert!(window.active().panes()[0].keystrokes.is_empty());
    }
}
