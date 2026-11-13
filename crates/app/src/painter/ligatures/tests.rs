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
            vec![
                0..2,
                2..3,
                3..4,
                5..7,
                7..8,
                8..9,
                10..12,
                12..13,
                13..14,
                15..17,
                17..18,
                18..19,
                20..24
            ]
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

#[derive(Clone, Debug)]
struct Quad {
    rect: Rect,
    uv: Vec<Pos2>,
}

fn quads(galley: &Galley) -> Vec<Quad> {
    galley.rows[0]
        .visuals
        .mesh
        .vertices
        .chunks_exact(4)
        .map(|vertices| {
            let min_x = vertices
                .iter()
                .map(|v| v.pos.x)
                .fold(f32::INFINITY, f32::min);
            let max_x = vertices
                .iter()
                .map(|v| v.pos.x)
                .fold(f32::NEG_INFINITY, f32::max);
            let min_y = vertices
                .iter()
                .map(|v| v.pos.y)
                .fold(f32::INFINITY, f32::min);
            let max_y = vertices
                .iter()
                .map(|v| v.pos.y)
                .fold(f32::NEG_INFINITY, f32::max);
            Quad {
                rect: Rect::from_min_max(pos2(min_x, min_y), pos2(max_x, max_y)),
                uv: vertices.iter().map(|v| v.uv).collect(),
            }
        })
        .collect()
}

fn assert_same_quads(actual: &[Quad], expected: &[Quad], scale: f32, context: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{context}: visible quad count"
    );
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        // Compare geometry in device pixels so this tolerance stays meaningful at every scale.
        let close = |a: f32, b: f32| ((a - b) * scale).abs() <= 0.001;
        assert!(
            close(actual.rect.min.x, expected.rect.min.x),
            "{context} quad {index}: left edge {:?} != {:?}",
            actual,
            expected
        );
        assert!(
            close(actual.rect.max.x, expected.rect.max.x),
            "{context} quad {index}: right edge {:?} != {:?}",
            actual,
            expected
        );
        assert!(
            close(actual.rect.min.y, expected.rect.min.y),
            "{context} quad {index}: top edge {:?} != {:?}",
            actual,
            expected
        );
        assert!(
            close(actual.rect.max.y, expected.rect.max.y),
            "{context} quad {index}: bottom edge {:?} != {:?}",
            actual,
            expected
        );
        assert_eq!(
            actual.uv, expected.uv,
            "{context} quad {index}: atlas UVs differ"
        );
    }
}

fn per_cell_quads(
    painter: &Painter,
    text: &str,
    font: &FontId,
    metrics: &CellMetrics,
) -> Vec<Quad> {
    text.chars()
        .enumerate()
        .flat_map(|(column, character)| {
            let one = painter.ctx().fonts_mut(|fonts| {
                fonts.layout_no_wrap(character.to_string(), font.clone(), Color32::WHITE)
            });
            let mut visible = quads(&one);
            assert_eq!(visible.len(), 1, "{character:?} must have one visible quad");
            for quad in &mut visible {
                quad.rect = quad
                    .rect
                    .translate(vec2(column as f32 * metrics.width, 0.0));
            }
            visible
        })
        .collect()
}

#[test]
fn ordinary_ascii_matches_per_cell_and_is_invariant_across_selection_splits() {
    // This command-like string contains ordinary ASCII and no fixture substitutions.
    let plain = "repo/mashware/agent/runner/body/file";
    // Exercise accumulated placement error as well as both requested split columns.
    let text = plain.repeat(8);

    for scale in [1.0, 1.25, 1.5, 2.0] {
        for size in [13.0, 14.0, 14.7, 17.3] {
            let ctx = tests::context();
            ctx.set_pixels_per_point(scale);
            ctx.run_ui(Default::default(), |ui| {
                let font = FontId::new(size, eframe::egui::FontFamily::Name("fixture".into()));
                let metrics = CellMetrics::measure(&ctx, font.clone());
                let full = layout(
                    ui.painter(),
                    text.clone(),
                    font.clone(),
                    Color32::WHITE,
                    &metrics,
                )
                .expect("ordinary ASCII run must be shaped");
                let full_quads = quads(&full);
                assert_eq!(
                    full_quads.len(),
                    text.len(),
                    "one ordinary glyph quad per source character"
                );
                let cells = per_cell_quads(ui.painter(), &text, &font, &metrics);
                // Place the whole row at a nonzero, device-pixel-aligned origin.
                let origin_x = (13.0 * scale).round() / scale;
                let place_at_origin = |mut items: Vec<Quad>| {
                    for quad in &mut items {
                        quad.rect = quad.rect.translate(vec2(origin_x, 0.0));
                    }
                    items
                };
                let full_quads = place_at_origin(full_quads);
                let cells = place_at_origin(cells);
                assert_same_quads(&full_quads, &cells, scale, "full run vs per-cell");
                for quad in &full_quads {
                    for edge in [quad.rect.min.x, quad.rect.max.x] {
                        let device_x = (origin_x + edge) * scale;
                        assert!(
                            (device_x - device_x.round()).abs() <= 0.01,
                            "size={size}, scale={scale}: quad edge {device_x} is off device pixel"
                        );
                    }
                }

                // Each partition models a style boundary introduced by selection. Re-layout every
                // resulting run, restore its source-column origin, and compare the complete row.
                for cuts in [&[7usize][..], &[19usize][..], &[7usize, 19usize][..]] {
                    let mut starts = vec![0];
                    starts.extend(cuts.iter().copied());
                    starts.push(text.len());
                    let mut split_quads = Vec::new();
                    for pair in starts.windows(2) {
                        let start = pair[0];
                        let end = pair[1];
                        let segment = layout(
                            ui.painter(),
                            text[start..end].to_owned(),
                            font.clone(),
                            Color32::WHITE,
                            &metrics,
                        )
                        .expect("ordinary ASCII segment must be shaped");
                        for mut quad in quads(&segment) {
                            quad.rect = quad
                                .rect
                                .translate(vec2(origin_x + start as f32 * metrics.width, 0.0));
                            split_quads.push(quad);
                        }
                    }
                    assert_same_quads(
                        &split_quads,
                        &full_quads,
                        scale,
                        &format!("split at {cuts:?}, size={size}, scale={scale}"),
                    );
                }
            })
            .drop_without_applying_deltas();
        }
    }
}

