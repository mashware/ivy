//! The kit's line icons (`Window.dc.html`, `GitDiff.dc.html`), drawn from their 16-unit SVG paths
//! so the title bar, the console headers and the drawer share one set (D96, D72).
use eframe::egui::{self, Color32, Pos2, Stroke, epaint::PathShape};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Icon {
    Files,
    Panel,
    SplitRight,
    SplitDown,
    Markdown,
    Search,
    Connect,
    Menu,
    Board,
    Minimize,
    Maximize,
    Restore,
    Close,
    Branch,
    Expand,
    Collapse,
    Refresh,
    File,
    Folder,
}

/// Paint `icon` centred on `center`, `size` points across, as the kit's 16-unit path.
pub(crate) fn paint(painter: &egui::Painter, center: Pos2, icon: Icon, size: f32, color: Color32) {
    let scale = size / 16.0;
    let at = |x: f32, y: f32| center + egui::vec2(x - 8.0, y - 8.0) * scale;
    let width = match icon {
        Icon::Search | Icon::Menu | Icon::Expand | Icon::Collapse | Icon::Close => 1.5,
        _ => 1.4,
    };
    let stroke = Stroke::new(width * scale.max(0.75), color);
    let polyline = |points: &[(f32, f32)], closed: bool| {
        let points = points.iter().map(|(x, y)| at(*x, *y)).collect();
        painter.add(if closed {
            PathShape::closed_line(points, stroke)
        } else {
            PathShape::line(points, stroke)
        });
    };
    let line = |a: (f32, f32), b: (f32, f32)| polyline(&[a, b], false);
    let rounded = |x: f32, y: f32, w: f32, h: f32, r: f32| {
        painter.rect_stroke(
            egui::Rect::from_min_max(at(x, y), at(x + w, y + h)),
            r * scale,
            stroke,
            egui::StrokeKind::Middle,
        );
    };
    match icon {
        Icon::Files => {
            rounded(1.5, 2.5, 13.0, 11.0, 2.0);
            line((6.0, 2.5), (6.0, 13.5));
        }
        Icon::Panel => {
            rounded(1.5, 2.5, 13.0, 11.0, 2.0);
            line((10.0, 2.5), (10.0, 13.5));
        }
        Icon::SplitRight => {
            rounded(1.5, 2.5, 13.0, 11.0, 2.0);
            line((8.0, 2.5), (8.0, 13.5));
        }
        Icon::SplitDown => {
            rounded(1.5, 2.5, 13.0, 11.0, 2.0);
            line((1.5, 8.0), (14.5, 8.0));
        }
        Icon::Markdown => {
            polyline(
                &[
                    (3.5, 1.5),
                    (9.5, 1.5),
                    (12.5, 4.5),
                    (12.5, 14.5),
                    (3.5, 14.5),
                ],
                true,
            );
            line((5.5, 8.0), (10.5, 8.0));
            line((5.5, 10.5), (10.5, 10.5));
            line((5.5, 5.5), (7.5, 5.5));
        }
        Icon::Search => {
            painter.circle_stroke(at(7.0, 7.0), 4.5 * scale, stroke);
            line((10.5, 10.5), (14.0, 14.0));
        }
        Icon::Connect => {
            line((6.0, 10.0), (10.0, 6.0));
            let mut lower = vec![(5.0, 7.5), (3.5, 9.0)];
            lower.extend(arc((3.5, 9.0), (7.0, 12.5), 2.5, false));
            lower.push((8.5, 11.0));
            polyline(&lower, false);
            let mut upper = vec![(7.5, 5.0), (9.0, 3.5)];
            upper.extend(arc((9.0, 3.5), (12.5, 7.0), 2.5, true));
            upper.push((11.0, 8.5));
            polyline(&upper, false);
        }
        Icon::Menu => {
            for y in [4.5, 8.0, 11.5] {
                line((3.0, y), (13.0, y));
            }
        }
        Icon::Board => {
            for (x, y) in [(2.0, 2.0), (9.0, 2.0), (2.0, 9.0), (9.0, 9.0)] {
                rounded(x, y, 5.0, 5.0, 1.0);
            }
        }
        Icon::Minimize => line((3.0, 8.0), (13.0, 8.0)),
        Icon::Maximize => rounded(3.0, 3.0, 10.0, 10.0, 1.5),
        Icon::Restore => {
            rounded(2.5, 5.5, 8.0, 8.0, 1.5);
            polyline(
                &[
                    (5.5, 5.5),
                    (5.5, 2.5),
                    (13.5, 2.5),
                    (13.5, 10.5),
                    (10.5, 10.5),
                ],
                false,
            );
        }
        Icon::Close => {
            line((4.0, 4.0), (12.0, 12.0));
            line((12.0, 4.0), (4.0, 12.0));
        }
        Icon::Branch => {
            for (x, y) in [(4.5, 3.5), (4.5, 12.5), (11.5, 6.5)] {
                painter.circle_stroke(at(x, y), 1.8 * scale, stroke);
            }
            line((4.5, 5.3), (4.5, 10.7));
            polyline(
                &[
                    (11.5, 8.3),
                    (11.2, 9.6),
                    (10.0, 10.4),
                    (8.0, 10.9),
                    (5.0, 11.5),
                ],
                false,
            );
        }
        Icon::Expand => {
            polyline(&[(9.5, 2.5), (13.5, 2.5), (13.5, 6.5)], false);
            polyline(&[(6.5, 13.5), (2.5, 13.5), (2.5, 9.5)], false);
            line((13.5, 2.5), (9.0, 7.0));
            line((2.5, 13.5), (7.0, 9.0));
        }
        Icon::Collapse => {
            polyline(&[(9.0, 3.0), (9.0, 7.0), (13.0, 7.0)], false);
            polyline(&[(7.0, 13.0), (7.0, 9.0), (3.0, 9.0)], false);
            line((9.0, 7.0), (13.5, 2.5));
            line((7.0, 9.0), (2.5, 13.5));
        }
        Icon::Refresh => {
            let mut points = arc((13.0, 8.0), (8.0, 3.0), 5.0, false);
            points.extend(arc((8.0, 3.0), (3.0, 8.0), 5.0, false));
            points.extend(arc((3.0, 8.0), (8.0, 13.0), 5.0, false));
            polyline(&points, false);
            polyline(&[(13.0, 3.5), (13.0, 7.0), (9.5, 7.0)], false);
        }
        Icon::File => {
            polyline(
                &[
                    (4.0, 1.5),
                    (9.0, 1.5),
                    (12.5, 5.0),
                    (12.5, 14.5),
                    (4.0, 14.5),
                ],
                true,
            );
        }
        Icon::Folder => {
            polyline(
                &[
                    (1.5, 4.5),
                    (1.5, 13.0),
                    (14.5, 13.0),
                    (14.5, 5.5),
                    (8.0, 5.5),
                    (6.5, 3.5),
                    (1.5, 3.5),
                ],
                true,
            );
        }
    }
}

