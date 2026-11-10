//! Read-only live board of every console, grouped by project. Its state never owns a console or
//! producer authority.
mod input;
mod layout;
mod model;
pub(crate) mod probe;
mod view;

use eframe::egui;
use std::time::Instant;

pub(crate) struct State {
    pub(crate) enabled: bool,
    pub(crate) probe: Option<probe::Probe>,
    input_frame: Option<u64>,
    pub(crate) warning_due: Option<Instant>,
    open: bool,
    selected: Option<usize>,
    ordinal: usize,
    original: Option<usize>,
    painted: Vec<(usize, egui::Rect)>,
    pressed: Option<(usize, egui::Rect)>,
    activate: Option<usize>,
    order: Vec<usize>,
    /// Grid row and column of each entry of `order`, from the last painting; what ↑/↓ follow.
    grid: Vec<(usize, usize)>,
    scroll_selected: bool,
    blocked_frame: Option<u64>,
    awaiting_release: bool,
    pub(crate) timing: crate::measure::Board,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: true,
            probe: None,
            input_frame: None,
            warning_due: None,
            open: false,
            selected: None,
            ordinal: 0,
            original: None,
            painted: Vec::new(),
            pressed: None,
            activate: None,
            order: Vec::new(),
            grid: Vec::new(),
            scroll_selected: false,
            blocked_frame: None,
            awaiting_release: false,
            timing: crate::measure::Board::default(),
        }
    }
}

impl State {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
    #[cfg(test)]
    pub(crate) fn selected(&self) -> Option<usize> {
        self.selected
    }
    #[cfg(test)]
    pub(crate) fn row_rect(&self, id: usize) -> Option<egui::Rect> {
        self.painted
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, rect)| *rect)
    }
}

pub(crate) use input::{before_keys, blocked, cancel_gestures, toggle};

/// Runs only after active and background panel refresh, preserving normal PTY geometry.
pub(crate) fn paint(window: &mut crate::Window, ui: &mut egui::Ui, area: egui::Rect) {
    if !window.board.enabled || !window.board.open {
        return;
    }
    let started = Instant::now();
    let sections = model::project(window, std::time::SystemTime::now());
    let order: Vec<usize> = sections
        .iter()
        .flat_map(|s| &s.entries)
        .map(|entry| entry.pane)
        .collect();
    window.board.timing.saw_open();
    window.board.timing.projected(order.len());
    window.board.timing.sections(sections.len());
    window.board.selected = model::reconcile(window.board.selected, window.board.ordinal, &order);
    window.board.ordinal = order
        .iter()
        .position(|pane| Some(*pane) == window.board.selected)
        .unwrap_or(0);
    window.board.order = order;
    view::paint(window, ui, area, &sections);
    if let Some(target) = window.board.activate.take() {
        activate(window, ui.ctx(), target);
    }
    window.board.timing.record_work(started.elapsed());
    window.timed_separately += window.board.timing.take_cost();
}

pub(crate) fn activate(window: &mut crate::Window, ctx: &egui::Context, target: usize) {
    let owner = window
        .tabs
        .iter()
        .position(|tab| tab.panes().iter().any(|p| p.name == target));
    let Some(owner) = owner else {
        ctx.request_repaint();
        return;
    };
    let changed = window.active != owner || window.tabs[owner].focus != target;
    window.active = owner;
    let tab = &mut window.tabs[owner];
    tab.focus = target;
    if window
        .pane_view
        .zoomed(tab.name)
        .is_some_and(|p| p != target)
    {
        window.pane_view.clear(tab.name);
    }
    window.board.timing.navigation(changed);
    window.board.original = None;
    input::close(window, ctx);
}
