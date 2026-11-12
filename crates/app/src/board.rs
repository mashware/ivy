//! Read-only live board of every console, grouped by project. Its state never owns a console or
//! producer authority.
#[path = "roadmap_file.rs"]
mod files;
mod input;
mod layout;
mod model;
pub(crate) mod plans;
pub(crate) mod probe;
mod roadmap;
mod view;

use eframe::egui;
use std::time::Instant;

pub(crate) struct State {
    pub(crate) plans: plans::Store,
    files: Option<files::Reader>,
    file_snapshots: Vec<files::Snapshot>,
    file_config: std::collections::BTreeMap<String, std::path::PathBuf>,
    file_reload: bool,
    roadmap_window: (usize, usize),
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
    /// The cards of the last painting in reading order, with where each sat in the grid.
    order: Vec<layout::Slot>,
    /// The part of the board on screen last frame, in board coordinates, with the area it was
    /// painted in.
    viewport: Option<(egui::Rect, egui::Rect)>,
    scroll_selected: bool,
    blocked_frame: Option<u64>,
    awaiting_release: bool,
    pub(crate) timing: crate::measure::Board,
}

impl Default for State {
    fn default() -> Self {
        Self {
            plans: plans::Store::default(),
            files: None,
            file_snapshots: Vec::new(),
            file_config: Default::default(),
            file_reload: false,
            roadmap_window: (2, 3),
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
            viewport: None,
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
    pub(crate) fn card_rect(&self, id: usize) -> Option<egui::Rect> {
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
    let mut sections = model::sections(window, std::time::SystemTime::now());
    let plan_started = Instant::now();
    roadmap::refresh_files(window, ui.ctx());
    roadmap::enrich(window, &mut sections, std::time::SystemTime::now());
    window.board.timing.plan_projection(plan_started.elapsed());
    let panes: Vec<usize> = sections
        .iter()
        .flat_map(|s| &s.entries)
        .map(|entry| entry.pane)
        .collect();
    window.board.timing.saw_open();
    window.board.timing.projected(panes.len());
    window.board.timing.grouped(
        sections
            .iter()
            .filter(|s| matches!(s.kind, model::Kind::Repository { .. }))
            .count(),
    );
    window.board.selected = model::reconcile(window.board.selected, window.board.ordinal, &panes);
    window.board.ordinal = panes
        .iter()
        .position(|pane| Some(*pane) == window.board.selected)
        .unwrap_or(0);
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

pub(crate) fn configure(window: &mut crate::Window, settings: &crate::settings::Settings) {
    if window.board.file_config != settings.roadmaps {
        window.board.file_config = settings.roadmaps.clone();
        window.board.file_reload = true;
    }
    window.board.roadmap_window = (settings.roadmap_window.done, settings.roadmap_window.next);
}
