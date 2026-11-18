//! The interface's icons: one Phosphor glyph each (D103), so the title bar, the tabs, the console
//! headers, the sidebar, the Files tree, the Board and Settings share one set.
use eframe::egui::{self, Color32, FontId, Pos2};
use egui_phosphor::regular as glyph;

mod file_kind;
pub(crate) use file_kind::FileKind;
#[cfg(test)]
pub(crate) use tests::{installed as installed_for_tests, painted_icon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Icon {
    Files,
    Panel,
    SplitRight,
    SplitDown,
    Markdown,
    Search,
    Connect,
    Menu,
    Board,
    Minimize,
    Maximize,
    Restore,
    Close,
    Branch,
    Expand,
    Collapse,
    Refresh,
    Folder { open: bool },
    Caret { open: bool },
    CollapseAll,
    File(FileKind),
    Plus,
    Check,
    DotsThree,
    Gear,
    ArrowUpRight,
    CircleHalf,
    NavigationArrow,
    Keyboard,
}

impl Icon {
    fn glyph(self) -> &'static str {
        match self {
            Self::Files => glyph::FOLDERS,
            Self::Panel => glyph::SIDEBAR_SIMPLE,
            Self::SplitRight => glyph::SQUARE_SPLIT_HORIZONTAL,
            Self::SplitDown => glyph::SQUARE_SPLIT_VERTICAL,
            Self::Markdown => glyph::MARKDOWN_LOGO,
            Self::Search => glyph::MAGNIFYING_GLASS,
            Self::Connect => glyph::PLUGS_CONNECTED,
            Self::Menu => glyph::LIST,
            Self::Board => glyph::KANBAN,
            Self::Minimize => glyph::MINUS,
            Self::Maximize => glyph::SQUARE,
            Self::Restore => glyph::COPY,
            Self::Close => glyph::X,
            Self::Branch => glyph::GIT_BRANCH,
            Self::Expand => glyph::ARROWS_OUT_SIMPLE,
            Self::Collapse => glyph::ARROWS_IN_SIMPLE,
            Self::Refresh => glyph::ARROW_CLOCKWISE,
            Self::Folder { open: false } => glyph::FOLDER,
            Self::Folder { open: true } => glyph::FOLDER_OPEN,
            Self::Caret { open: false } => glyph::CARET_RIGHT,
            Self::Caret { open: true } => glyph::CARET_DOWN,
            Self::CollapseAll => glyph::ARROWS_IN_LINE_VERTICAL,
            Self::File(kind) => kind.glyph(),
            Self::Plus => glyph::PLUS,
            Self::Check => glyph::CHECK,
            Self::DotsThree => glyph::DOTS_THREE,
            Self::Gear => glyph::GEAR,
            Self::ArrowUpRight => glyph::ARROW_UP_RIGHT,
            Self::CircleHalf => glyph::CIRCLE_HALF,
            Self::NavigationArrow => glyph::NAVIGATION_ARROW,
            Self::Keyboard => glyph::KEYBOARD,
        }
    }
}

