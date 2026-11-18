use super::*;
use crate::painter::{CellMetrics, CursorInk, paint};
use crate::theme::Theme;
use eframe::egui::{Context, FontId, Id, LayerId, Order, Pos2, Shape as Drawn, epaint::RectShape};
use term_core::{Screen, Size};

/// The scales the defect was measured at (D35's amendment).
const SCALES: [f32; 5] = [1.0, 1.25, 1.5, 5.0 / 3.0, 2.0];
/// A fractional font size among them, so the cell comes out odd on some axis.
const BODIES: [f32; 2] = [14.0, 14.7];

/// Paints `bytes` with the production painter and returns what it drew: rectangles, and how many
/// text shapes.
fn drawn(bytes: &[u8], per_point: f32, body: f32) -> (Vec<RectShape>, usize) {
    let ctx = Context::default();
    ctx.set_pixels_per_point(per_point);
    let mut screen = Screen::new(Size { rows: 8, cols: 12 });
    screen.advance(b"\x1b[?25l");
    screen.advance(bytes);
    let view = screen.view();
    let output = ctx.run_ui(Default::default(), |ui| {
        let metrics = CellMetrics::measure(ui.ctx(), FontId::monospace(body));
        let painter = ui
            .ctx()
            .layer_painter(LayerId::new(Order::Foreground, Id::new("box drawing")));
        paint(
            &painter,
            &view,
            &Theme::default(),
            &metrics,
            Pos2::new(3.3, 7.7),
            &mut crate::fonts::Discoverer::default(),
            CursorInk::Filled,
        );
    });
    let (mut rects, mut texts) = (Vec::new(), 0);
    fn walk(shape: &Drawn, rects: &mut Vec<RectShape>, texts: &mut usize) {
        match shape {
            Drawn::Rect(rect) => rects.push(rect.clone()),
            Drawn::Text(_) => *texts += 1,
            Drawn::Vec(many) => many.iter().for_each(|one| walk(one, rects, texts)),
            _ => {}
        }
    }
    for shape in &output.shapes {
        walk(&shape.shape, &mut rects, &mut texts);
    }
    output.drop_without_applying_deltas();
    (rects, texts)
}

/// How many times each device pixel is covered, rounding every edge the way epaint does.
fn coverage(rects: &[RectShape], per_point: f32) -> std::collections::BTreeMap<(i32, i32), u32> {
    let mut covered = std::collections::BTreeMap::new();
    for rect in rects {
        let edge = |v: f32| (v * per_point).round() as i32;
        let r = rect.rect;
        for y in edge(r.top())..edge(r.bottom()) {
            for x in edge(r.left())..edge(r.right()) {
                *covered.entry((x, y)).or_insert(0) += 1;
            }
        }
    }
    covered
}

#[test]
fn a_run_of_lines_is_a_band_of_whole_rows_each_pixel_painted_once() {
    // **The two defects of D35's amendment, asserted where a person sees them**: on the pixels. Painted
    // from the font, a run of `─` overlaps at every seam — a pixel covered twice — and its stroke
    // straddles rows, so a row is covered under one cell and not under the next, or not to the
    // end. A band is the same set of rows at every column from the first cell to the last.
    for per_point in SCALES {
        for body in BODIES {
            for (bytes, horizontal) in [
                ("────────".as_bytes(), true),
                ("│\r\n│\r\n│\r\n│\r\n│\r\n│".as_bytes(), false),
            ] {
                let (rects, texts) = drawn(bytes, per_point, body);
                assert_eq!(texts, 0, "a line was still asked of the font");
                let covered = coverage(&rects, per_point);
                assert!(!covered.is_empty(), "nothing was drawn at {per_point}");
                assert!(
                    covered.values().all(|&n| n == 1),
                    "at {per_point} × {body} a pixel was painted twice: a bead at a seam"
                );
                // Along the run: every position from the first to the last holds the same band.
                let (along, across): (Vec<i32>, Vec<i32>) = covered
                    .keys()
                    .map(|&(x, y)| if horizontal { (x, y) } else { (y, x) })
                    .unzip();
                let (first, last) = (*along.iter().min().unwrap(), *along.iter().max().unwrap());
                let band: std::collections::BTreeSet<i32> = across.into_iter().collect();
                for at in first..=last {
                    for &row in &band {
                        let key = if horizontal { (at, row) } else { (row, at) };
                        assert!(
                            covered.contains_key(&key),
                            "at {per_point} × {body} the band breaks at {at}: a gap or a row \
                             that only some cells cover"
                        );
                    }
                }
            }
        }
    }
}

