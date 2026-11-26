//! Integrated window controls. All commands are sent through the native viewport.
use crate::{Window, WindowRequest, painter::CellMetrics, tiling::Orientation};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, Vec2, ViewportCommand};
use std::time::{Duration, Instant};

/// The title bar's partition for this window, shared by its painting, its claim on the pointer
/// and the tab strip.
pub(crate) fn strip(window: &Window, area: Rect) -> strip::Strip {
    let sizes = window.chrome.title_sizes;
    strip::within(
        area,
        sizes.bar_height(),
        sizes.tab_height(),
        connection::compact_reserve,
    )
}

/// Whether the window was opened by a person or by the measurement bench: a measured run checks
/// for no updates and fades nothing. A type rather than a `bool` because the argument it replaced
/// meant the opposite.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Run {
    Interactive,
    Measured,
}

#[derive(Default, Debug)]
pub(crate) struct Chrome {
    enabled: bool,
    pub(crate) agent_preferences: crate::agent_signals::Enabled,
    pub(crate) title_sizes: typography::Sizes,
    zoom_probe: bool,
    menu: bool,
    menu_scroll: f32,
    menu_selected: Option<usize>,
    guide: connection::Guide,
    updates: updates::Dialog,
    held: bool,
    /// Passes in a row that had to move macOS's window buttons back.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    buttons_moved: u8,
    blocked_frame: Option<u64>,
    cost: Duration,
    frame: Duration,
    total: Duration,
    frames: usize,
    view_visible: usize,
    view_hidden: usize,
}

impl Chrome {
    pub(crate) fn enable(&mut self) {
        self.enabled = true;
    }
    pub(crate) fn start(&mut self, ctx: &egui::Context, run: Run) {
        self.enable();
        let interactive = run == Run::Interactive;
        // The focus green fades only where nobody is measuring: the frames a fade wakes would be
        // published as the framework's cost (D13).
        crate::pane_view::allow_focus_fades(ctx, interactive);
        if interactive {
            self.updates.start(ctx);
        }
    }
    pub(crate) fn set_zoom_probe(&mut self, value: bool) {
        self.zoom_probe = value;
    }
    pub(crate) fn record_view(&mut self, visible: usize, hidden: usize) {
        self.view_visible = self.view_visible.max(visible);
        self.view_hidden = self.view_hidden.max(hidden);
    }
    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }
    pub(crate) fn guide_blocked(&self, ctx: &egui::Context) -> bool {
        self.guide.blocked(ctx) || self.updates.blocked(ctx)
    }
    pub(crate) fn record_work(&mut self, cost: Duration) {
        self.cost += cost;
        self.frame += cost;
    }
    pub(crate) fn take_cost(&mut self) -> Duration {
        std::mem::take(&mut self.cost)
    }
    pub(crate) fn close_frame(&mut self) {
        if self.enabled {
            self.total += self.frame;
            self.frames += 1;
            self.frame = Duration::ZERO;
        }
    }
    pub(crate) fn publish(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        writeln!(
            to,
            "chrome regime: {}",
            if !self.enabled {
                "historical"
            } else if self.zoom_probe {
                "integrated-zoom"
            } else {
                "integrated"
            }
        )?;
        writeln!(
            to,
            "chrome view: visible={}, hidden={}",
            self.view_visible, self.view_hidden
        )?;
        writeln!(
            to,
            "integrated chrome: enabled={}, frames={}, total={:?}",
            self.enabled, self.frames, self.total
        )
    }
}

pub(crate) use crate::opening::chrome_viewport as viewport;

mod branding;
mod connection;
mod menu;
pub(crate) mod strip;
mod tools;
mod traffic_lights;
pub(crate) mod typography;
mod updates;
pub(crate) use branding::native_options;
pub(crate) use connection::{begin_pass, prepare};

#[derive(Clone, Copy)]
enum Action {
    Tool(crate::sidebar::Tool),
    SplitRight,
    SplitDown,
    Search,
    Connect,
    Updates(bool),
    Menu,
    Preferences,
    Usage,
    Shortcuts,
    Minimize,
    Maximize,
    Close,
}

fn button(ui: &egui::Ui, rect: Rect, label: &str, hint: &str, bg: Color32, fg: Color32) -> bool {
    button_of(ui, rect, label, hint, bg, fg, false).0
}

