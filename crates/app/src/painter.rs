//! Draws the terminal grid: backgrounds, glyphs and cursor.
//!
//! **It is deliberately the most naive painter that works**: one cell at a time, with no cache and
//! no tracking of what changed. This is neither carelessness nor a first draft — optimizing before
//! measuring would hide the very symptom this milestone exists to observe, and the failure mode of a
//! glyph cache is phantom cells, which are the kind you don't see until you are already weeks deep.
//!
//! **The cell's size is asked for, not invented.** The width comes from the font's reference glyph
//! and the height from its line height. Guessing it by eye is what throws the rows out of line.

mod blocks;

/// What the application says **over** a console, which is a piece of this module's job and not of
/// its six hundred lines (**D28**): it left when the second notice arrived, and it is the piece this
/// delivery was already editing.
pub(crate) mod notices;

use crate::fonts::Presentation;
use crate::theme::Theme;
use eframe::egui::{
    Align2, Color32, Context, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2,
};
use std::time::Duration;
use term_core::{CellWidth, CursorShape, Size, TermColor, View, ViewCell};

/// The largest size accepted for a grid.
///
/// Deliberately generous: a 4K monitor with the smallest legible font does not go past about 300
/// columns. It is not a product limit, it is the filter that keeps a rectangle that corresponds to
/// no window from becoming a grid of hundreds of thousands of cells.
const CEILING: Size = Size {
    rows: 300,
    cols: 1000,
};

/// Whether the font being painted with has letters of different widths.
///
/// **The grid is derived from ONE letter** —the `M` in [`CellMetrics::measure`]— because in a
/// monospaced font they all measure the same, so asking about one stands for all. Since the settings
/// file can name any installed family, a proportional one can reach here: whoever asked for it is
/// entitled to it, and this is what stops the consequence being silent. Every cell would keep the
/// width of the `M` while each glyph carries its own advance, so the columns stop lining up and the
/// wide ones spill into the cell next door — which `paint` deliberately does not clip.
///
/// **It is measured HERE and not where the font is chosen**, and that is not a preference: what
/// throws the grid out is what the text engine ends up laying out, and `set_fonts` takes effect a
/// pass later, so asked at choosing time this question still answers for the previous font.
///
/// **It decides and does not print**, like its siblings across this crate: printing cannot be
/// observed from inside the process, so what a test can hold on to is the decision.
///
/// The margin is half a point over a cell of about eight: no monospaced font differs at all —the two
/// advances are the same number— and a proportional one differs by two to four.
fn the_letters_do_not_all_measure_the_same(wide: f32, narrow: f32) -> bool {
    (wide - narrow).abs() > 0.5
}

/// Whether the sentence above has been said already.
static SAID_THE_LETTERS_DIFFER: std::sync::Once = std::sync::Once::new();

/// A length in points taken **down** to a whole device pixel.
///
/// **The engine rounds each cell's drawing to the pixel grid on its own, and a fractional step is
/// what turns that into a gap.** This painter puts one glyph per cell, so epaint rounds **each
/// origin separately** — `round_text_to_pixels`, on by default — and a step of `8.232` gives origins
/// at `0, 8.232, 16.464, 24.696…`, which round to `0, 8, 16, 25, 33, 41`: the steps are `8, 8, 9, 8,
/// 8`. The ninth pixel is a column of nothing between two glyphs about eight wide, and it comes back
/// every time the fraction accumulates a half. Verified by tessellating a run of six and reading the
/// vertices, not inferred.
///
/// **So the mechanism is the step and not the antialiasing**, which is the correction this record
/// carries: the first version of it blamed a half-covered pixel at every boundary and named two cells
/// of different background meeting there. Backgrounds never had it — `round_rects_to_pixels` is on by
/// default too, and its own doc says it is what makes filled rectangles tile perfectly — and the
/// reported defect was in the glyphs.
///
/// Measured with `Ubuntu Sans Mono` at 14.7 points, which is what a cell of `8.232 × 17.625` pixels
/// comes out of: the drift is `0.232` of a pixel per column and `0.625` per row, so a whole pixel
/// accumulates every **4.3** columns and every **1.6** rows.
///
/// **Down and never up**, and that is the half that is not obvious: a cell wider than the glyph's own
/// advance makes a run of `─` stop touching, which is the gap this exists to close. Going down leaves
/// the cell a fraction narrower than the advance, and consecutive glyphs overlap by that fraction
/// instead of parting.
///
/// **Nearest is not good enough for that, and it was what this shipped first.** Rounding to the
/// nearest goes up whenever the fraction is at least a half — with the seventeen-point body it would
/// take a cell of `9.52` pixels to ten, which is `0.48` of a gap at every column boundary. A guard
/// asserting only «the cell is a whole number of pixels» is satisfied by all three of `floor`, `round`
/// and `ceil`; what pins this down is that the cell is never **larger** than what the font measured.
///
/// The floor of one pixel is there because a cell of zero makes a grid of no size, and it is the one
/// case where the result may exceed the measurement: a font under a pixel has no honest cell at all.
/// **D34** carries the measurements, the discarded alternatives and what is not measured.
fn on_whole_pixels(points: f32, per_point: f32) -> f32 {
    ((points * per_point).floor().max(1.0)) / per_point
}

/// How much a cell takes up on screen.
#[derive(Debug, Clone)]
pub struct CellMetrics {
    /// Private together: the width and the height are measurements **of this font**. With the three
    /// fields open, changing just one leaves a metric that measures one font and paints with another,
    /// and the grid goes out of line without anything failing — the same reason the cell view keeps
    /// its size private.
    width: f32,
    height: f32,
    font: FontId,
    /// How many device pixels a point is worth, kept so the grid can be pinned to whole ones. It
    /// travels with the two sizes because it is what they were rounded by: rounded with one scale and
    /// placed with another, the boundaries land back on fractions.
    per_point: f32,
}

impl CellMetrics {
    /// Asks the font how much a cell measures.
    ///
    /// The width is taken from any glyph **of fixed width**: in a monospaced font they all measure
    /// the same, so asking about one stands for all. If some day a font that is not monospaced is
    /// admitted, this is the place where it will break — and breaking here is better than going out
    /// of line on screen without warning.
    ///
    /// **It is asked for on every pass, not cached.** When the desktop zoom changes the font changes
    /// size, and a cached measurement would throw the whole grid out of line without warning.
    pub fn measure(ctx: &Context, font: FontId) -> Self {
        let (width, narrow, height) = ctx.fonts_mut(|fonts| {
            (
                fonts.glyph_width(&font, 'M'),
                fonts.glyph_width(&font, 'i'),
                fonts.row_height(&font),
            )
        });
        if the_letters_do_not_all_measure_the_same(width, narrow) {
            // **Said once and not per pass**: this runs on every frame, and a line per frame would
            // bury the error output of a program that has done nothing wrong.
            SAID_THE_LETTERS_DIFFER.call_once(|| {
                eprintln!(
                    "agent-terminal: the font in use is not of fixed width, so the columns will not \
                     line up: every cell keeps the width of an 'M' and each letter carries its own."
                );
            });
        }
        let per_point = ctx.pixels_per_point();
        Self {
            width: on_whole_pixels(width, per_point),
            height: on_whole_pixels(height, per_point),
            font,
            per_point,
        }
    }

    /// A metric with the two sizes written out, **for a test whose subject is arithmetic**.
    ///
    /// Measuring for real needs a context, and a module that only does rectangle arithmetic would
    /// have to open a window to check a sum. This is the same helper `painter`'s own tests already
    /// built by hand with the fields in front of them; it exists so that a sibling module can have
    /// one too without the fields being opened — they stay private together for the reason the
    /// struct gives.
    ///
    /// **One device pixel to the point**, so the snapping neither moves a corner nor a size and the
    /// test's arithmetic reads as it is written.
    #[cfg(test)]
    pub(crate) fn of(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            font: FontId::monospace(14.0),
            per_point: 1.0,
        }
    }

    /// How wide a cell is.
    ///
    /// It can be read, but not set: whoever measures a grid needs this number —to place two consoles
    /// side by side, or to know whether they fit in the window— and computing it on their own would
    /// be exactly what this crate avoids, measuring with one font and painting with another.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// How tall a cell is. It is read for the same reason as [`CellMetrics::width`].
    pub fn height(&self) -> f32 {
        self.height
    }

    /// The font it was measured with, for whoever paints text that has to line up with this grid.
    ///
    /// It is cloned rather than borrowed because `Painter::text` consumes it, and a `FontId` is two
    /// fields. It still cannot be set: measuring with one and painting with another is what this
    /// crate exists to prevent.
    pub fn font(&self) -> FontId {
        self.font.clone()
    }

    /// How many whole cells fit in the given space.
    ///
    /// **Never fewer than one row and one column, and never more than [`CEILING`].** The floor is
    /// there because `Screen::resize` panics with zero columns: without it, dragging the window edge
    /// until it is narrow would kill the application. The ceiling is there because the available
    /// space is not always that of a real window — while the desktop has not yet said how big it is,
    /// egui answers with its reservation rectangle, which is ten thousand points on a side. Without a
    /// ceiling, that is hundreds of thousands of reserved cells and an absurd size notice to the
    /// program inside.
    pub fn cells_in(&self, space: Vec2) -> Size {
        // Impossible sizes —negative, or with no number while the window is being created— fall to
        // zero when converted to an integer, and the floor lifts them to one.
        Size {
            rows: ((space.y / self.height) as u16).clamp(1, CEILING.rows),
            cols: ((space.x / self.width) as u16).clamp(1, CEILING.cols),
        }
    }

    /// What one device pixel is worth in points, for whoever has to keep a shape from rounding away
    /// to nothing.
    pub fn a_pixel(&self) -> f32 {
        1.0 / self.per_point
    }

    /// The rectangle the cell of a given row and column occupies, from the given origin.
    ///
    /// **The origin is used as it arrives, fraction and all**, and that is worth a line because the
    /// first version of this snapped it and a test demanded it. It buys nothing: the tessellator
    /// rounds each glyph's origin and each rectangle to the pixel grid itself, and a fraction shared
    /// by every cell of the pane survives that rounding as the same offset for all of them. What has
    /// to be whole is the **step** — see [`on_whole_pixels`] — not where the run starts.
    fn cell(&self, origin: Pos2, row: u16, column: u16) -> Rect {
        let corner = origin + vec2(column as f32 * self.width, row as f32 * self.height);
        Rect::from_min_size(corner, vec2(self.width, self.height))
    }
}