/// Points along the shorter circular arc of `radius` from `from` to `to`, `from` excluded;
/// `clockwise` as on screen (y grows downwards), which is SVG's sweep flag 1.
fn arc(from: (f32, f32), to: (f32, f32), radius: f32, clockwise: bool) -> Vec<(f32, f32)> {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let chord = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
    let half = (chord / 2.0).min(radius);
    let offset = (radius * radius - half * half).max(0.0).sqrt();
    let middle = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    // The centre sits on the side that makes the short arc turn the asked way.
    let side = if clockwise { 1.0 } else { -1.0 };
    let center = (
        middle.0 - side * dy / chord * offset,
        middle.1 + side * dx / chord * offset,
    );
    let start = (from.1 - center.1).atan2(from.0 - center.0);
    let mut end = (to.1 - center.1).atan2(to.0 - center.0);
    if clockwise && end < start {
        end += std::f32::consts::TAU;
    }
    if !clockwise && end > start {
        end -= std::f32::consts::TAU;
    }
    (1..=8)
        .map(|step| {
            let angle = start + (end - start) * step as f32 / 8.0;
            (
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arc_ends_where_it_was_asked_and_keeps_its_radius() {
        for clockwise in [false, true] {
            let points = arc((3.5, 9.0), (7.0, 12.5), 2.5, clockwise);
            let last = points.last().unwrap();
            assert!((last.0 - 7.0).abs() < 1e-3 && (last.1 - 12.5).abs() < 1e-3);
        }
        // A half turn from the right of a circle to its left, clockwise on screen, passes below.
        let below = arc((13.0, 8.0), (3.0, 8.0), 5.0, true);
        assert!(below.iter().any(|p| p.1 > 12.0), "{below:?}");
        let above = arc((13.0, 8.0), (3.0, 8.0), 5.0, false);
        assert!(above.iter().any(|p| p.1 < 4.0), "{above:?}");
    }

    #[test]
    fn every_icon_stays_inside_its_box() {
        let ctx = egui::Context::default();
        let center = egui::pos2(50.0, 50.0);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            for icon in [
                Icon::Files,
                Icon::Panel,
                Icon::SplitRight,
                Icon::SplitDown,
                Icon::Markdown,
                Icon::Search,
                Icon::Connect,
                Icon::Menu,
                Icon::Board,
                Icon::Minimize,
                Icon::Maximize,
                Icon::Restore,
                Icon::Close,
                Icon::Branch,
                Icon::Expand,
                Icon::Collapse,
                Icon::Refresh,
                Icon::File,
                Icon::Folder,
            ] {
                paint(ui.painter(), center, icon, 16.0, Color32::WHITE);
            }
        });
        output.textures_delta.clear();
        let bounds = egui::Rect::from_center_size(center, egui::vec2(18.0, 18.0));
        for shape in output.shapes {
            let rect = shape.shape.visual_bounding_rect();
            assert!(bounds.contains_rect(rect), "{rect:?} outside {bounds:?}");
        }
    }
}
