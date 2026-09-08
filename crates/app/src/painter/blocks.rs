//! The block elements —`U+2580`–`U+259F`— measured against the cell instead of asked of the font.
//!
//! **Why they are not left to the font.** A block is not a letter: it is a shape whose whole job is to
//! meet its neighbour with no seam and to fill the cell exactly. What the font ships is a glyph drawn
//! for one nominal size, rasterized with antialiasing and stamped inside a cell of another — so a run
//! of full blocks comes out with hairlines through it and the shades come out as moiré instead of an
//! even wash. The case that brought this in was a status line that draws its progress bar out of ten
//! `░` (`U+2591`, in dim green): beside the same bar in Tilix, ours was visibly speckled.
//!
//! **Everybody outside solves it the same way** —Kitty, WezTerm, Ghostty and VTE all draw these
//! characters themselves— and this is the same answer, kept to the 32 characters of the Block Elements
//! range. **The box drawing block (`U+2500`–`U+257F`) is deliberately left out**: its lines close with
//! the neighbouring cell **because the cell is never larger than what the font measured** —
//! consecutive glyphs overlap by that fraction instead of parting, on both axes, which is the half of
//! **D34** that had to be corrected in review — and synthesising the range means also deciding about
//! heavy, double, rounded and diagonal, which is a different piece of work.
//!
//! **The three shades are a fraction of the ink, not a dot pattern.** A real 25 % dither has to land on
//! the pixel lattice to look even, and at a cell of eight by seventeen there are not enough pixels for
//! it to read as anything but noise — which is the very defect this replaces. A flat wash is what the
//! comparison screenshot shows.
//!
//! **D35** carries all of it with what was discarded, and **D34** the whole-pixel grid this rests on:
//! without that one, rectangles of the right size still meet on a half-covered pixel.

/// The part of a cell a block covers, in fractions of it: left, top, right, bottom.
type Part = (f32, f32, f32, f32);

const UPPER_HALF: Part = (0.0, 0.0, 1.0, 0.5);
const LOWER_HALF: Part = (0.0, 0.5, 1.0, 1.0);
const LEFT_HALF: Part = (0.0, 0.0, 0.5, 1.0);
const RIGHT_HALF: Part = (0.5, 0.0, 1.0, 1.0);
const UPPER_LEFT: Part = (0.0, 0.0, 0.5, 0.5);
const UPPER_RIGHT: Part = (0.5, 0.0, 1.0, 0.5);
const LOWER_LEFT: Part = (0.0, 0.5, 0.5, 1.0);
const LOWER_RIGHT: Part = (0.5, 0.5, 1.0, 1.0);
const WHOLE: Part = (0.0, 0.0, 1.0, 1.0);

/// What a block element covers of its cell, and with how much of the ink.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Filling {
    /// **Two are enough for all thirty-two**, and that is worth writing down because the quadrant
    /// characters look as though they need three: `▙` is upper-left, lower-left and lower-right, which
    /// is the left half plus the bottom half. Every three-quadrant character folds the same way.
    parts: [Part; 2],
    /// How many of the two count. A second part of zero size would still be a rectangle handed to the
    /// tessellator on the hottest path of the frame.
    used: usize,
    /// How much of the foreground reaches the cell, out of 255. Only the three shades ask for less
    /// than all of it.
    alpha: u8,
}

impl Filling {
    /// The parts that count, in the order they are painted.
    pub(crate) fn parts(&self) -> &[Part] {
        &self.parts[..self.used]
    }

    /// How much of the foreground reaches the cell, out of 255.
    pub(crate) fn alpha(&self) -> u8 {
        self.alpha
    }

    /// One rectangle, with all the ink.
    const fn solid(part: Part) -> Self {
        Self {
            parts: [part, WHOLE],
            used: 1,
            alpha: u8::MAX,
        }
    }

    /// Two rectangles, with all the ink.
    const fn both(first: Part, second: Part) -> Self {
        Self {
            parts: [first, second],
            used: 2,
            alpha: u8::MAX,
        }
    }

    /// The whole cell, with a fraction of the ink.
    const fn wash(alpha: u8) -> Self {
        Self {
            parts: [WHOLE, WHOLE],
            used: 1,
            alpha,
        }
    }
}

