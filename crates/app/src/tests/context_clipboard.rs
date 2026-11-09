#[cfg(test)]
mod tests {
    use super::super::*;

    fn warmed() -> (Window, egui::Context) {
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        (window, ctx)
    }

    #[test]
    fn rename_dialogs_stay_centered_after_viewport_resize() {
        for tab_name in [false, true] {
            let (mut window, ctx) = warmed();
            let tab = window.active().name;
            let pane = window.active().focus;
            let title = if tab_name {
                window.pane_view.begin_tab_rename(tab, "Review".into());
                "Rename tab"
            } else {
                window.pane_view.begin_rename(tab, pane, "Review".into());
                "Rename console"
            };
            for size in [egui::vec2(1400.0, 900.0), egui::vec2(950.0, 720.0)] {
                let area = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
                ui_pass(&mut window, &ctx, area, egui::RawInput::default());
                let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());
                let rect = dialog_rect(&shapes, title, window.theme.panel_background());
                assert!(
                    rect.center().distance(area.center()) < 2.0,
                    "{title}: {rect:?}"
                );
            }
        }
    }

    fn dialog_rect(
        shapes: &[egui::epaint::ClippedShape],
        title: &str,
        fill: egui::Color32,
    ) -> egui::Rect {
        fn heading(shape: &egui::Shape, title: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == title => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| heading(shape, title)),
                _ => None,
            }
        }
        let center = shapes
            .iter()
            .find_map(|shape| heading(&shape.shape, title))
            .expect("dialog heading is painted");
        fn frames(
            shape: &egui::Shape,
            center: egui::Pos2,
            fill: egui::Color32,
            result: &mut Vec<egui::Rect>,
        ) {
            match shape {
                egui::Shape::Rect(rect)
                    if rect.fill == fill
                        && rect.rect.contains(center)
                        && rect.rect.height() > 80.0 =>
                {
                    result.push(rect.rect)
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        frames(shape, center, fill, result);
                    }
                }
                _ => {}
            }
        }
        let mut rects = Vec::new();
        for shape in shapes {
            frames(&shape.shape, center, fill, &mut rects);
        }
        rects
            .into_iter()
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .expect("dialog frame is painted")
    }

    fn mouse(
        at: egui::Pos2,
        button: egui::PointerButton,
        modifiers: egui::Modifiers,
    ) -> Vec<egui::Event> {
        let mut events = vec![egui::Event::PointerMoved(at)];
        events.extend(
            [true, false]
                .into_iter()
                .map(|pressed| egui::Event::PointerButton {
                    pos: at,
                    button,
                    pressed,
                    modifiers,
                }),
        );
        events
    }

    fn header_point(
        window: &Window,
        ctx: &egui::Context,
        area: egui::Rect,
        pane: usize,
    ) -> egui::Pos2 {
        let metrics = CellMetrics::measure(ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let work = tab_bar::geometry(window, area, &metrics).consoles;
        let rect = window.active().root.place(work, metrics.width()).parts[pane];
        header::set_the_header_aside(rect, &metrics, window.headers)
            .0
            .unwrap()
            .center()
    }

    #[test]
    fn menu_opens_at_pointer_and_stays_inside_viewport() {
        let (mut window, ctx) = warmed();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 900.0));
        one_frame_in(&mut window, &ctx, area, vec![]);
        let at = header_point(&window, &ctx, area, 1);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            mouse(at, egui::PointerButton::Secondary, egui::Modifiers::NONE),
        );
        one_frame_in(&mut window, &ctx, area, vec![]);
        let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());
        let rect = dialog_rect(&shapes, "Console actions", window.theme.panel_background());
        assert!(area.contains_rect(rect), "menu escapes viewport: {rect:?}");
        assert!(
            rect.distance_to_pos(at) < 3.0,
            "menu detached from click: {rect:?}, {at:?}"
        );
    }

    #[test]
    fn dismissing_menu_with_escape_or_outside_click_never_reaches_program() {
        for outside in [false, true] {
            let received = term_core::received::Received::new(if outside {
                "menu-outside"
            } else {
                "menu-escape"
            });
            let ctx = egui::Context::default();
            let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            received.wait_until_ready(Duration::from_secs(5));
            let area = area_of(1280.0);
            one_frame_in(&mut window, &ctx, area, vec![]);
            let at = header_point(&window, &ctx, area, 1);
            one_frame_in(
                &mut window,
                &ctx,
                area,
                mouse(at, egui::PointerButton::Secondary, egui::Modifiers::NONE),
            );
            one_frame_in(&mut window, &ctx, area, vec![]);
            let open = ui_pass(&mut window, &ctx, area, egui::RawInput::default());
            assert!(
                the_text_queued_up(&open, "Console actions"),
                "menu did not open"
            );
            let events = if outside {
                mouse(
                    area.left_bottom() + egui::vec2(20.0, -20.0),
                    egui::PointerButton::Primary,
                    egui::Modifiers::NONE,
                )
            } else {
                vec![key_press(egui::Key::Escape, egui::Modifiers::NONE)]
            };
            one_frame_in(&mut window, &ctx, area, events);
            one_frame_in(&mut window, &ctx, area, vec![]);
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Text("sentinel".into())],
            );
            assert_eq!(received.wait_for(8, Duration::from_secs(5)), b"sentinel");
        }
    }

    #[test]
    fn opening_menu_preserves_unrelated_console_colors() {
        let (mut window, ctx) = warmed();
        window.active_mut().panes_mut()[0].label = Some("Unaffected console".into());
        let area = area_of(1280.0);
        let colors = |shapes: &[egui::epaint::ClippedShape]| {
            shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Unaffected console" => Some(
                        text.galley
                            .rows
                            .iter()
                            .flat_map(|row| {
                                row.row
                                    .visuals
                                    .mesh
                                    .vertices
                                    .iter()
                                    .map(|vertex| vertex.color)
                            })
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .expect("console label is painted")
        };
        let baseline = colors(&ui_pass(&mut window, &ctx, area, egui::RawInput::default()));
        assert!(!baseline.is_empty());
        let at = header_point(&window, &ctx, area, 1);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            mouse(at, egui::PointerButton::Secondary, egui::Modifiers::NONE),
        );
        one_frame_in(&mut window, &ctx, area, vec![]);
        let open = ui_pass(&mut window, &ctx, area, egui::RawInput::default());
        assert!(the_text_queued_up(&open, "Console actions"));
        assert_eq!(colors(&open), baseline, "opening a menu dimmed the console");
    }

    fn console_point(
        window: &Window,
        ctx: &egui::Context,
        area: egui::Rect,
        index: usize,
    ) -> egui::Pos2 {
        let metrics = CellMetrics::measure(ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let work = tab_bar::geometry(window, area, &metrics).consoles;
        let rect = pane_view::view_tiling(window, work, &metrics).parts[index];
        window.active().panes()[index]
            .chrome_within(rect, &metrics, SystemTime::now(), window.headers)
            .console()
            .center()
    }

    fn text_center(shapes: &[egui::epaint::ClippedShape], label: &str) -> egui::Pos2 {
        fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
                _ => None,
            }
        }
        shapes
            .iter()
            .find_map(|shape| find(&shape.shape, label))
            .expect("menu item is painted")
    }

    fn menu_at_console(
        window: &mut Window,
        ctx: &egui::Context,
        area: egui::Rect,
        index: usize,
    ) -> Vec<egui::epaint::ClippedShape> {
        let at = console_point(window, ctx, area, index);
        one_frame_in(
            window,
            ctx,
            area,
            mouse(at, egui::PointerButton::Secondary, egui::Modifiers::NONE),
        );
        one_frame_in(window, ctx, area, vec![]);
        ui_pass(window, ctx, area, egui::RawInput::default())
    }

    #[test]
    fn console_menu_copy_preserves_selection_and_copies_it() {
        let (mut window, ctx) = warmed();
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, vec![]);
        one_frame_in(&mut window, &ctx, area, vec![]);
        let pane = &mut window.active_mut().panes_mut()[0];
        pane.screen.advance(b"selected text");
        pane.screen.start_selection(Cell {
            row: 0,
            col: 0,
            half: Half::Left,
        });
        pane.screen.extend_selection(Cell {
            row: 0,
            col: 7,
            half: Half::Right,
        });
        let selected = pane.screen.selected_text().expect("fixture selection");
        let shapes = menu_at_console(&mut window, &ctx, area, 0);
        assert_eq!(
            window.active().panes()[0].screen.selected_text().as_deref(),
            Some(selected.as_str())
        );
        let at = text_center(&shapes, "Copy");
        one_frame_in(&mut window, &ctx, area, vec![egui::Event::PointerMoved(at)]);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let mut frame = eframe::Frame::_new_kittest();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area),
                events: vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| eframe::App::ui(&mut window, ui, &mut frame),
        );
        assert!(
        output.platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::CopyText(text) if text == &selected)
        ),
        "Copy did not publish the selected text"
    );
        output.drop_without_applying_deltas();
        assert_eq!(
            window.active().panes()[0].screen.selected_text().as_deref(),
            Some(selected.as_str())
        );
    }

    #[test]
    fn copy_without_selection_does_not_write_clipboard() {
        let (mut window, ctx) = warmed();
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, vec![]);
        let shapes = menu_at_console(&mut window, &ctx, area, 0);
        let at = text_center(&shapes, "Copy");
        one_frame_in(&mut window, &ctx, area, vec![egui::Event::PointerMoved(at)]);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let mut frame = eframe::Frame::_new_kittest();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area),
                events: vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| eframe::App::ui(&mut window, ui, &mut frame),
        );
        assert!(
            !output
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyText(_))),
            "disabled Copy wrote the clipboard"
        );
        output.drop_without_applying_deltas();
        let shown = ui_pass(&mut window, &ctx, area, egui::RawInput::default());
        assert!(
            the_text_queued_up(&shown, "Console actions"),
            "disabled Copy closed the menu"
        );
    }

    fn choose_menu_item(
        window: &mut Window,
        ctx: &egui::Context,
        area: egui::Rect,
        shapes: &[egui::epaint::ClippedShape],
        label: &str,
    ) {
        let at = text_center(shapes, label);
        one_frame_in(window, ctx, area, vec![egui::Event::PointerMoved(at)]);
        for pressed in [true, false] {
            one_frame_in(
                window,
                ctx,
                area,
                vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }

    #[test]
    fn menu_paste_reply_targets_original_console_after_focus_changes() {
        let first = term_core::received::Received::new("menu-paste-first");
        let second = term_core::received::Received::new("menu-paste-second");
        let mut window = window_with(
            &first.script("stty raw -echo"),
            &second.script("stty raw -echo"),
        );
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let (service, harness) = crate::clipboard::Service::testing();
        window.clipboard = service;
        first.wait_until_ready(Duration::from_secs(5));
        second.wait_until_ready(Duration::from_secs(5));
        let ctx = egui::Context::default();
        let area = area_of(1280.0);
        for _ in 0..2 {
            one_frame_in(&mut window, &ctx, area, vec![]);
        }
        let target = window.active().panes()[1].name;
        let shapes = menu_at_console(&mut window, &ctx, area, 1);
        choose_menu_item(&mut window, &ctx, area, &shapes, "Paste");
        assert_eq!(
            harness.requested(),
            Some((target, crate::clipboard::Source::Clipboard))
        );
        one_frame_in(&mut window, &ctx, area, vec![]);
        let first_point = console_point(&window, &ctx, area, 0);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            mouse(
                first_point,
                egui::PointerButton::Primary,
                egui::Modifiers::NONE,
            ),
        );
        assert_ne!(window.active().focus, target);
        harness.reply(target, Ok("original".into()));
        one_frame_in(&mut window, &ctx, area, vec![]);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("sentinel".into())],
        );
        assert_eq!(second.wait_for(8, Duration::from_secs(5)), b"original");
        assert_eq!(first.wait_for(8, Duration::from_secs(5)), b"sentinel");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn dragging_console_selection_publishes_primary() {
        let (mut window, ctx) = warmed();
        let (service, harness) = crate::clipboard::Service::testing();
        window.clipboard = service;
        let area = area_of(1280.0);
        for _ in 0..2 {
            one_frame_in(&mut window, &ctx, area, vec![]);
        }
        window.active_mut().panes_mut()[0]
            .screen
            .advance(b"selected text");
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let work = tab_bar::geometry(&window, area, &metrics).consoles;
        let rect = pane_view::view_tiling(&window, work, &metrics).parts[0];
        let console = window.active().panes()[0]
            .chrome_within(rect, &metrics, SystemTime::now(), window.headers)
            .console();
        let start = console.left_top() + egui::vec2(metrics.width() * 0.2, metrics.height() * 0.5);
        let end = start + egui::vec2(metrics.width() * 7.0, 0.0);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(end)],
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let selected = window.active().panes()[0]
            .screen
            .selected_text()
            .expect("drag selected text");
        assert!(!selected.is_empty());
        assert_eq!(harness.primary().as_deref(), Some(selected.as_str()));
    }

    #[test]
    fn menu_fits_short_viewport() {
        let (mut window, ctx) = warmed();
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, vec![]);
        let shapes = menu_at_console(&mut window, &ctx, area, 1);
        let rect = dialog_rect(&shapes, "Console actions", window.theme.panel_background());
        assert!(
            area.contains_rect(rect),
            "menu escapes a short viewport: {rect:?}"
        );
    }

    #[test]
    fn delayed_paste_to_closed_console_is_discarded() {
        let received = term_core::received::Received::new("menu-paste-closed");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let (service, harness) = crate::clipboard::Service::testing();
        window.clipboard = service;
        received.wait_until_ready(Duration::from_secs(5));
        let ctx = egui::Context::default();
        let area = area_of(1280.0);
        for _ in 0..2 {
            one_frame_in(&mut window, &ctx, area, vec![]);
        }
        let target = window.active().panes()[1].name;
        let shapes = menu_at_console(&mut window, &ctx, area, 1);
        choose_menu_item(&mut window, &ctx, area, &shapes, "Paste");
        assert_eq!(
            harness.requested(),
            Some((target, crate::clipboard::Source::Clipboard))
        );
        window.close_a_pane_called(target, &ctx).unwrap();
        harness.reply(target, Ok("wrong console".into()));
        one_frame_in(&mut window, &ctx, area, vec![]);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("sentinel".into())],
        );
        assert_eq!(received.wait_for(8, Duration::from_secs(5)), b"sentinel");
    }

    #[test]
    fn reporting_program_keeps_context_buttons_until_shift_overrides_them() {
        let buttons = if cfg!(target_os = "linux") {
            vec![egui::PointerButton::Secondary, egui::PointerButton::Middle]
        } else {
            vec![egui::PointerButton::Secondary]
        };
        for button in buttons {
            let received = term_core::received::Received::new("context-program-mouse");
            let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let (service, harness) = crate::clipboard::Service::testing();
            window.clipboard = service;
            received.wait_until_ready(Duration::from_secs(5));
            let ctx = egui::Context::default();
            let area = area_of(1280.0);
            for _ in 0..2 {
                one_frame_in(&mut window, &ctx, area, vec![]);
            }
            window.active_mut().panes_mut()[0]
                .screen
                .advance(b"\x1b[?1000h\x1b[?1006h");
            let at = console_point(&window, &ctx, area, 0);
            let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
            let work = tab_bar::geometry(&window, area, &metrics).consoles;
            let rect = pane_view::view_tiling(&window, work, &metrics).parts[0];
            let pane = window.active().panes()[0];
            let console = pane
                .chrome_within(rect, &metrics, SystemTime::now(), window.headers)
                .console();
            let cell = pane.cell_under(console, at, &metrics).unwrap();
            let position = mouse::CellPos::from_zero(cell.row, cell.col);
            let reporting = pane.screen.mouse_reporting();
            let mut expected = mouse::encode(
                mouse::MouseReport::Press(button),
                position,
                reporting,
                egui::Modifiers::NONE,
            )
            .unwrap();
            expected.extend(
                mouse::encode(
                    mouse::MouseReport::Release(button),
                    position,
                    reporting,
                    egui::Modifiers::NONE,
                )
                .unwrap(),
            );
            one_frame_in(
                &mut window,
                &ctx,
                area,
                mouse(at, button, egui::Modifiers::NONE),
            );
            assert!(!window.pane_view.is_modal());
            assert_eq!(harness.requested(), None);
            one_frame_in(
                &mut window,
                &ctx,
                area,
                mouse(at, button, egui::Modifiers::SHIFT),
            );
            if button == egui::PointerButton::Secondary {
                one_frame_in(&mut window, &ctx, area, vec![]);
                let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());
                assert!(the_text_queued_up(&shapes, "Console actions"));
                one_frame_in(
                    &mut window,
                    &ctx,
                    area,
                    vec![key_press(egui::Key::Escape, egui::Modifiers::NONE)],
                );
            } else {
                let target = window.active().focus;
                assert_eq!(
                    harness.requested(),
                    Some((target, crate::clipboard::Source::Primary))
                );
                harness.reply(target, Ok("PRIMARY".into()));
                one_frame_in(&mut window, &ctx, area, vec![]);
                expected.extend_from_slice(b"PRIMARY");
            }
            one_frame_in(&mut window, &ctx, area, vec![]);
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Text("sentinel".into())],
            );
            expected.extend_from_slice(b"sentinel");
            assert_eq!(
                received.wait_for(expected.len(), Duration::from_secs(5)),
                expected
            );
        }
    }

    #[test]
    fn menu_paste_uses_current_bracketed_mode_and_holds_overlong_lines() {
        for bracketed in [false, true] {
            let Some(ceiling) = keyboard::the_longest_line_a_reader_keeps() else {
                return;
            };
            let received = term_core::received::Received::new("context-paste-protection");
            let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let (service, harness) = crate::clipboard::Service::testing();
            window.clipboard = service;
            received.wait_until_ready(Duration::from_secs(5));
            let ctx = egui::Context::default();
            let area = area_of(1280.0);
            for _ in 0..2 {
                one_frame_in(&mut window, &ctx, area, vec![]);
            }
            let target = window.active().focus;
            let shapes = menu_at_console(&mut window, &ctx, area, 0);
            choose_menu_item(&mut window, &ctx, area, &shapes, "Paste");
            assert_eq!(
                harness.requested(),
                Some((target, crate::clipboard::Source::Clipboard))
            );
            if bracketed {
                window
                    .pane_called(target)
                    .unwrap()
                    .screen
                    .advance(b"\x1b[?2004h");
            }
            let text = "x".repeat(ceiling + 1);
            harness.reply(target, Ok(text.clone()));
            one_frame_in(&mut window, &ctx, area, vec![]);
            assert_eq!(
                window.pane_called(target).unwrap().held.is_some(),
                !bracketed
            );
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Text("sentinel".into())],
            );
            let mut expected = Vec::new();
            if bracketed {
                expected.extend_from_slice(b"\x1b[200~");
                expected.extend_from_slice(text.as_bytes());
                expected.extend_from_slice(b"\x1b[201~");
            }
            expected.extend_from_slice(b"sentinel");
            assert_eq!(
                received.wait_for(expected.len(), Duration::from_secs(5)),
                expected
            );
        }
    }
}
