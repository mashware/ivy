//! The image's own viewport (D72): a scale and an offset, where the offset is where the view's
//! top-left corner falls on the scaled image, in points, negative where the image starts inside
//! the view. No egui scroll area holds it, so no state of egui's can disagree with it.
use eframe::egui;

/// How far the zoom goes, in screen points per pixel of the original.
pub(super) const SMALLEST: f32 = 0.01;
pub(super) const LARGEST: f32 = 32.0;

/// The scale that fits an image `width` pixels wide into `room` points, never enlarging it past
/// one physical pixel per pixel: a screenshot shown larger than it was taken only blurs.
pub(super) fn fit(room: f32, width: f32, pixels_per_point: f32) -> f32 {
    (room / width.max(1.0))
        .min(1.0 / pixels_per_point)
        .clamp(SMALLEST, LARGEST)
}

/// The scale and offset after zooming by `factor`, keeping the point of the image of `size`
/// pixels under `at`, a position inside the view, where it was on screen. With `at` off the image,
/// the image's nearest point is the one kept, so a zoom over the blank around it never carries the
/// image out of sight. A factor or scale that is not a positive number changes nothing.
pub(super) fn zoom_around(
    scale: f32,
    offset: egui::Vec2,
    at: egui::Vec2,
    factor: f32,
    size: egui::Vec2,
) -> (f32, egui::Vec2) {
    let usable = |x: f32| x.is_finite() && x > 0.0;
    if !usable(scale) || !usable(factor) {
        return (scale, offset);
    }
    let zoomed = (scale * factor).clamp(SMALLEST, LARGEST);
    let at = at.max(-offset).min(size * scale - offset);
    let point = (offset + at) / scale;
    (zoomed, point * zoomed - at)
}

/// Where `offset` may be for an image of `shown` size in a `view`. On an axis where the image is
/// larger than the view, no gap opens between its edge and the view's. Where it is smaller, it
/// belongs whole inside the view, but a zoom around the pointer may have carried it across an
/// edge: with `before`, where it was, it may stay as far out as that and only come back, so a zoom
/// (`before` = `offset`) never moves it and a pan never pushes it further out. Without `before`, as
/// when the view changed size, it is put whole inside.
pub(super) fn clamp(
    offset: egui::Vec2,
    before: Option<egui::Vec2>,
    shown: egui::Vec2,
    view: egui::Vec2,
) -> egui::Vec2 {
    let axis = |offset: f32, before: Option<f32>, shown: f32, view: f32| {
        let spare = shown - view;
        let (mut low, mut high) = (spare.min(0.0), spare.max(0.0));
        if let Some(before) = before.filter(|b| spare <= 0.0 && b.is_finite()) {
            (low, high) = (low.min(before), high.max(before));
        }
        offset.max(low).min(high)
    };
    egui::vec2(
        axis(offset.x, before.map(|b| b.x), shown.x, view.x),
        axis(offset.y, before.map(|b| b.y), shown.y, view.y),
    )
}

/// Where a wheel's `delta` moves an image of `shown` size at `offset` in a `view`, and the part of
/// the delta that moved it. It applies on an axis where the image is larger than the view, and on
/// one where a zoom carried a smaller image across an edge, which the wheel may bring back but,
/// through `clamp`, never push further out. What an edge stopped is not part of what moved it.
pub(super) fn wheel(
    delta: egui::Vec2,
    offset: egui::Vec2,
    shown: egui::Vec2,
    view: egui::Vec2,
) -> (egui::Vec2, egui::Vec2) {
    let applies = |delta: f32, offset: f32, shown: f32, view: f32| {
        let spare = shown - view;
        if spare > 0.0 || offset < spare || offset > 0.0 {
            delta
        } else {
            0.0
        }
    };
    let applied = egui::vec2(
        applies(delta.x, offset.x, shown.x, view.x),
        applies(delta.y, offset.y, shown.y, view.y),
    );
    let to = clamp(offset - applied, Some(offset), shown, view);
    // Where the clamp let the axis through whole, the delta itself, so no rounding is left behind.
    let taken = |applied: f32, from: f32, to: f32| {
        if from - applied == to {
            applied
        } else {
            from - to
        }
    };
    let taken = egui::vec2(
        taken(applied.x, offset.x, to.x),
        taken(applied.y, offset.y, to.y),
    );
    (to, taken)
}

