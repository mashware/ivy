//! Board effects through application dispatch and the real program's input file.
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
    fn board_open_and_escape_preserve_consoles_and_swallow_the_closing_frames_input() {
        let received = term_core::received::Received::new("board-escape");
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
            window.board.is_open(),
            "default shortcut did not open Board"
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
        assert!(!window.board.is_open());
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
            "Board sent bytes to its program or never released input"
        );
    }
    #[test]
    fn configurable_board_binding_returns_the_disabled_chord_to_the_program() {
        for (config, opening) in [
            ("key.board=none", None),
            ("key.board=ctrl+shift+u", Some(egui::Key::U)),
        ] {
            let received = term_core::received::Received::new("board-binding");
            let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
            integrated(&mut window);
            let ctx = egui::Context::default();
            received.wait_until_ready(Duration::from_secs(5));
            let (settings, warnings) = settings::interpret(config).unwrap();
            assert!(warnings.kept().is_empty());
            settings.apply_to(&mut window);
            frame(&mut window, &ctx, chord_with(egui::Key::O));
            assert!(
                !window.board.is_open(),
                "the factory chord survived {config}"
            );
            frame(&mut window, &ctx, vec![egui::Event::Text("witness".into())]);
            assert_eq!(received.wait_for(8, Duration::from_secs(5)), b"\x0fwitness");
            if let Some(opening) = opening {
                frame(&mut window, &ctx, chord_with(opening));
                assert!(
                    window.board.is_open(),
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
    fn the_name_before_the_board_is_an_unknown_key() {
        let (_, warnings) = settings::interpret("# mine\nkey.overview=ctrl+shift+o").unwrap();
        let warning = warnings
            .kept()
            .first()
            .expect("key.overview was taken silently");
        assert_eq!(warning.line, 2, "the warning points at the wrong line");
        assert!(warning.to_string().contains("overview"), "{warning}");
    }

    #[test]
    fn up_and_down_cross_into_the_next_section_by_the_grid() {
        use crate::project::Key;
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        let badge = |path: &str, name: &str| {
            Some(crate::project_view::Badge {
                key: Key::Repository(path.into()),
                name: name.to_owned(),
                details: String::new(),
                ink: egui::Color32::RED,
                painted: Default::default(),
            })
        };
        let names: Vec<usize> = window.all_panes().map(|pane| pane.name).collect();
        let projects = [
            badge("/work/ivy/.git", "Ivy"),
            badge("/work/ivy/.git", "Ivy"),
            badge("/work/ivy/.git", "Ivy"),
            badge("/work/facq/.git", "FacQ"),
        ];
        for (pane, project) in window.all_panes_mut().zip(projects) {
            pane.project = project;
        }
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        frame(&mut window, &ctx, vec![key(egui::Key::Home)]);
        assert_eq!(window.board.selected(), Some(names[0]));
        frame(&mut window, &ctx, vec![key(egui::Key::ArrowDown)]);
        assert_eq!(
            window.board.selected(),
            Some(names[3]),
            "↓ from the first Ivy card goes to FacQ's row, where reading order gives the next Ivy card"
        );
        frame(&mut window, &ctx, vec![key(egui::Key::ArrowUp)]);
        assert_eq!(window.board.selected(), Some(names[0]));
        frame(&mut window, &ctx, vec![key(egui::Key::ArrowRight)]);
        assert_eq!(window.board.selected(), Some(names[1]));
    }

    #[test]
    fn the_board_scrolls_only_to_bring_a_card_and_its_header_into_sight() {
        use crate::project::Key;
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 4]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        let small = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 560.0));
        for pane in window.all_panes_mut() {
            pane.project = Some(crate::project_view::Badge {
                key: Key::Repository("/work/ivy/.git".into()),
                name: "Ivy".to_owned(),
                details: String::new(),
                ink: egui::Color32::RED,
                painted: Default::default(),
            });
        }
        let names: Vec<usize> = window.all_panes().map(|pane| pane.name).collect();
        let pass = |window: &mut Window, events| {
            ui_pass(
                window,
                &ctx,
                small,
                egui::RawInput {
                    events,
                    ..Default::default()
                },
            )
        };
        pass(&mut window, chord_with(egui::Key::O));
        pass(&mut window, vec![key(egui::Key::End)]);
        pass(&mut window, vec![]);
        assert!(
            window.board.card_rect(names[0]).is_none(),
            "eight cards in two columns do not fit 560 points, so End must scroll the first away"
        );
        // The last row sits at the bottom of the screen; its neighbour is in sight there too, and
        // aligning it to the top would be the jump this test exists to forbid.
        let bottom = window
            .board
            .card_rect(names[7])
            .expect("last card on screen");
        assert!(
            window.board.card_rect(names[5]).is_some(),
            "End aligned the last row to the top instead of scrolling the least"
        );
        pass(&mut window, vec![key(egui::Key::ArrowLeft)]);
        pass(&mut window, vec![]);
        assert_eq!(window.board.selected(), Some(names[6]));
        assert_eq!(
            window.board.card_rect(names[7]),
            Some(bottom),
            "moving to a card in sight at the bottom scrolled the board"
        );
        pass(&mut window, vec![key(egui::Key::Home)]);
        let shapes = pass(&mut window, vec![]);
        let header_seen = shapes.iter().any(|s| {
            matches!(&s.shape, egui::Shape::Text(t)
            if t.galley.text() == "Ivy" && small.contains(t.pos)
                && t.galley.job.sections.first().is_some_and(|section| {
                    section.format.font_id.family == egui::FontFamily::Monospace
                }))
        });
        assert!(
            header_seen,
            "Home brought the first card back without its section header"
        );
        let before = window
            .board
            .card_rect(names[0])
            .expect("first card on screen");
        // The second row is on screen whole under the first; aligning it to the top would move
        // the first card off, which is what an unremembered viewport does.
        // Painted rects are clipped to the screen, so a whole card is one as tall as the first.
        assert!(
            window
                .board
                .card_rect(names[2])
                .is_some_and(|card| card.height() == before.height()),
            "the fixture needs the second row fully in sight"
        );
        pass(&mut window, vec![key(egui::Key::ArrowDown)]);
        pass(&mut window, vec![]);
        assert_eq!(window.board.selected(), Some(names[2]));
        assert_eq!(
            window.board.card_rect(names[0]),
            Some(before),
            "moving to a card already in sight scrolled the board"
        );
    }

    #[test]
    fn a_card_paints_its_claim_and_its_warning() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        {
            let pane = &mut window.tabs[0].panes_mut()[0];
            pane.panel = Some(PanelDir::create().unwrap());
            publish_at(pane, r#"{"attention":"wait","lines":["asking"]}"#);
            pane.to_reread.raise();
        }
        publish_with_timestamp(
            window.tabs[1].panes_mut()[1],
            1,
            SystemTime::UNIX_EPOCH + Duration::from_secs(60),
        );
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        // The console panels under the board paint the same claim and warning, so only text that
        // starts inside the console's own card counts.
        let on_card = |pane: usize, needle: &str| {
            let card = window.board.card_rect(pane).expect("card painted");
            shapes.iter().any(|s| {
                matches!(&s.shape, egui::Shape::Text(t)
                    if t.galley.text().contains(needle) && card.contains(t.pos))
            })
        };
        let asking = window.tabs[0].panes()[0].name;
        assert!(
            on_card(asking, "it is waiting for you"),
            "the claim is not on its card"
        );
        let stale = window.tabs[1].panes()[1].name;
        let warning = crate::panel::summary::snapshot(
            &window.tabs[1].panes()[1].panel_state,
            SystemTime::now(),
        )
        .warning
        .expect("the publication is stale");
        let start: String = warning.chars().take(12).collect();
        assert!(
            on_card(stale, &start),
            "the warning {warning:?} is not on its card"
        );
    }

    #[test]
    fn consoles_are_grouped_by_repository_and_a_folder_joins_the_other_consoles() {
        use crate::project::Key;
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        let badge = |key: Key, name: &str| crate::project_view::Badge {
            key,
            name: name.to_owned(),
            details: String::new(),
            ink: egui::Color32::RED,
            painted: Default::default(),
        };
        let ivy = || Key::Repository("/work/ivy/.git".into());
        let projects = [
            Some(badge(ivy(), "Ivy")),
            Some(badge(Key::Repository("/work/facq/.git".into()), "FacQ")),
            Some(badge(Key::Folder("/home/me/Downloads".into()), "Downloads")),
            Some(badge(ivy(), "Ivy")),
        ];
        let names: Vec<usize> = window.all_panes().map(|pane| pane.name).collect();
        for (pane, project) in window.all_panes_mut().zip(projects) {
            pane.project = project;
        }
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        // The console headers under the board paint the same project names, in a proportional face.
        let on_the_board =
            |t: &egui::epaint::TextShape| {
                t.galley.job.sections.first().is_some_and(|section| {
                    section.format.font_id.family == egui::FontFamily::Monospace
                })
            };
        let header = |name: &str| {
            shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == name && on_the_board(t) => {
                        Some(t.pos.y)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no section header {name:?}"))
        };
        let (ivy_at, facq_at, other_at) = (header("Ivy"), header("FacQ"), header("Other consoles"));
        assert!(
            ivy_at < facq_at && facq_at < other_at,
            "sections out of order"
        );
        assert!(
            shapes.iter().all(|s| !matches!(&s.shape,
                egui::Shape::Text(t) if t.galley.text() == "Downloads" && on_the_board(t))),
            "a folder became a section"
        );
        let card = |n: usize| window.board.card_rect(names[n]).expect("card painted");
        assert!(
            card(2).top() > other_at,
            "the folder console left «Other consoles»"
        );
        assert!(
            card(3).top() < facq_at,
            "the second Ivy console left its section"
        );
        let mut report = Vec::new();
        window.board.timing.publish(&mut report).unwrap();
        let report = String::from_utf8(report).unwrap();
        assert!(
            report.contains("repositories=2, ungrouped=0,"),
            "the report counted «Other consoles» as a repository, or missed one: {report}"
        );
    }

    #[test]
    fn enter_restores_a_hidden_target_and_does_not_deliver_trailing_text_or_paste() {
        let received = term_core::received::Received::new("board-enter");
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
        assert!(!window.board.is_open());
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
        let received = term_core::received::Received::new("board-repeat");
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
                    ctx.request_discard("repeat the Board closing frame");
                }
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(passes, 2);
        assert!(!window.board.is_open());
        frame(&mut window, &ctx, vec![egui::Event::Text("after".into())]);
        assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"after");
    }

    #[test]
    fn quiet_background_folded_panels_refresh_into_the_visible_board() {
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
            "Board refreshed only the active or unfolded panes"
        );
        assert!(the_text_queued_up(&shapes, "quiet phase"));
        assert!(!the_text_queued_up(&shapes, "Before quiet publication"));
        assert_eq!(window.active, 0);
        assert!(window.tabs[1].panes()[1].folded);
    }

    #[test]
    fn a_pressed_card_removed_before_release_never_opens_its_replacement() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        integrated(&mut window);
        let ctx = egui::Context::default();
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        frame(&mut window, &ctx, vec![]);
        let target = window.tabs[1].panes()[0].name;
        let at = window
            .board
            .card_rect(target)
            .expect("target card painted")
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
        assert_eq!(window.board.selected(), Some(target));
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
        assert!(window.board.card_rect(survivor).unwrap().contains(at));
        frame(&mut window, &ctx, vec![pointer(false)]);
        assert!(
            window.board.is_open(),
            "release activated the replacement at the removed card's ordinal"
        );
        assert_eq!(
            window.active, 0,
            "removed target redirected navigation to a different tab"
        );
        assert_eq!(
            window.board.selected(),
            Some(survivor),
            "selection did not clamp to the nearest surviving card"
        );
    }

    #[test]
    fn board_shortcut_cannot_replace_settings_or_a_console_rename() {
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
            assert!(!window.board.is_open());
            assert!(
                if rename {
                    window.pane_view.is_renaming()
                } else {
                    window.settings_editor.is_open()
                },
                "Board discarded another modal's draft"
            );
            assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
        }
    }

    #[test]
    fn escaping_board_keeps_the_existing_search_selection_and_scrollback() {
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
            "Board Escape closed the underlying search"
        );
        assert_eq!(
            pane.screen.selected_text().as_deref(),
            Some(selected.as_str())
        );
        assert_eq!(
            pane.screen.to_text(),
            visible,
            "Board changed the console's scroll position"
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
    fn board_alarm_uses_inactive_and_folded_producers_only_while_their_cards_are_visible() {
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
            assert!(window.board.is_open());
            assert_eq!(
                crate::alarms::panel_warning_due(&window, now),
                Some(Duration::from_secs(50)),
                "Board omitted the earliest quiet producer: folded={folded_minutes}, background={background_minutes}"
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
                "closing Board left hidden producer alarms armed"
            );
        }
    }

    #[test]
    fn long_metadata_at_640_by_480_keeps_cards_separate_and_last_console_reachable() {
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
        let cards: Vec<_> = window
            .all_panes()
            .filter_map(|pane| window.board.card_rect(pane.name))
            .filter(|r| r.is_positive())
            .collect();
        assert!(cards.len() >= 2, "small Board did not expose two cards");
        // Cards sit side by side as well as one under another, so no pair may share a point.
        for (n, card) in cards.iter().enumerate() {
            for other in &cards[n + 1..] {
                assert!(
                    !card.intersects(*other),
                    "long metadata overlapped card hit targets: {card:?} {other:?}"
                );
            }
        }
        let mut checked_slots = 0;
        for shape in &shapes {
            if let egui::Shape::Text(text) = &shape.shape
                && (text.galley.text().contains("Long console")
                    || text.galley.text().contains("long phase")
                    || text.galley.text().contains("long body"))
                && cards
                    .iter()
                    .any(|card| card.contains(text.pos) && card.contains_rect(shape.clip_rect))
            {
                checked_slots += 1;
                assert_eq!(
                    text.galley.rows.len(),
                    1,
                    "metadata expanded a reserved line slot"
                );
                assert!(
                    narrow.contains_rect(shape.clip_rect),
                    "metadata escaped the Board viewport"
                );
            }
        }
        assert!(
            checked_slots >= 2,
            "long metadata was not painted inside the Board card slots"
        );
        assert!(
            the_text_queued_up(&shapes, "Close"),
            "close control was displaced by metadata"
        );
        pass(&mut window, vec![key(egui::Key::End)]);
        let target = window.tabs[1].panes()[1].name;
        let rect = window
            .board
            .card_rect(target)
            .expect("End scrolled target into view");
        assert!(
            rect.is_positive() && narrow.contains(rect.center()),
            "selected console remained outside the small viewport"
        );
        pass(&mut window, vec![key(egui::Key::Enter)]);
        assert_eq!(window.active().focus, target);
        assert!(!window.board.is_open());
    }

    #[test]
    fn board_paints_only_three_literal_body_lines_with_their_style_and_indentation() {
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
        ] {
            assert!(
                the_text_queued_up(&shapes, text),
                "literal projection omitted {text:?}"
            );
        }
        for text in [
            "fourth must stay hidden",
            "No attention requested",
            "Do not render this action",
            "unsafe command",
            "https://example.test/inert",
        ] {
            assert!(
                !the_text_queued_up(&shapes, text),
                "Board rendered non-excerpt content {text:?}"
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
    fn one_console_can_open_board_by_mouse_with_the_shortcut_disabled() {
        let mut window = window_with("sleep 30", "sleep 30");
        let removed = window.active().panes()[1].name;
        drop(window.active_mut().root.close(removed).unwrap());
        integrated(&mut window);
        settings::interpret("key.board=none")
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
            window.board.is_open(),
            "mouse entry depended on a bound shortcut or multiple consoles"
        );
        assert_eq!(window.tabs.len(), 1);
        assert_eq!(window.active().panes().len(), 1);
    }

    #[test]
    fn project_plan_calls_refuse_unknown_and_folder_projects_without_changing_a_plan() {
        let mut window = window_with("sleep 30", "sleep 30");
        let ctx = egui::Context::default();
        let origin = window.active().focus;
        attach_a_channel(&mut window, origin);
        let answer = ask_the_window(
            &mut window,
            &ctx,
            origin,
            1,
            panel_control::What::PlanRetire,
        );
        assert!(
            matches!(answer, panel_control::Answer::Refused(s) if s == "this console's project is not known yet")
        );
        let pane = window.all_panes_mut().find(|p| p.name == origin).unwrap();
        pane.project = Some(crate::project_view::Badge {
            key: crate::project::Key::Folder("/plain".into()),
            name: "Plain".into(),
            details: String::new(),
            ink: egui::Color32::RED,
            painted: Default::default(),
        });
        let answer = ask_the_window(&mut window, &ctx, origin, 1, panel_control::What::PlanGet);
        assert!(
            matches!(answer, panel_control::Answer::Refused(s) if s == "this console is not in a Git repository")
        );
    }

    #[test]
    fn project_plan_is_shared_by_panes_and_painted_with_source_and_progress() {
        let mut window = window_with("sleep 30", "sleep 30");
        integrated(&mut window);
        let ctx = egui::Context::default();
        let origins: Vec<_> = window.all_panes().map(|p| p.name).collect();
        for pane in window.all_panes_mut() {
            pane.project = Some(crate::project_view::Badge {
                key: crate::project::Key::Repository("/sample/.git".into()),
                name: "Sample".into(),
                details: String::new(),
                ink: egui::Color32::RED,
                painted: Default::default(),
            });
        }
        for origin in &origins {
            attach_a_channel(&mut window, *origin);
        }
        let document = r#"{"title":"Delivery","source":"roadmap.md","milestones":[{"id":"H3","title":"Build","items":[{"id":"D","text":"Feature","mark":"current"}]}]}"#;
        let (plan, dropped) = panel_contract::plan::parse_document_noting(document).unwrap();
        let answer = ask_the_window(
            &mut window,
            &ctx,
            origins[0],
            1,
            panel_control::What::PlanSet {
                document: document.into(),
                plan,
                dropped,
            },
        );
        assert!(
            matches!(answer, panel_control::Answer::Text(s) if s.starts_with("published for Sample"))
        );
        let answer = ask_the_window(
            &mut window,
            &ctx,
            origins[1],
            2,
            panel_control::What::PlanGet,
        );
        assert!(matches!(answer, panel_control::Answer::Document(s) if s == document));
        crate::board::plans::refresh_readers(&mut window);
        let dir = window
            .all_panes()
            .find(|p| p.name == origins[1])
            .unwrap()
            .panel
            .as_ref()
            .unwrap();
        let mut producer = panel_control::Producer::of(dir.path(), dir.id()).unwrap();
        assert!(
            matches!(producer.read_plan(Some("/another/repo")).unwrap(), panel_control::Answered::Document(s) if s == document)
        );
        let panel = panel_contract::parse_document(
            r#"{"milestone":"H3","deliverable":"D","lines":["working"]}"#,
        )
        .unwrap();
        window
            .all_panes_mut()
            .find(|p| p.name == origins[0])
            .unwrap()
            .panel_state
            .deliver(1, "flow", panel, SystemTime::now())
            .unwrap();
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        for expected in [
            "published ",
            "roadmap.md",
            "Build - 0/1",
            "Feature",
            "Off the plan",
        ] {
            assert!(the_text_queued_up(&shapes, expected), "missing {expected}");
        }
        let first = window.board.card_rect(origins[0]).unwrap();
        let second = window.board.card_rect(origins[1]).unwrap();
        assert!(
            first.top() < second.top(),
            "deliverable card must precede the off-plan card"
        );
        let mut report = Vec::new();
        window.board.timing.publish(&mut report).unwrap();
        let report = String::from_utf8(report).unwrap();
        assert!(report.contains("board plan projection:"));
        assert!(report.contains("board plan paint enqueue:"));
    }
}
