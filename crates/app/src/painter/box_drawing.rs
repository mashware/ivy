//! The box drawing characters — `U+2500`–`U+257F` but the three diagonals — drawn against the cell.
//!
//! **A line is not a letter, for the same reason a block is not one** (`blocks`, **D35**): its job
//! is to meet the line in the next cell. A font's glyph cannot. Its raster is wider than a cell
//! floored to whole pixels (**D34**), so neighbours overlap and their antialiased ends composite
//! twice into a bright bead at every seam. And its stroke sits at the font's fractional height and
//! thickness, so it is spread over two or three partly covered rows. Both were measured, at five
//! scales and with two fonts, in issue #103.
//!
//! **So the shape is computed in device pixels and painted as disjoint rectangles.** Every
//! thickness and every band position is a whole number of pixels taken from the cell's own pixel
//! size, which every cell of a grid shares, so a band leaves one cell at exactly the rows the next
//! one enters at. Kitty, WezTerm, Ghostty and VTE all draw this range themselves.
//!
//! **The cell is rasterized into a mask first and the mask is cut into rectangles**, rather than
//! one rectangle per arm. Arms meet at the joint, and two translucent rectangles over the same
//! pixel paint it twice — the bead again, now drawn by us. A mask is a union by construction, and
//! it is also what draws the arcs.

/// How heavy an arm is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Weight {
    None,
    Light,
    Heavy,
    Double,
}

use Weight::{Double as D, Heavy as H, Light as L, None as N};

/// What one character draws: its four arms, whether it is dashed and whether its corner is round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shape {
    /// Left, right, up, down.
    arms: [Weight; 4],
    /// How many dashes the line is cut into; `0` is a solid line.
    dashes: u8,
    /// A rounded corner instead of a square one.
    arc: bool,
}

const fn line(left: Weight, right: Weight, up: Weight, down: Weight) -> Option<Shape> {
    Some(Shape {
        arms: [left, right, up, down],
        dashes: 0,
        arc: false,
    })
}

const fn dashed(
    dashes: u8,
    left: Weight,
    right: Weight,
    up: Weight,
    down: Weight,
) -> Option<Shape> {
    Some(Shape {
        arms: [left, right, up, down],
        dashes,
        arc: false,
    })
}

const fn arc(left: Weight, right: Weight, up: Weight, down: Weight) -> Option<Shape> {
    Some(Shape {
        arms: [left, right, up, down],
        dashes: 0,
        arc: true,
    })
}

