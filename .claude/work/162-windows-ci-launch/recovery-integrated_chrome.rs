//! Effects of the integrated pane view through the real application pass.
#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn pane_header_gestures_preserve_unrelated_console_colors() {
        for (action, initially_folded, from_right) in [
            ("hide", false, 62.5),
            ("show", true, 62.5),
            ("close", false, 12.5),
        ] {
            let ctx = egui::Context::default();
            chrome::prepare(&ctx);
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            with_the_panel_in_place(&mut window, 0, r#"{"lines":["Left panel"]}"#);
            window.active_mut().panes_mut()[0].folded = initially_folded;
            with_the_panel_in_place(&mut window, 1, r#"{"lines":["Sibling panel"]}"#);
            window.active_mut().panes_mut()[1].label = Some("Unaffected console".into());
            let area = area_of(1400.0);
            let colors = |shapes: &[egui::epaint::ClippedShape]| {
                shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.text() == "Unaffected console" => Some(
                            t.galley
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
                    .expect("unaffected console is painted")
            };
            let baseline = colors(&frame(&mut window, &ctx, area, vec![]));
            assert!(
                !baseline.is_empty(),
                "unrelated label has no rendered vertices"
            );
            let at = control(&window, &ctx, area, 0, from_right);
            let pressed = frame(
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
            if action != "close" {
                assert!(
                    window.active().panes()[0].folded != initially_folded,
                    "panel {action} did not execute"
                );
                assert_eq!(window.active().panes().len(), 2);
            } else {
                assert_eq!(
                    window.active().panes().len(),
                    1,
                    "pane close did not execute"
                );
                assert_eq!(
                    window.active().panes()[0].label.as_deref(),
                    Some("Unaffected console")
                );
            }
            // Releasing over another panel control must not transfer the gesture.
            let sibling_index = usize::from(action != "close");
            let sibling_control = control(&window, &ctx, area, sibling_index, 62.5);
            let held = frame(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::PointerMoved(sibling_control)],
            );
            let released = frame(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::PointerButton {
                    pos: sibling_control,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            let idle = frame(&mut window, &ctx, area, vec![]);
            assert!(
                !window.active().panes()[sibling_index].folded,
                "{action} gesture clicked through to the sibling panel control"
            );
            assert_eq!(
                window.active().panes().len(),
                if action == "close" { 1 } else { 2 }
            );
            // Closing hands the keys to the sibling, which lights its task label from Ivy Night's
            // secondary text to its text; nothing else about its painting may change.
            let expected = if action == "close" {
                assert!(
                    baseline
                        .iter()
                        .all(|c| *c == egui::Color32::from_rgb(0xa5, 0xaa, 0xa6))
                );
                vec![egui::Color32::from_rgb(0xe4, 0xe8, 0xe2); baseline.len()]
            } else {
                baseline.clone()
            };
            for (phase, shapes) in [
                ("press", pressed),
                ("held", held),
                ("release", released),
                ("idle", idle),
            ] {
                assert_eq!(
                    colors(&shapes),
                    expected,
                    "{action} changes an unrelated console's rendered colors on {phase}"
                );
            }
            assert!(
                window.drag_in_progress.is_none(),
                "header gesture started a console drag"
            );
        }
    }

    /// Same-project headers carry no stripe (the owner removed it, 17 November 2026); focus moves
    /// the green between them (#141): the focused header is the panel tinted with the accent and
    /// outlined in it, project or not, and the other keeps its project's ink on the chrome. Ivy
    /// Night's panel, chrome, accent and line are written out.
    #[test]
    fn focus_moves_the_green_surface_and_outline_between_consoles() {
        let ctx = egui::Context::default();
        chrome::prepare(&ctx);
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let ink = egui::Color32::from_rgb(224, 175, 104);
        for pane in window.active_mut().panes_mut() {
            pane.project = Some(crate::project_view::Badge {
                key: crate::project::Key::Repository("/work/backend".into()),
                name: "Backend".into(),
                details: "/work/backend".into(),
                ink,
                painted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            });
        }
        let area = area_of(1400.0);
        // Ivy Night's panel #171D21 with 18 % of the accent #3CC384, and its chrome #0F1315 with
        // 14 % of the project's ink.
        let panel = egui::Color32::from_rgb(0x1e, 0x3b, 0x33);
        let chrome = egui::Color32::from_rgb(0x2c, 0x29, 0x21);
        let accent = egui::Color32::from_rgb(0x3c, 0xc3, 0x84);
        let line = egui::Color32::from_rgb(0x25, 0x2e, 0x33);
        let stripes = |shapes: &[egui::epaint::ClippedShape]| {
            shapes
                .iter()
                .filter(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect.width() == 3.0 && r.fill == ink))
                .count()
        };
        let header = |shapes: &[egui::epaint::ClippedShape], fill: egui::Color32| {
            shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Rect(r)
                    if r.fill == fill && r.rect.width() > 100.0 && r.rect.height() <= 40.0 =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
        };
        let outline_around = |shapes: &[egui::epaint::ClippedShape], inside: egui::Rect| {
            shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Rect(r)
                    if r.fill == egui::Color32::TRANSPARENT
                        && r.stroke.width == 1.0
                        && r.rect.height() > 100.0
                        && r.rect.contains(inside.center()) =>
                {
                    Some(r.stroke.color)
                }
                _ => None,
            })
        };
        let baseline = frame(&mut window, &ctx, area, vec![]);
        assert_eq!(
            stripes(&baseline),
            0,
            "a project header still paints a stripe"
        );
        let focused = header(&baseline, panel).expect("the focused header is on the panel");
        let other = header(&baseline, chrome).expect("the other header is on the chrome");
        assert!(
            focused.left() < other.left(),
            "focus starts on the first console"
        );
        assert_eq!(outline_around(&baseline, focused), Some(accent));
        assert_eq!(outline_around(&baseline, other), Some(line));
        window.active_mut().focus = 1;
        let moved = frame(&mut window, &ctx, area, vec![]);
        let focused = header(&moved, panel).unwrap();
        assert!(
            focused.left() > other.left() - 1.0,
            "the surface did not follow the focus"
        );
        assert_eq!(
            outline_around(&moved, focused),
            Some(accent),
            "the outline did not follow the focus"
        );
        window.active_mut().focus = 0;
        window.active_mut().panes_mut()[0].project = None;
        let neutral = frame(&mut window, &ctx, area, vec![]);
        let first = window.active().panes()[0].name;
        assert_eq!(window.active().focus, first);
        let green =
            header(&neutral, panel).expect("a focused console without a project turns green too");
        assert_eq!(outline_around(&neutral, green), Some(accent));
    }

    /// With fades allowed, moving focus fades the outline: half-way through the 150 ms neither
    /// console's outline is the plain line or the accent, after it the new one is the accent and
    /// the old one the line, and once settled nothing asks for another repaint. With fades not
    /// allowed — every measured run — the same move lands on the same frame and asks for nothing.
    #[test]
    fn moving_focus_fades_the_green_and_a_measured_run_does_not() {
        let accent = egui::Color32::from_rgb(0x3c, 0xc3, 0x84);
        let line = egui::Color32::from_rgb(0x25, 0x2e, 0x33);
        let area = area_of(1400.0);
        let outlines = |shapes: &[egui::epaint::ClippedShape]| -> Vec<egui::Color32> {
            let mut found: Vec<(f32, egui::Color32)> = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r)
                        if r.fill == egui::Color32::TRANSPARENT
                            && r.stroke.width == 1.0
                            && r.rect.height() > 100.0 =>
                    {
                        Some((r.rect.left(), r.stroke.color))
                    }
                    _ => None,
                })
                .collect();
            found.sort_by(|a, b| a.0.total_cmp(&b.0));
            found.into_iter().map(|(_, c)| c).collect()
        };
        let pass = |window: &mut Window, ctx: &egui::Context, time: f64| {
            let mut frame = eframe::Frame::_new_kittest();
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(area),
                    time: Some(time),
                    ..Default::default()
                },
                |ui| eframe::App::ui(window, ui, &mut frame),
            );
            let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
            let shapes = std::mem::take(&mut output.shapes);
            output.drop_without_applying_deltas();
            (outlines(&shapes), delay)
        };
        for fades in [true, false] {
            let ctx = egui::Context::default();
            chrome::prepare(&ctx);
            crate::pane_view::allow_focus_fades(&ctx, fades);
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            for time in [0.0, 1.0, 1.5] {
                pass(&mut window, &ctx, time);
            }
            assert_eq!(pass(&mut window, &ctx, 2.0).0, vec![accent, line]);
            window.active_mut().focus = 1;
            // The two passes after focus moves ask for another anyway (the focused header's
            // controls appear, D96); the third is where a fade keeps the window awake or not.
            let (moved, _) = pass(&mut window, &ctx, 2.0);
            pass(&mut window, &ctx, 2.016);
            let (_, delay) = pass(&mut window, &ctx, 2.032);
            if fades {
                // egui steps the animation by one predicted frame on the pass the target moves,
                // so neither end has been reached yet — that is what «it did not jump» means.
                assert!(
                    moved[0] != line && moved[1] != accent,
                    "the fade jumped: {moved:?}"
                );
                assert!(
                    delay < std::time::Duration::from_secs(1),
                    "the fade asked for no frame"
                );
                let (half, _) = pass(&mut window, &ctx, 2.075);
                assert!(
                    half.iter().all(|c| *c != accent && *c != line),
                    "half-way, an outline is already at an end: {half:?}"
                );
                let (done, _) = pass(&mut window, &ctx, 2.2);
                assert_eq!(done, vec![line, accent]);
                // The cursor blink (600 ms halves) still wakes the window; a fade asks for the
                // next frame, about 16 ms away, so anything past 100 ms is not the fade.
                let (_, settled) = pass(&mut window, &ctx, 2.3);
                assert!(
                    settled > std::time::Duration::from_millis(100),
                    "a settled fade still wakes: {settled:?}"
                );
            } else {
                assert_eq!(moved, vec![line, accent], "a measured run faded");
                assert!(
                    delay > std::time::Duration::from_millis(100),
                    "a measured run woke for the fade: {delay:?}"
                );
            }
        }
    }

    fn has_text(shapes: &[egui::epaint::ClippedShape], needle: &str) -> bool {
        fn inside(shape: &egui::Shape, needle: &str) -> bool {
            match shape {
                egui::Shape::Text(t) => t.galley.text().contains(needle),
                egui::Shape::Vec(shapes) => shapes.iter().any(|s| inside(s, needle)),
                _ => false,
            }
        }
        shapes.iter().any(|s| inside(&s.shape, needle))
    }

    fn frame(
        window: &mut Window,
        ctx: &egui::Context,
        area: egui::Rect,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        ui_pass(
            window,
            ctx,
            area,
            egui::RawInput {
                events,
                ..Default::default()
            },
        )
    }

    fn control(
        window: &Window,
        ctx: &egui::Context,
        area: egui::Rect,
        which: usize,
        from_right: f32,
    ) -> egui::Pos2 {
        let m = CellMetrics::measure(ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let work = tab_bar::geometry(window, area, &m).consoles;
        let rect = pane_view::view_tiling(window, work, &m).parts[which];
        egui::pos2(
            rect.right() - PANEL_GRIP - DIVIDER_THICKNESS - from_right,
            rect.top() + header::height(&m, window.headers) / 2.0,
        )
    }

    /// The active tab's surface: Ivy Night's console background, which it opens onto.
    const ACTIVE_TAB: egui::Color32 = egui::Color32::from_rgb(0x13, 0x18, 0x1b);

    /// A point on one of the title bar's tab targets, read from the geometry the strip uses.
    fn tab_target(
        window: &Window,
        ctx: &egui::Context,
        area: egui::Rect,
        which: usize,
        target: &str,
    ) -> egui::Pos2 {
        let m = CellMetrics::measure(ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let band = tab_bar::geometry(window, area, &m)
            .cards
            .expect("the title bar holds the tabs");
        let count = window.tabs.len();
        let card = tab_bar::title_card(band, which, count);
        match target {
            "body" => egui::pos2(card.left() + 40.0, card.center().y),
            "close" => egui::pos2(card.right() - 13.0, card.center().y),
            _ => egui::pos2(
                tab_bar::occupied(band, count).unwrap().right() - 14.0,
                card.center().y,
            ),
        }
    }

    /// The full bar's buttons over `area`, laid out as the title bar lays them.
    fn slots(area: egui::Rect) -> chrome::strip::Slots {
        chrome::strip::slots(egui::Rect::from_min_size(
            area.min,
            egui::vec2(area.width(), crate::theme::metrics::TITLE_BAR_HEIGHT),
        ))
    }

    fn connection_button(area: egui::Rect) -> egui::Pos2 {
        if chrome::strip::is_compact(area.width()) {
            // Left of the compact bar's 40-point menu, one 28-point button wide.
            egui::pos2(area.right() - 54.0, area.top() + 20.0)
        } else {
            slots(area).connect.center()
        }
    }

    /// The connect icon stays in neutral ink, with no dot beside it.
    fn connection_neutral(shapes: &[egui::epaint::ClippedShape], area: egui::Rect) -> bool {
        let neutral = crate::theme::Roles::of(&crate::theme::Theme::default()).tx2;
        let button = egui::Rect::from_center_size(connection_button(area), egui::vec2(28.0, 28.0));
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Circle(c)
                if button.expand(4.0).contains(c.center) && c.radius == 3.0)),
            "the connect button still carries a dot"
        );
        // Chrome paints a base icon before after_panels repaints it, so only the last icon painted
        // in the button is the visible one; the earlier one can be covered by amber.
        let ink = shapes
            .iter()
            .rev()
            .filter_map(|shape| crate::ui::icons::painted_icon(&shape.shape))
            .find(|(glyph, _, square)| {
                glyph == egui_phosphor::regular::PLUGS_CONNECTED && button.contains(square.center())
            })
            .map(|(_, color, _)| color)
            .expect("the connector's visible icon must be painted");
        ink == neutral
    }

    fn open_connection_guide(
        window: &mut Window,
        ctx: &egui::Context,
        area: egui::Rect,
    ) -> Vec<egui::epaint::ClippedShape> {
        frame(window, ctx, area, vec![]);
        frame(window, ctx, area, click_at(connection_button(area)));
        frame(window, ctx, area, vec![]);
        frame(window, ctx, area, vec![])
    }

    #[test]
    fn connection_guide_opens_from_wide_and_compact_title_bars_and_close_is_reachable() {
        for width in [1400.0, 240.0] {
            let ctx = egui::Context::default();
            crate::fonts::install_icons_only(&ctx);
            chrome::prepare(&ctx);
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            crate::viewer::activate(&mut window, true);
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 420.0));
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(connection_neutral(&shapes, area));
            let shapes = open_connection_guide(&mut window, &ctx, area);
            assert!(has_text(&shapes, "Connect agents"));
            assert!(
                !has_text(&shapes, "Choose a file in Files."),
                "the drawer overlaps the modal"
            );
            assert!(has_text(&shapes, "No panels published yet"));
            let heading = text_center(&shapes, "Connect agents");
            let close = shapes
                .iter()
                .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
                .filter(|(glyph, _, _)| glyph == egui_phosphor::regular::X)
                .map(|(_, _, square)| square.center())
                .min_by(|a, b| (a.y - heading.y).abs().total_cmp(&(b.y - heading.y).abs()))
                .expect("guide has a close icon");
            assert!(area.contains(close), "guide close control left the window");
            assert!(
                close.x > heading.x,
                "close icon is not to the right of the heading"
            );
            frame(&mut window, &ctx, area, click_at(close));
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(!has_text(&shapes, "No panels published yet"));
            assert!(
                has_text(&shapes, "Choose a file in Files."),
                "the drawer did not resume after closure at {width}"
            );
        }
    }

    #[test]
    fn neutral_connection_icon_and_guide_observe_file_publication_even_in_a_background_tab() {
        for tab in [0, 1] {
            let ctx = egui::Context::default();
            crate::fonts::install_icons_only(&ctx);
            ctx.global_style_mut(|style| style.animation_time = 0.0);
            let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1800.0));
            assert!(connection_neutral(
                &frame(&mut window, &ctx, area, vec![]),
                area
            ));
            let pane = &mut window.tabs[tab].panes_mut()[0];
            pane.panel = Some(PanelDir::create().unwrap());
            publish_at(pane, r#"{"lines":["A file producer needs no MCP"]}"#);
            pane.to_reread.raise();
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(
                connection_neutral(&shapes, area),
                "publication changed the neutral connection icon"
            );
            let shapes = open_connection_guide(&mut window, &ctx, area);
            assert!(has_text(&shapes, "A panel has been published"));
            assert!(!has_text(&shapes, "Live publishing connections: 0"));
            let at = text_center(&shapes, "Manual setup and diagnostics");
            frame(&mut window, &ctx, area, vec![egui::Event::PointerMoved(at)]);
            frame(&mut window, &ctx, area, click_at(at));
            let time = ctx.input(|input| input.time) + 1.0;
            ui_pass(
                &mut window,
                &ctx,
                area,
                egui::RawInput {
                    time: Some(time),
                    ..Default::default()
                },
            );
            let shapes = ui_pass(
                &mut window,
                &ctx,
                area,
                egui::RawInput {
                    time: Some(time + 1.0),
                    ..Default::default()
                },
            );
            assert!(has_text(&shapes, "Panels published: 1 of 4 consoles"));
            assert!(has_text(&shapes, "Live publishing connections: 0"));
        }
    }

    #[test]
    fn connection_icon_stays_neutral_through_publication_and_retirement() {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        assert!(connection_neutral(
            &frame(&mut window, &ctx, area, vec![]),
            area
        ));
        window.active_mut().panes_mut()[0]
            .panel_state
            .deliver(
                7,
                "example",
                panel_contract::parse_document(r#"{"lines":["Published over the channel"]}"#)
                    .unwrap(),
                SystemTime::now(),
            )
            .unwrap();
        assert!(connection_neutral(
            &frame(&mut window, &ctx, area, vec![]),
            area
        ));
        let shapes = open_connection_guide(&mut window, &ctx, area);
        assert!(has_text(&shapes, "A panel has been published"));
        let escape = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut window, &ctx, area, vec![escape]);
        let dir = PanelDir::create().unwrap();
        window.active_mut().panes_mut()[0]
            .panel_state
            .retire(
                7,
                panel::PanelState::retirement_baseline(dir.path()).unwrap(),
                panel::Retirement {
                    closed: 0,
                    preserved: 0,
                },
            )
            .unwrap();
        assert!(connection_neutral(
            &frame(&mut window, &ctx, area, vec![]),
            area
        ));
    }
