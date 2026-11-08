//! Read-only live Overview. Its state never owns a console or producer authority.
mod input;
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
    scroll_selected: bool,
    blocked_frame: Option<u64>,
    awaiting_release: bool,
    pub(crate) timing: crate::measure::Overview,
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
            scroll_selected: false,
            blocked_frame: None,
            awaiting_release: false,
            timing: crate::measure::Overview::default(),
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

pub(crate) use input::{before_keys, blocked, toggle};

/// Runs only after active and background panel refresh, preserving normal PTY geometry.
pub(crate) fn paint(window: &mut crate::Window, ui: &mut egui::Ui, area: egui::Rect) {
    if !window.summary.enabled || !window.summary.open {
        return;
    }
    let started = Instant::now();
    let rows = model::project(window, std::time::SystemTime::now());
    window.summary.timing.saw_open();
    window.summary.timing.projected(rows.len());
    window.summary.selected =
        model::reconcile(window.summary.selected, window.summary.ordinal, &rows);
    window.summary.order = rows.iter().map(|row| row.pane).collect();
    window.summary.ordinal = rows
        .iter()
        .position(|r| Some(r.pane) == window.summary.selected)
        .unwrap_or(0);
    view::paint(window, ui, area, &rows);
    if let Some(target) = window.summary.activate.take() {
        activate(window, ui.ctx(), target);
    }
    window.summary.timing.record_work(started.elapsed());
    window.timed_separately += window.summary.timing.take_cost();
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
    window.summary.timing.navigation(changed);
    window.summary.original = None;
    input::close(window, ctx);
}