/// The whole range, in code point order, generated from the Unicode character names.
///
/// **The three diagonals are `None` and stay with the font.** A diagonal cannot be cut into
/// rectangles on a pixel grid without a staircase, and nothing beside it in a box has to meet it
/// on a shared row.
const TABLE: [Option<Shape>; 128] = [
    line(L, L, N, N),      // 2500 ─ light horizontal
    line(H, H, N, N),      // 2501 ━ heavy horizontal
    line(N, N, L, L),      // 2502 │ light vertical
    line(N, N, H, H),      // 2503 ┃ heavy vertical
    dashed(3, L, L, N, N), // 2504 ┄ light triple dash horizontal
    dashed(3, H, H, N, N), // 2505 ┅ heavy triple dash horizontal
    dashed(3, N, N, L, L), // 2506 ┆ light triple dash vertical
    dashed(3, N, N, H, H), // 2507 ┇ heavy triple dash vertical
    dashed(4, L, L, N, N), // 2508 ┈ light quadruple dash horizontal
    dashed(4, H, H, N, N), // 2509 ┉ heavy quadruple dash horizontal
    dashed(4, N, N, L, L), // 250A ┊ light quadruple dash vertical
    dashed(4, N, N, H, H), // 250B ┋ heavy quadruple dash vertical
    line(N, L, N, L),      // 250C ┌ light down and right
    line(N, H, N, L),      // 250D ┍ down light and right heavy
    line(N, L, N, H),      // 250E ┎ down heavy and right light
    line(N, H, N, H),      // 250F ┏ heavy down and right
    line(L, N, N, L),      // 2510 ┐ light down and left
    line(H, N, N, L),      // 2511 ┑ down light and left heavy
    line(L, N, N, H),      // 2512 ┒ down heavy and left light
    line(H, N, N, H),      // 2513 ┓ heavy down and left
    line(N, L, L, N),      // 2514 └ light up and right
    line(N, H, L, N),      // 2515 ┕ up light and right heavy
    line(N, L, H, N),      // 2516 ┖ up heavy and right light
    line(N, H, H, N),      // 2517 ┗ heavy up and right
    line(L, N, L, N),      // 2518 ┘ light up and left
    line(H, N, L, N),      // 2519 ┙ up light and left heavy
    line(L, N, H, N),      // 251A ┚ up heavy and left light
    line(H, N, H, N),      // 251B ┛ heavy up and left
    line(N, L, L, L),      // 251C ├ light vertical and right
    line(N, H, L, L),      // 251D ┝ vertical light and right heavy
    line(N, L, H, L),      // 251E ┞ up heavy and right down light
    line(N, L, L, H),      // 251F ┟ down heavy and right up light
    line(N, L, H, H),      // 2520 ┠ vertical heavy and right light
    line(N, H, H, L),      // 2521 ┡ down light and right up heavy
    line(N, H, L, H),      // 2522 ┢ up light and right down heavy
    line(N, H, H, H),      // 2523 ┣ heavy vertical and right
    line(L, N, L, L),      // 2524 ┤ light vertical and left
    line(H, N, L, L),      // 2525 ┥ vertical light and left heavy
    line(L, N, H, L),      // 2526 ┦ up heavy and left down light
    line(L, N, L, H),      // 2527 ┧ down heavy and left up light
    line(L, N, H, H),      // 2528 ┨ vertical heavy and left light
    line(H, N, H, L),      // 2529 ┩ down light and left up heavy
    line(H, N, L, H),      // 252A ┪ up light and left down heavy
    line(H, N, H, H),      // 252B ┫ heavy vertical and left
    line(L, L, N, L),      // 252C ┬ light down and horizontal
    line(H, L, N, L),      // 252D ┭ left heavy and right down light
    line(L, H, N, L),      // 252E ┮ right heavy and left down light
    line(H, H, N, L),      // 252F ┯ down light and horizontal heavy
    line(L, L, N, H),      // 2530 ┰ down heavy and horizontal light
    line(H, L, N, H),      // 2531 ┱ right light and left down heavy
    line(L, H, N, H),      // 2532 ┲ left light and right down heavy
    line(H, H, N, H),      // 2533 ┳ heavy down and horizontal
    line(L, L, L, N),      // 2534 ┴ light up and horizontal
    line(H, L, L, N),      // 2535 ┵ left heavy and right up light
    line(L, H, L, N),      // 2536 ┶ right heavy and left up light
    line(H, H, L, N),      // 2537 ┷ up light and horizontal heavy
    line(L, L, H, N),      // 2538 ┸ up heavy and horizontal light
    line(H, L, H, N),      // 2539 ┹ right light and left up heavy
    line(L, H, H, N),      // 253A ┺ left light and right up heavy
    line(H, H, H, N),      // 253B ┻ heavy up and horizontal
    line(L, L, L, L),      // 253C ┼ light vertical and horizontal
    line(H, L, L, L),      // 253D ┽ left heavy and right vertical light
    line(L, H, L, L),      // 253E ┾ right heavy and left vertical light
    line(H, H, L, L),      // 253F ┿ vertical light and horizontal heavy
    line(L, L, H, L),      // 2540 ╀ up heavy and down horizontal light
    line(L, L, L, H),      // 2541 ╁ down heavy and up horizontal light
    line(L, L, H, H),      // 2542 ╂ vertical heavy and horizontal light
    line(H, L, H, L),      // 2543 ╃ left up heavy and right down light
    line(L, H, H, L),      // 2544 ╄ right up heavy and left down light
    line(H, L, L, H),      // 2545 ╅ left down heavy and right up light
    line(L, H, L, H),      // 2546 ╆ right down heavy and left up light
    line(H, H, H, L),      // 2547 ╇ down light and up horizontal heavy
    line(H, H, L, H),      // 2548 ╈ up light and down horizontal heavy
    line(H, L, H, H),      // 2549 ╉ right light and left vertical heavy
    line(L, H, H, H),      // 254A ╊ left light and right vertical heavy
    line(H, H, H, H),      // 254B ╋ heavy vertical and horizontal
    dashed(2, L, L, N, N), // 254C ╌ light double dash horizontal
    dashed(2, H, H, N, N), // 254D ╍ heavy double dash horizontal
    dashed(2, N, N, L, L), // 254E ╎ light double dash vertical
    dashed(2, N, N, H, H), // 254F ╏ heavy double dash vertical
    line(D, D, N, N),      // 2550 ═ double horizontal
    line(N, N, D, D),      // 2551 ║ double vertical
    line(N, D, N, L),      // 2552 ╒ down single and right double
    line(N, L, N, D),      // 2553 ╓ down double and right single
    line(N, D, N, D),      // 2554 ╔ double down and right
    line(D, N, N, L),      // 2555 ╕ down single and left double
    line(L, N, N, D),      // 2556 ╖ down double and left single
    line(D, N, N, D),      // 2557 ╗ double down and left
    line(N, D, L, N),      // 2558 ╘ up single and right double
    line(N, L, D, N),      // 2559 ╙ up double and right single
    line(N, D, D, N),      // 255A ╚ double up and right
    line(D, N, L, N),      // 255B ╛ up single and left double
    line(L, N, D, N),      // 255C ╜ up double and left single
    line(D, N, D, N),      // 255D ╝ double up and left
    line(N, D, L, L),      // 255E ╞ vertical single and right double
    line(N, L, D, D),      // 255F ╟ vertical double and right single
    line(N, D, D, D),      // 2560 ╠ double vertical and right
    line(D, N, L, L),      // 2561 ╡ vertical single and left double
    line(L, N, D, D),      // 2562 ╢ vertical double and left single
    line(D, N, D, D),      // 2563 ╣ double vertical and left
    line(D, D, N, L),      // 2564 ╤ down single and horizontal double
    line(L, L, N, D),      // 2565 ╥ down double and horizontal single
    line(D, D, N, D),      // 2566 ╦ double down and horizontal
    line(D, D, L, N),      // 2567 ╧ up single and horizontal double
    line(L, L, D, N),      // 2568 ╨ up double and horizontal single
    line(D, D, D, N),      // 2569 ╩ double up and horizontal
    line(D, D, L, L),      // 256A ╪ vertical single and horizontal double
    line(L, L, D, D),      // 256B ╫ vertical double and horizontal single
    line(D, D, D, D),      // 256C ╬ double vertical and horizontal
    arc(N, L, N, L),       // 256D ╭ light arc down and right
    arc(L, N, N, L),       // 256E ╮ light arc down and left
    arc(L, N, L, N),       // 256F ╯ light arc up and left
    arc(N, L, L, N),       // 2570 ╰ light arc up and right
    None,                  // 2571 ╱ light diagonal upper right to lower left
    None,                  // 2572 ╲ light diagonal upper left to lower right
    None,                  // 2573 ╳ light diagonal cross
    line(L, N, N, N),      // 2574 ╴ light left
    line(N, N, L, N),      // 2575 ╵ light up
    line(N, L, N, N),      // 2576 ╶ light right
    line(N, N, N, L),      // 2577 ╷ light down
    line(H, N, N, N),      // 2578 ╸ heavy left
    line(N, N, H, N),      // 2579 ╹ heavy up
    line(N, H, N, N),      // 257A ╺ heavy right
    line(N, N, N, H),      // 257B ╻ heavy down
    line(L, H, N, N),      // 257C ╼ light left and heavy right
    line(N, N, L, H),      // 257D ╽ light up and heavy down
    line(H, L, N, N),      // 257E ╾ heavy left and light right
    line(N, N, H, L),      // 257F ╿ heavy up and light down
];

