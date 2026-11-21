//! The marks a person draws over the drawer's image: arrows, ellipses, rectangles and numbered
//! markers, each with a note for a marker, and a general note. They are kept in the original
//! image's pixels, so a zoom or a pan moves them with the image and never relative to it.
use eframe::egui;

/// A drag shorter than this, in screen points, is a slip and leaves no shape. Measured on screen
/// because a 4 000-pixel screenshot fitted to 500 points makes an image pixel meaningless.
const SLIP: f32 = 4.0;

/// The one colour every mark is drawn in, on screen and in the copy alike.
pub(crate) const INK: [u8; 4] = [255, 45, 85, 255];

/// The smallest radius a marker's disc is drawn at on screen, however far the image is zoomed
/// out: below it the number inside stops being legible.
pub(crate) const DISC_FLOOR: f32 = 9.0;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum Tool {
    #[default]
    Arrow,
    Ellipse,
    Rectangle,
    Marker,
}

/// A mark, in pixels of the original image. Only a marker carries a note: it is the one with a
/// number for the note to be read under. Its `serial` names its note's field, so the field's
/// focus and undo history follow the marker and not its place in the list (#197): keyed on the
/// place, deleting marker 1 handed marker 2 the first one's history, and a Ctrl+Z inside it
/// brought the deleted note's text back.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Shape {
    Arrow {
        from: egui::Pos2,
        to: egui::Pos2,
    },
    Ellipse(egui::Rect),
    Rectangle(egui::Rect),
    Marker {
        serial: u64,
        at: egui::Pos2,
        note: String,
    },
}

/// The proportions of a mark for an image of a given size, in its pixels: the copy draws them as
/// they are, and the screen scales them with the image but keeps a floor under the stroke and the
/// marker's disc, so a mark on a screenshot shown small stays visible and reads thicker there than
/// in the copy.
pub(crate) struct Style {
    pub(crate) stroke: f32,
    /// How long an arrow's head is, and half how wide.
    pub(crate) head: (f32, f32),
    pub(crate) radius: f32,
}

impl Style {
    pub(crate) fn of(size: egui::Vec2) -> Self {
        let stroke = (size.x.min(size.y) / 300.0).max(3.0);
        Self {
            stroke,
            head: (stroke * 4.0, stroke * 2.2),
            radius: stroke * 4.5,
        }
    }
}

/// An arrow's head: its tip at `to`, and the two corners of its base. The shaft ends at the base's
/// middle, so its round cap never pokes past the tip.
pub(crate) fn head(from: egui::Pos2, to: egui::Pos2, style: &Style) -> [egui::Pos2; 3] {
    let along = (to - from).normalized();
    let across = along.rot90();
    let base = to - along * style.head.0;
    [
        to,
        base + across * style.head.1,
        base - across * style.head.1,
    ]
}

/// Takes a press of Ctrl+Z (Cmd+Z on macOS) with no other modifier, answering whether there was
/// one. egui's `consume_key` would also take Ctrl+Shift+Z, which elsewhere means redo, and there is
/// no redo here to give it.
pub(crate) fn take_undo(input: &mut egui::InputState) -> bool {
    let before = input.events.len();
    input.events.retain(|event| {
        !matches!(
            event,
            egui::Event::Key {
                key: egui::Key::Z,
                pressed: true,
                modifiers,
                ..
            } if modifiers.matches_exact(egui::Modifiers::COMMAND)
        )
    });
    input.events.len() != before
}

/// Where the whole image is on screen this frame, which maps its pixels to points and back.
#[derive(Clone, Copy)]
pub(crate) struct Placement {
    whole: egui::Rect,
    size: egui::Vec2,
}

impl Placement {
    /// `whole` is the image's rect as painted, snapped by `viewport::shown`, so a mark follows the
    /// pixels on screen rather than the scale the snap rounded away.
    pub(crate) fn new(whole: egui::Rect, size: egui::Vec2) -> Self {
        Self { whole, size }
    }

    fn per_pixel(&self) -> egui::Vec2 {
        self.whole.size() / self.size
    }

    fn to_screen(self, at: egui::Pos2) -> egui::Pos2 {
        self.whole.min + at.to_vec2() * self.per_pixel()
    }

