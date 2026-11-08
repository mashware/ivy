//! Optional shaped ASCII runs, adapted once to the terminal's fixed column step.
use super::*;
use eframe::egui::epaint::text::Galley;
use std::sync::Arc;

/// Clone before transforming: epaint owns and shares its cached layout and texel UVs.
fn on_grid(source: &Arc<Galley>, advance: f32, metrics: &CellMetrics) -> Arc<Galley> {
    let ratio = metrics.width / advance;
    let mut galley = (**source).clone();
    let scale_rect = |rect: &mut Rect| {
        rect.min.x *= ratio;
        rect.max.x *= ratio;
    };
    scale_rect(&mut galley.rect);
    scale_rect(&mut galley.mesh_bounds);
    for placed in &mut galley.rows {
        placed.pos.x *= ratio;
        let row = Arc::make_mut(&mut placed.row);
        row.size.x *= ratio;
        for glyph in &mut row.glyphs {
            glyph.pos.x *= ratio;
            glyph.advance_width *= ratio;
        }
        for vertex in &mut row.visuals.mesh.vertices {
            vertex.pos.x *= ratio;
        }
        scale_rect(&mut row.visuals.mesh_bounds);
    }
    Arc::new(galley)
}

fn layout(
    painter: &Painter,
    text: String,
    font: FontId,
    color: Color32,
    metrics: &CellMetrics,
) -> Option<Arc<Galley>> {
    painter.ctx().fonts_mut(|fonts| {
        let advance = fonts.glyph_width(&font, 'M');
        if !advance.is_finite()
            || advance <= 0.0
            || text
                .chars()
                .any(|c| (fonts.glyph_width(&font, c) - advance).abs() > 0.001)
        {
            return None;
        }
        let expected = text.len() as f32 * advance;
        let shaped = fonts.layout_no_wrap(text, font, color);
        if shaped.rows.len() != 1 || (shaped.rows[0].size.x - expected).abs() > metrics.a_pixel() {
            return None;
        }
        Some(on_grid(&shaped, advance, metrics))
    })
}

fn eligible(view: &View, cell: &ViewCell, row: u16, col: usize) -> bool {
    cell.c.is_ascii_graphic()
        && !cell.hidden
        && cell.width == CellWidth::Single
        && view.combining_of(cell).is_empty()
        && !view
            .cursor()
            .is_some_and(|cursor| cursor.row == row && cursor.col as usize == col)
}

