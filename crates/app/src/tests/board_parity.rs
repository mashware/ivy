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
        pane.signals.sync_native(&mut pane.screen, Instant::now());
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
}
