//! The color glyph: rasterizing it out of the machine's emoji fonts, and the one atlas every glyph
//! lives in — the console's and the status panel's alike (**D25**).

use super::*;

/// What the color glyph atlas measures on a side, in pixels.
///
/// **The ceiling of this milestone is a FIXED-size texture, not a counter**, and that is better than what the
/// design asked for: the memory the graphics card dedicates to the emojis is **1 MiB and does not change**, whether
/// one or a thousand appear. At this machine's cell height about eight hundred glyphs fit —17 px tall, 18
/// wide—, many more than a person sees in a session; when it fills, painting continues, but in
/// monochrome by the usual text path.
///
/// It is the same criterion **D8** bounded the output queue with and **D21** the history: an entry that
/// arrives from outside and nobody inside controls carries a ceiling, and the ceiling is said.
pub(super) const ATLAS_SIDE: usize = 512;

/// The size the bitmap is asked of the font with.
///
/// **It is not the cell height on purpose.** `Noto Color Emoji` brings **a single run** of 136×128
/// pixels —checked by running it, not by reading the specification— so asking it for one size or another
/// returns exactly the same image. Asking for the largest of the reasonable ones is the only thing that changes anything in
/// a font that does bring several: the largest is taken and reduced, which loses less than enlarging a
/// small one.
pub(super) const SIZE_TO_RASTERIZE_WITH: f32 = 128.0;

/// Where a glyph is within the atlas and how much it measures.
#[derive(Debug, Clone, Copy)]
pub(super) struct InAtlas {
    /// The piece of the atlas it occupies, in coordinates from 0 to 1 — which is how it is asked for when painting.
    pub(super) uv: egui::Rect,
    /// What it actually measures, in pixels. The painter needs it to center it without deforming it.
    pub(super) measurements: egui::Vec2,
}

/// The color glyphs already rasterized, **all within a single texture**.
///
/// **One texture and not one per glyph, and the difference is measured.** With one texture per emoji, the engine
/// cannot join two in the same drawing batch —its tessellator says so: mixing texture identifiers
/// in the same mesh is an error— so each emoji on the screen splits the batch. Measured with the two
/// consoles dumping emojis against their control in ASCII: **the rest of the frame went from 275 µs to
/// 743 µs**, that is the figure
/// **D3** judges the framework with almost tripled, and for work that is not the framework's. With an atlas,
/// all the emojis on the screen are drawn together. It is what the terminals that are the bar do.
///
/// **The height is a field and not part of the key, and that is a decision with a consequence.** The whole
/// window shares a single metric —it is measured once per pass, with the same size for all the
/// panes—, so two different heights never coexist: they only follow one another, when the desktop zoom
/// changes. Keeping the height in the key would leave the old entries taking up space forever;
/// having it here allows **emptying the atlas whole** as soon as it changes, which is what the design contract
/// requires and what prevents painting an emoji of the previous size in a cell of the new one. **The
/// width is the opposite case and it is in the key**: two widths of one character do coexist, the
/// console's cells and the room the status panel's text engine laid out.
#[derive(Default)]
pub(super) struct Glyphs {
    /// The texture where they all live. **It does not exist until the first one is needed**: a session without
    /// emojis does not ask the graphics card for a single byte.
    pub(super) atlas: Option<egui::TextureHandle>,
    /// What is remembered, failures included: a `None` is "nobody on this machine paints this in color".
    ///
    /// **Remembering the failure is not a detail.** Without it, a character that no color font covers
    /// would traverse the emoji families **on every frame** — and it is exactly the character a broken program
    /// spits in a loop. It is the same reason [`Discoverer`] remembers its own.
    ///
    /// **Keyed by the width it was asked at too**, because two surfaces ask: the console at the cells
    /// the engine gives a character, the status panel at the room its text engine laid out for it.
    /// Keyed by character alone, whichever painted first fixed the drawing for the other.
    pub(super) saved_ones: std::collections::HashMap<(ColorEmoji, u32), Option<InAtlas>>,
    /// Where the free slot of the row being filled starts.
    pub(super) next_spot: [usize; 2],
    /// How tall the row being filled measures. On jumping rows this is advanced.
    pub(super) row_height: usize,
    /// The cell height the ones above are rasterized at, in physical pixels.
    pub(super) height: u32,
    /// Whether it was already said that the atlas filled.
    pub(super) said_the_ceiling: bool,
}

