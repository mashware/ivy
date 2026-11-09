//! Independent title sizes reach real painting, controls and hidden console grids.
use super::*;

fn configure(window: &mut Window, pane: f32, tab: f32) {
    settings::interpret(&format!("pane.title_size={pane}\ntab.label_size={tab}"))
        .unwrap()
        .0
        .apply_to(window);
}

fn label_size(shapes: &[egui::epaint::ClippedShape], label: &str) -> f32 {
    shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.galley.job.sections[0].format.font_id.size)
            }
            _ => None,
        })
        .expect("configured label was painted")
}

#[test]
fn title_typography_sizes_control_paint_and_reserved_space_independently() {
    for (pane_size, tab_size) in [(32.0, 8.0), (8.0, 32.0)] {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        window.chrome.enable();
        window.headers = window.headers.integrated(true);
        window.tabs[0].label = Some("Tab typography sample".into());
        window.active_mut().panes_mut()[0].label = Some("Pane typography sample".into());
        let area = area_of(1400.0);
        ui_pass(&mut window, &ctx, area, Default::default());
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let header_before = header::height(&metrics, window.headers);
        let tab_before = tab_bar::geometry(&window, area, &metrics)
            .cards
            .unwrap()
            .height();
        configure(&mut window, pane_size, tab_size);
        let shapes = ui_pass(&mut window, &ctx, area, Default::default());
        assert_eq!(label_size(&shapes, "Pane typography sample"), pane_size);
        assert_eq!(label_size(&shapes, "Tab typography sample"), tab_size);
        let header_after = header::height(&metrics, window.headers);
        let tab_after = tab_bar::geometry(&window, area, &metrics)
            .cards
            .unwrap()
            .height();
        if pane_size == 32.0 {
            assert!(header_after > header_before, "larger pane text needs space");
            assert_eq!(tab_after, tab_before, "pane size changed the tab strip");
        } else {
            assert_eq!(header_after, header_before, "tab size changed pane strips");
            assert!(tab_after > tab_before, "larger tab text needs space");
        }
        assert_eq!(window.font_size, DEFAULT_FONT_SIZE);
    }
}

#[test]
fn title_typography_live_geometry_resizes_hidden_consoles_without_changing_columns() {
    let ctx = egui::Context::default();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
    window.chrome.enable();
    window.headers = window.headers.integrated(true);
    let area = area_of(1400.0);
    for _ in 0..2 {
        one_frame_in(&mut window, &ctx, area, vec![]);
    }
    let before: Vec<_> = window
        .tabs
        .iter()
        .flat_map(|tab| tab.panes())
        .map(|pane| (pane.name, pane.screen.size()))
        .collect();
    configure(&mut window, 32.0, 32.0);
    for _ in 0..3 {
        one_frame_in(&mut window, &ctx, area, vec![]);
    }
    for ((name, old), pane) in before
        .iter()
        .zip(window.tabs.iter().flat_map(|tab| tab.panes()))
    {
        assert_eq!(*name, pane.name);
        assert_eq!(pane.screen.size().cols, old.cols);
        assert!(
            pane.screen.size().rows < old.rows,
            "console {} kept its old rows",
            pane.name
        );
    }
    assert_eq!(
        window.active, 0,
        "hidden consoles were resized without visiting them"
    );
}

#[test]
fn title_typography_enlarged_header_keeps_its_lower_control_area_clickable() {
    let ctx = egui::Context::default();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    window.chrome.enable();
    window.headers = window.headers.integrated(true);
    configure(&mut window, 32.0, 8.0);
    let area = area_of(1400.0);
    one_frame_in(&mut window, &ctx, area, vec![]);
    let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
    let work = tab_bar::geometry(&window, area, &metrics).consoles;
    let rect = pane_view::view_tiling(&window, work, &metrics).parts[0];
    let at = egui::pos2(
        rect.right() - PANEL_GRIP - DIVIDER_THICKNESS - 12.5,
        rect.top() + header::height(&metrics, window.headers) - 5.0,
    );
    assert!(
        at.y > rect.top() + 33.0,
        "probe must be below the old strip"
    );
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
    assert_eq!(
        window.active().panes().len(),
        1,
        "close target did not follow title height"
    );
}
