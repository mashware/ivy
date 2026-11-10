//! The rename editor owns input through the frame which dismisses it.
#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn editing_console_and_tab_names_keeps_them_in_the_layout() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(1280.0);
        let tab = window.active().name;
        let pane = window.active().focus;
        for is_tab in [false, true] {
            if is_tab {
                window.pane_view.begin_tab_rename(tab, String::new());
            } else {
                window.pane_view.begin_rename(tab, pane, String::new());
            }
            one_frame_in(&mut window, &ctx, area, vec![]);
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Text(
                    if is_tab {
                        "Review tab"
                    } else {
                        "Review console"
                    }
                    .into(),
                )],
            );
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(!window.pane_view.is_renaming());
        }
        assert_eq!(window.active().label.as_deref(), Some("Review tab"));
        assert_eq!(
            window.pane_called(pane).unwrap().label.as_deref(),
            Some("Review console")
        );
        let restored = wrap_around(&window, &ctx);
        assert_eq!(restored[0].label.as_deref(), Some("Review tab"));
        assert_eq!(
            restored[0].panes()[0].label.as_deref(),
            Some("Review console")
        );
        assert!(
            window
                .all_panes_mut()
                .all(|pane| pane.keystrokes.is_empty())
        );
    }

    #[test]
    fn real_double_click_on_label_opens_rename_on_second_release() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let pane_name = window.active().focus;
        window
            .pane_called(pane_name)
            .unwrap()
            .screen
            .advance(b"\x1b]2;Session name\x07");
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, vec![]);
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let work = tab_bar::geometry(&window, area, &metrics).consoles;
        let pane = pane_view::view_tiling(&window, work, &metrics).parts[0];
        let at = pane.left_top()
            + egui::vec2(
                metrics.width() * 2.0,
                header::height(&metrics, window.headers) / 2.0,
            );
        for (time, pressed) in [(1.0, true), (1.05, false), (1.1, true), (1.15, false)] {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::logic(&mut window, &ctx, &mut frame);
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(area),
                    time: Some(time),
                    events: vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| eframe::App::ui(&mut window, ui, &mut frame),
            )
            .drop_without_applying_deltas();
            if time < 1.15 {
                assert!(!window.pane_view.is_renaming());
            }
        }
        assert!(window.pane_view.is_renaming());
        draw(&mut window, &ctx, vec![key(egui::Key::Enter)]);
        let shapes = draw(&mut window, &ctx, vec![]);
        assert_eq!(
            window.pane_called(pane_name).unwrap().label.as_deref(),
            Some("Session name")
        );
        assert!(the_text_queued_up(&shapes, "Session name"));
        assert!(!the_text_queued_up(&shapes, "Console 1"));
        assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
    }

    fn draw(
        window: &mut Window,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        ui_pass(
            window,
            ctx,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 1100.0)),
            egui::RawInput {
                events,
                ..Default::default()
            },
        )
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn text_center(shapes: &[egui::epaint::ClippedShape], text: &str) -> egui::Pos2 {
        fn find(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == text => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, text)),
                _ => None,
            }
        }
        shapes
            .iter()
            .find_map(|s| find(&s.shape, text))
            .expect("the requested control is painted")
    }

    #[test]
    fn terminal_titles_reach_headers_and_board_including_background_panes() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let names = ["OC | Feature", "Review", "Background", "Fourth"];
        for (pane, title) in window.all_panes_mut().zip(names) {
            pane.screen
                .advance(format!("\x1b]2;{title}\x07").as_bytes());
        }
        let ctx = egui::Context::default();
        let shapes = draw(&mut window, &ctx, vec![]);
        assert!(the_text_queued_up(&shapes, "OC | Feature"));
        assert!(the_text_queued_up(&shapes, "Review"));
        let active = window.active().focus;
        window
            .pane_called(active)
            .unwrap()
            .screen
            .advance(b"\x1b]2;Ship next\x07");
        let shapes = draw(&mut window, &ctx, vec![]);
        assert!(the_text_queued_up(&shapes, "Ship next"));
        assert!(!the_text_queued_up(&shapes, "OC | Feature"));
        let shapes = draw(&mut window, &ctx, chord_with(egui::Key::O));
        for title in ["Ship next", names[1], names[2], names[3]] {
            assert!(the_text_queued_up(&shapes, title), "Board omitted {title}");
        }
        window.tabs[1].panes_mut()[0]
            .screen
            .advance(b"\x1b]2;Renamed background\x07");
        let shapes = draw(&mut window, &ctx, vec![]);
        assert!(the_text_queued_up(&shapes, "Renamed background"));
        draw(&mut window, &ctx, vec![key(egui::Key::Escape)]);
        let pane = window.active().focus;
        window
            .pane_called(pane)
            .unwrap()
            .screen
            .advance(b"\x1b]2;\x07");
        let shapes = draw(&mut window, &ctx, vec![]);
        assert!(the_text_queued_up(&shapes, "Console 1"));
        assert!(!the_text_queued_up(&shapes, "OC | Feature"));
    }

    #[test]
    fn terminal_title_override_and_return_action_own_input_and_preserve_manual_names_only() {
        let received = term_core::received::Received::new("terminal-title-rename");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        received.wait_until_ready(Duration::from_secs(5));
        let ctx = egui::Context::default();
        let tab = window.active().name;
        let name = window.active().focus;
        window
            .pane_called(name)
            .unwrap()
            .screen
            .advance(b"\x1b]2;Session\x07");
        draw(&mut window, &ctx, vec![]);
        window.pane_view.begin_rename(tab, name, String::new());
        draw(&mut window, &ctx, vec![]);
        draw(&mut window, &ctx, vec![egui::Event::Text("Manual".into())]);
        draw(&mut window, &ctx, vec![key(egui::Key::Enter)]);
        window
            .pane_called(name)
            .unwrap()
            .screen
            .advance(b"\x1b]2;New session\x07");
        let shapes = draw(&mut window, &ctx, vec![]);
        assert!(the_text_queued_up(&shapes, "Manual"));
        assert!(!the_text_queued_up(&shapes, "New session"));
        let restored = wrap_around(&window, &ctx);
        assert_eq!(pane_view::display_label(restored[0].panes()[0]), "Manual");
        assert_eq!(restored[0].panes()[0].screen.terminal_title(), None);
        drop(restored);

        window.pane_view.begin_rename(tab, name, "Manual".into());
        draw(&mut window, &ctx, vec![]);
        draw(&mut window, &ctx, vec![key(egui::Key::Escape)]);
        assert_eq!(
            pane_view::display_label(window.pane_called(name).unwrap()),
            "Manual"
        );
        window.pane_view.begin_rename(tab, name, "Manual".into());
        draw(&mut window, &ctx, vec![]);
        let shapes = draw(&mut window, &ctx, vec![]);
        let at = text_center(&shapes, "Use terminal title");
        draw(&mut window, &ctx, click_at(at));
        let shapes = draw(&mut window, &ctx, vec![]);
        assert!(!window.pane_view.is_renaming());
        assert!(the_text_queued_up(&shapes, "New session"));
        assert_eq!(window.pane_called(name).unwrap().label, None);
        let restored = wrap_around(&window, &ctx);
        assert_eq!(
            pane_view::display_label(restored[0].panes()[0]),
            "Console 1"
        );
        assert_eq!(restored[0].panes()[0].label, None);
        draw(&mut window, &ctx, vec![egui::Event::Text("after".into())]);
        assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"after");
    }

    #[test]
    fn terminal_title_blank_manual_labels_and_empty_save_follow_the_latest_title() {
        let mut window = window_with("sleep 30", "sleep 30");
        let ctx = egui::Context::default();
        let name = window.active().focus;
        let tab = window.active().name;
        for label in ["", "   "] {
            window.pane_called(name).unwrap().label = Some(label.into());
            let mut restored = wrap_around(&window, &ctx);
            let pane = &mut restored[0].panes_mut()[0];
            pane.screen.advance(b"\x1b]2;Live name\x07");
            assert_eq!(pane_view::display_label(pane), "Live name");
        }
        window.pane_called(name).unwrap().label = Some("Manual".into());
        window
            .pane_called(name)
            .unwrap()
            .screen
            .advance(b"\x1b]2;Live name\x07");
        window.pane_view.begin_rename(tab, name, String::new());
        draw(&mut window, &ctx, vec![]);
        draw(&mut window, &ctx, vec![key(egui::Key::Enter)]);
        assert_eq!(window.pane_called(name).unwrap().label, None);
        assert_eq!(
            pane_view::display_label(window.pane_called(name).unwrap()),
            "Live name"
        );
    }

    #[test]
    fn dismissing_rename_does_not_type_into_the_console() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(1280.0);
        let tab = window.active().name;
        let pane = window.active().focus;
        window.pane_view.begin_rename(tab, pane, "Console".into());
        one_frame_in(&mut window, &ctx, area, vec![]);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::Text("not for the shell".into()),
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(!window.pane_view.is_renaming());
        assert!(
            window
                .all_panes_mut()
                .all(|pane| pane.keystrokes.is_empty())
        );
    }
    #[test]
    fn a_modal_owner_prevents_double_click_from_opening_another_editor() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        window.settings_editor = settings::editor::Editor::resolved(settings::Entry::Normal, None);
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
        one_frame_in(&mut window, &ctx, area, vec![]);
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let work = tab_bar::geometry(&window, area, &metrics).consoles;
        let at = work.left_top() + egui::vec2(16.0, header::height(&metrics, window.headers) / 2.0);
        window.settings_editor.open_from_chrome();
        one_frame_in(&mut window, &ctx, area, vec![]);
        for (time, pressed) in [(1.0, true), (1.05, false), (1.1, true), (1.15, false)] {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::logic(&mut window, &ctx, &mut frame);
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(area),
                    time: Some(time),
                    events: vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| eframe::App::ui(&mut window, ui, &mut frame),
            )
            .drop_without_applying_deltas();
            assert!(
                !window.pane_view.is_renaming(),
                "a release escaped the Preferences input owner"
            );
            assert!(window.settings_editor.is_open());
        }
    }
}
