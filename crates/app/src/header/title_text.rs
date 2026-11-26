//! Paint title text with font-independent braille and half-circle indicators.

use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Vec2};

/// Paint a single-line title, keeping ordinary text in egui-shaped runs.
pub(crate) fn paint(
    painter: &Painter,
    position: Pos2,
    anchor: Align2,
    text: &str,
    font: FontId,
    color: Color32,
) -> Vec2 {
    let half_circle = if text.starts_with("◐ ") {
        Some(true)
    } else if text.starts_with("◑ ") {
        Some(false)
    } else {
        None
    };
    if half_circle.is_none() && !text.chars().any(is_braille) {
        return painter.text(position, anchor, text, font, color).size();
    }

    enum Part {
        Text(std::sync::Arc<egui::Galley>),
        Braille(u8),
    }
    let reference = painter.layout_no_wrap(" ".to_owned(), font.clone(), color);
    let height = reference.size().y;
    // A braille cell has two columns and four rows; its width scales with the font.
    let advance = font.size * 0.6;
    let diameter = font.size * 0.7;
    let circle_width = half_circle.map_or(0.0, |_| diameter);
    let rest = if half_circle.is_some() {
        &text['◐'.len_utf8()..]
    } else {
        text
    };
    let mut parts = Vec::new();
    let mut width = circle_width;
    let mut start = 0;
    for (index, c) in rest.char_indices().filter(|(_, c)| is_braille(*c)) {
        if start < index {
            let galley = painter.layout_no_wrap(rest[start..index].to_owned(), font.clone(), color);
            width += galley.size().x;
            parts.push(Part::Text(galley));
        }
        parts.push(Part::Braille((c as u32 - 0x2800) as u8));
        width += advance;
        start = index + c.len_utf8();
    }
    if start < rest.len() {
        let galley = painter.layout_no_wrap(rest[start..].to_owned(), font, color);
        width += galley.size().x;
        parts.push(Part::Text(galley));
    }
    let size = egui::vec2(width, height);
    let origin = anchor.anchor_size(position, size).min;
    if let Some(left_filled) = half_circle {
        let center = origin + egui::vec2(diameter / 2.0, height / 2.0);
        paint_half_circle(painter, center, diameter, left_filled, color);
    }
    let mut x = origin.x + circle_width;
    for part in parts {
        match part {
            Part::Text(galley) => {
                let w = galley.size().x;
                painter.galley(egui::pos2(x, origin.y), galley, color);
                x += w;
            }
            Part::Braille(mask) => {
                let radius = (advance * 0.095).min(height * 0.045);
                for (column, bits) in [[0, 1, 2, 6], [3, 4, 5, 7]].iter().enumerate() {
                    for (row, bit) in bits.iter().enumerate() {
                        if mask & (1 << bit) != 0 {
                            painter.circle_filled(
                                egui::pos2(
                                    x + advance * (0.32 + column as f32 * 0.36),
                                    origin.y + height * (0.2 + row as f32 * 0.2),
                                ),
                                radius,
                                color,
                            );
                        }
                    }
                }
                x += advance;
            }
        }
    }
    size
}

fn paint_half_circle(
    painter: &Painter,
    center: Pos2,
    diameter: f32,
    left_filled: bool,
    color: Color32,
) {
    let stroke = egui::Stroke::new(diameter * 0.1, color);
    let radius = (diameter - stroke.width) / 2.0;
    let start = if left_filled { 0.5 } else { -0.5 };
    let vertices = (0..=12)
        .map(|step| {
            let angle = (start + step as f32 / 12.0) * std::f32::consts::PI;
            center + egui::vec2(angle.cos(), angle.sin()) * radius
        })
        .collect();
    painter.add(egui::Shape::convex_polygon(
        vertices,
        color,
        egui::Stroke::NONE,
    ));
    painter.circle_stroke(center, radius, stroke);
}

