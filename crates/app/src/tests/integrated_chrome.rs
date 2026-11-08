//! Effects of the integrated pane view through the real application pass.
#[cfg(test)]
mod tests {
    use super::super::*;

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

    fn connection_button(area: egui::Rect) -> egui::Pos2 {
        let from_right = if area.width() < 300.0 {
            57.0
        } else if cfg!(target_os = "macos") {
            125.0
        } else {
            227.0
        };
        egui::pos2(area.right() - from_right, area.top() + 24.5)
    }

    fn connection_highlight(shapes: &[egui::epaint::ClippedShape], area: egui::Rect) -> bool {
        let dot = connection_button(area) + egui::vec2(8.0, -8.0);
        shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Circle(c) if c.center.distance(dot) < 0.1 && c.radius == 3.0))
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
            chrome::prepare(&ctx);
            let mut window = window_with("sleep 30", "sleep 30");
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            window.viewer.open_markdown();
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 420.0));
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(connection_highlight(&shapes, area));
            let shapes = open_connection_guide(&mut window, &ctx, area);
            assert!(has_text(&shapes, "Connect agents"));
            assert!(
                !has_text(&shapes, "Markdown viewer"),
                "viewer overlaps the modal"
            );
            assert!(has_text(&shapes, "No panels published yet"));
            let heading = text_center(&shapes, "Connect agents");
            let close = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "×" => {
                        Some(t.pos + t.galley.size() / 2.0)
                    }
                    _ => None,
                })
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
                has_text(&shapes, "Markdown viewer"),
                "viewer did not resume after closure"
            );
        }
    }

    #[test]
    fn connection_indicator_sees_a_first_file_publication_in_the_same_pass_even_in_a_background_tab()
     {
        for tab in [0, 1] {
            let ctx = egui::Context::default();
            ctx.global_style_mut(|style| style.animation_time = 0.0);
            let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1800.0));
            assert!(connection_highlight(
                &frame(&mut window, &ctx, area, vec![]),
                area
            ));
            let pane = &mut window.tabs[tab].panes_mut()[0];
            pane.panel = Some(PanelDir::create().unwrap());
            publish_at(pane, r#"{"lines":["A file producer needs no MCP"]}"#);
            pane.to_reread.raise();
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert!(
                !connection_highlight(&shapes, area),
                "first publication left a stale setup highlight"
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
    fn connection_indicator_follows_committed_producer_publication_and_retirement() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        assert!(connection_highlight(
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
        assert!(!connection_highlight(
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
        assert!(connection_highlight(
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
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1800.0));
        let shapes = open_connection_guide(&mut window, &ctx, area);
        assert!(!has_text(&shapes, "1. Locate the MCP bridge"));
        let at = text_center(&shapes, "Copy setup prompt");
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
            && text.contains("Never hard-code")
            && text.contains("PANEL_ID and PANEL_DIR")
            && text.contains("reconnecting or restarting")
            && text.contains("panel_get and panel_set")
            && text.contains("Report success only after the tool calls succeed"))));
        assert!(
            has_text(&output.shapes, "Copied"),
            "copy has no visible confirmation"
        );
        output.drop_without_applying_deltas();
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
    fn ivy_identity_is_painted_in_the_real_title_bar_without_covering_controls() {
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
        for (width, visible) in [(1400.0, true), (480.0, true), (240.0, true), (100.0, false)] {
            let area = area_of(width);
            let shapes = frame(&mut window, &ctx, area, vec![]);
            assert_eq!(has_text(&shapes, "Ivy"), visible);
            assert!(!has_text(&shapes, "agent-terminal"));
            let logos: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == texture => Some(mesh),
                    _ => None,
                })
                .collect();
            assert_eq!(logos.len(), usize::from(visible));
            for mesh in logos {
                let bounds = mesh.calc_bounds();
                assert!(bounds.top() >= area.top());
                assert!(bounds.bottom() <= area.top() + chrome::HEIGHT);
                let left = if cfg!(target_os = "macos") { 78.0 } else { 6.0 };
                let right = if width < 300.0 {
                    40.0
                } else if cfg!(target_os = "macos") {
                    108.0
                } else {
                    210.0
                };
                let left = left + if width < 300.0 { 0.0 } else { 116.0 };
                assert!(bounds.left() >= area.left() + left);
                assert!(bounds.right() <= area.right() - right);
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
        window.pane_view.toggle(tab, name);
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
    fn global_search_opens_the_focused_consoles_search_and_menu_escape_is_owned() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        let area = area_of(1400.0);
        frame(&mut window, &ctx, area, vec![]);
        let search = egui::pos2(area.right() - 159.0, area.top() + 24.0);
        frame(&mut window, &ctx, area, click_at(search));
        assert!(window.active().panes()[0].search.is_open());
        assert!(!window.active().panes()[1].search.is_open());
        let menu = egui::pos2(area.right() - 125.0, area.top() + 24.0);
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
    fn narrow_tab_labels_select_without_invisible_close_targets() {
        for width in [130.0, 95.0] {
            let ctx = egui::Context::default();
            let mut window =
                window_of_tabs(vec![["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
            window.chrome.enable();
            window.headers = window.headers.integrated(true);
            let area = area_of(width);
            frame(&mut window, &ctx, area, vec![]);
            let card_width = (width - 72.0).max(0.0) / 2.0;
            // The right-hand part of a narrow card used to contain a close hitbox without a glyph.
            let at = egui::pos2(10.0 + card_width * 1.5 - 1.5, 65.0);
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
        assert!(has_text(&shapes, "Split down"));
        let split = text_center(&shapes, "Split right");
        frame(&mut window, &ctx, area, click_at(split));
        assert_eq!(
            window.active().panes().len(),
            3,
            "context menu lost its split action"
        );
        assert!(window.all_panes_mut().all(|p| p.keystrokes.is_empty()));
    }

    #[test]
    fn narrow_tab_keeps_attention_separate_from_its_number() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 3]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        *window.active_mut().panes_mut()[0] =
            pane_with_panel(r#"{"attention":"wait","lines":["Waiting"]}"#);
        let shapes = frame(&mut window, &ctx, area_of(190.0), vec![]);
        let texts: Vec<_> = shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.pos.y > 49.0 && t.pos.y < 88.0 => Some(t),
                _ => None,
            })
            .collect();
        let label = texts
            .iter()
            .find(|t| t.galley.text() == "1")
            .expect("narrow tab number painted");
        let claim = texts
            .iter()
            .find(|t| t.galley.text() == "?")
            .expect("attention painted");
        assert!(
            label.pos.x >= claim.pos.x + claim.galley.size().x,
            "attention overlaps the tab number"
        );
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
                window.pane_view.open_menu(tab, pane, true);
                "Console actions"
            };
            for (config, surface, ink) in [
                (
                    "panel=#112233\nforeground=#efcba9",
                    egui::Color32::from_rgb(0x11, 0x22, 0x33),
                    egui::Color32::from_rgb(0xef, 0xcb, 0xa9),
                ),
                (
                    "theme=gruvbox-dark\npanel=#e2d4c6\nforeground=#241c14",
                    egui::Color32::from_rgb(0xe2, 0xd4, 0xc6),
                    egui::Color32::from_rgb(0x24, 0x1c, 0x14),
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
                    "{heading} ignored the configured dialog surface"
                );
                assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t)
                    if t.galley.text() == heading && t.galley.job.sections.iter().all(|section| section.format.color == ink))),
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
    fn global_menu_groups_commands_and_operates_panel_and_shortcut_settings() {
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut window = window_with("sleep 30", "sleep 30");
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        window.settings_editor = settings::editor::Editor::resolved(settings::Entry::Normal, None);
        *window.active_mut().panes_mut()[0] = pane_with_panel(r#"{"lines":["Visible document"]}"#);
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0));
        frame(&mut window, &ctx, area, vec![]);
        let menu = egui::pos2(area.right() - 125.0, area.top() + 24.0);
        frame(&mut window, &ctx, area, click_at(menu));
        let shapes = frame(&mut window, &ctx, area, vec![]);
        for label in [
            "New tab",
            "Split right",
            "Split down",
            "Toggle information panel",
            "Preferences",
            "Keyboard shortcuts",
            "Ctrl Shift T",
            "Ctrl Shift D",
            "Ctrl Shift B",
        ] {
            assert!(has_text(&shapes, label), "menu omitted {label}");
        }
        let new_tab = text_center(&shapes, "New tab").y;
        let right = text_center(&shapes, "Split right").y;
        let down = text_center(&shapes, "Split down").y;
        let panel = text_center(&shapes, "Toggle information panel");
        let preferences = text_center(&shapes, "Preferences").y;
        assert!(
            right - new_tab > down - right,
            "new-tab group has no visible separation"
        );
        assert!(
            preferences - panel.y > down - right,
            "preferences group has no visible separation"
        );
        let shapes = frame(&mut window, &ctx, area, click_at(panel));
        assert!(window.active().panes()[0].folded);
        assert!(
            !has_text(&shapes, "Visible document"),
            "menu toggle left the document visible"
        );
        frame(&mut window, &ctx, area, click_at(menu));
        let shapes = frame(&mut window, &ctx, area, vec![]);
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
            (
                "panel=#112233\nforeground=#efcba9",
                egui::Color32::from_rgb(0x11, 0x22, 0x33),
                egui::Color32::from_rgb(0xef, 0xcb, 0xa9),
            ),
            (
                "theme=gruvbox-dark\npanel=#e2d4c6\nforeground=#241c14",
                egui::Color32::from_rgb(0xe2, 0xd4, 0xc6),
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
                if r.rect.top()==area.top() && r.rect.height()==49.0 && r.fill==background)),
                "titlebar ignored the configured surface"
            );
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t)
                if t.galley.text()=="Console 1" && t.galley.job.sections.iter().all(|section|section.format.color==foreground))),
                "console label ignored the configured foreground");
        }
    }
}
