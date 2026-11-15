//! A shared dock whose partition is also the console dispatch and PTY sizing boundary.
pub(crate) mod probe;

use crate::{Window, painter::CellMetrics, tiling::Orientation, tree::Node};
use eframe::egui;
use std::time::{Duration, Instant};

const RAIL: f32 = 36.0;
const DIVIDER: f32 = 6.0;
const MINIMUM: f32 = 320.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Files,
    Markdown,
    Git,
    Board,
}

impl Tool {
    fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Markdown => "Markdown",
            Self::Git => "Git",
            Self::Board => "Board",
        }
    }
}

pub(crate) struct Sidebar {
    enabled: bool,
    probe: Option<probe::Probe>,
    active: Option<Tool>,
    width: f32,
    context: Option<egui::Context>,
    buttons: Vec<(Tool, egui::Rect)>,
    held: bool,
    resizing: bool,
    blocked_frame: Option<u64>,
    pub(crate) last_dock: Option<egui::Rect>,
    last_consoles: egui::Rect,
    touched: bool,
    frame: Duration,
    total: Duration,
    frames: usize,
    cost: Duration,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            enabled: true,
            probe: None,
            active: None,
            width: 480.0,
            context: None,
            buttons: Vec::new(),
            held: false,
            resizing: false,
            blocked_frame: None,
            last_dock: None,
            last_consoles: egui::Rect::NOTHING,
            touched: false,
            frame: Duration::ZERO,
            total: Duration::ZERO,
            frames: 0,
            cost: Duration::ZERO,
        }
    }
}

impl Sidebar {
    #[cfg(test)]
    pub(crate) fn set_width(&mut self, width: f32) {
        self.width = width.max(MINIMUM);
    }
    pub(crate) fn record(&mut self, duration: Duration) {
        self.touched = true;
        self.frame += duration;
        self.cost += duration;
    }
    pub(crate) fn take_cost(&mut self) -> Duration {
        std::mem::take(&mut self.cost)
    }
    pub(crate) fn close_frame(&mut self) {
        if std::mem::take(&mut self.touched) {
            self.total += std::mem::take(&mut self.frame);
            self.frames += 1;
        }
    }
    pub(crate) fn publish(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        writeln!(
            to,
            "sidebar application: frames={}, total={:?}",
            self.frames, self.total
        )
    }
}

pub(crate) struct Layout {
    pub(crate) consoles: egui::Rect,
    pub(crate) rail: egui::Rect,
    pub(crate) dock: Option<egui::Rect>,
    pub(crate) divider: Option<egui::Rect>,
}

fn layout(area: egui::Rect, active: bool, preferred: f32, floor: f32) -> Layout {
    let right = area.right();
    let x = (right - RAIL).max(area.left());
    let rail = egui::Rect::from_min_max(egui::pos2(x, area.top()), area.max);
    let available = (x - area.left()).max(0.0);
    let width = if active && available >= floor + DIVIDER + MINIMUM {
        preferred.clamp(MINIMUM, available - floor - DIVIDER)
    } else {
        0.0
    };
    let dock = (width > 0.0)
        .then(|| egui::Rect::from_min_max(egui::pos2(x - width, area.top()), rail.left_bottom()));
    let divider = dock.map(|dock| {
        egui::Rect::from_min_max(
            egui::pos2(dock.left() - DIVIDER, area.top()),
            dock.left_bottom(),
        )
    });
    Layout {
        consoles: egui::Rect::from_min_max(
            area.min,
            egui::pos2(divider.map_or(x, |r| r.left()), area.bottom()),
        ),
        rail,
        dock,
        divider,
    }
}