/// Where `wanted` last went down this frame. Read from the frame's own events rather than from the
/// pointer: it may leave the image in the same frame, and egui forgets `press_origin` on any
/// button's release.
pub(super) fn pressed_at(
    input: &egui::InputState,
    wanted: egui::PointerButton,
) -> Option<egui::Pos2> {
    input.events.iter().rev().find_map(|event| match *event {
        egui::Event::PointerButton {
            pos,
            button,
            pressed: true,
            ..
        } if button == wanted => Some(pos),
        _ => None,
    })
}

/// How far a pan drag moved the pointer this frame, and the hand the cursor shows. A plain left
/// drag never pans: marks are drawn with it. Space + left drag is Figma's convention, and the
/// middle button pans with no key for a mouse that has one. The button that started a pan is
/// latched in `panning` until it is released, so letting Space go halfway through, or a stray
/// press of another button, does not turn the rest into something else. `over` is whether the
/// pointer is on the image with nothing of a higher layer above it, and `pressed` whether the
/// middle and the primary button went down on it this frame, as `pressed_at` places a press.
pub(super) fn pan(
    input: &egui::InputState,
    over: bool,
    pressed: [bool; 2],
    space: bool,
    panning: &mut Option<egui::PointerButton>,
) -> (egui::Vec2, Option<egui::CursorIcon>) {
    use egui::PointerButton::{Middle, Primary};
    let pointer = &input.pointer;
    if let Some(button) = *panning {
        if pointer.button_down(button) {
            let moved = if pointer.button_pressed(button) {
                egui::Vec2::ZERO
            } else {
                pointer.delta()
            };
            return (moved, Some(egui::CursorIcon::Grabbing));
        }
        *panning = None;
    }
    let [middle, primary] = pressed;
    *panning = if middle {
        Some(Middle)
    } else if space && primary {
        Some(Primary)
    } else {
        None
    };
    if panning.is_some() {
        // The press frame's delta is the travel from wherever the pointer was before it.
        return (egui::Vec2::ZERO, Some(egui::CursorIcon::Grabbing));
    }
    let hand = (space && over).then_some(egui::CursorIcon::Grab);
    (egui::Vec2::ZERO, hand)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: egui::Vec2 = egui::vec2(800.0, 600.0);

    /// The point of the image under `at`, in pixels of the original.
    fn under(scale: f32, offset: egui::Vec2, at: egui::Vec2) -> egui::Vec2 {
        (offset + at) / scale
    }

    #[test]
    fn the_zoom_stops_at_its_bounds() {
        let (offset, at) = (egui::vec2(100.0, 40.0), egui::vec2(200.0, 150.0));
        assert_eq!(zoom_around(0.5, offset, at, 1000.0, SIZE).0, LARGEST);
        assert_eq!(zoom_around(0.5, offset, at, 0.0001, SIZE).0, SMALLEST);
    }

    /// A large image already panned into its middle keeps the pixel under the pointer there, both
    /// zooming in and out.
    #[test]
    fn a_panned_large_image_keeps_the_point_under_the_pointer() {
        let (scale, offset, at) = (2.0, egui::vec2(530.0, 310.0), egui::vec2(170.0, 90.0));
        for factor in [1.5, 0.8] {
            let (zoomed, moved) = zoom_around(scale, offset, at, factor, SIZE);
            assert_eq!(zoomed, scale * factor);
            assert!(
                (under(zoomed, moved, at) - under(scale, offset, at)).length() < 1e-3,
                "{factor}: {moved:?}"
            );
        }
    }

    /// Beside the image the anchor is its nearest point: below a picture that starts inside the
    /// view, its bottom edge stays where it was.
    #[test]
    fn beside_the_image_the_nearest_point_is_kept() {
        let (scale, offset) = (0.5, egui::vec2(-50.0, -20.0));
        let at = egui::vec2(250.0, 500.0);
        let (zoomed, moved) = zoom_around(scale, offset, at, 2.0, SIZE);
        let bottom = |scale: f32, offset: egui::Vec2| SIZE.y * scale - offset.y;
        assert_eq!(bottom(zoomed, moved), bottom(scale, offset));
    }

    /// Without a previous position, as after the view changed size, a smaller image is put whole
    /// inside the view, and left anywhere there.
    #[test]
    fn a_smaller_image_is_put_whole_inside_the_view_anywhere() {
        let (shown, view) = (egui::vec2(300.0, 100.0), egui::vec2(500.0, 400.0));
        let inside = egui::vec2(-120.0, -250.0);
        assert_eq!(clamp(inside, None, shown, view), inside);
        assert_eq!(
            clamp(egui::vec2(30.0, 10.0), None, shown, view),
            egui::Vec2::ZERO
        );
        assert_eq!(
            clamp(egui::vec2(-400.0, -900.0), None, shown, view),
            egui::vec2(-200.0, -300.0)
        );
    }

    /// A smaller image a zoom carried across the view's top-left edge stays there through the
    /// zoom, comes back with a pan toward the view, and is never pushed further out.
    #[test]
    fn a_smaller_image_across_an_edge_only_comes_back() {
        let (shown, view) = (egui::vec2(300.0, 100.0), egui::vec2(500.0, 400.0));
        let across = egui::vec2(40.0, 30.0);
        assert_eq!(clamp(across, Some(across), shown, view), across);
        assert_eq!(
            clamp(across + egui::vec2(25.0, 5.0), Some(across), shown, view),
            across
        );
        let back = egui::vec2(10.0, 12.0);
        assert_eq!(clamp(back, Some(across), shown, view), back);
    }

    /// A larger image never opens a gap at its edges, whether a zoom, a pan or a resize moved it.
    #[test]
    fn a_larger_image_never_opens_a_gap_at_its_edges() {
        let (shown, view) = (egui::vec2(1200.0, 900.0), egui::vec2(500.0, 400.0));
        for before in [None, Some(egui::vec2(-30.0, -1.0))] {
            assert_eq!(
                clamp(egui::vec2(-30.0, -1.0), before, shown, view),
                egui::Vec2::ZERO
            );
            assert_eq!(
                clamp(egui::vec2(900.0, 600.0), before, shown, view),
                egui::vec2(700.0, 500.0)
            );
            let middle = egui::vec2(350.0, 250.0);
            assert_eq!(clamp(middle, before, shown, view), middle);
        }
    }

    /// No input of a degenerate view or scale divides by zero, panics, or leaves a NaN behind.
    #[test]
    fn degenerate_views_and_scales_leave_finite_state() {
        for room in [0.0, -5.0, f32::NAN, f32::INFINITY] {
            let scale = fit(room, 2400.0, 1.0);
            assert!(scale.is_finite() && scale > 0.0, "{room}: {scale}");
        }
        assert!(fit(500.0, 0.0, 1.0).is_finite());
        let offset = egui::vec2(10.0, 20.0);
        let at = egui::vec2(5.0, 5.0);
        for (scale, factor) in [(0.0, 2.0), (f32::NAN, 2.0), (1.0, f32::NAN), (1.0, 0.0)] {
            assert_eq!(
                zoom_around(scale, offset, at, factor, SIZE).1,
                offset,
                "{scale} {factor}"
            );
        }
        let nan = egui::vec2(f32::NAN, f32::NAN);
        assert_eq!(clamp(nan, None, SIZE, egui::Vec2::ZERO), egui::Vec2::ZERO);
        assert_eq!(clamp(offset, None, nan, SIZE), egui::Vec2::ZERO);
        assert_eq!(clamp(offset, Some(nan), SIZE, egui::Vec2::ZERO), offset);
        assert_eq!(clamp(offset, None, SIZE, egui::Vec2::ZERO), offset);
    }

    /// The wheel takes what moved the image: all of a turn the edge does not reach, only the part
    /// before it of one that does, and nothing of one the edge stops entirely.
    #[test]
    fn the_wheel_takes_only_what_moved_the_image() {
        let (shown, view) = (egui::vec2(1200.0, 900.0), egui::vec2(500.0, 400.0));
        let offset = egui::vec2(690.0, 500.0);
        let (to, taken) = wheel(egui::vec2(20.0, -30.0), offset, shown, view);
        assert_eq!(to, egui::vec2(670.0, 500.0));
        assert_eq!(taken, egui::vec2(20.0, 0.0));
        let (to, taken) = wheel(egui::vec2(-30.0, 7.0), offset, shown, view);
        assert_eq!(to, egui::vec2(700.0, 493.0));
        assert_eq!(taken, egui::vec2(-10.0, 7.0));
    }

    #[test]
    fn an_image_is_fitted_and_never_enlarged() {
        assert_eq!(fit(500.0, 2000.0, 1.0), 0.25);
        assert_eq!(fit(500.0, 300.0, 1.0), 1.0);
        assert_eq!(fit(500.0, 300.0, 2.0), 0.5);
    }
}