fn painted_text_quads(output: &eframe::egui::FullOutput) -> Vec<Quad> {
    let mut result = output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            eframe::egui::Shape::Text(text) => Some(text),
            _ => None,
        })
        .flat_map(|text| {
            let mut visible = quads(&text.galley);
            let origin = text.pos.to_vec2() + text.galley.rows[0].pos.to_vec2();
            for quad in &mut visible {
                quad.rect = quad.rect.translate(origin);
            }
            visible
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| a.rect.min.x.total_cmp(&b.rect.min.x));
    result
}

#[test]
fn selection_does_not_change_unselected_singleton_glyphs_at_either_edge() {
    let text = "repo/mashware/agent/runner/body/file";
    let edge_columns = [0, text.len() - 1];

    for scale in [1.0, 1.25, 1.5, 2.0] {
        for size in [13.0, 14.0, 14.7, 17.3] {
            let ctx = Context::default();
            ctx.set_pixels_per_point(scale);
            let mut discoverer = selected(&ctx);
            // The queued face becomes active on this pass; both captures then use the same atlas.
            ctx.run_ui(Default::default(), |_| {})
                .drop_without_applying_deltas();

            let mut screen = Screen::new(Size {
                rows: 1,
                cols: text.len() as u16,
            });
            let input = format!("{text}\x1b[?25l");
            screen.advance(input.as_bytes());
            let view = screen.view();
            let before = ctx.run_ui(Default::default(), |ui| {
                let metrics = CellMetrics::measure(&ctx, FontId::monospace(size));
                paint_with_ligatures(
                    ui.painter(),
                    &view,
                    &Theme::default(),
                    &metrics,
                    Pos2::ZERO,
                    &mut discoverer,
                    CursorInk::Filled,
                    true,
                );
            });
            let before_quads = painted_text_quads(&before);
            before.drop_without_applying_deltas();
            assert_eq!(before_quads.len(), text.len(), "unselected row lost glyphs");

            screen.start_selection(term_core::Cell {
                row: 0,
                col: 1,
                half: term_core::Half::Left,
            });
            screen.extend_selection(term_core::Cell {
                row: 0,
                col: (text.len() - 2) as u16,
                half: term_core::Half::Right,
            });
            let selected_view = screen.view();
            assert!(!selected_view.cell(0, 0).unwrap().selected);
            assert!(selected_view.cell(0, 1).unwrap().selected);
            assert!(
                selected_view
                    .cell(0, (text.len() - 2) as u16)
                    .unwrap()
                    .selected
            );
            assert!(
                !selected_view
                    .cell(0, (text.len() - 1) as u16)
                    .unwrap()
                    .selected
            );
            let after = ctx.run_ui(Default::default(), |ui| {
                let metrics = CellMetrics::measure(&ctx, FontId::monospace(size));
                paint_with_ligatures(
                    ui.painter(),
                    &selected_view,
                    &Theme::default(),
                    &metrics,
                    Pos2::ZERO,
                    &mut discoverer,
                    CursorInk::Filled,
                    true,
                );
            });
            let after_quads = painted_text_quads(&after);
            after.drop_without_applying_deltas();
            assert_eq!(after_quads.len(), text.len(), "selected row lost glyphs");

            for column in edge_columns {
                let before = &before_quads[column];
                let after = &after_quads[column];
                assert_eq!(
                    before.uv, after.uv,
                    "size={size}, scale={scale}, unselected edge column {column}: atlas UV changed"
                );
                let same_device_edge = |a: f32, b: f32| ((a - b) * scale).abs() <= 0.001;
                assert!(
                    same_device_edge(before.rect.min.x, after.rect.min.x)
                        && same_device_edge(before.rect.max.x, after.rect.max.x)
                        && same_device_edge(before.rect.min.y, after.rect.min.y)
                        && same_device_edge(before.rect.max.y, after.rect.max.y),
                    "size={size}, scale={scale}, unselected edge column {column}: glyph geometry changed"
                );
            }
        }
    }
}