/// The shape a character draws, or `None` if it is not one this module draws.
pub(crate) fn shape_of(c: char) -> Option<Shape> {
    let index = (c as u32).checked_sub(0x2500)?;
    TABLE.get(index as usize).copied().flatten()
}

/// A rectangle in whole device pixels, relative to the cell: left, top, right, bottom.
pub(crate) type Run = (i32, i32, i32, i32);

/// How thick a light line is, in device pixels.
///
/// **It follows the font's size and the screen's scale together**, so a light line keeps the
/// weight of a thin stroke of the text beside it: one pixel at the default size on a screen at
/// 100 %, two at 166 % and 200 %. It is never thinner than a pixel, which would be no line.
pub(crate) fn light_thickness(font_points: f32, per_point: f32) -> i32 {
    ((font_points * per_point / 12.0).round() as i32).max(1)
}

/// A cell of `width` × `height` device pixels, rasterized and cut into disjoint rectangles.
pub(crate) fn runs(shape: Shape, width: i32, height: i32, light: i32) -> Vec<Run> {
    let mut mask = Mask::new(width, height);
    if shape.arc {
        mask.arc(shape, light);
    } else {
        mask.lines(shape, light);
    }
    mask.into_runs()
}

/// Where a band of thickness `thick` starts so that it is centred in `span`, rounded down.
///
/// **Rounded down, always, and from the cell's own size**: every cell of the grid has the same
/// size, so every cell puts the same band at the same offset, and that is what joins them.
fn centred(span: i32, thick: i32) -> i32 {
    (span - thick).div_euclid(2)
}

