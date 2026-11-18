//! Header gestures outlive their first frame: a toggle leaves the source widget alive.

use super::controls::{Action, Controls, Hit};
use super::menu::Menu;
use crate::{
    Window,
    painter::CellMetrics,
    pointer::MouseOutcome,
    tiling::{Orientation, can_split_content as can_split},
};
use eframe::egui;
use std::time::SystemTime;

#[derive(Default)]
pub(crate) struct Input {
    pub(super) menu: Option<Menu>,
    pub(super) held: Option<egui::PointerButton>,
    last_frame: Option<u64>,
    pub(super) blocked_frame: Option<u64>,
    pub(super) pending: Option<(usize, Action)>,
    /// The console whose hidden control cell was last pressed, and when that press was released
    /// (egui's input time; `None` while it is still held).
    unseen: Option<(usize, Option<f64>)>,
}

impl Input {
    pub(crate) fn hold_context(&mut self, button: egui::PointerButton, frame: u64) {
        self.held = Some(button);
        self.blocked_frame = Some(frame);
    }

    pub(crate) fn block_for_settings(&mut self, frame: u64) {
        self.menu = None;
        self.pending = None;
        self.blocked_frame = Some(frame);
    }
}

pub(crate) fn enabled(
    window: &Window,
    name: usize,
    rect: egui::Rect,
    metrics: &CellMetrics,
) -> [bool; 3] {
    [
        can_split(rect, Orientation::Columns, metrics, window.headers),
        can_split(rect, Orientation::Rows, metrics, window.headers),
        window
            .active()
            .panes()
            .into_iter()
            .find(|pane| pane.name == name)
            .is_some_and(|pane| pane.panel_state.is_visible(SystemTime::now())),
    ]
}

/// What a press at `at` reaches on one of the active tab's integrated headers: the console, the
/// target — the title's whenever the control there may not be used — and whether that is because
/// the header's controls were not painted in the previous pass (D99's rule).
///
/// The other reason a control may not be used: the press comes within a double click of the release
/// of a press that fell on this console's hidden controls. That first press is the title's and hands
/// the console the keyboard, which paints its controls at once; without this, the second click of a
/// double click aimed at the title would land on whatever appeared under it, a close included. The
/// window is counted from the release because egui counts a double click from release to release.
fn integrated_hit(
    window: &Window,
    ctx: &egui::Context,
    rects: &[egui::Rect],
    metrics: &CellMetrics,
    at: egui::Pos2,
) -> Option<(usize, super::integrated::Target, bool)> {
    use super::integrated::{Offer, Target, hit, revealed};
    let can_close = super::Button::in_a_tab_of(rects).is_drawn();
    let zoomed = window.pane_view.zoomed(window.active().name);
    let now = ctx.input(|i| i.time);
    let delay = ctx.options(|o| o.input_options.max_double_click_delay);
    window
        .active()
        .panes()
        .into_iter()
        .zip(rects)
        .find_map(|(pane, rect)| {
            if *rect == egui::Rect::NOTHING {
                return None;
            }
            let strip = pane
                .chrome_within(*rect, metrics, SystemTime::now(), window.headers)
                .header()?;
            let offer = Offer::of(
                *rect,
                metrics,
                window.headers,
                can_close,
                zoomed == Some(pane.name),
            );
            let target = hit(strip, offer, at)?;
            let second_click = window.header_input.unseen.is_some_and(|(name, released)| {
                name == pane.name && released.is_none_or(|released| now - released <= delay)
            });
            let unpainted = target != Target::Label && !revealed(ctx, pane.name);
            let usable = target == Target::Label || !(unpainted || second_click);
            Some((
                pane.name,
                if usable { target } else { Target::Label },
                unpainted,
            ))
        })
}