/// What that character covers of its cell, or nothing if it is not a block element.
///
/// **The eighths are arithmetic and the quadrants are a table**, and that is not a stylistic choice:
/// `U+2581`–`U+2588` and `U+2589`–`U+258F` are each a run in which the codepoint's distance from one
/// end **is** the numerator — from the start for the lower eighths, from the end for the left ones,
/// which is the asymmetry the two arms below spell out — so a table there would be fifteen chances to
/// typo a fraction. The ten quadrant characters follow no such order and are the only ones worth
/// spelling out.
pub(crate) fn filling_of(character: char) -> Option<Filling> {
    let code = character as u32;
    Some(match code {
        // `▀` and `▐`, the two halves that are not part of either run.
        0x2580 => Filling::solid(UPPER_HALF),
        0x2590 => Filling::solid(RIGHT_HALF),
        // `▁`…`█`: the lower n eighths, with `▄` the lower half and `█` the whole cell falling out of
        // the same arithmetic.
        0x2581..=0x2588 => {
            let eighths = (code - 0x2580) as f32 / 8.0;
            Filling::solid((0.0, 1.0 - eighths, 1.0, 1.0))
        }
        // `▉`…`▏`: the left n eighths, counted **backwards** — the range starts at seven eighths and
        // narrows towards `▏`, which is one.
        0x2589..=0x258F => {
            let eighths = (0x2590 - code) as f32 / 8.0;
            Filling::solid((0.0, 0.0, eighths, 1.0))
        }
        // `░`, `▒`, `▓`: a quarter, a half and three quarters of the ink over the whole cell.
        0x2591 => Filling::wash(64),
        0x2592 => Filling::wash(128),
        0x2593 => Filling::wash(191),
        // `▔` and `▕`, the two eighths that go against the grain of their runs.
        0x2594 => Filling::solid((0.0, 0.0, 1.0, 1.0 / 8.0)),
        0x2595 => Filling::solid((7.0 / 8.0, 0.0, 1.0, 1.0)),
        // The quadrants. The three-quadrant ones are written as the two halves they add up to.
        0x2596 => Filling::solid(LOWER_LEFT),
        0x2597 => Filling::solid(LOWER_RIGHT),
        0x2598 => Filling::solid(UPPER_LEFT),
        0x2599 => Filling::both(LEFT_HALF, LOWER_HALF),
        0x259A => Filling::both(UPPER_LEFT, LOWER_RIGHT),
        0x259B => Filling::both(UPPER_HALF, LEFT_HALF),
        0x259C => Filling::both(UPPER_HALF, RIGHT_HALF),
        0x259D => Filling::solid(UPPER_RIGHT),
        0x259E => Filling::both(UPPER_RIGHT, LOWER_LEFT),
        0x259F => Filling::both(RIGHT_HALF, LOWER_HALF),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_of_the_range_is_covered_and_nothing_outside_it_is() {
        // **The whole range or none of it.** A hole in the middle would be a character drawn by the font
        // beside its neighbours drawn here, which is worse than either alone: the mismatch moves as the
        // text does. The range is closed, so this walks it.
        for code in 0x2580..=0x259Fu32 {
            let character = char::from_u32(code).unwrap();
            assert!(
                filling_of(character).is_some(),
                "{character} (U+{code:04X}) is a block element and is left to the font"
            );
        }
        // **The neighbours on both sides**, so the range cannot silently grow: `╿` closes the box
        // drawing block and `■` opens what comes after.
        for code in [0x257Fu32, 0x25A0, 0x2500, 0x2588 + 0x1000] {
            let character = char::from_u32(code).unwrap();
            assert!(
                filling_of(character).is_none(),
                "{character} (U+{code:04X}) is not a block element and is drawn as one"
            );
        }
    }

    #[test]
    fn the_full_block_covers_its_whole_cell_and_the_thin_ones_a_sliver() {
        // **Assert on the fractions, not on which arm matched.** The eighths come out of arithmetic on
        // the codepoint, so an off-by-one in either run gives a filling that is perfectly well formed
        // and covers the wrong part of the cell — which is exactly what a test on «it returned
        // something» cannot see.
        assert_eq!(filling_of('█').unwrap().parts(), [WHOLE]);
        assert_eq!(filling_of('▀').unwrap().parts(), [(0.0, 0.0, 1.0, 0.5)]);
        assert_eq!(filling_of('▄').unwrap().parts(), [(0.0, 0.5, 1.0, 1.0)]);
        assert_eq!(filling_of('▌').unwrap().parts(), [(0.0, 0.0, 0.5, 1.0)]);
        assert_eq!(filling_of('▐').unwrap().parts(), [(0.5, 0.0, 1.0, 1.0)]);
        // The two ends of each run, which is where an off-by-one shows.
        assert_eq!(
            filling_of('▁').unwrap().parts(),
            [(0.0, 7.0 / 8.0, 1.0, 1.0)],
            "the lowest eighth"
        );
        assert_eq!(
            filling_of('▏').unwrap().parts(),
            [(0.0, 0.0, 1.0 / 8.0, 1.0)],
            "the leftmost eighth"
        );
        assert_eq!(
            filling_of('▉').unwrap().parts(),
            [(0.0, 0.0, 7.0 / 8.0, 1.0)],
            "seven eighths from the left"
        );
        // **The two that go against the grain of their runs**, and therefore the two most typo-prone
        // lines in the table: they are the only eighths measured from the top and from the right.
        // Found unguarded by a reviewer who moved `▕` to the left edge — where it is `▏` — and made
        // `▔` three eighths tall, with the suite green both times.
        assert_eq!(
            filling_of('▔').unwrap().parts(),
            [(0.0, 0.0, 1.0, 1.0 / 8.0)],
            "the topmost eighth"
        );
        assert_eq!(
            filling_of('▕').unwrap().parts(),
            [(7.0 / 8.0, 0.0, 1.0, 1.0)],
            "the rightmost eighth"
        );
    }

    #[test]
    fn the_shades_take_the_whole_cell_with_part_of_the_ink_and_are_ordered() {
        // **The three have to differ and to grow in that order**, and the assertion is on the order
        // rather than on three literals: swapping two of the constants leaves three valid alphas, and a
        // test on the numbers one by one passes while `░` paints darker than `▓`.
        let alphas: Vec<u8> = "░▒▓"
            .chars()
            .map(|c| {
                let filling = filling_of(c).unwrap();
                assert_eq!(filling.parts(), [WHOLE], "a shade covers the whole cell");
                filling.alpha()
            })
            .collect();
        assert!(
            alphas[0] < alphas[1] && alphas[1] < alphas[2],
            "the shades came out {alphas:?}, which is not light, medium and dark"
        );
        assert!(
            alphas[2] < u8::MAX,
            "the darkest shade takes all the ink, so it is indistinguishable from a full block"
        );
        // **And each one is near the fraction its name promises.** Order alone is satisfied by `1, 2,
        // 3` — a light shade at alpha 1 is invisible on screen and the ordering holds — so the band is
        // what keeps `░` a quarter of the ink instead of merely the lightest of three. It is a band and
        // not three literals so that the exact constants stay somebody's to tune.
        for (shade, alpha, fraction) in [
            ('░', alphas[0], 0.25),
            ('▒', alphas[1], 0.5),
            ('▓', alphas[2], 0.75),
        ] {
            let taken = f32::from(alpha) / f32::from(u8::MAX);
            assert!(
                (taken - fraction).abs() < 0.05,
                "{shade} takes {taken:.2} of the ink and its name promises {fraction}"
            );
        }
    }

    #[test]
    fn every_quadrant_character_covers_its_quadrants_and_no_others() {
        // **What is checked is the AREA covered**, not the shape of the fold, because the
        // three-quadrant ones are written as the two halves they add up to. Written as «upper-left
        // plus lower-right» —a plausible-looking slip— `▙` covers half the cell instead of three
        // quarters, and every assertion about it being two parts still holds.
        //
        // **All ten, and that is a correction.** The first version walked only the four
        // three-quadrant ones, so a reviewer could put `▖` in the upper-right corner and flip `▚` into
        // `▞` with the suite green — six of the ten covered by nothing but «it returned something»,
        // which is the disguise the sentence above is about.
        for (character, quadrants) in [
            ('▖', &[LOWER_LEFT][..]),
            ('▗', &[LOWER_RIGHT]),
            ('▘', &[UPPER_LEFT]),
            ('▝', &[UPPER_RIGHT]),
            ('▚', &[UPPER_LEFT, LOWER_RIGHT]),
            ('▞', &[UPPER_RIGHT, LOWER_LEFT]),
            ('▙', &[UPPER_LEFT, LOWER_LEFT, LOWER_RIGHT]),
            ('▛', &[UPPER_LEFT, UPPER_RIGHT, LOWER_LEFT]),
            ('▜', &[UPPER_LEFT, UPPER_RIGHT, LOWER_RIGHT]),
            ('▟', &[UPPER_RIGHT, LOWER_LEFT, LOWER_RIGHT]),
        ] {
            let parts = filling_of(character).unwrap();
            for &quadrant in quadrants {
                let center = (
                    (quadrant.0 + quadrant.2) / 2.0,
                    (quadrant.1 + quadrant.3) / 2.0,
                );
                assert!(
                    parts.parts().iter().any(|part| {
                        center.0 > part.0
                            && center.0 < part.2
                            && center.1 > part.1
                            && center.1 < part.3
                    }),
                    "{character} leaves the quadrant at {center:?} unpainted"
                );
            }
            // **And it covers none of the others**, which is the other half of the mistake and the
            // half that catches a quadrant put in the wrong corner: covering the right ones is
            // satisfied by covering the whole cell.
            for empty in [UPPER_LEFT, UPPER_RIGHT, LOWER_LEFT, LOWER_RIGHT]
                .into_iter()
                .filter(|corner| !quadrants.contains(corner))
            {
                let center = ((empty.0 + empty.2) / 2.0, (empty.1 + empty.3) / 2.0);
                assert!(
                    !parts.parts().iter().any(|part| {
                        center.0 > part.0
                            && center.0 < part.2
                            && center.1 > part.1
                            && center.1 < part.3
                    }),
                    "{character} paints the quadrant at {center:?}, which is one it leaves empty"
                );
            }
        }
    }
}