struct Mask {
    width: i32,
    height: i32,
    covered: Vec<bool>,
}

impl Mask {
    fn new(width: i32, height: i32) -> Self {
        let (width, height) = (width.max(0), height.max(0));
        Self {
            width,
            height,
            covered: vec![false; (width * height) as usize],
        }
    }

    /// Covers `left..right` × `top..bottom`, clipped to the cell.
    fn fill(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        let (left, right) = (left.max(0), right.min(self.width));
        let (top, bottom) = (top.max(0), bottom.min(self.height));
        for y in top..bottom {
            for x in left..right {
                self.covered[(y * self.width + x) as usize] = true;
            }
        }
    }

    /// A horizontal band from `from` to `to`, cut into dashes when the shape asks for them.
    fn horizontal(&mut self, from: i32, to: i32, top: i32, thick: i32, dashes: u8) {
        for (start, end) in dashes_of(from, to, dashes) {
            self.fill(start, top, end, top + thick);
        }
    }

    fn vertical(&mut self, from: i32, to: i32, left: i32, thick: i32, dashes: u8) {
        for (start, end) in dashes_of(from, to, dashes) {
            self.fill(left, start, left + thick, end);
        }
    }

    /// Straight arms, single or double, meeting at the joint.
    ///
    /// **A single arm stops where the perpendicular one starts**, which a union makes harmless.
    /// **A double arm is two light lines a light line apart**, and each of its lines ends against
    /// the line of the perpendicular arm it would otherwise cross: that is what leaves `╬` its
    /// four corners and `╦` its gap underneath.
    fn lines(&mut self, shape: Shape, light: i32) {
        let [left, right, up, down] = shape.arms;
        let (w, h) = (self.width, self.height);
        let single = |weight: Weight| match weight {
            Weight::Light => light,
            Weight::Heavy => light * 2,
            _ => 0,
        };
        // A dashed line is one straight line across the whole cell, cut: its dashes are spaced
        // over the cell, not over each half of it.
        if shape.dashes > 0 {
            let thick = single(left).max(single(up));
            if left != N {
                self.horizontal(0, w, centred(h, thick), thick, shape.dashes);
            } else {
                self.vertical(0, h, centred(w, thick), thick, shape.dashes);
            }
            return;
        }
        // The thickest single arm on each axis decides where the other axis's arms meet it.
        let across = single(left).max(single(right));
        let along = single(up).max(single(down));
        // The two lines of a double, on each axis: where each starts.
        let (row_one, column_one) = (centred(h, light * 3), centred(w, light * 3));
        let (row_two, column_two) = (row_one + light * 2, column_one + light * 2);
        let horizontal_double = left == D || right == D;
        let vertical_double = up == D || down == D;
        // A single arm meeting a double axis: across the gap where the double passes through,
        // against its near line where the double turns a corner.
        let both_vertical = up != N && down != N;
        let both_horizontal = left != N && right != N;

        for (weight, is_right) in [(left, false), (right, true)] {
            match weight {
                N => {}
                D => {
                    for (row, opens_to) in [(row_one, up), (row_two, down)] {
                        // This line of the double stops at the near line of a double crossing on
                        // its side, at the band of a single one, or runs to the far line.
                        let (start, end) = if is_right {
                            let start = if opens_to == D {
                                column_two
                            } else if along > 0 {
                                centred(w, along)
                            } else {
                                column_one
                            };
                            (start, w)
                        } else {
                            let end = if opens_to == D {
                                column_one + light
                            } else if along > 0 {
                                centred(w, along) + along
                            } else {
                                column_two + light
                            };
                            (0, end)
                        };
                        self.horizontal(start, end, row, light, 0);
                    }
                }
                _ => {
                    let thick = single(weight);
                    let (start, end) = if vertical_double {
                        let near = if both_vertical == is_right {
                            column_two
                        } else {
                            column_one
                        };
                        if is_right {
                            (near, w)
                        } else {
                            (0, near + light)
                        }
                    } else {
                        let meet = if along > 0 { along } else { thick };
                        let at = centred(w, meet);
                        if is_right { (at, w) } else { (0, at + meet) }
                    };
                    self.horizontal(start, end, centred(h, thick), thick, 0);
                }
            }
        }

        for (weight, is_down) in [(up, false), (down, true)] {
            match weight {
                N => {}
                D => {
                    for (column, opens_to) in [(column_one, left), (column_two, right)] {
                        let (start, end) = if is_down {
                            let start = if opens_to == D {
                                row_two
                            } else if across > 0 {
                                centred(h, across)
                            } else {
                                row_one
                            };
                            (start, h)
                        } else {
                            let end = if opens_to == D {
                                row_one + light
                            } else if across > 0 {
                                centred(h, across) + across
                            } else {
                                row_two + light
                            };
                            (0, end)
                        };
                        self.vertical(start, end, column, light, 0);
                    }
                }
                _ => {
                    let thick = single(weight);
                    let (start, end) = if horizontal_double {
                        let near = if both_horizontal == is_down {
                            row_two
                        } else {
                            row_one
                        };
                        if is_down {
                            (near, h)
                        } else {
                            (0, near + light)
                        }
                    } else {
                        let meet = if across > 0 { across } else { thick };
                        let at = centred(h, meet);
                        if is_down { (at, h) } else { (0, at + meet) }
                    };
                    self.vertical(start, end, centred(w, thick), thick, 0);
                }
            }
        }
    }