/// A title-bar button; `on` keeps the hover fill while the tool it opens is shown. Returns
/// whether it was clicked and its response, for the tool buttons' hover text.
fn button_of(
    ui: &egui::Ui,
    rect: Rect,
    label: &str,
    hint: &str,
    bg: Color32,
    fg: Color32,
    on: bool,
) -> (bool, Option<egui::Response>) {
    if !rect.is_positive() {
        return (false, None);
    }
    // The viewport state may change in the same event batch as a click. Keep its identity
    // stable when the tooltip changes from maximizing to restoring the window.
    let identity = if hint == "Restore window" {
        "Maximize window"
    } else {
        hint
    };
    let response = ui
        .interact(
            rect,
            ui.id().with(("integrated-chrome", identity)),
            Sense::CLICK,
        )
        .on_hover_text(hint);
    if response.hovered() || on {
        ui.painter().rect_filled(rect.shrink(2.0), 4.0, bg);
    }
    paint_button_icon(&ui.painter_at(rect), rect, label, hint, fg);
    (response.clicked(), Some(response))
}

pub(crate) fn paint_button_icon(
    p: &egui::Painter,
    rect: Rect,
    label: &str,
    hint: &str,
    fg: Color32,
) {
    use crate::ui::icons::Icon;
    let c = rect.center();
    let (icon, size) = match hint {
        "Split right" => (Icon::SplitRight, 15.0),
        "Split down" => (Icon::SplitDown, 15.0),
        "Menu" => (Icon::Menu, 15.0),
        "Connect agents" => {
            connection::paint_icon(p, rect, fg);
            return;
        }
        _ if let Some(tool) = tools::tool_of(hint) => {
            crate::sidebar::paint_icon(
                p,
                Rect::from_center_size(c, egui::vec2(15.0, 15.0)),
                tool,
                fg,
            );
            return;
        }
        "Close window" => (Icon::Close, 12.0),
        "Minimize window" => (Icon::Minimize, 12.0),
        "Maximize window" => (Icon::Maximize, 12.0),
        "Restore window" => (Icon::Restore, 12.0),
        _ => {
            p.text(
                c,
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(12.0),
                fg,
            );
            return;
        }
    };
    crate::ui::icons::paint(p, c, icon, size, fg);
}

/// Opens the title bar's menu, or its connection guide through the guide's own door, for a test
/// outside the title bar that needs one of them in front.
#[cfg(test)]
pub(crate) fn open_for_tests(window: &mut Window, ctx: &egui::Context, guide: bool) {
    if guide {
        connection::open(window, ctx, false);
    } else {
        window.chrome.menu = true;
    }
}

/// Whether the title bar holds a modal surface — its menu or a dialog — that dims the consoles.
pub(crate) fn is_modal(window: &Window, ctx: &egui::Context) -> bool {
    window.chrome.menu || window.chrome.guide_blocked(ctx) || window.usage.blocks(ctx)
}