fn tree_floor(node: &Node, leaf: f32) -> f32 {
    match node {
        Node::Leaf(_) => leaf,
        Node::Branch {
            orientation: Orientation::Rows,
            children,
        } => children
            .iter()
            .map(|child| tree_floor(&child.node, leaf))
            .fold(leaf, f32::max),
        Node::Branch { children, .. } => {
            let total: f64 = children.iter().map(|c| f64::from(c.weight.value())).sum();
            let usable = children
                .iter()
                .map(|child| {
                    f64::from(tree_floor(&child.node, leaf)) * total
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
    if !window.sidebar.enabled {
        return Layout {
            consoles: area,
            rail: egui::Rect::NOTHING,
            dock: None,
            divider: None,
        };
    }
    let leaf = crate::panel::MINIMUM_CONSOLE * metrics.width()
        + 2.0 * crate::painter::CONSOLE_PADDING
        + 2.0 * window.headers.pane_inset();
    let floor = tree_floor(&window.active().root, leaf).max(MINIMUM);
    let mut result = layout(
        area,
        window.sidebar.active.is_some(),
        window.sidebar.width,
        floor,
    );
    // Tiling clamps degenerate weights. Verify the actual allocations before reserving a dock.
    if result.dock.is_some()
        && window
            .active()
            .root
            .place(result.consoles, metrics.width())
            .parts
            .iter()
            .any(|r| r.width() + 0.01 < leaf)
    {
        result = layout(area, false, window.sidebar.width, floor);
    }
    result
}

pub(crate) fn active(window: &Window) -> Option<Tool> {
    window.sidebar.active
}

pub(crate) fn select(window: &mut Window, tool: Tool) {
    if window.sidebar.active == Some(tool) {
        return;
    }
    crate::viewer::suspend(window);
    crate::explorer::suspend(window);
    crate::board::suspend(window);
    window.sidebar.active = Some(tool);
    let ctx = window.sidebar.context.clone().unwrap_or_default();
    match tool {
        Tool::Files => crate::explorer::reveal(window),
        Tool::Board => crate::board::reveal(window, &ctx),
        Tool::Markdown | Tool::Git => {}
    }
    window.sidebar.blocked_frame = Some(ctx.cumulative_frame_nr());
    ctx.request_repaint();
}

pub(crate) fn close(window: &mut Window, ctx: &egui::Context) {
    crate::viewer::suspend(window);
    crate::explorer::suspend(window);
    crate::board::suspend(window);
    window.sidebar.active = None;
    window.sidebar.blocked_frame = Some(ctx.cumulative_frame_nr());
    ctx.request_repaint();
}

pub(crate) fn before_keys(window: &mut Window, ui: &mut egui::Ui, metrics: &CellMetrics) -> bool {
    if !window.sidebar.enabled {
        return false;
    }
    let started = Instant::now();
    let ctx = ui.ctx().clone();
    window.sidebar.context = Some(ctx.clone());
    let geometry = crate::tab_bar::geometry(window, ui.max_rect(), metrics);
    window.sidebar.last_dock = geometry.dock;
    window.sidebar.last_consoles = geometry.consoles;
    if window.settings_editor.is_open()
        || window.pane_view.is_modal()
        || window.chrome.guide_blocked(&ctx)
    {
        window.sidebar.record(started.elapsed());
        window.timed_separately += window.sidebar.take_cost();
        return false;
    }
    let frame = ctx.cumulative_frame_nr();
    let over = ctx.input(|i| {
        i.pointer.interact_pos().is_some_and(|p| {
            geometry.rail.contains(p) || geometry.divider.is_some_and(|r| r.contains(p))
        })
    });
    if ctx.input(|i| i.pointer.any_pressed()) && !window.sidebar.held {
        window.sidebar.held = over;
        window.sidebar.resizing = geometry.divider.is_some_and(|r| {
            ctx.input(|i| i.pointer.interact_pos().is_some_and(|p| r.contains(p)))
        });
        if over {
            window.drag_in_progress = None;
            window.link_press_in_progress = None;
            window.last_cell_reported = None;
            window.the_press_went_to_the_program = false;
            window.presses_for_the_program.clear();
            window.where_the_last_press_fell = None;
        }
    }
    let held = window.sidebar.held;
    if window.sidebar.resizing
        && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
    {
        window.sidebar.width = (geometry.rail.left() - pos.x).max(MINIMUM);
        ctx.request_repaint();
    }
    if !ctx.input(|i| i.focused && i.pointer.any_down()) {
        window.sidebar.held = false;
        window.sidebar.resizing = false;
    }
    let mut clicked = None;
    let mut buttons = Vec::new();
    egui::Area::new(egui::Id::new("sidebar-rail"))
        .fade_in(false)
        .constrain(false)
        .fixed_pos(geometry.rail.min)
        .movable(false)
        .order(egui::Order::Foreground)
        .show(&ctx, |ui| {
            ui.set_min_size(geometry.rail.size());
            ui.set_max_size(geometry.rail.size());
            ui.set_clip_rect(geometry.rail);
            ui.painter()
                .rect_filled(geometry.rail, 0, window.theme.panel_background());
            egui::ScrollArea::vertical()
                .id_salt("sidebar-tools")
                .show(ui, |ui| {
                    for tool in [Tool::Files, Tool::Markdown, Tool::Git, Tool::Board] {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::CLICK);
                        buttons.push((tool, rect));
                        let selected = active(window) == Some(tool);
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::SelectableLabel,
                                true,
                                selected,
                                tool.label(),
                            )
                        });
                        let response =
                            response.on_hover_text(if selected && geometry.dock.is_none() {
                                format!(
                                    "Widen the window to show {} beside the consoles.",
                                    tool.label()
                                )
                            } else {
                                tool.label().into()
                            });
                        if selected || response.hovered() {
                            ui.painter()
                                .rect_filled(rect.shrink(1.0), 3, window.theme.divider());
                        }
                        icon(ui, rect.shrink(8.0), tool, window.theme.foreground());
                        if response.clicked() {
                            clicked = Some(tool);
                        }
                    }
                });
        });
    window.sidebar.buttons = buttons;
    if let Some(divider) = geometry.divider {
        ui.painter().rect_filled(divider, 0, window.theme.divider());
        if window.sidebar.resizing
            || ctx.input(|i| i.pointer.hover_pos().is_some_and(|p| divider.contains(p)))
        {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }
    let pointer =
        ctx.input(|i| i.pointer.any_pressed() || i.smooth_scroll_delta != egui::Vec2::ZERO);
    let own = held
        || (over && pointer)
        || clicked.is_some()
        || window.sidebar.blocked_frame == Some(frame);
    // Only host work goes in this span; tool lifecycle and rendering have their own accounting.
    window.sidebar.record(started.elapsed());
    window.timed_separately += window.sidebar.take_cost();
    if let Some(tool) = clicked {
        if active(window) == Some(tool) {
            close(window, &ctx);
        } else {
            match tool {
                Tool::Markdown => crate::viewer::activate(window, true),
                Tool::Git => crate::viewer::activate(window, false),
                _ => select(window, tool),
            }
        }
    }
    if own {
        window.header_input.block_for_settings(frame);
        ctx.input_mut(|i| {
            i.events.retain(|event| {
                matches!(
                    event,
                    egui::Event::ModifiersChanged(_) | egui::Event::WindowFocused(_)
                )
            })
        });
    }
    own
}

pub(crate) fn tool_keys(
    window: &mut Window,
    ui: &mut egui::Ui,
    metrics: &CellMetrics,
    blocked: bool,
    chrome_owned: bool,
) -> (bool, bool, Vec<crate::WindowRequest>) {
    let (explorer_keys, explorer_pointer) = crate::explorer::before_keys(
        window,
        ui,
        metrics,
        blocked || chrome_owned || crate::viewer::is_open(window),
    );
    let (viewer_owned, requests) =
        crate::viewer::keys(window, ui, metrics, blocked || explorer_keys);
    (viewer_owned, explorer_pointer, requests)
}

pub(crate) fn paint_file_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    directory: bool,
    stroke: egui::Stroke,
) {
    if directory {
        let top = rect.min + egui::vec2(2.0, 4.0);
        painter.add(egui::Shape::closed_line(
            vec![
                top,
                top + egui::vec2(6.0, 0.0),
                top + egui::vec2(8.0, 2.0),
                top + egui::vec2(14.0, 2.0),
                top + egui::vec2(14.0, 11.0),
                top + egui::vec2(0.0, 11.0),
            ],
            stroke,
        ));
    } else {
        let paper =
            egui::Rect::from_min_size(rect.min + egui::vec2(3.0, 1.0), egui::vec2(10.0, 14.0));
        painter.rect_stroke(paper, 1.0, stroke, egui::StrokeKind::Inside);
        for y in [6.0, 9.0, 12.0] {
            painter.line_segment(
                [
                    paper.min + egui::vec2(2.0, y - 1.0),
                    paper.min + egui::vec2(8.0, y - 1.0),
                ],
                stroke,
            );
        }
    }
}

