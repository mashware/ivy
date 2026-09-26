//! Color emoji in the status panel.
//!
//! **The panel cannot take the console's route**, which decides character by character inside a grid:
//! its text is laid out by the engine a row at a time, and epaint cannot rasterize color at all
//! (**D25**). So a row carrying a character whose presentation asks for color is laid out with that
//! character **invisible** — a transparent section keeps its advance, so nothing after it moves — and
//! the console's own color glyph is drawn where it landed. The rule deciding which characters go to
//! color and the atlas they are drawn from are the console's, not a copy.

use std::ops::Range;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Align2, Color32, Galley, Painter, Pos2, Rect, vec2};

use super::Row;
use crate::fonts::{Presentation, presentation_of};
use crate::painter::{CellMetrics, ColorGlyph, color_glyph_in};

/// Notes on each row which of its characters go to color. **Once per layout, not per frame**: the
/// rows are remembered between frames, and so is this.
pub(super) fn flag(rows: &mut [Row]) {
    for row in rows {
        row.color_spans = color_spans(&row.text);
    }
}

/// Paints a row's text at `origin` and returns the rectangle it inked. A row with nothing going to
/// color is painted exactly as every row was before this module existed.
pub(super) fn paint_text(
    painter: &Painter,
    origin: Pos2,
    row: &Row,
    metrics: &CellMetrics,
    discoverer: &mut crate::fonts::Discoverer,
    glyphs: &mut Vec<ColorGlyph>,
) -> Rect {
    if row.color_spans.is_empty() {
        return painter.text(
            origin,
            Align2::LEFT_TOP,
            &row.text,
            metrics.font(),
            row.color,
        );
    }
    let chars: Vec<char> = row.text.chars().collect();
    let mut shown = row.color_spans.clone();
    let galley = painter.layout_job(job(&chars, &shown, row.color, metrics));
    // **A character with no color glyph is shown, never left invisible.** Whether one exists is only
    // known once the row is laid out, because where it has to go comes from that layout — so the rare
    // row where one is missing, with no color font or the atlas full, is laid out a second time with
    // that character in ink.
    let before = shown.len();
    shown.retain(|span| {
        let found = slot(&galley, &chars, span, origin, metrics)
            .and_then(|slot| color_glyph_in(painter.ctx(), chars[span.start], slot, discoverer));
        // **Queued rather than drawn**, so the caller draws every one after all of the panel's text:
        // the engine only batches consecutive shapes that share a texture (**D25**).
        match found {
            Some(glyph) => {
                glyphs.push(glyph);
                true
            }
            None => false,
        }
    });
    let galley = if shown.len() == before {
        galley
    } else {
        painter.layout_job(job(&chars, &shown, row.color, metrics))
    };
    let inked = galley.rect.translate(origin.to_vec2());
    painter.galley(origin, galley, row.color);
    inked
}

/// Which characters of a text go to color, each with what is attached to it, as ranges of character
/// indices.
///
/// A text that is all ASCII answers none without consulting the tables, and that is not the shortcut
/// **D25** warns about: the twelve ASCII keycap bases only go to color with a selector behind them,
/// and a selector is not ASCII.
fn color_spans(text: &str) -> Vec<Range<usize>> {
    if text.is_ascii() {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len())
        .filter(|&at| !is_attached(chars[at]))
        .filter_map(|at| {
            let attached = attached_to(&chars, at);
            (presentation_of(chars[at], attached) == Presentation::Color)
                .then(|| at..at + 1 + attached.len())
        })
        .collect()
}

/// The zero-width characters that follow the one at `at` and belong to it.
///
/// **In the console the engine hands these over already stuck to their cell**; here they are the next
/// characters of the string, so this is where they are gathered. The joiner and the tag characters
/// are not among them because the panel never paints them: its sanitizing, by the contract's list of
/// what is shown as itself, turns them into spaces before anything is laid out.
fn attached_to(chars: &[char], at: usize) -> &[char] {
    let rest = &chars[at + 1..];
    let how_many = rest.iter().take_while(|&&c| is_attached(c)).count();
    &rest[..how_many]
}

/// The variation selectors and the keycap's enclosing mark: what can follow an emoji and belong to it.
fn is_attached(c: char) -> bool {
    matches!(c, '\u{fe00}'..='\u{fe0f}' | '\u{20e3}')
}

