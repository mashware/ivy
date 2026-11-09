//! Integrated window controls. All commands are sent through the native viewport.
use crate::{Window, WindowRequest, painter::CellMetrics, tiling::Orientation};
use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, Vec2, ViewportCommand};
use std::time::{Duration, Instant};

pub(crate) const HEIGHT: f32 = 49.0;

#[derive(Default, Debug)]
pub(crate) struct Chrome {
    enabled: bool,
    pub(crate) title_sizes: typography::Sizes,
    zoom_probe: bool,
    menu: bool,
    menu_scroll: f32,
    guide: connection::Guide,
    updates: updates::Dialog,
    held: bool,
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
    pub(crate) fn start(&mut self, ctx: &egui::Context, automatic_updates: bool) {
        self.enable();
        if automatic_updates {
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
pub(crate) mod typography;
mod updates;
pub(crate) use branding::native_options;
pub(crate) use connection::{after_panels, begin_pass, prepare};

#[derive(Clone, Copy)]
enum Action {
    SplitRight,
    SplitDown,
    NewTab,
    Search,
    Overview,
    Connect,
    Updates(bool),
    Menu,
    Preferences,
    Viewer(bool),
    Shortcuts,
    TogglePanel,
    Minimize,
    Maximize,
    Close,
}

fn button(ui: &egui::Ui, rect: Rect, label: &str, hint: &str, bg: Color32, fg: Color32) -> bool {
    if !rect.is_positive() {
        return false;
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
    if response.hovered() {
        ui.painter().rect_filled(rect.shrink(2.0), 4.0, bg);
    }
    let p = ui.painter_at(rect);
    let c = rect.center();
    // Use the approved prototype's 24-unit SVG paths inside its 17-point icon box.
    let scale = 17.0 / 24.0;
    let at = |x, y| c + egui::vec2(x, y) * scale;
    let stroke = Stroke::new(1.6 * scale, fg);
    let line = |a: (f32, f32), b: (f32, f32)| p.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    let outline = |a: (f32, f32), b: (f32, f32), radius| {
        p.rect_stroke(
            Rect::from_min_max(at(a.0, a.1), at(b.0, b.1)),
            radius * scale,
            stroke,
            egui::StrokeKind::Middle,
        )
    };
    match hint {
        "Split right" | "Split down" => {
            outline((-9.0, -8.0), (9.0, 8.0), 2.0);
            if hint == "Split right" {
                line((0.0, -8.0), (0.0, 8.0));
                line((4.5, -3.0), (4.5, 3.0));
                line((2.0, 0.0), (7.0, 0.0));
            } else {
                line((-9.0, 0.0), (9.0, 0.0));
                line((0.0, 2.0), (0.0, 6.0));
                line((-2.0, 4.0), (2.0, 4.0));
            }
        }
        "New tab" => {
            line((-7.0, 0.0), (7.0, 0.0));
            line((0.0, -7.0), (0.0, 7.0));
        }
        "Search console" => {
            p.circle_stroke(at(-1.5, -1.5), 6.5 * scale, stroke);
            line((4.0, 4.0), (9.0, 9.0));
        }
        "Overview" => {
            outline((-9.0, -8.0), (9.0, 8.0), 2.0);
            for y in [-4.0, 0.0, 4.0] {
                line((-5.0, y), (5.0, y));
            }
        }
        "Menu" => {
            for y in [-6.0, 0.0, 6.0] {
                line((-7.0, y), (7.0, y));
            }
        }
        "Connect agents" => connection::paint_icon(&p, rect, fg, false),
        "Close window" => {
            line((-6.0, -6.0), (6.0, 6.0));
            line((6.0, -6.0), (-6.0, 6.0));
        }
        "Minimize window" => {
            line((-7.0, 0.0), (7.0, 0.0));
        }
        "Maximize window" => {
            outline((-7.0, -7.0), (7.0, 7.0), 1.0);
        }
        "Restore window" => {
            for (a, b) in [
                ((-3.0, -7.0), (-3.0, -9.0)),
                ((-3.0, -9.0), (9.0, -9.0)),
                ((9.0, -9.0), (9.0, 3.0)),
                ((9.0, 3.0), (7.0, 3.0)),
            ] {
                line(a, b);
            }
            outline((-9.0, -3.0), (3.0, 9.0), 1.0);
        }
        _ => {
            p.text(
                c,
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(12.0),
                fg,
            );
        }
    }
    response.clicked()
}

/// Paint and claim the titlebar before settings and terminal input dispatch.
pub(crate) fn before_keys(window: &mut Window, ui: &mut egui::Ui, metrics: &CellMetrics) -> bool {
    if !window.chrome.enabled {
        return false;
    }
    let started = Instant::now();
    let ctx = ui.ctx().clone();
    let area = ui.max_rect();
    let row = Rect::from_min_max(
        area.min,
        egui::pos2(area.right(), (area.top() + HEIGHT).min(area.bottom())),
    );
    let bg = window.theme.panel_background();
    let fg = window.theme.foreground();
    let hover = window.theme.divider();
    ui.painter().rect_filled(row, 0.0, bg);
    ui.painter().line_segment(
        [row.left_bottom(), row.right_bottom()],
        Stroke::new(1.0, hover),
    );
    let mut action = None;
    let left_offset = if cfg!(target_os = "macos") { 78.0 } else { 6.0 };
    let width = 34.0;
    let compact = row.width() < 300.0;
    let left = [
        ("⊞", "Split right", Action::SplitRight),
        ("⊟", "Split down", Action::SplitDown),
        ("+", "New tab", Action::NewTab),
    ];
    for (i, (label, hint, cmd)) in left.into_iter().enumerate() {
        let x = row.left() + left_offset + i as f32 * width + if i > 1 { 14.0 } else { 0.0 };
        let rect = Rect::from_min_size(
            egui::pos2(x, row.top() + 7.0),
            Vec2::new(width, (row.height() - 14.0).max(0.0)),
        );
        if !compact && rect.right() <= row.right() && button(ui, rect, label, hint, hover, fg) {
            action = Some(cmd);
        }
    }
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let right = [
        ("×", "Close window", Action::Close),
        (
            "□",
            if maximized {
                "Restore window"
            } else {
                "Maximize window"
            },
            Action::Maximize,
        ),
        ("−", "Minimize window", Action::Minimize),
        ("☰", "Menu", Action::Menu),
        ("⌕", "Search console", Action::Search),
        ("≡", "Overview", Action::Overview),
        ("", "Connect agents", Action::Connect),
    ];
    let skip = if cfg!(target_os = "macos") { 3 } else { 0 };
    for (i, (label, hint, cmd)) in right.into_iter().skip(skip).enumerate() {
        let x = row.right() - 6.0 - (i + 1) as f32 * width;
        let rect = Rect::from_min_size(
            egui::pos2(x, row.top() + 7.0),
            Vec2::new(width, (row.height() - 14.0).max(0.0)),
        );
        if !compact
            && rect.left() >= row.left() + left_offset + 3.0 * width + 14.0
            && button(ui, rect, label, hint, hover, fg)
        {
            action = Some(cmd);
        }
    }
    if !compact {
        for x in [
            row.left() + left_offset + 2.0 * width + 6.0,
            row.right() - 6.0 - 3.0 * width,
        ] {
            ui.painter().line_segment(
                [
                    egui::pos2(x, row.top() + 15.0),
                    egui::pos2(x, row.bottom() - 15.0),
                ],
                Stroke::new(1.0, hover),
            );
        }
    }
    if compact {
        let menu = Rect::from_min_max(
            egui::pos2((row.right() - 40.0).max(row.left()), row.top() + 7.0),
            egui::pos2(row.right(), (row.bottom() - 7.0).max(row.top() + 7.0)),
        );
        if button(ui, menu, "", "Menu", hover, fg) {
            action = Some(Action::Menu);
        }
    }
    connection::compact_button(window, ui, row);
    updates::indicator(window, ui, row);
    let left_end = row.left() + left_offset + if compact { 0.0 } else { 3.0 * width + 14.0 };
    let right_start = row.right()
        - if compact {
            connection::compact_reserve(row)
        } else {
            6.0 + (7 - skip) as f32 * width
        };
    let drag = Rect::from_min_max(
        egui::pos2(left_end, row.top()),
        egui::pos2(right_start.max(left_end), row.bottom()),
    );
    if drag.width() > 75.0 {
        branding::paint_identity(ui, drag, fg);
    }
    if drag.is_positive() {
        let response = ui.interact(drag, ui.id().with("chrome-drag"), Sense::click_and_drag());
        if response.double_clicked() {
            action = Some(Action::Maximize);
        } else if response.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
    }
    if matches!(action, Some(Action::Menu)) {
        window.chrome.menu = !window.chrome.menu;
        window.chrome.menu_scroll = 0.0;
        action = None;
        ctx.request_repaint();
    }
    connection::open_requested(window, &ctx, &mut action);
    let (menu_rect, menu_action) = menu::show(window, ui, row, area, compact);
    action = menu_action.or(action);
    let frame = ctx.cumulative_frame_nr();
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    let over =
        pointer.is_some_and(|p| row.contains(p) || (window.chrome.menu && menu_rect.contains(p)));
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
        || !ui.is_enabled()
        || (crate::summary::blocked(window, &ctx)
            && !matches!(
                action,
                Some(Action::Overview | Action::Minimize | Action::Maximize | Action::Close)
            ))
    {
        return owned;
    }
    if action.is_some() && !matches!(action, Some(Action::Menu)) {
        window.chrome.menu = false;
    }
    let dispatch = Instant::now();
    let inline = matches!(
        action,
        Some(
            Action::Search
                | Action::Viewer(_)
                | Action::TogglePanel
                | Action::Minimize
                | Action::Maximize
                | Action::Close
                | Action::Connect
                | Action::Updates(_)
        )
    );
    match action {
        Some(Action::SplitRight) => window.apply(
            WindowRequest::Split(Orientation::Columns),
            &ctx,
            area,
            metrics,
        ),
        Some(Action::SplitDown) => {
            window.apply(WindowRequest::Split(Orientation::Rows), &ctx, area, metrics)
        }
        Some(Action::NewTab) => window.apply(WindowRequest::NewTab, &ctx, area, metrics),
        Some(Action::Search) => {
            let focus = window.active().focus;
            if let Some(pane) = window
                .active_mut()
                .panes_mut()
                .into_iter()
                .find(|p| p.name == focus)
            {
                let mut steps = crate::search::Steps::new(pane.search.is_open());
                steps.toggle();
                crate::search::apply(&mut pane.search, &mut pane.screen, steps);
            }
        }
        Some(Action::Overview) => crate::summary::toggle(window, &ctx),
        Some(Action::Connect) => connection::open(window, &ctx),
        Some(Action::Updates(check)) => updates::open(window, &ctx, check),
        Some(Action::Menu) => window.chrome.menu = !window.chrome.menu,
        Some(Action::Preferences) => {
            window.chrome.menu = false;
            window.settings_editor.open_from_chrome();
        }
        Some(Action::Shortcuts) => window.settings_editor.open_shortcuts_from_chrome(),
        Some(Action::Viewer(markdown)) => crate::viewer::open(window, markdown),
        Some(Action::TogglePanel) => {
            let focus = window.active().focus;
            if let Some(pane) = window
                .active_mut()
                .panes_mut()
                .into_iter()
                .find(|p| p.name == focus)
            {
                pane.folded = !pane.folded;
            }
        }
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
