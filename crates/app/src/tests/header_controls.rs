//! Exercise header actions through the application's real input and painting passes.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::header::controls::{Action, Controls, Hit};

    fn parts(window: &Window, area: egui::Rect, metrics: &CellMetrics) -> Vec<egui::Rect> {
        window.active().root.place(area, metrics.width()).parts
    }

    fn targets(window: &Window, area: egui::Rect, which: usize, metrics: &CellMetrics) -> Controls {
        let rects = parts(window, area, metrics);
        let strip = header::set_the_header_aside(rects[which], metrics, window.headers)
            .0
            .expect("the ordinary session paints a header");
        Controls::within(
            strip,
            metrics,
            header::button_within(strip, metrics, header::Button::in_a_tab_of(&rects))
                .map(|button| button.rect()),
        )
    }

    fn target(
        window: &Window,
        area: egui::Rect,
        which: usize,
        metrics: &CellMetrics,
        hit: Hit,
    ) -> egui::Pos2 {
        targets(window, area, which, metrics)
            .targets()
            .find_map(|(candidate, rect)| (candidate == hit).then_some(rect.center()))
            .expect("this presentation must expose the requested control")
    }

    fn warmed(window: &mut Window, ctx: &egui::Context, area: egui::Rect) -> CellMetrics {
        one_frame_in(window, ctx, area, Vec::new());
        CellMetrics::measure(ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE))
    }

    fn right_click(at: egui::Pos2) -> Vec<egui::Event> {
        [true, false]
            .into_iter()
            .map(|pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            })
            .collect()
    }

    #[test]
    fn splitting_an_unfocused_header_creates_the_newborn_beside_its_owner() {
        for action in [Action::SplitRight, Action::SplitDown] {
            let ctx = egui::Context::default();
            let mut window = window_with("sleep 30", "sleep 30");
            let area = area_of(1600.0);
            let metrics = warmed(&mut window, &ctx, area);
            let old_names = names_in_order(&window);
            let before = parts(&window, area, &metrics);
            assert_eq!(window.active().focus, old_names[0]);
            let at = target(&window, area, 1, &metrics, Hit::Action(action));

            one_frame_in(&mut window, &ctx, area, click_at(at));

            let names = names_in_order(&window);
            assert_eq!(
                names.len(),
                3,
                "the visible split control created no console"
            );
            assert_eq!(&names[..2], &old_names, "a different pane was split");
            assert_eq!(
                window.active().focus,
                names[2],
                "the newborn did not receive the keyboard"
            );
            let after = parts(&window, area, &metrics);
            match action {
                Action::SplitRight => {
                    // A third sibling consumes another divider; preserve its weight, not its
                    // pixel width before that divider existed.
                    assert_eq!(weights_of_the_root(&window), vec![0.5, 0.25, 0.25]);
                    assert_eq!(after[0].left(), before[0].left());
                    assert_eq!(after[0].top(), before[0].top());
                    assert_eq!(after[0].bottom(), before[0].bottom());
                    assert!(after[1].right() < after[2].left());
                    assert_eq!(after[1].top(), after[2].top());
                    assert_eq!(after[1].bottom(), after[2].bottom());
                }
                Action::SplitDown => {
                    assert_eq!(
                        after[0], before[0],
                        "the unrelated sibling changed geometry"
                    );
                    assert!(after[1].bottom() < after[2].top());
                    assert_eq!(after[1].left(), after[2].left());
                    assert_eq!(after[1].right(), after[2].right());
                }
                Action::TogglePanel => unreachable!(),
            }
        }
    }

    #[test]
    fn an_unavailable_split_preserves_the_consoles_geometry_and_keyboard_owner() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(480.0);
        let metrics = warmed(&mut window, &ctx, area);
        let before = parts(&window, area, &metrics);
        let names = names_in_order(&window);
        let focus = window.active().focus;
        assert!(!tiling::can_split(
            before[1],
            Orientation::Columns,
            &metrics,
            window.headers
        ));
        let at = target(&window, area, 1, &metrics, Hit::Action(Action::SplitRight));

        one_frame_in(&mut window, &ctx, area, click_at(at));

        assert_eq!(names_in_order(&window), names);
        assert_eq!(parts(&window, area, &metrics), before);
        assert_eq!(
            window.active().focus,
            focus,
            "the disabled control stole the keyboard"
        );
    }

    #[test]
    fn toggling_an_unfocused_panel_restores_its_painted_document_and_width() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        with_the_panel_in_place(&mut window, 1, r#"{"lines":["retained-panel-marker"]}"#);
        window.active_mut().panes_mut()[1].panel_width = 27;
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let focus = window.active().focus;
        let rect = parts(&window, area, &metrics)[1];
        let before = window.active().panes()[1].tile_with_its_panel(
            rect,
            metrics.width(),
            SystemTime::now(),
        );
        let panel_before = before
            .panel
            .expect("a real published document occupies a panel");
        let at = target(&window, area, 1, &metrics, Hit::Action(Action::TogglePanel));

        let hidden = ui_pass(
            &mut window,
            &ctx,
            area,
            egui::RawInput {
                events: click_at(at),
                ..Default::default()
            },
        );
        assert!(!the_text_queued_up(&hidden, "retained-panel-marker"));
        assert!(
            window.active().panes()[1]
                .tile_with_its_panel(rect, metrics.width(), SystemTime::now())
                .panel
                .is_none()
        );
        assert_eq!(window.active().focus, focus);

        let shown = ui_pass(
            &mut window,
            &ctx,
            area,
            egui::RawInput {
                events: click_at(at),
                ..Default::default()
            },
        );
        assert!(
            the_text_queued_up(&shown, "retained-panel-marker"),
            "the document disappeared instead of being hidden"
        );
        let panel_after = window.active().panes()[1]
            .tile_with_its_panel(rect, metrics.width(), SystemTime::now())
            .panel
            .expect("the second press restores the panel");
        assert_eq!(
            panel_after, panel_before,
            "showing the panel forgot its previous width"
        );
        assert_eq!(window.active().focus, focus);
    }

    #[test]
    fn holding_a_toggle_and_moving_over_another_pane_cannot_turn_into_a_pane_drag() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["held-toggle-marker"]}"#);
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let before = parts(&window, area, &metrics);
        let names = names_in_order(&window);
        let at = target(&window, area, 0, &metrics, Hit::Action(Action::TogglePanel));
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let drop_at = before[1].right_center() - egui::vec2(10.0, 0.0);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(drop_at)],
        );
        one_frame_in(&mut window, &ctx, area, release(drop_at).events);

        assert_eq!(
            names_in_order(&window),
            names,
            "the toggle press moved its console on release"
        );
        assert_eq!(parts(&window, area, &metrics), before);
        assert!(
            window.active().panes()[0]
                .tile_with_its_panel(before[0], metrics.width(), SystemTime::now())
                .panel
                .is_none(),
            "the held press toggled the panel more than once"
        );
    }

    #[test]
    fn a_menu_keeps_its_unfocused_owner_when_enter_creates_a_console() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let before = parts(&window, area, &metrics);
        let names = names_in_order(&window);
        let strip = header::set_the_header_aside(before[1], &metrics, window.headers)
            .0
            .unwrap();
        one_frame_in(&mut window, &ctx, area, right_click(strip.center()));
        assert_eq!(
            window.active().focus,
            names[0],
            "opening the menu focused its console"
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                key_press(egui::Key::ArrowDown, egui::Modifiers::NONE),
                key_press(egui::Key::Enter, egui::Modifiers::NONE),
            ],
        );

        let after = parts(&window, area, &metrics);
        assert_eq!(
            after.len(),
            3,
            "choosing Split down in the menu did nothing"
        );
        assert_eq!(
            after[0], before[0],
            "the menu split the focused console instead of its owner"
        );
        assert!(after[1].bottom() < after[2].top());
        assert_eq!(&names_in_order(&window)[..2], &names);
        assert_eq!(window.active().focus, names_in_order(&window)[2]);
    }

    #[test]
    fn compact_menu_navigation_never_reaches_the_program() {
        let received = term_core::received::Received::new("header-menu-keys");
        let ctx = egui::Context::default();
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        received.wait_until_ready(Duration::from_secs(5));
        let area = area_of(220.0);
        let metrics = warmed(&mut window, &ctx, area);
        let at = target(&window, area, 1, &metrics, Hit::Menu);
        for key in [egui::Key::Escape, egui::Key::Enter] {
            let opened = ui_pass(
                &mut window,
                &ctx,
                area,
                egui::RawInput {
                    events: click_at(at),
                    ..Default::default()
                },
            );
            assert!(
                the_text_queued_up(&opened, "Split right (not enough room)"),
                "the compact control painted no menu"
            );
            let dismissed = ui_pass(
                &mut window,
                &ctx,
                area,
                egui::RawInput {
                    events: vec![key_press(key, egui::Modifiers::NONE)],
                    ..Default::default()
                },
            );
            assert!(
                !the_text_queued_up(&dismissed, "Split right (not enough room)"),
                "the menu did not consume its dismissal/selection key"
            );
        }
        assert_eq!(
            window.active().panes().len(),
            2,
            "an unavailable compact split created a console"
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("sentinel".to_owned())],
        );
        assert_eq!(
            received.wait_for(8, Duration::from_secs(5)),
            b"sentinel",
            "menu Escape or Enter leaked into the program before the sentinel"
        );
    }

    #[test]
    fn toggling_the_panel_sends_no_mouse_report_to_a_program_that_requested_clicks() {
        let received = term_core::received::Received::new("header-toggle-input");
        let ctx = egui::Context::default();
        let script =
            received.script("stty raw -echo && printf '\\033[?1000h\\033[?1006hmouse-ready'");
        let mut window = window_with(&script, "sleep 30");
        received.wait_until_ready(Duration::from_secs(5));
        wait_for_the_pane_to_say(&mut window, 0, "mouse-ready");
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["mouse-panel"]}"#);
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let at = target(&window, area, 0, &metrics, Hit::Action(Action::TogglePanel));

        one_frame_in(&mut window, &ctx, area, click_at(at));
        assert!(
            window.active().panes()[0].folded,
            "the control did not hide the panel"
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("sentinel".to_owned())],
        );

        assert_eq!(
            received.wait_for(8, Duration::from_secs(5)),
            b"sentinel",
            "the header press or release reached the program before the sentinel"
        );
    }

    #[test]
    fn modifier_changes_inside_a_menu_still_make_copy_copy_after_dismissal() {
        let received = term_core::received::Received::new("header-menu-modifiers");
        let ctx = egui::Context::default();
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        received.wait_until_ready(Duration::from_secs(5));
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let strip = header::set_the_header_aside(
            parts(&window, area, &metrics)[1],
            &metrics,
            window.headers,
        )
        .0
        .unwrap();
        one_frame_in(&mut window, &ctx, area, right_click(strip.center()));
        let chord = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::ModifiersChanged(chord)],
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![key_press(egui::Key::Escape, chord)],
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Copy, egui::Event::Text("sentinel".to_owned())],
        );
        assert_eq!(
            received.wait_for(8, Duration::from_secs(5)),
            b"sentinel",
            "copying after menu dismissal interrupted the program"
        );
    }

    #[test]
    fn losing_window_focus_while_a_menu_is_open_does_not_leave_shift_stuck() {
        let received = term_core::received::Received::new("header-menu-focus");
        let ctx = egui::Context::default();
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        received.wait_until_ready(Duration::from_secs(5));
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let chord = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::ModifiersChanged(chord)],
        );
        let strip = header::set_the_header_aside(
            parts(&window, area, &metrics)[1],
            &metrics,
            window.headers,
        )
        .0
        .unwrap();
        one_frame_in(&mut window, &ctx, area, right_click(strip.center()));
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::WindowFocused(false)],
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::WindowFocused(true)],
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::Paste("private-clipboard".to_owned()),
                egui::Event::Text("sentinel".to_owned()),
            ],
        );
        assert_eq!(
            received.wait_for(9, Duration::from_secs(5)),
            b"\x16sentinel",
            "returning from another window pasted the clipboard instead of forwarding Ctrl+V"
        );
    }

    #[test]
    fn toolbar_icons_are_painted_and_unavailable_actions_are_dimmed() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(480.0);
        let metrics = warmed(&mut window, &ctx, area);
        let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());
        let dim = window.theme.foreground().gamma_multiply(0.35);
        for (action, color) in [
            (Action::SplitRight, dim),
            (Action::SplitDown, window.theme.foreground()),
            (Action::TogglePanel, dim),
        ] {
            let rect = targets(&window, area, 1, &metrics)
                .targets()
                .find_map(|(hit, rect)| (hit == Hit::Action(action)).then_some(rect))
                .unwrap();
            assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(icon) if rect.contains_rect(icon.rect) && icon.stroke.color == color && icon.stroke.width > 0.0)), "the visible {action:?} icon is missing or has the wrong availability ink");
            if action != Action::TogglePanel {
                assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment {points, stroke} if rect.contains(points[0]) && rect.contains(points[1]) && stroke.color == color && if action == Action::SplitRight { points[0].x == points[1].x && points[0].y != points[1].y } else { points[0].y == points[1].y && points[0].x != points[1].x })), "the painted split direction is wrong");
            }
        }
    }

    #[test]
    fn a_strip_too_small_for_a_menu_button_still_opens_its_actions_by_right_click() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(80.0);
        let metrics = warmed(&mut window, &ctx, area);
        assert_eq!(targets(&window, area, 1, &metrics).targets().count(), 0);
        let strip = header::set_the_header_aside(
            parts(&window, area, &metrics)[1],
            &metrics,
            window.headers,
        )
        .0
        .unwrap();
        let shapes = ui_pass(
            &mut window,
            &ctx,
            area,
            egui::RawInput {
                events: right_click(strip.center()),
                ..Default::default()
            },
        );
        assert!(the_text_queued_up(&shapes, "Split right (not enough room)"));
        assert_eq!(window.active().panes().len(), 2);
    }

    #[test]
    fn hovering_a_menu_action_then_enter_uses_that_same_action() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let before = parts(&window, area, &metrics);
        let strip = header::set_the_header_aside(before[1], &metrics, window.headers)
            .0
            .unwrap();
        one_frame_in(&mut window, &ctx, area, right_click(strip.center()));
        let row = strip.left_bottom() + egui::vec2(20.0, (metrics.height() + 10.0) * 1.5);
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(row),
                key_press(egui::Key::Enter, egui::Modifiers::NONE),
            ],
        );
        let after = parts(&window, area, &metrics);
        assert_eq!(after.len(), 3);
        assert!(
            after[1].bottom() < after[2].top(),
            "Enter did not select the hovered Split down row"
        );
    }

    #[test]
    fn a_warning_only_panel_can_be_hidden_and_restored_without_a_document() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        with_the_panel_in_place(&mut window, 1, "{broken-json");
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let rect = parts(&window, area, &metrics)[1];
        let before = window.active().panes()[1]
            .tile_with_its_panel(rect, metrics.width(), SystemTime::now())
            .panel
            .expect("an unreadable document still paints its warning");
        let at = target(&window, area, 1, &metrics, Hit::Action(Action::TogglePanel));
        one_frame_in(&mut window, &ctx, area, click_at(at));
        assert!(
            window.active().panes()[1]
                .tile_with_its_panel(rect, metrics.width(), SystemTime::now())
                .panel
                .is_none()
        );
        one_frame_in(&mut window, &ctx, area, click_at(at));
        assert_eq!(
            window.active().panes()[1]
                .tile_with_its_panel(rect, metrics.width(), SystemTime::now())
                .panel,
            Some(before)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_hidden_panel_keeps_its_live_producer_and_shows_its_next_publication() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let name = window.active().panes()[1].name;
        attach_a_channel(&mut window, name);
        let mut producer = {
            let panes = window.active().panes();
            let dir = panes[1].panel.as_ref().unwrap();
            panel_control::Producer::of(dir.path(), dir.id()).expect("connect the producer")
        };
        let (commands, receiver) = std::sync::mpsc::channel::<String>();
        let (replies, answers) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            while let Ok(document) = receiver.recv() {
                if replies
                    .send(producer.publish("flow", &document).is_ok())
                    .is_err()
                {
                    break;
                }
            }
        });
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let publish = |window: &mut Window, text: &str| {
            commands.send(format!(r#"{{"lines":["{text}"]}}"#)).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                one_frame_in(window, &ctx, area, Vec::new());
                match answers.try_recv() {
                    Ok(accepted) => {
                        assert!(accepted, "the existing producer lost its connection");
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        assert!(Instant::now() < deadline, "publication was never answered")
                    }
                    Err(error) => panic!("producer stopped: {error}"),
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        publish(&mut window, "before-hide");
        let at = target(&window, area, 1, &metrics, Hit::Action(Action::TogglePanel));
        one_frame_in(&mut window, &ctx, area, click_at(at));
        publish(&mut window, "published-while-hidden");
        let rect = parts(&window, area, &metrics)[1];
        assert!(
            window.active().panes()[1]
                .tile_with_its_panel(rect, metrics.width(), SystemTime::now())
                .panel
                .is_none()
        );
        let shown = ui_pass(
            &mut window,
            &ctx,
            area,
            egui::RawInput {
                events: click_at(at),
                ..Default::default()
            },
        );
        assert!(
            the_text_queued_up(&shown, "published-while-hidden"),
            "showing the panel lost the document published over the same live connection"
        );
        drop(commands);
        worker.join().unwrap();
    }

    #[test]
    fn folding_with_the_header_survives_a_saved_layout_round_trip() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        with_the_panel_in_place(&mut window, 1, r#"{"lines":["layout-panel"]}"#);
        window.active_mut().panes_mut()[1].panel_width = 27;
        let area = area_of(1600.0);
        let metrics = warmed(&mut window, &ctx, area);
        let at = target(&window, area, 1, &metrics, Hit::Action(Action::TogglePanel));
        one_frame_in(&mut window, &ctx, area, click_at(at));
        let back = wrap_around(&window, &ctx);
        assert!(back[0].panes()[1].folded);
        assert!(!back[0].panes()[0].folded);
        assert_eq!(back[0].panes()[1].panel_width, 27);
    }
    #[test]
    fn a_cached_menu_split_rechecks_the_settings_reduced_height() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.settings_editor = settings::editor::Editor::resolved(settings::Entry::Normal, None);
        let large = area_of(1280.0);
        let metrics = warmed(&mut window, &ctx, large);
        let content = window.settings_editor.console_area(large);
        let before = parts(&window, content, &metrics);
        let strip = header::set_the_header_aside(before[1], &metrics, window.headers)
            .0
            .unwrap();
        one_frame_in(&mut window, &ctx, large, right_click(strip.center()));
        assert!(header::menu::pointer_blocked(&window, &ctx));
        let height = (40..400)
            .find(|h| {
                let area =
                    egui::Rect::from_min_size(large.min, egui::vec2(large.width(), *h as f32));
                let full = parts(&window, area, &metrics)[1];
                let reduced =
                    parts(&window, window.settings_editor.console_area(area), &metrics)[1];
                tiling::can_split(full, Orientation::Rows, &metrics, window.headers)
                    && !tiling::can_split(reduced, Orientation::Rows, &metrics, window.headers)
            })
            .unwrap();
        let small = egui::Rect::from_min_size(large.min, egui::vec2(large.width(), height as f32));
        let events = [egui::Key::ArrowDown, egui::Key::Enter]
            .into_iter()
            .map(|key| egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            })
            .collect();
        one_frame_in(&mut window, &ctx, small, events);
        assert_eq!(
            window.active().panes().len(),
            2,
            "cached menu split below the actual minimum after resize; viewport height={height}, actual={}",
            window.settings_editor.console_area(small).height()
        );
    }
}