fn same_style(a: &ViewCell, b: &ViewCell) -> bool {
    (
        a.fg,
        a.bg,
        a.inverse,
        a.bold,
        a.dim,
        a.italic,
        a.underline,
        a.strikeout,
        a.selected,
        a.matched,
    ) == (
        b.fg,
        b.bg,
        b.inverse,
        b.bold,
        b.dim,
        b.italic,
        b.underline,
        b.strikeout,
        b.selected,
        b.matched,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "row shaping uses the same geometry and font inputs as painting"
)]
pub(super) fn paint_row(
    painter: &Painter,
    view: &View,
    cells: &[ViewCell],
    row: u16,
    theme: &Theme,
    metrics: &CellMetrics,
    origin: Pos2,
    discoverer: &mut crate::fonts::Discoverer,
) -> Vec<std::ops::Range<usize>> {
    let before = std::time::Instant::now();
    let mut cost = crate::fonts::LigatureCost::default();
    let mut drawn = Vec::new();
    let family = crate::fonts::ligature_family(discoverer, painter.ctx());
    let mut start = 0;
    while start < cells.len() {
        if !eligible(view, &cells[start], row, start) {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        while end < cells.len()
            && eligible(view, &cells[end], row, end)
            && same_style(&cells[start], &cells[end])
        {
            end += 1;
        }
        if end - start >= 2 {
            cost.eligible_runs += 1;
            let text: String = cells[start..end].iter().map(|cell| cell.c).collect();
            let galley = family
                .as_ref()
                .filter(|_| crate::fonts::ligature_covers(discoverer, &text))
                .and_then(|family| {
                    layout(
                        painter,
                        text.clone(),
                        FontId::new(metrics.font.size, family.clone()),
                        colors_of(&cells[start], theme).0,
                        metrics,
                    )
                });
            if let Some(galley) = galley {
                cost.shaped_runs += 1;
                cost.vertices += galley.num_vertices;
                painter.galley(
                    metrics.cell(origin, row, start as u16).min,
                    galley,
                    Color32::WHITE,
                );
                drawn.push(start..end);
            } else {
                cost.fallback_runs += 1;
            }
        }
        start = end;
    }
    cost.spent = before.elapsed();
    crate::fonts::record_ligatures(discoverer, cost);
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn context() -> Context {
        let ctx = Context::default();
        let mut definitions = eframe::egui::FontDefinitions::default();
        definitions.font_data.insert(
            "fixture".into(),
            Arc::new(eframe::egui::FontData::from_static(include_bytes!(
                "../../tests/fixtures/ligatures/JetBrainsMono-Regular.ttf"
            ))),
        );
        definitions.families.insert(
            eframe::egui::FontFamily::Name("fixture".into()),
            vec!["fixture".into()],
        );
        ctx.set_fonts(definitions);
        ctx
    }
    #[test]
    fn real_ligature_ink_and_fixed_grid_survive_fractional_sizes_scales_and_long_runs() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for size in [13.0, 14.7, 17.3] {
                let ctx = context();
                ctx.set_pixels_per_point(scale);
                ctx.run_ui(Default::default(), |ui| {
                    let font = FontId::new(size, eframe::egui::FontFamily::Name("fixture".into()));
                    let metrics = CellMetrics::measure(&ctx, font.clone());
                    for text in ["!=", "=>", "==", "==="] {
                        let shaped = layout(
                            ui.painter(),
                            text.into(),
                            font.clone(),
                            Color32::WHITE,
                            &metrics,
                        )
                        .expect("supported font must shape");
                        let uv = |g: &Galley| {
                            g.rows[0]
                                .visuals
                                .mesh
                                .vertices
                                .iter()
                                .map(|v| v.uv)
                                .collect::<Vec<_>>()
                        };
                        let joined_uv = uv(&shaped);
                        // Separate shaping sections preserve positions and raster subpixel phases,
                        // so a differing patch proves substitution rather than merely raster rounding.
                        let separated = [ctx.fonts_mut(|fonts| {
                            fonts.layout_job(eframe::egui::text::LayoutJob {
                                sections: (0..text.len())
                                    .map(|i| eframe::egui::text::LayoutSection {
                                        leading_space: 0.0,
                                        byte_range: eframe::egui::text::ByteIndex(i)
                                            ..eframe::egui::text::ByteIndex(i + 1),
                                        format: eframe::egui::TextFormat::simple(
                                            font.clone(),
                                            Color32::WHITE,
                                        ),
                                    })
                                    .collect(),
                                text: text.into(),
                                ..Default::default()
                            })
                        })];
                        let separated_uv = separated.iter().flat_map(|g| uv(g)).collect::<Vec<_>>();
                        let image = ctx.fonts(|fonts| fonts.image());
                        let image = &image;
                        // Compare raster pixels, not only glyph count or differing atlas addresses.
                        let ink = |g: &Galley| {
                            g.rows[0]
                                .visuals
                                .mesh
                                .vertices
                                .chunks_exact(4)
                                .map(|quad| {
                                    let x0 = quad.iter().map(|v| v.uv.x as usize).min().unwrap();
                                    let y0 = quad.iter().map(|v| v.uv.y as usize).min().unwrap();
                                    let x1 = quad.iter().map(|v| v.uv.x as usize).max().unwrap();
                                    let y1 = quad.iter().map(|v| v.uv.y as usize).max().unwrap();
                                    let pixels = (y0..y1)
                                        .flat_map(|y| (x0..x1).map(move |x| image[(x, y)].a()))
                                        .collect::<Vec<_>>();
                                    (x1 - x0, y1 - y0, pixels)
                                })
                                .collect::<Vec<_>>()
                        };
                        let joined_ink = ink(&shaped);
                        let separate_ink =
                            separated.iter().flat_map(|g| ink(g)).collect::<Vec<_>>();
                        assert_ne!(
                            joined_ink, separate_ink,
                            "{text}: substituted glyphs have identical raster ink"
                        );
                        assert_ne!(
                            joined_uv, separated_uv,
                            "{text}: font substituted no visible ink"
                        );
                        assert!(!joined_uv.is_empty());
                    }
                    for text in ["a!=b=>c==d===e".to_string(), "a!=b=>c==d===e".repeat(70)] {
                        let source = ctx.fonts_mut(|f| {
                            f.layout_no_wrap(text.clone(), font.clone(), Color32::WHITE)
                        });
                        let before = (*source).clone();
                        let advance = ctx.fonts_mut(|f| f.glyph_width(&font, 'M'));
                        let transformed = layout(
                            ui.painter(),
                            text.clone(),
                            font.clone(),
                            Color32::WHITE,
                            &metrics,
                        )
                        .unwrap();
                        assert_eq!(*source, before, "shared cached galley mutated");
                        let right = transformed.rows[0].size.x;
                        assert!(
                            (right - text.len() as f32 * metrics.width).abs() <= metrics.a_pixel(),
                            "accumulated drift {right}"
                        );
                        for (a, b) in source.rows[0]
                            .visuals
                            .mesh
                            .vertices
                            .iter()
                            .zip(&transformed.rows[0].visuals.mesh.vertices)
                        {
                            assert_eq!(a.uv, b.uv, "texel UV changed");
                            assert_eq!(b.pos.x, a.pos.x * (metrics.width / advance));
                        }
                    }
                })
                .drop_without_applying_deltas();
            }
        }
    }
}

#[cfg(test)]
#[path = "ligatures/tests.rs"]
mod integration_tests;