/// What the font inked for whatever `bytes` still asks of it, as device pixels: every glyph quad the
/// engine tessellates, sampled from its atlas at the pixel's centre, each quad counting once where
/// its texel is not empty. Without this, a box asked of the font would show up as no pixels at all
/// and the test below would fail on «nothing was drawn» instead of on the gaps and the beads.
fn ink_from_the_font(
    bytes: &[u8],
    per_point: f32,
    body: f32,
) -> std::collections::BTreeMap<(i32, i32), u32> {
    use eframe::egui::epaint::{ClippedShape, Primitive};
    let ctx = Context::default();
    ctx.set_pixels_per_point(per_point);
    let mut screen = Screen::new(Size { rows: 8, cols: 12 });
    screen.advance(b"\x1b[?25l");
    screen.advance(bytes);
    let view = screen.view();
    let mut texts = Vec::new();
    // Twice, because the atlas only holds a glyph once a frame has asked for it.
    for _ in 0..2 {
        let output = ctx.run_ui(Default::default(), |ui| {
            let metrics = CellMetrics::measure(ui.ctx(), FontId::monospace(body));
            let painter = ui
                .ctx()
                .layer_painter(LayerId::new(Order::Foreground, Id::new("box drawing")));
            paint(
                &painter,
                &view,
                &Theme::default(),
                &metrics,
                Pos2::new(3.3, 7.7),
                &mut crate::fonts::Discoverer::default(),
                CursorInk::Filled,
            );
        });
        fn walk(shape: &Drawn, clip: eframe::egui::Rect, texts: &mut Vec<ClippedShape>) {
            match shape {
                Drawn::Text(_) => texts.push(ClippedShape {
                    clip_rect: clip,
                    shape: shape.clone(),
                }),
                Drawn::Vec(many) => many.iter().for_each(|one| walk(one, clip, texts)),
                _ => {}
            }
        }
        texts.clear();
        for shape in &output.shapes {
            walk(&shape.shape, shape.clip_rect, &mut texts);
        }
        output.drop_without_applying_deltas();
    }
    let mut covered = std::collections::BTreeMap::new();
    if texts.is_empty() {
        return covered;
    }
    let atlas = ctx.fonts(|fonts| fonts.image());
    let [width, height] = atlas.size;
    for primitive in ctx.tessellate(texts, per_point) {
        let Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        // A glyph is one quad: four vertices, two triangles.
        for quad in mesh.indices.chunks(6) {
            let corners: Vec<_> = quad.iter().map(|&i| mesh.vertices[i as usize]).collect();
            let span = |of: &dyn Fn(&eframe::egui::epaint::Vertex) -> f32| {
                corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                    (lo.min(of(v)), hi.max(of(v)))
                })
            };
            let (x0, x1) = span(&|v| v.pos.x * per_point);
            let (y0, y1) = span(&|v| v.pos.y * per_point);
            let (u0, u1) = span(&|v| v.uv.x);
            let (t0, t1) = span(&|v| v.uv.y);
            for y in y0.floor() as i32..y1.ceil() as i32 {
                for x in x0.floor() as i32..x1.ceil() as i32 {
                    let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
                    if cx < x0 || cx > x1 || cy < y0 || cy > y1 {
                        continue;
                    }
                    let u = u0 + (cx - x0) / (x1 - x0) * (u1 - u0);
                    let t = t0 + (cy - y0) / (y1 - y0) * (t1 - t0);
                    let tx = ((u * width as f32) as usize).min(width - 1);
                    let ty = ((t * height as f32) as usize).min(height - 1);
                    if atlas.pixels[ty * width + tx].a() > 0 {
                        *covered.entry((x, y)).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    covered
}

/// How many separate pieces `pixels` falls into: four-connected when `diagonal` is false, eight
/// when it is true.
fn pieces(pixels: &std::collections::BTreeSet<(i32, i32)>, diagonal: bool) -> usize {
    let mut seen = std::collections::BTreeSet::new();
    let mut count = 0;
    for &start in pixels {
        if !seen.insert(start) {
            continue;
        }
        count += 1;
        let mut pending = vec![start];
        while let Some((x, y)) = pending.pop() {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if (dx, dy) == (0, 0) || (!diagonal && dx != 0 && dy != 0) {
                        continue;
                    }
                    let next = (x + dx, y + dy);
                    if pixels.contains(&next) && seen.insert(next) {
                        pending.push(next);
                    }
                }
            }
        }
    }
    count
}

#[test]
fn a_box_drawn_by_the_painter_closes_at_every_corner_and_seam() {
    // **What a person reads as a box is a closed frame**: the corner has to meet the line beside it
    // and the line under it, and every seam between two cells has to be neither a gap nor a pixel
    // inked twice. Asserted on the device pixels of the whole frame, with topology instead of a
    // shape: the strokes are four-connected rings — one, or two for a double line — and the empty
    // pixels around them split into the outside plus one hole per ring, eight-connected, so a
    // corner that only touches its neighbour diagonally lets the hole leak out and fails. A rounded
    // corner is held to the same pairing: its staircase shares an edge from step to step.
    //
    // **The grids are where the joints inside a cell are asserted**, which the edges of a single
    // cell cannot see: a tee or a cross that leaves a gap, or a double whose lines run through
    // where they should stop, changes the number of holes. The channel between the two lines of
    // a double is a hole of its own, closed at each end by the line that crosses it, so a single
    // line that breaks where it crosses a double (`╪` `╫`) merges two channels into one.
    for (frame, rings, holes) in [
        ("┌──┐\r\n│  │\r\n└──┘", 1, 1),
        ("┏━━┓\r\n┃  ┃\r\n┗━━┛", 1, 1),
        ("╔══╗\r\n║  ║\r\n╚══╝", 2, 2),
        ("╭──╮\r\n│  │\r\n╰──╯", 1, 1),
        ("┌─┬─┐\r\n│ │ │\r\n├─┼─┤\r\n│ │ │\r\n└─┴─┘", 1, 4),
        ("┏━┳━┓\r\n┃ ┃ ┃\r\n┣━╋━┫\r\n┃ ┃ ┃\r\n┗━┻━┛", 1, 4),
        ("╔═╦═╗\r\n║ ║ ║\r\n╠═╬═╣\r\n║ ║ ║\r\n╚═╩═╝", 5, 5),
        ("╒═╤═╕\r\n│ │ │\r\n╞═╪═╡\r\n│ │ │\r\n╘═╧═╛", 1, 8),
        ("╓─╥─╖\r\n║ ║ ║\r\n╟─╫─╢\r\n║ ║ ║\r\n╙─╨─╜", 1, 8),
    ] {
        for per_point in SCALES {
            for body in BODIES {
                let (rects, _) = drawn(frame.as_bytes(), per_point, body);
                let mut covered = coverage(&rects, per_point);
                for (pixel, n) in ink_from_the_font(frame.as_bytes(), per_point, body) {
                    *covered.entry(pixel).or_insert(0) += n;
                }
                let at = format!("{frame:?} at {per_point} × {body}");
                assert!(!covered.is_empty(), "{at}: nothing was drawn");
                let twice: Vec<_> = covered.iter().filter(|&(_, &n)| n > 1).collect();
                assert!(
                    twice.is_empty(),
                    "{at}: pixels inked twice, a bead at a seam: {twice:?}"
                );
                let ink: std::collections::BTreeSet<(i32, i32)> = covered.keys().copied().collect();
                assert_eq!(
                    pieces(&ink, false),
                    rings,
                    "{at}: the strokes break into pieces, a gap at a corner or a seam"
                );
                // The empty pixels in a margin of one around the frame: the outside, and a hole
                // inside each ring only if that ring is closed.
                let (xs, ys): (Vec<i32>, Vec<i32>) = ink.iter().copied().unzip();
                let (left, right) = (xs.iter().min().unwrap() - 1, xs.iter().max().unwrap() + 1);
                let (top, bottom) = (ys.iter().min().unwrap() - 1, ys.iter().max().unwrap() + 1);
                let empty: std::collections::BTreeSet<(i32, i32)> = (top..=bottom)
                    .flat_map(|y| (left..=right).map(move |x| (x, y)))
                    .filter(|pixel| !ink.contains(pixel))
                    .collect();
                assert_eq!(
                    pieces(&empty, true),
                    1 + holes,
                    "{at}: the inside leaks to the outside, the frame does not close"
                );
            }
        }
    }
}

/// The rows (or columns) a straight line of `weight` occupies across a span.
fn band(weight: Weight, span: i32, light: i32) -> Vec<i32> {
    match weight {
        Weight::None => vec![],
        Weight::Light => (centred(span, light)..centred(span, light) + light).collect(),
        Weight::Heavy => (centred(span, light * 2)..centred(span, light * 2) + light * 2).collect(),
        Weight::Double => {
            let one = centred(span, light * 3);
            (one..one + light)
                .chain(one + light * 2..one + light * 3)
                .collect()
        }
    }
}

#[test]
fn every_arm_reaches_its_edge_on_the_rows_the_neighbouring_line_uses() {
    // **What makes a box close**: `┌` meets the `─` beside it and the `│` under it only if its
    // arm leaves the cell on exactly the rows a plain line enters on. Asserted for every
    // character, at cell sizes that are odd and even on each axis, against the band a straight
    // line of that weight draws — and an arm that is not there must leave its edge empty.
    for c in (0x2500..=0x257F).filter_map(char::from_u32) {
        let Some(shape) = shape_of(c) else { continue };
        if shape.dashes > 0 {
            continue;
        }
        // Each light thickness at the cells it comes with: two pixels is what a cell eleven or
        // more wide gets, and in a narrower one a double vertical touches the cell's own edge.
        for (w, h, light) in [
            (7, 17, 1),
            (8, 17, 1),
            (11, 29, 1),
            (11, 29, 2),
            (13, 28, 2),
            (15, 33, 2),
            (16, 34, 2),
        ] {
            {
                let mut mask = vec![false; (w * h) as usize];
                for (x0, y0, x1, y1) in runs(shape, w, h, light) {
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let cell = &mut mask[(y * w + x) as usize];
                            assert!(!*cell, "{c} paints a pixel twice at {w}×{h}");
                            *cell = true;
                        }
                    }
                }
                let [left, right, up, down] = shape.arms;
                let column = |x: i32| {
                    (0..h)
                        .filter(|&y| mask[(y * w + x) as usize])
                        .collect::<Vec<_>>()
                };
                let row = |y: i32| {
                    (0..w)
                        .filter(|&x| mask[(y * w + x) as usize])
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    column(0),
                    band(left, h, light),
                    "{c} left edge at {w}×{h}, light {light}"
                );
                assert_eq!(
                    column(w - 1),
                    band(right, h, light),
                    "{c} right edge at {w}×{h}"
                );
                assert_eq!(
                    row(0),
                    band(up, w, light),
                    "{c} top edge at {w}×{h}, light {light}"
                );
                assert_eq!(
                    row(h - 1),
                    band(down, w, light),
                    "{c} bottom edge at {w}×{h}"
                );
            }
        }
    }
}