    /// A rounded corner: a quarter circle tangent to both arms, then straight to the edges.
    ///
    /// **The circle is centred off the bands' own centre lines**, so where it leaves the arc it
    /// is on exactly the rows and columns a straight line in the next cell uses.
    ///
    /// **Whole pixels, and a staircase rather than a diagonal.** The radius is about half a cell,
    /// a few pixels. Taking only the pixels the stroke's centre line passes through left two pixels
    /// touching at a corner, and antialiasing them left them at half ink beside lines at full ink.
    /// Both read as a gap in the corner of the box on the desktop. A pixel the stroke covers at
    /// least a third of is inked whole, which makes each step share an edge with the next.
    fn arc(&mut self, shape: Shape, light: i32) {
        let [left, _, up, _] = shape.arms;
        let (w, h) = (self.width, self.height);
        let (band_x, band_y) = (centred(w, light), centred(h, light));
        let half = light as f32 / 2.0;
        let (cx, cy) = (band_x as f32 + half, band_y as f32 + half);
        let toward_x = if left != N { -1.0 } else { 1.0 };
        let toward_y = if up != N { -1.0 } else { 1.0 };
        let room_x = if toward_x > 0.0 { w as f32 - cx } else { cx };
        let room_y = if toward_y > 0.0 { h as f32 - cy } else { cy };
        // A pixel short of the room, so the curve ends inside the cell: what crosses into the next
        // cell is only the straight band.
        let radius = (room_x.min(room_y) - 1.0).max(half);
        let (ox, oy) = (cx + toward_x * radius, cy + toward_y * radius);
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let in_quarter = (px - ox) * toward_x <= 0.0 && (py - oy) * toward_y <= 0.0;
                let distance = ((px - ox).powi(2) + (py - oy).powi(2)).sqrt();
                let coverage = half + 0.5 - (distance - radius).abs();
                if in_quarter && coverage >= 1.0 / 3.0 {
                    self.fill(x, y, x + 1, y + 1);
                }
            }
        }
        // What the circle did not reach, straight to the two edges.
        let ox = ox.round() as i32;
        let oy = oy.round() as i32;
        if toward_x > 0.0 {
            self.fill(ox, band_y, w, band_y + light);
        } else {
            self.fill(0, band_y, ox, band_y + light);
        }
        if toward_y > 0.0 {
            self.fill(band_x, oy, band_x + light, h);
        } else {
            self.fill(band_x, 0, band_x + light, oy);
        }
    }

    /// The covered pixels as rectangles that never overlap: runs per row, and a run carried down
    /// while the next row has exactly the same one.
    fn into_runs(self) -> Vec<Run> {
        let mut done: Vec<Run> = Vec::new();
        let mut open: Vec<Run> = Vec::new();
        for y in 0..self.height {
            let row = &self.covered[(y * self.width) as usize..((y + 1) * self.width) as usize];
            let mut here = Vec::new();
            let mut x = 0;
            while x < self.width {
                if row[x as usize] {
                    let start = x;
                    while x < self.width && row[x as usize] {
                        x += 1;
                    }
                    here.push((start, x));
                } else {
                    x += 1;
                }
            }
            let mut next = Vec::with_capacity(here.len());
            for (start, end) in here {
                match open.iter().position(|r| r.0 == start && r.2 == end) {
                    Some(at) => {
                        let mut run = open.swap_remove(at);
                        run.3 = y + 1;
                        next.push(run);
                    }
                    None => next.push((start, y, end, y + 1)),
                }
            }
            done.append(&mut open);
            open = next;
        }
        done.append(&mut open);
        done
    }
}

