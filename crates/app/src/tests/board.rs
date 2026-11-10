//! Overview effects through application dispatch and the real program's input file.
#[cfg(test)]
mod tests {
    use super::super::*;

    fn area() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0))
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

    fn frame(
        window: &mut Window,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        ui_pass(
            window,
            ctx,
            area(),
            egui::RawInput {
                events,
                ..Default::default()
            },
        )
    }

    fn integrated(window: &mut Window) {
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
    }

    #[test]
    fn overview_open_and_escape_preserve_consoles_and_swallow_the_closing_frames_input() {
        let received = term_core::received::Received::new("overview-escape");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        integrated(&mut window);
        let ctx = egui::Context::default();
        received.wait_until_ready(Duration::from_secs(5));
        frame(&mut window, &ctx, vec![]);
        let layout = window.active().root.describe();
        let focus = window.active().focus;
        let identities: Vec<_> = window.active().panes().iter().map(|p| p.name).collect();
        let grids: Vec<_> = window.active().panes().iter().map(|p| p.measured).collect();
        let shapes = frame(&mut window, &ctx, chord_with(egui::Key::O));
        assert!(
            window.summary.is_open(),
            "default shortcut did not open Overview"
        );
        assert!(the_text_queued_up(
            &shapes,
            "No task state has been published yet"
        ));
        frame(
            &mut window,
            &ctx,
            vec![
                egui::Event::Text("modal".into()),
                egui::Event::Paste("paste".into()),
            ],
        );
        frame(
            &mut window,
            &ctx,
            vec![
                key(egui::Key::Escape),
                egui::Event::Text("closing".into()),
                egui::Event::Paste("closing paste".into()),
            ],
        );
        assert!(!window.summary.is_open());
        assert_eq!(window.active().root.describe(), layout);
        assert_eq!(window.active().focus, focus);
        assert_eq!(
            window
                .active()
                .panes()
                .iter()
                .map(|p| p.name)
                .collect::<Vec<_>>(),
            identities
        );
        assert_eq!(
            window
                .active()
                .panes()
                .iter()
                .map(|p| p.measured)
                .collect::<Vec<_>>(),
            grids
        );
        frame(&mut window, &ctx, vec![egui::Event::Text("after".into())]);
        assert_eq!(
            received.wait_for(5, Duration::from_secs(5)),
            b"after",
            "Overview sent bytes to its program or never released input"
        );
    }
    #[test]
    fn configurable_overview_binding_returns_the_disabled_chord_to_the_program() {
        for (config, opening) in [
            ("key.overview=none", None),
            ("key.overview=ctrl+shift+u", Some(egui::Key::U)),
        ] {
            let received = term_core::received::Received::new("overview-binding");
            let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
            integrated(&mut window);
            let ctx = egui::Context::default();
            received.wait_until_ready(Duration::from_secs(5));
            let (settings, warnings) = settings::interpret(config).unwrap();
            assert!(warnings.kept().is_empty());
            settings.apply_to(&mut window);
            frame(&mut window, &ctx, chord_with(egui::Key::O));
            assert!(
                !window.summary.is_open(),
                "the factory chord survived {config}"
            );
            frame(&mut window, &ctx, vec![egui::Event::Text("witness".into())]);
            assert_eq!(received.wait_for(8, Duration::from_secs(5)), b"\x0fwitness");
            if let Some(opening) = opening {
                frame(&mut window, &ctx, chord_with(opening));
                assert!(
                    window.summary.is_open(),
                    "configured binding was parsed but not dispatched"
                );
                frame(&mut window, &ctx, vec![key(egui::Key::Escape)]);
                frame(&mut window, &ctx, vec![egui::Event::Text("after".into())]);
                assert_eq!(
                    received.wait_for(13, Duration::from_secs(5)),
                    b"\x0fwitnessafter"
                );
            }
        }
    }

    #[test]
    fn enter_restores_a_hidden_target_and_does_not_deliver_trailing_text_or_paste() {
        let received = term_core::received::Received::new("overview-enter");
        let script = received.script("stty raw -echo");
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", script.as_str()]]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        received.wait_until_ready(Duration::from_secs(5));
        let tab = window.tabs[1].name;
        let hidden = window.tabs[1].panes()[1].name;
        let first = window.tabs[1].panes()[0].name;
        window.pane_view.toggle(tab, first);
        let layout = window.tabs[1].root.describe();
        frame(&mut window, &ctx, vec![]);
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        frame(&mut window, &ctx, vec![key(egui::Key::End)]);
        frame(
            &mut window,
            &ctx,
            vec![
                key(egui::Key::Enter),
                egui::Event::Text("closing".into()),
                egui::Event::Paste("paste".into()),
            ],
        );
        assert!(!window.summary.is_open());
        assert_eq!(window.active().name, tab);
        assert_eq!(
            window.active().focus,
            hidden,
            "Enter did not resolve the selected console identity"
        );
        assert_eq!(
            window.pane_view.zoomed(tab),
            None,
            "target remained hidden behind a maximized sibling"
        );
        assert_eq!(window.active().root.describe(), layout);
        frame(&mut window, &ctx, vec![egui::Event::Text("after".into())]);
        assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"after");
    }

    #[test]
    fn repeated_egui_closing_passes_cannot_send_the_frames_input_to_a_program() {
        let received = term_core::received::Received::new("overview-repeat");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        integrated(&mut window);
        let ctx = egui::Context::default();
        received.wait_until_ready(Duration::from_secs(5));
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let mut native_frame = eframe::Frame::_new_kittest();
        let mut passes = 0;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area()),
                events: vec![
                    key(egui::Key::Escape),
                    egui::Event::Text("leak".into()),
                    egui::Event::Paste("paste".into()),
                ],
                ..Default::default()
            },
            |ui| {
                passes += 1;
                eframe::App::ui(&mut window, ui, &mut native_frame);
                if passes == 1 {
                    ctx.request_discard("repeat the Overview closing frame");
                }
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(passes, 2);
        assert!(!window.summary.is_open());
        frame(&mut window, &ctx, vec![egui::Event::Text("after".into())]);
        assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"after");
    }

    #[test]
    fn quiet_background_folded_panels_refresh_into_the_visible_overview() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        let background = &mut window.tabs[1].panes_mut()[1];
        background.panel = Some(PanelDir::create().unwrap());
        background.folded = true;
        publish_at(background, r#"{"lines":["Before quiet publication"]}"#);
        background.to_reread.raise();
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        assert!(the_text_queued_up(&shapes, "Before quiet publication"));
        let background = window.tabs[1].panes()[1];
        publish_at(
            background,
            r#"{"attention":"wait","phase":"quiet phase","lines":["Changed without terminal output"]}"#,
        );
        background.to_reread.raise();
        let shapes = frame(&mut window, &ctx, vec![]);
        assert!(
            the_text_queued_up(&shapes, "Changed without terminal output"),
            "Overview refreshed only the active or unfolded panes"
        );
        assert!(the_text_queued_up(&shapes, "quiet phase"));
        assert!(!the_text_queued_up(&shapes, "Before quiet publication"));
        assert_eq!(window.active, 0);
        assert!(window.tabs[1].panes()[1].folded);
    }

    #[test]
    fn a_pressed_row_removed_before_release_never_opens_its_replacement() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        frame(&mut window, &ctx, vec![]);
        let target = window.tabs[1].panes()[0].name;
        let at = window
            .summary
            .row_rect(target)
            .expect("target row painted")
            .center();
        let pointer = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut window,
            &ctx,
            vec![egui::Event::PointerMoved(at), pointer(true)],
        );
        assert_eq!(window.summary.selected(), Some(target));
        drop(
            window.tabs[1]
                .root
                .close(target)
                .expect("remove pressed console"),
        );
        let survivor = window.tabs[1].panes()[0].name;
        window.tabs[1].focus = survivor;
        // A control-channel removal can repaint while the person still holds the button.
        frame(&mut window, &ctx, vec![]);
        assert!(window.summary.row_rect(survivor).unwrap().contains(at));
        frame(&mut window, &ctx, vec![pointer(false)]);
        assert!(
            window.summary.is_open(),
            "release activated the replacement at the removed row's ordinal"
        );
        assert_eq!(
            window.active, 0,
            "removed target redirected navigation to a different tab"
        );
        assert_eq!(
            window.summary.selected(),
            Some(survivor),
            "selection did not clamp to the nearest surviving row"
        );
    }

    #[test]
    fn overview_shortcut_cannot_replace_settings_or_a_console_rename() {
        for rename in [false, true] {
            let mut window = window_with("sleep 30", "sleep 30");
            integrated(&mut window);
            let ctx = egui::Context::default();
            ctx.global_style_mut(|style| style.animation_time = 0.0);
            if rename {
                let tab = window.active().name;
                let pane = window.active().focus;
                window
                    .pane_view
                    .begin_rename(tab, pane, "Original label".into());
            } else {
                window.settings_editor =
                    settings::editor::Editor::resolved(settings::Entry::Measuring, Some("closed"));
                window.settings_editor.open_from_chrome();
            }
            frame(&mut window, &ctx, vec![]);
            frame(&mut window, &ctx, chord_with(egui::Key::O));
            assert!(!window.summary.is_open());
            assert!(
                if rename {
                    window.pane_view.is_renaming()
                } else {
                    window.settings_editor.is_open()
                },
                "Overview discarded another modal's draft"
            );
            assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
        }
    }

    #[test]
    fn escaping_overview_keeps_the_existing_search_selection_and_scrollback() {
        let mut window = window_with("sleep 30", "sleep 30");
        integrated(&mut window);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            one_frame_in(&mut window, &ctx, area(), vec![]);
        }
        let pane = &mut window.active_mut().panes_mut()[0];
        for n in 0..160 {
            pane.screen.advance(format!("needle {n}\r\n").as_bytes());
        }
        let mut steps = search::Steps::new(false);
        steps.toggle();
        assert!(steps.took(&egui::Event::Text("needle".into()), egui::Modifiers::NONE));
        search::apply(&mut pane.search, &mut pane.screen, steps);
        pane.screen.scroll_by(Scroll::Lines(5));
        pane.screen.start_selection(Cell {
            row: 0,
            col: 0,
            half: Half::Left,
        });
        pane.screen.extend_selection(Cell {
            row: 0,
            col: 5,
            half: Half::Right,
        });
        let selected = pane
            .screen
            .selected_text()
            .expect("fixture has selected text");
        let visible = pane.screen.to_text();
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        frame(
            &mut window,
            &ctx,
            vec![
                key(egui::Key::Escape),
                egui::Event::Text("must not edit query".into()),
            ],
        );
        let shapes = frame(&mut window, &ctx, vec![]);
        let pane = window.active().panes()[0];
        assert!(
            pane.search.is_open(),
            "Overview Escape closed the underlying search"
        );
        assert_eq!(
            pane.screen.selected_text().as_deref(),
            Some(selected.as_str())
        );
        assert_eq!(
            pane.screen.to_text(),
            visible,
            "Overview changed the console's scroll position"
        );
        assert!(the_text_queued_up(&shapes, "needle"));
        assert!(!the_text_queued_up(&shapes, "must not edit query"));
    }

    fn publish_with_timestamp(pane: &mut Pane, minutes: u64, written: SystemTime) {
        pane.panel = Some(PanelDir::create().unwrap());
        publish_at(
            pane,
            &format!(r#"{{"stale_after_minutes":{minutes},"lines":["Quiet producer"]}}"#),
        );
        let path = pane
            .panel
            .as_ref()
            .unwrap()
            .path()
            .join(panel_contract::PANEL_FILE);
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(written))
            .unwrap();
        pane.refresh_its_panel(&egui::Context::default());
    }

    #[test]
    fn overview_alarm_uses_inactive_and_folded_producers_only_while_their_rows_are_visible() {
        let written = SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000);
        let now = written + Duration::from_secs(10);
        for (folded_minutes, background_minutes) in [(1, 3), (3, 1)] {
            let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
            integrated(&mut window);
            publish_with_timestamp(window.active_mut().panes_mut()[0], folded_minutes, written);
            window.active_mut().panes_mut()[0].folded = true;
            publish_with_timestamp(window.active_mut().panes_mut()[1], 4, written);
            publish_with_timestamp(window.tabs[1].panes_mut()[0], background_minutes, written);
            assert_eq!(
                crate::alarms::panel_warning_due(&window, now),
                Some(Duration::from_secs(230))
            );
            let ctx = egui::Context::default();
            frame(&mut window, &ctx, chord_with(egui::Key::O));
            assert!(window.summary.is_open());
            assert_eq!(
                crate::alarms::panel_warning_due(&window, now),
                Some(Duration::from_secs(50)),
                "Overview omitted the earliest quiet producer: folded={folded_minutes}, background={background_minutes}"
            );
            assert_eq!(
                crate::alarms::panel_warning_due(&window, written + Duration::from_secs(65)),
                Some(Duration::from_secs(55)),
                "the stale producer's warning lost its next minute deadline"
            );
            frame(&mut window, &ctx, vec![key(egui::Key::Escape)]);
            assert_eq!(
                crate::alarms::panel_warning_due(&window, now),
                Some(Duration::from_secs(230)),
                "closing Overview left hidden producer alarms armed"
            );
        }
    }

    #[test]
    fn long_metadata_at_640_by_480_keeps_rows_separate_and_last_console_reachable() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        integrated(&mut window);
        let narrow = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0));
        window.font_size = 14.0;
        for tab in &mut window.tabs {
            tab.label = Some("Long tab ".repeat(600));
            for pane in tab.panes_mut() {
                pane.label = Some(format!(
                    "Long console {} {}",
                    pane.name,
                    "界\n".repeat(1000)
                ));
                pane.panel = Some(PanelDir::create().unwrap());
                let document = serde_json::json!({"phase":"long phase ".repeat(2000), "lines":["long body ".repeat(3000), "second", "third"]});
                publish_at(pane, &document.to_string());
                pane.to_reread.raise();
            }
        }
        let pass = |window: &mut Window, events| {
            ui_pass(
                window,
                &ctx,
                narrow,
                egui::RawInput {
                    events,
                    ..Default::default()
                },
            )
        };
        pass(&mut window, chord_with(egui::Key::O));
        let shapes = pass(&mut window, vec![]);
        let mut rows: Vec<_> = window
            .all_panes()
            .filter_map(|pane| window.summary.row_rect(pane.name))
            .filter(|r| r.is_positive())
            .collect();
        rows.sort_by(|a, b| a.top().total_cmp(&b.top()));
        assert!(
            rows.len() >= 2,
            "small Overview did not expose two list entries"
        );
        for pair in rows.windows(2) {
            assert!(
                pair[0].bottom() <= pair[1].top(),
                "long metadata overlapped row hit targets: {pair:?}"
            );
        }
        let mut checked_slots = 0;
        for shape in &shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && (text.galley.text().contains("Long console")
                    || text.galley.text().contains("long phase")
                    || text.galley.text().contains("long body"))
                && rows
                    .iter()
                    .any(|row| row.contains(text.pos) && row.contains_rect(shape.clip_rect))
            {
                checked_slots += 1;
                assert_eq!(
                    text.galley.rows.len(),
                    1,
                    "metadata expanded a reserved line slot"
                );
                assert!(
                    narrow.contains_rect(shape.clip_rect),
                    "metadata escaped the Overview viewport"
                );
            }
        }
        assert!(
            checked_slots >= 2,
            "long metadata was not painted inside the Overview row slots"
        );
        assert!(
            the_text_queued_up(&shapes, "Close"),
            "close control was displaced by metadata"
        );
        pass(&mut window, vec![key(egui::Key::End)]);
        let target = window.tabs[1].panes()[1].name;
        let rect = window
            .summary
            .row_rect(target)
            .expect("End scrolled target into view");
        assert!(
            rect.is_positive() && narrow.contains(rect.center()),
            "selected console remained outside the small viewport"
        );
        pass(&mut window, vec![key(egui::Key::Enter)]);
        assert_eq!(window.active().focus, target);
        assert!(!window.summary.is_open());
    }

    #[test]
    fn overview_paints_only_three_literal_body_lines_with_their_style_and_indentation() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        let pane = &mut window.tabs[1].panes_mut()[0];
        pane.panel = Some(PanelDir::create().unwrap());
        publish_at(
            pane,
            r##"{"phase":"literal phase","lines":["   ",{"text":"first literal","mark":"block","style":"warn","indent":2,"ref":"#42","link":"https://example.test/inert"},"second literal","third literal","fourth must stay hidden"],"actions":[{"label":"Do not render this action","send":"unsafe command"}]}"##,
        );
        pane.to_reread.raise();
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        for text in [
            "literal phase",
            "#42 first literal",
            "second literal",
            "third literal",
            "No attention requested",
        ] {
            assert!(
                the_text_queued_up(&shapes, text),
                "literal projection omitted {text:?}"
            );
        }
        for text in [
            "fourth must stay hidden",
            "Do not render this action",
            "unsafe command",
            "https://example.test/inert",
        ] {
            assert!(
                !the_text_queued_up(&shapes, text),
                "Overview rendered non-excerpt content {text:?}"
            );
        }
        let text_shape = |needle: &str| {
            shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text().contains(needle) => Some(t),
                    _ => None,
                })
                .unwrap()
        };
        let first = text_shape("first literal");
        let second = text_shape("second literal");
        let third = text_shape("third literal");
        assert!(first.pos.x > second.pos.x, "producer indentation was lost");
        assert!(
            first.pos.y < second.pos.y && second.pos.y < third.pos.y,
            "body order changed"
        );
        let expected = window.theme.panel_color(
            Some(panel_contract::Mark::Block),
            Some(panel_contract::Style::Warn),
        );
        assert!(
            first
                .galley
                .job
                .sections
                .iter()
                .all(|s| s.format.color == expected),
            "producer style/mark did not reach painted text"
        );
    }

    #[test]
    fn one_console_can_open_overview_by_mouse_with_the_shortcut_disabled() {
        let mut window = window_with("sleep 30", "sleep 30");
        let removed = window.active().panes()[1].name;
        drop(window.active_mut().root.close(removed).unwrap());
        integrated(&mut window);
        settings::interpret("key.overview=none")
            .unwrap()
            .0
            .apply_to(&mut window);
        let ctx = egui::Context::default();
        frame(&mut window, &ctx, vec![]);
        // The chrome draws this control as vector strokes, not as a text widget.
        let from_right = if cfg!(target_os = "macos") {
            91.0
        } else {
            193.0
        };
        let at = egui::pos2(area().right() - from_right, 24.0);
        frame(&mut window, &ctx, click_at(at));
        assert!(
            window.summary.is_open(),
            "mouse entry depended on a bound shortcut or multiple consoles"
        );
        assert_eq!(window.tabs.len(), 1);
        assert_eq!(window.active().panes().len(), 1);
    }
}
