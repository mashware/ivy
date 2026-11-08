use super::*;
use crate::fonts::Discoverer;
use term_core::{Screen, Size};

fn selected(ctx: &Context) -> Discoverer {
    let mut discoverer = Discoverer::default();
    crate::fonts::use_measurement_face(
        &mut discoverer,
        eframe::egui::FontData::from_static(include_bytes!(
            "../../../tests/fixtures/ligatures/JetBrainsMono-Regular.ttf"
        )),
        ctx,
    );
    discoverer
}

#[test]
fn runs_stop_at_visual_boundaries_and_keep_the_original_cells() {
    let ctx = Context::default();
    let mut discoverer = selected(&ctx);
    let mut screen = Screen::new(Size { rows: 2, cols: 32 });
    screen.advance(b"==!= ==!= ==!= ==!= ==!=\x1b[2;32H");
    let view = screen.view();
    let mut cells = view.cells()[..32].to_vec();
    cells[2].selected = true;
    cells[7].matched = true;
    cells[12].fg = TermColor::Indexed(1);
    cells[17].bold = true;
    let before = cells.clone();
    ctx.run_ui(Default::default(), |ui| {
        let metrics = CellMetrics::measure(&ctx, FontId::monospace(14.7));
        let ranges = paint_row(
            ui.painter(),
            &view,
            &cells,
            0,
            &Theme::default(),
            &metrics,
            Pos2::ZERO,
            &mut discoverer,
        );
        assert_eq!(
            ranges,
            vec![0..2, 5..7, 10..12, 15..17, 18..19, 20..24]
                .into_iter()
                .filter(|r| r.end - r.start >= 2)
                .collect::<Vec<_>>()
        );
        assert_eq!(cells, before);
        assert_eq!(view.cells()[0].c, '=');
    })
    .drop_without_applying_deltas();
    assert!(!discoverer.has_searched_the_machine());
    // Cursor cells are rejected even when the block cursor will paint in a later pass.
    let cursor = view.cursor().unwrap();
    assert!(!eligible(
        &view,
        view.cell(cursor.row, cursor.col).unwrap(),
        cursor.row,
        cursor.col as usize
    ));
    let mut mixed = Screen::new(Size { rows: 1, cols: 24 });
    mixed.advance("a\u{301}中😀█\u{e000}".as_bytes());
    let mixed = mixed.view();
    for (col, cell) in mixed.cells().iter().enumerate().take(7) {
        assert!(
            !eligible(&mixed, cell, 0, col),
            "non-ASCII/combining cell admitted: {cell:?}"
        );
    }
}

#[test]
fn off_and_no_selected_face_preserve_per_cell_text_and_do_not_discover_fonts() {
    for enabled in [false, true] {
        let ctx = Context::default();
        let mut discoverer = Discoverer::default();
        let mut screen = Screen::new(Size { rows: 1, cols: 16 });
        screen.advance(b"!= => ===\x1b[?25l");
        let original = screen.view().cells().to_vec();
        let output = ctx.run_ui(Default::default(), |ui| {
            let metrics = CellMetrics::measure(&ctx, FontId::monospace(14.7));
            paint_with_ligatures(
                ui.painter(),
                &screen.view(),
                &Theme::default(),
                &metrics,
                Pos2::ZERO,
                &mut discoverer,
                CursorInk::Filled,
                enabled,
            );
        });
        let texts = output
            .shapes
            .iter()
            .filter(|shape| matches!(shape.shape, eframe::egui::Shape::Text(_)))
            .count();
        assert_eq!(texts, 7, "fallback lost individual source characters");
        assert_eq!(screen.view().cells(), original);
        assert!(!discoverer.has_searched_the_machine());
        let cost = discoverer.ligature_cost();
        assert_eq!(cost.shaped_runs, 0);
        assert_eq!(cost.vertices, 0);
        if enabled {
            assert_eq!(cost.fallback_runs, 3);
        } else {
            assert_eq!(cost.spent, Duration::ZERO);
            assert_eq!(cost.eligible_runs, 0);
        }
        output.drop_without_applying_deltas();
    }
}

#[test]
fn registration_waits_for_the_active_engine_and_incompatible_metrics_fall_back() {
    let ctx = Context::default();
    ctx.run_ui(Default::default(), |_| {})
        .drop_without_applying_deltas();
    let discoverer = selected(&ctx);
    assert!(
        crate::fonts::ligature_family(&discoverer, &ctx).is_none(),
        "pending family was used in registration pass"
    );
    ctx.run_ui(Default::default(), |ui| {
        assert!(crate::fonts::ligature_family(&discoverer, &ctx).is_some());
        let metrics = CellMetrics::measure(&ctx, FontId::monospace(14.0));
        assert!(
            layout(
                ui.painter(),
                "WiWi".into(),
                FontId::proportional(14.0),
                Color32::WHITE,
                &metrics
            )
            .is_none()
        );
    })
    .drop_without_applying_deltas();
}

#[test]
fn later_atlas_growth_preserves_earlier_ligature_texels_until_text_tessellation() {
    let ctx = super::tests::context();
    let mut earlier = None;
    let mut before_size = [0, 0];
    let mut after_size = [0, 0];
    let output = ctx.run_ui(Default::default(), |ui| {
        // Later layouts may grow the same atlas after this ligature was queued.
        let font = FontId::new(14.7, eframe::egui::FontFamily::Name("fixture".into()));
        let metrics = CellMetrics::measure(&ctx, font.clone());
        let galley = layout(ui.painter(), "===>".into(), font, Color32::WHITE, &metrics).unwrap();
        before_size = ctx.fonts(|f| f.font_image_size());
        ui.painter()
            .galley(Pos2::ZERO, galley.clone(), Color32::WHITE);
        earlier = Some(ctx.fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                "===>".into(),
                FontId::new(14.7, eframe::egui::FontFamily::Name("fixture".into())),
                Color32::WHITE,
            )
        }));
        ctx.fonts_mut(|fonts| {
            let alphabet: String = ('!'..='~').collect();
            for size in [40.0, 60.0, 90.0, 120.0] {
                fonts.layout_no_wrap(alphabet.clone(), FontId::monospace(size), Color32::WHITE);
            }
        });
        after_size = ctx.fonts(|f| f.font_image_size());
    });
    assert!(
        after_size[1] > before_size[1],
        "fixture did not grow the atlas"
    );
    let galley = earlier.unwrap();
    let texels = &galley.rows[0].visuals.mesh.vertices;
    let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
    let mesh = primitives
        .iter()
        .find_map(|p| match &p.primitive {
            eframe::egui::epaint::Primitive::Mesh(m) => Some(m),
            _ => None,
        })
        .unwrap();
    assert_eq!(mesh.vertices.len(), texels.len());
    for (v, t) in mesh.vertices.iter().zip(texels) {
        assert!((v.uv.x - t.uv.x / after_size[0] as f32).abs() < 1e-6);
        assert!((v.uv.y - t.uv.y / after_size[1] as f32).abs() < 1e-6);
    }
    output.drop_without_applying_deltas();
}