impl Glyphs {
    /// Where that character's color glyph is within the atlas, rasterizing it if needed.
    ///
    /// Returns `None` when the machine has no font that paints it in color, or when the atlas
    /// filled: the caller **has to fall to the text path**, which is Contract 5 of the design —what
    /// is not there is not invented—.
    pub(super) fn texture(
        &mut self,
        ctx: &egui::Context,
        collection: &mut Collection,
        sources: &mut SourceCache,
        which_char: ColorEmoji,
        height: u32,
        max_width: u32,
    ) -> Option<(egui::TextureId, egui::Rect, egui::Vec2)> {
        if height == 0 || max_width == 0 {
            return None;
        }
        // **The cell height changed: what is remembered is no longer valid.** Without this, raising the desktop
        // zoom would leave the emojis of the previous size painted in cells of the new one — and since the
        // atlas is never emptied, for the rest of the process's life.
        if self.height != height {
            self.saved_ones.clear();
            self.next_spot = [0, 0];
            self.row_height = 0;
            self.height = height;
            self.said_the_ceiling = false;
        }
        // **A failure is remembered under width zero, for every width**: whether a face paints the
        // character in color does not depend on how wide it is asked, and asking the families again at
        // each new width is the per-frame walk the memory exists to stop.
        let saved = self.saved_ones.get(&(which_char, max_width));
        let spot = if let Some(saved) = saved.or_else(|| self.saved_ones.get(&(which_char, 0))) {
            *saved
        } else {
            let spot = color_glyph_of(collection, sources, which_char, height, max_width)
                .and_then(|image| self.blit(ctx, image));
            let key = if spot.is_some() { max_width } else { 0 };
            self.saved_ones.insert((which_char, key), spot);
            spot
        };
        // **The atlas exists whenever there is a saved spot**: it is created on pasting the first glyph, and what
        // is saved is precisely the result of having pasted it.
        let spot = spot?;
        Some((self.atlas.as_ref()?.id(), spot.uv, spot.measurements))
    }

    /// Puts an image in the first free slot of the atlas. `None` if it no longer fits.
    pub(super) fn blit(&mut self, ctx: &egui::Context, image: egui::ColorImage) -> Option<InAtlas> {
        let [width, height] = [image.width(), image.height()];
        if width > ATLAS_SIDE || height > ATLAS_SIDE {
            return None;
        }
        // Placement by rows: the row is filled left to right and it jumps to the next when
        // it does not fit. **It leaves gaps and it does not matter**: the glyphs of a session all measure almost the same —the
        // same cell height rasterizes them— so the rows come out even.
        if self.next_spot[0] + width > ATLAS_SIDE {
            self.next_spot = [0, self.next_spot[1] + self.row_height];
            self.row_height = 0;
        }
        if self.next_spot[1] + height > ATLAS_SIDE {
            // **It is said, and only once.** A silent ceiling is indistinguishable from a failure: whoever sees their
            // emojis go monochrome all at once would have no way to know that the terminal knows how to paint them and has
            // run out of room to store them.
            if !self.said_the_ceiling {
                eprintln!(
                    "ivy: the color glyph atlas ({ATLAS_SIDE}x{ATLAS_SIDE} \
                     pixels) has filled. Emojis not already in it will keep coming out in \
                     monochrome."
                );
                self.said_the_ceiling = true;
            }
            return None;
        }
        let spot = self.next_spot;
        let atlas = self.atlas.get_or_insert_with(|| {
            ctx.load_texture(
                "glifos-de-color",
                egui::ColorImage::filled([ATLAS_SIDE, ATLAS_SIDE], egui::Color32::TRANSPARENT),
                // **Linear and not `NEAREST`**: the bitmap comes at 136×128 and the cell measures a fraction
                // of that, so the reduction is done by the CPU resampling; what the card does
                // here is only the final adjustment to physical pixels. With `NEAREST` that adjustment shows as
                // sawtooth on the edge of the drawing.
                egui::TextureOptions::LINEAR,
            )
        });
        // **Only the rectangle of the new glyph, not the whole atlas.** Re-uploading the complete megabyte for
        // each discovered emoji would be half a gig of traffic to the card in a session that reaches the
        // ceiling, to change a few hundred bytes.
        atlas.set_partial(spot, image, egui::TextureOptions::LINEAR);
        self.next_spot[0] = spot[0] + width;
        self.row_height = self.row_height.max(height);
        let side = ATLAS_SIDE as f32;
        Some(InAtlas {
            uv: egui::Rect::from_min_max(
                egui::pos2(spot[0] as f32 / side, spot[1] as f32 / side),
                egui::pos2(
                    (spot[0] + width) as f32 / side,
                    (spot[1] + height) as f32 / side,
                ),
            ),
            measurements: egui::vec2(width as f32, height as f32),
        })
    }

