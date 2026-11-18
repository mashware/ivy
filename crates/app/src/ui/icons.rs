//! The interface's icons: one Phosphor glyph each (D103), so the title bar, the tabs, the console
//! headers, the sidebar, the Files tree, the Board and Settings share one set.
use eframe::egui::{self, Color32, FontId, Pos2};
use egui_phosphor::regular as glyph;

mod file_kind;
pub(crate) use file_kind::{FileKind, legible_or, row_grounds};
#[cfg(test)]
pub(crate) use tests::{installed as installed_for_tests, painted_icon};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Icon {
    Files,
    Panel,
    PanelShown,
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
    Cursor,
    Keyboard,
}

impl Icon {
    fn glyph(self) -> &'static str {
        match self {
            Self::Files => glyph::FOLDERS,
            Self::Panel => glyph::SIDEBAR_SIMPLE,
            Self::PanelShown => glyph::SQUARE_HALF,
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
            Self::Cursor => glyph::CURSOR,
            Self::Keyboard => glyph::KEYBOARD,
        }
    }
}

/// Paint `icon` centred on `center`, `size` points across.
///
/// Every Phosphor glyph is drawn on the same em square, and laid out at `size` the galley is that
/// square (advance and row height both one em, the height rounded to a device pixel), so centring
/// the galley centres the square the designer centred the glyph in. Centring each glyph's ink
/// instead would move an open folder against a closed one and pull an asymmetric glyph off its
/// optical centre. epaint snaps the origin to whole pixels itself. The glyph is laid out without
/// a colour and painted in `color`, so one layout serves every ink an icon is shown in.
pub(crate) fn paint(painter: &egui::Painter, center: Pos2, icon: Icon, size: f32, color: Color32) {
    let family = crate::fonts::ui_icon_family(painter.ctx());
    // Production installs the bundled definitions before its first frame and rebuilds them from
    // that base only (D95), so a missing family there is a defect, not a state: an invisible close
    // button still ends a program. A bare context in a unit test has none and gets no icon.
    debug_assert!(
        cfg!(test) || family.is_some(),
        "the interface's icon family is not installed"
    );
    let Some(family) = family else {
        return;
    };
    let galley = painter.layout_no_wrap(
        icon.glyph().to_owned(),
        FontId::new(size, family),
        Color32::PLACEHOLDER,
    );
    painter.galley(center - galley.size() / 2.0, galley, color);
}