#[cfg(unix)]
#[test]
    fn the_search_box_keeps_clear_of_a_held_paste_s_question() {
        let Some(boundary) = keyboard::the_longest_line_a_reader_keeps() else {
            return;
        };
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = integrated_pair();
        let area = tall_area();
        frame(&mut window, &ctx, area, vec![]);
        let names = names_in_order(&window);
        {
            let pane = window.pane_called(names[0]).unwrap();
            keyboard::paste_or_hold(
                &"x".repeat(boundary + 1),
                false,
                &mut Vec::new(),
                &mut pane.held,
            );
            assert!(pane.held.is_some(), "the premise: the paste is held");
        }
        let at = control(&window, &ctx, area, 0, 112.5);
        frame(&mut window, &ctx, area, click_at(at));
        frame(&mut window, &ctx, area, vec![]);
        let pane = pane_of(&window, names[0]);
        let painted = pane.search.open.as_ref().unwrap().painted.clone().unwrap();
        let painter = ctx.layer_painter(egui::LayerId::background());
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let console = pane.painted_console.as_ref().unwrap().area;
        let question =
            crate::painter::notices::the_boxes(&painter, pane, &window.theme, &metrics, console)
                .question
                .expect("the question is laid out");
        assert!(
            !painted.layout.frame.intersects(question),
            "the box covers the held paste's question: {:?} over {question:?}",
            painted.layout.frame
        );
    }
    crate::windows_unix_omission!(
        search_navigation_edits_the_query_without_reaching_the_program,
        "unix-only: requires POSIX raw-mode child byte capture to verify UI input does not reach the program"
    );

    #[cfg(unix)]
    #[test]
    fn search_navigation_edits_the_query_without_reaching_the_program() {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let received = term_core::received::Received::new("search-navigation");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        received.wait_until_ready(Duration::from_secs(5));
        let area = tall_area();
        frame(&mut window, &ctx, area, vec![]);
        let at = control(&window, &ctx, area, 0, 112.5);
        window.blink.typed_at = Instant::now() - Duration::from_secs(20);
        frame(&mut window, &ctx, area, click_at(at));
        assert!(window.blink.typed_at.elapsed() < Duration::from_secs(1));
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("Ask".into())],
        );
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut window,
            &ctx,
            area,
            vec![key(egui::Key::ArrowLeft), egui::Event::Text("!".into())],
        );
        assert_eq!(
            window.active().panes()[0]
                .search
                .open
                .as_ref()
                .unwrap()
                .query(),
            "As!k"
        );
        for navigation in [
            egui::Key::Home,
            egui::Key::End,
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
            egui::Key::ArrowRight,
            egui::Key::PageUp,
            egui::Key::PageDown,
        ] {
            frame(&mut window, &ctx, area, vec![key(navigation)]);
            assert!(
                window.active().panes()[0].search.is_open(),
                "{navigation:?} closed search"
            );
        }
        let shapes = frame(&mut window, &ctx, area, vec![]);
        assert!(has_text(&shapes, "As!k"));
        assert!(!has_text(&shapes, "As!k_"));
        frame(
            &mut window,
            &ctx,
            area,
            vec![key(egui::Key::Escape), egui::Event::Text("Q".into())],
        );
        assert_eq!(
            received.wait_for(1, Duration::from_secs(5)),
            b"Q",
            "navigation/editing leaked to the program before the sentinel"
        );
    }
    fn search_blink_fixture() -> (Window, egui::Context) {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = integrated_pair();
        for pane in window.active_mut().panes_mut() {
            // The field must be the only possible source of a blink deadline.
            pane.screen.advance(b"\x1b[?25l");
        }
        let area = tall_area();
        frame(&mut window, &ctx, area, vec![]);
        let at = control(&window, &ctx, area, 0, 112.5);
        frame(&mut window, &ctx, area, click_at(at));
        assert!(window.active().panes()[0].search.is_open());
        (window, ctx)
    }

    fn search_blink_delay(
        window: &mut Window,
        ctx: &egui::Context,
        elapsed: Duration,
        focused: bool,
        disabled: bool,
    ) -> Duration {
        let mut delay = Duration::ZERO;
        // Drain the immediate paints requested by opening the box and loading fonts.
        for _ in 0..4 {
            window.blink.typed_at = Instant::now() - elapsed;
            let mut app_frame = eframe::Frame::_new_kittest();
            eframe::App::logic(window, ctx, &mut app_frame);
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(tall_area()),
                    focused,
                    ..Default::default()
                },
                |ui| {
                    if disabled {
                        ui.disable();
                    }
                    eframe::App::ui(window, ui, &mut app_frame);
                },
            );
            delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
            output.drop_without_applying_deltas();
        }
        delay
    }

    #[test]
    fn search_blink_deadline_covers_both_halves_and_disarms_when_closed() {
        let (mut window, ctx) = search_blink_fixture();
        for millis in [100, 700, 1300] {
            let delay = search_blink_delay(
                &mut window,
                &ctx,
                Duration::from_millis(millis),
                true,
                false,
            );
            assert!(
                window.blink.painted,
                "search did not participate at {millis} ms"
            );
            assert!(
                !delay.is_zero() && delay <= Duration::from_millis(600),
                "search did not arm its blink deadline at {millis} ms: {delay:?}"
            );
        }
        let delay = search_blink_delay(&mut window, &ctx, Duration::from_secs(16), true, false);
        assert!(!window.blink.painted, "idle search still participates");
        assert_eq!(delay, Duration::MAX, "idle search still wakes the window");
        let delay = search_blink_delay(&mut window, &ctx, Duration::from_millis(100), true, false);
        assert!(window.blink.painted && !delay.is_zero() && delay <= Duration::from_millis(600));
        frame(
            &mut window,
            &ctx,
            tall_area(),
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!window.active().panes()[0].search.is_open());
        let delay = search_blink_delay(&mut window, &ctx, Duration::from_millis(100), true, false);
        assert!(
            !window.blink.painted,
            "closed search left stale blink participation"
        );
        assert_eq!(delay, Duration::MAX, "closed search left a blink deadline");
    }

    #[test]
    fn search_blink_deadlines_follow_focus_enabled_state_and_the_measurement_ceiling() {
        let (mut window, ctx) = search_blink_fixture();
        let age = Duration::from_millis(700);
        let active = search_blink_delay(&mut window, &ctx, age, true, false);
        assert!(window.blink.painted && !active.is_zero() && active <= Duration::from_millis(600));
        let names = names_in_order(&window);
        window.active_mut().focus = names[1];
        let delay = search_blink_delay(&mut window, &ctx, age, true, false);
        assert!(!window.blink.painted, "unfocused search still participates");
        assert_eq!(
            delay,
            Duration::MAX,
            "unfocused search still wakes the window"
        );
        window.active_mut().focus = names[0];
        for (focused, disabled) in [(false, false), (true, true)] {
            let active = search_blink_delay(&mut window, &ctx, age, true, false);
            assert!(
                window.blink.painted && !active.is_zero() && active <= Duration::from_millis(600)
            );
            let delay = search_blink_delay(&mut window, &ctx, age, focused, disabled);
            assert!(
                !window.blink.painted,
                "focused={focused}, disabled={disabled}"
            );
            assert_eq!(delay, Duration::MAX, "inactive field still has a deadline");
        }
        let active = search_blink_delay(&mut window, &ctx, age, true, false);
        assert!(window.blink.painted && !active.is_zero() && active <= Duration::from_millis(600));
        window.blink.ceiling = Some(false);
        let delay = search_blink_delay(&mut window, &ctx, age, true, false);
        assert!(
            !window.blink.painted,
            "blink-off measurement ceiling was bypassed"
        );
        assert_eq!(
            delay,
            Duration::MAX,
            "measurement search still wakes the window"
        );
    }

    /// One pass at an input time: what it painted and how soon it asked to be woken.
    fn timed_pass(
        window: &mut Window,
        ctx: &egui::Context,
        area: egui::Rect,
        time: f64,
        events: Vec<egui::Event>,
    ) -> (Vec<egui::epaint::ClippedShape>, Duration) {
        let mut frame = eframe::Frame::_new_kittest();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| eframe::App::ui(window, ui, &mut frame),
        );
        let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();
        (shapes, delay)
    }

    /// The alpha of every header icon and group rule painted inside `cells`.
    fn icon_alphas(shapes: &[egui::epaint::ClippedShape], cells: egui::Rect) -> Vec<u8> {
        let icons = shapes
            .iter()
            .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
            .filter(|(_, _, square)| cells.contains(square.center()))
            .map(|(_, color, _)| color.a());
        let rules = shapes.iter().filter_map(|s| match &s.shape {
            egui::Shape::LineSegment { points, stroke } if cells.contains(points[0]) => {
                Some(stroke.color.a())
            }
            _ => None,
        });
        let alphas: Vec<u8> = icons.chain(rules).collect();
        alphas
    }

    fn whole(alphas: &[u8]) -> bool {
        !alphas.is_empty() && alphas.iter().all(|a| *a == 255)
    }

    fn fading(alphas: &[u8]) -> bool {
        !alphas.is_empty() && alphas.iter().all(|a| *a > 0 && *a < 255)
    }

    /// A window where fades are allowed, the blink is off so it cannot wake anything, and the
    /// right-hand console (unfocused) has its header under the pointer from the start.
    fn lingering_pair(fades: bool) -> (egui::Context, Window, egui::Rect) {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        crate::pane_view::allow_focus_fades(&ctx, fades);
        let mut window = integrated_pair();
        window.blink.ceiling = Some(false);
        let area = tall_area();
        timed_pass(&mut window, &ctx, area, 0.0, vec![]);
        (ctx, window, area)
    }

    /// Leaving a header after the window slept: the controls stay whole for 3 s, fade for 0.5 s, then
    /// go and stop waking the window; appearing is immediate, and coming back mid-fade is whole again.
    #[test]
    fn unwanted_header_controls_linger_then_fade_and_come_back_whole() {
        let (ctx, mut window, area) = lingering_pair(true);
        let title = control(&window, &ctx, area, 1, 300.0);
        let body = pane_rect(&window, &ctx, area, names_in_order(&window)[1]).center();
        let cells = control_cells(&window, &ctx, area, 1);
        let at = |window: &mut Window, time, events| {
            let (shapes, delay) = timed_pass(window, &ctx, area, time, events);
            (icon_alphas(&shapes, cells), delay)
        };
        let (entered, _) = at(&mut window, 0.1, vec![egui::Event::PointerMoved(title)]);
        assert!(whole(&entered), "appearing is not immediate: {entered:?}");
        at(&mut window, 0.2, vec![]);
        at(&mut window, 0.3, vec![]);
        // Nothing paints for ten seconds, then the pointer leaves in one jump.
        let (left, _) = at(&mut window, 10.0, vec![egui::Event::PointerMoved(body)]);
        assert!(whole(&left), "the controls vanished on the leave: {left:?}");
        // egui answers a pointer move with one more pass of its own; the wait is measured after it.
        at(&mut window, 10.1, vec![]);
        let (waiting, delay) = at(&mut window, 10.5, vec![]);
        assert!(whole(&waiting), "{waiting:?}");
        assert!(
            delay > Duration::from_secs(1) && delay <= Duration::from_millis(2500),
            "the wait should wake once, at its end: {delay:?}"
        );
        let (late, _) = at(&mut window, 12.9, vec![]);
        assert!(whole(&late), "gone before the 3 s were up: {late:?}");
        let (half, delay) = at(&mut window, 13.25, vec![]);
        assert!(fading(&half), "not fading half-way: {half:?}");
        assert!(
            delay < Duration::from_millis(100),
            "the fade asked for no frame: {delay:?}"
        );
        let (gone, _) = at(&mut window, 13.6, vec![]);
        assert!(gone.is_empty(), "still painted after the fade: {gone:?}");
        let (_, rest) = at(&mut window, 13.7, vec![]);
        assert!(
            rest > Duration::from_secs(5),
            "a finished fade still wakes: {rest:?}"
        );

        // Leave again, and come back half-way through the fade.
        at(&mut window, 20.0, vec![egui::Event::PointerMoved(title)]);
        at(&mut window, 20.1, vec![egui::Event::PointerMoved(body)]);
        let (half, _) = at(&mut window, 23.35, vec![]);
        assert!(fading(&half), "{half:?}");
        let (back, _) = at(&mut window, 23.36, vec![egui::Event::PointerMoved(title)]);
        assert!(whole(&back), "coming back did not restore them: {back:?}");
        for time in [23.7, 30.0] {
            let (kept, _) = at(&mut window, time, vec![]);
            assert!(whole(&kept), "wanted controls faded at {time}: {kept:?}");
        }
    }

    /// Focus moving away after the window slept starts the same countdown, and a control pressed
    /// while it fades still acts.
    #[test]
    fn a_console_that_loses_focus_keeps_pressable_controls_while_they_fade() {
        let (ctx, mut window, area) = lingering_pair(true);
        let names = names_in_order(&window);
        assert_eq!(window.active().focus, names[0]);
        let cells = control_cells(&window, &ctx, area, 0);
        let split_right = control(&window, &ctx, area, 0, 162.5);
        let other = pane_rect(&window, &ctx, area, names[1]).center();
        let at = |window: &mut Window, time, events| {
            icon_alphas(&timed_pass(window, &ctx, area, time, events).0, cells)
        };
        at(&mut window, 10.0, click_at(other));
        assert_eq!(
            window.active().focus,
            names[1],
            "the click did not move focus"
        );
        assert!(whole(&at(&mut window, 12.9, vec![])));
        assert!(fading(&at(&mut window, 13.25, vec![])));
        at(&mut window, 13.26, click_at(split_right));
        at(&mut window, 13.27, vec![]);
        assert_eq!(
            window.active().panes().len(),
            3,
            "a fading split did not split its console"
        );
    }

    /// AC2 in full: the console that lost focus goes at the end of its fade, and the one that took
    /// it shows its controls whole on the pass of the click.
    #[test]
    fn focus_moving_away_ends_in_gone_and_the_new_console_shows_its_controls_at_once() {
        let (ctx, mut window, area) = lingering_pair(true);
        let names = names_in_order(&window);
        let left = control_cells(&window, &ctx, area, 0);
        let right = control_cells(&window, &ctx, area, 1);
        let other = pane_rect(&window, &ctx, area, names[1]).center();
        let at = |window: &mut Window, time, events| timed_pass(window, &ctx, area, time, events).0;
        let before = at(&mut window, 0.1, vec![]);
        assert!(
            icon_alphas(&before, right).is_empty(),
            "the unfocused console showed its controls before the click"
        );
        let clicked = at(&mut window, 10.0, click_at(other));
        assert_eq!(
            window.active().focus,
            names[1],
            "the click did not move focus"
        );
        assert!(
            whole(&icon_alphas(&clicked, right)),
            "the console that took focus did not show its controls at once"
        );
        assert!(whole(&icon_alphas(&clicked, left)));
        assert!(whole(&icon_alphas(&at(&mut window, 12.9, vec![]), left)));
        assert!(fading(&icon_alphas(&at(&mut window, 13.25, vec![]), left)));
        let gone = at(&mut window, 13.6, vec![]);
        assert!(
            icon_alphas(&gone, left).is_empty(),
            "the old console's controls outlived their fade"
        );
        assert!(whole(&icon_alphas(&gone, right)));
    }

    /// AC3 during the wait: a second after focus left, the controls are whole and a press acts.
    #[test]
    fn a_lingering_control_acts_before_the_fade_starts() {
        let (ctx, mut window, area) = lingering_pair(true);
        let names = names_in_order(&window);
        let split_right = control(&window, &ctx, area, 0, 162.5);
        let other = pane_rect(&window, &ctx, area, names[1]).center();
        let cells = control_cells(&window, &ctx, area, 0);
        timed_pass(&mut window, &ctx, area, 10.0, click_at(other));
        assert_eq!(window.active().focus, names[1]);
        // The pass just before the press is the one the press gate reads (D99's `revealed`): it
        // must be a second into the wait, not the click's own pass, where anything is still whole.
        timed_pass(&mut window, &ctx, area, 10.5, vec![]);
        let waiting = icon_alphas(&timed_pass(&mut window, &ctx, area, 10.99, vec![]).0, cells);
        assert!(
            whole(&waiting),
            "not whole a second into the wait: {waiting:?}"
        );
        timed_pass(&mut window, &ctx, area, 11.0, click_at(split_right));
        timed_pass(&mut window, &ctx, area, 11.01, vec![]);
        assert_eq!(
            window.active().panes().len(),
            3,
            "a lingering split did not split its console"
        );
    }

    /// AC4 by focus: taking focus back mid-fade restores the controls whole on that pass, and
    /// they stay whole while wanted.
    #[test]
    fn focus_coming_back_mid_fade_restores_the_controls_whole() {
        let (ctx, mut window, area) = lingering_pair(true);
        let names = names_in_order(&window);
        let cells = control_cells(&window, &ctx, area, 0);
        let own = pane_rect(&window, &ctx, area, names[0]).center();
        let other = pane_rect(&window, &ctx, area, names[1]).center();
        let at = |window: &mut Window, time, events| {
            icon_alphas(&timed_pass(window, &ctx, area, time, events).0, cells)
        };
        at(&mut window, 10.0, click_at(other));
        assert!(fading(&at(&mut window, 13.25, vec![])));
        let back = at(&mut window, 13.26, click_at(own));
        assert_eq!(
            window.active().focus,
            names[0],
            "the click did not move focus back"
        );
        assert!(
            whole(&back),
            "taking focus back did not restore them: {back:?}"
        );
        for time in [13.6, 20.0] {
            let kept = at(&mut window, time, vec![]);
            assert!(whole(&kept), "wanted controls faded at {time}: {kept:?}");
        }
    }

    /// AC7 by focus: without fades, the console that loses focus drops its controls on that pass.
    #[test]
    fn without_fades_a_console_losing_focus_drops_its_controls_at_once() {
        let (ctx, mut window, area) = lingering_pair(false);
        let names = names_in_order(&window);
        let cells = control_cells(&window, &ctx, area, 0);
        let other = pane_rect(&window, &ctx, area, names[1]).center();
        let (shown, _) = timed_pass(&mut window, &ctx, area, 0.1, vec![]);
        assert!(whole(&icon_alphas(&shown, cells)));
        let (left, _) = timed_pass(&mut window, &ctx, area, 0.2, click_at(other));
        assert_eq!(window.active().focus, names[1]);
        assert!(
            icon_alphas(&left, cells).is_empty(),
            "a run without fades lingered after focus left"
        );
    }

    /// Near the end of the fade a control is too faint to be aimed at, and a press there is the
    /// title's: it focuses the console and closes nothing.
    #[test]
    fn a_control_too_faint_to_see_does_not_answer() {
        // The left console loses focus at 10.0; its close is pressed on the pass after one
        // painted it at 27 % (answers) or at 23 % (the title's) — either side of a quarter.
        for (painted_at, closes) in [(13.365, true), (13.385, false)] {
            let (ctx, mut window, area) = lingering_pair(true);
            let names = names_in_order(&window);
            let close = control(&window, &ctx, area, 0, 12.5);
            let other = pane_rect(&window, &ctx, area, names[1]).center();
            let at = |window: &mut Window, time, events| {
                timed_pass(window, &ctx, area, time, events);
            };
            at(&mut window, 10.0, click_at(other));
            for time in [12.0, 13.0, painted_at] {
                at(&mut window, time, vec![]);
            }
            at(&mut window, painted_at + 0.001, click_at(close));
            at(&mut window, painted_at + 0.002, vec![]);
            assert_eq!(
                window.active().panes().len(),
                if closes { 1 } else { 2 },
                "painted at {painted_at}"
            );
            if !closes {
                assert_eq!(
                    window.active().focus,
                    names[0],
                    "the faint press did not go to the title"
                );
            }
        }
    }

    /// A console whose header was not painted for a while — its tab in the background — comes back
    /// without lingering controls nobody saw leave.
    #[test]
    fn a_header_back_from_a_background_tab_does_not_linger() {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        crate::pane_view::allow_focus_fades(&ctx, true);
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        window.blink.ceiling = Some(false);
        let area = tall_area();
        timed_pass(&mut window, &ctx, area, 0.0, vec![]);
        let title = control(&window, &ctx, area, 1, 300.0);
        let body = pane_rect(&window, &ctx, area, names_in_order(&window)[1]).center();
        let cells = control_cells(&window, &ctx, area, 1);
        let (seen, _) = timed_pass(
            &mut window,
            &ctx,
            area,
            0.1,
            vec![egui::Event::PointerMoved(title)],
        );
        assert!(
            whole(&icon_alphas(&seen, cells)),
            "the hover did not show them"
        );
        window.active = 1;
        for time in [1.0, 2.0] {
            timed_pass(&mut window, &ctx, area, time, vec![]);
        }
        window.active = 0;
        let (back, _) = timed_pass(
            &mut window,
            &ctx,
            area,
            60.0,
            vec![egui::Event::PointerMoved(body)],
        );
        assert!(
            icon_alphas(&back, cells).is_empty(),
            "controls nobody saw leave came back lingering"
        );
    }

    /// Where fades are off — every measured run — the controls go on the pass that stops wanting them.
    #[test]
    fn without_fades_unwanted_controls_go_at_once() {
        let (ctx, mut window, area) = lingering_pair(false);
        let title = control(&window, &ctx, area, 1, 300.0);
        let body = pane_rect(&window, &ctx, area, names_in_order(&window)[1]).center();
        let cells = control_cells(&window, &ctx, area, 1);
        let (shown, _) = timed_pass(
            &mut window,
            &ctx,
            area,
            0.1,
            vec![egui::Event::PointerMoved(title)],
        );
        assert!(whole(&icon_alphas(&shown, cells)));
        let (left, _) = timed_pass(
            &mut window,
            &ctx,
            area,
            0.2,
            vec![egui::Event::PointerMoved(body)],
        );
        assert!(
            icon_alphas(&left, cells).is_empty(),
            "a run without fades lingered"
        );
    }
}
