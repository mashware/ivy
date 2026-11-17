use super::*;
use crate::tests::{one_frame_in, window_of_tabs};
use crate::tree::Node;

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
fn files_and_the_drawer_keep_their_minimums_or_step_aside_for_the_consoles() {
    use layout::{DRAWER_MARGIN, DRAWER_MINIMUM, FILES_MINIMUM, Floors, layout};
    let floors = Floors {
        width: 320.0,
        height: 120.0,
    };
    for height in [0.0, 20.0, 291.0, 292.0, 600.0, 900.0] {
        for width in [0.0, 12.0, 300.0, 525.0, 526.0, 1000.0, 1400.0] {
            let area = egui::Rect::from_min_size(egui::pos2(7.0, 11.0), egui::vec2(width, height));
            let result = layout(area, Some(264.0), Some(370.0), false, floors);
            assert!(area.contains_rect(result.consoles), "{width}x{height}");
            match result.files {
                Some(files) => {
                    assert!(files.width() >= FILES_MINIMUM);
                    assert_eq!(files.left(), area.left());
                    assert!(result.consoles.width() >= floors.width);
                    assert!(files.right() < result.consoles.left());
                }
                None => assert!(width < FILES_MINIMUM + DIVIDER + floors.width, "{width}"),
            }
            match result.dock {
                Some(drawer) => {
                    assert!(drawer.height() + DRAWER_MARGIN >= DRAWER_MINIMUM);
                    assert!(result.consoles.height() >= floors.height);
                    assert!(result.consoles.bottom() < drawer.top());
                    assert!(area.contains_rect(drawer));
                    assert_eq!(drawer.left(), result.consoles.left() + DRAWER_MARGIN);
                }
                None => assert!(
                    height < DRAWER_MINIMUM + DIVIDER + floors.height
                        || result.consoles.width() <= 2.0 * DRAWER_MARGIN,
                    "{width}x{height}"
                ),
            }
        }
    }
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
    let open = layout(area, Some(264.0), Some(370.0), false, floors);
    assert_eq!(open.files.unwrap().width(), 264.0);
    assert_eq!(
        open.dock.unwrap().height() + DRAWER_MARGIN,
        370.0,
        "the drawer keeps its preferred height"
    );
    let maximized = layout(area, Some(264.0), Some(370.0), true, floors);
    let closed = layout(area, Some(264.0), None, false, floors);
    assert_eq!(
        maximized.consoles, open.consoles,
        "maximizing the drawer resizes no console"
    );
    assert_eq!(maximized.dock, Some(closed.consoles.shrink(DRAWER_MARGIN)));
    assert!(maximized.divider.is_none());
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
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 800.0));
    one_frame_in(&mut window, &ctx, area, vec![]);
    let identities: Vec<_> = window.all_panes().map(|p| p.name).collect();
    let measured = |window: &Window| window.all_panes().map(|p| p.measured).collect::<Vec<_>>();
    let closed = measured(&window);
    select(&mut window, Tool::Files);
    one_frame_in(&mut window, &ctx, area, vec![]);
    let files = window.sidebar.last_files.expect("Files opens at the left");
    assert_eq!(files.left(), 0.0);
    assert_eq!(files.width(), 264.0);
    assert!(window.sidebar.last_dock.is_none());
    assert_ne!(measured(&window), closed);
    window.sidebar.set_files_width(300.0);
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(window.sidebar.last_files.unwrap().width(), 300.0);
    let beside = measured(&window);
    crate::viewer::activate(&mut window, false);
    one_frame_in(&mut window, &ctx, area, vec![]);
    let drawer = window.sidebar.last_dock.expect("the drawer opens below");
    assert!(files_visible(&window), "the drawer does not close Files");
    assert_eq!(window.sidebar.last_files.unwrap().width(), 300.0);
    assert!(
        drawer.left() > 300.0,
        "the drawer sits beside Files, below the consoles"
    );
    assert!((drawer.bottom() - (area.bottom() - layout::DRAWER_MARGIN)).abs() < 0.01);
    assert_ne!(
        measured(&window),
        beside,
        "the consoles give up their bottom rows"
    );
    select(&mut window, Tool::Board);
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(active(&window), Some(Tool::Board));
    assert!(
        window.sidebar.last_files.is_none(),
        "the Board covers Files"
    );
    close(&mut window, &ctx);
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert!(files_visible(&window), "closing the Board gives Files back");
    assert_eq!(measured(&window), beside);
    let narrow = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 800.0));
    one_frame_in(&mut window, &ctx, narrow, vec![]);
    assert!(window.sidebar.last_files.is_none());
    assert!(
        files_visible(&window),
        "a narrow window suppresses Files without closing it"
    );
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(window.sidebar.last_files.unwrap().width(), 300.0);
    close_files(&mut window);
    one_frame_in(&mut window, &ctx, area, vec![]);
    assert_eq!(measured(&window), closed);
    assert_eq!(
        window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
        identities
    );
}