/// Called inside the mouse stopwatch; splitting must not subtract its work a second time.
pub(crate) fn pointer(
    window: &mut Window,
    ui: &egui::Ui,
    rects: &[egui::Rect],
    metrics: &CellMetrics,
) -> Option<MouseOutcome> {
    if !window.headers.are_painted() {
        return None;
    }
    let frame = ui.ctx().cumulative_frame_nr();
    let quiet = || MouseOutcome {
        target: None,
        the_tree_changed: false,
    };
    if window.header_input.last_frame == Some(frame) {
        return (window.header_input.blocked_frame == Some(frame)).then(quiet);
    }
    window.header_input.last_frame = Some(frame);
    if let Some((name, action)) = window.header_input.pending.take() {
        window.header_input.blocked_frame = Some(frame);
        return Some(MouseOutcome {
            target: None,
            the_tree_changed: apply(window, name, action, ui.ctx(), metrics),
        });
    }
    // The Board covers the console headers and took this frame's input before this pass ran: a
    // press on it must not also press the header strip underneath (a Git or close button).
    if crate::board::blocked(window, ui.ctx()) {
        return Some(quiet());
    }
    let events = ui.ctx().input(|input| input.events.clone());
    let released = |events: &[egui::Event]| {
        events.iter().any(|event| {
            matches!(
                event,
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                }
            )
        })
    };
    if let Some((_, pending @ None)) = &mut window.header_input.unseen
        && released(&events)
    {
        *pending = Some(ui.ctx().input(|i| i.time));
    }
    // egui reports a double click on the second release, not on its press.
    if window.chrome.enabled()
        && ui.is_enabled()
        && window.header_input.blocked_frame != Some(frame)
        && ui.ctx().input(|i| {
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary)
        })
        && let Some(pos) = events.iter().find_map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            } if modifiers.is_none() => Some(*pos),
            _ => None,
        })
    {
        let owner = integrated_hit(window, ui.ctx(), rects, metrics, pos)
            .filter(|(_, target, _)| *target == super::integrated::Target::Label)
            .map(|(name, _, _)| name);
        if let Some(name) = owner {
            let tab = window.active().name;
            let initial = window
                .pane_called(name)
                .map(|p| crate::pane_view::display_label(p).into_owned())
                .unwrap_or_else(|| format!("Console {}", name + 1));
            window.pane_view.begin_rename(tab, name, initial);
            window.header_input.held = None;
            window.header_input.blocked_frame = Some(frame);
            window.drag_in_progress = None;
            ui.ctx().request_repaint();
            return Some(quiet());
        }
    }
    if let Some(held) = window.header_input.held {
        if events.iter().any(|event| matches!(event, egui::Event::PointerButton {button, pressed:false,..} if *button == held))
                || !ui.ctx().input(|input| input.focused && input.pointer.button_down(held))
            {
                window.header_input.held = None;
            }
        window.header_input.blocked_frame = Some(frame);
        return Some(quiet());
    }
    if window.header_input.menu.is_some() {
        window.header_input.blocked_frame = Some(frame);
        let mut chosen = None;
        for event in &events {
            if let egui::Event::PointerButton {
                pos,
                button,
                pressed: true,
                ..
            } = event
            {
                window.header_input.held = ui
                    .ctx()
                    .input(|input| input.pointer.button_down(*button))
                    .then_some(*button);
                if *button == egui::PointerButton::Primary {
                    chosen = window
                        .header_input
                        .menu
                        .as_ref()
                        .and_then(|menu| menu.action_at(*pos));
                }
                window.header_input.menu = None;
                break;
            }
        }
        let changed =
            chosen.is_some_and(|(name, action)| apply(window, name, action, ui.ctx(), metrics));
        return Some(MouseOutcome {
            target: None,
            the_tree_changed: changed,
        });
    }
    for event in &events {
        let egui::Event::PointerButton {
            pos,
            button,
            pressed: true,
            modifiers,
        } = event
        else {
            continue;
        };
        if !modifiers.is_none() {
            continue;
        }
        if window.chrome.enabled()
            && *button == egui::PointerButton::Primary
            && let Some((name, target, unpainted)) =
                integrated_hit(window, ui.ctx(), rects, metrics, *pos)
        {
            if unpainted {
                let now = ui.ctx().input(|i| i.time);
                let pressed_at = events.iter().position(|e| std::ptr::eq(e, event));
                let released_after = pressed_at.is_some_and(|at| released(&events[at..]));
                window.header_input.unseen = Some((name, released_after.then_some(now)));
            }
            if target == super::integrated::Target::Label {
                continue;
            }
            window.header_input.held = ui
                .ctx()
                .input(|input| input.pointer.button_down(*button))
                .then_some(*button);
            window.header_input.blocked_frame = Some(frame);
            window.drag_in_progress = None;
            let tab = window.active().name;
            let changed = match target {
                super::integrated::Target::Label => unreachable!("a title press continued above"),
                super::integrated::Target::SplitRight => {
                    apply(window, name, Action::SplitRight, ui.ctx(), metrics)
                }
                super::integrated::Target::SplitDown => {
                    apply(window, name, Action::SplitDown, ui.ctx(), metrics)
                }
                super::integrated::Target::Review => {
                    crate::viewer::open_diff_for_pane(window, name);
                    ui.ctx().request_repaint();
                    false
                }
                super::integrated::Target::Panel => {
                    if let Some(pane) = window.pane_called(name)
                        && pane.panel_state.is_visible(SystemTime::now())
                    {
                        pane.folded = !pane.folded;
                    }
                    false
                }
                super::integrated::Target::Zoom => {
                    window.active_mut().focus = name;
                    window.pane_view.toggle(tab, name, rects.len());
                    true
                }
                super::integrated::Target::Search => {
                    // The keyboard follows the search, so what is typed next lands in its query.
                    window.active_mut().focus = name;
                    if let Some(pane) = window.pane_called(name) {
                        crate::search::toggle_on(&mut pane.search, &mut pane.screen);
                    }
                    ui.ctx().request_repaint();
                    false
                }
                super::integrated::Target::Close => {
                    window.pane_view.clear(tab);
                    if let Err(error) = window.close_a_pane_called(name, ui.ctx()) {
                        eprintln!("the button could not close its console: {error}");
                    }
                    true
                }
            };
            return Some(MouseOutcome {
                target: None,
                the_tree_changed: changed,
            });
        }
        let close = super::Button::in_a_tab_of(rects);
        let hit = window
            .active()
            .panes()
            .into_iter()
            .zip(rects)
            .find_map(|(pane, rect)| {
                if *rect == egui::Rect::NOTHING {
                    return None;
                }
                let chrome = pane.chrome_within(*rect, metrics, SystemTime::now(), window.headers);
                if !chrome.strip_holds(*pos) {
                    return None;
                }
                let strip = chrome.header()?;
                let controls = Controls::within(
                    strip,
                    metrics,
                    super::button_within(strip, metrics, close).map(|button| button.rect()),
                );
                let hit = if *button == egui::PointerButton::Secondary {
                    Some(Hit::Menu)
                } else if *button == egui::PointerButton::Primary && !window.chrome.enabled() {
                    controls.hit(*pos)
                } else {
                    None
                };
                hit.map(|hit| (pane.name, *rect, strip, hit))
            });
        if let Some((name, rect, strip, hit)) = hit {
            window.header_input.held = ui
                .ctx()
                .input(|input| input.pointer.button_down(*button))
                .then_some(*button);
            // A quick click can contain both edges in one frame.
            if events.iter().any(|event| matches!(event, egui::Event::PointerButton {button:released, pressed:false,..} if released == button)) {
                    window.header_input.held = None;
                }
            window.header_input.blocked_frame = Some(frame);
            window.drag_in_progress = None;
            let changed = match hit {
                Hit::Action(action) if enabled(window, name, rect, metrics)[action.index()] => {
                    apply(window, name, action, ui.ctx(), metrics)
                }
                Hit::Action(_) => false,
                Hit::Menu if window.chrome.enabled() => {
                    window
                        .pane_view
                        .open_menu(window.active().name, name, rects.len() > 1, *pos);
                    ui.ctx().request_repaint();
                    false
                }
                Hit::Menu => {
                    window.header_input.menu = Some(Menu::new(
                        name,
                        strip.left_bottom(),
                        ui.ctx().viewport_rect(),
                        metrics,
                        enabled(window, name, rect, metrics),
                        window.pane_called(name).is_some_and(|pane| pane.folded),
                    ));
                    false
                }
            };
            return Some(MouseOutcome {
                target: None,
                the_tree_changed: changed,
            });
        }
    }
    (window.header_input.blocked_frame == Some(frame)).then(quiet)
}

fn apply(
    window: &mut Window,
    name: usize,
    action: Action,
    ctx: &egui::Context,
    metrics: &CellMetrics,
) -> bool {
    let content = ctx.viewport_rect();
    let area = crate::tab_bar::geometry(window, content, metrics).consoles;
    let panes = window.active().panes();
    let Some(at) = panes.iter().position(|pane| pane.name == name) else {
        return false;
    };
    let rect = window.active().root.place(area, metrics.width()).parts[at];
    if !enabled(window, name, rect, metrics)[action.index()] {
        return false;
    }
    match action {
        Action::SplitRight | Action::SplitDown => {
            let before = window.active().panes().len();
            let direction = if action == Action::SplitRight {
                Orientation::Columns
            } else {
                Orientation::Rows
            };
            super::splitting::split(window, name, direction, ctx, content, metrics);
            window.active().panes().len() != before
        }
        Action::TogglePanel => {
            if let Some(pane) = window
                .active_mut()
                .panes_mut()
                .into_iter()
                .find(|pane| pane.name == name)
            {
                pane.folded = !pane.folded;
                true
            } else {
                false
            }
        }
    }
}
