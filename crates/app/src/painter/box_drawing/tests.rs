use super::*;
use crate::painter::{CellMetrics, CursorInk, paint};
use crate::theme::Theme;
use eframe::egui::{Context, FontId, Id, LayerId, Order, Pos2, Shape as Drawn, epaint::RectShape};
use term_core::{Screen, Size};

/// The scales the defect was measured at in #103, a fractional font size among them so the cell
/// comes out odd on some axis.
const SCALES: [f32; 5] = [1.0, 1.25, 1.5, 5.0 / 3.0, 2.0];
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
    // **The two defects of #103, asserted where a person sees them**: on the pixels. Painted
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
fn every_dashed_line_is_cut_into_its_number_of_dashes() {
    for (c, expected) in [
        ('┄', 3),
        ('┅', 3),
        ('┆', 3),
        ('┈', 4),
        ('┋', 4),
        ('╌', 2),
        ('╏', 2),
    ] {
        let shape = shape_of(c).unwrap();
        for (w, h) in [(8, 17), (16, 34)] {
            let rects = runs(shape, w, h, 1);
            assert_eq!(
                rects.len(),
                expected,
                "{c} at {w}×{h} came out as {rects:?}"
            );
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
    assert_eq!(light_thickness(14.0, 5.0 / 3.0), 2);
    assert_eq!(light_thickness(14.0, 2.0), 2);
    assert_eq!(light_thickness(4.0, 1.0), 1);
}