/// The pieces of `from..to` a line cut into `dashes` dashes keeps, each followed by its gap.
///
/// **The gap goes at the end of each piece, not around it**, so the last gap of one cell and the
/// first dash of the next keep the same rhythm across the seam.
fn dashes_of(from: i32, to: i32, dashes: u8) -> Vec<(i32, i32)> {
    if dashes == 0 {
        return vec![(from, to)];
    }
    let n = i32::from(dashes);
    let span = to - from;
    let gap = (span / (n * 3)).max(1);
    (0..n)
        .map(|i| {
            let start = from + span * i / n;
            let end = from + span * (i + 1) / n;
            // Where a piece is too short to hold both, the dash wins: a line with no gaps is a
            // line, a dash of nothing is not there.
            if end - start > gap {
                (start, end - gap)
            } else {
                (start, end)
            }
        })
        .collect()
}

/// Paints a box drawing character over its cell.
///
/// **The cell is taken to device pixels by rounding its two corners, not its size**: the corner a
/// cell ends at is the corner the next one starts at, so both round to the same pixel and the
/// bands meet. The rectangles go back to points exactly, and epaint's own pixel rounding of a
/// filled rectangle then leaves them where they are.
pub(super) fn paint(
    painter: &eframe::egui::Painter,
    shape: Shape,
    slot: eframe::egui::Rect,
    color: eframe::egui::Color32,
    font_points: f32,
    per_point: f32,
) {
    use eframe::egui::{Rect, pos2};
    let left = (slot.left() * per_point).round();
    let top = (slot.top() * per_point).round();
    let width = (slot.right() * per_point).round() - left;
    let height = (slot.bottom() * per_point).round() - top;
    let light = light_thickness(font_points, per_point);
    for (x0, y0, x1, y1) in runs(shape, width as i32, height as i32, light) {
        let rect = Rect::from_min_max(
            pos2(
                (left + x0 as f32) / per_point,
                (top + y0 as f32) / per_point,
            ),
            pos2(
                (left + x1 as f32) / per_point,
                (top + y1 as f32) / per_point,
            ),
        );
        painter.rect_filled(rect, 0.0, color);
    }
}

#[cfg(test)]
mod tests;
