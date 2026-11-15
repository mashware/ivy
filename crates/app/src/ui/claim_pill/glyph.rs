//! Application-owned attention marks do not depend on text-font coverage.
use eframe::egui;
use panel_contract::Attention;

pub(crate) fn paint(
    painter: &egui::Painter,
    rect: egui::Rect,
    class: Attention,
    color: egui::Color32,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let stroke = egui::Stroke::new(
        (rect.width() / 10.0)
            .max(1.0 / painter.ctx().pixels_per_point())
            .min(rect.width() / 4.0),
        color,
    );
    let r = rect.shrink(stroke.width / 2.0);
    let point = |x: f32, y: f32| egui::pos2(r.left() + r.width() * x, r.top() + r.height() * y);
    // One grouped shape keeps each identity inspectable without a hidden text glyph.
    let shapes = match class {
        Attention::Wait => vec![
            egui::Shape::Path(egui::epaint::PathShape::line(
                vec![
                    point(0.20, 0.28),
                    point(0.25, 0.14),
                    point(0.42, 0.06),
                    point(0.64, 0.07),
                    point(0.79, 0.19),
                    point(0.80, 0.36),
                    point(0.70, 0.49),
                    point(0.52, 0.59),
                    point(0.50, 0.71),
                ],
                stroke,
            )),
            egui::Shape::circle_filled(point(0.50, 0.91), stroke.width * 0.65, color),
        ],
        Attention::Block => vec![
            egui::Shape::circle_stroke(r.center(), r.width() / 2.0, stroke),
            egui::Shape::line_segment([point(0.16, 0.84), point(0.84, 0.16)], stroke),
        ],
        Attention::Done => vec![egui::Shape::rect_filled(r, 0.0, color)],
    };
    painter.add(egui::Shape::Vec(shapes));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_is_drawn_without_any_font_at_all_sizes_and_scales() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            ctx.set_fonts(egui::FontDefinitions::empty());
            for size in [8.0, 12.0, 20.0, 32.0] {
                for class in [Attention::Wait, Attention::Block, Attention::Done] {
                    let output = ctx.run_ui(Default::default(), |ui| {
                        let rect = egui::Rect::from_min_size(
                            egui::pos2(10.0, 10.0),
                            egui::Vec2::splat(size + 2.0),
                        );
                        paint(ui.painter(), rect, class, egui::Color32::RED);
                    });
                    let parts = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Vec(parts) => Some(parts),
                            _ => None,
                        })
                        .expect("status shapes exist without fonts");
                    assert!(
                        parts
                            .iter()
                            .all(|shape| !matches!(shape, egui::Shape::Text(_)))
                    );
                    match class {
                        Attention::Wait => assert!(matches!(
                            parts.as_slice(),
                            [egui::Shape::Path(_), egui::Shape::Circle(_)]
                        )),
                        Attention::Block => assert!(matches!(
                            parts.as_slice(),
                            [egui::Shape::Circle(_), egui::Shape::LineSegment { .. }]
                        )),
                        Attention::Done => {
                            assert!(matches!(parts.as_slice(), [egui::Shape::Rect(_)]))
                        }
                    }
                    let bounds = parts
                        .iter()
                        .map(egui::Shape::visual_bounding_rect)
                        .reduce(|a, b| a.union(b))
                        .unwrap();
                    assert!(
                        bounds.width() <= size + 2.1 && bounds.height() <= size + 2.1,
                        "{class:?}: {bounds:?}"
                    );
                    output.drop_without_applying_deltas();
                }
            }
        }
    }
}