/// Paint and claim the titlebar before settings and terminal input dispatch.
pub(crate) fn before_keys(window: &mut Window, ui: &mut egui::Ui, metrics: &CellMetrics) -> bool {
    if !window.chrome.enabled {
        return false;
    }
    let started = Instant::now();
    let ctx = ui.ctx().clone();
    let area = ui.max_rect();
    let geometry = strip(window, area);
    let row = geometry.row;
    let roles = crate::ui::style::roles(&ctx, &window.theme);
    let fg = roles.tx2;
    let hover = roles.hover;
    ui.painter().rect_filled(row, 0.0, roles.chrome);
    // Inside the row, so the active tab, painted later down to the row's edge, covers it.
    ui.painter().line_segment(
        [
            row.left_bottom() - Vec2::new(0.0, 0.5),
            row.right_bottom() - Vec2::new(0.0, 0.5),
        ],
        Stroke::new(1.0, roles.line),
    );
    let mut action = None;
    let compact = geometry.compact();
    if let Some(logo) = geometry.logo {
        branding::paint_logo(ui, logo);
    }
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if !compact {
        let slots = strip::slots(row);
        let mut buttons = vec![
            (slots.split_right, "Split right", Action::SplitRight),
            (slots.split_down, "Split down", Action::SplitDown),
            (slots.connect, "Connect agents", Action::Connect),
            (slots.menu, "Menu", Action::Menu),
        ];
        if let Some([minimize, maximize, close]) = slots.window {
            buttons.push((minimize, "Minimize window", Action::Minimize));
            buttons.push((
                maximize,
                if maximized {
                    "Restore window"
                } else {
                    "Maximize window"
                },
                Action::Maximize,
            ));
            buttons.push((close, "Close window", Action::Close));
        }
        for (rect, hint, cmd) in buttons {
            if rect.left() >= geometry.left_end && button(ui, rect, "", hint, hover, fg) {
                action = Some(cmd);
            }
        }
        action = tools::show(window, ui, &geometry, hover, fg).or(action);
        for x in slots.separators.into_iter().filter(|x| x.is_finite()) {
            ui.painter().line_segment(
                [
                    egui::pos2(x, row.center().y - 8.0),
                    egui::pos2(x, row.center().y + 8.0),
                ],
                Stroke::new(1.0, roles.line2),
            );
        }
    }
    if compact {
        let menu = Rect::from_min_max(
            egui::pos2(
                (row.right() - strip::COMPACT_MENU).max(row.left()),
                row.top() + 7.0,
            ),
            egui::pos2(row.right(), (row.bottom() - 7.0).max(row.top() + 7.0)),
        );
        if button(ui, menu, "", "Menu", hover, fg) {
            action = Some(Action::Menu);
        }
    }
    connection::compact_button(window, ui, row);
    updates::indicator(window, ui, row);
    // The tabs and their add button are the tab strip's targets; the rest of the row drags.
    let tabs = crate::tab_bar::occupied(geometry.tabs, window.tabs.len());
    let drag_left = tabs.map_or(geometry.left_end, |tabs| tabs.right());
    let drag = Rect::from_min_max(
        egui::pos2(drag_left, row.top()),
        egui::pos2(geometry.right_start.max(drag_left), row.bottom()),
    );
    // The air above the tabs belongs to the bar too: the claim below swallows a press anywhere in
    // the row outside the tabs, so a strip nobody answers would be a dead spot.
    let above = tabs.map(|tabs| {
        Rect::from_min_max(
            egui::pos2(tabs.left(), row.top()),
            egui::pos2(tabs.right(), tabs.top()),
        )
    });
    for (rect, id) in [(Some(drag), "chrome-drag"), (above, "chrome-drag-above")] {
        let Some(rect) = rect.filter(|rect| rect.is_positive()) else {
            continue;
        };
        let response = ui.interact(rect, ui.id().with(id), Sense::click_and_drag());
        if response.double_clicked() {
            action = Some(Action::Maximize);
        } else if response.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
    }
    if window.settings_editor.is_open()
        || window.pane_view.is_modal()
        || window.usage.blocks(&ctx)
        || window.chrome.guide_blocked(&ctx)
    {
        action = None;
        window.chrome.menu = false;
    }
    if matches!(action, Some(Action::Menu)) {
        window.chrome.menu = !window.chrome.menu;
        window.chrome.menu_scroll = 0.0;
        window.chrome.menu_selected = None;
        action = None;
        ctx.request_repaint();
    }
    let (menu_rect, menu_action) = menu::show(window, ui, row, area, compact);
    action = menu_action.or(action);
    let frame = ctx.cumulative_frame_nr();
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    let over = pointer.is_some_and(|p| {
        (row.contains(p) && !tabs.is_some_and(|tabs| tabs.contains(p)))
            || (window.chrome.menu && menu_rect.contains(p))
    });
    let gesture = ctx.input(|i| {
        i.pointer.any_pressed() || i.pointer.any_released() || i.smooth_scroll_delta != Vec2::ZERO
    });
    let edge = pointer.and_then(|p| resize_direction(area, p));
    if !cfg!(target_os = "macos")
        && ui.is_enabled()
        && !maximized
        && !window.settings_editor.is_open()
        && let Some(edge) = edge
        && ctx.input(|i| i.pointer.primary_pressed())
    {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(edge));
    }
    let menu_owned = window.chrome.menu;
    if menu_owned
        && (ctx.input(|i| i.key_pressed(egui::Key::Escape))
            || (ctx.input(|i| i.pointer.primary_pressed()) && !over))
    {
        window.chrome.menu = false;
    }
    let owned = window.chrome.guide_blocked(&ctx)
        || menu_owned
        || ((over || edge.is_some()) && gesture)
        || window.chrome.held
        || window.chrome.blocked_frame == Some(frame);
    if owned {
        window.chrome.blocked_frame = Some(frame);
        window.chrome.held = ctx.input(|i| i.focused && i.pointer.any_down());
        window.header_input.block_for_settings(frame);
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                matches!(
                    e,
                    egui::Event::ModifiersChanged(_)
                        | egui::Event::WindowFocused(_)
                        | egui::Event::PointerButton { pressed: false, .. }
                )
            })
        });
    }
    let elapsed = started.elapsed();
    window.chrome.record_work(elapsed);
    if window.settings_editor.is_open()
        || window.pane_view.is_modal()
        || window.usage.blocks(&ctx)
        || window.chrome.guide_blocked(&ctx)
        || !ui.is_enabled()
        || (crate::board::blocked(window, &ctx)
            && !window.board.is_open()
            && !matches!(
                action,
                Some(Action::Minimize | Action::Maximize | Action::Close)
            ))
    {
        return owned;
    }
    if action.is_some() && !matches!(action, Some(Action::Menu)) {
        window.chrome.menu = false;
    }
    let board_handoff = window.board.is_open()
        && matches!(
            action,
            Some(
                Action::Preferences
                    | Action::Usage
                    | Action::Shortcuts
                    | Action::Connect
                    | Action::Updates(_)
                    | Action::Search
            )
        );
    if board_handoff {
        crate::board::dismiss_for_chrome(window, &ctx);
    }
    let dispatch = Instant::now();
    let inline = matches!(
        action,
        Some(
            Action::Search
                | Action::Minimize
                | Action::Maximize
                | Action::Close
                | Action::Connect
                | Action::Updates(_)
        )
    );
    match action {
        Some(Action::Tool(tool)) => crate::sidebar::toggle(window, tool, &ctx),
        Some(Action::SplitRight) => window.apply(
            WindowRequest::Split(Orientation::Columns),
            &ctx,
            area,
            metrics,
        ),
        Some(Action::SplitDown) => {
            window.apply(WindowRequest::Split(Orientation::Rows), &ctx, area, metrics)
        }
        Some(Action::Search) => {
            let focus = window.active().focus;
            if let Some(pane) = window
                .active_mut()
                .panes_mut()
                .into_iter()
                .find(|p| p.name == focus)
            {
                crate::search::toggle_on(&mut pane.search, &mut pane.screen);
            }
        }
        Some(Action::Connect) => connection::open(window, &ctx, board_handoff),
        Some(Action::Updates(check)) => updates::open(window, &ctx, check, board_handoff),
        Some(Action::Menu) => window.chrome.menu = !window.chrome.menu,
        Some(Action::Preferences) => {
            window.chrome.menu = false;
            window.settings_editor.open_from_chrome();
        }
        Some(Action::Usage) => crate::usage::open(window, &ctx),
        Some(Action::Shortcuts) => window.settings_editor.open_shortcuts_from_chrome(),
        Some(Action::Minimize) => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
        Some(Action::Maximize) => {
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
        Some(Action::Close) => ctx.send_viewport_cmd(ViewportCommand::Close),
        None => {}
    }
    if inline {
        window.chrome.record_work(dispatch.elapsed());
    }
    owned
}

