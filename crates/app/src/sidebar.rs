//! Files at the left of the consoles, the review drawer below them and the Board over them: the
//! partition is also the console dispatch and PTY sizing boundary (D72).
mod layout;
pub(crate) mod probe;

use crate::{Window, painter::CellMetrics};
use eframe::egui;
pub(crate) use layout::{DIVIDER, drawer_has_room, partition};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Files,
    /// The drawer tab that shows one file: Markdown formatted, anything else as code.
    Document,
    /// The drawer tab that shows an image a console printed, with its marks.
    Image,
    Git,
    Board,
}

impl Tool {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Document => "File",
            Self::Image => "Image",
            Self::Git => "Git",
            Self::Board => "Board",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    Files,
    Drawer,
}

pub(crate) struct Sidebar {
    enabled: bool,
    probe: Option<probe::Probe>,
    /// The drawer's tab (File, Image or Git) or the Board; Files is independent of it.
    active: Option<Tool>,
    files_width: f32,
    height: f32,
    pub(crate) maximized: bool,
    context: Option<egui::Context>,
    /// Where the title bar painted each tool's button this frame.
    pub(crate) buttons: Vec<(Tool, egui::Rect)>,
    pressed: Option<crate::viewer::StripAction>,
    held: bool,
    resizing: Option<Edge>,
    blocked_frame: Option<u64>,
    pub(crate) last_dock: Option<egui::Rect>,
    pub(crate) last_files: Option<egui::Rect>,
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
            files_width: layout::FILES_WIDTH,
            height: layout::DRAWER_HEIGHT,
            maximized: false,
            context: None,
            buttons: Vec::new(),
            pressed: None,
            held: false,
            resizing: None,
            blocked_frame: None,
            last_dock: None,
            last_files: None,
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
    pub(crate) fn set_files_width(&mut self, width: f32) {
        self.files_width = width.max(layout::FILES_MINIMUM);
    }
    #[cfg(test)]
    pub(crate) fn set_height(&mut self, height: f32) {
        self.height = height.max(layout::DRAWER_MINIMUM);
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
    fn block(&mut self) {
        // Before the first frame there is no input to hold back and no window to repaint.
        if let Some(ctx) = &self.context {
            self.blocked_frame = Some(ctx.cumulative_frame_nr());
            ctx.request_repaint();
        }
    }
}

pub(crate) fn files_visible(window: &Window) -> bool {
    window.explorer.visible()
}

pub(crate) fn toggle_maximize(window: &mut Window) {
    window.sidebar.maximized = !window.sidebar.maximized;
    crate::viewer::release_gesture(window);
    window.sidebar.block();
}

/// Complete releases for presses already sent to a program, even under a newly covered console.
pub(crate) fn consume_input(window: &Window, ctx: &egui::Context) {
    ctx.input_mut(|i| {
        i.events.retain(|event| match event {
            egui::Event::ModifiersChanged(_) | egui::Event::WindowFocused(_) => true,
            egui::Event::PointerButton {
                button,
                pressed: false,
                ..
            } => window
                .presses_for_the_program
                .iter()
                .any(|(_, sent)| sent == button),
            _ => false,
        })
    });
}

/// The drawer's tab or the Board; `None` when neither is open, whatever Files shows.
pub(crate) fn active(window: &Window) -> Option<Tool> {
    window.sidebar.active
}

/// Show a tool. Files opens beside whatever else is open; the drawer's tabs and the Board replace
/// each other.
pub(crate) fn select(window: &mut Window, tool: Tool) {
    if tool == Tool::Files {
        if !files_visible(window) {
            crate::explorer::reveal(window);
            crate::viewer::blur(window);
        }
        window.sidebar.block();
        return;
    }
    if window.sidebar.active == Some(tool) {
        return;
    }
    if tool == Tool::Board || window.sidebar.active == Some(Tool::Board) {
        window.sidebar.maximized = false;
    }
    crate::viewer::suspend(window);
    crate::board::suspend(window);
    window.sidebar.active = Some(tool);
    if tool == Tool::Board {
        let ctx = window.sidebar.context.clone().unwrap_or_default();
        crate::board::reveal(window, &ctx);
    } else {
        crate::explorer::blur(window);
    }
    window.sidebar.block();
}

/// Close the drawer or the Board. Files stays as it is.
pub(crate) fn close(window: &mut Window, ctx: &egui::Context) {
    window.sidebar.maximized = false;
    crate::viewer::suspend(window);
    crate::board::suspend(window);
    window.sidebar.active = None;
    window.sidebar.context.get_or_insert_with(|| ctx.clone());
    window.sidebar.block();
}

pub(crate) fn close_files(window: &mut Window) {
    crate::explorer::suspend(window);
    window.sidebar.block();
}

pub(crate) fn toggle(window: &mut Window, tool: Tool, ctx: &egui::Context) {
    match tool {
        // Files lies under the Board: its button leaves the Board for Files.
        Tool::Files if active(window) == Some(Tool::Board) => {
            close(window, ctx);
            select(window, Tool::Files);
        }
        Tool::Files if files_visible(window) => close_files(window),
        _ if active(window) == Some(tool) => close(window, ctx),
        Tool::Document => crate::viewer::activate(window, true),
        Tool::Git => crate::viewer::activate(window, false),
        Tool::Image => crate::viewer::show_image(window),
        _ => select(window, tool),
    }
}

/// Place Files and the drawer for this frame, then take the gestures that are theirs. A press the
/// title bar took (`chrome_owned`) still places them: a title-bar button that opens or closes Files
/// changes the partition in this same frame, and painting Files from the last frame's place left
/// one frame where the consoles had moved and Files had not.
pub(crate) fn before_keys(
    window: &mut Window,
    ui: &mut egui::Ui,
    metrics: &CellMetrics,
    chrome_owned: bool,
) -> bool {
    if !window.sidebar.enabled {
        return false;
    }
    let started = Instant::now();
    let ctx = ui.ctx().clone();
    window.sidebar.context = Some(ctx.clone());
    let geometry = crate::tab_bar::geometry(window, ui.max_rect(), metrics);
    window.sidebar.last_dock = geometry.dock;
    window.sidebar.last_files = geometry.files;
    window.sidebar.last_consoles = geometry.consoles;
    if chrome_owned
        || window.settings_editor.is_open()
        || window.pane_view.is_modal()
        || window.chrome.guide_blocked(&ctx)
    {
        window.sidebar.record(started.elapsed());
        window.timed_separately += window.sidebar.take_cost();
        return false;
    }
    let frame = ctx.cumulative_frame_nr();
    // The drawer's own buttons change the partition, so they are resolved before final tiling,
    // against the drawer as displayed.
    let drawer = matches!(
        active(window),
        Some(Tool::Document | Tool::Image | Tool::Git)
    )
    .then_some(geometry.dock)
    .flatten();
    let list = crate::explorer::list_rect(window);
    let target = |window: &Window| {
        ctx.input(|i| i.pointer.interact_pos())
            .filter(|p| !list.is_some_and(|r| r.contains(*p)))
            .zip(drawer)
            .and_then(|(p, dock)| crate::viewer::strip_action(window, dock, p))
    };
    if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
        window.sidebar.pressed = target(window);
    }
    if ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
        && let Some(pressed) = window.sidebar.pressed.take()
        && ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
        && target(window) == Some(pressed)
    {
        crate::viewer::apply_strip_action(window, pressed, &ctx);
    }
    let on_list = |p: egui::Pos2| list.is_some_and(|r| r.contains(p));
    let over_divider = |p: egui::Pos2| {
        !on_list(p)
            && (geometry.files_divider.is_some_and(|r| r.contains(p))
                || geometry.divider.is_some_and(|r| r.contains(p)))
    };
    let over = ctx.input(|i| i.pointer.interact_pos().is_some_and(over_divider));
    if ctx.input(|i| i.pointer.any_pressed()) && !window.sidebar.held {
        window.sidebar.held = over;
        let pos = ctx
            .input(|i| i.pointer.interact_pos())
            .filter(|p| over_divider(*p));
        window.sidebar.resizing = pos.and_then(|p| {
            if geometry.files_divider.is_some_and(|r| r.contains(p)) {
                Some(Edge::Files)
            } else if geometry.divider.is_some_and(|r| r.contains(p)) {
                Some(Edge::Drawer)
            } else {
                None
            }
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
    if let Some(edge) = window.sidebar.resizing
        && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
    {
        match (edge, geometry.files, geometry.dock) {
            (Edge::Files, Some(files), _) => {
                window.sidebar.files_width = (pos.x - files.left()).max(layout::FILES_MINIMUM);
            }
            (Edge::Drawer, _, Some(dock)) => {
                let bottom = dock.bottom() + layout::DRAWER_MARGIN;
                window.sidebar.height =
                    (bottom - pos.y - DIVIDER / 2.0).max(layout::DRAWER_MINIMUM);
            }
            _ => {}
        }
        ctx.request_repaint();
    }
    if !ctx.input(|i| i.focused && i.pointer.any_down()) {
        window.sidebar.held = false;
        window.sidebar.resizing = None;
    }
    for (divider, cursor) in [
        (geometry.files_divider, egui::CursorIcon::ResizeHorizontal),
        (geometry.divider, egui::CursorIcon::ResizeVertical),
    ] {
        if let Some(divider) = divider
            && ctx.input(|i| {
                i.pointer
                    .hover_pos()
                    .is_some_and(|p| divider.contains(p) && !on_list(p))
            })
        {
            ctx.set_cursor_icon(cursor);
        }
    }
    match window.sidebar.resizing {
        Some(Edge::Files) => ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal),
        Some(Edge::Drawer) => ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical),
        None => {}
    }
    let pointer =
        ctx.input(|i| i.pointer.any_pressed() || i.smooth_scroll_delta != egui::Vec2::ZERO);
    let own = held || (over && pointer) || window.sidebar.blocked_frame == Some(frame);
    // Only host work goes in this span; tool lifecycle and rendering have their own accounting.
    window.sidebar.record(started.elapsed());
    window.timed_separately += window.sidebar.take_cost();
    if own {
        window.header_input.block_for_settings(frame);
        consume_input(window, &ctx);
    }
    own
}

/// The dividers between the consoles, Files and the drawer, painted after the consoles.
pub(crate) fn paint_dividers(window: &Window, ui: &egui::Ui) {
    let roles = crate::ui::style::roles(ui.ctx(), &window.theme);
    if let Some(divider) = window.sidebar.last_files.map(|files| {
        egui::Rect::from_min_size(files.right_top(), egui::vec2(DIVIDER, files.height()))
    }) {
        ui.painter().rect_filled(divider, 0, roles.base);
        ui.painter().line_segment(
            [divider.left_top(), divider.left_bottom()],
            egui::Stroke::new(1.0, roles.line),
        );
    }
}

pub(crate) fn tool_keys(
    window: &mut Window,
    ui: &mut egui::Ui,
    metrics: &CellMetrics,
    blocked: bool,
    chrome_owned: bool,
) -> (bool, bool, Vec<crate::WindowRequest>) {
    let (explorer_keys, explorer_pointer) =
        crate::explorer::before_keys(window, ui, metrics, blocked || chrome_owned);
    let (viewer_owned, requests) =
        crate::viewer::keys(window, ui, metrics, blocked || explorer_keys);
    (viewer_owned, explorer_pointer, requests)
}

pub(crate) fn paint_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    tool: Tool,
    color: egui::Color32,
) {
    use crate::ui::icons::Icon;
    let icon = match tool {
        Tool::Files => Icon::Files,
        Tool::Document => Icon::Document,
        Tool::Image => Icon::File(crate::ui::icons::FileKind::Image),
        Tool::Git => Icon::Branch,
        Tool::Board => Icon::Board,
    };
    crate::ui::icons::paint(
        painter,
        rect.center(),
        icon,
        rect.width().min(rect.height()),
        color,
    );
}

#[cfg(test)]
mod tests;