/// A floating surface's close button: egui's own button, so it keeps the surface's spacing, hover
/// and keyboard focus, holding the set's cross instead of a text one.
///
/// A square as tall as the surface's controls with the cross painted on its centre (`paint`): laid
/// out as the button's text, the glyph sat on the text row's baseline and showed above the box's
/// centre (#119).
pub(crate) fn close_button(ui: &mut egui::Ui) -> egui::Response {
    let side = ui.spacing().interact_size.y;
    let response = ui.add(egui::Button::new("").min_size(egui::vec2(side, side)));
    let ink = ui.style().interact(&response).fg_stroke.color;
    paint(ui.painter(), response.rect.center(), Icon::Close, 16.0, ink);
    // The glyph is a private-use codepoint: the button is named for what it does instead.
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Close"));
    response.on_hover_text("Close")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon, every file kind included. The match makes a new variant need an arm here, which
    /// is the reminder to list it too; nothing checks that it was.
    pub(super) fn all() -> Vec<Icon> {
        let mut all = vec![
            Icon::Files,
            Icon::Panel,
            Icon::PanelShown,
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
            Icon::Plus,
            Icon::Check,
            Icon::DotsThree,
            Icon::Gear,
            Icon::ArrowUpRight,
            Icon::CircleHalf,
            Icon::Cursor,
            Icon::Keyboard,
        ];
        all.extend(FileKind::ALL.map(Icon::File));
        for icon in &all {
            match icon {
                Icon::Files
                | Icon::Panel
                | Icon::PanelShown
                | Icon::SplitRight
                | Icon::SplitDown
                | Icon::Markdown
                | Icon::Search
                | Icon::Connect
                | Icon::Menu
                | Icon::Board
                | Icon::Minimize
                | Icon::Maximize
                | Icon::Restore
                | Icon::Close
                | Icon::Branch
                | Icon::Expand
                | Icon::Collapse
                | Icon::Refresh
                | Icon::Folder { .. }
                | Icon::Caret { .. }
                | Icon::CollapseAll
                | Icon::File(_)
                | Icon::Plus
                | Icon::Check
                | Icon::DotsThree
                | Icon::Gear
                | Icon::ArrowUpRight
                | Icon::CircleHalf
                | Icon::Cursor
                | Icon::Keyboard => {}
            }
        }
        all
    }

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
    /// Known by the family it was laid out in, not by its codepoints: the terminal's own
    /// private-use text shares them.
    pub(crate) fn painted_icon(shape: &egui::Shape) -> Option<(String, Color32, egui::Rect)> {
        let egui::Shape::Text(text) = shape else {
            return None;
        };
        let section = text.galley.job.sections.first()?;
        let ours = matches!(&section.format.font_id.family,
            egui::FontFamily::Name(name) if name.as_ref() == crate::fonts::UI_ICONS);
        let glyph = text.galley.text().to_owned();
        if !ours || glyph.is_empty() {
            return None;
        }
        // A colourless layout takes the shape's colour; one laid out in a colour shows that one.
        let color = if section.format.color == Color32::PLACEHOLDER {
            text.fallback_color
        } else {
            section.format.color
        };
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
    /// at the fixed sizes the chrome uses, 8 to 18 points, at 1×, 1.25× and 2×. The icons sized from
    /// the console's cells grow with its font past that range; their squares are centred the same.
    #[test]
    fn every_icon_is_one_inked_glyph_centred_in_its_box() {
        let center = egui::pos2(50.0, 50.0);
        for ppp in [1.0, 1.25, 2.0] {
            let ctx = installed(ppp);
            for size in [8.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0] {
                for icon in all() {
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
                    // The raster carries a pixel of antialiasing around the outline, and at 8
                    // points an arrow's tip reaches the square's edge: one device pixel of slack.
                    assert!(
                        bounds.expand(1.0 / ppp + 1e-3).contains_rect(raster),
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

    /// The ink of `icon` laid out 32 points across at 1×, as a 32×32 grid of alpha.
    fn raster(icon: Icon) -> Vec<Vec<u8>> {
        let ctx = installed(1.0);
        let family = crate::fonts::ui_icon_family(&ctx).unwrap();
        ctx.fonts_mut(|fonts| {
            let galley = fonts.layout_no_wrap(
                icon.glyph().to_owned(),
                FontId::new(32.0, family),
                Color32::WHITE,
            );
            let glyph = &galley.rows[0].glyphs[0];
            let uv = glyph.uv_rect;
            let origin = glyph.pos.to_vec2() + uv.offset;
            let image = fonts.image();
            let mut grid = vec![vec![0u8; 32]; 32];
            for y in uv.min[1]..uv.max[1] {
                for x in uv.min[0]..uv.max[0] {
                    let gx = origin.x as i32 + i32::from(x - uv.min[0]);
                    let gy = origin.y as i32 + i32::from(y - uv.min[1]);
                    if (0..32).contains(&gx) && (0..32).contains(&gy) {
                        grid[gy as usize][gx as usize] =
                            image[(usize::from(x), usize::from(y))].a();
                    }
                }
            }
            grid
        })
    }

    fn column(grid: &[Vec<u8>], x: usize) -> u32 {
        grid.iter().map(|row| u32::from(row[x])).sum()
    }

    fn row(grid: &[Vec<u8>], y: usize) -> u32 {
        grid[y].iter().map(|&a| u32::from(a)).sum()
    }

    /// Splitting right shows a vertical divider and splitting down a horizontal one, as the drawings
    /// they replaced did: read off the glyph's own pixels, not off the name Phosphor gave it.
    #[test]
    fn the_split_glyphs_divide_the_way_they_split() {
        let right = raster(Icon::SplitRight);
        let down = raster(Icon::SplitDown);
        let centre = |grid: &[Vec<u8>]| {
            (
                (15..=16).map(|i| column(grid, i)).max().unwrap(),
                (15..=16).map(|i| row(grid, i)).max().unwrap(),
            )
        };
        let (right_column, right_row) = centre(&right);
        let (down_column, down_row) = centre(&down);
        assert!(right_column > 2 * right_row, "{right_column} {right_row}");
        assert!(down_row > 2 * down_column, "{down_row} {down_column}");
    }

    /// The panel shown fills its side, the right where the panel sits, and the panel folded is an
    /// outline with its divider: the state the two hand-drawn marks told apart, read off the
    /// glyphs' pixels. Measured at 32 points: about 1.8 times the ink on the right.
    #[test]
    fn the_shown_panel_fills_a_side_the_folded_one_does_not() {
        let total = |grid: &[Vec<u8>], xs: std::ops::Range<usize>| -> u32 {
            xs.map(|x| column(grid, x)).sum()
        };
        let shown = raster(Icon::PanelShown);
        let folded = raster(Icon::Panel);
        let (left, right) = (total(&shown, 4..14), total(&shown, 18..28));
        assert!(
            2 * right > 3 * left,
            "the shown panel does not fill its right side: {left} {right}"
        );
        let (all_shown, all_folded) = (total(&shown, 0..32), total(&folded, 0..32));
        assert!(all_shown > all_folded, "{all_shown} {all_folded}");
        let folded_right = total(&folded, 18..28);
        assert!(
            2 * right > 3 * folded_right,
            "the folded panel fills its side as much as the shown one: {right} {folded_right}"
        );
    }
}
