use super::*;
use crate::tests::{one_frame_in, window_of_tabs};

#[test]
fn board_fills_content_at_every_width_without_resizing_consoles() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    for width in [500.0, 1600.0] {
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 700.0));
        close(&mut window, &ctx);
        one_frame_in(&mut window, &ctx, area, vec![]);
        let closed: Vec<_> = window.all_panes().map(|p| p.measured).collect();
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
        let baseline = crate::tab_bar::geometry(&window, area, &metrics);
        select(&mut window, Tool::Board);
        one_frame_in(&mut window, &ctx, area, vec![]);
        let opened = crate::tab_bar::geometry(&window, area, &metrics);
        assert_eq!(opened.dock, Some(baseline.consoles));
        assert!(
            opened.divider.is_none(),
            "Board must not have a resize handle"
        );
        assert_eq!(opened.consoles, baseline.consoles);
        assert_eq!(window.sidebar.last_dock, opened.dock);
        assert_eq!(
            window.all_panes().map(|p| p.measured).collect::<Vec<_>>(),
            closed
        );
    }
}

#[test]
fn narrow_windows_suppress_only_the_body_and_restore_preferred_width() {
    for height in [0.0, 20.0, 600.0] {
        for width in [0.0, 12.0, 36.0, 500.0, 681.0, 1000.0, 1400.0] {
            let area = egui::Rect::from_min_size(egui::pos2(7.0, 11.0), egui::vec2(width, height));
            let result = layout(area, true, 480.0, 320.0);
            assert!(area.contains_rect(result.consoles));
            assert!(area.contains_rect(result.rail));
            assert!(result.consoles.width() >= 0.0);
            assert!(result.consoles.right() <= result.rail.left());
            if let Some(dock) = result.dock {
                assert!(result.consoles.width() >= 320.0);
                assert!(dock.width() >= 320.0);
                assert_eq!(dock.right(), result.rail.left());
                assert!(result.consoles.right() < dock.left());
            } else {
                assert!(width < 682.0);
            }
        }
    }
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 700.0));
    assert_eq!(
        layout(area, true, 480.0, 320.0).dock.unwrap().width(),
        480.0
    );
}

#[cfg(unix)]
#[test]
fn board_blocks_all_mouse_buttons_and_motion_to_covered_programs() {
    let received = term_core::received::Received::new("board-covered-mouse");
    let script = received.script("stty raw -echo");
    let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
    received.wait_until_ready(Duration::from_secs(5));
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 700.0));
    let focus = window.active().focus;
    window
        .pane_called(focus)
        .unwrap()
        .screen
        .advance(b"\x1b[?1003h\x1b[?1006h");
    select(&mut window, Tool::Board);
    one_frame_in(&mut window, &ctx, area, vec![]);
    one_frame_in(&mut window, &ctx, area, vec![]);
    let pos = window
        .pane_called(focus)
        .unwrap()
        .painted_console
        .as_ref()
        .unwrap()
        .area
        .center();
    one_frame_in(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::PointerMoved(pos)],
    );
    for button in [egui::PointerButton::Secondary, egui::PointerButton::Middle] {
        for pressed in [true, false] {
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }
    close(&mut window, &ctx);
    one_frame_in(&mut window, &ctx, area, vec![]);
    one_frame_in(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::Text("after".into())],
    );
    assert_eq!(received.wait_for(5, Duration::from_secs(5)), b"after");
}

#[test]
fn opening_resizing_switching_and_closing_preserve_consoles_and_update_their_grids() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 700.0));
    one_frame_in(&mut window, &ctx, area, vec![]);
    let identities: Vec<_> = window.all_panes().map(|p| p.name).collect();
    let closed: Vec<_> = window.all_panes().map(|p| p.measured).collect();
    select(&mut window, Tool::Files);
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert!(window.sidebar.last_dock.is_some());
    assert_ne!(
        window.all_panes().map(|p| p.measured).collect::<Vec<_>>(),
        closed
    );
    window.sidebar.width = 360.0;
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(window.sidebar.last_dock.unwrap().width(), 360.0);
    select(&mut window, Tool::Board);
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(active(&window), Some(Tool::Board));
    close(&mut window, &ctx);
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(
        window.all_panes().map(|p| p.measured).collect::<Vec<_>>(),
        closed
    );
    select(&mut window, Tool::Files);
    let narrow = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 700.0));
    one_frame_in(&mut window, &ctx, narrow, vec![]);
    assert!(window.sidebar.last_dock.is_none());
    assert_eq!(active(&window), Some(Tool::Files));
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(window.sidebar.last_dock.unwrap().width(), 360.0);
    assert_eq!(
        window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
        identities
    );
}