/// Click `pos` with the primary button over two frames.
fn click(window: &mut Window, ctx: &egui::Context, area: egui::Rect, pos: egui::Pos2) {
    for pressed in [true, false] {
        one_frame_in(
            window,
            ctx,
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
}

fn button_of(window: &Window, tool: Tool) -> egui::Pos2 {
    window
        .sidebar
        .buttons
        .iter()
        .find(|(which, _)| *which == tool)
        .unwrap_or_else(|| panic!("the title bar paints no {tool:?} button"))
        .1
        .center()
}

#[test]
fn title_bar_buttons_open_and_close_each_tool_without_forwarding_the_gesture() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    window.chrome.enable();
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 800.0));
    for _ in 0..2 {
        one_frame_in(&mut window, &ctx, area, vec![]);
    }
    let files = button_of(&window, Tool::Files);
    assert!(files.y < 40.0, "the Files button sits in the title bar");
    assert!(files.x < 60.0, "the Files button opens the bar on the left");
    for expected in [true, false] {
        click(&mut window, &ctx, area, files);
        assert_eq!(files_visible(&window), expected);
        assert!(window.presses_for_the_program.is_empty());
    }
    for tool in [Tool::Markdown, Tool::Board] {
        let pos = button_of(&window, tool);
        assert!(pos.x > 1000.0, "{tool:?} sits in the right-hand group");
        for expected in [Some(tool), None] {
            let at = button_of(&window, tool);
            click(&mut window, &ctx, area, at);
            assert_eq!(active(&window), expected, "{tool:?}");
            assert!(window.presses_for_the_program.is_empty());
        }
    }
}

#[cfg(unix)]
#[test]
fn divider_drags_retain_consoles_and_cannot_reach_their_mouse_protocol() {
    let received = term_core::received::Received::new("sidebar-divider");
    let script = received.script("stty raw -echo && printf '\\033[?1000h\\033[?1006h'");
    let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
    received.wait_until_ready(Duration::from_secs(5));
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 800.0));
    select(&mut window, Tool::Files);
    crate::viewer::activate(&mut window, false);
    for _ in 0..2 {
        one_frame_in(&mut window, &ctx, area, vec![]);
    }
    let names: Vec<_> = window.all_panes().map(|p| p.name).collect();
    let weights = window.active().divider_fractions();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let drag = |window: &mut Window, from: egui::Pos2, to: egui::Pos2| {
        one_frame_in(
            window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        );
        one_frame_in(window, &ctx, area, vec![egui::Event::PointerMoved(to)]);
        one_frame_in(window, &ctx, area, vec![button(to, false)]);
    };
    let files = window.sidebar.last_files.unwrap();
    let from = egui::pos2(files.right() + DIVIDER / 2.0, files.top() + 100.0);
    drag(&mut window, from, from + egui::vec2(90.0, 0.0));
    assert!(window.sidebar.last_files.unwrap().width() > files.width() + 80.0);
    let drawer = window.sidebar.last_dock.unwrap();
    let from = egui::pos2(drawer.center().x, drawer.top() - DIVIDER / 2.0);
    drag(&mut window, from, from - egui::vec2(0.0, 90.0));
    assert!(window.sidebar.last_dock.unwrap().height() > drawer.height() + 80.0);
    assert_eq!(
        window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
        names
    );
    assert_eq!(window.active().divider_fractions(), weights);
    assert!(
        received.so_far().is_empty(),
        "a divider's input reached the program"
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
fn sidebar_state_is_transient_and_each_new_window_starts_closed_at_default_sizes() {
    let mut previous = window_of_tabs([["sleep 30", "sleep 30"]]);
    select(&mut previous, Tool::Git);
    select(&mut previous, Tool::Files);
    previous.sidebar.set_files_width(735.0);
    previous.sidebar.set_height(735.0);

    let fresh = window_of_tabs([["sleep 30", "sleep 30"]]);
    assert_eq!(active(&fresh), None);
    assert!(!files_visible(&fresh));
    assert_eq!(fresh.sidebar.files_width, layout::FILES_WIDTH);
    assert_eq!(fresh.sidebar.height, layout::DRAWER_HEIGHT);

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
        "files_width",
        "drawer",
        "735",
        "Git",
    ] {
        assert!(!saved.contains(transient), "session serialized {transient}");
    }
}