#[test]
fn every_dashed_line_is_cut_into_its_dashes_or_as_many_as_fit() {
    // A cell too short for every dash and its gap gets as many as fit, a pixel each: dropping one
    // gap merged two dashes into a long one, and a solid fallback read as the plain line. The
    // short sizes are on both axes, so the fallback cannot be measured along the wrong one.
    for (c, dashes, horizontal) in [
        ('┄', 3, true),
        ('┅', 3, true),
        ('┆', 3, false),
        ('┇', 3, false),
        ('┈', 4, true),
        ('┉', 4, true),
        ('┊', 4, false),
        ('┋', 4, false),
        ('╌', 2, true),
        ('╍', 2, true),
        ('╎', 2, false),
        ('╏', 2, false),
    ] {
        let shape = shape_of(c).unwrap();
        for (w, h) in [(5, 5), (7, 7), (7, 17), (8, 17), (16, 34)] {
            let span = if horizontal { w } else { h };
            let expected = dashes.min(span / 2).max(1) as usize;
            let rects = runs(shape, w, h, 1);
            assert_eq!(
                rects.len(),
                expected,
                "{c} at {w}×{h} came out as {rects:?}"
            );
            assert!(
                rects.iter().all(|r| r.2 > r.0 && r.3 > r.1),
                "{c} at {w}×{h} has an empty dash: {rects:?}"
            );
        }
    }
}