#[test]
fn toolbar_click_opens_and_closes_files_without_forwarding_the_gesture() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 700.0));
    one_frame_in(&mut window, &ctx, area, vec![]);
    // The first Area pass settles its geometry; clicks are tested against the painted pass.
    one_frame_in(&mut window, &ctx, area, vec![]);
    let pos = window
        .sidebar
        .buttons
        .iter()
        .find(|(tool, _)| *tool == Tool::Files)
        .unwrap()
        .1
        .center();
    let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let geometry = crate::tab_bar::geometry(&window, area, &metrics);
    assert!(
        geometry.rail.contains(pos),
        "button={pos:?}, expected rail={:?}, rendered consoles={:?}",
        geometry.rail,
        window.sidebar.last_consoles,
    );
    for expected in [Some(Tool::Files), None] {
        for pressed in [true, false] {
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(
            active(&window),
            expected,
            "pos={pos:?}, top layer={:?}, rail layer={:?}",
            ctx.layer_id_at(pos),
            egui::LayerId::new(egui::Order::Foreground, egui::Id::new("sidebar-rail"))
        );
        assert!(window.presses_for_the_program.is_empty());
    }
}

#[cfg(unix)]
#[test]
fn divider_drag_retains_consoles_and_cannot_reach_their_mouse_protocol() {
    use std::time::Duration;
    let received = term_core::received::Received::new("sidebar-divider");
    let script = received.script("stty raw -echo && printf '\\033[?1000h\\033[?1006h'");
    let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
    received.wait_until_ready(Duration::from_secs(5));
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 700.0));
    select(&mut window, Tool::Files);
    for _ in 0..2 {
        one_frame_in(&mut window, &ctx, area, vec![]);
    }
    let names: Vec<_> = window.all_panes().map(|p| p.name).collect();
    let weights = window.active().divider_fractions();
    let dock = window.sidebar.last_dock.unwrap();
    let from = egui::pos2(dock.left() - 3.0, dock.center().y);
    let to = from - egui::vec2(90.0, 0.0);
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    one_frame_in(
        &mut window,
        &ctx,
        area,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    one_frame_in(&mut window, &ctx, area, vec![egui::Event::PointerMoved(to)]);
    one_frame_in(&mut window, &ctx, area, vec![button(to, false)]);
    assert!(window.sidebar.last_dock.unwrap().width() > dock.width() + 80.0);
    assert_eq!(
        window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
        names
    );
    assert_eq!(window.active().divider_fractions(), weights);
    assert!(
        received.so_far().is_empty(),
        "dock divider input reached the program"
    );
}

#[test]
fn host_accounting_counts_a_touched_frame_once_even_below_clock_resolution() {
    let mut sidebar = Sidebar::default();
    sidebar.record(Duration::ZERO);
    sidebar.close_frame();
    sidebar.close_frame();
    let mut report = Vec::new();
    sidebar.publish(&mut report).unwrap();
    assert_eq!(
        String::from_utf8(report).unwrap(),
        "sidebar application: frames=1, total=0ns\n"
    );
}

