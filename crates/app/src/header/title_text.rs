//! Paint title text with font-independent Unicode braille cells.

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
    if !text.chars().any(is_braille) {
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
    let mut parts = Vec::new();
    let mut width = 0.0;
    let mut start = 0;
    for (index, c) in text.char_indices().filter(|(_, c)| is_braille(*c)) {
        if start < index {
            let galley = painter.layout_no_wrap(text[start..index].to_owned(), font.clone(), color);
            width += galley.size().x;
            parts.push(Part::Text(galley));
        }
        parts.push(Part::Braille((c as u32 - 0x2800) as u8));
        width += advance;
        start = index + c.len_utf8();
    }
    if start < text.len() {
        let galley = painter.layout_no_wrap(text[start..].to_owned(), font, color);
        width += galley.size().x;
        parts.push(Part::Text(galley));
    }
    let size = egui::vec2(width, height);
    let origin = anchor.anchor_size(position, size).min;
    let mut x = origin.x;
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

fn is_braille(c: char) -> bool {
    ('\u{2800}'..='\u{28ff}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Rect, epaint::Shape};

    fn scene(text: &str, clip: Rect) -> (Vec2, Vec<egui::epaint::ClippedShape>) {
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
            FontId::proportional(20.0),
            Color32::WHITE,
        );
        let mut output = ctx.end_pass();
        output.textures_delta.clear();
        (size, output.shapes)
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