/// Paints the whole view at the indicated spot.
///
/// **Precondition: the surface must already be painted with the theme background.** Cells that carry
/// that same background are not painted —they would be nearly two thousand rectangles per pass to
/// change nothing, and with antialiasing on, one boundary per cell where a seam can appear—, so on
/// an uncleared surface the window's gap would show between the letters.
///
/// The order matters and is always the same: **first all the backgrounds, then all the glyphs and
/// the cursor last**. Painting each cell whole at once would leave one cell's background on top of
/// the previous glyph as soon as a glyph spilled out of its slot — which is precisely what is not
/// clipped here, so it can be seen.
pub fn paint(
    painter: &Painter,
    view: &View,
    theme: &Theme,
    metrics: &CellMetrics,
    origin: Pos2,
    discoverer: &mut crate::fonts::Discoverer,
    ink: CursorInk,
) -> bool {
    let grid_size = view.size();
    // The default background is the same for the whole screen: asking for it once per cell would be
    // nearly two thousand queries per pass to always get the same thing.
    let window_background = theme.background();

    // The whole grid is walked in one-row chunks. The view is dense by contract —there are exactly
    // rows-by-columns cells— so this cannot leave any out, and there is no impossible case to pretend
    // to handle.
    let width = grid_size.cols as usize;
    for (row, cells) in view.cells().chunks_exact(width).enumerate() {
        let row = row as u16;
        for (column, cell) in cells.iter().enumerate() {
            let (_, background) = colors_of(cell, theme);
            // The whole window already paints the default background: repeating it cell by cell is
            // work for nothing and, with antialiasing on, a seam at every boundary.
            if background != window_background {
                painter.rect_filled(metrics.cell(origin, row, column as u16), 0.0, background);
            }
        }
    }

    // **The glyphs are painted in TWO passes —first the text ones, then the color ones— and that is
    // not organization: it is what decides how much drawing the frame costs.** The engine can only
    // batch into the same draw call shapes that share a texture; emojis live in their atlas and the
    // letters in the font one, so painting them intermingled in grid order splits the batch **at every
    // emoji**. See **D25**, which brings the measurement.
    //
    // **The cost of the extra pass is walking the grid one more time**, a few microseconds: the color
    // one discards with a comparison everything that does not go to color, which is nearly everything.
    // **The glyphs are painted in TWO passes —first the text ones, then the color ones— and that is
    // not organization: it is what decides how much drawing the frame costs.** The engine can only
    // batch into the same draw call shapes that share a texture; emojis live in their atlas and the
    // letters in the font one, so painting them intermingled in grid order splits the batch **at every
    // emoji**. See **D25**, which brings the measurement.
    //
    // **The cost of the extra pass is walking the grid one more time**, a few microseconds: the color
    // one discards with a comparison everything that does not go to color, which is nearly everything.
    // **And the second pass only happens if the first saw any emoji.** A screen without a single one
    // —which is the vast majority— would walk the whole grid to paint nothing. With ASCII the shortcut
    // below would leave it at almost nothing anyway, but with Japanese on screen no shortcut helps:
    // each cell would consult the Unicode tables a second time.
    //
    // **What it saves is below what the bench can tell apart, and it is stated that way**: two runs of
    // the same batch with Japanese give 280 and 301 µs in the painting stretch, so any difference
    // smaller than those 21 µs cannot be asserted. It is done for the shape —not walking the grid to
    // paint nothing— and not because there is a number behind it.
    let mut there_was_color = false;
    for pass in [Presentation::Text, Presentation::Color] {
        if pass == Presentation::Color && !there_was_color {
            break;
        }
        for (row, cells) in view.cells().chunks_exact(width).enumerate() {
            let row = row as u16;
            for (column, cell) in cells.iter().enumerate() {
                // **Noting is the only thing that runs per cell, and that is why it has to be cheap**:
                // an ASCII or already-resolved character is discarded without touching anything.
                // Finding the font and registering it happen outside this pass — see
                // `Discoverer::resolve`. It runs only on the first of the two: noting the same thing
                // twice changes nothing, but would be paid for twice.
                // **A block element is noted like anything else**, even though no font is asked for it.
                // Skipping it here looks like the obvious saving and it buys nothing: a character the
                // loaded faces already cover is discarded by `resolve`'s oracle before the machine is
                // touched, and the factory faces cover this range whatever family the settings name.
                // The scenario the skip defends against does not exist, and the check would run on
                // every cell of the grid. **D35** carries the measurement it was discarded on.
                if pass == Presentation::Text {
                    discoverer.note(cell.c);
                }
                // The companion of a wide character **paints nothing**. Its first column draws the whole
                // glyph and spills over this one; painting it again here would duplicate it, half on top
                // of the other. And the gap left by a wide character that did not fit at the end of the
                // row is not a space either: it is a spot where there is no character.
                if matches!(cell.width, CellWidth::WideTail | CellWidth::WrapFiller) {
                    continue;
                }
                // A space has nothing to draw, and they are most of a screen.
                if cell.c == ' ' || cell.hidden {
                    continue;
                }
                let attached = view.combining_of(cell);
                // **An ASCII WITH NOTHING ATTACHED is discarded from the color pass without consulting
                // the Unicode tables.** It is the vast majority of what passes through a terminal screen.
                //
                // **What this shortcut saves is NOT distinguishable from the noise between runs** —366 µs
                // against 377 in the painting stretch— and it is stated that way rather than attributing
                // to it a number that is not its own: the 92 µs the extra pass costs are paid by **walking
                // the grid again**, and the shortcut does not avoid that. What it does avoid is consulting
                // the Unicode tables in almost every cell, which is an argument of shape and not a
                // measurement.
                //
                // **The "with nothing attached" part is not prudence, it is mandatory**: `#`, `*` and the
                // digits have `Emoji=Yes`, so `#` followed by the selector is `#️⃣` and goes to color. The
                // first version of this shortcut discarded all ASCII and ate them; it was uncovered by the
                // test that walks the 128 with both selectors behind, written precisely because
                // duplicating here a rule that lives in `fonts` is only acceptable if something warns when
                // they stop saying the same thing.
                if pass == Presentation::Color && cell.c.is_ascii() && attached.is_empty() {
                    continue;
                }
                // Each cell is painted in the pass its presentation calls for, and not in the other.
                if crate::fonts::presentation_of(cell.c, attached) != pass {
                    // If in the text pass a cell asks for color, the color pass has work to do.
                    there_was_color |= pass == Presentation::Text;
                    continue;
                }
                paint_character(
                    painter,
                    cell.c,
                    attached,
                    metrics.cell(origin, row, column as u16),
                    cell.width == CellWidth::WideHead,
                    colors_of(cell, theme).0,
                    metrics,
                    discoverer,
                );
            }
        }
    }

    paint_cursor(painter, view, theme, metrics, origin, discoverer, ink)
}

/// The two colors of a cell, with the intensity already applied where it belongs.
///
/// **Bold and dim describe the color the program declared as foreground**, and with inverse video
/// that color is the one traveling in the background — the engine delivers it swapped and marks that
/// it is. Always applying the intensity to the foreground seems natural and is the mistake: it dims a
/// color that is no longer seen and leaves the other at full intensity.
fn colors_of(cell: &ViewCell, theme: &Theme) -> (Color32, Color32) {
    let (foreground, background) = if cell.inverse {
        (
            theme.color(cell.fg),
            theme.with_attributes(cell.bg, cell.bold, cell.dim),
        )
    } else {
        (
            theme.with_attributes(cell.fg, cell.bold, cell.dim),
            theme.color(cell.bg),
        )
    };

    // **A selection is highlighted by swapping its two colors, and not with a new theme color.** It
    // is what any terminal does, and the reason is that **it always reads**: a cell's two colors were
    // already legible one over the other, so swapping them cannot leave text illegible. A highlight
    // color of its own can — and it would do so right where the program chose odd colors, which is
    // where no one would test it.
    //
    // **It goes after inverse video and not before**, so on an already-inverted cell the highlight
    // returns it to its original colors. That is correct and what is expected: selecting over `vim`'s
    // cursor shows it like the rest of the text instead of leaving it indistinguishable.
    if cell.selected {
        (background, foreground)
    } else {
        (foreground, background)
    }
}

/// Paints a character in its slot: in color if color is called for, and via the text path if not.
///
/// **It exists as a separate function because the block cursor repaints the character it covers**, and
/// without this that repaint would not know about emojis: an emoji under the cursor would come out
/// monochrome or not come out at all.
#[expect(
    clippy::too_many_arguments,
    reason = "    These are the data needed to paint a character, and none is derived from another: the character, what \
    is attached to it —which decides the presentation—, where it goes, whether it takes two columns, what color if it \
    is text, at what size, who knows about fonts and where the color glyphs are remembered. Grouping them into \
    a type would be a struct that means nothing outside this call."
)]
fn paint_character(
    painter: &Painter,
    character: char,
    attached: &[char],
    slot: Rect,
    is_wide: bool,
    color: Color32,
    metrics: &CellMetrics,
    discoverer: &mut crate::fonts::Discoverer,
) {
    // **A block element is drawn here and never asked of the font.** It is not a letter: its whole job
    // is to fill its cell and meet its neighbour, which a glyph drawn for another size cannot do. See
    // `blocks`, which carries the reason and what was left out.
    if let Some(filling) = blocks::filling_of(character) {
        paint_a_block(painter, &filling, slot, color, metrics);
        return;
    }
    // **A wide emoji occupies the TWO columns the engine reserved for it**, not one. The slot that
    // arrives is that of its first column; the drawing is centered on the two, which is what its width
    // declares.
    let its_own = if is_wide {
        Rect::from_min_max(slot.min, slot.max + vec2(metrics.width, 0.0))
    } else {
        slot
    };
    if crate::fonts::presentation_of(character, attached) == crate::fonts::Presentation::Color
        && paint_in_color(painter, character, its_own, discoverer)
    {
        return;
    }
    // **The glyph is not clipped to its cell.** If the font paints wider than the parser reserved, it
    // has to be seen: it is the datum that decides whether a font of our own has to be shipped.
    // Clipping it would hide the mismatch and turn it into a far more expensive surprise.
    //
    // **The family is chosen per character, and that is the only place where it can be chosen.** egui
    // orders its fallback chain by whole family, not by range: for the icon font the user installed to
    // win in the private-use areas **without** taking the letters away from the terminal's, it has to
    // live in its own family and be requested only where it belongs. What walks cell by cell is this.
    let font = eframe::egui::FontId::new(metrics.font.size, discoverer.family_of(character));
    painter.text(slot.left_top(), Align2::LEFT_TOP, character, font, color);
}

/// One axis of a block's part: never thinner than a pixel, and never past the cell's far edge.
///
/// **A part that rounds away to nothing is not a thinner mark, it is no mark at all** — for `▏` used
/// as a rule, the rule is simply gone — so it is given a pixel. **And the pixel is taken from inside
/// its own cell**: where the far edge leaves no room, the part moves back against that edge instead of
/// growing over the neighbour, which for a cell a few pixels tall would be a mark in the wrong row.
///
/// **The whole pixel has a second job the reasoning above does not reach**, found by reading the
/// tessellator: a rectangle thinner than `0.9` of a feather takes epaint's `thin_line` branch
/// (`tessellator.rs:1008`), which draws **no end caps** — its own `TODO` says so — and would give the
/// part a feathered ramp of its own at both ends. A whole device pixel is exactly one feather, so the
/// branch is never reached. A part rounded to a fraction would land in it.
fn at_least_a_pixel(low: f32, high: f32, edge: f32, pixel: f32) -> (f32, f32) {
    if high - low >= pixel {
        (low, high)
    } else if edge - low >= pixel {
        (low, low + pixel)
    } else {
        (edge - pixel, edge)
    }
}

/// Draws a block element to the exact measure of its cell.
///
/// **The sides are handed over as the fractions give them, and the tessellator rounds them.** Doing
/// it here as well was in the first version of this and it was taken out: epaint aligns every filled
/// rectangle to the pixel grid on its own —`round_rects_to_pixels`, on by default, whose own doc says
/// it is what makes filled rectangles tile perfectly— so two parts that share a boundary land on the
/// same pixel either way, and a rounding of our own only adds a place for the two to disagree.
///
/// **What is NOT the tessellator's to fix is a part that has collapsed** — see [`at_least_a_pixel`],
/// which also says where its pixel is taken from and which branch of epaint it keeps the part out of.
///
/// **A block takes one column and is not widened to two.** `is_wide` is not consulted here because
/// every character of the range is width 1 — which is a fact of `unicode-width`, reached through
/// `alacritty_terminal`, and not of this repository. If a future table ever widened one, its tail
/// column paints nothing by design and half the drawing would be missing.
fn paint_a_block(
    painter: &Painter,
    filling: &blocks::Filling,
    slot: Rect,
    color: Color32,
    metrics: &CellMetrics,
) {
    // The shades ask for a fraction of the ink; everything else takes all of it.
    let ink = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), filling.alpha());
    let pixel = metrics.a_pixel();
    for &(left, top, right, bottom) in filling.parts() {
        let (left, right) = at_least_a_pixel(
            slot.left() + left * slot.width(),
            slot.left() + right * slot.width(),
            slot.right(),
            pixel,
        );
        let (top, bottom) = at_least_a_pixel(
            slot.top() + top * slot.height(),
            slot.top() + bottom * slot.height(),
            slot.bottom(),
            pixel,
        );
        painter.rect_filled(
            Rect::from_min_max(pos2(left, top), pos2(right, bottom)),
            0.0,
            ink,
        );
    }
}

/// Paints that character's color glyph, centered in its slot. Says whether it painted anything.
///
/// **Returning `false` is not a failure: it is Contract 5 of the design.** A character whose
/// presentation asks for color but that no font on the machine paints in color is drawn with whatever
/// the engine gives —its monochrome silhouette, or its tofu box— and nothing more is attempted. The
/// terminal does not draw glyphs the machine does not have.
fn paint_in_color(
    painter: &Painter,
    character: char,
    slot: Rect,
    discoverer: &mut crate::fonts::Discoverer,
) -> bool {
    // **In PHYSICAL pixels, not in points.** The texture is uploaded at the size it will be seen at:
    // asking for it in points would leave it at half resolution on a screen at 200 % and the emoji
    // would come out blurry right where it shows most.
    let per_point = painter.ctx().pixels_per_point();
    let height = (slot.height() * per_point).round() as u32;
    let width = (slot.width() * per_point).round() as u32;
    let Some((atlas, uv, measurements)) =
        discoverer.color_glyph(painter.ctx(), character, height, width)
    else {
        return false;
    };
    // **Centered in its slot, and that is why the real texture size is needed**: the rescaling keeps
    // the drawing's proportion, so it almost never fills the exact slot — there is room to spare on one
    // axis. Stretching it to fill would deform the emoji.
    let drawing = Rect::from_center_size(slot.center(), measurements / per_point);
    painter.image(atlas, drawing, uv, Color32::WHITE);
    true
}

/// How long each half of a blink lasts.
///
/// **1.2 s for the whole cycle, which is the desktop's own number here**:
/// `gsettings get org.gnome.desktop.interface cursor-blink-time` answers `1200`, described as the
/// cycle, so a terminal on this machine is in step with every other window on it. WezTerm's 800 ms
/// is the same order. Reading that setting at runtime is what **D37** refuses, and for reasons this
/// repository can check rather than cite: this application links neither GTK nor gsettings, the
/// alternative is spawning a process on the start-up path — the one cost **D26** lets only the
/// font-by-name setting pay — and it is a Linux-only answer to a question **F2.7** is about to ask
/// on two other platforms.
const HALF_A_BLINK: Duration = Duration::from_millis(600);

/// How long after the last keystroke the blink gives up, leaving the cursor **painted**.
///
/// **This is what keeps «the window is at rest» true**, which is the premise every published
/// measurement batch rests on (**D13**). This is a terminal for hosting agents: its panes are left
/// open for hours with an editor sitting in insert mode, which is exactly the state that asks for a
/// blink. Without this, one such pane keeps the window repainting at 1.7 Hz for as long as the
/// machine is on. kitty ships `cursor_stop_blinking_after 15.0` on by default, with the same
/// reasoning; it is adopted rather than invented.
const A_BLINK_STOPS_AFTER: Duration = Duration::from_secs(15);