#[test]
fn sidebar_state_is_transient_and_each_new_window_starts_closed_at_default_width() {
    let mut previous = window_of_tabs([["sleep 30", "sleep 30"]]);
    select(&mut previous, Tool::Git);
    previous.sidebar.width = 735.0;

    let fresh = window_of_tabs([["sleep 30", "sleep 30"]]);
    assert_eq!(active(&fresh), None);
    assert_eq!(fresh.sidebar.width, 480.0);

    let saved = crate::session::serialize(&crate::session::StoredDocument {
        v: crate::session::VERSION,
        tabs: vec![crate::session::StoredTab {
            root: crate::session::StoredNode::Leaf(crate::session::StoredLeaf {
                panel_width: 80,
                folded: false,
                label: None,
                cwd: None,
                resume: None,
            }),
            label: None,
        }],
    })
    .expect("the ordinary session layout serializes");
    for transient in [
        "sidebar",
        "visibility",
        "preferred_width",
        "sidebar_width",
        "735",
        "Git",
    ] {
        assert!(!saved.contains(transient), "session serialized {transient}");
    }
}

#[cfg(unix)]
#[test]
fn console_mouse_release_crossing_into_each_tool_or_rail_reaches_its_program() {
    for tool in [Tool::Files, Tool::Markdown, Tool::Git] {
        for rail in [false, true] {
            let received = term_core::received::Received::new("sidebar-console-release");
            let script = received.script("stty raw -echo");
            let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
            received.wait_until_ready(Duration::from_secs(5));
            let ctx = egui::Context::default();
            let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 700.0));
            match tool {
                Tool::Markdown => crate::viewer::activate(&mut window, true),
                Tool::Git => crate::viewer::activate(&mut window, false),
                _ => select(&mut window, tool),
            }
            let name = window.active().panes()[0].name;
            window
                .pane_called(name)
                .unwrap()
                .screen
                .advance(b"\x1b[?1000h\x1b[?1006h");
            for _ in 0..2 {
                one_frame_in(&mut window, &ctx, area, vec![]);
            }
            let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
            let geometry = crate::tab_bar::geometry(&window, area, &metrics);
            let from = window
                .pane_called(name)
                .unwrap()
                .painted_console
                .as_ref()
                .unwrap()
                .area
                .center();
            let to = if rail {
                geometry.rail.center()
            } else {
                geometry.dock.unwrap().center()
            };
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::PointerMoved(from), button(from, true)],
            );
            assert!(
                !window.presses_for_the_program.is_empty(),
                "{tool:?}: program never received press"
            );
            let press = received.wait_for(1, Duration::from_secs(5));
            assert!(press.ends_with(b"M"), "missing SGR press: {press:?}");
            one_frame_in(
                &mut window,
                &ctx,
                area,
                vec![egui::Event::PointerMoved(to), button(to, false)],
            );
            assert!(
                window.presses_for_the_program.is_empty(),
                "{tool:?}: release captured by dock (rail={rail})"
            );
            let complete = received.wait_for(press.len() + 1, Duration::from_secs(5));
            assert!(
                complete.ends_with(b"m"),
                "{tool:?}: missing SGR release: {complete:?}"
            );
        }
    }
}

#[test]
fn tab_click_repartitions_the_dock_for_the_new_tree_in_the_same_frame() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
    if let Node::Branch { children, .. } = &mut window.tabs[1].root {
        children[0].weight = crate::tiling::Weight::new(0.02).unwrap();
        children[1].weight = crate::tiling::Weight::new(0.98).unwrap();
    } else {
        panic!("two-pane fixture needs a split");
    }
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 700.0));
    select(&mut window, Tool::Files);
    for _ in 0..2 {
        one_frame_in(&mut window, &ctx, area, vec![]);
    }
    assert!(window.sidebar.last_dock.is_some());
    let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let strip = crate::tab_bar::geometry(&window, area, &metrics)
        .cards
        .unwrap();
    let pos = crate::tab_bar::tab_rect(strip, 1, 2).center();
    for pressed in [true, false] {
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(window.active, 1);
    assert!(
        window.sidebar.last_dock.is_none(),
        "new tree needs the full console width immediately"
    );
    let grids: Vec<_> = window.all_panes().map(|pane| pane.measured).collect();
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(
        window
            .all_panes()
            .map(|pane| pane.measured)
            .collect::<Vec<_>>(),
        grids,
        "tab switch published a transient grid"
    );
}
