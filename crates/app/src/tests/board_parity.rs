//! Board parity exercised through the same UI passes and pointer input as the window.
#[cfg(test)]
mod tests {
    use super::super::*;
    use std::time::Instant;

    fn area() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1800.0, 1600.0))
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

    fn prepare(window: &mut Window) {
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
    }

    fn text_position(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<egui::Pos2> {
        shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.text().contains(needle) && shape.clip_rect.contains(text.pos) =>
            {
                Some(text.pos)
            }
            _ => None,
        })
    }

    fn native(pane: &mut Pane, state: &str) {
        native_at(pane, state, Instant::now());
    }

    fn native_at(pane: &mut Pane, state: &str, now: Instant) {
        pane.signals.configure(
            crate::agent_signals::Enabled {
                claude: true,
                codex: true,
                opencode: true,
            },
            &mut pane.screen,
        );
        pane.screen
            .advance(format!("\x1b]7501;state={state}:app=claude-code\x1b\\").as_bytes());
        pane.signals.sync_native(&mut pane.screen, now);
    }

    #[test]
    fn an_expired_native_wait_is_visibly_unconfirmed_on_its_card() {
        let mut window = window_with("sleep 30", "sleep 30");
        prepare(&mut window);
        let ctx = egui::Context::default();
        let target = window.active().panes()[0].name;
        native_at(
            window.active_mut().panes_mut()[0],
            "blocked:kind=question",
            Instant::now() - crate::agent_signals::STALE_AFTER - Duration::from_secs(1),
        );
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        let card = window.board.card_rect(target).unwrap();
        assert!(shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(t) if t.galley.text().contains("Waiting state is unconfirmed") && card.contains(t.pos) && shape.clip_rect.contains(t.pos))),"expired native wait looked confirmed");
    }

    #[test]
    fn waits_are_observed_before_board_opens_and_survive_republishing() {
        let mut window = window_with("sleep 30", "sleep 30");
        prepare(&mut window);
        let ctx = egui::Context::default();
        let pane = &mut *window.active_mut().panes_mut()[0];
        let target = pane.name;
        pane.panel = Some(PanelDir::create().unwrap());
        publish_at(pane, r#"{"attention":"wait","lines":["first"]}"#);
        pane.to_reread.raise();
        frame(&mut window, &ctx, vec![]);
        let before = window
            .board
            .observed_since(target)
            .expect("closed Board did not observe wait");
        let pane = &mut *window.active_mut().panes_mut()[0];
        publish_at(pane, r#"{"attention":"wait","lines":["updated"]}"#);
        pane.to_reread.raise();
        frame(&mut window, &ctx, vec![]);
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        assert_eq!(window.board.observed_since(target), Some(before));
    }

    #[test]
    fn card_link_opens_disclosed_url_without_opening_console() {
        let mut window = window_with("sleep 30", "sleep 30");
        prepare(&mut window);
        let ctx = egui::Context::default();
        window.desktop = crate::links::Desktop::Recorded(Vec::new());
        let pane = &mut *window.active_mut().panes_mut()[0];
        let target = pane.name;
        pane.panel = Some(PanelDir::create().unwrap());
        publish_at(
            pane,
            r#"{"attention":"wait","lines":[{"text":"Review","link":"https://example.test/pull/42"}]}"#,
        );
        pane.to_reread.raise();
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        let card = window.board.card_rect(target).unwrap();
        let at = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.text().contains("#42") && card.contains(t.pos) => {
                    Some(t.pos + egui::vec2(3.0, 3.0))
                }
                _ => None,
            })
            .expect("disclosed card link");
        frame(&mut window, &ctx, click_at(at));
        assert!(window.board.is_open());
        match &window.desktop {
            crate::links::Desktop::Recorded(opened) => {
                assert_eq!(opened, &["https://example.test/pull/42"])
            }
            _ => panic!("test desktop changed"),
        };
    }

    #[test]
    fn columns_are_painted_in_order_and_hold_the_explicit_states() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 3]);
        prepare(&mut window);
        let ctx = egui::Context::default();
        let names: Vec<_> = window.all_panes().map(|pane| pane.name).collect();
        for (pane, attention) in
            window
                .all_panes_mut()
                .zip([Some("wait"), Some("block"), None, None, None])
        {
            if let Some(attention) = attention {
                pane.panel = Some(PanelDir::create().unwrap());
                publish_at(
                    pane,
                    &format!(r#"{{"attention":"{attention}","lines":["fixture"]}}"#),
                );
                pane.to_reread.raise();
            }
        }
        native(window.tabs[1].panes_mut()[0], "working");
        native(window.tabs[1].panes_mut()[1], "done");
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        let labels = ["Waiting for you", "Stuck", "Working", "Finished", "Idle"];
        let xs: Vec<_> = labels
            .iter()
            .map(|label| {
                text_position(&shapes, label)
                    .unwrap_or_else(|| panic!("missing Board column {label}"))
                    .x
            })
            .collect();
        assert!(
            xs.windows(2).all(|pair| pair[0] < pair[1]),
            "columns are not in the requested order: {xs:?}"
        );
        let cards: Vec<_> = names[..5]
            .iter()
            .map(|name| window.board.card_rect(*name).expect("card painted"))
            .collect();
        assert!(
            cards
                .windows(2)
                .all(|pair| pair[0].center().x < pair[1].center().x),
            "explicit states did not reach their respective columns: {cards:?}"
        );
        assert!(
            window.tabs[1].panes()[1].panel_state.document().is_err(),
            "native Done fixture unexpectedly has a panel"
        );
    }

    #[test]
    fn expanding_a_long_panel_reveals_the_last_line_without_opening_its_console() {
        let mut window = window_with("sleep 30", "sleep 30");
        prepare(&mut window);
        let ctx = egui::Context::default();
        let target = window.active().panes()[1].name;
        let pane = &mut *window.active_mut().panes_mut()[1];
        pane.panel = Some(PanelDir::create().unwrap());
        let mut lines: Vec<String> = (0..21)
            .map(|n| format!("parity body line {n:02}"))
            .collect();
        lines.push("parity final sentinel".into());
        publish_at(
            pane,
            &serde_json::json!({"attention":"wait", "lines":lines}).to_string(),
        );
        pane.to_reread.raise();
        frame(&mut window, &ctx, vec![]);
        let focus = window.active().focus;
        let layout = window.active().root.describe();
        let grids: Vec<_> = window.active().panes().iter().map(|p| p.measured).collect();
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        let card = window.board.card_rect(target).expect("long card painted");
        let on_card = |shapes: &[egui::epaint::ClippedShape], needle: &str| {
            shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.text().contains(needle)
                        && card.contains(text.pos)
                        && shape.clip_rect.contains(text.pos) =>
                {
                    Some(text.pos)
                }
                _ => None,
            })
        };
        assert!(
            on_card(&shapes, "parity final sentinel").is_none(),
            "sentinel should start behind Show all"
        );
        let show = on_card(&shapes, "Show all").expect("long panel has no Show all control");
        frame(&mut window, &ctx, click_at(show + egui::vec2(3.0, 3.0)));
        assert!(window.board.is_open(), "expansion activated the console");
        assert_eq!(window.active().focus, focus);
        let shapes = frame(&mut window, &ctx, vec![]);
        let expanded = window
            .board
            .card_rect(target)
            .expect("expanded card painted");
        assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("parity final sentinel") && expanded.contains(text.pos) && shape.clip_rect.contains(text.pos))), "Show all did not reveal the last panel line on its card");
        assert_eq!(window.active().root.describe(), layout);
        assert_eq!(
            window
                .active()
                .panes()
                .iter()
                .map(|p| p.measured)
                .collect::<Vec<_>>(),
            grids
        );
    }

    #[test]
    fn opening_a_section_does_not_open_its_console() {
        let mut window = window_with("sleep 30", "sleep 30");
        prepare(&mut window);
        let ctx = egui::Context::default();
        let target = window.active().panes()[1].name;
        let pane = &mut *window.active_mut().panes_mut()[1];
        pane.panel = Some(PanelDir::create().unwrap());
        publish_at(
            pane,
            r#"{"attention":"wait","lines":["Section fixture",{"text":"Parity section","style":"section","collapsed":true,"key":"parity"},"Section hidden witness"]}"#,
        );
        pane.to_reread.raise();
        frame(&mut window, &ctx, vec![]);
        let focus = window.active().focus;
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        let card = window
            .board
            .card_rect(target)
            .expect("section card painted");
        let section = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.text().contains("Parity section")
                        && card.contains(text.pos)
                        && shape.clip_rect.contains(text.pos) =>
                {
                    Some(text.pos)
                }
                _ => None,
            })
            .expect("section heading is absent from its card");
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("Section hidden witness") && card.contains(text.pos))));
        frame(&mut window, &ctx, click_at(section + egui::vec2(3.0, 3.0)));
        assert!(
            window.board.is_open(),
            "section click activated the console"
        );
        assert_eq!(window.active().focus, focus);
        let shapes = frame(&mut window, &ctx, vec![]);
        let card = window
            .board
            .card_rect(target)
            .expect("expanded section card painted");
        assert!(shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("Section hidden witness") && card.contains(text.pos) && shape.clip_rect.contains(text.pos))), "section click did not reveal its body");
    }

    fn card_text(
        window: &Window,
        shapes: &[egui::epaint::ClippedShape],
        pane: usize,
        needle: &str,
    ) -> Option<egui::Pos2> {
        let card = window.board.card_rect(pane)?;
        shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(t)
                if t.galley.text().contains(needle)
                    && card.contains(t.pos)
                    && shape.clip_rect.contains(t.pos) =>
            {
                let start = t.galley.text().find(needle).unwrap();
                let cursor = t.galley.text()[..start].chars().count() + needle.chars().count() / 2;
                Some(
                    t.galley
                        .pos_from_cursor(egui::text::CCursor::new(cursor))
                        .center()
                        + t.pos.to_vec2(),
                )
            }
            _ => None,
        })
    }

    #[test]
    fn compact_cards_hide_show_and_hide_their_body_without_selecting_a_console() {
        for attention in ["done", "idle"] {
            let mut window = window_with("sleep 30", "sleep 30");
            prepare(&mut window);
            let ctx = egui::Context::default();
            let pane = &mut *window.active_mut().panes_mut()[1];
            let target = pane.name;
            pane.panel = Some(PanelDir::create().unwrap());
            let mut doc = serde_json::json!({"lines":[
                {"text":"Producer heading", "style":"title"},
                "ZBODY"
            ]});
            if attention == "done" {
                doc["attention"] = "done".into();
            }
            publish_at(pane, &doc.to_string());
            pane.to_reread.raise();
            let focus = window.active().focus;
            frame(&mut window, &ctx, chord_with(egui::Key::O));
            let shapes = frame(&mut window, &ctx, vec![]);
            assert!(card_text(&window, &shapes, target, "Producer heading").is_some());
            assert!(card_text(&window, &shapes, target, "ZBODY").is_none());
            let show = card_text(&window, &shapes, target, "Show panel").unwrap();
            frame(&mut window, &ctx, click_at(show));
            let shapes = frame(&mut window, &ctx, vec![]);
            assert!(
                card_text(&window, &shapes, target, "ZBODY").is_some(),
                "{attention} body stayed hidden after Show panel"
            );
            let hide = card_text(&window, &shapes, target, "Hide panel").unwrap();
            frame(&mut window, &ctx, click_at(hide));
            let shapes = frame(&mut window, &ctx, vec![]);
            assert!(card_text(&window, &shapes, target, "ZBODY").is_none());
            assert!(card_text(&window, &shapes, target, "Show panel").is_some());
            assert!(window.board.is_open());
            assert_eq!(window.active().focus, focus);
        }
    }

    #[test]
    fn board_section_choice_survives_replacement_without_opening_the_sidebar_section() {
        let mut window = window_with("sleep 30", "sleep 30");
        prepare(&mut window);
        let ctx = egui::Context::default();
        let target = window.active().panes()[1].name;
        let publish = |window: &mut Window, heading: &str, body: &str| {
            let pane = &mut *window.active_mut().panes_mut()[1];
            if pane.panel.is_none() {
                pane.panel = Some(PanelDir::create().unwrap());
            }
            publish_at(
                pane,
                &serde_json::json!({"attention":"wait","lines":[
                    {"text":heading,"style":"section","collapsed":true,"key":"stable-section"},
                    body
                ]})
                .to_string(),
            );
            pane.to_reread.raise();
        };
        publish(&mut window, "Before replacement", "Old hidden body");
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        let section = card_text(&window, &shapes, target, "Before replacement").unwrap();
        frame(&mut window, &ctx, click_at(section));
        publish(&mut window, "After replacement", "New republished body");
        let shapes = frame(&mut window, &ctx, vec![]);
        assert!(card_text(&window, &shapes, target, "New republished body").is_some());
        frame(
            &mut window,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!window.board.is_open());
        let shapes = frame(&mut window, &ctx, vec![]);
        assert!(
            !the_text_queued_up(&shapes, "New republished body"),
            "Board fold choice leaked into the sidebar"
        );
        frame(
            &mut window,
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::O,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        let shapes = frame(&mut window, &ctx, vec![]);
        assert!(
            card_text(&window, &shapes, target, "New republished body").is_some(),
            "closing Board discarded its reader choice; open={}",
            window.board.is_open()
        );
    }

    #[test]
    fn waiting_cards_use_observed_age_then_stable_tree_order() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 2]);
        prepare(&mut window);
        let ctx = egui::Context::default();
        let names: Vec<_> = window.all_panes().map(|p| p.name).collect();
        for group in [1, 0] {
            for pane in window.tabs[group].panes_mut() {
                pane.panel = Some(PanelDir::create().unwrap());
                publish_at(pane, r#"{"attention":"wait","lines":["Waiting witness"]}"#);
                pane.to_reread.raise();
            }
            frame(&mut window, &ctx, vec![]);
        }
        assert_eq!(
            window.board.observed_since(names[2]),
            window.board.observed_since(names[3])
        );
        assert!(window.board.observed_since(names[2]) < window.board.observed_since(names[0]));
        frame(&mut window, &ctx, chord_with(egui::Key::O));
        frame(&mut window, &ctx, vec![]);
        let cards: Vec<_> = [2, 3, 0, 1]
            .map(|i| window.board.card_rect(names[i]).unwrap())
            .into();
        assert!(
            cards.windows(2).all(|r| r[0].bottom() < r[1].top()),
            "wait age or stable ties did not control the painted order: {cards:?}"
        );
    }

    #[test]
    fn native_wheel_reveals_lower_waiting_cards_without_keyboard_navigation() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]; 4]);
        prepare(&mut window);
        let ctx = egui::Context::default();
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2048.0, 1224.0));
        let names: Vec<_> = window.all_panes().map(|p| p.name).collect();
        for pane in window.all_panes_mut() {
            pane.panel = Some(PanelDir::create().unwrap());
            publish_at(pane, &serde_json::json!({"attention":"wait","lines":(0..33).map(|n|format!("Waiting line {n}: {}", "long content ".repeat(6))).collect::<Vec<_>>()} ).to_string());
            pane.to_reread.raise();
        }
        let pass = |window: &mut Window, events| {
            ui_pass(
                window,
                &ctx,
                viewport,
                egui::RawInput {
                    events,
                    ..Default::default()
                },
            )
        };
        pass(&mut window, chord_with(egui::Key::O));
        pass(&mut window, vec![]);
        let first_before = window
            .board
            .card_rect(names[0])
            .expect("first waiting card initially visible");
        assert!(
            window.board.card_rect(names[7]).is_none(),
            "fixture needs lower cards outside the viewport"
        );
        for _ in 0..20 {
            pass(
                &mut window,
                vec![
                    egui::Event::PointerMoved(egui::pos2(200.0, 600.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: egui::vec2(0.0, -600.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            pass(&mut window, vec![]);
            if window.board.card_rect(names[7]).is_some() {
                break;
            }
        }
        assert!(window.board.is_open());
        assert_ne!(
            window.board.card_rect(names[0]),
            Some(first_before),
            "wheel left first card fixed in place"
        );
        assert!(
            window
                .board
                .card_rect(names[7])
                .is_some_and(|r| r.is_positive() && viewport.contains(r.center())),
            "wheel never revealed the last waiting card"
        );
    }

    #[test]
    fn first_open_keeps_column_headers_visible_with_a_tall_selected_wait_card() {
        let mut window = window_with("sleep 30", "sleep 30");
        let removed = window.active().panes()[1].name;
        drop(window.active_mut().root.close(removed).unwrap());
        prepare(&mut window);
        let ctx = egui::Context::default();
        let pane = &mut *window.active_mut().panes_mut()[0];
        pane.panel = Some(PanelDir::create().unwrap());
        publish_at(pane, &serde_json::json!({"attention":"wait","lines":(0..33).map(|n|format!("Waiting line {n}: {}", "long content ".repeat(6))).collect::<Vec<_>>()} ).to_string());
        pane.to_reread.raise();
        let root = std::env::temp_dir().join(format!("ivy-71-first-open-{}", std::process::id()));
        let mut configured = String::new();
        for name in ["demo", "empty"] {
            let directory = root.join(name);
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join("roadmap.md");
            std::fs::write(&path, "# Roadmap\n- [ ] Pending item\n").unwrap();
            configured.push_str(&format!("roadmap.{name}={}\n", path.display()));
        }
        settings::interpret(&configured)
            .unwrap()
            .0
            .apply_to(&mut window);
        let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(2048.0, 1224.0));
        let pass = |window: &mut Window, events| {
            ui_pass(
                window,
                &ctx,
                viewport,
                egui::RawInput {
                    events,
                    ..Default::default()
                },
            )
        };
        pass(&mut window, chord_with(egui::Key::O));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let shapes = pass(&mut window, vec![]);
            if the_text_queued_up(&shapes, "demo") && the_text_queued_up(&shapes, "empty") {
                for label in ["Waiting for you", "Stuck", "Working", "Finished", "Idle"] {
                    let header = text_position(&shapes, label).unwrap_or_else(|| {
                        panic!(
                            "first open hid the {label} column header without any scrolling input"
                        )
                    });
                    let first_project = text_position(&shapes, "demo").unwrap();
                    assert!(
                        header.y < first_project.y,
                        "{label} header ink at {header:?} is underneath the first project row at {first_project:?}"
                    );
                }
                break;
            }
            assert!(
                Instant::now() < deadline,
                "configured project rows did not arrive"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(window);
        std::fs::remove_dir_all(root).unwrap();
    }
}