/// Paint `icon` centred on `center`, `size` points across.
///
/// Every Phosphor glyph is drawn on the same em square, and laid out at `size` the galley is that
/// square (advance and row height both one em, the height rounded to a device pixel), so centring
/// the galley centres the square the designer centred the glyph in. Centring each glyph's ink instead would move an open folder against
/// a closed one and pull an asymmetric glyph off its optical centre. epaint snaps the origin to whole
/// pixels itself.
pub(crate) fn paint(painter: &egui::Painter, center: Pos2, icon: Icon, size: f32, color: Color32) {
    let Some(family) = crate::fonts::icon_family(painter.ctx()) else {
        return;
    };
    let galley = painter.layout_no_wrap(icon.glyph().to_owned(), FontId::new(size, family), color);
    painter.galley(center - galley.size() / 2.0, galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) const ALL: &[Icon] = &[
        Icon::Files,
        Icon::Panel,
        Icon::SplitRight,
        Icon::SplitDown,
        Icon::Markdown,
        Icon::Search,
        Icon::Connect,
        Icon::Menu,
        Icon::Board,
        Icon::Minimize,
        Icon::Maximize,
        Icon::Restore,
        Icon::Close,
        Icon::Branch,
        Icon::Expand,
        Icon::Collapse,
        Icon::Refresh,
        Icon::Folder { open: false },
        Icon::Folder { open: true },
        Icon::Caret { open: false },
        Icon::Caret { open: true },
        Icon::CollapseAll,
        Icon::File(FileKind::Plain),
        Icon::Plus,
        Icon::Check,
        Icon::DotsThree,
        Icon::Gear,
        Icon::ArrowUpRight,
        Icon::CircleHalf,
        Icon::NavigationArrow,
        Icon::Keyboard,
    ];

    /// A context with the bundled fonts, at `ppp` device pixels per point, after one pass so the
    /// definitions are live.
    pub(crate) fn installed(ppp: f32) -> egui::Context {
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        ctx.set_pixels_per_point(ppp);
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        ctx
    }

    /// What an icon `shape` is, for tests that look at painted output: its glyph, the colour it was
    /// painted in and the square it occupies. `None` for any shape that is not an icon's text.
    pub(crate) fn painted_icon(shape: &egui::Shape) -> Option<(String, Color32, egui::Rect)> {
        let egui::Shape::Text(text) = shape else {
            return None;
        };
        let glyph = text.galley.text().to_owned();
        if !glyph
            .chars()
            .all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c))
        {
            return None;
        }
        let color = text.galley.job.sections.first()?.format.color;
        Some((
            glyph,
            color,
            egui::Rect::from_min_size(text.pos, text.galley.size()),
        ))
    }

    /// The rectangle, in points, the glyph's raster covers, and whether any of it carries ink.
    fn ink(ctx: &egui::Context, pos: Pos2, galley: &egui::Galley) -> (egui::Rect, bool) {
        let glyph = &galley.rows[0].glyphs[0];
        let uv = glyph.uv_rect;
        let min = pos + glyph.pos.to_vec2() + uv.offset;
        let rect = egui::Rect::from_min_size(min, uv.size);
        let inked = ctx.fonts_mut(|fonts| {
            let image = fonts.image();
            (uv.min[1]..uv.max[1]).any(|y| {
                (uv.min[0]..uv.max[0]).any(|x| image[(usize::from(x), usize::from(y))].a() > 0)
            })
        });
        (rect, inked)
    }

    /// Every icon is one glyph of text, inked, inside its box and centred on the point it was given —
    /// at the sizes the chrome paints and at 1×, 1.25× and 2×.
    #[test]
    fn every_icon_is_one_inked_glyph_centred_in_its_box() {
        let center = egui::pos2(50.0, 50.0);
        for ppp in [1.0, 1.25, 2.0] {
            let ctx = installed(ppp);
            for size in [12.0, 13.0, 15.0, 16.0] {
                for &icon in ALL {
                    let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                        paint(ui.painter(), center, icon, size, Color32::WHITE);
                    });
                    assert_eq!(output.shapes.len(), 1, "{icon:?}");
                    let egui::Shape::Text(text) = &output.shapes[0].shape else {
                        panic!("{icon:?} is not a glyph: {:?}", output.shapes[0].shape);
                    };
                    // The row height is rounded to device pixels, so the square is exact only to
                    // within one of them.
                    assert!(
                        (text.galley.size() - egui::vec2(size, size))
                            .abs()
                            .max_elem()
                            <= 1.0 / ppp + 1e-3,
                        "{icon:?} at {size}×{ppp}: {:?}",
                        text.galley.size()
                    );
                    let square = egui::Rect::from_min_size(text.pos, text.galley.size());
                    // epaint snaps the origin to a device pixel: at most half of one off.
                    let slack = 0.5 / ppp + 1e-3;
                    assert!(
                        (square.center() - center).abs().max_elem() <= slack,
                        "{icon:?} at {size}×{ppp}: {square:?}"
                    );
                    let (raster, inked) = ink(&ctx, text.pos, &text.galley);
                    assert!(inked, "{icon:?} drew nothing");
                    let bounds = egui::Rect::from_center_size(center, egui::vec2(size, size));
                    assert!(
                        bounds.expand(slack).contains_rect(raster),
                        "{icon:?} at {size}×{ppp}: {raster:?} outside {bounds:?}"
                    );
                    output.drop_without_applying_deltas();
                }
            }
        }
    }

    /// A folder that opens and a caret that turns keep their square where it was: the row does not
    /// jump when it is expanded.
    #[test]
    fn paired_glyphs_share_their_square() {
        let ctx = installed(1.25);
        let square = |icon| {
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint(
                    ui.painter(),
                    egui::pos2(20.3, 31.7),
                    icon,
                    15.0,
                    Color32::WHITE,
                );
            });
            let egui::Shape::Text(text) = &output.shapes[0].shape else {
                panic!("not a glyph");
            };
            let pos = text.pos;
            output.drop_without_applying_deltas();
            pos
        };
        assert_eq!(
            square(Icon::Folder { open: false }),
            square(Icon::Folder { open: true })
        );
        assert_eq!(
            square(Icon::Caret { open: false }),
            square(Icon::Caret { open: true })
        );
    }

    /// Where the bundled definitions were never installed the icon is left out rather than asking
    /// egui for a family it does not have, which panics.
    #[test]
    fn a_context_without_the_bundled_fonts_paints_nothing() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint(
                ui.painter(),
                egui::pos2(8.0, 8.0),
                Icon::Close,
                16.0,
                Color32::WHITE,
            );
        });
        assert!(output.shapes.is_empty());
        output.drop_without_applying_deltas();
    }
}
