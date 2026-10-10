//! The console's colour emoji: the drawing asked of the atlas, where it goes on screen, and when it
//! spills into the blank cell beside it (**D25**).

use eframe::egui::{Color32, Painter, Rect};
use term_core::{CellWidth, View, ViewCell};

/// Whether a narrow colour emoji is drawn across the cell after it too.
///
/// **An emoji carrying the selector takes one cell**, because that is what the engine — and every
/// program counting with the C library's wcwidth — gives it, and a colour drawing squeezed into one
/// cell is a speck. Widening the grid would move the column every program counts; drawing into the
/// next cell when it is blank moves nothing anyone counts. **D25** carries the alternatives.
///
/// **Blank means a space painted like the emoji's own cell**: same background, same inversion, same
/// selection. Agent programs hide the terminal's cursor and draw their own as an inverted space, and a
/// status bar paints its blanks in a colour of its own; spilling over either would cover what the
/// program meant to show — while inside an inverted bar or a selection, where the neighbour looks
/// like the emoji's own cell, it still spills.
///
/// **Not into the cell the cursor sits on**, which is where it sits while somebody types right after
/// the emoji: the cursor is painted after the emoji and would cover half of it. **Its position only**
/// — not its blink, and not whether a program has hidden it — because a program that hides and shows
/// the cursor around every redraw would otherwise make the emoji change size on every frame.
/// The price, accepted: a program that parks its hidden cursor right after an emoji keeps that emoji
/// in one cell.
///
/// Decided to be asked by the colour pass, the cursor's repaint and the cursor's own slot alike, so
/// the three agree on a cell's size.
pub(super) fn spills(view: &View, cell: &ViewCell, row: u16, column: u16) -> bool {
    if cell.width != CellWidth::Single
        || crate::fonts::presentation_of(cell.c, view.combining_of(cell))
            != crate::fonts::Presentation::Color
    {
        return false;
    }
    let beside = column.saturating_add(1);
    let blank = view.cell(row, beside).is_some_and(|next| {
        next.c == ' '
            && next.bg == cell.bg
            && next.inverse == cell.inverse
            && next.selected == cell.selected
            && next.matched == cell.matched
    });
    blank && !matches!(view.cursor(), Some(cursor) if (cursor.row, cursor.col) == (row, beside))
}

/// Paints that character's color glyph, centered in its slot. Says whether it painted anything.
///
/// **Returning `false` is not a failure: it is Contract 5 of the design.** A character whose
/// presentation asks for color but that no font on the machine paints in color is drawn with whatever
/// the engine gives —its monochrome silhouette, or its tofu box— and nothing more is attempted. The
/// terminal does not draw glyphs the machine does not have.
pub(super) fn paint_in_color(
    painter: &Painter,
    character: crate::fonts::ColorEmoji,
    slot: Rect,
    discoverer: &mut crate::fonts::Discoverer,
) -> bool {
    let Some(glyph) = color_glyph_in(painter.ctx(), character, slot, discoverer) else {
        return false;
    };
    glyph.paint(painter);
    true
}

/// A color glyph ready to be queued: which texture, where on screen, and which part of the texture.
///
/// **Separate from painting it because the status panel has to know before it paints.** Its text is
/// laid out in one go, and the character a color glyph replaces has to be hidden in that layout or
/// shown in it — which is only known once the glyph was or was not found.
pub(crate) struct ColorGlyph {
    atlas: eframe::egui::TextureId,
    drawing: Rect,
    uv: Rect,
}

impl ColorGlyph {
    pub(crate) fn paint(&self, painter: &Painter) {
        painter.image(self.atlas, self.drawing, self.uv, Color32::WHITE);
    }
}

/// That character's color glyph, centered in its slot, if any font on the machine paints it that way.
pub(crate) fn color_glyph_in(
    ctx: &eframe::egui::Context,
    character: impl Into<crate::fonts::ColorEmoji>,
    slot: Rect,
    discoverer: &mut crate::fonts::Discoverer,
) -> Option<ColorGlyph> {
    // **In PHYSICAL pixels, not in points.** The texture is uploaded at the size it will be seen at:
    // asking for it in points would leave it at half resolution on a screen at 200 % and the emoji
    // would come out blurry right where it shows most.
    let per_point = ctx.pixels_per_point();
    let height = (slot.height() * per_point).round() as u32;
    let width = (slot.width() * per_point).round() as u32;
    let (atlas, uv, measurements) = discoverer.color_glyph(ctx, character, height, width)?;
    // **Centered in its slot, and that is why the real texture size is needed**: the rescaling keeps
    // the drawing's proportion, so it almost never fills the exact slot — there is room to spare on one
    // axis. Stretching it to fill would deform the emoji.
    let drawing = Rect::from_center_size(slot.center(), measurements / per_point);
    Some(ColorGlyph { atlas, drawing, uv })
}