#[test]
fn an_arm_that_is_not_there_leaves_nothing_past_the_joint_on_its_side() {
    // **The other half of a joint**: an arm reaching its edge proves the box closes, and an arm
    // that is absent must not poke out of the joint either — `┍` with its heavy line starting
    // left of the light one is a tick nobody asked for. The joint's extent on a side is where the
    // nearest band of the other axis starts, or the arm's own band where nothing crosses it.
    for c in (0x2500..=0x257F).filter_map(char::from_u32) {
        let Some(shape) = shape_of(c) else { continue };
        if shape.dashes > 0 || shape.arc {
            continue;
        }
        let [left, right, up, down] = shape.arms;
        for (w, h, light) in [(7, 17, 1), (8, 17, 1), (11, 29, 2), (16, 34, 2)] {
            let mut ink = Vec::new();
            for (x0, y0, x1, y1) in runs(shape, w, h, light) {
                for y in y0..y1 {
                    for x in x0..x1 {
                        ink.push((x, y));
                    }
                }
            }
            let bands = |arms: [Weight; 2], span: i32| {
                arms.iter()
                    .flat_map(|&weight| band(weight, span, light))
                    .collect::<Vec<_>>()
            };
            let (across, along) = (bands([up, down], w), bands([left, right], h));
            let (own_x, own_y) = (bands([left, right], w), bands([up, down], h));
            let (xs, ys): (Vec<i32>, Vec<i32>) = ink.iter().copied().unzip();
            let reach = |pixels: &[i32], low: bool| {
                if low {
                    *pixels.iter().min().unwrap()
                } else {
                    *pixels.iter().max().unwrap()
                }
            };
            // Where a crossing band exists, the joint ends at it; otherwise at the arm's own band.
            let limit = |crossing: &[i32], own: &[i32], low: bool| {
                reach(if crossing.is_empty() { own } else { crossing }, low)
            };
            for (absent, pixels, crossing, own, low, side) in [
                (left == N, &xs, &across, &own_x, true, "left"),
                (right == N, &xs, &across, &own_x, false, "right"),
                (up == N, &ys, &along, &own_y, true, "above"),
                (down == N, &ys, &along, &own_y, false, "below"),
            ] {
                if absent {
                    assert_eq!(
                        reach(pixels, low),
                        limit(crossing, own, low),
                        "{c} reaches {side} of its joint at {w}×{h}, light {light}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_range_is_drawn_whole_except_the_three_diagonals() {
    let left_to_the_font: Vec<char> = (0x2500..=0x257F)
        .filter_map(char::from_u32)
        .filter(|&c| shape_of(c).is_none())
        .collect();
    assert_eq!(left_to_the_font, ['╱', '╲', '╳']);
    assert_eq!(shape_of('▀'), None, "a block element belongs to `blocks`");
    assert_eq!(shape_of('┐'), line(L, N, N, L));
}

#[test]
fn a_light_line_follows_the_scale_and_is_never_thinner_than_a_pixel() {
    assert_eq!(light_thickness(14.0, 1.0), 1);
    assert_eq!(light_thickness(14.0, 1.25), 1);
    assert_eq!(light_thickness(16.0, 1.5), 2);
    assert_eq!(light_thickness(14.0, 5.0 / 3.0), 2);
    assert_eq!(light_thickness(14.0, 2.0), 2);
    assert_eq!(light_thickness(4.0, 1.0), 1);
}

#[test]
fn a_rounded_corner_is_one_piece_whose_every_step_shares_an_edge() {
    // **Seen on the desktop, not in a test**: a one-pixel arc made of the pixels its centre line
    // crosses was two pixels touching only at a corner, and it read as a gap in the box. Asserted
    // as the reason — the stroke is one 4-connected piece — at the cell sizes light lines come with.
    for c in ['╭', '╮', '╯', '╰'] {
        for (w, h, light) in [(7, 16, 1), (7, 17, 1), (8, 17, 1), (11, 27, 2), (12, 29, 2)] {
            let mut mask = vec![false; (w * h) as usize];
            for (x0, y0, x1, y1) in runs(shape_of(c).unwrap(), w, h, light) {
                for y in y0..y1 {
                    for x in x0..x1 {
                        mask[(y * w + x) as usize] = true;
                    }
                }
            }
            let start = mask
                .iter()
                .position(|&inked| inked)
                .expect("the arc drew nothing");
            let mut seen = vec![false; mask.len()];
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(at) = stack.pop() {
                let (x, y) = (at as i32 % w, at as i32 / w);
                for (nx, ny) in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                    if (0..w).contains(&nx) && (0..h).contains(&ny) {
                        let next = (ny * w + nx) as usize;
                        if mask[next] && !seen[next] {
                            seen[next] = true;
                            stack.push(next);
                        }
                    }
                }
            }
            assert_eq!(
                seen.iter().filter(|&&s| s).count(),
                mask.iter().filter(|&&m| m).count(),
                "{c} at {w}×{h}, light {light}: a step of the curve only touches the next at a corner"
            );
        }
    }
}
