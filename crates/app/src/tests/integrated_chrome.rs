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
    /// the panel surface between them, and the focused console is outlined in its project's ink —
    /// the accent once it has none. Ivy Night's panel, chrome and accent are written out.
    #[test]
    fn focus_moves_the_surface_and_an_outline_in_the_projects_ink() {
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
        // Ivy Night's panel #171D21 with 18 % of the ink, and its chrome #0F1315 with 14 %.
        let panel = egui::Color32::from_rgb(0x3b, 0x37, 0x2e);
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
        assert_eq!(outline_around(&baseline, focused), Some(ink));
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
            Some(ink),
            "the outline did not follow the focus"
        );
        window.active_mut().focus = 0;
        window.active_mut().panes_mut()[0].project = None;
        let neutral = frame(&mut window, &ctx, area, vec![]);
        let first = window.active().panes()[0].name;
        assert_eq!(window.active().focus, first);
        let plain = header(&neutral, egui::Color32::from_rgb(0x17, 0x1d, 0x21))
            .expect("a console without a project keeps the plain panel");
        assert_eq!(outline_around(&neutral, plain), Some(accent));
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
                !has_text(&shapes, "Choose a Markdown file in Files."),
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
                has_text(&shapes, "Choose a Markdown file in Files."),
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

    #[test]
    fn connection_guide_and_its_closing_frame_deliver_no_keys_or_paste_to_the_program() {
        let ctx = egui::Context::default();
        let received = term_core::received::Received::new("connection-guide");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        received.wait_until_ready(Duration::from_secs(5));
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let grids: Vec<_> = window.active().panes().iter().map(|p| p.measured).collect();
        open_connection_guide(&mut window, &ctx, area);
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::Text("NEVER".into()),
                egui::Event::Paste("PASTE".into()),
            ],
        );
        let held_at = egui::pos2(50.0, 150.0);
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(held_at),
                egui::Event::PointerButton {
                    pos: held_at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let escape = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                escape,
                egui::Event::Text("TAIL".into()),
                egui::Event::Paste("CLOSING".into()),
            ],
        );
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerButton {
                    pos: held_at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Paste("RELEASE".into()),
            ],
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
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("AFTER".into())],
        );
        assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"AFTER");
    }

    #[test]
    fn reconnect_help_copies_only_the_command_and_never_types_into_the_console() {
        let ctx = egui::Context::default();
        chrome::prepare(&ctx);
        let received = term_core::received::Received::new("connection-reconnect-copy");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        received.wait_until_ready(Duration::from_secs(5));
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0));
        let shapes = open_connection_guide(&mut window, &ctx, area);
        assert!(!has_text(&shapes, "Copy first-time setup prompt"));
        assert!(has_text(&shapes, "Reconnect an existing Claude session"));
        let at = text_center(&shapes, "Copy reconnect command");
        frame(&mut window, &ctx, area, click_at(at)[..2].to_vec());
        let mut native = eframe::Frame::_new_kittest();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area),
                events: click_at(at)[2..].to_vec(),
                ..Default::default()
            },
            |ui| eframe::App::ui(&mut window, ui, &mut native),
        );
        let copied: Vec<_> = output
            .platform_output
            .commands
            .iter()
            .filter_map(|command| {
                if let egui::OutputCommand::CopyText(text) = command {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(copied, vec!["/mcp reconnect ivy"]);
        assert!(has_text(&output.shapes, "Copied"));
        output.drop_without_applying_deltas();
        frame(
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
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("SAFE".into())],
        );
        assert_eq!(received.wait_for(4, Duration::from_secs(5)), b"SAFE");
    }

    #[test]
    fn connection_guide_copies_setup_prompt_with_local_path_without_typing_it() {
        fn button_fill(shapes: &[egui::epaint::ClippedShape], at: egui::Pos2) -> egui::Color32 {
            shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.rect.contains(at) => Some(rect),
                    _ => None,
                })
                .min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
                .expect("copy button painted")
                .fill
        }
        let ctx = egui::Context::default();
        chrome::prepare(&ctx);
        let received = term_core::received::Received::new("connection-copy");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        received.wait_until_ready(Duration::from_secs(5));
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1200.0));
        let shapes = open_connection_guide(&mut window, &ctx, area);
        assert!(!has_text(&shapes, "1. Locate the MCP bridge"));
        assert!(!has_text(&shapes, "Copy first-time setup prompt"));
        assert!(has_text(&shapes, "Copy reconnect command"));
        let manual = text_center(&shapes, "Manual setup and diagnostics");
        frame(&mut window, &ctx, area, click_at(manual));
        frame(&mut window, &ctx, area, vec![]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        assert!(has_text(&shapes, "First-time setup"));
        let at = text_center(&shapes, "Copy first-time setup prompt");
        let idle = button_fill(&shapes, at);
        frame(&mut window, &ctx, area, vec![egui::Event::PointerMoved(at)]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        let hovered = button_fill(&shapes, at);
        assert_ne!(idle, hovered, "copy button gives no hover feedback");
        let shapes = frame(&mut window, &ctx, area, click_at(at)[..2].to_vec());
        let pressed = button_fill(&shapes, at);
        assert_ne!(pressed, hovered, "copy button gives no pressed feedback");
        let mut native = eframe::Frame::_new_kittest();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area),
                events: click_at(at)[2..].to_vec(),
                ..Default::default()
            },
            |ui| eframe::App::ui(&mut window, ui, &mut native),
        );
        let expected = std::env::current_exe()
            .unwrap()
            .with_file_name(if cfg!(windows) {
                "panel-mcp.exe"
            } else {
                "panel-mcp"
            });
        assert!(output.platform_output.commands.iter().any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text.contains(&serde_json::to_string(&expected.to_string_lossy()).unwrap())
            && text.contains("local stdio MCP server named ivy")
            && text.contains("env_vars = [\"PANEL_ID\", \"PANEL_DIR\"]")
            && text.contains("Tool discovery alone does not prove a connection")
            && text.contains("shared background server lacks that console identity")
            && text.contains("do not assume the client is outside Ivy")
            && text.contains("0.167.2 or 0.169.0 use the shared daemon")
            && text.contains("A connected host does not prove")
            && text.contains("phone history, input and panel publication were verified")
            && text.contains("does not edit global shell or Codex configuration")
            && text.contains("Existing aliases or functions win with a diagnostic")
            && text.contains("not correct-console routing")
            && text.contains("distinct statuses from two separate Codex consoles")
            && text.contains("Never hard-code")
            && text.contains("PANEL_ID and PANEL_DIR")
            && text.contains("reconnecting or restarting")
            && text.contains("panel_get and panel_set")
            && text.contains("Report success only after the tool calls succeed"))));
        let copied_prompt = output
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.as_str()),
                _ => None,
            })
            .expect("setup prompt is copied");
        assert!(
            copied_prompt
                .contains("If this client did not pass you the bridge's server instructions")
        );
        assert!(copied_prompt.contains(
            "An agent working for another agent in the same console must leave the panel alone"
        ));
        assert!(
            copied_prompt.contains("An agent started in a console of its own keeps its own panel")
        );
        let readme = include_str!("../../../../README.md")
            .lines()
            .map(|line| line.trim_start().trim_start_matches("> "))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            readme.contains("If this client did not pass you the bridge's server instructions")
        );
        assert!(readme.contains(
            "An agent working for another agent in the same console must leave the panel alone"
        ));
        assert!(readme.contains("An agent started in a console of its own keeps its own panel"));
        assert!(
            has_text(&output.shapes, "Copied"),
            "copy has no visible confirmation"
        );
        output.drop_without_applying_deltas();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0));
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
        let expected_manual = [
            "Codex: forward the console identity",
            "env_vars = [\"PANEL_ID\", \"PANEL_DIR\"]",
            "Do not hard-code PANEL_ID or PANEL_DIR",
            "These are variable names, not saved console values.",
            "Tool discovery alone does not prove a connection",
            "type codex normally",
            "Existing codex aliases or functions take precedence",
            "unsupported versions and advanced options",
            "A connected host does not prove",
            "A successful tool call proves a usable connection.",
        ];
        let visible = |shapes: &[egui::epaint::ClippedShape], needle: &str| {
            shapes.iter().any(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => {
                    text.galley.text().contains(needle)
                        && clipped
                            .clip_rect
                            .contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size()))
                }
                _ => false,
            })
        };
        assert!(
            !visible(
                &shapes,
                "A successful tool call proves a usable connection."
            ),
            "the lower guidance must require scrolling in a normal-height window"
        );
        let mut seen = expected_manual.map(|text| visible(&shapes, text));
        for step in 0..8 {
            let shapes = ui_pass(
                &mut window,
                &ctx,
                area,
                egui::RawInput {
                    time: Some(time + 2.0 + f64::from(step)),
                    events: vec![egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: egui::vec2(0.0, -300.0),
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
            );
            for (found, text) in seen.iter_mut().zip(expected_manual) {
                *found |= visible(&shapes, text);
            }
        }
        for (found, text) in seen.into_iter().zip(expected_manual) {
            assert!(found, "manual guidance missing while scrolling: {text}");
        }
        let escape = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&mut window, &ctx, area, vec![escape]);
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("SAFE".into())],
        );
        assert_eq!(received.wait_for(4, Duration::from_secs(5)), b"SAFE");
    }

    #[test]
    fn ivy_logo_opens_the_bar_before_files_and_the_tabs() {
        let ctx = egui::Context::default();
        chrome::prepare(&ctx);
        let texture = ctx.data(|data| {
            data.get_temp::<egui::TextureHandle>(egui::Id::new("ivy-logo"))
                .unwrap()
                .id()
        });
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        for (width, visible) in [
            (1400.0, true),
            (600.0, true),
            (240.0, false),
            (100.0, false),
        ] {
            let area = area_of(width);
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(!has_text(&shapes, "agent-terminal"));
            let logos: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh),
                    _ => None,
                })
                .collect();
            assert_eq!(logos.len(), usize::from(visible), "{width}");
            let m = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
            let tabs = tab_bar::geometry(&window, area, &m).cards.unwrap();
            for mesh in logos {
                let bounds = mesh.calc_bounds();
                let left = if cfg!(target_os = "macos") { 78.0 } else { 6.0 };
                assert!(bounds.top() >= area.top());
                assert!(bounds.bottom() <= area.top() + crate::theme::metrics::TITLE_BAR_HEIGHT);
                // The logo opens the bar; Files stands a little apart after it, then the tabs.
                assert!(bounds.left() >= area.left() + left);
                assert!(bounds.right() + 8.0 <= slots(area).files.left());
                assert!(bounds.right() <= tabs.left());
            }
        }
    }

    #[test]
    fn maximizing_an_unfocused_console_restores_the_original_weighted_tree() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        if let Node::Branch { children, .. } = &mut window.active_mut().root {
            children[0].weight = Weight::new(0.3).unwrap();
            children[1].weight = Weight::new(0.7).unwrap();
        }
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let original = window.active().root.describe();
        let names = names_in_order(&window);
        let zoom = control(&window, &ctx, area, 1, 37.5);
        // An unfocused header's controls are pressable once a pass has painted them (D99).
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(zoom)],
        );
        let shapes = frame(&mut window, &ctx, area, click_at(zoom));
        assert_eq!(window.active().focus, names[1]);
        assert_eq!(
            window.pane_view.zoomed(window.active().name),
            Some(names[1])
        );
        assert!(the_text_queued_up(&shapes, "Console 2"));
        assert!(!the_text_queued_up(&shapes, "Console 1"));
        assert_eq!(window.active().root.describe(), original);
        let restore = control(&window, &ctx, area, 1, 37.5);
        let shapes = frame(&mut window, &ctx, area, click_at(restore));
        assert_eq!(window.pane_view.zoomed(window.active().name), None);
        assert!(the_text_queued_up(&shapes, "Console 1"));
        assert!(the_text_queued_up(&shapes, "Console 2"));
        assert_eq!(window.active().root.describe(), original);
    }

    fn tall_area() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0))
    }

    fn pane_rect(
        window: &Window,
        ctx: &egui::Context,
        area: egui::Rect,
        name: usize,
    ) -> egui::Rect {
        let m = CellMetrics::measure(ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let work = tab_bar::geometry(window, area, &m).consoles;
        let at = names_in_order(window)
            .iter()
            .position(|n| *n == name)
            .unwrap();
        pane_view::view_tiling(window, work, &m).parts[at]
    }

    /// The seven control cells of console `which`'s header, from the left of split right to close.
    fn control_cells(
        window: &Window,
        ctx: &egui::Context,
        area: egui::Rect,
        which: usize,
    ) -> egui::Rect {
        let m = CellMetrics::measure(ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let right = control(window, ctx, area, which, 0.0).x;
        let rect = pane_rect(window, ctx, area, names_in_order(window)[which]);
        egui::Rect::from_min_max(
            egui::pos2(right - 175.0, rect.top()),
            egui::pos2(right, rect.top() + header::height(&m, window.headers)),
        )
    }

    /// Icons and rules painted inside `cells`. Icons are glyphs since D103, and a context without
    /// the icon family paints none, so a test counting them must install it
    /// (`crate::fonts::install_icons_only`) or a zero here proves nothing.
    fn strokes_in(shapes: &[egui::epaint::ClippedShape], cells: egui::Rect) -> usize {
        shapes
            .iter()
            .filter(|s| match &s.shape {
                egui::Shape::Path(path) => path.points.first().is_some_and(|p| cells.contains(*p)),
                egui::Shape::LineSegment { points, .. } => cells.contains(points[0]),
                shape => crate::ui::icons::painted_icon(shape)
                    .is_some_and(|(_, _, square)| cells.contains(square.center())),
            })
            .count()
    }

    fn integrated_pair() -> Window {
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        window
    }

    /// D99: an unfocused console is split from its own header, beside it or below it, and the
    /// focused console does not move.
    #[test]
    fn an_unfocused_console_splits_from_its_own_header() {
        for (from_right, beside) in [(162.5, true), (137.5, false)] {
            let ctx = egui::Context::default();
            let mut window = integrated_pair();
            let area = tall_area();
            frame(&mut window, &ctx, area, vec![]);
            let names = names_in_order(&window);
            assert_eq!(
                window.active().focus,
                names[0],
                "the right console must be unfocused"
            );
            let left_before = pane_rect(&window, &ctx, area, names[0]);
            let owner_before = pane_rect(&window, &ctx, area, names[1]);
            let at = control(&window, &ctx, area, 1, from_right);
            frame(&mut window, &ctx, area, vec![egui::Event::PointerMoved(at)]);
            frame(&mut window, &ctx, area, click_at(at));
            frame(&mut window, &ctx, area, vec![]);
            let after = names_in_order(&window);
            assert_eq!(after.len(), 3, "from_right={from_right}: nothing was split");
            let newborn = *after.iter().find(|n| !names.contains(n)).unwrap();
            let born = pane_rect(&window, &ctx, area, newborn);
            let owner = pane_rect(&window, &ctx, area, names[1]);
            let left = pane_rect(&window, &ctx, area, names[0]);
            // Re-tiling three siblings rounds each edge to a whole point, no more.
            assert!(
                (left.min - left_before.min).length() <= 1.0
                    && (left.max - left_before.max).length() <= 1.0,
                "the focused console moved: {left_before:?} -> {left:?}"
            );
            assert!(owner_before.expand(1.0).contains_rect(born));
            if beside {
                assert!(
                    born.left() > owner.right() - 1.0 && (born.top() - owner.top()).abs() < 1.0
                );
            } else {
                assert!(
                    born.top() > owner.bottom() - 1.0 && (born.left() - owner.left()).abs() < 1.0
                );
            }
        }
    }

    /// D99: only the focused console shows its controls with the pointer elsewhere; another shows
    /// them while the pointer is over its header, and its title stays where it was.
    #[test]
    fn an_unfocused_header_shows_its_controls_only_under_the_pointer() {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = integrated_pair();
        let area = tall_area();
        frame(&mut window, &ctx, area, vec![]);
        let names = names_in_order(&window);
        let focused_cells = control_cells(&window, &ctx, area, 0);
        let cells = control_cells(&window, &ctx, area, 1);
        let body = pane_rect(&window, &ctx, area, names[1]).center();
        let away = frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(body)],
        );
        assert_eq!(strokes_in(&away, cells), 0, "hidden controls were painted");
        assert!(
            strokes_in(&away, focused_cells) > 0,
            "the focused console hid its controls"
        );
        // The icons are glyphs and the two rules between the three groups are the only segments, in
        // the ink of the title's own separator — muted text, not the border role the owner found
        // barely visible on the desktop.
        let roles = crate::ui::style::roles(&ctx, &window.theme);
        let rules: Vec<_> = away
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::LineSegment { points, stroke }
                    if focused_cells.contains(points[0])
                        && (points[0].x - points[1].x).abs() < 0.01 =>
                {
                    Some(stroke.color)
                }
                _ => None,
            })
            .collect();
        assert_eq!(rules.len(), 2, "the groups are not separated by two rules");
        assert!(
            roles.tx3 != roles.line2,
            "the colour check below would be vacuous"
        );
        assert!(
            rules.iter().all(|c| *c == roles.tx3),
            "the rules are not in the title separator's ink: {rules:?}"
        );
        let title = egui::pos2(cells.left() - 40.0, cells.center().y);
        let over = frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(title)],
        );
        assert!(
            strokes_in(&over, cells) > 0,
            "the controls did not appear under the pointer"
        );
        let header = egui::Rect::from_min_max(
            egui::pos2(pane_rect(&window, &ctx, area, names[1]).left(), cells.top()),
            egui::pos2(cells.left(), cells.bottom()),
        );
        let titles = |shapes: &[egui::epaint::ClippedShape]| -> Vec<(String, egui::Pos2)> {
            shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) if header.contains(t.pos) => {
                        Some((t.galley.text().to_owned(), t.pos))
                    }
                    _ => None,
                })
                .collect()
        };
        assert!(!titles(&away).is_empty(), "the title was not found");
        assert_eq!(
            titles(&away),
            titles(&over),
            "the title moved when the controls appeared"
        );
    }

    /// D99 after D96: a move and a press in one batch on an unfocused console's close cannot close
    /// it, because nothing was painted there — nor can one whose controls were painted passes ago
    /// and left; the same press right after a pass under the pointer does. Each case starts from a
    /// fresh window, because a press on a header hands that console the keyboard, and a focused
    /// console's controls are always shown.
    #[test]
    fn a_hidden_close_cannot_be_pressed_until_it_was_painted() {
        for (case, before, closes) in [
            ("never painted", 0, false),
            ("painted, then left for two passes", 3, false),
            ("painted in the previous pass", 1, true),
        ] {
            let ctx = egui::Context::default();
            crate::fonts::install_icons_only(&ctx);
            let mut window = integrated_pair();
            let area = tall_area();
            frame(&mut window, &ctx, area, vec![]);
            let names = names_in_order(&window);
            let close = control(&window, &ctx, area, 1, 12.5);
            let body = pane_rect(&window, &ctx, area, names[1]).center();
            for at in [close, body, body].into_iter().take(before) {
                frame(&mut window, &ctx, area, vec![egui::Event::PointerMoved(at)]);
            }
            assert_eq!(
                window.active().focus,
                names[0],
                "{case}: the console is focused"
            );
            frame(&mut window, &ctx, area, click_at(close));
            frame(&mut window, &ctx, area, vec![]);
            assert_eq!(
                window.active().panes().len(),
                if closes { 1 } else { 2 },
                "{case}"
            );
            if before == 0 {
                // The second click of a double click aimed at what looked like the title: the
                // first one focused the console and its controls are painted now, close included.
                assert!(
                    strokes_in(
                        &frame(&mut window, &ctx, area, vec![]),
                        control_cells(&window, &ctx, area, 1)
                    ) > 0,
                    "the controls are not shown after the first press"
                );
                frame(&mut window, &ctx, area, click_at(close));
                frame(&mut window, &ctx, area, vec![]);
                assert_eq!(
                    window.active().panes().len(),
                    2,
                    "the second click of a double click closed the console"
                );
            }
        }
    }

    /// The second click of a slow double click on hidden controls is the title's too — egui counts
    /// a double click from release to release, so the press that started it was held — and once
    /// the double click is over the same console's controls answer again.
    #[test]
    fn a_double_click_on_hidden_controls_is_the_titles_until_it_is_over() {
        let ctx = egui::Context::default();
        let mut window = integrated_pair();
        let area = tall_area();
        let at_time = |window: &mut Window, time: f64, events: Vec<egui::Event>| {
            ui_pass(
                window,
                &ctx,
                area,
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
            );
        };
        let button = |at, pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        at_time(&mut window, 0.0, vec![]);
        let close = control(&window, &ctx, area, 1, 12.5);
        // Pressed unseen and held for 200 ms, then pressed again 150 ms after the release: 350 ms
        // from press to press, over egui's 300 ms, and still one double click.
        at_time(
            &mut window,
            1.0,
            vec![egui::Event::PointerMoved(close), button(close, true)],
        );
        at_time(&mut window, 1.2, vec![button(close, false)]);
        at_time(&mut window, 1.3, vec![]);
        at_time(
            &mut window,
            1.35,
            vec![button(close, true), button(close, false)],
        );
        at_time(&mut window, 1.4, vec![]);
        assert_eq!(
            window.active().panes().len(),
            2,
            "the second click of a double click closed the console"
        );
        // A double click on the title renames: that is what the person aimed at.
        assert!(
            window.pane_view.is_renaming(),
            "the double click did not reach the title"
        );
        at_time(
            &mut window,
            2.0,
            key_with(egui::Key::Escape, egui::Modifiers::NONE),
        );
        at_time(&mut window, 2.1, vec![]);
        assert!(!window.pane_view.is_renaming());
        at_time(&mut window, 3.0, vec![egui::Event::PointerMoved(close)]);
        at_time(
            &mut window,
            3.1,
            vec![button(close, true), button(close, false)],
        );
        at_time(&mut window, 3.2, vec![]);
        assert_eq!(
            window.active().panes().len(),
            1,
            "the console's close stayed swallowed after the double click was over"
        );
    }

    /// A console too short to split below offers only the split beside it, in the cell next to
    /// the review; a maximized console offers neither.
    #[test]
    fn a_split_that_cannot_fit_is_not_offered_and_a_maximized_console_offers_none() {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = integrated_pair();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 200.0));
        frame(&mut window, &ctx, area, vec![]);
        let names = names_in_order(&window);
        let m = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let short = pane_rect(&window, &ctx, area, names[1]);
        assert!(
            !crate::tiling::can_split_content(
                short,
                crate::tiling::Orientation::Rows,
                &m,
                window.headers
            ) && crate::tiling::can_split_content(
                short,
                crate::tiling::Orientation::Columns,
                &m,
                window.headers
            ),
            "this test needs a console that fits a split beside it and not below it: {short:?}"
        );
        let next_to_review = control(&window, &ctx, area, 1, 137.5);
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(next_to_review)],
        );
        frame(&mut window, &ctx, area, click_at(next_to_review));
        frame(&mut window, &ctx, area, vec![]);
        let after = names_in_order(&window);
        assert_eq!(after.len(), 3);
        let newborn = *after.iter().find(|n| !names.contains(n)).unwrap();
        let born = pane_rect(&window, &ctx, area, newborn);
        let owner = pane_rect(&window, &ctx, area, names[1]);
        assert!(
            (born.top() - owner.top()).abs() < 1.0,
            "a console too short to split below was split below"
        );

        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = integrated_pair();
        let area = tall_area();
        frame(&mut window, &ctx, area, vec![]);
        let zoom = control(&window, &ctx, area, 0, 37.5);
        frame(&mut window, &ctx, area, click_at(zoom));
        assert!(window.pane_view.zoomed(window.active().name).is_some());
        let cells = control_cells(&window, &ctx, area, 0);
        let splits =
            egui::Rect::from_min_max(cells.min, egui::pos2(cells.left() + 50.0, cells.bottom()));
        let shapes = frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(splits.center())],
        );
        assert_eq!(
            strokes_in(&shapes, splits),
            0,
            "a maximized console painted a split"
        );
        for from_right in [162.5, 137.5] {
            let at = control(&window, &ctx, area, 0, from_right);
            frame(&mut window, &ctx, area, click_at(at));
            frame(&mut window, &ctx, area, vec![]);
        }
        assert_eq!(
            window.active().panes().len(),
            2,
            "a maximized console was split"
        );
    }

    #[test]
    fn singleton_controls_follow_split_zoom_close_and_tab_switches() {
        let ctx = egui::Context::default();
        chrome::prepare(&ctx);
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let tab = window.active().name;
        let sibling = window.active().panes()[1].name;
        window.close_a_pane_called(sibling, &ctx).unwrap();
        for width in [1400.0, 400.0] {
            let area = area_of(width);
            frame(&mut window, &ctx, area, vec![]);
            let old_zoom = control(&window, &ctx, area, 0, 37.5);
            frame(&mut window, &ctx, area, click_at(old_zoom));
            assert_eq!(window.pane_view.zoomed(tab), None);
            frame(
                &mut window,
                &ctx,
                area,
                key_with(egui::Key::Escape, egui::Modifiers::NONE),
            );
            frame(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        let area = area_of(1400.0);
        let pane = window.active().focus;
        window
            .pane_view
            .open_menu(tab, pane, false, egui::pos2(200.0, 150.0));
        frame(&mut window, &ctx, area, vec![]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        assert!(has_text(&shapes, "Rename"));
        assert!(!has_text(&shapes, "Maximize / restore"));
        frame(
            &mut window,
            &ctx,
            area,
            key_with(egui::Key::Escape, egui::Modifiers::NONE),
        );
        frame(&mut window, &ctx, area, vec![]);
        assert!(
            !window.pane_view.is_modal(),
            "menu remained modal after Escape"
        );
        frame(&mut window, &ctx, area, chord_with(egui::Key::D));
        assert_eq!(window.active().panes().len(), 2);
        let original = window.active().root.describe();
        let zoom = control(&window, &ctx, area, 0, 37.5);
        frame(&mut window, &ctx, area, click_at(zoom));
        assert_eq!(window.pane_view.zoomed(tab), Some(pane));
        frame(&mut window, &ctx, area, chord_with(egui::Key::ArrowRight));
        assert_ne!(window.active().name, tab);
        frame(&mut window, &ctx, area, chord_with(egui::Key::ArrowLeft));
        assert_eq!(window.active().name, tab);
        assert_eq!(window.pane_view.zoomed(tab), Some(pane));
        let restore = control(&window, &ctx, area, 0, 37.5);
        frame(&mut window, &ctx, area, click_at(restore));
        assert_eq!(window.pane_view.zoomed(tab), None);
        assert_eq!(window.active().root.describe(), original);
        let zoom = control(&window, &ctx, area, 0, 37.5);
        frame(&mut window, &ctx, area, click_at(zoom));
        let hidden = window.active().panes()[1].name;
        window.close_a_pane_called(hidden, &ctx).unwrap();
        assert_eq!(window.pane_view.zoomed(tab), None);
        frame(&mut window, &ctx, area, the_chord_again(egui::Key::D));
        assert_eq!(window.active().panes().len(), 2);
        assert_eq!(window.pane_view.zoomed(tab), None);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        for pane in window.active().panes() {
            assert!(has_text(&shapes, &format!("Console {}", pane.name + 1)));
        }
    }

    #[test]
    fn singleton_and_multi_pane_tab_switches_update_zoom_control_immediately() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        let singleton = window.active().name;
        let sibling = window.active().panes()[1].name;
        window.close_a_pane_called(sibling, &ctx).unwrap();
        let multi = window.tabs[1].name;

        frame(&mut window, &ctx, area, vec![]);
        frame(&mut window, &ctx, area, chord_with(egui::Key::ArrowRight));
        assert_eq!(window.active().name, multi);
        let pane = window.active().focus;
        let zoom = control(&window, &ctx, area, 0, 37.5);
        frame(&mut window, &ctx, area, click_at(zoom));
        assert_eq!(window.pane_view.zoomed(multi), Some(pane));

        frame(&mut window, &ctx, area, chord_with(egui::Key::ArrowLeft));
        assert_eq!(window.active().name, singleton);
        assert_eq!(window.pane_view.zoomed(multi), Some(pane));
        let former_zoom = control(&window, &ctx, area, 0, 37.5);
        frame(&mut window, &ctx, area, click_at(former_zoom));
        assert_eq!(window.pane_view.zoomed(singleton), None);
        frame(
            &mut window,
            &ctx,
            area,
            key_with(egui::Key::Escape, egui::Modifiers::NONE),
        );
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(&mut window, &ctx, area, chord_with(egui::Key::ArrowRight));
        assert_eq!(window.active().name, multi);
        assert_eq!(window.pane_view.zoomed(multi), Some(pane));
        let restore = control(&window, &ctx, area, 0, 37.5);
        frame(&mut window, &ctx, area, click_at(restore));
        assert_eq!(window.pane_view.zoomed(multi), None);
    }

    #[test]
    fn inactive_removals_clear_zoom_before_a_later_split() {
        for detach in [false, true] {
            let ctx = egui::Context::default();
            let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let tab = window.tabs[1].name;
            let survivor = window.tabs[1].focus;
            let hidden = window.tabs[1]
                .panes()
                .iter()
                .find(|p| p.name != survivor)
                .unwrap()
                .name;
            window.pane_view.toggle(tab, survivor, 2);
            if detach {
                drop(window.detach_panes_called(&[hidden], &ctx).unwrap());
            } else {
                window.close_a_pane_called(hidden, &ctx).unwrap();
            }
            assert_eq!(window.pane_view.zoomed(tab), None);
            window.active = 1;
            let area = area_of(1400.0);
            frame(&mut window, &ctx, area, chord_with(egui::Key::D));
            assert_eq!(window.active().panes().len(), 2);
            assert_eq!(window.pane_view.zoomed(tab), None);
        }
    }

    #[test]
    fn hidden_sibling_keeps_original_grid_and_its_unpainted_round_is_closed() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let hidden_grid = window.active().panes()[1].measured;
        let name = window.active().focus;
        let tab = window.active().name;
        window.pane_view.toggle(tab, name, 2);
        let (tx, rx) = std::sync::mpsc::channel();
        let ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal = ready.clone();
        window.active_mut().panes_mut()[1].pty_output = PtyOutput::new(rx, move || {
            signal.store(true, std::sync::atomic::Ordering::Release);
        });
        tx.send(vec![b'x'; PUMP_BUDGET * 2]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.load(std::sync::atomic::Ordering::Acquire) {
            assert!(
                Instant::now() < deadline,
                "output bridge did not queue the fixture"
            );
            std::thread::yield_now();
        }
        window.pump_them_all();
        assert_eq!(
            window.active().panes()[1].received.len(),
            PUMP_BUDGET,
            "maximizing reduced the hidden console's pump budget"
        );
        frame(&mut window, &ctx, area, vec![]);
        let panes = window.active().panes();
        assert_eq!(panes[1].measured, hidden_grid);
        assert!(panes[0].measured.cols > hidden_grid.cols);
        assert!(
            panes[1].pending_measurement.is_none(),
            "hidden output work escaped frame accounting"
        );
        assert!(
            panes[1].painted_console.is_none(),
            "hidden console retained clickable pixels"
        );
        let mut report = Vec::new();
        window.chrome.publish(&mut report).unwrap();
        let report = String::from_utf8(report).unwrap();
        assert!(report.contains("hidden=1"));
    }

    #[test]
    fn panel_button_targets_only_its_console_and_preserves_the_document() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        *window.active_mut().panes_mut()[0] =
            pane_with_panel(r#"{"lines":["Keep this document"]}"#);
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let at = control(&window, &ctx, area, 0, 62.5);
        frame(&mut window, &ctx, area, click_at(at));
        assert!(window.active().panes()[0].folded);
        assert!(!window.active().panes()[1].folded);
        let at = control(&window, &ctx, area, 0, 62.5);
        let shapes = frame(&mut window, &ctx, area, click_at(at));
        assert!(!window.active().panes()[0].folded);
        assert!(the_text_queued_up(&shapes, "Keep this document"));
    }

    #[test]
    fn explicit_label_is_painted_and_survives_the_session_round_trip() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        window.active_mut().panes_mut()[1].label = Some("Review".to_owned());
        let shapes = frame(&mut window, &ctx, area_of(1400.0), vec![]);
        assert!(the_text_queued_up(&shapes, "Review"));
        let document = session::StoredDocument {
            v: session::VERSION,
            tabs: vec![session::StoredTab {
                label: None,
                root: window.active().root.describe(),
            }],
        };
        let written = session::serialize(&document).unwrap();
        assert!(written.contains("Review"));
        let read: session::StoredDocument = serde_json::from_str(&written).unwrap();
        assert_eq!(read, document);
    }
    #[test]
    fn a_header_search_opens_its_consoles_search_and_menu_escape_is_owned() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let search = control(&window, &ctx, area, 0, 112.5);
        frame(&mut window, &ctx, area, click_at(search));
        assert!(window.active().panes()[0].search.is_open());
        assert!(!window.active().panes()[1].search.is_open());
        let menu = slots(area).menu.center();
        frame(&mut window, &ctx, area, click_at(menu));
        let shapes = frame(&mut window, &ctx, area, vec![]);
        assert!(
            has_text(&shapes, "Preferences"),
            "chrome {:?}; menu text: {:?}",
            window.chrome,
            shapes
                .iter()
                .filter_map(|s| if let egui::Shape::Text(t) = &s.shape {
                    Some(t.galley.text())
                } else {
                    None
                })
                .collect::<Vec<_>>()
        );
        let shapes = frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::Text("not shell input".into()),
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(
            window
                .all_panes_mut()
                .all(|pane| pane.keystrokes.is_empty())
        );
        // Menu painting precedes dismissal; the next pass must no longer show it.
        let _ = shapes;
        let shapes = frame(&mut window, &ctx, area, vec![]);
        assert!(!has_text(&shapes, "Preferences"));
    }
    #[test]
    fn tab_controls_own_the_press_until_release_and_keep_selection_visible() {
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for target in ["body", "close", "add"] {
            let ctx = egui::Context::default();
            let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = area_of(800.0);
            frame(&mut window, &ctx, area, vec![]);
            let at = tab_target(&window, &ctx, area, 1, target);
            // An inactive tab's cross exists once the pointer has been over its card for a pass.
            frame(&mut window, &ctx, area, vec![egui::Event::PointerMoved(at)]);
            let pressed = frame(&mut window, &ctx, area, vec![button(at, true)]);
            assert_eq!(window.active, 0, "{target} selected on press");
            assert_eq!(window.tabs.len(), 2, "{target} acted on press");
            assert!(pressed.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == ACTIVE_TAB && r.rect.height() == 34.0)), "the selected tab lost its surface");
            let away = egui::pos2(50.0, 200.0);
            frame(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::PointerMoved(away)],
            );
            frame(&mut window, &ctx, area, vec![button(away, false)]);
            assert_eq!(window.active, 0);
            assert_eq!(window.tabs.len(), 2, "cancelled {target} acted");
            frame(&mut window, &ctx, area, vec![egui::Event::PointerMoved(at)]);
            frame(&mut window, &ctx, area, vec![button(at, true)]);
            frame(&mut window, &ctx, area, vec![button(at, false)]);
            match target {
                "body" => {
                    assert_eq!(window.active, 1);
                    assert_eq!(window.tabs.len(), 2);
                }
                "close" => {
                    assert_eq!(window.tabs.len(), 1);
                    assert_eq!(window.tabs[0].name, 0);
                }
                _ => {
                    assert_eq!(window.tabs.len(), 3);
                    assert_eq!(window.active, 2);
                }
            }
        }
    }

    #[test]
    fn a_held_tab_press_cannot_activate_another_target() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(800.0);
        frame(&mut window, &ctx, area, vec![]);
        let body = tab_target(&window, &ctx, area, 1, "body");
        let close = tab_target(&window, &ctx, area, 1, "close");
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(body),
                egui::Event::PointerButton {
                    pos: body,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(close)],
        );
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerButton {
                pos: close,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(window.active, 0);
        assert_eq!(window.tabs.len(), 2);
    }

    #[test]
    fn held_press_cannot_transfer_after_a_preceding_tab_is_removed() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 3]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1016.0);
        frame(&mut window, &ctx, area, vec![]);
        let at = tab_target(&window, &ctx, area, 1, "body");
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(at), button(true)],
        );
        // A keyboard close can remove the active predecessor while the pointer remains held.
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        window.apply(WindowRequest::CloseTab, &ctx, area, &metrics);
        let held = frame(&mut window, &ctx, area, vec![]);
        assert!(!held.iter().any(|s| matches!(&s.shape,
            egui::Shape::Rect(r) if r.rect.contains(at) && r.fill == egui::Color32::from_rgb(0x37, 0x3b, 0x3d)
        )), "held feedback transferred to the next tab at the same index");
        frame(&mut window, &ctx, area, vec![button(false)]);
        assert_eq!(window.tabs.len(), 2);
        assert_eq!(
            window.tabs[window.active].name, 1,
            "release selected the successor at the old index"
        );
    }

    #[test]
    fn tabs_keep_the_active_surface_and_distinct_hover_and_held_feedback() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(800.0);
        ui_pass(&mut window, &ctx, area, Default::default());
        let at = tab_target(&window, &ctx, area, 1, "body");
        let card = title_card(&window, &ctx, area, 1);
        let open = title_card(&window, &ctx, area, 0);
        let open_at = tab_target(&window, &ctx, area, 0, "body");
        let mut pass = |time, events| {
            ui_pass(
                &mut window,
                &ctx,
                area,
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
            )
        };
        let baseline = pass(0.0, vec![]);
        pass(0.01, vec![egui::Event::PointerMoved(at)]);
        let hover = pass(0.14, vec![]);
        let held = pass(
            0.15,
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let fills = |shapes: &[egui::epaint::ClippedShape]| {
            shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    // The card's own surfaces, not the title bar's row behind them.
                    egui::Shape::Rect(r) if r.rect.contains(at) && r.rect.height() <= 34.0 => {
                        Some(r.fill)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        // An idle inactive tab is a card of its own, Ivy Night's `tab`; the hover surface and the
        // held one are its `hover` and `line2`, painted over that card. All written out.
        let resting = egui::Color32::from_rgb(0x0b, 0x0d, 0x0f);
        assert_eq!(
            fills(&baseline),
            vec![resting],
            "an idle inactive tab has no card of its own"
        );
        assert_eq!(
            fills(&hover),
            vec![resting, egui::Color32::from_rgb(0x26, 0x2b, 0x2d)]
        );
        // The whole card, cross included, short of the bar's bottom line.
        assert!(hover.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r)
            if r.fill == egui::Color32::from_rgb(0x26, 0x2b, 0x2d)
                && r.rect == egui::Rect::from_min_max(card.min, egui::pos2(card.right(), card.bottom() - 1.0)))));
        assert_eq!(
            fills(&held),
            vec![resting, egui::Color32::from_rgb(0x37, 0x3b, 0x3d)]
        );
        // A press held on the open tab itself: its `line2` surface must not cover the accent. The
        // earlier press is let go over the consoles, where releasing it is no click on a tab.
        let away = area.center();
        pass(
            0.3,
            vec![
                egui::Event::PointerMoved(away),
                egui::Event::PointerButton {
                    pos: away,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let held_open = pass(
            0.45,
            vec![
                egui::Event::PointerMoved(open_at),
                egui::Event::PointerButton {
                    pos: open_at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        // Ivy Night's accent, written out: the open card's own rounded shape, clipped to a
        // 2-point band along its top edge, so the line follows the card's corners.
        let accent = egui::Color32::from_rgb(0x3c, 0xc3, 0x84);
        let band = egui::Rect::from_min_max(open.min, egui::pos2(open.right(), open.top() + 2.0));
        let accent_line = |shapes: &[egui::epaint::ClippedShape]| {
            shapes.iter().position(|s| {
                matches!(&s.shape, egui::Shape::Rect(r) if r.fill == accent && r.rect == open)
                    && s.clip_rect == band
            })
        };
        let held_on_open = held_open.iter().position(|s| {
            matches!(&s.shape, egui::Shape::Rect(r)
                if r.fill == egui::Color32::from_rgb(0x37, 0x3b, 0x3d) && r.rect.intersects(open))
        });
        assert!(
            held_on_open.is_some(),
            "the press on the open tab painted no held surface"
        );
        assert!(
            accent_line(&held_open) > held_on_open,
            "the held surface covers the open tab's accent line"
        );
        for shapes in [&baseline, &hover, &held, &held_open] {
            assert!(
                shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Rect(r) if r.fill == ACTIVE_TAB && r.rect.height() == 34.0
                )),
                "the selected tab lost its surface"
            );
            assert!(
                accent_line(shapes).is_some(),
                "the open tab has no accent line along its top edge"
            );
            assert!(
                !shapes.iter().any(|s| matches!(&s.shape,
                    egui::Shape::Rect(r) if r.fill == accent && r.rect.intersects(card)
                )),
                "an inactive tab carries the accent line"
            );
        }
        assert_eq!(window.active, 0);
    }

    #[test]
    fn narrow_tab_labels_select_without_invisible_close_targets() {
        for width in [240.0, 200.0] {
            let ctx = egui::Context::default();
            let mut window =
                window_of_tabs(vec![["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = area_of(width);
            frame(&mut window, &ctx, area, vec![]);
            let m = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
            let band = tab_bar::geometry(&window, area, &m).cards.unwrap();
            let card = tab_bar::title_card(band, 1, 2);
            assert!(card.width() > 20.0 && card.width() < 65.0, "{card:?}");
            // The right-hand part of a narrow card used to contain a close hitbox without a glyph.
            let at = egui::pos2(card.right() - 1.5, card.center().y);
            frame(&mut window, &ctx, area, click_at(at));
            assert_eq!(
                window.tabs.len(),
                2,
                "selecting a tab terminated its consoles"
            );
            assert_eq!(window.active, 1, "a narrow tab could not be selected");
        }
    }

    #[test]
    fn integrated_partition_gaps_use_the_app_background_and_headers_rename_by_right_click() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let m = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let work = tab_bar::geometry(&window, area, &m).consoles;
        let dividers = window.active().root.place(work, m.width()).dividers;
        let shapes = frame(&mut window, &ctx, area, vec![]);
        for divider in dividers {
            let fills: Vec<_> = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Rect(r) if r.rect == divider.rect => Some(r.fill),
                    _ => None,
                })
                .collect();
            assert!(!fills.is_empty(), "no partition gap painted");
            assert!(
                fills
                    .iter()
                    .all(|color| *color == window.theme.background())
            );
        }
        let pane = pane_view::view_tiling(&window, work, &m).parts[0];
        let at = pane.left_top() + egui::vec2(60.0, 16.0);
        let events = [true, false]
            .into_iter()
            .map(|pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            })
            .collect();
        frame(&mut window, &ctx, area, events);
        frame(&mut window, &ctx, area, vec![]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        assert!(has_text(&shapes, "Rename"));
        assert!(!has_text(&shapes, "Console actions"));
        assert!(!has_text(&shapes, "Cancel"));
        assert!(!has_text(&shapes, "Split down"));
        let popup = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("console-context-menu")))
            .unwrap();
        let split = super::super::context_clipboard::split_menu_targets(&shapes, popup)[0].center();
        frame(&mut window, &ctx, area, click_at(split));
        assert_eq!(
            window.active().panes().len(),
            3,
            "context menu lost its split action"
        );
        assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
    }

    /// A tab dot's own radius: a ring is anything drawn beyond it.
    const DOT_RADIUS: f32 = 3.0;

    /// The rings a tab card painted around its dots, held or not: 1-point strokes with no fill,
    /// wider than a dot, centred in the card — centre, radius, colour, width.
    fn rings_in(
        shapes: &[egui::epaint::ClippedShape],
        card: egui::Rect,
    ) -> Vec<(egui::Pos2, f32, egui::Color32, f32)> {
        shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Circle(c)
                    if c.stroke.width == 1.0
                        && c.radius > DOT_RADIUS
                        && c.fill == egui::Color32::TRANSPARENT
                        && card.contains(c.center) =>
                {
                    Some((c.center, c.radius, c.stroke.color, c.stroke.width))
                }
                _ => None,
            })
            .collect()
    }

    /// Ivy Night's three claim colours, written out: yellow wait, red block, and done **blue** —
    /// green is the accent (D94), and the owner kept done blue against the mock (D96 amended).
    const WAIT: egui::Color32 = egui::Color32::from_rgb(0xf2, 0xb8, 0x4b);
    const BLOCK: egui::Color32 = egui::Color32::from_rgb(0xf2, 0x7a, 0x66);
    const DONE: egui::Color32 = egui::Color32::from_rgb(0x72, 0xb7, 0xf2);

    /// Ivy Night's muted text (`tx3`): the outline of a dot with no project, and a summary dot.
    const MUTED: egui::Color32 = egui::Color32::from_rgb(0x8e, 0x93, 0x90);

    /// A project badge in `ink`, as the project view hands one to a pane.
    fn badge(ink: egui::Color32) -> crate::project_view::Badge {
        crate::project_view::Badge {
            key: crate::project::Key::Repository("/work/backend".into()),
            name: "Backend".into(),
            details: "/work/backend".into(),
            ink,
            painted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Whether a dot of radius 3 filled with `ink` sits at `at`.
    fn filled_dot_at(
        shapes: &[egui::epaint::ClippedShape],
        at: egui::Pos2,
        ink: egui::Color32,
    ) -> bool {
        shapes.iter().any(|s| {
            matches!(&s.shape, egui::Shape::Circle(c) if c.center == at
                && c.radius == DOT_RADIUS && c.fill == ink)
        })
    }

    /// Whether the hollow 1-point outline in `ink` of a console with no project sits at `at`.
    fn hollow_dot_at(
        shapes: &[egui::epaint::ClippedShape],
        at: egui::Pos2,
        ink: egui::Color32,
    ) -> bool {
        shapes.iter().any(|s| {
            matches!(&s.shape, egui::Shape::Circle(c) if c.center == at
                && c.radius < DOT_RADIUS && c.fill == egui::Color32::TRANSPARENT
                && c.stroke.width == 1.0 && c.stroke.color == ink)
        })
    }

    /// Each console's claim rides on its own dot, in tree order, and nothing is painted left of the
    /// label: the first tab's consoles have a project, the first waits and the second claims
    /// nothing; the second tab's consoles have none and are blocked and done. A ring is centred on
    /// its dot, 5.25 points out, the dot under it keeps its own fill or outline, and the bar counts
    /// the two cards that painted one.
    #[test]
    fn each_consoles_claim_is_a_ring_on_its_own_dot() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let ink = egui::Color32::from_rgb(224, 175, 104);
        publish_in_the_tab(&mut window, 0, 0, r#"{"attention":"wait","lines":["x"]}"#);
        publish_in_the_tab(&mut window, 1, 0, r#"{"attention":"block","lines":["x"]}"#);
        publish_in_the_tab(&mut window, 1, 1, r#"{"attention":"done","lines":["x"]}"#);
        for pane in window.tabs[0].panes_mut() {
            pane.project = Some(badge(ink));
        }
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        for (which, project, expected) in [
            (0, Some(ink), vec![(0, WAIT)]),
            (1, None, vec![(0, BLOCK), (1, DONE)]),
        ] {
            let card = title_card(&window, &ctx, area, which);
            // Laid out with the cross's room whether or not it shows, as every card with a sibling.
            let (label, dots, summary) = tab_bar::title_label_dots_summary(card, 2, true);
            assert_eq!(summary, None);
            let dots = dots.expect("a full-width card holds two dots");
            let center =
                |n: usize| egui::pos2(dots.left() + 6.0 + 16.0 * n as f32, dots.center().y);
            for n in 0..2 {
                let kept = match project {
                    Some(ink) => filled_dot_at(&shapes, center(n), ink),
                    None => hollow_dot_at(&shapes, center(n), MUTED),
                };
                assert!(kept, "tab {which}: dot {n} lost its own fill");
            }
            let rings = rings_in(&shapes, card);
            assert_eq!(rings.len(), expected.len(), "tab {which}: {rings:?}");
            for ((at, radius, color, width), (n, class)) in rings.into_iter().zip(expected) {
                assert_eq!(color, class, "tab {which}, dot {n}");
                assert_eq!(radius, 5.25, "tab {which}, dot {n}");
                assert_eq!(width, 1.0, "tab {which}, dot {n}: a held claim's ring");
                assert_eq!(at, center(n), "tab {which}: the ring is not on dot {n}");
            }
            // No pill and no muted dot where the pill used to be — the card's first 36 points,
            // its padding, the 18-point pill and its gap — and the painted label starts at the
            // padding. Only the label's text and the card-wide surfaces may touch that slot.
            assert_eq!(label.left(), card.left() + 10.0, "tab {which}");
            let text = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if card.contains(t.pos) => Some(t.pos.x),
                    _ => None,
                })
                .expect("the tab's label is painted");
            assert_eq!(text, card.left() + 10.0, "tab {which}: the label moved");
            let old_slot =
                egui::Rect::from_min_max(card.min, egui::pos2(card.left() + 36.0, card.bottom()));
            let stray: Vec<_> = shapes
                .iter()
                .filter(|s| !matches!(s.shape, egui::Shape::Text(_)))
                .map(|s| s.shape.visual_bounding_rect())
                .filter(|r| r.width() <= 18.0 && r.intersects(old_slot))
                .collect();
            assert!(
                stray.is_empty(),
                "tab {which}: painted in the old pill's slot: {stray:?}"
            );
        }
        assert_eq!(window.tab_bar_cost.marked_tabs, 2);
    }

    /// A claim nobody holds up draws the same whole ring as a held one: the class's full colour,
    /// 1 point, 5.25 out — not dimmed (the owner read the dimmed red as brown) and not dashed
    /// (the owner asked for a continuous ring) at the desktop check (D96).
    #[test]
    fn an_unheld_claims_ring_is_the_same_whole_ring_in_its_full_colour() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        publish_in_the_tab(&mut window, 1, 0, r#"{"attention":"block","lines":["x"]}"#);
        age_the_panel(&mut window, 1, 0);
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        // The premise: nobody holds this claim up, or the test proves nothing about it.
        assert!(
            window.tabs[1]
                .attention(std::time::SystemTime::now())
                .is_some_and(|c| !c.backed)
        );
        let card = title_card(&window, &ctx, area, 1);
        let rings = rings_in(&shapes, card);
        assert_eq!(
            rings.iter().map(|r| (r.1, r.2, r.3)).collect::<Vec<_>>(),
            vec![(5.25, BLOCK, 1.0)]
        );
        // The control inside: the first card paints its dots and claims nothing, so it is not marked.
        assert!(rings_in(&shapes, title_card(&window, &ctx, area, 0)).is_empty());
        assert_eq!(window.tab_bar_cost.marked_tabs, 1);
    }

    /// A card without room for both of its consoles' dots shows a single muted dot ringed with the
    /// loudest claim of the two — wait over block — and counts as marked; the cards that claim
    /// nothing show neither the ring nor the muted dot (D96 amended). Run at a card of 80–96 points
    /// and at the narrowest window, where the card is under the cross's threshold: the summary is
    /// there at both, so a claim never blinks out while the window narrows.
    #[test]
    fn a_tab_without_room_for_its_dots_rings_one_dot_with_its_loudest_claim() {
        let ctx = egui::Context::default();
        let mut probe = window_of_tabs([["sleep 30", "sleep 30"]; 3]);
        probe.chrome.enable();
        probe.headers = probe.headers.integrated(true);
        // The fonts the geometry measures with exist after a first pass.
        frame(&mut probe, &ctx, area_of(1400.0), vec![]);
        let between = (500..1400)
            .map(|w| area_of(w as f32))
            .find(|area| (80.0..96.0).contains(&title_card(&probe, &ctx, *area, 0).width()))
            .expect("some window width gives a card between 80 and 96 points");
        for area in [between, area_of(240.0)] {
            // A window that has only ever been this narrow: the bar's count is a running maximum,
            // and a wide frame first would have marked the card before the summary had to.
            let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 3]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            publish_in_the_tab(&mut window, 0, 0, r#"{"attention":"block","lines":["x"]}"#);
            publish_in_the_tab(&mut window, 0, 1, r#"{"attention":"wait","lines":["x"]}"#);
            frame(&mut window, &ctx, area, vec![]);
            let shapes = frame(&mut window, &ctx, area, vec![]);
            let card = title_card(&window, &ctx, area, 0);
            let (_, dots, summary) = tab_bar::title_label_dots_summary(card, 2, true);
            assert_eq!(dots, None, "{card:?}");
            let summary = summary.expect("the card keeps one dot's slot");
            let center = egui::pos2(summary.left() + 6.0, summary.center().y);
            assert!(
                filled_dot_at(&shapes, center, MUTED),
                "{card:?}: the summary dot is not the muted one"
            );
            assert_eq!(rings_in(&shapes, card), vec![(center, 5.25, WAIT, 1.0)]);
            for other in 1..3 {
                let card = title_card(&window, &ctx, area, other);
                assert!(rings_in(&shapes, card).is_empty());
                let (_, _, slot) = tab_bar::title_label_dots_summary(card, 2, true);
                let slot = slot.expect("the same width keeps the same slot");
                let at = egui::pos2(slot.left() + 6.0, slot.center().y);
                assert!(
                    !filled_dot_at(&shapes, at, MUTED),
                    "a card that claims nothing painted the summary dot"
                );
            }
            assert_eq!(window.tab_bar_cost.marked_tabs, 1, "{card:?}");
        }
    }

    /// The tabs share the title bar's row, so the consoles start 40 points down instead of the
    /// 88 the separate tab row used to cost (49 of title bar plus 39 of tabs).
    #[test]
    fn tabs_live_in_the_title_bar_and_the_consoles_get_the_old_tab_row_back() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let m = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let geometry = tab_bar::geometry(&window, area, &m);
        let cards = geometry.cards.unwrap();
        assert_eq!(cards.bottom(), area.top() + 40.0);
        assert_eq!(cards.height(), 34.0);
        assert_eq!(geometry.consoles.top(), area.top() + 40.0);
    }

    /// Between the last tab and the right-hand buttons the bar still drags and maximizes the
    /// window; on a tab the same double click renames instead.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_title_bar_still_drags_between_the_tabs_and_its_buttons() {
        let double_click = |at| {
            let mut events = click_at(at);
            events.extend(click_at(at));
            events
        };
        for place in ["gap", "tab", "above"] {
            let on_tab = place == "tab";
            let ctx = egui::Context::default();
            let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = area_of(1400.0);
            frame(&mut window, &ctx, area, vec![]);
            let at = match place {
                "tab" => tab_target(&window, &ctx, area, 1, "body"),
                // The six points over a tab, under the window's resize edge.
                "above" => egui::pos2(
                    tab_target(&window, &ctx, area, 1, "body").x,
                    area.top() + 5.5,
                ),
                _ => {
                    let add = tab_target(&window, &ctx, area, 0, "add");
                    egui::pos2(add.x + 40.0, area.top() + 20.0)
                }
            };
            let input = egui::RawInput {
                screen_rect: Some(area),
                events: double_click(at),
                ..Default::default()
            };
            let mut native_frame = eframe::Frame::_new_kittest();
            let output = ctx.run_ui(input, |ui| {
                eframe::App::ui(&mut window, ui, &mut native_frame)
            });
            let maximized = output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&egui::ViewportCommand::Maximized(true));
            output.drop_without_applying_deltas();
            assert_eq!(maximized, !on_tab, "{place}");
            assert_eq!(window.tabs.len(), 2);
            assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn window_buttons_emit_native_commands_from_real_clicks() {
        for (from_right, maximized, expected) in [
            (91.0, false, egui::ViewportCommand::Minimized(true)),
            (57.0, false, egui::ViewportCommand::Maximized(true)),
            (57.0, true, egui::ViewportCommand::Maximized(false)),
            (23.0, false, egui::ViewportCommand::Close),
        ] {
            let ctx = egui::Context::default();
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = area_of(1400.0);
            frame(&mut window, &ctx, area, vec![]);
            let at = egui::pos2(area.right() - from_right, area.top() + 24.0);
            let mut input = egui::RawInput {
                screen_rect: Some(area),
                events: click_at(at),
                ..Default::default()
            };
            input
                .viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .maximized = Some(maximized);
            let mut native_frame = eframe::Frame::_new_kittest();
            let output = ctx.run_ui(input, |ui| {
                eframe::App::ui(&mut window, ui, &mut native_frame)
            });
            assert!(
                output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .contains(&expected),
                "titlebar click did not request {expected:?}"
            );
            output.drop_without_applying_deltas();
            assert!(
                window
                    .all_panes_mut()
                    .all(|pane| pane.keystrokes.is_empty())
            );
        }
    }

    #[test]
    fn console_dialogs_paint_the_effective_colors_after_theme_changes() {
        fn dialog_surface(shape: &egui::Shape, surface: egui::Color32) -> bool {
            match shape {
                egui::Shape::Rect(r) => {
                    r.fill == surface && r.rect.height() > 80.0 && r.rect.width() > 100.0
                }
                egui::Shape::Vec(shapes) => shapes.iter().any(|s| dialog_surface(s, surface)),
                _ => false,
            }
        }
        for rename in [false, true] {
            let ctx = egui::Context::default();
            // Inspect final colors, without egui's time-dependent window fade multiplying alpha.
            ctx.global_style_mut(|style| style.animation_time = 0.0);
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = area_of(1400.0);
            let tab = window.active().name;
            let pane = window.active().focus;
            let heading = if rename {
                window.pane_view.begin_rename(tab, pane, "Review".into());
                "Rename console"
            } else {
                window
                    .pane_view
                    .open_menu(tab, pane, true, egui::pos2(400.0, 300.0));
                "Paste"
            };
            // Floating on the theme's `raised` role (D98), with its `tx` ink — which turns white
            // when an override leaves the foreground unreadable over the background (D94).
            for (config, surface, ink) in [
                (
                    "panel=#112233\nforeground=#efcba9",
                    egui::Color32::from_rgb(0x20, 0x23, 0x24),
                    egui::Color32::from_rgb(0xef, 0xcb, 0xa9),
                ),
                (
                    "theme=gruvbox-dark\npanel=#e2d4c6\nforeground=#241c14",
                    // Raised steps toward the readable (white) text, not the unreadable foreground.
                    egui::Color32::from_rgb(0x35, 0x35, 0x35),
                    egui::Color32::WHITE,
                ),
            ] {
                let (settings, warnings) = settings::interpret(config).unwrap();
                assert!(warnings.kept().is_empty());
                settings.apply_to(&mut window);
                // A new egui window first measures itself before painting visibly.
                frame(&mut window, &ctx, area, vec![]);
                let shapes = frame(&mut window, &ctx, area, vec![]);
                assert!(
                    shapes.iter().any(|s| dialog_surface(&s.shape, surface)),
                    "{heading} is not on the theme's raised surface"
                );
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t)
                    if t.galley.text() == heading && t.galley.job.sections.iter().all(|section| {
                        // A label with no colour of its own is painted in the shape's fallback.
                        let color = if section.format.color == egui::Color32::PLACEHOLDER {
                            t.fallback_color
                        } else {
                            section.format.color
                        };
                        color == ink
                    }))),
                    "{heading} ignored the configured heading foreground");
            }
        }
    }

    fn text_center(shapes: &[egui::epaint::ClippedShape], text: &str) -> egui::Pos2 {
        shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == text => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{text:?} was not painted"))
    }

    #[test]
    fn tab_pointer_rename_saves_a_painted_session_label_without_console_input() {
        for button in [egui::PointerButton::Primary, egui::PointerButton::Secondary] {
            let ctx = egui::Context::default();
            ctx.global_style_mut(|style| style.animation_time = 0.0);
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0));
            let shapes = frame(&mut window, &ctx, area, vec![]);
            let at = text_center(&shapes, "Terminal 1");
            let click_count = if button == egui::PointerButton::Primary {
                2
            } else {
                1
            };
            for click in 0..click_count {
                for (release_offset, pressed) in [(0.0, true), (0.04, false)] {
                    ui_pass(
                        &mut window,
                        &ctx,
                        area,
                        egui::RawInput {
                            time: Some(1.0 + click as f64 * 0.1 + release_offset),
                            events: vec![
                                egui::Event::PointerMoved(at),
                                egui::Event::PointerButton {
                                    pos: at,
                                    button,
                                    pressed,
                                    modifiers: egui::Modifiers::NONE,
                                },
                            ],
                            ..Default::default()
                        },
                    );
                }
            }
            frame(&mut window, &ctx, area, vec![]);
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(
                has_text(&shapes, "Rename tab"),
                "tab gesture did not open its rename editor"
            );
            let modifiers = egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            let mut events: Vec<_> = [true, false]
                .into_iter()
                .map(|pressed| egui::Event::Key {
                    key: egui::Key::A,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                })
                .collect();
            events.push(egui::Event::Text("Release review".into()));
            let shapes = frame(&mut window, &ctx, area, events);
            let save = text_center(&shapes, "Save");
            frame(&mut window, &ctx, area, click_at(save));
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(!has_text(&shapes, "Rename tab"));
            assert!(has_text(&shapes, "Release review"));
            let stored = window.active().describe();
            let round_trip: session::StoredTab =
                serde_json::from_str(&serde_json::to_string(&stored).unwrap()).unwrap();
            assert_eq!(round_trip.label.as_deref(), Some("Release review"));
            assert!(
                window
                    .all_panes_mut()
                    .all(|pane| pane.keystrokes.is_empty())
            );
        }
    }

    #[test]
    fn global_menu_keeps_requested_commands_and_opens_shortcut_settings() {
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        window.settings_editor = settings::editor::Editor::resolved(settings::Entry::Normal, None);
        *window.active_mut().panes_mut()[0] = pane_with_panel(r#"{"lines":["Visible document"]}"#);
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0));
        frame(&mut window, &ctx, area, vec![]);
        let menu = slots(area).menu.center();
        frame(&mut window, &ctx, area, click_at(menu));
        let shapes = frame(&mut window, &ctx, area, vec![]);
        for label in [
            "Check for updates",
            "What's new",
            "Preferences",
            "Keyboard shortcuts",
            "Connect agents",
            "Search console",
        ] {
            assert!(has_text(&shapes, label), "menu omitted {label}");
        }
        for label in [
            "New tab",
            "Split right",
            "Split down",
            "Toggle information panel",
            "Markdown viewer",
            "Git diff viewer",
            "Board",
        ] {
            // The title bar's own labelled Board button is not the menu.
            let in_menu = shapes.iter().any(|shape| {
                matches!(&shape.shape, egui::Shape::Text(text)
                    if text.galley.text() == label
                        && text.pos.y > crate::theme::metrics::TITLE_BAR_HEIGHT)
            });
            assert!(!in_menu, "menu retained {label}");
        }
        let notes = text_center(&shapes, "What's new").y;
        let check = text_center(&shapes, "Check for updates").y;
        let preferences = text_center(&shapes, "Preferences").y;
        assert!(
            preferences - notes > notes - check,
            "preferences group has no separation"
        );
        let at = text_center(&shapes, "Keyboard shortcuts");
        frame(&mut window, &ctx, area, click_at(at));
        frame(&mut window, &ctx, area, vec![]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        assert!(window.settings_editor.is_open());
        assert!(
            has_text(&shapes, "Blank uses the factory chord"),
            "shortcut command did not display the bindings page"
        );
        assert!(
            window
                .all_panes_mut()
                .all(|pane| pane.keystrokes.is_empty())
        );
    }

    #[test]
    fn chrome_text_keeps_its_size_when_terminal_text_changes() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0));
        let mut sizes = Vec::new();
        for terminal_size in [10.0, 24.0] {
            window.font_size = terminal_size;
            let shapes = frame(&mut window, &ctx, area, vec![]);
            let actual: Vec<_> = ["Terminal 1", "Console 1"]
                .iter()
                .map(|label| {
                    shapes
                        .iter()
                        .find_map(|s| match &s.shape {
                            egui::Shape::Text(t) if t.galley.text() == *label => {
                                Some(t.galley.job.sections[0].format.font_id.size)
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("{label} was not painted"))
                })
                .collect();
            sizes.push(actual);
        }
        assert_eq!(
            sizes[0], sizes[1],
            "terminal font settings resized the interface labels"
        );
    }

    #[test]
    fn configured_colors_reach_the_painted_titlebar_and_console_label_live() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        for (config, background, foreground) in [
            // The title bar is the theme's chrome role: one step darker than a dark background,
            // and 6 % toward the text on a light one.
            (
                "background=#112233\nforeground=#efcba9",
                egui::Color32::from_rgb(0x0d, 0x1b, 0x28),
                egui::Color32::from_rgb(0xef, 0xcb, 0xa9),
            ),
            (
                "theme=gruvbox-dark\nbackground=#e2d4c6\nforeground=#241c14",
                egui::Color32::from_rgb(0xd7, 0xc9, 0xbb),
                egui::Color32::from_rgb(0x24, 0x1c, 0x14),
            ),
        ] {
            let (settings, warnings) = settings::interpret(config).unwrap();
            assert!(warnings.kept().is_empty());
            settings.apply_to(&mut window);
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(
                shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                if r.rect.top()==area.top() && r.rect.height()==40.0 && r.fill==background)),
                "titlebar ignored the configured surface"
            );
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t)
                if t.galley.text()=="Console 1" && t.galley.job.sections.iter().all(|section|section.format.color==foreground))),
                "console label ignored the configured foreground");
        }
    }

    fn open_updates_menu(
        window: &mut Window,
        ctx: &egui::Context,
        area: egui::Rect,
    ) -> Vec<egui::epaint::ClippedShape> {
        let at = if chrome::strip::is_compact(area.width()) {
            egui::pos2(area.right() - 20.0, area.top() + 20.0)
        } else {
            slots(area).menu.center()
        };
        frame(window, ctx, area, vec![]);
        frame(window, ctx, area, click_at(at));
        frame(window, ctx, area, vec![])
    }

    #[test]
    fn update_notes_are_reachable_in_wide_and_compact_windows_without_network() {
        for width in [1400.0, 240.0] {
            let ctx = egui::Context::default();
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.start(&ctx, false);
            window.headers = window.headers.integrated(true);
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 600.0));
            let shapes = open_updates_menu(&mut window, &ctx, area);
            for label in ["Check for updates", "What's new"] {
                assert!(has_text(&shapes, label));
                assert!(
                    area.contains(text_center(&shapes, label)),
                    "menu item is clipped"
                );
            }
            let at = text_center(&shapes, "What's new");
            frame(&mut window, &ctx, area, click_at(at));
            frame(&mut window, &ctx, area, vec![]);
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(has_text(&shapes, "Installed: Ivy"));
            assert!(has_text(&shapes, "New in this alpha"));
            assert!(!has_text(&shapes, "Checking for updates"));
        }
    }

    #[test]
    fn update_notes_close_and_pointer_release_do_not_type_or_resize_consoles() {
        let ctx = egui::Context::default();
        let received = term_core::received::Received::new("updates-input");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        window.chrome.start(&ctx, false);
        window.headers = window.headers.integrated(true);
        received.wait_until_ready(Duration::from_secs(5));
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
        let menu = open_updates_menu(&mut window, &ctx, area);
        let grids = window
            .active()
            .panes()
            .iter()
            .map(|p| p.measured)
            .collect::<Vec<_>>();
        let at = text_center(&menu, "What's new");
        let mut events = click_at(at);
        events.push(egui::Event::Text("OPENING".into()));
        frame(&mut window, &ctx, area, events);
        frame(&mut window, &ctx, area, vec![]);
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Paste("INSIDE".into())],
        );
        let held_at = egui::pos2(10.0, 200.0);
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(held_at),
                egui::Event::PointerButton {
                    pos: held_at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Text("CLOSING".into()),
            ],
        );
        frame(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerButton {
                    pos: held_at,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Paste("RELEASE".into()),
            ],
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
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("AFTER".into())],
        );
        assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"AFTER");
    }

    #[test]
    fn short_window_menu_scroll_keeps_existing_commands_reachable_without_console_input() {
        for width in [1400.0, 240.0] {
            let ctx = egui::Context::default();
            let received = term_core::received::Received::new("updates-menu-scroll");
            let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
            window.chrome.start(&ctx, false);
            window.headers = window.headers.integrated(true);
            received.wait_until_ready(Duration::from_secs(5));
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 250.0));
            open_updates_menu(&mut window, &ctx, area);
            let point = egui::pos2(width - 100.0, 250.0);
            frame(
                &mut window,
                &ctx,
                area,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: egui::vec2(0.0, -2000.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::Text("SCROLL".into()),
                ],
            );
            let shapes = frame(&mut window, &ctx, area, vec![]);
            let at = text_center(&shapes, "Connect agents");
            assert!(area.contains(at));
            frame(&mut window, &ctx, area, click_at(at));
            frame(&mut window, &ctx, area, vec![]);
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(has_text(&shapes, "Set up Ivy once per AI client."));
            frame(
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
            frame(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Text("AFTER".into())],
            );
            assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"AFTER");
        }
    }
    fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn separate_click(window: &mut Window, ctx: &egui::Context, area: egui::Rect, at: egui::Pos2) {
        // A tab's cross exists once the pointer has been over its card for a pass.
        frame(window, ctx, area, vec![egui::Event::PointerMoved(at)]);
        frame(window, ctx, area, vec![pointer_button(at, true)]);
        frame(window, ctx, area, vec![pointer_button(at, false)]);
    }

    #[test]
    fn board_allows_tab_controls_and_preserves_the_result_after_escape() {
        for board_open in [false, true] {
            for target in ["body", "close", "add"] {
                let ctx = egui::Context::default();
                let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
                window.chrome.enable();
                window.headers = window.headers.integrated(true);
                let area = area_of(1400.0);
                frame(&mut window, &ctx, area, vec![]);
                if board_open {
                    crate::board::toggle(&mut window, &ctx);
                }
                frame(&mut window, &ctx, area, vec![]);
                frame(&mut window, &ctx, area, vec![]);
                let selected = window.board.selected();
                let at = tab_target(&window, &ctx, area, 1, target);
                separate_click(&mut window, &ctx, area, at);
                assert_eq!(
                    window.board.is_open(),
                    board_open,
                    "{target} dismissed Board"
                );
                match target {
                    "body" => {
                        assert_eq!(window.active, 1);
                        assert_eq!(window.tabs.len(), 2);
                    }
                    "close" => {
                        assert_eq!(window.tabs.len(), 1);
                        assert_eq!(window.tabs[0].name, 0);
                    }
                    _ => {
                        assert_eq!(window.tabs.len(), 3);
                        assert_eq!(window.active, 2);
                    }
                }
                assert_eq!(
                    window.board.selected(),
                    selected,
                    "tab gesture selected a card"
                );
                assert!(
                    window
                        .all_panes_mut()
                        .all(|pane| pane.keystrokes.is_empty())
                );
                let focus = window.active().focus;
                if board_open {
                    let metrics = crate::painter::CellMetrics::measure(
                        &ctx,
                        egui::FontId::monospace(window.font_size),
                    );
                    let dock = crate::tab_bar::geometry(&window, area, &metrics)
                        .dock
                        .expect("Board is visible beside the consoles");
                    // Escape dismisses the dock only after it receives keyboard focus.
                    separate_click(
                        &mut window,
                        &ctx,
                        area,
                        dock.right_top() + egui::vec2(-8.0, 8.0),
                    );
                }
                frame(&mut window, &ctx, area, vec![menu_key(egui::Key::Escape)]);
                assert!(!window.board.is_open());
                assert_eq!(window.active().focus, focus, "Board close undid tab action");
            }
        }
    }

    fn menu_key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn board_menu_preferences_and_shortcuts_receive_separate_clicks() {
        for board_open in [false, true] {
            for label in ["Preferences", "Keyboard shortcuts"] {
                let ctx = egui::Context::default();
                chrome::prepare(&ctx);
                let mut window = window_with("sleep 30", "sleep 30");
                window.chrome.enable();
                window.headers = window.headers.integrated(true);
                window.settings_editor =
                    settings::editor::Editor::resolved(settings::Entry::Normal, None);
                let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0));
                frame(&mut window, &ctx, area, vec![]);
                if board_open {
                    crate::board::toggle(&mut window, &ctx);
                }
                frame(&mut window, &ctx, area, vec![]);
                let focus_before_menu_action = window.active().focus;
                let shapes = open_updates_menu(&mut window, &ctx, area);
                let at = text_center(&shapes, label);
                separate_click(&mut window, &ctx, area, at);
                assert!(
                    window.settings_editor.is_open(),
                    "Board={board_open}: {label} did not open"
                );
                assert!(!window.board.is_open());
                frame(&mut window, &ctx, area, vec![]);
                let shapes = frame(&mut window, &ctx, area, vec![]);
                if label == "Keyboard shortcuts" {
                    assert!(has_text(&shapes, "Blank uses the factory chord"));
                }
                assert!(
                    window
                        .all_panes_mut()
                        .all(|pane| pane.keystrokes.is_empty())
                );
                assert_eq!(
                    window.active().focus,
                    focus_before_menu_action,
                    "{label} changed the active application focus"
                );
                frame(&mut window, &ctx, area, vec![menu_key(egui::Key::Escape)]);
                assert!(!window.settings_editor.is_open(), "{label} did not close");
                assert_eq!(
                    window.active().focus,
                    focus_before_menu_action,
                    "closing {label} did not restore the active application focus"
                );
            }
        }
    }
    #[test]
    fn board_menu_retained_actions_work_at_both_widths() {
        for board_open in [false, true] {
            for width in [1400.0, 240.0] {
                for (label, proof) in [
                    ("Check for updates", "Installed: Ivy"),
                    ("What's new", "New in this alpha"),
                    ("Connect agents", "Set up Ivy once per AI client."),
                    ("Search console", ""),
                ] {
                    let ctx = egui::Context::default();
                    chrome::prepare(&ctx);
                    let mut window = window_with("sleep 30", "sleep 30");
                    window.chrome.start(&ctx, false);
                    window.headers = window.headers.integrated(true);
                    let area =
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 900.0));
                    frame(&mut window, &ctx, area, vec![]);
                    if board_open {
                        crate::board::toggle(&mut window, &ctx);
                    }
                    frame(&mut window, &ctx, area, vec![]);
                    let shapes = open_updates_menu(&mut window, &ctx, area);
                    let menu_clip = shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(t) if t.galley.text() == "Preferences" => {
                                Some(shape.clip_rect)
                            }
                            _ => None,
                        })
                        .unwrap();
                    let in_menu = |label: &str| {
                        shapes.iter().any(|shape| shape.clip_rect == menu_clip && matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text() == label))
                    };
                    for removed in [
                        "New tab",
                        "Split right",
                        "Split down",
                        "Toggle information panel",
                        "Markdown viewer",
                        "Git diff viewer",
                    ] {
                        assert!(!in_menu(removed), "menu retained {removed}");
                    }
                    // A compact bar has no room for the tools' buttons, so its menu carries them.
                    for tool in ["Files", "Markdown", "Board"] {
                        assert_eq!(in_menu(tool), width < 500.0, "{tool} at {width}");
                    }
                    let at = text_center(&shapes, label);
                    separate_click(&mut window, &ctx, area, at);
                    assert!(!window.board.is_open(), "{label} did not dismiss Board");
                    frame(&mut window, &ctx, area, vec![]);
                    let shapes = frame(&mut window, &ctx, area, vec![]);
                    if label == "Search console" {
                        assert!(window.active().panes()[0].search.is_open());
                    } else {
                        assert!(
                            has_text(&shapes, proof),
                            "Board={board_open}, width={width}: {label} did not open"
                        );
                    }
                    assert!(
                        window
                            .all_panes_mut()
                            .all(|pane| pane.keystrokes.is_empty())
                    );
                }
            }
        }
    }

    #[test]
    fn board_menu_owns_keyboard_dismissal_and_cross_surface_releases() {
        for board_open in [false, true] {
            let ctx = egui::Context::default();
            chrome::prepare(&ctx);
            let received = term_core::received::Received::new("board-menu-ownership");
            let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
            received.wait_until_ready(Duration::from_secs(5));
            window.chrome.start(&ctx, false);
            window.headers = window.headers.integrated(true);
            window.settings_editor =
                settings::editor::Editor::resolved(settings::Entry::Normal, None);
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
            frame(&mut window, &ctx, area, vec![]);
            if board_open {
                crate::board::toggle(&mut window, &ctx);
            }
            frame(&mut window, &ctx, area, vec![]);
            let selected = window.board.selected();
            let shapes = open_updates_menu(&mut window, &ctx, area);
            let row = text_center(&shapes, "Preferences");
            // A press on the menu followed by release over the canvas cancels, never activates a card.
            frame(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::PointerMoved(row), pointer_button(row, true)],
            );
            let card = egui::pos2(100.0, 250.0);
            frame(
                &mut window,
                &ctx,
                area,
                vec![
                    egui::Event::PointerMoved(card),
                    pointer_button(card, false),
                    egui::Event::Text("LEAK".into()),
                ],
            );
            assert_eq!(window.board.is_open(), board_open);
            assert_eq!(window.board.selected(), selected);
            assert!(!window.settings_editor.is_open());
            frame(&mut window, &ctx, area, vec![menu_key(egui::Key::Escape)]);
            assert_eq!(
                window.board.is_open(),
                board_open,
                "Escape closed Board behind menu"
            );
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(!has_text(&shapes, "Preferences"));
            open_updates_menu(&mut window, &ctx, area);
            // Outside press dismisses; its release over a card is still owned by chrome.
            frame(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::PointerMoved(card), pointer_button(card, true)],
            );
            frame(
                &mut window,
                &ctx,
                area,
                vec![
                    pointer_button(card, false),
                    egui::Event::Text("LEAK".into()),
                ],
            );
            assert_eq!(window.board.is_open(), board_open);
            assert_eq!(window.board.selected(), selected);
            open_updates_menu(&mut window, &ctx, area);
            for _ in 0..3 {
                frame(
                    &mut window,
                    &ctx,
                    area,
                    vec![menu_key(egui::Key::ArrowDown)],
                );
            }
            frame(&mut window, &ctx, area, vec![menu_key(egui::Key::Enter)]);
            assert!(
                window.settings_editor.is_open(),
                "keyboard did not activate Preferences"
            );
            assert!(!window.board.is_open());
            assert!(
                window
                    .all_panes_mut()
                    .all(|pane| pane.keystrokes.is_empty())
            );
            // Verify the real PTY after releasing both overlays: only this sentinel may arrive.
            frame(&mut window, &ctx, area, vec![]);
            frame(&mut window, &ctx, area, vec![menu_key(egui::Key::Escape)]);
            frame(&mut window, &ctx, area, vec![]);
            frame(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::Text("AFTER".into())],
            );
            assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"AFTER");
        }
    }
    #[test]
    fn compact_menu_window_controls_emit_commands_with_board_open_or_closed() {
        for board_open in [false, true] {
            for (label, command) in [
                ("Minimize window", egui::ViewportCommand::Minimized(true)),
                ("Maximize window", egui::ViewportCommand::Maximized(true)),
                ("Close window", egui::ViewportCommand::Close),
            ] {
                let ctx = egui::Context::default();
                let mut window = window_with("sleep 30", "sleep 30");
                window.chrome.enable();
                window.headers = window.headers.integrated(true);
                let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(240.0, 600.0));
                frame(&mut window, &ctx, area, vec![]);
                if board_open {
                    crate::board::toggle(&mut window, &ctx);
                }
                frame(&mut window, &ctx, area, vec![]);
                let shapes = open_updates_menu(&mut window, &ctx, area);
                let at = text_center(&shapes, label);
                frame(
                    &mut window,
                    &ctx,
                    area,
                    vec![egui::Event::PointerMoved(at), pointer_button(at, true)],
                );
                let mut native_frame = eframe::Frame::_new_kittest();
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(area),
                        events: vec![pointer_button(at, false)],
                        ..Default::default()
                    },
                    |ui| eframe::App::ui(&mut window, ui, &mut native_frame),
                );
                assert!(
                    output.viewport_output[&egui::ViewportId::ROOT]
                        .commands
                        .contains(&command),
                    "{label} did not dispatch with Board={board_open}"
                );
                output.drop_without_applying_deltas();
                assert_eq!(window.board.is_open(), board_open);
                assert!(
                    window
                        .all_panes_mut()
                        .all(|pane| pane.keystrokes.is_empty())
                );
            }
        }
    }
    #[test]
    fn compact_menu_selection_is_cleared_when_window_controls_disappear() {
        for board_open in [false, true] {
            let ctx = egui::Context::default();
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let compact = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(240.0, 600.0));
            let wide = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 600.0));
            frame(&mut window, &ctx, compact, vec![]);
            if board_open {
                crate::board::toggle(&mut window, &ctx);
            }
            frame(&mut window, &ctx, compact, vec![]);
            open_updates_menu(&mut window, &ctx, compact);
            // Up selects Close window, which is unavailable after widening.
            frame(
                &mut window,
                &ctx,
                compact,
                vec![menu_key(egui::Key::ArrowUp)],
            );
            frame(&mut window, &ctx, wide, vec![menu_key(egui::Key::Enter)]);
            assert_eq!(window.board.is_open(), board_open);
            let shapes = frame(&mut window, &ctx, wide, vec![]);
            assert!(
                has_text(&shapes, "Preferences"),
                "invalid selection dispatched an action"
            );
        }
    }
    #[test]
    fn application_menu_does_not_intercept_escape_from_a_pane_dialog() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        let tab = window.active().name;
        let pane = window.active().focus;
        window.pane_view.begin_rename(tab, pane, "Console".into());
        frame(&mut window, &ctx, area, vec![]);
        open_updates_menu(&mut window, &ctx, area);
        frame(&mut window, &ctx, area, vec![menu_key(egui::Key::Escape)]);
        assert!(
            !window.pane_view.is_modal(),
            "hidden chrome menu stole dialog dismissal"
        );
    }

    /// The title bar's band of tab cards, and one of its cards.
    fn title_card(
        window: &Window,
        ctx: &egui::Context,
        area: egui::Rect,
        which: usize,
    ) -> egui::Rect {
        let m = CellMetrics::measure(ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let band = tab_bar::geometry(window, area, &m)
            .cards
            .expect("the title bar holds the tabs");
        tab_bar::title_card(band, which, window.tabs.len())
    }

    /// The colours of the tab crosses painted inside `close`.
    fn cross_strokes(
        shapes: &[egui::epaint::ClippedShape],
        close: egui::Rect,
    ) -> Vec<egui::Color32> {
        shapes
            .iter()
            .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
            .filter(|(glyph, _, square)| {
                glyph == egui_phosphor::regular::X && close.contains(square.center())
            })
            .map(|(_, color, _)| color)
            .collect()
    }

    /// An inactive tab's cross shows only under the pointer: none while idle, muted text over the
    /// card, full text over the cross itself, and muted again the next time the card is reached
    /// after leaving from the cross. Ivy Night's tx3 and tx, written out.
    #[test]
    fn an_inactive_tabs_cross_shows_under_the_pointer_and_brightens_over_itself() {
        let tx3 = egui::Color32::from_rgb(0x8e, 0x93, 0x90);
        let tx = egui::Color32::from_rgb(0xe4, 0xe8, 0xe2);
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(800.0);
        let mut time = 0.0;
        let mut pass = |window: &mut Window, events: Vec<egui::Event>| {
            time += 0.05;
            ui_pass(
                window,
                &ctx,
                area,
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
            )
        };
        pass(&mut window, vec![]);
        let card = title_card(&window, &ctx, area, 1);
        let close = egui::Rect::from_min_max(
            egui::pos2(card.right() - 26.0, card.top()),
            card.right_bottom(),
        );
        let idle = pass(&mut window, vec![]);
        assert!(
            cross_strokes(&idle, close).is_empty(),
            "an idle inactive tab shows its cross"
        );
        let body = tab_target(&window, &ctx, area, 1, "body");
        let settle = |pass: &mut dyn FnMut(
            &mut Window,
            Vec<egui::Event>,
        ) -> Vec<egui::epaint::ClippedShape>,
                      window: &mut Window,
                      at: egui::Pos2| {
            pass(window, vec![egui::Event::PointerMoved(at)]);
            for _ in 0..4 {
                pass(window, vec![]);
            }
            pass(window, vec![])
        };
        let over_card = settle(&mut pass, &mut window, body);
        let strokes = cross_strokes(&over_card, close);
        assert!(
            !strokes.is_empty() && strokes.iter().all(|c| *c == tx3),
            "{strokes:?}"
        );
        let over_cross = settle(&mut pass, &mut window, close.center());
        let strokes = cross_strokes(&over_cross, close);
        assert!(
            !strokes.is_empty() && strokes.iter().all(|c| *c == tx),
            "{strokes:?}"
        );
        let away = settle(&mut pass, &mut window, egui::pos2(50.0, 200.0));
        assert!(cross_strokes(&away, close).is_empty());
        let back = pass(&mut window, vec![egui::Event::PointerMoved(body)]);
        let strokes = cross_strokes(&back, close);
        assert!(
            !strokes.is_empty() && strokes.iter().all(|c| *c == tx3),
            "the cross came back still hovered: {strokes:?}"
        );
    }

    /// The right edge of an inactive tab nobody pointed at is its body: a press there selects the
    /// tab and leaves its consoles running.
    #[test]
    fn a_press_on_an_idle_inactive_tabs_right_edge_selects_it_instead_of_closing_it() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(800.0);
        frame(&mut window, &ctx, area, vec![]);
        frame(&mut window, &ctx, area, vec![]);
        let at = tab_target(&window, &ctx, area, 1, "close");
        frame(&mut window, &ctx, area, click_at(at));
        assert_eq!(window.tabs.len(), 2, "an unseen cross closed the tab");
        assert_eq!(window.active, 1);
    }

    /// The add button stays after the last card, so the second click of a double click lands on
    /// the tab the first one created: one new tab, and no rename of it.
    #[test]
    fn a_double_click_on_the_add_button_opens_a_tab_without_renaming_it() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let at = tab_target(&window, &ctx, area, 0, "add");
        let pass = |window: &mut Window, time: f64, events: Vec<egui::Event>| {
            ui_pass(
                window,
                &ctx,
                area,
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
            );
        };
        pass(&mut window, 0.0, vec![egui::Event::PointerMoved(at)]);
        pass(&mut window, 0.02, click_at(at));
        assert_eq!(window.tabs.len(), 3);
        pass(&mut window, 0.05, vec![]);
        let card = title_card(&window, &ctx, area, 2);
        assert!(
            card.contains(at),
            "the case is a second click on the new card: {card:?}"
        );
        pass(&mut window, 0.1, click_at(at));
        pass(&mut window, 0.15, vec![]);
        assert_eq!(window.tabs.len(), 3);
        assert!(
            !window.pane_view.is_renaming(),
            "the second half of a double click on the add button renamed the new tab"
        );
        // A real double click on that tab still renames it.
        pass(&mut window, 1.0, click_at(at));
        pass(&mut window, 1.1, click_at(at));
        assert!(window.pane_view.is_renaming());
    }

    /// Between 100 and 150 points a card cannot hold «Terminal N» or a long name: the unnamed tab
    /// shows its number and the named one its name cut with «…», both inside the label's room.
    #[test]
    fn tab_labels_fit_their_room_whole_cut_or_as_a_number() {
        for width in [730.0, 764.0, 804.0, 830.0, 1400.0] {
            let ctx = egui::Context::default();
            let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            window.tabs[1].label = Some("Release notes for the week".into());
            let area = area_of(width);
            frame(&mut window, &ctx, area, vec![]);
            let shapes = frame(&mut window, &ctx, area, vec![]);
            for which in 0..2 {
                let card = title_card(&window, &ctx, area, which);
                if width < 1400.0 {
                    assert!((100.0..=150.0).contains(&card.width()), "{width}: {card:?}");
                }
                let (label, _, _) = tab_bar::title_label_dots_summary(card, 2, true);
                let texts: Vec<_> = shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) if card.contains(t.pos) => Some(t),
                        _ => None,
                    })
                    .collect();
                assert_eq!(texts.len(), 1, "{width}: tab {which}");
                let text = texts[0];
                assert!(
                    text.pos.x + text.galley.size().x <= label.right() + 0.5,
                    "{width}: tab {which} «{}» overflows its room {label:?}",
                    text.galley.text()
                );
                let ellipsis = text
                    .galley
                    .rows
                    .iter()
                    .any(|row| row.glyphs.iter().any(|g| g.chr == '…'));
                match which {
                    0 if width == 616.0 => assert_eq!(text.galley.text(), "1", "{width}"),
                    0 if width == 1400.0 => assert_eq!(text.galley.text(), "Terminal 1"),
                    0 => assert!(["1", "Terminal 1"].contains(&text.galley.text()), "{width}"),
                    _ => assert!(ellipsis, "{width}: the name is not cut"),
                }
            }
        }
    }

    /// The tab strip of the measurement batches keeps D13's look: the bar on the console
    /// background, the active card filled with the focus colour and its number in the background
    /// colour over it. Ivy Night's literals, written out.
    #[test]
    fn the_historical_tab_strip_keeps_its_painting() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        let area = area_of(800.0);
        frame(&mut window, &ctx, area, vec![]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        let m = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let bar = tab_bar::geometry(&window, area, &m)
            .cards
            .expect("two tabs have a bar");
        assert_eq!(Some(bar), tab_bar::set_the_tab_bar_aside(area, 2, &m).0);
        let filled = |rect: egui::Rect, color: egui::Color32| {
            shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == rect && r.fill == color),
            )
        };
        assert!(filled(bar, egui::Color32::from_rgb(0x13, 0x18, 0x1b)));
        assert!(filled(
            tab_bar::tab_rect(bar, 0, 2),
            egui::Color32::from_rgb(0x3c, 0xc3, 0x84)
        ));
        assert_eq!(
            text_color_of(&shapes, "1"),
            Some(egui::Color32::from_rgb(0x13, 0x18, 0x1b))
        );
        assert_eq!(
            text_color_of(&shapes, "2"),
            Some(egui::Color32::from_rgb(0xe4, 0xe8, 0xe2))
        );
    }

    fn text_color_of(shapes: &[egui::epaint::ClippedShape], text: &str) -> Option<egui::Color32> {
        shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                Some(t.galley.job.sections[0].format.color)
            }
            _ => None,
        })
    }

    /// **The window hands its integrated chrome to the panel** (D97): a console whose producer
    /// declares `wait` paints, through the real application pass, a band in the wait tint across
    /// its panel's whole width with the claim's words on it — not the historical row of text.
    #[test]
    fn an_integrated_window_paints_the_panels_claim_as_a_strip() {
        let ctx = egui::Context::default();
        chrome::prepare(&ctx);
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        with_the_panel_in_place(
            &mut window,
            0,
            r#"{"attention":"wait","lines":["Waiting"]}"#,
        );
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let shapes = frame(&mut window, &ctx, area, vec![]);
        let tint = egui::Color32::from_rgba_unmultiplied(0xf2, 0xb8, 0x4b, 38);
        let band = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.fill == tint && r.rect.height() == 32.0 => Some(r.rect),
                _ => None,
            })
            .expect("the panel's claim strip is painted");
        let words = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "it is waiting for you" => Some(t.pos),
                _ => None,
            })
            .expect("the claim's words are painted");
        assert!(band.contains(words), "{words:?} is not on {band:?}");
        assert!(
            !has_text(&shapes, "? it is waiting for you"),
            "the historical claim row is painted too"
        );
        let m = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        assert!(
            band.width() >= 20.0 * m.width(),
            "the band spans the panel, not a pill: {band:?}"
        );
    }

    /// The application menu floats on the theme's `raised` role with a control border, and the
    /// console's search row is laid on the same surface (D98).
    #[test]
    fn the_application_menu_and_the_search_row_float_on_raised() {
        fn rects(shape: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
            match shape {
                egui::Shape::Rect(r) => out.push(r.clone()),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| rects(s, out)),
                _ => {}
            }
        }
        let (raised, line2) = (
            egui::Color32::from_rgb(0x20, 0x24, 0x27),
            egui::Color32::from_rgb(0x37, 0x3b, 0x3d),
        );
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        let search_row = |shapes: &[egui::epaint::ClippedShape]| {
            let mut found = Vec::new();
            for clipped in shapes {
                rects(&clipped.shape, &mut found);
            }
            found
                .iter()
                .any(|r| r.fill == raised && r.rect.height() < 40.0 && r.rect.width() > 200.0)
        };
        // The control: no such row before the search opens.
        assert!(!search_row(&frame(&mut window, &ctx, area, vec![])));
        let search = control(&window, &ctx, area, 0, 112.5);
        frame(&mut window, &ctx, area, click_at(search));
        // Read before the menu opens: an open menu dims the consoles beneath it.
        assert!(
            search_row(&frame(&mut window, &ctx, area, vec![])),
            "the search row is not on raised"
        );
        let menu = slots(area).menu.center();
        frame(&mut window, &ctx, area, click_at(menu));
        let shapes = frame(&mut window, &ctx, area, vec![]);
        let mut all = Vec::new();
        for clipped in &shapes {
            rects(&clipped.shape, &mut all);
        }
        let item = text_center(&shapes, "Preferences");
        let body = all
            .iter()
            .filter(|r| r.rect.contains(item) && r.fill == raised)
            .min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
            .expect("the menu is filled with raised");
        assert_eq!(body.corner_radius, egui::CornerRadius::same(10));
        assert!(
            all.iter()
                .any(|r| r.rect == body.rect && r.stroke == egui::Stroke::new(1.0, line2)),
            "the menu has no control border"
        );
    }

    #[test]
    fn colliding_menu_hover_paints_for_pointer_and_keyboard() {
        for source in [
            "theme=ivy-night\npanel=#262b2d\n",
            "background=#000000\nforeground=#757575\n",
        ] {
            let (settings, _) = settings::interpret(source).unwrap();
            let theme = settings
                .theme
                .map(theme::NamedTheme::theme)
                .unwrap_or_default()
                .with_colors(&settings.colors);
            let roles = theme::Roles::of(&theme);
            let ctx = egui::Context::default();
            let mut window = window_with("sleep 30", "sleep 30");
            window.theme = theme;
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = area_of(1400.0);
            frame(&mut window, &ctx, area, vec![]);
            let menu = egui::pos2(area.right() - 125.0, area.top() + 24.0);
            frame(&mut window, &ctx, area, click_at(menu));
            let shapes = frame(&mut window, &ctx, area, vec![]);
            let at = text_center(&shapes, "Preferences");
            let keyboard_target = text_center(&shapes, "Keyboard shortcuts");
            for (events, target) in [
                (vec![egui::Event::PointerMoved(at)], at),
                (
                    vec![
                        egui::Event::PointerMoved(egui::pos2(
                            area.left() + 2.0,
                            area.bottom() - 2.0,
                        )),
                        egui::Event::Key {
                            key: egui::Key::ArrowDown,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    keyboard_target,
                ),
            ] {
                let shapes = frame(&mut window, &ctx, area, events);
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == roles.hover && r.rect.contains(target) && r.rect.width() > 100.0 && r.rect.height() <= 30.0)), "{source}");
                assert!(theme::project_ink_contrast(roles.hover, roles.raised) >= 1.03);
            }
        }
    }

    fn pane_of(window: &Window, name: usize) -> &Pane {
        window
            .active()
            .panes()
            .into_iter()
            .find(|pane| pane.name == name)
            .expect("a console of the active tab")
    }

    /// Where a part of console `which`'s search box was painted in the last pass.
    fn search_target(
        window: &Window,
        which: usize,
        target: search::floating::Target,
    ) -> egui::Pos2 {
        let painted = window.active().panes()[which]
            .search
            .open
            .as_ref()
            .and_then(|open| open.painted.as_ref())
            .expect("the box was painted");
        painted
            .layout
            .targets
            .iter()
            .find(|(kind, _)| *kind == target)
            .map(|(_, rect)| rect.center())
            .expect("the box lays that part out")
    }

    /// AC2, AC3: a console's own header button searches that console — not the focused one — and
    /// hands it the keyboard; pressed again it closes the search.
    #[test]
    fn a_header_search_button_searches_its_own_console_and_gives_it_the_keys() {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = integrated_pair();
        let area = tall_area();
        frame(&mut window, &ctx, area, vec![]);
        let names = names_in_order(&window);
        assert_eq!(
            window.active().focus,
            names[0],
            "the premise: the left one has the keys"
        );
        let button = control(&window, &ctx, area, 1, 112.5);
        // The unfocused header shows its controls under the pointer, and a press counts only once
        // they were painted (D99).
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(button)],
        );
        frame(&mut window, &ctx, area, click_at(button));
        let open = |window: &Window, name| pane_of(window, name).search.is_open();
        assert!(
            open(&window, names[1]),
            "the pressed console's search did not open"
        );
        assert!(
            !open(&window, names[0]),
            "the focused console was searched instead"
        );
        assert_eq!(
            window.active().focus,
            names[1],
            "the keyboard stayed elsewhere"
        );
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("needle".into())],
        );
        assert_eq!(
            pane_of(&window, names[1])
                .search
                .open
                .as_ref()
                .unwrap()
                .query(),
            "needle"
        );
        frame(&mut window, &ctx, area, click_at(button));
        assert!(
            !open(&window, names[1]),
            "pressed again, the search stayed open"
        );
    }

    /// AC5–AC9: the box sits inside its console without resizing it; its buttons do what their keys
    /// do; its options list toggles; and a press on it reaches nothing underneath.
    #[test]
    fn the_search_box_buttons_act_and_nothing_under_the_box_hears_them() {
        use search::floating::Target;
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = integrated_pair();
        let area = tall_area();
        frame(&mut window, &ctx, area, vec![]);
        let names = names_in_order(&window);
        let grid = pane_of(&window, names[0]).screen.size();
        window
            .pane_called(names[0])
            .unwrap()
            .screen
            .advance(b"one needle\r\ntwo needle\r\n");
        let at = control(&window, &ctx, area, 0, 112.5);
        frame(&mut window, &ctx, area, click_at(at));
        frame(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::Text("needle".into())],
        );
        let shapes = frame(&mut window, &ctx, area, vec![]);
        assert!(!shapes.is_empty());
        let pane = pane_of(&window, names[0]);
        let console = pane.painted_console.as_ref().unwrap().area;
        let box_ = pane
            .search
            .open
            .as_ref()
            .unwrap()
            .painted
            .as_ref()
            .unwrap()
            .layout
            .frame;
        assert!(
            console.contains_rect(box_),
            "{box_:?} is not inside {console:?}"
        );
        assert_eq!(
            pane.screen.size(),
            grid,
            "opening the search resized the console"
        );

        // Focus elsewhere, then the field: the console takes the keys back, nothing is selected.
        window.active_mut().focus = names[1];
        frame(&mut window, &ctx, area, vec![]);
        let at = search_target(&window, 0, Target::Field);
        frame(&mut window, &ctx, area, click_at(at));
        assert_eq!(
            window.active().focus,
            names[0],
            "a press on the field left the keys elsewhere"
        );
        let pane = pane_of(&window, names[0]);
        assert!(pane.search.is_open());
        assert_eq!(
            pane.screen.selected_text(),
            None,
            "the press on the box selected underneath"
        );

        let row_of_selection = |window: &Window| {
            let view = pane_of(window, names[0]).screen.view();
            let cols = usize::from(view.size().cols);
            view.cells()
                .chunks_exact(cols)
                .position(|row| row.iter().any(|cell| cell.selected))
        };
        frame(&mut window, &ctx, area, vec![]);
        let at = search_target(&window, 0, Target::Next);
        frame(&mut window, &ctx, area, click_at(at));
        assert_eq!(
            pane_of(&window, names[0]).screen.selected_text().as_deref(),
            Some("needle"),
            "next did not step to a match"
        );
        let newest = row_of_selection(&window);
        frame(&mut window, &ctx, area, vec![]);
        let at = search_target(&window, 0, Target::Next);
        frame(&mut window, &ctx, area, click_at(at));
        let older = row_of_selection(&window);
        assert!(
            older < newest,
            "next did not walk older: {older:?} then {newest:?}"
        );
        frame(&mut window, &ctx, area, vec![]);
        let at = search_target(&window, 0, Target::Previous);
        frame(&mut window, &ctx, area, click_at(at));
        assert_eq!(
            row_of_selection(&window),
            newest,
            "previous did not walk newer"
        );

        frame(&mut window, &ctx, area, vec![]);
        let at = search_target(&window, 0, Target::Options);
        frame(&mut window, &ctx, area, click_at(at));
        assert!(
            pane_of(&window, names[0])
                .search
                .open
                .as_ref()
                .unwrap()
                .list()
        );
        frame(&mut window, &ctx, area, vec![]);
        let whole_word = search_target(&window, 0, Target::Choice(search::Choice::WholeWord));
        frame(&mut window, &ctx, area, click_at(whole_word));
        assert!(pane_of(&window, names[0]).search.options().whole_word);
        assert_eq!(
            pane_of(&window, names[0]).screen.selected_text().as_deref(),
            Some("needle"),
            "a press on the list reached the console underneath"
        );
        frame(&mut window, &ctx, area, vec![]);
        let elsewhere = pane_rect(&window, &ctx, area, names[1]).center();
        frame(&mut window, &ctx, area, click_at(elsewhere));
        let pane = pane_of(&window, names[0]);
        assert!(
            !pane.search.open.as_ref().unwrap().list(),
            "a press elsewhere left the list open"
        );

        frame(&mut window, &ctx, area, vec![]);
        window.active_mut().focus = names[0];
        frame(&mut window, &ctx, area, vec![]);
        let at = search_target(&window, 0, Target::Close);
        frame(&mut window, &ctx, area, click_at(at));
        assert!(
            !pane_of(&window, names[0]).search.is_open(),
            "close did not close"
        );
    }

    /// AC8: what lies under the box is out of sight, so a secondary press on the box opens no
    /// context menu over the console beneath. The control: the same press on the console does.
    #[test]
    fn a_secondary_press_on_the_search_box_opens_no_menu_underneath() {
        let ctx = egui::Context::default();
        crate::fonts::install_icons_only(&ctx);
        let mut window = integrated_pair();
        let area = tall_area();
        frame(&mut window, &ctx, area, vec![]);
        let names = names_in_order(&window);
        let at = control(&window, &ctx, area, 0, 112.5);
        frame(&mut window, &ctx, area, click_at(at));
        frame(&mut window, &ctx, area, vec![]);
        let secondary = |at: egui::Pos2| {
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Secondary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ]
        };
        let field = search_target(&window, 0, search::floating::Target::Field);
        frame(&mut window, &ctx, area, secondary(field));
        assert!(
            !window.pane_view.is_modal(),
            "a press on the box opened the menu of the console under it"
        );
        frame(&mut window, &ctx, area, vec![]);
        let console = pane_of(&window, names[0])
            .painted_console
            .as_ref()
            .unwrap()
            .area;
        frame(&mut window, &ctx, area, secondary(console.center()));
        assert!(
            window.pane_view.is_modal(),
            "the premise: the same press on the console opens its menu"
        );
    }
}