fn is_braille(c: char) -> bool {
    ('\u{2800}'..='\u{28ff}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Rect, epaint::Shape};

    fn scene(text: &str, clip: Rect) -> (Vec2, Vec<egui::epaint::ClippedShape>) {
        scene_at_size(text, clip, 20.0)
    }

    fn scene_at_size(text: &str, clip: Rect, size: f32) -> (Vec2, Vec<egui::epaint::ClippedShape>) {
        let ctx = egui::Context::default();
        ctx.begin_pass(Default::default());
        let painter = ctx
            .layer_painter(egui::LayerId::background())
            .with_clip_rect(clip);
        let size = paint(
            &painter,
            egui::pos2(10.0, 30.0),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(size),
            Color32::WHITE,
        );
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
        (size, output.shapes)
    }

    #[test]
    fn the_half_circle_frames_keep_the_outline_and_following_text_in_place() {
        let clip = Rect::from_min_max(egui::pos2(0.0, 10.0), egui::pos2(80.0, 50.0));
        for size in [8.0, 14.0, 24.0] {
            let (left_size, left) = scene_at_size("◐ Title", clip, size);
            let (right_size, right) = scene_at_size("◑ Title", clip, size);
            assert_eq!(left_size, right_size);
            assert!(
                left.iter()
                    .chain(&right)
                    .all(|shape| shape.clip_rect == clip)
            );
            let parts = |shapes: &[egui::epaint::ClippedShape]| match shapes {
                [fill, outline, title] => match (&fill.shape, &outline.shape, &title.shape) {
                    (Shape::Path(fill), Shape::Circle(outline), Shape::Text(title)) => {
                        assert_eq!(title.galley.job.text, " Title");
                        let middle = fill.points[fill.points.len() / 2];
                        (
                            middle,
                            outline.center,
                            outline.radius,
                            outline.stroke,
                            title.pos,
                        )
                    }
                    _ => panic!("expected a half fill, circular outline and title"),
                },
                _ => panic!("expected exactly three shapes"),
            };
            let (left_fill, left_center, left_radius, left_stroke, left_title) = parts(&left);
            let (right_fill, right_center, right_radius, right_stroke, right_title) = parts(&right);
            assert_eq!(left_center, right_center);
            assert_eq!(left_radius, right_radius);
            assert_eq!(left_stroke, right_stroke);
            assert_eq!(left_title, right_title);
            assert!((left_fill.x - (left_center.x - left_radius)).abs() < 0.001);
            assert!((right_fill.x - (right_center.x + right_radius)).abs() < 0.001);
        }
    }

    #[test]
    fn every_mask_paints_the_correct_dots_and_blank_advances() {
        let clip = Rect::EVERYTHING;
        let (blank, shapes) = scene("⠀", clip);
        assert!(shapes.is_empty());
        assert!(blank.x > 0.0);
        for mask in 0u32..=255 {
            let c = char::from_u32(0x2800 + mask).unwrap().to_string();
            let (size, shapes) = scene(&c, clip);
            assert_eq!(size, blank);
            let circles: Vec<_> = shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    Shape::Circle(c) => Some(c.center),
                    _ => None,
                })
                .collect();
            assert_eq!(circles.len(), mask.count_ones() as usize, "mask {mask}");
            for (column, bits) in [[0, 1, 2, 6], [3, 4, 5, 7]].iter().enumerate() {
                for (row, bit) in bits.iter().enumerate() {
                    let expected = egui::pos2(
                        10.0 + size.x * (0.32 + column as f32 * 0.36),
                        30.0 - size.y / 2.0 + size.y * (0.2 + row as f32 * 0.2),
                    );
                    assert_eq!(
                        circles.iter().any(|p| p.distance(expected) < 0.001),
                        mask & (1 << bit) != 0,
                        "mask {mask}, bit {bit}"
                    );
                }
            }
        }
    }

    #[test]
    fn mixed_runs_keep_order_and_caller_clip() {
        let clip = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(20.0, 50.0));
        let (_, shapes) = scene("é界⠁🙂z", clip);
        assert!(shapes.iter().all(|s| s.clip_rect == clip));
        assert_eq!(shapes.len(), 3);
        let (left, dot, right) = match (&shapes[0].shape, &shapes[1].shape, &shapes[2].shape) {
            (Shape::Text(a), Shape::Circle(b), Shape::Text(c)) => (a, b, c),
            _ => panic!("expected text, dot, text"),
        };
        assert_eq!(left.galley.job.text, "é界");
        assert_eq!(right.galley.job.text, "🙂z");
        assert!(left.pos.x < dot.center.x && dot.center.x < right.pos.x);
    }
}