/// What the window knows about the blink when it hands a console over to be painted.
///
/// **It travels as one value and not as two arguments** because the two are one question — is this
/// cursor lit right now — and because the window's core is under a ratchet with no room for the
/// second argument (**D28**).
#[derive(Clone, Copy)]
pub struct Blink {
    /// What the settings file settles over the program's head, `None` leaving it to the program.
    pub ceiling: Option<bool>,
    /// How long since a keystroke last reached a console.
    ///
    /// **The anchor is a keystroke and not the process's start**, so a cursor is lit the instant you
    /// type instead of at whatever point of the cycle you caught it. Every terminal does this, and
    /// it is why typing continuously never leaves the cursor half-drawn.
    pub since_a_key: Duration,
    /// Whether **this** console has the keyboard in a window that has the desktop's focus.
    ///
    /// **The only per-console piece, and it rides here rather than as an argument of its own**: a
    /// console is painted through seven parameters already, and an eighth is where a reader stops
    /// being able to tell which `bool` is which.
    pub focused: bool,
}

/// The three things the frame resolves before it walks the panes, travelling together.
///
/// **It is an argument count and it is said so**, rather than dressed up as a concept: painting a
/// console reached eight arguments with the strip's decision, and the lint that guards this crate
/// stops at seven. Riding them in one value is the same answer [`Blink::focused`] already gave one
/// level down, for the same reason its docblock gives — an eighth argument is where a reader stops
/// being able to tell which value is which.
///
/// **None of the three is purely the session's**: the strip's decision is, the blink arrives
/// **narrowed per console** with [`Blink::on`] because whether the cursor is lit depends on which
/// console has the keyboard, and the button depends on how many consoles the tab being painted has.
#[derive(Clone, Copy)]
pub(crate) struct HowToPaint {
    /// Whether this console's cursor is lit, and for how much longer.
    pub(crate) blink: Blink,
    /// Whether a strip is painted over it. See [`crate::header::Headers`].
    pub(crate) headers: crate::header::Headers,
    /// Whether that strip carries the button that closes it. See [`crate::header::Button`].
    pub(crate) button: crate::header::Button,
    /// Whether batch W needs painted points for its injector.
    pub(crate) measure_links: bool,
}

impl HowToPaint {
    /// The three of them, in the order the frame resolves them.
    ///
    /// **It exists to keep the call site one line long**, which is not a style preference here:
    /// `lib.rs` is a ratchet that may only shrink (**D28**), and the literal form of this value
    /// costs that file three lines for nothing. The tests build it as a literal, which is fine —
    /// they are not counted.
    /// **The third arrives as the consoles of the tab being painted**, which is what the caller is
    /// holding — it is walking their rectangles — and what [`crate::header::Button::in_a_tab_of`]
    /// turns into the answer. Handing over the list rather than the verdict is what keeps the one
    /// production call site inside the line that `lib.rs`'s ratchet pays for; handing over the list
    /// rather than its **length** is what stops another `usize` in scope reaching this argument.
    pub(crate) fn of(
        blink: Blink,
        headers: crate::header::Headers,
        consoles: &[eframe::egui::Rect],
    ) -> Self {
        Self {
            blink,
            headers,
            button: crate::header::Button::in_a_tab_of(consoles),
            measure_links: false,
        }
    }

    pub(crate) fn measuring_links(mut self, yes: bool) -> Self {
        self.measure_links = yes;
        self
    }
}

impl Blink {
    /// Whether the cursor is lit, and how long until it flips. `None` once the blink has stopped.
    ///
    /// **The deadline is clamped to the stop, and with today's constants that clamp never binds** —
    /// `A_BLINK_STOPS_AFTER` is exactly twenty-five halves of `HALF_A_BLINK`, so the last flip lands
    /// on the stop rather than past it. It is written as a constraint and not as a mechanism doing
    /// something: it keeps this function correct for constants that do not divide, and the test
    /// below that pins the two constants dividing evenly fails the day either one moves, which is
    /// when the clamp starts mattering.
    fn phase(self) -> Option<(bool, Duration)> {
        // **Exactly fifteen seconds is already stopped**, not one last dark half: `checked_sub`
        // answers `Some(0)` on the boundary, and a deadline of zero is a frame asked for right
        // now, for ever.
        let left = A_BLINK_STOPS_AFTER
            .checked_sub(self.since_a_key)
            .filter(|left| !left.is_zero())?;
        let half = HALF_A_BLINK.as_millis();
        let elapsed = self.since_a_key.as_millis();
        let lit = (elapsed / half).is_multiple_of(2);
        let until_it_flips = HALF_A_BLINK - Duration::from_millis((elapsed % half) as u64);
        Some((lit, until_it_flips.min(left)))
    }

    /// The same blink, said of one console.
    pub(crate) fn on(self, focused: bool) -> Self {
        Self { focused, ..self }
    }

    /// The ink a console's cursor is drawn with this frame.
    ///
    /// `asked_for` is what the program wants; the ceiling overrules it in both directions, which is
    /// what makes `cursor.blink` a ceiling instead of a default a program can climb over (**D37**).
    pub(crate) fn ink(self, asked_for: bool) -> CursorInk {
        if !self.focused {
            return CursorInk::Outline;
        }
        match self.ceiling.unwrap_or(asked_for).then(|| self.phase()) {
            Some(Some((lit, _))) => CursorInk::Blinking { lit },
            // Not blinking at all, or blinking that has stopped: either way it is painted and
            // nothing is due.
            _ => CursorInk::Filled,
        }
    }

    /// How long until the window has to wake up to flip a cursor, or nothing if none is blinking.
    ///
    /// **The caller decides whether anything is blinking, and it decides it from what the painting
    /// drew.** Asking the engine instead would arm this for a cursor nobody can see: that bit is
    /// independent of `SHOW_CURSOR`, so a full-screen program — every one of them sends `CSI ?25l` —
    /// reports a blink it never shows, and the window would wake at 1.7 Hz to paint nothing.
    pub(crate) fn until_it_flips(self) -> Option<Duration> {
        self.phase().map(|(_, left)| left)
    }
}

/// What a console's cursor is drawn as, this frame.
///
/// **«Is this cursor in a blink» is one variant and not a payload spread over two**, so the
/// contradiction «solid, not blinking, and still owed a repaint» cannot be written down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CursorInk {
    /// Solid, and not blinking at all.
    Filled,
    /// In a blink, `lit` saying which half. **The dark half draws nothing and still owes a
    /// repaint**, which is why it is this variant and not a fourth: answering «no blink here» while
    /// the cursor is merely dark would disarm the deadline on every other flip and the blink would
    /// stop after one. It is **reachable only through a blink** — a console paints no cursor for
    /// three other reasons (no cursor in the view, a shape of `Hidden`, no cell where it stands) and
    /// all three are decided before this is looked at.
    Blinking { lit: bool },
    /// The outline every console that does not have the keyboard draws, and every console at all
    /// when the window has lost the desktop's focus (**D36**).
    Outline,
}