    /// Whether that character is already remembered —including the memory that it is not painted in color.
    ///
    /// **It looks at the height too**, because a change of height empties the whole atlas: without that, the first
    /// glyph after a zoom change would be rasterized **without** its cost entering the stretch.
    pub(super) fn remembers(&self, which_char: ColorEmoji, height: u32, max_width: u32) -> bool {
        self.height == height
            && [max_width, 0]
                .iter()
                .any(|width| self.saved_ones.contains_key(&(which_char, *width)))
    }

    /// How many glyphs are remembered.
    pub(super) fn how_many(&self) -> usize {
        self.saved_ones.len()
    }
}

/// The color image of that character, already rescaled, if any font on the machine paints it that way.
///
/// **It is a free function and not a method, and that solves a real problem.** It needs the system
/// collection and the file cache, which are **two fields** of the discoverer; as a method it would borrow the
/// whole discoverer, and its caller —the glyph cache, which lives **inside** it— already has its
/// part borrowed. The first version got around it by taking the cache out with `mem::take` and returning it afterward, which
/// left a trap: a second entry by that path would have seen an empty cache and the first would have
/// overwritten on return whatever the second saved. Nobody re-enters today, and nothing prevented it.
///
/// **It returns the image and not the face**: the caller stores textures, not fonts. Lending it the open face
/// would force it to carry the file's twenty megs while it uses it, and to speak the font model
/// of a third-party crate for nothing.
pub(super) fn color_glyph_of(
    collection: &mut Collection,
    sources: &mut SourceCache,
    which_char: ColorEmoji,
    height: u32,
    max_width: u32,
) -> Option<egui::ColorImage> {
    // **Only the families the system declares as emoji, and there the search ends.** It does not fall to
    // traversing them all as `look_for` does: a family that is not emoji and still covers an emoji
    // paints it **in monochrome** —on this machine `DejaVu Sans` takes the 😀 with `Noto Color Emoji`
    // installed—, so the net underneath would give exactly what this function exists to avoid.
    let generic_ones: Vec<_> = collection.generic_families(GenericFamily::Emoji).collect();
    for id in generic_ones {
        let Some(faces) = collection.family(id).map(|family| family.fonts().to_vec()) else {
            continue;
        };
        for face in faces {
            let Some(blob) = sources.get(face.source()) else {
                continue;
            };
            let Ok(font) = skrifa::FontRef::from_index(blob.data(), face.index()) else {
                continue;
            };
            let Some(glyph) = glyph_of(&font, which_char) else {
                continue;
            };
            // **Covering it is not enough: it has to bring it in color.** The families the system
            // declares as emoji on this machine are dozens —almost all the symbol `Noto`s— and
            // most are monochrome. Without this check the first the system enumerates would win,
            // which is the defect this project has documented since **D24** and the one
            // Alacritty carries.
            if let Some(image) = rasterize(&font, &blob, face.index(), glyph, height, max_width) {
                return Some(image);
            }
        }
    }
    None
}