#[cfg(unix)]
#[test]
fn console_mouse_release_crossing_into_files_or_the_drawer_reaches_its_program() {
    for (tool, maximized) in [
        (Tool::Files, false),
        (Tool::Markdown, false),
        (Tool::Git, false),
        (Tool::Markdown, true),
    ] {
        let received = term_core::received::Received::new("sidebar-console-release");
        let script = received.script("stty raw -echo");
        let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
        received.wait_until_ready(Duration::from_secs(5));
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 800.0));
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
        let from = window
            .pane_called(name)
            .unwrap()
            .painted_console
            .as_ref()
            .unwrap()
            .area
            .center();
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
        if maximized {
            toggle_maximize(&mut window);
        }
        assert!(press.ends_with(b"M"), "missing SGR press: {press:?}");
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
        let geometry = crate::tab_bar::geometry(&window, area, &metrics);
        let to = match tool {
            Tool::Files => geometry.files.unwrap().center(),
            _ => geometry.dock.unwrap().center(),
        };
        one_frame_in(
            &mut window,
            &ctx,
            area,
            vec![egui::Event::PointerMoved(to), button(to, false)],
        );
        assert!(
            window.presses_for_the_program.is_empty(),
            "{tool:?}: release captured (maximized={maximized})"
        );
        let complete = received.wait_for(press.len() + 1, Duration::from_secs(5));
        assert!(
            complete.ends_with(b"m"),
            "{tool:?}: missing SGR release: {complete:?}"
        );
    }
}

#[test]
fn tab_click_repartitions_files_for_the_new_tree_in_the_same_frame() {
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
    assert!(window.sidebar.last_files.is_some());
    let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let strip = crate::tab_bar::geometry(&window, area, &metrics)
        .cards
        .unwrap();
    click(
        &mut window,
        &ctx,
        area,
        crate::tab_bar::tab_rect(strip, 1, 2).center(),
    );
    assert_eq!(window.active, 1);
    assert!(
        window.sidebar.last_files.is_none(),
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

#[test]
fn the_drawer_maximizes_over_the_consoles_restores_its_height_and_keeps_files_beside_it() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1500.0, 800.0));
    window.sidebar.set_height(300.0);
    crate::viewer::activate(&mut window, true);
    for _ in 0..2 {
        one_frame_in(&mut window, &ctx, area, vec![]);
    }
    let before: Vec<_> = window.all_panes().map(|p| (p.name, p.measured)).collect();
    let drawer = window.sidebar.last_dock.unwrap();
    let maximize =
        crate::viewer::strip_place(&window, drawer, crate::viewer::StripAction::Maximize)
            .expect("the strip paints maximize");
    click(&mut window, &ctx, area, maximize);
    assert!(
        window.sidebar.maximized,
        "the painted maximize control did not change the presentation"
    );
    let covering = window.sidebar.last_dock.unwrap();
    assert!(covering.height() > 600.0);
    assert_eq!(
        window
            .all_panes()
            .map(|p| (p.name, p.measured))
            .collect::<Vec<_>>(),
        before,
        "a maximized drawer resizes no console"
    );
    let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let cards = crate::tab_bar::geometry(&window, area, &metrics)
        .cards
        .unwrap();
    click(
        &mut window,
        &ctx,
        area,
        crate::tab_bar::tab_rect(cards, 1, 2).center(),
    );
    assert_eq!(
        window.active, 1,
        "the maximized drawer captured a tab click"
    );
    select(&mut window, Tool::Files);
    one_frame_in(&mut window, &ctx, area, vec![]);
    let files = window
        .sidebar
        .last_files
        .expect("Files opens beside the drawer");
    assert!(files.right() < window.sidebar.last_dock.unwrap().left());
    assert!(window.sidebar.maximized);
    let restore = crate::viewer::strip_place(
        &window,
        window.sidebar.last_dock.unwrap(),
        crate::viewer::StripAction::Maximize,
    )
    .unwrap();
    click(&mut window, &ctx, area, restore);
    assert!(!window.sidebar.maximized);
    assert!(files_visible(&window), "restoring keeps Files");
    assert_eq!(
        window.sidebar.last_dock.unwrap().height() + layout::DRAWER_MARGIN,
        300.0
    );
    let close_at = crate::viewer::strip_place(
        &window,
        window.sidebar.last_dock.unwrap(),
        crate::viewer::StripAction::Close,
    )
    .unwrap();
    click(&mut window, &ctx, area, close_at);
    assert_eq!(active(&window), None, "close shuts the drawer");
    assert!(files_visible(&window), "closing the drawer keeps Files");
}
