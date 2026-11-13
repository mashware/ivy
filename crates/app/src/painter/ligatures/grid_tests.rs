use super::*;

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