fn icon(ui: &egui::Ui, rect: egui::Rect, tool: Tool, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.4, color);
    let painter = ui.painter();
    match tool {
        Tool::Files => paint_file_icon(painter, rect, true, stroke),
        Tool::Markdown => {
            painter.rect_stroke(rect, 0, stroke, egui::StrokeKind::Inside);
            for y in [5.0, 9.0, 13.0] {
                painter.line_segment(
                    [
                        rect.min + egui::vec2(3.0, y),
                        rect.min + egui::vec2(12.0, y),
                    ],
                    stroke,
                );
            }
        }
        Tool::Git => {
            let x = rect.left() + 4.0;
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                stroke,
            );
            painter.line_segment([egui::pos2(x, rect.center().y), rect.right_top()], stroke);
            for p in [
                egui::pos2(x, rect.top()),
                egui::pos2(x, rect.bottom()),
                rect.right_top(),
            ] {
                painter.circle_filled(p, 2.0, color);
            }
        }
        Tool::Board => {
            for x in [0.0, 6.0, 12.0] {
                painter.rect_stroke(
                    egui::Rect::from_min_size(rect.min + egui::vec2(x, 0.0), egui::vec2(4.0, 16.0)),
                    0,
                    stroke,
                    egui::StrokeKind::Inside,
                );
                for y in [3.0, 8.0] {
                    painter.line_segment(
                        [
                            rect.min + egui::vec2(x + 1.0, y),
                            rect.min + egui::vec2(x + 3.0, y),
                        ],
                        stroke,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
