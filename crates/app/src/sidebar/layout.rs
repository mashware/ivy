//! Where Files, the drawer and the Board sit around the console tree (D72).
use super::{Tool, Window};
use crate::{painter::CellMetrics, tiling::Orientation, tree::Node};
use eframe::egui;

pub(crate) const DIVIDER: f32 = 6.0;
pub(crate) const FILES_WIDTH: f32 = 264.0;
pub(crate) const FILES_MINIMUM: f32 = 200.0;
pub(crate) const DRAWER_HEIGHT: f32 = 370.0;
pub(crate) const DRAWER_MINIMUM: f32 = 160.0;
/// The drawer floats inside the window's content, as in `DockB.dc.html`.
pub(crate) const DRAWER_MARGIN: f32 = 6.0;
/// Below this the console tree is never squeezed by Files (D72's dock floor, kept).
const CONSOLE_WIDTH: f32 = 320.0;

pub(crate) struct Layout {
    pub(crate) consoles: egui::Rect,
    pub(crate) files: Option<egui::Rect>,
    pub(crate) files_divider: Option<egui::Rect>,
    /// The drawer's frame, or the Board's cover.
    pub(crate) dock: Option<egui::Rect>,
    /// The strip above the drawer that resizes it.
    pub(crate) divider: Option<egui::Rect>,
}

impl Layout {
    fn bare(area: egui::Rect) -> Self {
        Self {
            consoles: area,
            files: None,
            files_divider: None,
            dock: None,
            divider: None,
        }
    }
}

/// What the console tree needs, along each axis, to keep every console at its floor.
#[derive(Clone, Copy)]
pub(crate) struct Floors {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

/// Files takes the left edge first; the drawer then takes the bottom of what remains. Either is
/// left out, never shrunk below its own minimum, when the consoles could not keep their floor.
pub(crate) fn layout(
    area: egui::Rect,
    files: Option<f32>,
    drawer: Option<f32>,
    maximized: bool,
    floors: Floors,
) -> Layout {
    let mut result = Layout::bare(area);
    let mut rest = area;
    if let Some(preferred) = files {
        let spare = area.width() - DIVIDER - floors.width;
        if spare >= FILES_MINIMUM {
            let width = preferred.clamp(FILES_MINIMUM, spare);
            let (left, right) = area.split_left_right_at_x(area.left() + width);
            let (divider, consoles) = right.split_left_right_at_x(right.left() + DIVIDER);
            result.files = Some(left);
            result.files_divider = Some(divider);
            rest = consoles;
        }
    }
    result.consoles = rest;
    let Some(preferred) = drawer else {
        return result;
    };
    let spare = rest.height() - DIVIDER - floors.height;
    if spare >= DRAWER_MINIMUM && rest.width() > 2.0 * DRAWER_MARGIN {
        let height = preferred.clamp(DRAWER_MINIMUM, spare);
        let (consoles, below) = rest.split_top_bottom_at_y(rest.bottom() - height - DIVIDER);
        let (divider, drawer) = below.split_top_bottom_at_y(below.top() + DIVIDER);
        result.consoles = consoles;
        result.divider = Some(divider);
        result.dock = Some(egui::Rect::from_min_max(
            drawer.min + egui::vec2(DRAWER_MARGIN, 0.0),
            drawer.max - egui::vec2(DRAWER_MARGIN, DRAWER_MARGIN),
        ));
    }
    // Maximized, the drawer covers the consoles where they stand: toggling it resizes no
    // program, as the Board does not.
    if maximized && rest.width() > 2.0 * DRAWER_MARGIN && rest.height() > 2.0 * DRAWER_MARGIN {
        result.divider = None;
        result.dock = Some(rest.shrink(DRAWER_MARGIN));
    }
    result
}

/// The floor of a tree along one axis: siblings stacked along it add up (weighted, as tiling
/// shares them out), siblings across it only need the widest.
fn tree_floor(node: &Node, leaf: f32, stacked: Orientation) -> f32 {
    match node {
        Node::Leaf(_) => leaf,
        Node::Branch {
            orientation,
            children,
        } if *orientation != stacked => children
            .iter()
            .map(|child| tree_floor(&child.node, leaf, stacked))
            .fold(leaf, f32::max),
        Node::Branch { children, .. } => {
            let total: f64 = children.iter().map(|c| f64::from(c.weight.value())).sum();
            let usable = children
                .iter()
                .map(|child| {
                    f64::from(tree_floor(&child.node, leaf, stacked)) * total
                        / f64::from(child.weight.value())
                })
                .fold(0.0_f64, f64::max);
            (usable
                + children.len().saturating_sub(1) as f64
                    * f64::from(crate::tiling::DIVIDER_THICKNESS)) as f32
        }
    }
}

pub(crate) fn partition(window: &Window, area: egui::Rect, metrics: &CellMetrics) -> Layout {
    let state = &window.sidebar;
    if !state.enabled {
        return Layout::bare(area);
    }
    if state.active == Some(Tool::Board) {
        // Cover the console surface without changing the programs' terminal dimensions.
        let mut result = Layout::bare(area);
        result.dock = Some(area);
        return result;
    }
    let inset = 2.0 * crate::painter::CONSOLE_PADDING + 2.0 * window.headers.pane_inset();
    let leaf = egui::vec2(
        crate::panel::MINIMUM_CONSOLE * metrics.width() + inset,
        crate::tiling::MINIMUM_ROWS * metrics.height()
            + crate::header::height(metrics, window.headers)
            + inset,
    );
    let root = &window.active().root;
    let floors = Floors {
        width: tree_floor(root, leaf.x, Orientation::Columns).max(CONSOLE_WIDTH),
        height: tree_floor(root, leaf.y, Orientation::Rows),
    };
    let files = window.explorer.visible().then_some(state.files_width);
    let drawer = matches!(state.active, Some(Tool::Markdown | Tool::Git)).then_some(state.height);
    // Tiling clamps degenerate weights: verify the real allocations. A surface steps aside only
    // when it is what leaves a console under its floor — Files along the width, the drawer along
    // the height — never because the tree does not fit the window anyway.
    let parts = |result: &Layout| root.place(result.consoles, metrics.width()).parts;
    let wide = |result: &Layout| parts(result).iter().all(|r| r.width() + 0.01 >= leaf.x);
    let tall = |result: &Layout| parts(result).iter().all(|r| r.height() + 0.01 >= leaf.y);
    let maximized = state.maximized;
    let probe = |files, drawer| layout(area, files, drawer, maximized, floors);
    let mut files = files;
    if files.is_some() && !wide(&probe(files, None)) && wide(&probe(None, None)) {
        files = None;
    }
    let mut result = probe(files, drawer);
    if drawer.is_some() && !maximized && !tall(&result) && tall(&probe(files, None)) {
        result = probe(files, None);
    }
    result
}