/// The glyph that draws it in that face, if the face has one.
///
/// **A keycap is shaped, not mapped.** The emoji font draws it as a ligature of the whole sequence —
/// in `Noto Color Emoji`, `1 U+FE0F U+20E3` shapes to one glyph, the boxed keycap — while the digit's
/// own entry is a thin `1` in a 32×78 ink box on its 136×128 canvas. Shaping only runs here, on a glyph the
/// atlas does not remember yet, never on a frame that finds it. A face whose shaping does not end in
/// a single glyph has no keycap to give, and the next face is asked.
fn glyph_of(font: &skrifa::FontRef<'_>, which_char: ColorEmoji) -> Option<skrifa::GlyphId> {
    if !which_char.keycap {
        return font.charmap().map(which_char.base);
    }
    // A face that does not even map the base and the mark has no keycap to shape: most of the faces
    // the system calls emoji are monochrome symbol fonts, and building a shaper for each is waste.
    let charmap = font.charmap();
    if charmap.map(which_char.base).is_none() || charmap.map('\u{20E3}').is_none() {
        return None;
    }
    let data = harfrust::ShaperData::new(font);
    let shaper = data.shaper(font).build();
    let mut buffer = harfrust::UnicodeBuffer::new();
    buffer.push_str(&format!("{}\u{FE0F}\u{20E3}", which_char.base));
    buffer.guess_segment_properties();
    let shaped = shaper.shape(buffer, harfrust::ShapeOptions::new());
    // **A glyph with no advance is left out before counting**: a font whose ligature takes only the
    // digit and the mark keeps the selector as an empty glyph of its own, and that is still a keycap.
    let mut drawn = shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions())
        .filter(|(_, position)| position.x_advance != 0)
        .map(|(info, _)| info.glyph_id);
    match (drawn.next(), drawn.next()) {
        (Some(only), None) if only != 0 => Some(skrifa::GlyphId::new(only)),
        _ => None,
    }
}

/// The pixels of a bitmap, in RGBA and with **the alpha already premultiplied**. `None` if what it brings
/// is not a color drawing.
///
/// **It is apart from `rasterize` so it can be asserted on, and it is not a detour.** Which of the three
/// paths an emoji travels **is not decided by this project: it is decided by the font the machine has
/// installed**. On this one `Noto Color Emoji` rules, which brings PNG, so the other two would be delivered without
/// any test ever having run them — and they are the two that come first in **F2.7**.
/// Checked by mutating them with the body inside `rasterize`: removing the BGRA-to-RGBA swap, which paints
/// blue what the emoji has as red, left the 647 tests green.
pub(super) fn pixels_of(
    bitmap: &skrifa::bitmap::BitmapData<'_>,
    width: u32,
    height: u32,
) -> Option<image::RgbaImage> {
    match bitmap {
        skrifa::bitmap::BitmapData::Png(bytes) => {
            let mut pixels = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .ok()?
                .into_rgba8();
            // **It is premultiplied BEFORE rescaling, and that is the whole difference between a clean emoji and
            // one with a halo.** Averaging transparent pixels with opaque ones without premultiplying mixes the
            // color of the transparent ones into the edge of the drawing. It is the classic defect of rescaling with
            // alpha, and here it would happen in all of them: an emoji is a round drawing inside an empty box.
            for pixel in pixels.pixels_mut() {
                let alpha = u32::from(pixel.0[3]);
                for channel in &mut pixel.0[..3] {
                    *channel = u8::try_from(u32::from(*channel) * alpha / 255).unwrap_or(*channel);
                }
            }
            Some(pixels)
        }
        skrifa::bitmap::BitmapData::Bgra(bytes) => {
            let mut raw = bytes.to_vec();
            // From BGRA to RGBA. It already comes with the alpha premultiplied, which is what is needed above.
            for pixel in raw.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            image::RgbaImage::from_raw(width, height, raw)
        }
        // An alpha mask is a **monochrome** drawing: it is not a color glyph and it is not painted here.
        skrifa::bitmap::BitmapData::Mask(_) => None,
    }
}

