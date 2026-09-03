//! Draws the terminal grid: backgrounds, glyphs and cursor.
//!
//! **It is deliberately the most naive painter that works**: one cell at a time, with no cache and
//! no tracking of what changed. This is neither carelessness nor a first draft — optimizing before
//! measuring would hide the very symptom this milestone exists to observe, and the failure mode of a
//! glyph cache is phantom cells, which are the kind you don't see until you are already weeks deep.
//!
//! **The cell's size is asked for, not invented.** The width comes from the font's reference glyph
//! and the height from its line height. Guessing it by eye is what throws the rows out of line.

use crate::fonts::Presentation;
use crate::theme::Theme;
use eframe::egui::{
    Align2, Color32, Context, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2, vec2,
};
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
        let (width, height) =
            ctx.fonts_mut(|fonts| (fonts.glyph_width(&font, 'M'), fonts.row_height(&font)));
        Self {
            width,
            height,
            font,
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

    /// The rectangle the cell of a given row and column occupies, from the given origin.
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
) {
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

    paint_cursor(painter, view, theme, metrics, origin, discoverer);
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

fn paint_cursor(
    painter: &Painter,
    view: &View,
    theme: &Theme,
    metrics: &CellMetrics,
    origin: Pos2,
    discoverer: &mut crate::fonts::Discoverer,
) {
    // **With no cursor, no cursor is painted**, and since there is history that case really happens:
    // scrolling up far enough, the cursor stays in the active area, below what is being looked at. The
    // snapshot says so instead of clamping it, which is what this spot announced since F1.3.
    //
    // **And the guard below did not cover this**, even though its comment said it did: the row arrived
    // clamped, so it was always inside the grid and `cell` never returned anything. A phantom cursor
    // would have been painted in the last row.
    let Some(cursor) = view.cursor() else {
        return;
    };
    if cursor.shape == CursorShape::Hidden {
        return;
    }
    let Some(under_cursor) = view.cell(cursor.row, cursor.col) else {
        return;
    };
    let mut slot = metrics.cell(origin, cursor.row, cursor.col);
    // Over a wide character, the cursor covers **the two columns** that character occupies. Covering
    // only one leaves half the letter covered and the other half not, which reads as a drawing error.
    if under_cursor.width == CellWidth::WideHead {
        slot.max.x += metrics.width;
    }
    let color = theme.color(TermColor::Cursor);
    // No blinking, and not for lack of time: a cursor that blinks is a timer, and a timer is work at
    // rest — exactly what this milestone measures and cannot afford to contaminate.
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use term_core::Screen;

    /// A metric with round measurements, so the test's arithmetic reads on its own. It deliberately
    /// does not go through the font: asking it requires a window, and what is tested here is the
    /// arithmetic.
    fn metrics(width: f32, height: f32) -> CellMetrics {
        CellMetrics {
            width,
            height,
            font: FontId::monospace(14.0),
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