    fn to_image(self, at: egui::Pos2) -> egui::Pos2 {
        ((at - self.whole.min) / self.per_pixel())
            .to_pos2()
            .clamp(egui::Pos2::ZERO, self.size.to_pos2())
    }
}

/// A shape being drawn: where the press was, in pixels and on screen, and where the pointer is now.
struct Stroke {
    from: egui::Pos2,
    pressed: egui::Pos2,
    to: egui::Pos2,
    at: egui::Pos2,
}

#[derive(Default)]
pub(crate) struct Marks {
    list: Vec<Shape>,
    pub(crate) general: String,
    pub(crate) tool: Tool,
    stroke: Option<Stroke>,
    /// The last serial a marker of this image was given. Never wound back by an undo or a Clear,
    /// because egui keeps a field's state by its id after the field is gone: a serial given twice
    /// would hand the new marker's field the old one's undo history. Another image starts again
    /// from nothing, since its fields' ids carry the opening they belong to.
    placed: u64,
}

impl Marks {
    /// Whether there is nothing to send and nothing that would be lost.
    pub(crate) fn is_empty(&self) -> bool {
        self.list.is_empty() && self.general.trim().is_empty()
    }

    pub(crate) fn marks(&self) -> &[Shape] {
        &self.list
    }

    pub(crate) fn undo(&mut self) {
        self.stroke = None;
        self.list.pop();
    }

    pub(crate) fn clear(&mut self) {
        self.stroke = None;
        self.list.clear();
        self.general.clear();
    }

    /// Removes the mark at `index` of the list; the markers after it take the numbers before.
    pub(crate) fn delete(&mut self, index: usize) {
        if index < self.list.len() {
            self.list.remove(index);
        }
    }

