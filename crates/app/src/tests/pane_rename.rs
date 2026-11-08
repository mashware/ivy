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
        assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
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