/// The color drawing of a glyph, fitted to the slot it is given: the face's color bitmap if it has
/// one for the glyph, else its color outlines (`COLR`), else nothing.
///
/// Returns `None` when that face does **not** bring the glyph in color: no bitmap and no `COLR`
/// record, or a bitmap that is an **alpha mask**, which is a monochrome drawing and therefore not
/// this. The caller falls to the text path.
///
/// **The bitmap answers first** so that the faces Linux and macOS use — bitmap only — never reach the
/// second path (D25, #176).
pub(super) fn rasterize(
    font: &skrifa::FontRef<'_>,
    data: &fontique::Blob<u8>,
    index: u32,
    glyph: skrifa::GlyphId,
    height: u32,
    max_width: u32,
) -> Option<egui::ColorImage> {
    bitmap_of(font, glyph, height, max_width)
        .or_else(|| colr_of(font, data, index, glyph, height, max_width))
}

/// The color outlines of a glyph, drawn at the size the slot gives them.
///
/// **Only a glyph with a `COLR` record is drawn here.** The library draws whatever it is handed —
/// color outlines, else a bitmap, else the plain outline in the paint color — so a glyph without the
/// record would come out as a white silhouette, and in a monochrome symbol face (most of the families
/// the system calls emoji) it would win over the color face behind it: the defect **D24** closed.
///
/// **It is placed on the em box, never on its ink**: the advance across, ascender to descender down.
/// Fitted by its ink, every emoji of a face would get a scale and a baseline of its own. And it is
/// drawn directly at that size, which `glyph_run` does with its glyph cache off — its default.
///
/// A layer drawn «in the text color» is drawn white: the atlas is not keyed by color.
fn colr_of(
    font: &skrifa::FontRef<'_>,
    data: &fontique::Blob<u8>,
    index: u32,
    glyph: skrifa::GlyphId,
    height: u32,
    max_width: u32,
) -> Option<egui::ColorImage> {
    use skrifa::MetadataProvider as _;
    font.color_glyphs().get(glyph)?;
    let location = skrifa::instance::LocationRef::default();
    let metrics = font.metrics(skrifa::instance::Size::unscaled(), location);
    let advance = font
        .glyph_metrics(skrifa::instance::Size::unscaled(), location)
        .advance_width(glyph)?;
    let em_height = metrics.ascent - metrics.descent;
    if advance <= 0.0 || em_height <= 0.0 {
        return None;
    }
    // Pixels per font unit — the same rule the bitmap path fits by.
    let factor = (height as f32 / em_height).min(max_width as f32 / advance);
    let width = ((advance * factor).round() as u32).clamp(1, max_width);
    let tall = ((em_height * factor).round() as u32).clamp(1, height);
    let (width_px, tall_px) = (u16::try_from(width).ok()?, u16::try_from(tall).ok()?);

    let mut context = vello_cpu::RenderContext::new(width_px, tall_px);
    let mut resources = vello_cpu::Resources::new();
    context.set_paint(vello_cpu::color::palette::css::WHITE);
    let font_data = vello_cpu::peniko::FontData::new(data.clone(), index);
    context
        .glyph_run(&mut resources, &font_data)
        .font_size(f32::from(metrics.units_per_em) * factor)
        .fill_glyphs(std::iter::once(vello_cpu::Glyph {
            id: glyph.to_u32(),
            x: 0.0,
            y: metrics.ascent * factor,
        }));
    context.flush();
    let mut pixmap = vello_cpu::Pixmap::new(width_px, tall_px);
    context.render(&mut pixmap, &mut resources);
    Some(egui::ColorImage::from_rgba_premultiplied(
        [width as usize, tall as usize],
        pixmap.data_as_u8_slice(),
    ))
}