fn shortcut_label(chord: crate::keyboard::Chord) -> String {
    let mut parts = Vec::with_capacity(4);
    if chord.ctrl {
        parts.push("Ctrl");
    }
    if chord.shift {
        parts.push("Shift");
    }
    if chord.alt {
        parts.push("Alt");
    }
    parts.push(chord.key.name());
    parts.join(" ")
}

fn resize_direction(area: Rect, p: egui::Pos2) -> Option<egui::ResizeDirection> {
    if !area.contains(p) {
        return None;
    }
    let n = p.y - area.top() < 5.0;
    let s = area.bottom() - p.y < 5.0;
    let w = p.x - area.left() < 5.0;
    let e = area.right() - p.x < 5.0;
    use egui::ResizeDirection::*;
    match (n, s, w, e) {
        (true, _, true, _) => Some(NorthWest),
        (true, _, _, true) => Some(NorthEast),
        (_, true, true, _) => Some(SouthWest),
        (_, true, _, true) => Some(SouthEast),
        (true, _, _, _) => Some(North),
        (_, true, _, _) => Some(South),
        (_, _, true, _) => Some(West),
        (_, _, _, true) => Some(East),
        _ => None,
    }
}

pub(crate) fn after_panels(window: &mut Window, ui: &mut egui::Ui, area: Rect) {
    if let Some(dock) = window.sidebar.last_dock {
        crate::viewer::paint(window, ui, dock);
    }
    if let Some(files) = window.sidebar.last_files {
        crate::explorer::show(window, ui, files);
    }
    crate::sidebar::paint_dividers(window, ui);
    connection::after_panels(window, ui, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_hints_use_current_bindings_and_disappear_when_unbound() {
        let mut keymap = crate::keyboard::Keymap::default();
        let action = crate::keyboard::Action::NewTab;
        assert_eq!(
            shortcut_label(keymap.chord_for(action).unwrap()),
            "Ctrl Shift T"
        );
        let rebound = crate::keyboard::Chord::from_written("alt+f3").unwrap();
        keymap.bind(action, Some(rebound));
        assert_eq!(shortcut_label(keymap.chord_for(action).unwrap()), "Alt F3");
        keymap.bind(action, None);
        assert!(keymap.chord_for(action).is_none());
    }
    #[test]
    fn edges_and_corners_choose_distinct_native_resize_directions() {
        let area = Rect::from_min_size(egui::pos2(10.0, 20.0), Vec2::new(200.0, 100.0));
        assert_eq!(
            resize_direction(area, egui::pos2(11.0, 21.0)),
            Some(egui::ResizeDirection::NorthWest)
        );
        assert_eq!(
            resize_direction(area, egui::pos2(209.0, 70.0)),
            Some(egui::ResizeDirection::East)
        );
        assert_eq!(resize_direction(area, egui::pos2(100.0, 70.0)), None);
    }
}
