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
        let _output = isolate_output(&mut window);
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

    crate::windows_unix_omission!(
        terminal_title_override_and_return_action_own_input_and_preserve_manual_names_only,
        "unix-only: requires a POSIX raw-mode child byte recorder for terminal input behavior"
    );

    #[cfg(unix)]
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
    /// Every text the pass queued: what it says, where, and the colour it is drawn in.
    fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect, egui::Color32)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect, egui::Color32)>) {
            match shape {
                egui::Shape::Text(t) => {
                    let color = t
                        .galley
                        .job
                        .sections
                        .first()
                        .map(|s| s.format.color)
                        .filter(|c| *c != egui::Color32::PLACEHOLDER)
                        .unwrap_or(t.fallback_color);
                    out.push((
                        t.galley.text().to_owned(),
                        egui::Rect::from_min_size(t.pos, t.galley.size()),
                        color,
                    ));
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        shapes.iter().for_each(|s| walk(&s.shape, &mut out));
        out
    }

    /// Every opaque filled rectangle the pass queued: a frameless hit area is transparent and the
    /// dialog's shadow is a translucent rectangle offset under it, and neither is a surface.
    fn fills(shapes: &[egui::epaint::ClippedShape]) -> Vec<(egui::Rect, egui::Color32)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Color32)>) {
            match shape {
                egui::Shape::Rect(r) if r.fill.a() == 255 => out.push((r.rect, r.fill)),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        shapes.iter().for_each(|s| walk(&s.shape, &mut out));
        out
    }

    fn rect_of(texts: &[(String, egui::Rect, egui::Color32)], says: &str) -> egui::Rect {
        texts
            .iter()
            .find(|(s, ..)| s == says)
            .unwrap_or_else(|| panic!("{says} is not painted"))
            .1
    }

    /// The smallest filled rectangle under a point: the control a label sits on, or the surface.
    fn fill_under(
        fills: &[(egui::Rect, egui::Color32)],
        at: egui::Pos2,
    ) -> (egui::Rect, egui::Color32) {
        *fills
            .iter()
            .filter(|(r, _)| r.contains(at))
            .min_by(|a, b| a.0.area().total_cmp(&b.0.area()))
            .expect("something is painted under the point")
    }

    /// The close button: the only text on the heading's row, to its right.
    fn close_of(texts: &[(String, egui::Rect, egui::Color32)], heading: &str) -> egui::Pos2 {
        let title = rect_of(texts, heading);
        texts
            .iter()
            .find(|(_, r, _)| r.min.x > title.max.x && title.y_range().contains(r.center().y))
            .expect("a close button beside the title")
            .1
            .center()
    }

    fn open_console_rename(draft: &str) -> (Window, egui::Context, usize, usize) {
        let ctx = egui::Context::default();
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        let mut window = window_with("sleep 30", "sleep 30");
        let tab = window.active().name;
        let pane = window.active().focus;
        window.pane_called(pane).unwrap().label = Some("Before".into());
        window.pane_view.begin_rename(tab, pane, draft.into());
        draw(&mut window, &ctx, vec![]);
        draw(&mut window, &ctx, vec![]);
        (window, ctx, tab, pane)
    }

    #[test]
    fn the_rename_dialog_has_settings_header_and_footer() {
        for console in [true, false] {
            let ctx = egui::Context::default();
            ctx.all_styles_mut(|s| s.animation_time = 0.0);
            let mut window = window_with("sleep 30", "sleep 30");
            let tab = window.active().name;
            let pane = window.active().focus;
            if console {
                window.pane_view.begin_rename(tab, pane, "Draft".into());
            } else {
                window.pane_view.begin_tab_rename(tab, "Draft".into());
            }
            let heading = if console {
                "Rename console"
            } else {
                "Rename tab"
            };
            // egui lays a new window out invisibly on its first pass.
            draw(&mut window, &ctx, vec![]);
            let mut dialogs = Vec::new();
            let mut shapes = Vec::new();
            for _ in 0..3 {
                shapes = draw(&mut window, &ctx, vec![]);
                let texts = texts(&shapes);
                let fills = fills(&shapes);
                dialogs.push(fill_under(&fills, rect_of(&texts, heading).center()).0);
            }
            assert!(
                dialogs.windows(2).all(|w| w[0] == w[1]),
                "{heading}: the dialog changed size between passes: {dialogs:?}"
            );
            // 380 points of content and 20 of margin on each side, whatever the right-aligned
            // header and footer would otherwise settle on.
            assert!(
                (415.0..=425.0).contains(&dialogs[0].width()),
                "{heading}: the dialog is not 380 points wide: {:?}",
                dialogs[0]
            );
            let texts = texts(&shapes);
            let fills = fills(&shapes);
            let roles = crate::ui::style::roles(&ctx, &window.theme);
            let dialog = dialogs[0];
            assert_eq!(
                fill_under(&fills, rect_of(&texts, heading).center()).1,
                roles.raised
            );
            close_of(&texts, heading);

            let save = rect_of(&texts, "Save");
            let (save_rect, save_fill) = fill_under(&fills, save.center());
            assert_eq!(
                save_fill, roles.accent,
                "{heading}: Save is not the primary"
            );
            assert!(save_rect.height() >= 36.0, "{heading}: Save {save_rect:?}");
            let save_ink = texts.iter().find(|(s, ..)| s == "Save").unwrap().2;
            assert_eq!(save_ink, roles.on_accent, "{heading}: Save's ink");
            let cancel = rect_of(&texts, "Cancel");
            assert_eq!(
                fill_under(&fills, cancel.center()).0,
                dialog,
                "{heading}: Cancel has a fill of its own"
            );
            assert!(
                cancel.max.x < save.min.x,
                "{heading}: Cancel is not left of Save"
            );
            assert!(
                (dialog.max.x - save_rect.max.x) < 30.0,
                "{heading}: Save is not right-most: {save_rect:?} in {dialog:?}"
            );
            let field = fill_under(&fills, rect_of(&texts, "Draft").center()).0;
            assert!(field.height() >= 36.0, "{heading}: field {field:?}");
            if console {
                let follow = rect_of(&texts, "Use terminal title");
                assert_eq!(fill_under(&fills, follow.center()).0, dialog);
                assert!(follow.max.x < cancel.min.x);
                assert!(
                    follow.min.x - dialog.min.x < 40.0,
                    "Use terminal title is not at the footer's left edge: {follow:?} in {dialog:?}"
                );
            } else {
                assert!(!the_text_queued_up(&shapes, "Use terminal title"));
            }
        }
    }

    #[test]
    fn a_narrow_window_keeps_the_footer_actions_apart_and_inside_the_dialog() {
        // 320 leaves 240 points of content and the three actions fit on one row; 240 leaves 160
        // and the quiet one takes its own row.
        for (width, one_row) in [(320.0, true), (240.0, false)] {
            let ctx = egui::Context::default();
            // The fonts the app ships, so the one-row threshold is measured with the face people see.
            crate::fonts::install_bundled(&ctx);
            ctx.all_styles_mut(|s| s.animation_time = 0.0);
            let mut window = window_with("sleep 30", "sleep 30");
            let tab = window.active().name;
            let pane = window.active().focus;
            window.pane_view.begin_rename(tab, pane, "Draft".into());
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 700.0));
            let pass = |window: &mut Window| ui_pass(window, &ctx, area, egui::RawInput::default());
            pass(&mut window);
            pass(&mut window);
            let shapes = pass(&mut window);
            let texts = texts(&shapes);
            let fills = fills(&shapes);
            let field = fill_under(&fills, rect_of(&texts, "Draft").center()).0;
            let dialog = fill_under(&fills, field.left_top() - egui::vec2(4.0, 4.0)).0;
            assert!(
                area.contains_rect(dialog),
                "{width}: {dialog:?} leaves the window"
            );
            // The content (the window less 80) and 20 of margin on each side: a footer that does
            // not fit must wrap, not widen the dialog.
            assert!(
                dialog.width() <= width - 80.0 + 45.0,
                "{width}: the footer widened the dialog to {dialog:?}"
            );
            let actions = ["Use terminal title", "Cancel", "Save"].map(|label| {
                let rect = rect_of(&texts, label);
                assert!(
                    dialog.contains_rect(rect),
                    "{width}: {label} {rect:?} outside {dialog:?}"
                );
                rect
            });
            for (i, a) in actions.iter().enumerate() {
                for b in &actions[i + 1..] {
                    assert!(!a.intersects(*b), "{width}: {a:?} overlaps {b:?}");
                }
            }
            assert!(
                actions[1].max.x < actions[2].min.x,
                "{width}: Cancel is not left of Save"
            );
            assert_eq!(
                (actions[0].center().y - actions[2].center().y).abs() < 1.0,
                one_row,
                "{width}: Use terminal title {:?}, Save {:?}",
                actions[0],
                actions[2]
            );
            // The title is cut short before the close button rather than run under it.
            let close = shapes
                .iter()
                .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
                .filter(|(glyph, _, square)| {
                    glyph == egui_phosphor::regular::X && dialog.contains(square.center())
                })
                .map(|(.., square)| fill_under(&fills, square.center()).0)
                .next()
                .expect("the dialog's close box");
            let title = texts
                .iter()
                .filter(|(s, r, _)| s.starts_with("Rename") && dialog.contains_rect(*r))
                .map(|(_, r, _)| *r)
                .next()
                .expect("the dialog's title");
            assert!(
                title.max.x <= close.min.x,
                "{width}: title {title:?} runs under the close box {close:?}"
            );
        }
    }

    #[test]
    fn the_terminal_title_wins_over_an_enter_in_the_same_pass() {
        let (mut window, ctx, _, pane) = open_console_rename("Typed");
        let shapes = draw(&mut window, &ctx, vec![]);
        let mut events = click_at(text_center(&shapes, "Use terminal title"));
        events.push(key(egui::Key::Enter));
        draw(&mut window, &ctx, events);
        draw(&mut window, &ctx, vec![]);
        assert!(!window.pane_view.is_renaming());
        assert_eq!(window.pane_called(pane).unwrap().label, None);
    }

    #[test]
    fn the_close_cross_is_centred_in_its_box_and_on_the_titles_line() {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        let mut window = window_with("sleep 30", "sleep 30");
        let tab = window.active().name;
        let pane = window.active().focus;
        window.pane_view.begin_rename(tab, pane, "Draft".into());
        draw(&mut window, &ctx, vec![]);
        draw(&mut window, &ctx, vec![]);
        let shapes = draw(&mut window, &ctx, vec![]);
        let texts = texts(&shapes);
        let title = rect_of(&texts, "Rename console");
        let cross = shapes
            .iter()
            .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
            .filter(|(glyph, ..)| glyph == egui_phosphor::regular::X)
            .map(|(.., square)| square)
            // The consoles' own headers carry crosses too: the dialog's is right of its title.
            .filter(|square| square.min.x > title.max.x)
            .min_by(|a, b| {
                (a.center().y - title.center().y)
                    .abs()
                    .total_cmp(&(b.center().y - title.center().y).abs())
            })
            .expect("the close cross is painted");
        let close = fill_under(&fills(&shapes), cross.center()).0;
        assert!(
            close.width() < 60.0,
            "the cross has no box of its own: {close:?}"
        );
        assert!(
            (close.center() - cross.center()).length() < 1.0,
            "cross {cross:?} is off its box's centre {close:?}"
        );
        assert!(
            (close.center().y - title.center().y).abs() < 1.5,
            "box {close:?} and title {title:?} are not on one line"
        );
        // The kit's floating heading, not Settings' 25-point one.
        assert!(title.height() < 28.0, "the title is {title:?}");
    }

    #[test]
    fn the_close_button_cancels_the_rename() {
        let (mut window, ctx, _, pane) = open_console_rename("Changed");
        let shapes = draw(&mut window, &ctx, vec![]);
        let at = close_of(&texts(&shapes), "Rename console");
        draw(&mut window, &ctx, click_at(at));
        draw(&mut window, &ctx, vec![]);
        assert!(!window.pane_view.is_renaming());
        assert_eq!(
            window.pane_called(pane).unwrap().label.as_deref(),
            Some("Before")
        );
        assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
    }

    #[test]
    fn an_invalid_name_disables_save_but_not_the_terminal_title() {
        let invalid = "x".repeat(129);
        let (mut window, ctx, _, pane) = open_console_rename(&invalid);
        let shapes = draw(&mut window, &ctx, vec![]);
        let texts = texts(&shapes);
        let roles = crate::ui::style::roles(&ctx, &window.theme);
        let save = rect_of(&texts, "Save");
        assert_ne!(fill_under(&fills(&shapes), save.center()).1, roles.accent);
        draw(&mut window, &ctx, click_at(save.center()));
        draw(&mut window, &ctx, vec![key(egui::Key::Enter)]);
        assert!(window.pane_view.is_renaming(), "an invalid name was saved");
        assert_eq!(
            window.pane_called(pane).unwrap().label.as_deref(),
            Some("Before")
        );
        let shapes = draw(&mut window, &ctx, vec![]);
        draw(
            &mut window,
            &ctx,
            click_at(text_center(&shapes, "Use terminal title")),
        );
        draw(&mut window, &ctx, vec![]);
        assert!(!window.pane_view.is_renaming());
        assert_eq!(window.pane_called(pane).unwrap().label, None);
    }

    #[test]
    fn the_help_and_the_validation_use_settings_inks() {
        let (mut window, ctx, ..) = open_console_rename(&"x".repeat(129));
        let shapes = draw(&mut window, &ctx, vec![]);
        let texts = texts(&shapes);
        let (note, error) = crate::settings::editor::view::dialog::tests::inks(&ctx, &window.theme);
        let ink = |says: &str| texts.iter().find(|(s, ..)| s.starts_with(says)).unwrap().2;
        assert_eq!(ink("A manual name overrides"), note);
        assert_eq!(ink("Use at most 128 characters"), error);
        assert_ne!(note, error);
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