/// The color bitmap of a glyph, rescaled to fit in the slot it is given.
fn bitmap_of(
    font: &skrifa::FontRef<'_>,
    glyph: skrifa::GlyphId,
    height: u32,
    max_width: u32,
) -> Option<egui::ColorImage> {
    let bitmap = skrifa::bitmap::BitmapStrikes::new(font)
        .glyph_for_size(skrifa::instance::Size::new(SIZE_TO_RASTERIZE_WITH), glyph)?;
    let pixels = pixels_of(&bitmap.data, bitmap.width, bitmap.height)?;
    if pixels.width() == 0 || pixels.height() == 0 {
        return None;
    }
    // **The factor is the SMALLER of the two and not the height's, even though the contract speaks of the height.**
    // The two at once —"at the cell height" and "without invading the one next to it"— are only met this way: the
    // bitmap of `Noto Color Emoji` is wider than it is tall (136×128), so scaling it by the height
    // leaves it **wider than the two columns** the engine reserved for it, and the emoji would eat the edge
    // of its neighbor. Measured: 18 px wide for a cell 17 px tall.
    let factor =
        (height as f32 / pixels.height() as f32).min(max_width as f32 / pixels.width() as f32);
    let final_width = ((pixels.width() as f32 * factor).round() as u32).clamp(1, max_width);
    let final_height = ((pixels.height() as f32 * factor).round() as u32).clamp(1, height);
    let shrunk = image::imageops::resize(
        &pixels,
        final_width,
        final_height,
        // **Triangular and not Lanczos.** It is reduced from 136 px to seventeen: at that scale the two give a
        // result that cannot be told apart by looking, and the triangular one costs half. The cost is paid once
        // per character, but it is paid **inside a frame**.
        image::imageops::FilterType::Triangle,
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [final_width as usize, final_height as usize],
        shrunk.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Google's `COLR` version 1 test glyphs: outlines and a `COLR` table, no colour bitmap — the shape
    /// of `Segoe UI Emoji`, on a machine whose own emoji font is a bitmap one.
    fn colr_fixture() -> fontique::Blob<u8> {
        let bytes = include_bytes!("../../tests/fixtures/colr/colr_1.ttf");
        fontique::Blob::new(std::sync::Arc::new(bytes.to_vec()))
    }

    fn drawn(
        blob: &fontique::Blob<u8>,
        glyph: u32,
        height: u32,
        max_width: u32,
    ) -> Option<egui::ColorImage> {
        let font = skrifa::FontRef::from_index(blob.data(), 0).expect("the fixture opens");
        rasterize(
            &font,
            blob,
            0,
            skrifa::GlyphId::new(glyph),
            height,
            max_width,
        )
    }

    fn assert_near(
        image: &egui::ColorImage,
        (x, y): (usize, usize),
        expected: [u8; 4],
        what: &str,
    ) {
        let got = image.pixels[y * image.size[0] + x].to_array();
        let close = got.iter().zip(expected).all(|(g, e)| g.abs_diff(e) <= 2);
        assert!(
            close,
            "{what}: pixel ({x},{y}) is {got:?}, expected about {expected:?}"
        );
    }

    #[test]
    fn a_colr_glyph_is_drawn_in_its_palette_colours_premultiplied() {
        // Glyph 84 is a cross filled blue at alpha 127, with a narrower, taller cross behind it
        // filled orange (255, 165, 0) at alpha 178 — read from the paint graph `ttf-parser`'s own
        // test spells out. Premultiplied, the orange alone is (178, 115, 0, 178); where the blue
        // lies over it, blue + orange × (1 − 127/255) = (89, 58, 127, 217). A drawing that only
        // asserted «not empty» would pass a white silhouette, which is what the library draws
        // when it is handed a glyph and no colour.
        let blob = colr_fixture();
        let image = drawn(&blob, 84, 100, 100).expect("a COLR glyph with no bitmap gave no image");
        assert_eq!(
            image.size,
            [100, 95],
            "not fitted on the em box (advance 1000, ascender 950)"
        );
        assert_near(
            &image,
            (50, 10),
            [178, 115, 0, 178],
            "the orange layer alone",
        );
        assert_near(
            &image,
            (50, 50),
            [89, 58, 127, 217],
            "the blue layer over the orange one",
        );
        assert_near(&image, (10, 50), [0, 0, 0, 0], "outside both layers");
    }

    #[test]
    fn a_colr_glyph_takes_its_em_box_fitted_to_the_slot_and_not_its_ink() {
        // The em box is 1000 × 950 units; a 17-pixel cell two columns of 9 wide fits it at
        // 17/950 per unit, so 18 × 17. Fitted by its ink — a cross far narrower than its advance —
        // it would come out another size, and every emoji of the face a size of its own.
        let blob = colr_fixture();
        let image = drawn(&blob, 84, 17, 18).expect("a COLR glyph with no bitmap gave no image");
        assert_eq!(image.size, [18, 17]);
    }

    #[test]
    fn a_glyph_with_no_colour_outlines_is_not_drawn() {
        // Glyph 3 has an outline and no `COLR` record. The library would draw its outline in the
        // paint colour — a white silhouette that would win over the colour face behind it.
        let blob = colr_fixture();
        assert!(
            drawn(&blob, 3, 17, 18).is_none(),
            "a glyph with no colour drawing came out as an image"
        );
    }

    /// The premise the white context paint of `colr_of` rests on: the font Windows asks for emoji
    /// draws none of these in «the text colour» (palette index `0xFFFF`). It is the only place that
    /// font exists to be read, so it runs on Windows only. If it fails, the atlas has to learn the
    /// text colour, which is follow-up work and not a reason to loosen this.
    #[cfg(windows)]
    #[test]
    fn segoe_ui_emoji_draws_no_emoji_in_the_text_colour() {
        use skrifa::MetadataProvider as _;
        use skrifa::color::{Brush, ColorPainter, CompositeMode, Transform};

        #[derive(Default)]
        struct Fills {
            seen: usize,
            in_text_colour: usize,
        }
        impl Fills {
            fn note(&mut self, indexes: impl Iterator<Item = u16>) {
                for index in indexes {
                    self.seen += 1;
                    if index == 0xFFFF {
                        self.in_text_colour += 1;
                    }
                }
            }
        }
        impl ColorPainter for Fills {
            fn push_transform(&mut self, _: Transform) {}
            fn pop_transform(&mut self) {}
            fn push_clip_glyph(&mut self, _: skrifa::GlyphId) {}
            fn push_clip_box(&mut self, _: skrifa::raw::types::BoundingBox<f32>) {}
            fn pop_clip(&mut self) {}
            fn push_layer(&mut self, _: CompositeMode) {}
            fn fill(&mut self, brush: Brush<'_>) {
                match brush {
                    Brush::Solid { palette_index, .. } => self.note(std::iter::once(palette_index)),
                    Brush::LinearGradient { color_stops, .. }
                    | Brush::RadialGradient { color_stops, .. }
                    | Brush::SweepGradient { color_stops, .. } => {
                        self.note(color_stops.iter().map(|stop| stop.palette_index))
                    }
                }
            }
        }

        let windows = std::env::var("WINDIR").expect("WINDIR is set on Windows");
        let path = std::path::Path::new(&windows)
            .join("Fonts")
            .join("seguiemj.ttf");
        let bytes = std::fs::read(&path).expect("Segoe UI Emoji is installed with Windows");
        let font = skrifa::FontRef::new(&bytes).expect("Segoe UI Emoji opens");
        for emoji in ['\u{1F600}', '\u{1F44D}', '\u{2601}'] {
            let glyph = font.charmap().map(emoji).expect("Segoe UI Emoji maps it");
            let colour = font
                .color_glyphs()
                .get(glyph)
                .unwrap_or_else(|| panic!("{emoji:?} has no COLR record in Segoe UI Emoji"));
            let mut fills = Fills::default();
            colour
                .paint(skrifa::instance::LocationRef::default(), &mut fills)
                .expect("its paint graph walks");
            assert!(
                fills.seen > 0,
                "{emoji:?}: the walk saw no fill, so it proved nothing"
            );
            assert_eq!(
                fills.in_text_colour, 0,
                "{emoji:?} draws {} of its {} fills in the text colour, which the atlas paints white",
                fills.in_text_colour, fills.seen
            );
        }
    }
}