/// The row as a layout job: every character in `ink`, except the ones inside `hidden`, which are
/// transparent.
fn job(chars: &[char], hidden: &[Range<usize>], ink: Color32, metrics: &CellMetrics) -> LayoutJob {
    let mut job = LayoutJob::default();
    let mut run = String::new();
    let mut run_ink = ink;
    for (at, &c) in chars.iter().enumerate() {
        let this_ink = if hidden.iter().any(|span| span.contains(&at)) {
            Color32::TRANSPARENT
        } else {
            ink
        };
        if this_ink != run_ink && !run.is_empty() {
            job.append(&run, 0.0, TextFormat::simple(metrics.font(), run_ink));
            run.clear();
        }
        run_ink = this_ink;
        run.push(c);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, TextFormat::simple(metrics.font(), run_ink));
    }
    job
}

/// Where the glyph of `span` goes: the room its characters were laid out in, one cell tall.
///
/// **Laid out by index, and checked by character.** epaint pads a cluster that has fewer glyphs than
/// characters, but not one that has more, so past such a cluster the index drifts while staying in
/// range. A shaped cluster stamps its first character on every glyph it produced, so what is attached
/// comes back carrying the base's character — or its own, where epaint padded the cluster. A glyph
/// that is neither gives no slot and the character is shown in ink, as does one with no advance: a
/// ligature's continuation, with nothing of its own to replace.
///
/// **As wide as that room and no wider**, so the drawing never covers the letters beside it; the
/// atlas keeps it apart from the console's drawing of the same character, because it remembers each
/// glyph by the width it was asked at. **And one cell tall**, because a change of height empties the
/// atlas: a panel asking another would empty it on every frame that painted both.
fn slot(
    galley: &Galley,
    chars: &[char],
    span: &Range<usize>,
    origin: Pos2,
    metrics: &CellMetrics,
) -> Option<Rect> {
    let row = galley.rows.first()?;
    let first = row.glyphs.get(span.start)?;
    let last = row.glyphs.get(span.end - 1)?;
    let last_is_ours = last.chr == chars[span.start] || last.chr == chars[span.end - 1];
    if first.chr != chars[span.start] || !last_is_ours || first.advance_width <= 0.0 {
        return None;
    }
    // **The width is the sum of the advances and not the distance between positions**: epaint snaps
    // each glyph's position to the pixel grid on its own, so the distance changes by a pixel with the
    // column the emoji falls in — and every distinct width is one more drawing in the atlas.
    let width: f32 = row.glyphs[span.clone()]
        .iter()
        .map(|glyph| glyph.advance_width)
        .sum();
    let left = origin.x + row.pos.x + first.pos.x;
    Some(Rect::from_min_size(
        Pos2::new(left, origin.y),
        vec2(width, metrics.height()),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::tests::{paint_the_state_with, publish};
    use super::super::{PanelState, Rect};
    use crate::painter::CellMetrics;
    use eframe::egui::{Color32, Context, FontId, Pos2, RawInput, Shape, TextureId};
    use panel_contract::PanelDir;
    use std::time::SystemTime;

    const PLACE: Rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 600.0));

    /// What is queued for a panel with those lines, header off so only they are painted.
    fn queued_for(lines: &[&str], discoverer: &mut crate::fonts::Discoverer) -> Vec<Shape> {
        let lines = serde_json::to_string(lines).expect("the lines as JSON");
        let dir = PanelDir::create().expect("create the directory");
        publish(&dir, &format!(r#"{{"header":false,"lines":{lines}}}"#));
        let mut state = PanelState::default();
        state.refresh(dir.path());
        assert!(
            state.last_understood().is_some(),
            "the test's file was not read"
        );
        paint_the_state_with(&mut state, PLACE, SystemTime::now(), discoverer)
    }

    fn queued(lines: &[&str]) -> Vec<Shape> {
        queued_for(lines, &mut crate::fonts::Discoverer::default())
    }

    /// Every shape in queue order, with the nested ones flattened.
    fn flat(shapes: Vec<Shape>) -> Vec<Shape> {
        shapes
            .into_iter()
            .flat_map(|shape| match shape {
                Shape::Vec(many) => flat(many),
                other => vec![other],
            })
            .collect()
    }

    /// **An image is a mesh over a texture that is not the font's**, which is the only oracle for «came
    /// out in color»: the console's tests count the same thing.
    fn is_image(shape: &Shape) -> bool {
        matches!(shape, Shape::Mesh(mesh) if mesh.texture_id != TextureId::default())
    }

    fn images(shapes: &[Shape]) -> usize {
        shapes.iter().filter(|shape| is_image(shape)).count()
    }

    /// The ink each character of the painted row that contains `needle` was laid out with.
    fn ink_of(shapes: &[Shape], needle: char) -> Color32 {
        shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::Text(text) => {
                    let job = &text.galley.job;
                    let at = job.text.find(needle)?;
                    job.sections
                        .iter()
                        .find(|section| {
                            section.byte_range.start.0 <= at && at < section.byte_range.end.0
                        })
                        .map(|section| section.format.color)
                }
                _ => None,
            })
            .expect("no painted row carries that character")
    }

    /// Where each image was drawn, in queue order.
    fn image_rects(shapes: &[Shape]) -> Vec<Rect> {
        shapes
            .iter()
            .filter(|shape| is_image(shape))
            .filter_map(|shape| match shape {
                Shape::Mesh(mesh) => Some(mesh.calc_bounds()),
                _ => None,
            })
            .collect()
    }

    /// The room, on screen, the painted row that contains `needle` laid that character out in.
    fn box_of(shapes: &[Shape], needle: char) -> Rect {
        shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::Text(text) => {
                    let at = text.galley.text().chars().position(|c| c == needle)?;
                    let row = text.galley.rows.first()?;
                    let glyph = row.glyphs.get(at)?;
                    let top_left =
                        text.pos + row.pos.to_vec2() + eframe::egui::vec2(glyph.pos.x, 0.0);
                    Some(Rect::from_min_size(
                        top_left,
                        eframe::egui::vec2(glyph.advance_width, glyph.line_height),
                    ))
                }
                _ => None,
            })
            .expect("no painted row carries that character")
    }

    fn there_is_color_on_this_machine() -> bool {
        let ctx = Context::default();
        ctx.run_ui(RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        crate::fonts::Discoverer::default()
            .color_glyph(&ctx, '\u{1F525}', 20, 20)
            .is_some()
    }

    #[test]
    fn an_emoji_in_a_panel_line_comes_out_in_color_and_its_outline_does_not() {
        // AC1. Asserted in both scenarios, like the console's: without a color font the emoji still has
        // to come out, through the text path.
        let shapes = flat(queued(&["build \u{1F525} done"]));
        if there_is_color_on_this_machine() {
            assert_eq!(images(&shapes), 1, "the emoji was not drawn as an image");
            assert_eq!(
                ink_of(&shapes, '\u{1F525}'),
                Color32::TRANSPARENT,
                "the engine's outline is still drawn under the image"
            );
            // **Where it lands and how big, against the room the emoji was laid out in**: inside it,
            // so it covers no letter beside it; filling most of it, so it was asked at that room and
            // not at some smaller width; and on the row, not above or below it.
            let room = box_of(&shapes, '\u{1F525}');
            let image = image_rects(&shapes)[0];
            assert!(
                room.left() - 0.5 <= image.left() && image.right() <= room.right() + 0.5,
                "the image spans {:?} and the emoji it replaces was laid out in {room:?}",
                image.x_range()
            );
            // Filling the room in one of its two dimensions: the drawing keeps its proportion, so
            // it stops at the cell's height when the room is wider than the emoji is.
            assert!(
                image.width() > room.width() * 0.75 || image.height() > room.height() * 0.75,
                "the image is {:?} in a room of {:?}: it was asked at a smaller size",
                image.size(),
                room.size()
            );
            assert!(
                room.top() <= image.center().y && image.center().y <= room.bottom(),
                "the image is centered at y = {} and its row spans {:?}",
                image.center().y,
                room.y_range()
            );
        } else {
            assert_eq!(images(&shapes), 0);
            assert_ne!(ink_of(&shapes, '\u{1F525}'), Color32::TRANSPARENT);
        }
    }

    #[test]
    fn the_selectors_decide_as_they_do_in_the_console() {
        // AC2, the three directions of the rule. The two that stay text hold on any machine.
        for text_line in ["\u{1F600}\u{FE0E} text selector", "\u{2601} bare cloud"] {
            let shapes = flat(queued(&[text_line]));
            assert_eq!(images(&shapes), 0, "«{text_line}» went to color");
        }
        if there_is_color_on_this_machine() {
            let shapes = flat(queued(&["\u{2601}\u{FE0F} cloud with the emoji selector"]));
            assert_eq!(
                images(&shapes),
                1,
                "the emoji selector did not take the cloud to color"
            );
            assert_eq!(
                ink_of(&shapes, '\u{FE0F}'),
                Color32::TRANSPARENT,
                "the selector belongs to the emoji and has to be hidden with it"
            );
        }
    }

    #[test]
    fn hiding_the_emoji_moves_nothing_after_it() {
        // AC3. What follows the emoji lands where it lands when the row is painted as plain text, which
        // is how every row was painted before this module.
        let line = "\u{1F525} abc";
        let shapes = flat(queued(&[line]));
        let painted = shapes
            .iter()
            .find_map(|shape| match shape {
                Shape::Text(text) if text.galley.text().contains("abc") => Some(text.clone()),
                _ => None,
            })
            .expect("the row was painted");
        // **The emoji is still in what was laid out**, which is what makes the comparison below
        // mean anything: a row that swapped it for a space would match its own plain layout too.
        assert!(
            painted.galley.text().contains('\u{1F525}'),
            "the emoji was replaced in the laid-out text instead of hidden"
        );
        if there_is_color_on_this_machine() {
            assert_eq!(
                ink_of(&shapes, '\u{1F525}'),
                Color32::TRANSPARENT,
                "the row was not hidden, so it compares the plain layout with itself"
            );
        }
        let ctx = Context::default();
        let mut plain = None;
        let output = ctx.run_ui(RawInput::default(), |ui| {
            let metrics =
                CellMetrics::measure(ui.ctx(), FontId::monospace(crate::DEFAULT_FONT_SIZE));
            let text = painted.galley.text().to_owned();
            plain = Some(
                ui.painter()
                    .layout_no_wrap(text, metrics.font(), Color32::WHITE),
            );
        });
        output.drop_without_applying_deltas();
        let plain = plain.expect("laid out");
        let xs = |galley: &eframe::egui::Galley| -> Vec<f32> {
            galley.rows[0]
                .glyphs
                .iter()
                .map(|glyph| glyph.pos.x)
                .collect()
        };
        assert_eq!(
            xs(&painted.galley),
            xs(&plain),
            "hiding the emoji changed where the rest of the row is drawn"
        );
    }

    #[test]
    fn every_image_is_queued_after_every_letter() {
        // AC4: the engine only batches consecutive shapes that share a texture (D25).
        let shapes = flat(queued(&["\u{1F525} one", "plain", "two \u{2705}"]));
        if !there_is_color_on_this_machine() {
            return;
        }
        assert_eq!(images(&shapes), 2, "the case needs both emoji as images");
        let last_text = shapes
            .iter()
            .rposition(|shape| matches!(shape, Shape::Text(_)))
            .expect("text was painted");
        let first_image = shapes.iter().position(is_image).expect("an image");
        assert!(
            first_image > last_text,
            "an image was queued among the letters: the draw batch splits at every emoji"
        );
    }

    #[test]
    fn with_the_atlas_full_the_emoji_is_still_seen_in_monochrome() {
        // AC5. A character going to color with no glyph to draw is shown in ink, never left invisible.
        if !there_is_color_on_this_machine() {
            return;
        }
        let ctx = Context::default();
        let mut height = 0;
        ctx.run_ui(RawInput::default(), |ui| {
            let metrics =
                CellMetrics::measure(ui.ctx(), FontId::monospace(crate::DEFAULT_FONT_SIZE));
            height = metrics.height().round() as u32;
        })
        .drop_without_applying_deltas();
        let mut discoverer = crate::fonts::Discoverer::default();
        discoverer.fill_the_atlas(&ctx, height);

        let shapes = flat(queued_for(&["build \u{1F525} done"], &mut discoverer));

        assert_eq!(
            images(&shapes),
            0,
            "with the atlas full an image came out anyway"
        );
        assert_ne!(
            ink_of(&shapes, '\u{1F525}'),
            Color32::TRANSPARENT,
            "with the atlas full the emoji is neither drawn nor shown: it vanishes"
        );
    }

    #[test]
    fn a_panel_with_no_emoji_is_painted_as_before() {
        // AC6: one plain text shape per row, each in a single ink, and no image.
        let shapes = flat(queued(&["one", "two · three", "日本語"]));
        assert_eq!(images(&shapes), 0);
        let texts: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Text(text) => Some(text.galley.job.sections.len()),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts,
            vec![1, 1, 1],
            "a row with no emoji was split into sections"
        );
    }

    #[test]
    fn only_characters_with_color_presentation_are_flagged() {
        assert!(super::color_spans("plain ascii # * 1").is_empty());
        assert!(super::color_spans("\u{2601} and \u{1F600}\u{FE0E}").is_empty());
        assert_eq!(super::color_spans("a \u{1F525}"), vec![2..3]);
        assert_eq!(
            super::color_spans("#\u{FE0F}\u{20E3} key"),
            vec![0..3],
            "a keycap is an emoji, and its selector and mark go with it"
        );
    }

    /// Lays `text` out as the panel does, hands the galley to `doctor`, and returns the slot of the
    /// first character going to color.
    fn slot_after(
        text: &str,
        doctor: impl Fn(&mut Vec<eframe::egui::epaint::text::Glyph>),
    ) -> Option<Rect> {
        let ctx = Context::default();
        let mut slot = None;
        ctx.run_ui(RawInput::default(), |ui| {
            let metrics =
                CellMetrics::measure(ui.ctx(), FontId::monospace(crate::DEFAULT_FONT_SIZE));
            let chars: Vec<char> = text.chars().collect();
            let spans = super::color_spans(text);
            let galley =
                ui.painter()
                    .layout_job(super::job(&chars, &spans, Color32::WHITE, &metrics));
            let mut galley = (*galley).clone();
            doctor(&mut std::sync::Arc::make_mut(&mut galley.rows[0].row).glyphs);
            slot = super::slot(&galley, &chars, &spans[0], Pos2::ZERO, &metrics);
        })
        .drop_without_applying_deltas();
        slot
    }

    #[test]
    fn a_glyph_that_is_not_the_expected_character_gives_no_slot() {
        // The control: undoctored, the emoji has its slot.
        assert!(slot_after("ab \u{1F525} c", |_| {}).is_some());
        // A cluster that produced one glyph more than it had characters, before the emoji: every
        // index after it drifts by one while staying in range.
        let drifted = slot_after("ab \u{1F525} c", |glyphs| {
            let extra = glyphs[0];
            glyphs.insert(0, extra);
        });
        assert!(drifted.is_none(), "a drifted index still produced a slot");
        // The same drift over an emoji carrying its selector: the glyph at its last index is the
        // cluster's own and carries the base's character, so only the check on its first index can
        // tell that the slot would land one glyph early.
        assert!(
            slot_after("ab \u{2601}\u{FE0F} c", |_| {}).is_some(),
            "undoctored, the cloud with its selector has no slot: the case below would prove nothing"
        );
        let with_a_selector = slot_after("ab \u{2601}\u{FE0F} c", |glyphs| {
            let extra = glyphs[0];
            glyphs.insert(0, extra);
        });
        assert!(
            with_a_selector.is_none(),
            "a drifted index over an emoji with its selector still produced a slot"
        );
    }

    #[test]
    fn the_panel_painting_an_emoji_first_leaves_the_consoles_drawing_alone() {
        // The atlas is shared. The console asks `☁️` at one cell; the panel lays it out in about two.
        // Whatever the panel painted first, the console's request has to come back as it does on an
        // atlas nobody else touched.
        if !there_is_color_on_this_machine() {
            return;
        }
        let ctx = Context::default();
        let mut cell = (0, 0);
        ctx.run_ui(RawInput::default(), |ui| {
            let metrics =
                CellMetrics::measure(ui.ctx(), FontId::monospace(crate::DEFAULT_FONT_SIZE));
            cell = (
                metrics.height().round() as u32,
                metrics.width().round() as u32,
            );
        })
        .drop_without_applying_deltas();
        let (height, width) = cell;
        let mut shared = crate::fonts::Discoverer::default();
        let painted = flat(queued_for(&["\u{2601}\u{FE0F} cloud"], &mut shared));
        assert_eq!(images(&painted), 1, "the panel did not draw the cloud");

        let after_the_panel = shared.color_glyph(&ctx, '\u{2601}', height, width);
        let untouched =
            crate::fonts::Discoverer::default().color_glyph(&ctx, '\u{2601}', height, width);

        assert_eq!(
            after_the_panel.map(|(_, _, size)| size),
            untouched.map(|(_, _, size)| size),
            "the console's cloud came back at the size the panel asked for"
        );
    }

    #[test]
    fn a_glyph_with_no_advance_gives_no_slot() {
        let continuation = slot_after("ab \u{1F525} c", |glyphs| glyphs[3].advance_width = 0.0);
        assert!(
            continuation.is_none(),
            "a ligature's continuation, with nothing of its own, still produced a slot"
        );
    }
}