fn paint_cursor(
    painter: &Painter,
    view: &View,
    theme: &Theme,
    metrics: &CellMetrics,
    origin: Pos2,
    discoverer: &mut crate::fonts::Discoverer,
    ink: CursorInk,
) -> bool {
    // **With no cursor, no cursor is painted**, and since there is history that case really happens:
    // scrolling up far enough, the cursor stays in the active area, below what is being looked at. The
    // snapshot says so instead of clamping it, which is what this spot announced since F1.3.
    //
    // **And the guard below did not cover this**, even though its comment said it did: the row arrived
    // clamped, so it was always inside the grid and `cell` never returned anything. A phantom cursor
    // would have been painted in the last row.
    let Some(cursor) = view.cursor() else {
        return false;
    };
    if cursor.shape == CursorShape::Hidden {
        return false;
    }
    let Some(under_cursor) = view.cell(cursor.row, cursor.col) else {
        return false;
    };
    let mut slot = metrics.cell(origin, cursor.row, cursor.col);
    // Over a wide character, the cursor covers **the two columns** that character occupies. Covering
    // only one leaves half the letter covered and the other half not, which reads as a drawing error.
    if under_cursor.width == CellWidth::WideHead {
        slot.max.x += metrics.width;
    }
    let color = theme.color(TermColor::Cursor);
    // What an unfocused console paints and the alternatives discarded for it are **D36**; that this
    // one can blink at all, and on whose word, is **D37**.
    match ink {
        // The outline is asked for **before** the shape is looked at, so a console that is not being
        // typed into paints the same mark whatever its program asked for. Kitty defaults its
        // `cursor_shape_unfocused` to `hollow` and WezTerm applies only the block outline to
        // inactive panes: this follows them rather than deciding it again.
        CursorInk::Outline => {
            painter.rect_stroke(slot, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
            return false;
        }
        // The dark half: nothing is drawn, and the answer is still «this console is blinking».
        CursorInk::Blinking { lit: false } => return true,
        CursorInk::Filled | CursorInk::Blinking { lit: true } => {}
    }
    match cursor.shape {
        CursorShape::Block => {
            painter.rect_filled(slot, 0.0, color);
            // The block is opaque and goes after the glyphs, so it eats the character underneath. It is
            // repainted on top, in the background color: it is what a terminal does, and without this
            // the cursor **erases** the letter it sits on instead of highlighting it.
            if under_cursor.c != ' ' && !under_cursor.hidden {
                // **Via the same path as the normal painting, and not with a `painter.text` of its
                // own.** Without this, the repaint would know neither about the icon family —a Powerline
                // separator changed symbol when the cursor passed over it, a review found it— nor about
                // the color emojis, which would come out monochrome only under the cursor.
                //
                // **And the emoji is repainted in color as is, not tinted with the background color.** A
                // color drawing cannot be recolored to a single tone without leaving it a flat smear:
                // what is seen on top of the block is the emoji, which is what the other terminals do and
                // the only thing that reads.
                paint_character(
                    painter,
                    under_cursor.c,
                    view.combining_of(under_cursor),
                    metrics.cell(origin, cursor.row, cursor.col),
                    under_cursor.width == CellWidth::WideHead,
                    colors_of(under_cursor, theme).1,
                    metrics,
                    discoverer,
                );
            }
        }
        CursorShape::HollowBlock => {
            painter.rect_stroke(slot, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
        }
        CursorShape::Underline => {
            let height = (metrics.height * 0.1).max(1.0);
            let bar = Rect::from_min_size(
                slot.left_bottom() - vec2(0.0, height),
                vec2(metrics.width, height),
            );
            painter.rect_filled(bar, 0.0, color);
        }
        CursorShape::Beam => {
            let width = (metrics.width * 0.15).max(1.0);
            let bar = Rect::from_min_size(slot.left_top(), vec2(width, metrics.height));
            painter.rect_filled(bar, 0.0, color);
        }
        CursorShape::Hidden => {}
    }
    matches!(ink, CursorInk::Blinking { .. })
}

/// Paints a console whole: its grid, and over it whatever the **application** has to say.
///
/// **It exists so that `paint` can stay what it is** — a pure function from a view and a corner —
/// which is what the measurement bench and this module's own tests call. When it was written, a
/// second entry was cheaper than an argument on `paint` because `lib.rs` sat against **D28**'s
/// recorded number and the call site had to keep exactly the shape it had; that constraint went with
/// the delivery that moved [`paint_pane`] into this module, and `paint` does take the focus.
#[cfg(test)]
pub(crate) fn paint_a_console(
    painter: &Painter,
    console: &crate::Pane,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
    discoverer: &mut crate::fonts::Discoverer,
    ink: CursorInk,
) -> bool {
    let view = console.screen.view();
    let blinking = paint_a_console_view(painter, &view, theme, metrics, area, discoverer, ink);
    notices::paint_them_over_it(painter, console, theme, metrics, area);
    blinking
}

/// Paints from a snapshot the caller can retain as the authority for later pointer effects.
fn paint_a_console_view(
    painter: &Painter,
    view: &term_core::View,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
    discoverer: &mut crate::fonts::Discoverer,
    ink: CursorInk,
) -> bool {
    paint(
        painter,
        view,
        theme,
        metrics,
        area.left_top(),
        discoverer,
        ink,
    )
}

/// Paints a console clipped to its rectangle, and returns what queueing the painting cost.
///
/// **The clip is not cosmetic.** The painter deliberately does not clip the glyphs —so that a
/// character wider than its cell is seen sticking out— and without clipping per console that
/// glyph on the right-hand edge would appear inside the neighboring console, painted by a
/// program that is not its own. It returns what the window needs to know about this pane after
/// painting it. See [`crate::PaneOutcome`].
pub(crate) fn paint_pane(
    ui: &eframe::egui::Ui,
    pane: &mut crate::Pane,
    rect: eframe::egui::Rect,
    theme: &Theme,
    metrics: &CellMetrics,
    discoverer: &mut crate::fonts::Discoverer,
    how: HowToPaint,
) -> crate::PaneOutcome {
    let HowToPaint {
        blink,
        headers,
        button,
        measure_links,
    } = how;
    // **The panel is timed apart from the console's queueing, with two clocks that are added
    // together: one around reading and tiling, and another around painting it.** They are two
    // and not one because the console's painting falls right in between and has a span of its
    // own. For the frame's subtraction it would make no difference to add it all up —what
    // matters there is that it be declared—, but a span called «queueing the painting» that
    // also included reading a file off the disk is a number with the label changed, and in this
    // project that has already invalidated a whole measurement.
    //
    // **Rereading goes before tiling**, because how much width the console has left depends on
    // whether there is a panel or not: a console whose producer has just published for the
    // first time narrows in this very frame, not in the next one.
    let refreshed = pane.refresh_its_panel(ui.ctx());
    // **The clock starts AFTER the refresh and what the refresh cost is added back below**, rather
    // than spanning it and subtracting the channel's share out. The boundary of the «panel» span is
    // unchanged — it is still the read plus the tiling plus the border — but the control channel is
    // now never inside any clock this span is built from, so there is no subtraction for a later
    // caller to forget. It was written the other way first, and mutating it proved nothing failed.
    let before_the_panel = std::time::Instant::now();
    // A single clock for tiling and for painting: if the panel were to age between the two
    // questions, a strip would be reserved for a warning that never gets painted.
    let now = std::time::SystemTime::now();
    // **The strip is cut here and the panel is tiled over what is left**, so the console's
    // rectangle comes out of one split and not out of two agreeing by accident. See
    // [`crate::Pane::chrome_within`].
    let chrome = pane.chrome_within(rect, metrics, now, headers);
    let mut panel_split = chrome.split();

    // **The panel's border is attended to here, before measuring the grid, and the tiling is
    // redone in this very pass.** With the gesture attended to afterwards, the frame would
    // paint the panel in the old place and —worse— would announce the old grid to its program,
    // which is the one the pumping sends it on the next frame: a double readjustment, and a
    // visible one in any full-screen program.
    //
    // **And the tree's tiling does not have to be redone.** This gesture tiles *inside* one
    // pane, not between panes: the others' rectangles never find out. That is what tells it
    // apart from dragging a divider of the tree or from dropping one console onto another,
    // which do change it.
    //
    // The timed span encloses it for the same reason it encloses reading the file and sharing
    // out the width: it is the panel's work, and what is not declared is taken by the frame's
    // subtraction — which is the interface framework cost, the figure D3 has to decide on. **It
    // is called without a panel too, and that is what makes unfolding possible**: folded there
    // is no panel rectangle, but there is a grip flush against the edge to open it again by.
    // **The width is compared before and after, and what the gesture returns is not believed.**
    // The gesture returns «I have changed something», and folding and unfolding also change
    // something — without touching the width. Since the number this datum feeds is published in
    // the report under the «effects» label and the annex reads it as tilings and layouts,
    // counting a fold in there would be publishing work that did not happen.
    let width_before = pane.panel_width;
    if pane.drag_its_panel_divider(ui, chrome.below(), panel_split.panel, metrics, now) {
        panel_split = pane.tile_with_its_panel(chrome.below(), metrics.width(), now);
    }
    let width_changed = pane.panel_width != width_before;
    let cost_of_reading = refreshed.reading + before_the_panel.elapsed();

    pane.measured = metrics.cells_in(panel_split.console.size());

    let before = std::time::Instant::now();
    // **The engine's bit is asked for here and not in the window's walk**, because reading it costs
    // one call and reading it through a `View` would cost a snapshot of the whole grid. It says what
    // the program wants, never what gets drawn: `paint_cursor` answers that.
    let view = pane.screen.view();
    let blinking = paint_a_console_view(
        &ui.painter_at(panel_split.console),
        &view,
        theme,
        metrics,
        panel_split.console,
        discoverer,
        blink.ink(pane.screen.cursor_blinks()),
    );
    notices::paint_them_over_it(
        &ui.painter_at(panel_split.console),
        pane,
        theme,
        metrics,
        panel_split.console,
    );
    let painted = crate::links::PaintedConsole {
        view,
        area: panel_split.console,
        frame: ui.ctx().cumulative_frame_nr(),
    };
    painted.disclose_hovered_link(ui, metrics);
    // **Inside the console's span on purpose**: only batch W walks this, and outside every span it
    // would be published as the framework's cost (**D13**).
    let console_links = measure_links.then(|| painted.a_point_per_source(metrics));
    pane.painted_console = Some(painted);
    // **Rasterizing a color glyph runs INSIDE the painting, so it is subtracted from the
    // painting.** It is the only expensive thing this project does in there: looking up and
    // registering a face are done outside because they rebuild the atlas, but uploading a
    // texture invalidates nothing. Without this subtraction, «queueing the painting» would also
    // measure what decoding a PNG and scaling it down cost.
    let color = discoverer.color_cost();
    let queued = before.elapsed().saturating_sub(color.spent);

    let before_painting_it = std::time::Instant::now();
    let mut buttons = std::time::Duration::ZERO;
    // **A pane whose panel is not painted says so, and it is not an afterthought.** The panel is
    // folded away, or the pane has no directory: either way nothing of it is on screen, and without
    // this the previous frame's buttons stay reachable — a click on the console, where one used to
    // be, would run its command with nothing to read.
    if panel_split.panel.is_none() || pane.panel.is_none() {
        pane.panel_state.nothing_was_drawn();
    }
    if let (Some(panel_rect), Some(dir)) = (panel_split.panel, pane.panel.as_ref()) {
        buttons = crate::panel::paint(
            &ui.painter_at(panel_rect),
            &mut pane.panel_state,
            dir.id(),
            theme,
            metrics,
            now,
            discoverer,
        );
    }
    let links = console_links
        .map(|console| {
            crate::links::ForTheBench::of(
                console,
                &pane.panel_state,
                ui.ctx().cumulative_frame_nr(),
            )
        })
        .unwrap_or_default();
    // **Subtracted OUT of the panel's span and not folded into it**, the same shape the colour
    // glyphs above already have. That span means six named things and its docblock enumerates them
    // so that nothing creeps in; letting a seventh in would be a published series quietly changing
    // meaning, which this file records having paid for twice.
    let panel_cost = cost_of_reading + before_painting_it.elapsed().saturating_sub(buttons);

    // **The strip is painted last of the pane's own painting and timed on its own** (**D13**): the
    // frame's cost is split by subtraction, so a band painted inside somebody else's span would be
    // published as that span's, and one painted inside no span at all as the framework's. It goes
    // after the console because that is the order the notices already follow — chrome the
    // application owns goes over what the program wrote, never under it.
    //
    // **And it is handed back to the window rather than put into the round below**, because the
    // round is only closed on a frame this console produced output in: see
    // [`crate::PaneOutcome::strip_cost`].
    let strip_cost = chrome
        .header()
        .map(|header| crate::header::paint(&ui.painter_at(header), header, theme, metrics, button));
    // **The border is only reported if there is a grip behind it.** With the panel clipped
    // below the grip the gesture registers no control at all —so as not to steal the point from
    // the tree's divider—, and reporting the border anyway made the injected gesture press
    // right on the edge of that divider: the batch spent its life dragging the consoles' tiling
    // **under the label of the panel's gesture**. The script invalidated it for having no
    // effects, but blaming the window.
    let panel_divider = panel_split
        .panel
        .filter(|panel_rect| panel_rect.width() > crate::PANEL_GRIP)
        .map(|panel_rect| panel_rect.left());

    // The closing of the round goes here, in what paints one console, and not in the caller's walk
    // over the panes, because **the queueing always has to be declared**, also when no output
    // reached this console: it was painted just the same, and its cost has to come out of the
    // window's «rest of the frame».
    if let Some((waiting_since, collected_at, interpreted)) = pane.pending_measurement.take() {
        pane.timings.record_round(
            crate::measure::Spans::of_a_round(
                waiting_since,
                collected_at,
                interpreted,
                queued,
                panel_cost,
            )
            .and_its_buttons(buttons)
            .and_its_control(refreshed.applying),
        );
    }

    crate::PaneOutcome {
        panel_divider,
        width_changed,
        buttons_painted: pane.panel_state.buttons_drawn().count(),
        first_button: pane.panel_state.buttons_drawn().next(),
        links,
        color,
        blinking,
        strip_cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use term_core::Screen;

    #[test]
    fn a_font_whose_letters_do_not_measure_the_same_is_caught_and_a_monospaced_one_is_not() {
        // **In a monospaced font the two advances are the SAME NUMBER**, not a close one: that is
        // what being monospaced means, and it is why this guard can be strict.
        assert!(
            !the_letters_do_not_all_measure_the_same(8.0, 8.0),
            "a monospaced font would be reported as proportional, and every start-up would say so"
        );
        // A proportional face differs by whole points: Arial at 14 measures 11.7 for the M and 3.1
        // for the i.
        assert!(
            the_letters_do_not_all_measure_the_same(11.7, 3.1),
            "a proportional font goes through unsaid, and the columns stop lining up in silence"
        );
        // **Both sides of the margin**, so it is a threshold and not a decoration: half a point over
        // a cell of eight is half a column of drift by the sixteenth character.
        assert!(the_letters_do_not_all_measure_the_same(8.0, 7.4));
        assert!(!the_letters_do_not_all_measure_the_same(8.0, 7.6));
        // **And it is a distance, not a subtraction.** Without the absolute value a face whose `i`
        // is the wider of the two —they exist: a slab serif with a serifed `i`— goes through unsaid.
        assert!(
            the_letters_do_not_all_measure_the_same(3.1, 11.7),
            "the difference is only seen in one direction, so half the proportional fonts pass"
        );
    }

    /// A metric with round measurements, so the test's arithmetic reads on its own. It deliberately
    /// does not go through the font: asking it requires a window, and what is tested here is the
    /// arithmetic.
    fn metrics(width: f32, height: f32) -> CellMetrics {
        CellMetrics {
            width,
            height,
            font: FontId::monospace(14.0),
            // One pixel to the point, so the arithmetic of the tests that use this reads as it is
            // written and the whole-pixel snapping neither moves a corner nor a size.
            per_point: 1.0,
        }
    }

    #[test]
    fn a_selection_is_painted_with_its_two_colors_swapped() {
        // **The highlight always reads because it does not invent a color**: it swaps the two the cell
        // already had, which were necessarily legible one over the other. The assertion is on the
        // **colors that come out**, not on the cell's field: painting the highlight with the same
        // background as always would leave the selection invisible and the field set the same.
        let theme = Theme::default();
        let mut cell = ViewCell {
            fg: TermColor::Indexed(1),
            bg: TermColor::Indexed(4),
            ..ViewCell::default()
        };

        let (foreground, background) = colors_of(&cell, &theme);
        cell.selected = true;
        let (selected_foreground, selected_background) = colors_of(&cell, &theme);

        assert_eq!(
            (selected_foreground, selected_background),
            (background, foreground),
            "the selection did not come out with its two colors swapped"
        );
        assert_ne!(
            selected_background, background,
            "the highlighted cell's background is the same as before: the selection would not be seen"
        );
    }

    #[test]
    fn the_highlight_swaps_the_already_dimmed_colors_and_not_the_other_way_around() {
        // **The intensity is applied first and the highlight is swapped after**, and the order shows
        // right where the program asked for dim: it is the color it declared as foreground that goes
        // dimmed, and with inverse video that color travels in the background. Swapping before applying
        // the intensity, what is dimmed would be the wrong color — the same defect a review already
        // found in this painter without the selection in the way.
        let theme = Theme::default();
        let cell = ViewCell {
            fg: TermColor::Indexed(1),
            bg: TermColor::Indexed(4),
            inverse: true,
            dim: true,
            ..ViewCell::default()
        };

        let (_, dim_background) = colors_of(&cell, &theme);
        let (selected_foreground, _) = colors_of(
            &ViewCell {
                selected: true,
                ..cell
            },
            &theme,
        );

        assert_eq!(
            selected_foreground, dim_background,
            "the highlighted cell's letter is not the color that was dimmed: the highlight was swapped \
             before applying the intensity"
        );
        assert_ne!(
            selected_foreground,
            theme.color(TermColor::Indexed(4)),
            "the letter came out at full intensity: the dim the program asked for was lost"
        );
    }

    #[test]
    fn the_space_is_divided_into_whole_cells() {
        let grid = metrics(10.0, 20.0).cells_in(vec2(805.0, 419.0));

        // Cells that do not fit whole do not count: half a column painted at the edge would be half a
        // character cut off.
        assert_eq!(grid, Size { rows: 20, cols: 80 });
    }

    #[test]
    fn the_largest_font_body_admitted_does_not_leave_an_impossible_grid() {
        // **Since the body is configurable there is a new path to this guard**, and it is not the one
        // that motivated it: before, it was only reached by narrowing the window, now asking for a large
        // font is enough. With the maximum admitted —96 points— the cell measures 58×112, so a pane of
        // a small window is left without room for a single column.
        //
        // Verified by running it: the grid comes out **1×1** and not 0×1. A one-cell console is useless
        // but alive; a zero-column one is a division by zero waiting to happen.
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();

        let mut grids = Vec::new();
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let metrics = CellMetrics::measure(ui.ctx(), eframe::egui::FontId::monospace(96.0));
            for slot in [vec2(320.0, 200.0), vec2(40.0, 30.0), vec2(0.0, 0.0)] {
                grids.push(metrics.cells_in(slot));
            }
        })
        .drop_without_applying_deltas();

        for grid in grids {
            assert!(
                grid.cols >= 1 && grid.rows >= 1,
                "with the maximum body the grid came out {}x{}: a zero-column console is a \
                 division by zero waiting to happen",
                grid.cols,
                grid.rows
            );
        }
    }

    /// Where each glyph of a painted row ends up **after the tessellator has had it**, in the order
    /// they were queued.
    ///
    /// **It has to be after.** epaint rounds every glyph's origin to a whole pixel on its own, and
    /// that rounding is the mechanism this whole change is about — a test reading `output.shapes`
    /// looks at the coordinates we asked for, which is the one place the defect never was. Each glyph
    /// is a quad of four vertices, queued in the order painted, so the left edge of the n-th is the
    /// x of vertex `4n`.
    fn glyph_edges(cell: impl Fn(&Context) -> CellMetrics, row: &str) -> Vec<f32> {
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let cell = cell(&ctx);
        let cell = &cell;
        let screen = of_text(row);
        let view = screen.view();
        let mut discoverer = crate::fonts::Discoverer::default();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("the grid"),
            ));
            paint(
                &painter,
                &view,
                &Theme::default(),
                cell,
                Pos2::ZERO,
                &mut discoverer,
                CursorInk::Filled,
            );
        });
        let meshes = ctx.tessellate(output.shapes.clone(), 1.0);
        output.drop_without_applying_deltas();
        let mut edges = Vec::new();
        for clipped in &meshes {
            if let eframe::egui::epaint::Primitive::Mesh(mesh) = &clipped.primitive {
                for quad in mesh.vertices.chunks_exact(4).take(row.chars().count()) {
                    edges.push(quad[0].pos.x);
                }
            }
        }
        edges
    }

    #[test]
    fn a_row_of_glyphs_keeps_one_step_after_the_engine_rounds_each_one_to_a_pixel() {
        // **This is the defect, and the mechanism is not the one it looks like.** The report was a run
        // of `─` opening a gap every few characters and a logo of block characters crossed by
        // hairlines. It is not antialiasing painting a boundary half: this painter puts one glyph per
        // cell, epaint rounds **each origin separately**, and over a fractional step the rounded
        // origins do not keep that step — `0, 8.232, 16.464, 24.696…` become `0, 8, 16, 25`, so the
        // steps are `8, 8, 9`. The ninth pixel is a column of nothing between two glyphs eight wide.
        //
        // **The control comes first and it is the whole test.** A row painted on a cell of `8.232`
        // has to come out with unequal steps — otherwise this machine's font or this version of the
        // engine does not have the defect at all, and the assertion below would hold with the fix
        // deleted.
        let steps = |cell: fn(&Context) -> CellMetrics| {
            glyph_edges(cell, "──────")
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .collect::<Vec<f32>>()
        };
        let all_the_same = |steps: &[f32]| steps.windows(2).all(|p| (p[1] - p[0]).abs() < 0.001);

        let fractional = steps(|_| metrics(8.232, 17.625));
        assert!(
            !fractional.is_empty() && !all_the_same(&fractional),
            "a row painted on a cell of 8.232 pixels came out with even steps {fractional:?}, so \
             this test cannot see the defect it exists for"
        );

        // **The subject goes through `measure`, not through a hand-written cell.** With the size
        // written by hand this passes with the rounding deleted, which is the whole thing it guards.
        let measured =
            steps(|ctx| CellMetrics::measure(ctx, eframe::egui::FontId::monospace(14.7)));
        assert!(
            all_the_same(&measured),
            "a row painted on the cell `measure` produced came out with steps {measured:?}: the \
             engine's per-glyph rounding is opening a column of nothing between two of them"
        );
    }

    #[test]
    fn the_cell_is_never_larger_than_what_the_font_measured() {
        // **The half a whole-pixel guard cannot see, and this shipped the wrong rounding first.**
        // «The cell is a whole number of pixels» is satisfied by `floor`, by `round` and by `ceil`
        // alike; what separates them is whether the cell can come out **bigger** than the advance the
        // engine draws the glyph with, because that is what makes a run of `─` stop touching — the very
        // gap the change exists to close.
        //
        // **Measured, which is why these bodies and not others**: with the family in use, `round` takes
        // 9.52 pixels to ten at 17 points —0.48 of a gap at every column boundary— and 7.84 to eight at
        // 14. Nearest goes up about half the time, on each axis independently.
        let bodies = [12.0f32, 14.0, 14.7, 16.0, 17.0, 18.0];
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();

        let mut measured = Vec::new();
        ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            for body in bodies {
                let font = eframe::egui::FontId::monospace(body);
                let raw = ui
                    .ctx()
                    .fonts_mut(|fonts| (fonts.glyph_width(&font, 'M'), fonts.row_height(&font)));
                let cell = CellMetrics::measure(ui.ctx(), font);
                measured.push((body, raw, (cell.width(), cell.height())));
            }
        })
        .drop_without_applying_deltas();

        // **At least one body has to round UP under nearest**, or this test would pass with `round` in
        // place and cover nothing. Which bodies do depends on the machine's font, so it is asserted
        // rather than assumed.
        assert!(
            measured.iter().any(
                |(_, (width, height), _)| (width - width.round()).abs() > 0.001
                    && width.round() > *width
                    || (height - height.round()).abs() > 0.001 && height.round() > *height
            ),
            "no body among {bodies:?} measures a fraction of at least a half on this machine, so \
             this test cannot tell nearest from down: pick bodies that do"
        );

        for (body, (raw_width, raw_height), (width, height)) in measured {
            assert!(
                width <= raw_width,
                "at {body} points the cell is {width} wide against an advance of {raw_width}: a cell \
                 wider than the advance is what opens the gap in a run of `─`"
            );
            assert!(
                height <= raw_height,
                "at {body} points the cell is {height} tall against a row height of {raw_height}, so \
                 a band no glyph covers is left at every row boundary"
            );
        }
    }

    #[test]
    fn a_window_narrower_than_a_cell_still_gives_one_column() {
        let grid = metrics(10.0, 20.0).cells_in(vec2(3.0, 4.0));

        assert_eq!(grid, Size { rows: 1, cols: 1 });
    }

    #[test]
    fn eguis_reservation_rectangle_does_not_produce_an_enormous_grid() {
        // Ten thousand points on a side is what egui answers while the desktop has not said how big the
        // window is. Without a ceiling that is 743,000 reserved cells and an absurd size notice to the
        // program inside.
        let grid = metrics(8.4, 16.0).cells_in(vec2(10_000.0, 10_000.0));

        assert_eq!(grid, CEILING);
    }

    #[test]
    fn no_space_produces_a_grid_the_screen_rejects() {
        // This is the test that ties the two pieces together: if someone removes the floor, it blows up
        // here and not in the user's hand while dragging the window.
        let mut screen = Screen::new(Size { rows: 24, cols: 80 });

        for space in [
            vec2(0.0, 0.0),
            vec2(1.0, 1.0),
            vec2(-50.0, -50.0),
            vec2(f32::NAN, f32::NAN),
            vec2(f32::INFINITY, f32::INFINITY),
            vec2(1e9, 1e9),
        ] {
            let grid = metrics(10.0, 20.0).cells_in(space);

            assert!(
                grid.rows >= 1 && grid.cols >= 1,
                "{space:?} produced a grid that blows up the screen: {grid:?}"
            );
            assert!(
                grid.rows <= CEILING.rows && grid.cols <= CEILING.cols,
                "{space:?} skipped the ceiling: {grid:?}"
            );
            screen.resize(grid);
        }
    }

    /// What `paint_cursor` leaves drawn, counted in shapes.
    ///
    /// **What is PAINTED is counted, not what the snapshot says.** That the view returns `None` is half
    /// the criterion; the other —the one checked here— is that this painter draws nothing with that
    /// `None`. It paints onto its own empty layer, so what comes out is exactly this painter's and
    /// nothing more.
    /// How many images and how many texts the painter enqueues for that screen.
    ///
    /// **It is the only oracle that says whether an emoji came out in color**, and that is why the
    /// tests here count shapes instead of looking at fields. A color emoji is an **image** in the shape
    /// list; a monochrome one is a **text**. Asserting on what the rule decided —or on whether the cache
    /// has an entry— would pass just the same with the painter returning `false` always.
    fn shapes_of(screen: &Screen) -> (usize, usize) {
        shapes_of_with(screen, &mut crate::fonts::Discoverer::default())
    }

    /// The same, but with a discoverer the test has left in the state it wants to look at.
    fn shapes_of_with(
        screen: &Screen,
        discoverer: &mut crate::fonts::Discoverer,
    ) -> (usize, usize) {
        let ctx = Context::default();
        // **An empty pass first**, because the text engine does not exist until there is one: without
        // this the first layout happens halfway through the pass being measured.
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let view = screen.view();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("the grid"),
            ));
            paint(
                &painter,
                &view,
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                discoverer,
                CursorInk::Filled,
            );
        });
        let (mut images, mut texts) = (0, 0);
        for shape in &output.shapes {
            count(&shape.shape, &mut images, &mut texts);
        }
        output.drop_without_applying_deltas();
        (images, texts)
    }

    /// **An enqueued image is a mesh with a texture that is NOT the font atlas.** `Painter::image`
    /// produces no variant of its own: it builds a two-triangle mesh with the requested texture —read in
    /// `epaint/shapes/shape.rs:373`—. Distinguishing it by its texture is what keeps any mesh the
    /// framework paints on its own from being counted as an image.
    fn count(shape: &eframe::egui::Shape, images: &mut usize, texts: &mut usize) {
        match shape {
            eframe::egui::Shape::Mesh(mesh)
                if mesh.texture_id != eframe::egui::TextureId::default() =>
            {
                *images += 1;
            }
            eframe::egui::Shape::Text(_) => *texts += 1,
            eframe::egui::Shape::Vec(many) => {
                for one in many {
                    count(one, images, texts);
                }
            }
            _ => {}
        }
    }

    /// The rectangles that painting that screen enqueued, and how many text shapes went with them.
    ///
    /// It exists beside [`shapes_of`] rather than widening it: the rectangles are what the block
    /// elements come out as, and their color and size are the assertion — which counting cannot carry.
    fn rectangles_of(screen: &Screen) -> (Vec<eframe::egui::epaint::RectShape>, usize) {
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let view = screen.view();
        let mut discoverer = crate::fonts::Discoverer::default();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("the grid"),
            ));
            paint(
                &painter,
                &view,
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                &mut discoverer,
                CursorInk::Filled,
            );
        });
        let (mut rectangles, mut texts) = (Vec::new(), 0);
        fn walk(
            shape: &eframe::egui::Shape,
            rectangles: &mut Vec<eframe::egui::epaint::RectShape>,
            texts: &mut usize,
        ) {
            match shape {
                eframe::egui::Shape::Rect(rect) => rectangles.push(rect.clone()),
                eframe::egui::Shape::Text(_) => *texts += 1,
                eframe::egui::Shape::Vec(many) => {
                    for one in many {
                        walk(one, rectangles, texts);
                    }
                }
                _ => {}
            }
        }
        for shape in &output.shapes {
            walk(&shape.shape, &mut rectangles, &mut texts);
        }
        output.drop_without_applying_deltas();
        (rectangles, texts)
    }

    /// The rectangles of a run that its control did not also paint, told apart **by where they are**.
    ///
    /// The cursor is painted in both scenarios and it is a rectangle like any other, so it has to come
    /// out of the comparison. Telling it apart by its color or its opacity would be circular — those
    /// are what the tests using this assert on — while its position is decided by the text, which is
    /// the one thing the two runs share.
    fn only_the_new_ones<'a>(
        run: &'a [eframe::egui::epaint::RectShape],
        control: &[eframe::egui::epaint::RectShape],
    ) -> Vec<&'a eframe::egui::epaint::RectShape> {
        run.iter()
            .filter(|drawn| !control.iter().any(|shared| shared.rect == drawn.rect))
            .collect()
    }

    #[test]
    fn a_shade_is_drawn_as_a_wash_of_the_ink_and_never_asked_of_the_font() {
        // **The defect this covers was reported from a screenshot**: the status line draws its progress
        // bar out of ten `░`, and against the same bar in Tilix ours came out speckled — the font's
        // dither pattern rasterized into a cell it was not drawn for.
        //
        // **A row of letters is the control inside the test**, and it is what isolates the cursor: it is
        // painted in both scenarios, so what the subtraction leaves is the ten cells that changed path.
        // Asserting an absolute count instead would pin down whatever the cursor happens to enqueue.
        let (shaded, texts_shaded) = rectangles_of(&of_text("░░░░░░░░░░"));
        let (control, texts_control) = rectangles_of(&of_text("xxxxxxxxxx"));

        assert_eq!(
            texts_control - texts_shaded,
            10,
            "the shades still went to the font: {texts_shaded} text shapes against the control's \
             {texts_control}"
        );
        assert_eq!(
            shaded.len() - control.len(),
            10,
            "ten shades did not become ten rectangles"
        );

        // **The wash has to be a FRACTION of the ink**, which is the half that decides whether it reads
        // as a light shade or as a solid block. Painted opaque, every assertion above still holds.
        let washes: Vec<u8> = only_the_new_ones(&shaded, &control)
            .iter()
            .map(|rect| rect.fill.a())
            .collect();
        assert!(
            washes.iter().all(|alpha| *alpha > 0 && *alpha < u8::MAX),
            "a light shade was painted with {washes:?} of ink: at nothing it is invisible and at all \
             of it it is a full block"
        );
    }

    #[test]
    fn a_partial_block_is_painted_over_the_part_of_the_cell_its_table_names() {
        // **This is what connects the table to a rectangle, and without it there was nothing.** The
        // table's own tests assert the fractions and the painter's asserted the effect only for `░`
        // and `█` — the two characters whose part **is** the whole cell. A reviewer rebound every part
        // to the whole cell, so `▄`, `▌`, every eighth and every quadrant painted as a full block, and
        // all 833 tests stayed green.
        //
        // The three chosen cover both axes and the thinnest case: with a cell of ten by twenty, `▏` is
        // an eighth of ten. **It is asserted at 1.25 and not at a whole pixel**, because rounding it
        // is the tessellator's — see `paint_a_block`, which used to do it here as well and stopped.
        for (character, expected) in [
            ('▄', Rect::from_min_max(pos2(0.0, 10.0), pos2(10.0, 20.0))),
            ('▌', Rect::from_min_max(pos2(0.0, 0.0), pos2(5.0, 20.0))),
            ('▏', Rect::from_min_max(pos2(0.0, 0.0), pos2(1.25, 20.0))),
        ] {
            let (painted, _) = rectangles_of(&of_text(&character.to_string()));
            let (control, _) = rectangles_of(&of_text("x"));
            let drawn = only_the_new_ones(&painted, &control);

            assert_eq!(drawn.len(), 1, "{character} did not become one rectangle");
            assert_eq!(
                drawn[0].rect, expected,
                "{character} was painted over {:?} and its table says {expected:?}",
                drawn[0].rect
            );
        }
    }

    #[test]
    fn a_block_element_on_screen_never_walks_the_machines_fonts() {
        // **This is what makes skipping the note pointless, and it is why this test exists rather than
        // that skip.** Noting a character is what can walk the machine's families — the search **D24**
        // keeps out of start-up, 4.0 to 5.7 ms — but `resolve`'s oracle discards beforehand anything
        // the loaded faces already show, and the factory faces carry this whole range behind whatever
        // family the settings name. So the saving a per-cell check would buy is zero, and the check
        // would run on every cell of the grid.
        //
        // **It is asserted rather than assumed** because it is the premise of that decision: the day a
        // build ships without a factory face covering the blocks, this goes red and D35's reasoning
        // has to be reopened.
        let searched_the_machine = |text: &str| {
            let ctx = Context::default();
            ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
                .drop_without_applying_deltas();
            let screen = of_text(text);
            let view = screen.view();
            let mut discoverer = crate::fonts::Discoverer::default();
            ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
                let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                    eframe::egui::Order::Foreground,
                    eframe::egui::Id::new("the grid"),
                ));
                paint(
                    &painter,
                    &view,
                    &Theme::default(),
                    &metrics(10.0, 20.0),
                    Pos2::ZERO,
                    &mut discoverer,
                    CursorInk::Filled,
                );
            })
            .drop_without_applying_deltas();
            discoverer.resolve(&ctx).did_work()
        };

        // **The control comes first and it is not decoration.** Without it, «the blocks did not
        // search» is also what a painter that never notes anything at all would report. Japanese is
        // what no factory face here carries.
        assert!(
            searched_the_machine("ぁぁぁ"),
            "a row of Japanese did not send the machine's fonts to be searched, so this test cannot \
             tell a character that needs no search from a painter that searches for nothing"
        );
        assert!(
            !searched_the_machine("░▒▓█▌▏▟"),
            "a row of block elements sent the machine's fonts to be searched: the factory faces no \
             longer cover the range, and D35's reason for noting them like anything else is gone"
        );
    }

    #[test]
    fn nothing_this_painter_measures_is_allowed_to_round_away_to_nothing() {
        // **Both floors carry a paragraph of reasoning and neither was exercised**, because the test
        // metrics are ten by twenty at one pixel to the point, where an eighth is 1.25 and rounds to
        // one — never to zero. A reviewer deleted both and the suite stayed green.
        assert_eq!(
            on_whole_pixels(0.2, 1.0),
            1.0,
            "a font measuring a fifth of a pixel gave a cell of no size, and a grid of no size is a \
             division waiting to happen"
        );

        // A part that survives on its own is left exactly where it was.
        assert_eq!(at_least_a_pixel(3.0, 7.0, 10.0, 1.0), (3.0, 7.0));
        // One that collapsed grows into the room its cell has left.
        assert_eq!(at_least_a_pixel(3.0, 3.0, 10.0, 1.0), (3.0, 4.0));
        // **And one collapsed against the far edge moves BACK, it does not grow over the neighbour.**
        // Growing outwards puts the mark in the next row, which for `▁` in a cell a few pixels tall is
        // not a thinner mark but a mark in the wrong place.
        assert_eq!(at_least_a_pixel(10.0, 10.0, 10.0, 1.0), (9.0, 10.0));
    }

    #[test]
    fn a_full_block_covers_its_cell_exactly_so_no_seam_shows_between_two_of_them() {
        // **This is the half that was visible in the logo**: full blocks drawn by the font came out with
        // hairlines between them, because the glyph is rasterized for one size and stamped in a cell of
        // another. Drawn here, the rectangle **is** the cell.
        let (blocks, _) = rectangles_of(&of_text("███"));
        let (control, _) = rectangles_of(&of_text("xxx"));
        let cell = metrics(10.0, 20.0);
        let drawn = only_the_new_ones(&blocks, &control);

        assert_eq!(
            drawn.len(),
            3,
            "three full blocks did not become three rectangles"
        );
        for (which, rect) in drawn.iter().enumerate() {
            assert_eq!(
                rect.rect.width(),
                cell.width(),
                "block {which} is not as wide as its cell, so a gap or an overlap shows beside it"
            );
            assert_eq!(
                rect.rect.height(),
                cell.height(),
                "block {which} is not as tall as its cell"
            );
            assert_eq!(
                rect.fill.a(),
                u8::MAX,
                "a full block was painted with part of the ink, so what shows through is the background"
            );
        }
        // **And each one starts exactly where the previous one ends.** Two rectangles the right size
        // that do not touch leave the same seam the font's glyphs did.
        for pair in drawn.windows(2) {
            assert_eq!(
                pair[0].rect.right(),
                pair[1].rect.left(),
                "two consecutive blocks do not touch"
            );
        }
    }

    /// A single-line screen with that text inside.
    fn of_text(text: &str) -> Screen {
        let mut screen = Screen::new(Size { rows: 1, cols: 20 });
        screen.advance(text.as_bytes());
        screen
    }

    /// Whether this machine has what it takes to paint color emojis. Without that, half the milestone is
    /// not verifiable here.
    fn there_is_color_on_this_machine() -> bool {
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        crate::fonts::Discoverer::default()
            .color_glyph(&ctx, '\u{1F600}', 20, 20)
            .is_some()
    }

    #[test]
    fn a_good_old_fashioned_emoji_comes_out_in_color_without_anyone_asking() {
        // **AC3, and it is the visible half of the milestone.** `😀` has `Emoji_Presentation=Yes`, so it
        // goes to color with no selector at all.
        //
        // **The assertion is in BOTH scenarios**: where the machine has a color font, the emoji has to
        // come out as an image; where it does not, it has to keep coming out via the text path and the
        // window stay standing. Skipping the test on a bare machine would leave uncovered exactly the
        // path where an implementation falls over.
        let (images, texts) = shapes_of(&of_text("\u{1F600}"));
        if there_is_color_on_this_machine() {
            assert_eq!(
                images, 1,
                "the emoji was not painted as an image: it came out via the text path, that is, monochrome"
            );
        } else {
            assert_eq!(
                images, 0,
                "there is no color font on the machine and yet an image was painted"
            );
            assert_eq!(
                texts, 1,
                "without a color font the emoji has to come out anyway, in monochrome"
            );
        }
    }

    #[test]
    fn the_text_symbol_stays_text_and_with_its_selector_goes_to_color() {
        // **AC4, and both halves in the same test on purpose.** `☁` and `☁️` are **the same character**
        // with a different presentation: checking only one of the two would pass with a painter that sent
        // everything to color, or everything to text.
        let (no_selector, texts_without) = shapes_of(&of_text("\u{2601}"));
        assert_eq!(
            no_selector, 0,
            "a cloud with no selector was painted in color: it is a text symbol and this way it is lost"
        );
        assert_eq!(texts_without, 1, "the cloud was not painted in any way");

        let (with_selector, _) = shapes_of(&of_text("\u{2601}\u{FE0F}"));
        if there_is_color_on_this_machine() {
            assert_eq!(
                with_selector, 1,
                "the cloud with an emoji selector did not come out in color: the selector is not reaching the rule"
            );
        }
    }

    #[test]
    fn a_keycap_made_with_a_hash_comes_out_in_color() {
        // **The case the ASCII shortcut ate, and that is why it is here and not only in the rule.** The
        // color pass discards ASCII without consulting the Unicode tables, because it is almost all of
        // what passes through a screen; but `#`, `*` and the digits have `Emoji=Yes`, so with the
        // selector behind they are keycaps and go to color.
        //
        // The rule's test, in `fonts`, does not cover this: the shortcut lives **here**, and mutating it
        // to eat all of ASCII left that one green. It was verified by mutating it.
        if !there_is_color_on_this_machine() {
            return;
        }
        let (images, _) = shapes_of(&of_text("#\u{FE0F}\u{20E3}"));
        assert_eq!(
            images, 1,
            "the «#️⃣» keycap came out via the text path: the painter is discarding ASCII without \
             looking at what is attached to it, and eats the eleven keycaps Unicode makes with ASCII"
        );
    }

    #[test]
    fn the_text_selector_returns_to_monochrome_an_emoji_that_would_go_to_color() {
        // The other half of **AC4**, the one no one writes: the selector that **removes** the color.
        // Without it, a painter that ignored `U+FE0E` would pass the two tests above.
        let (images, texts) = shapes_of(&of_text("\u{1F600}\u{FE0E}"));
        assert_eq!(
            images, 0,
            "the emoji came out in color carrying the TEXT selector behind it, which is exactly what forbids it"
        );
        assert_eq!(
            texts, 1,
            "with the text selector the emoji has to come out the same, in monochrome"
        );
    }

    #[test]
    fn the_emoji_does_not_invade_the_neighboring_cell() {
        // **AC5.** The `Noto Color Emoji` bitmap is **wider than tall** —136×128, verified by running
        // it— so scaling it by the cell's height leaves it wider than the two columns the engine reserved
        // for it, and it eats its neighbor's edge.
        //
        // **The assertion is on the rectangle that was enqueued**, which is what is seen, and not on the
        // texture's size: a texture of the correct size painted in a stretched rectangle comes out just
        // as badly.
        if !there_is_color_on_this_machine() {
            return;
        }
        let screen = of_text("\u{1F600}");
        let view = screen.view();
        let metrics = metrics(10.0, 20.0);
        assert_eq!(
            view.cell(0, 0).map(|cell| cell.width),
            Some(CellWidth::WideHead),
            "the engine did not give the emoji two columns: the test is not measuring what it says"
        );

        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("an emoji"),
            ));
            paint(
                &painter,
                &view,
                &Theme::default(),
                &metrics,
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
                CursorInk::Filled,
            );
        });
        let mut drawing = None;
        for shape in &output.shapes {
            if let eframe::egui::Shape::Mesh(mesh) = &shape.shape
                && mesh.texture_id != eframe::egui::TextureId::default()
            {
                drawing = Some(mesh.calc_bounds());
            }
        }
        output.drop_without_applying_deltas();
        let drawing = drawing.expect("the emoji was not painted as an image");

        // The two columns its width declares, and not a point more.
        let its_own = Rect::from_min_size(Pos2::ZERO, vec2(metrics.width * 2.0, metrics.height));
        assert!(
            its_own.contains_rect(drawing),
            "the emoji occupies {drawing:?} and its two columns are {its_own:?}: it spills over the neighboring cell"
        );
        // **And that it has not been deformed to fill them.** The margin is tight on purpose: with one of
        // a tenth, stretching the emoji to the whole slot —ratio 1.000 against its own, 1.062— passed this
        // test. It was verified by mutating it, and it is the disguise this project has on file: a margin
        // chosen with which the defect and the fix give the same result.
        let ratio = drawing.width() / drawing.height();
        assert!(
            (ratio - 136.0 / 128.0).abs() < 0.03,
            "the emoji was painted with ratio {ratio:.3} instead of its own (1.062): it is stretched"
        );
        // **And the other face of the same thing, which is the one that is seen**: keeping the proportion
        // inside a slot that does not have it forces room to be left on one of the two axes. A drawing
        // that fills the exact slot is stretched by definition.
        assert!(
            drawing.width() < its_own.width() || drawing.height() < its_own.height(),
            "the emoji fills its exact slot ({drawing:?} against {its_own:?}): it is stretched to fill it"
        );
    }

    #[test]
    fn under_the_cursor_the_emoji_is_still_seen_in_color() {
        // **The block cursor covers the character and repaints it on top.** That repaint had its own call
        // to the text engine, so an emoji under the cursor came out monochrome —only there, and only
        // while the cursor was over it—. It is the same defect a review found with the Powerline icons, in
        // the same spot.
        if !there_is_color_on_this_machine() {
            return;
        }
        let mut screen = Screen::new(Size { rows: 1, cols: 20 });
        screen.advance("\u{1F600}".as_bytes());
        screen.advance(b"\x1b[1;1H");
        assert_eq!(
            screen.view().cursor().map(|c| (c.row, c.col)),
            Some((0, 0)),
            "the cursor did not land over the emoji: the test is not measuring what it says"
        );
        let (images, _) = shapes_of(&screen);
        assert_eq!(
            images, 2,
            "with the cursor over it there have to be two images —the painting one and the cursor's \
             repaint one—: if there is one, the cursor is covering the emoji and repainting it in monochrome"
        );
    }

    fn cursor_shapes(view: &View) -> usize {
        let ctx = Context::default();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("only the cursor"),
            ));
            paint_cursor(
                &painter,
                view,
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
                CursorInk::Filled,
            );
        });
        let shapes = output.shapes.len();
        output.drop_without_applying_deltas();
        shapes
    }

    #[test]
    fn with_the_history_scrolled_up_no_cursor_is_painted() {
        // **The other half of "the cursor is not seen", and no one tied it down.** That the snapshot
        // returns `None` is checked in `term-core`; that this painter does not draw a block in the last
        // row anyway was checked nowhere. The guard further below —the one that looks at whether the cell
        // exists— did not cover this even though its comment said it did: the row arrived clamped, so it
        // always fell inside the grid and a **phantom cursor** would have been painted in a spot where the
        // program does not write.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..40 {
            screen.advance(format!("line {n}\r\n").as_bytes());
        }

        // Up to date, the cursor is seen and painted: without this, a painter that never drew anything
        // would pass.
        assert!(
            cursor_shapes(&screen.view()) > 0,
            "with the screen up to date no cursor was painted, so the check below does not \
             distinguish anything"
        );

        screen.scroll_by(term_core::Scroll::ToTop);
        assert_eq!(
            screen.view().cursor(),
            None,
            "the test did not manage to leave the cursor outside what is being looked at"
        );

        assert_eq!(
            cursor_shapes(&screen.view()),
            0,
            "a cursor was painted with the snapshot saying it is not seen: it is a phantom cursor in the \
             last row, and whoever looks will believe the program writes there"
        );
    }

    /// A theme whose cursor is **not** the colour of its text.
    ///
    /// **Out of the box those two entries hold the same value** — `the_three_default_colors_are_the_project_ones`
    /// asserts both are the project's grey — so a test that reads the cursor's colour off the default
    /// theme cannot tell the two apart, and swapping them in the palette would leave it green. Every
    /// assertion here about what colour the cursor came out in runs on this one instead.
    fn theme_whose_cursor_is_its_own() -> Theme {
        Theme::from_colors(&crate::settings::Colors {
            cursor: Some(A_CURSOR_OF_ITS_OWN),
            ..Default::default()
        })
    }

    /// The colour the tests above put in the theme's cursor entry, **written out**.
    ///
    /// **Asking the theme for it instead would measure the code against itself.** The shape is found
    /// by the colour the painter used, so with both sides of that comparison coming from
    /// `Theme::color(TermColor::Cursor)`, pointing that entry at the foreground changes both at once
    /// and cancels out — checked by mutation, the seven tests stayed green with the cursor's entry
    /// serving the text's colour.
    const A_CURSOR_OF_ITS_OWN: Color32 = Color32::from_rgb(0xFF, 0x00, 0xFF);

    /// The rectangles the cursor put on the queue, as `(is it filled, its rectangle, how thick its
    /// line is)`.
    ///
    /// **It keeps the shape and not a count**, unlike [`cursor_shapes`]: a count cannot tell a filled
    /// block from an outlined one, which is the whole of what the focus decides.
    ///
    /// **And it keeps the stroke's width, which is not padding.** A stroke of zero is queued, carries
    /// the right colour and covers the right rectangle — and is **invisible on screen**. Nothing else
    /// in this crate asserts a stroke's width, so without this every test here passes over an
    /// unfocused console that draws no cursor at all: disguise 3, a shape in the queue that nobody
    /// can see.
    fn cursor_rectangles(view: &View, focused: bool) -> Vec<(bool, Rect, f32)> {
        cursor_rectangles_in(
            view,
            if focused {
                CursorInk::Filled
            } else {
                CursorInk::Outline
            },
        )
    }

    /// The same, said in the ink itself, for what only the blink can reach.
    fn cursor_rectangles_in(view: &View, ink: CursorInk) -> Vec<(bool, Rect, f32)> {
        let theme = theme_whose_cursor_is_its_own();
        let colour = A_CURSOR_OF_ITS_OWN;
        let ctx = Context::default();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("only the cursor"),
            ));
            paint_cursor(
                &painter,
                view,
                &theme,
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
                ink,
            );
        });
        let mut found = Vec::new();
        for shape in &output.shapes {
            if let eframe::egui::Shape::Rect(rect) = &shape.shape {
                if rect.fill == colour {
                    found.push((true, rect.rect, rect.stroke.width));
                } else if rect.stroke.color == colour {
                    found.push((false, rect.rect, rect.stroke.width));
                }
            }
        }
        output.drop_without_applying_deltas();
        found
    }

    /// A screen of one row whose program asked for `shape`, with the cursor over a letter.
    fn a_screen_asking_for(shape: &str) -> Screen {
        let mut screen = Screen::new(Size { rows: 2, cols: 8 });
        screen.advance(b"ab");
        screen.advance(shape.as_bytes());
        screen
    }

    #[test]
    fn the_console_without_the_keyboard_outlines_its_cursor_whatever_shape_it_asked_for() {
        // **The shape is the program's while the console is being typed into, and stops being it
        // when it is not** — that is what kitty's `hollow` default and WezTerm's inactive panes do,
        // and it is why this is one branch and not five. The three requests are exercised, not one:
        // with only the block, a version that outlined the block and left the beam a filled bar
        // would pass.
        // **And the focused half is asserted by its SIZE, not only by being filled.** «Filled» is
        // satisfied by a block, so a version that painted every focused cursor as a full block would
        // pass — and it would be a regression against what the program asked for, invisible to a
        // test that only asks whether the rectangle has a fill. The cell here is 10 by 20: the block
        // covers it, the underline is a tenth of its height along the bottom and the beam fifteen
        // hundredths of its width down the side.
        for (shape, request, filled_size) in [
            ("block", "\x1b[2 q", (10.0, 20.0)),
            ("underline", "\x1b[4 q", (10.0, 2.0)),
            ("beam", "\x1b[6 q", (1.5, 20.0)),
        ] {
            let screen = a_screen_asking_for(request);

            let focused = cursor_rectangles(&screen.view(), true);
            let &[(is_filled, mark, _)] = focused.as_slice() else {
                panic!("the {shape} cursor of a focused console queued {focused:?}");
            };
            assert!(
                is_filled,
                "the {shape} cursor of the console being typed into came out outlined"
            );
            assert_eq!(
                (mark.width(), mark.height()),
                filled_size,
                "the console being typed into did not paint the {shape} its program asked for"
            );

            let unfocused = cursor_rectangles(&screen.view(), false);
            let &[(is_filled, outline, thickness)] = unfocused.as_slice() else {
                panic!("the {shape} cursor of an unfocused console queued {unfocused:?}");
            };
            assert!(
                !is_filled,
                "the {shape} cursor of a console without the keyboard came out filled, so two \
                 consoles both look like the one being typed into"
            );
            // **The three of them are measured, not just the beam.** With the size asserted for one
            // shape only, an outline traced around the underline's own thin bar satisfies «not
            // filled» and is invisible at a tenth of a cell.
            assert_eq!(
                (outline.width(), outline.height()),
                (10.0, 20.0),
                "the outline over a {shape} is not the size of a cell, so what it outlines is the \
                 shape and not the cell"
            );
            // **And a line of no thickness is queued, is the right colour, covers the right
            // rectangle and cannot be seen.** Nothing else in this crate asserts a stroke's width,
            // so without this line every assertion above passes over a console that draws no cursor
            // at all.
            assert!(
                thickness >= 1.0,
                "the outline over a {shape} is {thickness} thick, so it is queued and invisible"
            );
        }
    }

    #[test]
    fn the_outline_of_an_unfocused_console_covers_both_columns_of_a_wide_character() {
        // **Over a wide character the outline covers the two columns it occupies**, exactly as the
        // filled block already does — half a letter inside the cursor and half outside reads as a
        // drawing error. The cell-sized outline for each shape the program can ask for is asserted
        // by the test above; what only this one exercises is the widening.
        let mut wide = Screen::new(Size { rows: 2, cols: 8 });
        wide.advance("日".as_bytes());
        wide.advance(b"\x1b[H");
        let over_a_wide_one = cursor_rectangles(&wide.view(), false);
        let &[(_, across_both, _)] = over_a_wide_one.as_slice() else {
            panic!("a wide character queued {over_a_wide_one:?} instead of one cursor");
        };
        assert_eq!(
            (across_both.width(), across_both.height()),
            (20.0, 20.0),
            "the outline did not cover the two columns a wide character occupies and the whole of \
             their height, so part of the letter is inside the cursor and part outside"
        );
    }

    #[test]
    fn a_hidden_cursor_stays_hidden_when_its_console_does_not_have_the_keyboard() {
        // **Losing the keyboard must not make a cursor appear.** The outline branch goes after the
        // guard for a shape of `Hidden` for exactly this; above it, a program that asked for no
        // cursor gets one as soon as its console stops being the one being typed into.
        let mut screen = Screen::new(Size { rows: 2, cols: 8 });
        screen.advance(b"ab\x1b[?25l");
        assert_eq!(
            screen.view().cursor().map(|cursor| cursor.shape),
            Some(CursorShape::Hidden),
            "the test did not manage to get the program to hide its cursor"
        );

        assert!(
            cursor_rectangles(&screen.view(), false).is_empty(),
            "a console without the keyboard drew a cursor its program had hidden"
        );
        assert!(
            cursor_rectangles(&screen.view(), true).is_empty(),
            "a console with the keyboard drew a cursor its program had hidden"
        );
    }

    /// A blink whose phase is `since` after the last keystroke, with the program left to decide.
    fn blink_at(millis: u64) -> Blink {
        Blink {
            ceiling: None,
            since_a_key: Duration::from_millis(millis),
            focused: true,
        }
    }

    #[test]
    fn the_cursor_is_lit_for_six_hundred_milliseconds_and_dark_for_the_next_six_hundred() {
        // **The boundaries and not the middles**, because a period read from the wrong constant —
        // 500, 1 200 — gives the same answer in the middle of a half and a different one here.
        for (millis, lit) in [
            (0, true),
            (599, true),
            (600, false),
            (1_199, false),
            (1_200, true),
            (1_800, false),
        ] {
            let (is_lit, _) = blink_at(millis)
                .phase()
                .expect("the blink has not stopped yet");
            assert_eq!(is_lit, lit, "at {millis} ms after the keystroke");
        }
    }

    #[test]
    fn the_deadline_is_what_is_left_of_this_half_and_never_more() {
        // Arming for a whole half from the middle of one paints the flip late by as much as it is
        // into it, which is a cursor that stutters instead of blinking.
        assert_eq!(
            blink_at(0).phase().map(|(_, left)| left),
            Some(Duration::from_millis(600))
        );
        assert_eq!(
            blink_at(100).phase().map(|(_, left)| left),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            blink_at(700).phase().map(|(_, left)| left),
            Some(Duration::from_millis(500))
        );
    }

    #[test]
    fn the_blink_stops_after_fifteen_seconds_of_silence_and_stops_lit() {
        // **The edge is «stop blinking», never «stop being visible».** A stop that landed in the
        // dark half would leave the console you are looking at with no cursor at all until you
        // typed, which is the worst possible way to end a blink.
        assert_eq!(
            blink_at(15_000).phase(),
            None,
            "the blink went on past its stop"
        );
        assert_eq!(blink_at(60_000).phase(), None);
        assert_eq!(
            blink_at(15_000).on(true).ink(true),
            CursorInk::Filled,
            "a stopped blink has to leave the cursor painted, and asking for no more frames"
        );

        // And the last deadline before the stop is the stop itself, so the frame that repaints it
        // lit is asked for and not missed.
        assert_eq!(
            blink_at(14_900).phase().map(|(_, left)| left),
            Some(Duration::from_millis(100))
        );
    }

    #[test]
    fn the_stop_is_a_whole_number_of_halves_and_that_is_why_the_clamp_is_inert() {
        // **This is the test the clamp deserved, and the one it did not have.** A reviewer showed
        // that `.min(left)` in `phase` can never bind — 15 000 is exactly 25 halves of 600, so the
        // last flip lands *on* the stop — and that the assertion above stays green with the clamp
        // deleted. So the clamp is not covered by testing a deadline; what is worth pinning is the
        // relationship that makes it inert. The day somebody moves either constant this fails, and
        // it says where to look. The comment that used to defend the clamp claimed the flip at
        // 14.9 s «would fall at 15.3»; it falls at 15.0, which is the arithmetic the whole guard
        // hangs on.
        assert_eq!(
            A_BLINK_STOPS_AFTER.as_millis() % HALF_A_BLINK.as_millis(),
            0,
            "the stop no longer falls on a flip: `Blink::phase`'s clamp to it now decides something, \
             and the last half of every blink is cut short"
        );
    }

    #[test]
    fn the_settings_file_overrules_the_program_in_both_directions() {
        // The whole of what makes `cursor.blink` a ceiling rather than a default: a program that
        // asks loudly is silenced, and one that asks for a steady cursor is overruled.
        let dark = Duration::from_millis(600);
        let lit = Duration::ZERO;

        let forced_off = Blink {
            ceiling: Some(false),
            since_a_key: dark,
            focused: true,
        };
        assert_eq!(forced_off.on(true).ink(true), CursorInk::Filled);

        let forced_on = Blink {
            ceiling: Some(true),
            since_a_key: dark,
            focused: true,
        };
        assert_eq!(
            forced_on.on(true).ink(false),
            CursorInk::Blinking { lit: false }
        );

        let program_decides = Blink {
            ceiling: None,
            since_a_key: dark,
            focused: true,
        };
        assert_eq!(
            program_decides.on(true).ink(true),
            CursorInk::Blinking { lit: false }
        );
        assert_eq!(program_decides.on(true).ink(false), CursorInk::Filled);
        assert_eq!(
            Blink {
                ceiling: None,
                since_a_key: lit,
                focused: true,
            }
            .on(true)
            .ink(true),
            CursorInk::Blinking { lit: true }
        );
    }

    #[test]
    fn a_console_without_the_keyboard_never_reaches_the_dark_half() {
        // The dark half is reachable only through a blink **of the console being typed into**.
        // Without this, a window with four panes would go dark in three of them every 600 ms.
        for asked_for in [true, false] {
            for millis in [0, 600, 1_200] {
                assert_eq!(
                    blink_at(millis).on(false).ink(asked_for),
                    CursorInk::Outline,
                    "an unfocused console at {millis} ms asking {asked_for}"
                );
            }
        }
    }

    /// What `paint_cursor` **answered**, which is what the repaint deadline is armed from.
    ///
    /// **Its own value, and not the shapes it queued.** A test that only counts rectangles is
    /// satisfied by a dark half that answers «no blink here», and that break — measured — left all
    /// 819 tests green while shipping a cursor that goes dark once and never comes back.
    fn what_paint_cursor_answered(view: &View, ink: CursorInk) -> bool {
        let ctx = Context::default();
        let mut answer = false;
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("only the cursor"),
            ));
            answer = paint_cursor(
                &painter,
                view,
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
                ink,
            );
        });
        output.drop_without_applying_deltas();
        answer
    }

    #[test]
    fn both_halves_of_a_blink_answer_that_the_window_has_to_wake_up() {
        // **The dark half draws nothing and still says «I am blinking».** Answering `false` there
        // would disarm the deadline on every other flip, and the cursor would go dark once and stay
        // that way until somebody typed — which is a suite-green way of shipping no blink at all.
        // **Mutated, and the first version of this test did not catch it**: it called `paint_cursor`
        // for its answer and asserted only how many rectangles came out.
        let screen = a_screen_asking_for("\x1b[5 q");
        let view = screen.view();

        let lit = cursor_rectangles_in(&view, CursorInk::Blinking { lit: true });
        assert_eq!(lit.len(), 1, "the lit half drew no cursor");
        assert!(
            cursor_rectangles_in(&view, CursorInk::Blinking { lit: false }).is_empty(),
            "the dark half drew a cursor"
        );

        assert!(
            what_paint_cursor_answered(&view, CursorInk::Blinking { lit: false }),
            "the dark half answered that nothing is blinking, so the deadline is disarmed and the \
             cursor never comes back"
        );
        assert!(what_paint_cursor_answered(
            &view,
            CursorInk::Blinking { lit: true }
        ));
        assert!(
            !what_paint_cursor_answered(&view, CursorInk::Filled),
            "a cursor that does not blink asked the window to wake up"
        );
        assert!(!what_paint_cursor_answered(&view, CursorInk::Outline));
    }

    #[test]
    fn a_cursor_nobody_can_see_wakes_nobody_however_loudly_its_program_asks() {
        // **The trap the deadline is built to avoid.** Every full-screen program sends `CSI ?25l`,
        // and the blink bit survives it: with `cursor.blink = on` forcing the ceiling, arming from
        // the request would wake the window at 1.7 Hz to paint nothing at all. The three guards run
        // first, and the answer they give is what the alarm reads.
        let mut screen = Screen::new(Size { rows: 2, cols: 8 });
        screen.advance(b"ab\x1b[5 q\x1b[?25l");
        assert!(
            screen.cursor_blinks(),
            "the test did not manage to leave a program asking for a blink it cannot show"
        );

        let ctx = Context::default();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("only the cursor"),
            ));
            let woke = paint_cursor(
                &painter,
                &screen.view(),
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
                CursorInk::Blinking { lit: false },
            );
            assert!(
                !woke,
                "a hidden cursor asked the window to wake up for its blink"
            );
        });
        output.drop_without_applying_deltas();
    }

    #[test]
    fn a_cursor_scrolled_out_of_the_view_is_not_drawn_by_a_console_without_the_keyboard_either() {
        // The sibling of `with_the_history_scrolled_up_no_cursor_is_painted`, for the branch that
        // did not exist when that one was written.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..40 {
            screen.advance(format!("line {n}\r\n").as_bytes());
        }
        assert!(
            !cursor_rectangles(&screen.view(), false).is_empty(),
            "with the screen up to date no cursor was drawn, so the check below distinguishes nothing"
        );

        screen.scroll_by(term_core::Scroll::ToTop);
        assert!(
            cursor_rectangles(&screen.view(), false).is_empty(),
            "a phantom cursor was outlined in the last row of a console nobody is typing into"
        );
    }

    #[test]
    fn a_blinking_console_scrolled_up_through_its_history_paints_nothing_and_wakes_nobody() {
        // **A focused console that paints no cursor is not proof of the dark half of a blink**, and
        // this is one of the three ways the converse fails. Scrolling up leaves the cursor in the
        // active area, below what is being looked at, so the view carries no cursor at all — and
        // the half that would go unnoticed is the **answer**, not the drawing. Arm the deadline
        // here and the window wakes 1.7 times a second, for ever, to flip a mark that is never
        // drawn: exactly the cost this whole deadline exists to bound, and invisible on screen.
        let mut screen = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..40 {
            screen.advance(format!("line {n}\r\n").as_bytes());
        }
        screen.scroll_by(term_core::Scroll::ToTop);
        assert!(
            screen.view().cursor().is_none(),
            "the test did not manage to scroll the cursor out of the view"
        );
        assert!(
            screen.cursor_blinks(),
            "the program is not even asking for a blink, so what follows distinguishes nothing"
        );

        for lit in [true, false] {
            assert!(
                cursor_rectangles_in(&screen.view(), CursorInk::Blinking { lit }).is_empty(),
                "a console whose cursor is scrolled out of the view drew one in the lit={lit} half"
            );
            assert!(
                !what_paint_cursor_answered(&screen.view(), CursorInk::Blinking { lit }),
                "a console with no cursor in its view asked the window to go on waking up, in the \
                 lit={lit} half, for a blink nobody can see"
            );
        }
    }

    #[test]
    fn a_blinking_console_whose_program_hid_its_cursor_draws_nothing_in_the_lit_half_either() {
        // **The second way a focused console paints no cursor without being in a blink's dark
        // half**, and the half `a_cursor_nobody_can_see_wakes_nobody_however_loudly_its_program_asks`
        // cannot reach: that one reads the dark half, where drawing nothing is also what a blink
        // does, so a `Hidden` guard deleted outright leaves it green. Here the ink says «lit», so
        // any rectangle queued is a cursor the program switched off with `CSI ?25l` coming back to
        // life every 600 ms — a cursor that flashes in a full-screen editor that asked for none.
        let mut screen = Screen::new(Size { rows: 2, cols: 8 });
        screen.advance(b"ab\x1b[5 q\x1b[?25l");
        assert!(
            screen.cursor_blinks(),
            "the test did not manage to leave a program asking for a blink it cannot show"
        );

        assert!(
            cursor_rectangles_in(&screen.view(), CursorInk::Blinking { lit: true }).is_empty(),
            "the lit half of a blink drew a cursor the program had hidden"
        );
        assert!(
            !what_paint_cursor_answered(&screen.view(), CursorInk::Blinking { lit: true }),
            "the lit half of a hidden cursor asked the window to wake up for its blink"
        );
    }

    #[test]
    fn a_reported_cursor_always_stands_on_a_cell_and_that_is_why_the_third_guard_is_inert() {
        // **The third of the three guards cannot be reached, and this is the test it deserves.**
        // The design names «no cell where it stands» beside the other two as a way a focused
        // console paints no cursor without being in a blink's dark half — but no `Screen` can
        // produce it: the row is range-checked and the column clamped where the view is built
        // (`screen.rs`), a grid of no columns is refused by the engine itself
        // (`grid/row.rs:38`, `assert!(columns >= 1)`), and `View::new` is `pub(crate)` to
        // `term-core`, so nothing here can fabricate one either. So there is no scenario to write,
        // and the same reasoning that gave `the_stop_is_a_whole_number_of_halves_and_that_is_why_the_clamp_is_inert`
        // its test applies: what is worth pinning is the **relationship that keeps the guard
        // inert** — «whenever the view reports a cursor at all, there is a cell where it stands».
        //
        // **The scrolled-up state is the one that carries the mutation**, and it is the reason it
        // is in this list even though it reports no cursor: `screen.rs` answers `None` there
        // instead of clamping the row, and turning that `None` into a clamp — the obvious
        // simplification, and the shape this crate had before F1.3 — puts a cursor outside the grid
        // and this test goes red on it. The other three are the states around it, so that a change
        // to the column's clamp is caught by the same assertion rather than by nothing.
        let mut scrolled = Screen::new(Size { rows: 4, cols: 20 });
        for n in 0..40 {
            scrolled.advance(format!("line {n}\r\n").as_bytes());
        }
        scrolled.scroll_by(term_core::Scroll::ToTop);
        let mut states: Vec<(&str, Screen)> = vec![("the history scrolled to the top", scrolled)];

        let mut at_the_edge = Screen::new(Size { rows: 4, cols: 20 });
        at_the_edge.advance(b"\x1b[999G");
        states.push(("the cursor driven past the last column", at_the_edge));

        let mut narrowed = Screen::new(Size { rows: 4, cols: 20 });
        narrowed.advance(b"\x1b[999G");
        narrowed.resize(Size { rows: 4, cols: 3 });
        states.push(("narrowed under a cursor that was to the right", narrowed));

        let mut shortened = Screen::new(Size { rows: 4, cols: 20 });
        shortened.advance(b"\x1b[4;1H");
        shortened.resize(Size { rows: 1, cols: 20 });
        states.push((
            "shortened under a cursor that was in the last row",
            shortened,
        ));

        let mut reported = 0;
        for (what, screen) in states {
            let view = screen.view();
            let Some(cursor) = view.cursor() else {
                continue;
            };
            reported += 1;
            assert!(
                view.cell(cursor.row, cursor.col).is_some(),
                "with {what} the view reports a cursor at ({}, {}) and no cell there: the third \
                 guard of `paint_cursor` now decides something",
                cursor.row,
                cursor.col
            );
        }
        // Without this the whole test is satisfied by four states that report no cursor at all,
        // which is the vacuous pass every helper in this file is shaped to avoid.
        assert_eq!(
            reported, 3,
            "three of the four states have to report a cursor and the scrolled-up one must not"
        );
    }

    /// In what order the painter enqueued its glyphs: `"text"` or `"image"`, one per shape.
    fn order_of(screen: &Screen) -> Vec<&'static str> {
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let view = screen.view();
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(eframe::egui::LayerId::new(
                eframe::egui::Order::Foreground,
                eframe::egui::Id::new("the order"),
            ));
            paint(
                &painter,
                &view,
                &Theme::default(),
                &metrics(10.0, 20.0),
                Pos2::ZERO,
                &mut crate::fonts::Discoverer::default(),
                CursorInk::Filled,
            );
        });
        let mut order = Vec::new();
        for shape in &output.shapes {
            note_down(&shape.shape, &mut order);
        }
        output.drop_without_applying_deltas();
        order
    }

    fn note_down(shape: &eframe::egui::Shape, order: &mut Vec<&'static str>) {
        match shape {
            eframe::egui::Shape::Mesh(mesh)
                if mesh.texture_id != eframe::egui::TextureId::default() =>
            {
                order.push("image");
            }
            eframe::egui::Shape::Text(_) => order.push("text"),
            eframe::egui::Shape::Vec(many) => {
                for one in many {
                    note_down(one, order);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn the_emojis_are_enqueued_all_together_and_not_among_the_letters() {
        // **It is the atlas's reason for being, and until here no one tied it down.** The engine only
        // batches into the same draw call shapes that share a texture: emojis live in their atlas and the
        // letters in the font one, so enqueuing them in grid order splits the batch **at every emoji**.
        // Measured in **D25** with the two consoles dumping emojis: the rest of the frame went from 275 to
        // 743 µs, that is, almost triple the figure with which **D3** judges the framework, and for work
        // that is not its own.
        //
        // **The assertion is on the order of what was enqueued, which is the only thing that decides the
        // split into batches.** Counting how many images come out —which is what the other tests here do—
        // passes just the same with the two passes merged into one: it was verified by merging them, and
        // the 647 stayed green.
        if !there_is_color_on_this_machine() {
            return;
        }
        let order = order_of(&of_text("a\u{1F600}b\u{1F600}c"));

        assert_eq!(
            order
                .iter()
                .filter(|character| **character == "image")
                .count(),
            2,
            "the two emojis did not come out as images ({order:?}): the test is not looking at what it says"
        );
        assert!(
            order
                .iter()
                .filter(|character| **character == "text")
                .count()
                >= 3,
            "the three letters did not come out ({order:?}): the test is not looking at what it says"
        );
        let first_image = order.iter().position(|what| *what == "image");
        let last_text = order.iter().rposition(|what| *what == "text");
        assert!(
            first_image > last_text,
            "the emojis were enqueued among the letters ({order:?}): the engine splits the draw batch at \
             each one, which is exactly what the atlas exists to avoid"
        );
    }

    #[test]
    fn with_the_atlas_full_the_emoji_is_still_seen_even_if_in_monochrome() {
        // **Contract 5 by the path this machine never walks.** When the atlas fills up, asking for the
        // color glyph returns nothing and the painter **has to fall back to the text path**: the emoji
        // comes out in monochrome, which is how it came out before the milestone, instead of disappearing
        // from the screen.
        //
        // **On a machine with an emoji font no other test exercises that fallback**, because here the
        // color is always there: it was verified by mutating the fallback so it painted nothing, and the
        // 647 stayed green. The other half —that with the atlas full no arbitrary texture is returned— is
        // tied down in `fonts`.
        if !there_is_color_on_this_machine() {
            return;
        }
        let mut discoverer = crate::fonts::Discoverer::default();
        let ctx = Context::default();
        ctx.run_ui(eframe::egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        // This test's cell is 20 points tall, and the physical pixels here go one to one.
        discoverer.fill_the_atlas(&ctx, 20);

        let (images, texts) = shapes_of_with(&of_text("\u{1F600}"), &mut discoverer);

        assert_eq!(
            images, 0,
            "with the atlas full an image was painted anyway: another glyph's drawing comes out"
        );
        assert_eq!(
            texts, 1,
            "with the atlas full the emoji was not painted in any way: it disappears from the screen \
             instead of coming out in monochrome, and whoever wrote it will not know it was there"
        );
    }
}