    /// Each marker's number, from 1 in the order they were placed, its place in the list and its
    /// serial.
    pub(crate) fn numbered(&self) -> impl Iterator<Item = (usize, usize, u64)> + '_ {
        self.list
            .iter()
            .enumerate()
            .filter_map(|(index, mark)| match mark {
                Shape::Marker { serial, .. } => Some((index, *serial)),
                _ => None,
            })
            .enumerate()
            .map(|(number, (index, serial))| (number + 1, index, serial))
    }

    /// The note of the marker at `index` of the list.
    pub(crate) fn note_mut(&mut self, index: usize) -> Option<&mut String> {
        match self.list.get_mut(index)? {
            Shape::Marker { note, .. } => Some(note),
            _ => None,
        }
    }

    /// Each marker's note, in its number's order.
    pub(crate) fn notes(&self) -> Vec<&str> {
        self.list
            .iter()
            .filter_map(|mark| match mark {
                Shape::Marker { note, .. } => Some(note.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Forgets a shape half drawn, as when the image was not painted for a frame and its release
    /// may have come then.
    pub(crate) fn cancel(&mut self) {
        self.stroke = None;
    }

    /// Draws with the selected tool. `pressed` is where the primary button went down on the image
    /// this frame and is not panning it; the drag goes on until that button is let go.
    pub(crate) fn gesture(
        &mut self,
        input: &egui::InputState,
        pressed: Option<egui::Pos2>,
        placement: Placement,
    ) {
        let pointer = &input.pointer;
        if self.stroke.is_none()
            && let Some(at) = pressed.filter(|at| placement.whole.contains(*at))
        {
            let from = placement.to_image(at);
            self.stroke = Some(Stroke {
                from,
                pressed: at,
                to: from,
                at,
            });
        }
        let Some(stroke) = self.stroke.as_mut() else {
            return;
        };
        if let Some(at) = pointer.latest_pos() {
            stroke.to = placement.to_image(at);
            stroke.at = at;
        }
        if !pointer.button_down(egui::PointerButton::Primary) {
            let stroke = self.stroke.take().expect("a stroke was being drawn");
            if let Some(shape) = finish(self.tool, &stroke) {
                self.keep(shape);
            }
        }
    }

    /// Adds a finished shape, giving a marker the next serial. Given here and not in `finish`,
    /// which also draws the shape being dragged, every frame.
    pub(super) fn keep(&mut self, mut shape: Shape) {
        if let Shape::Marker { serial, .. } = &mut shape {
            self.placed += 1;
            *serial = self.placed;
        }
        self.list.push(shape);
    }

    /// Paints every mark, and the one being drawn, over the image where it is this frame.
    pub(crate) fn paint(&self, painter: &egui::Painter, placement: Placement) {
        let style = Style::of(placement.size);
        let scale = placement.per_pixel().x;
        let width = (style.stroke * scale).max(1.5);
        let ink = ink();
        let line = egui::Stroke::new(width, ink);
        let drawing = self.stroke.as_ref().and_then(|s| finish(self.tool, s));
        let mut number = 0;
        for shape in self.list.iter().chain(drawing.as_ref()) {
            match *shape {
                Shape::Arrow { from, to } => {
                    let [tip, left, right] = head(from, to, &style).map(|p| placement.to_screen(p));
                    painter.line_segment([placement.to_screen(from), left.lerp(right, 0.5)], line);
                    painter.add(egui::Shape::convex_polygon(
                        vec![tip, left, right],
                        ink,
                        egui::Stroke::NONE,
                    ));
                }
                Shape::Ellipse(rect) => {
                    let rect = rect_on_screen(rect, placement);
                    painter.add(egui::Shape::ellipse_stroke(
                        rect.center(),
                        rect.size() / 2.0,
                        line,
                    ));
                }
                Shape::Rectangle(rect) => {
                    let rect = rect_on_screen(rect, placement);
                    painter.rect_stroke(rect, 0, line, egui::StrokeKind::Middle);
                }
                Shape::Marker { at, .. } => {
                    number += 1;
                    let center = placement.to_screen(at);
                    disc(
                        painter,
                        center,
                        (style.radius * scale).max(DISC_FLOOR),
                        number,
                    );
                }
            }
        }
    }
}

/// A marker's disc with its number, as the image shows it and as its note's row repeats it, so the
/// row reads as the same marker.
pub(crate) fn disc(painter: &egui::Painter, center: egui::Pos2, radius: f32, number: usize) {
    painter.circle_filled(center, radius, ink());
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        number.to_string(),
        egui::FontId::proportional(radius * 1.1),
        egui::Color32::WHITE,
    );
}

fn ink() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(INK[0], INK[1], INK[2], INK[3])
}

fn rect_on_screen(rect: egui::Rect, placement: Placement) -> egui::Rect {
    egui::Rect::from_two_pos(placement.to_screen(rect.min), placement.to_screen(rect.max))
}

/// The shape a stroke leaves with `tool`, or none for a slip. A marker goes where the press was,
/// with no serial yet.
fn finish(tool: Tool, stroke: &Stroke) -> Option<Shape> {
    let rect = egui::Rect::from_two_pos(stroke.from, stroke.to);
    let boxed = rect.width() == 0.0 || rect.height() == 0.0;
    // A drag past the image's edge is clamped to it, and can end with no extent left: nothing
    // would be seen, on screen or in the copy, yet the mark would keep the image from closing.
    let (shape, flat) = match tool {
        Tool::Marker => {
            let at = stroke.from;
            return Some(Shape::Marker {
                serial: 0,
                at,
                note: String::new(),
            });
        }
        Tool::Arrow => {
            let (from, to) = (stroke.from, stroke.to);
            (Shape::Arrow { from, to }, from == to)
        }
        Tool::Ellipse => (Shape::Ellipse(rect), boxed),
        Tool::Rectangle => (Shape::Rectangle(rect), boxed),
    };
    let slip = stroke.pressed.distance(stroke.at) < SLIP;
    (!slip && !flat).then_some(shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(at: (f32, f32), note: &str) -> Shape {
        Shape::Marker {
            serial: at.0 as u64,
            at: egui::pos2(at.0, at.1),
            note: note.into(),
        }
    }

    /// Markers are numbered by their order among markers, other shapes between them take no number,
    /// and deleting one gives the next its number and keeps its note.
    #[test]
    fn deleting_a_marker_renumbers_the_rest_with_their_notes() {
        let mut marks = Marks {
            list: vec![
                marker((1.0, 1.0), "first"),
                Shape::Rectangle(egui::Rect::from_min_max(
                    egui::pos2(0.0, 0.0),
                    egui::pos2(5.0, 5.0),
                )),
                marker((2.0, 2.0), "second"),
                marker((3.0, 3.0), "third"),
            ],
            ..Marks::default()
        };
        assert_eq!(
            marks.numbered().collect::<Vec<_>>(),
            [(1, 0, 1), (2, 2, 2), (3, 3, 3)]
        );
        marks.delete(2);
        assert_eq!(marks.numbered().collect::<Vec<_>>(), [(1, 0, 1), (2, 2, 3)]);
        assert_eq!(marks.notes(), ["first", "third"]);
        marks.undo();
        assert_eq!(marks.notes(), ["first"]);
        assert_eq!(marks.marks().len(), 2);
    }

    /// A marker's serial is never given again on its image: not after an undo takes the last
    /// marker back, nor after a Clear.
    #[test]
    fn a_markers_serial_never_comes_back() {
        let placed = |marks: &mut Marks| {
            marks.keep(marker((0.0, 0.0), ""));
            marks.numbered().last().expect("a marker was placed").2
        };
        let mut marks = Marks::default();
        let mut seen = vec![placed(&mut marks), placed(&mut marks)];
        marks.undo();
        seen.push(placed(&mut marks));
        marks.clear();
        seen.push(placed(&mut marks));
        assert_eq!(seen, [1, 2, 3, 4]);
    }

    /// Each tool leaves its own shape from the pixels pressed and let go; a marker goes where the
    /// press was.
    #[test]
    fn each_tool_leaves_its_own_shape() {
        let stroke = Stroke {
            from: egui::pos2(10.0, 20.0),
            pressed: egui::pos2(100.0, 100.0),
            to: egui::pos2(50.0, 60.0),
            at: egui::pos2(140.0, 140.0),
        };
        let rect = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(50.0, 60.0));
        assert_eq!(
            finish(Tool::Arrow, &stroke),
            Some(Shape::Arrow {
                from: stroke.from,
                to: stroke.to
            })
        );
        assert_eq!(finish(Tool::Ellipse, &stroke), Some(Shape::Ellipse(rect)));
        assert_eq!(
            finish(Tool::Rectangle, &stroke),
            Some(Shape::Rectangle(rect))
        );
        assert_eq!(
            finish(Tool::Marker, &stroke),
            Some(Shape::Marker {
                serial: 0,
                at: stroke.from,
                note: String::new()
            })
        );
    }

    /// A drag long on screen but clamped to no extent on the image leaves nothing: an arrow
    /// whose ends meet, a box with no width or no height.
    #[test]
    fn a_drag_clamped_to_no_extent_leaves_nothing() {
        let stroke = |from: (f32, f32), to: (f32, f32)| Stroke {
            from: egui::pos2(from.0, from.1),
            pressed: egui::pos2(0.0, 0.0),
            to: egui::pos2(to.0, to.1),
            at: egui::pos2(50.0, 50.0),
        };
        let corner = stroke((400.0, 0.0), (400.0, 0.0));
        let edge = stroke((400.0, 10.0), (400.0, 90.0));
        for tool in [Tool::Arrow, Tool::Ellipse, Tool::Rectangle] {
            assert_eq!(finish(tool, &corner), None, "{tool:?} at a corner");
        }
        for tool in [Tool::Ellipse, Tool::Rectangle] {
            assert_eq!(finish(tool, &edge), None, "{tool:?} along an edge");
        }
        assert!(
            finish(Tool::Arrow, &edge).is_some(),
            "an arrow along an edge is seen"
        );
        assert!(finish(Tool::Marker, &corner).is_some());
    }

    /// The image's pixel under a point on screen is the same however the image is scaled or moved,
    /// and a point beside the image lands on its nearest edge.
    #[test]
    fn a_placement_maps_pixels_to_points_and_back() {
        let size = egui::vec2(400.0, 200.0);
        let placement = Placement::new(
            egui::Rect::from_min_size(egui::pos2(-30.0, 10.0), egui::vec2(100.0, 50.0)),
            size,
        );
        let pixel = egui::pos2(200.0, 100.0);
        let on_screen = placement.to_screen(pixel);
        assert_eq!(on_screen, egui::pos2(20.0, 35.0));
        assert_eq!(placement.to_image(on_screen), pixel);
        assert_eq!(
            placement.to_image(egui::pos2(500.0, -40.0)),
            egui::pos2(400.0, 0.0)
        );
    }
}
