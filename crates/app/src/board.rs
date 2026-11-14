//! Read-only live board of every console, grouped by project. Its state never owns a console or
//! producer authority.
mod branches;
mod cards;
#[path = "roadmap_file.rs"]
mod files;
mod input;
mod layout;
mod model;
mod observation;
pub(crate) mod plans;
pub(crate) mod probe;
mod roadmap;
mod view;

use eframe::egui;
use std::time::Instant;

pub(crate) struct State {
    pub(crate) plans: plans::Store,
    observations: observation::Observations,
    branches: branches::Branches,
    cards: std::collections::HashMap<usize, cards::Reader>,
    roadmaps: roadmap::Reader,
    child_hits: Vec<(egui::Rect, cards::Command)>,
    child_pressed: Option<(egui::Rect, cards::Command)>,
    link_to_open: Option<String>,
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
            observations: Default::default(),
            branches: Default::default(),
            cards: Default::default(),
            roadmaps: Default::default(),
            child_hits: Vec::new(),
            child_pressed: None,
            link_to_open: None,
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
    fn apply_card_command(&mut self, command: &cards::Command) {
        let pane = match command {
            cards::Command::Body(pane, _)
            | cards::Command::Expand(pane)
            | cards::Command::Compact(pane) => *pane,
        };
        if !self.cards.contains_key(&pane) {
            return;
        }
        match command {
            cards::Command::Body(_, crate::panel::body::Action::Link(target)) => {
                self.link_to_open = Some(target.clone())
            }
            cards::Command::Body(_, crate::panel::body::Action::Fold(..)) => {
                self.cards.get_mut(&pane).unwrap().apply(command);
                self.timing.folded();
            }
            cards::Command::Expand(_) => {
                self.cards.get_mut(&pane).unwrap().apply(command);
                self.timing.expanded();
            }
            cards::Command::Compact(_) => self.cards.get_mut(&pane).unwrap().apply(command),
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
    #[cfg(test)]
    pub(crate) fn observed_since(&self, pane: usize) -> Option<Instant> {
        self.observations.since(pane)
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
    observe(window, ui.ctx());
    if !window.board.enabled || !window.board.open {
        return;
    }
    let started = Instant::now();
    if let Some(target) = window
        .board
        .link_to_open
        .take()
        .and_then(|target| crate::links::WebTarget::parse(&target))
    {
        window.desktop.open(&target);
    }
    let directories = window
        .all_panes()
        .filter_map(|pane| {
            window
                .projects
                .directory(pane.name)
                .map(|d| (pane.name, d.to_path_buf()))
        })
        .collect();
    window
        .board
        .branches
        .refresh(directories, Instant::now(), ui.ctx());
    if let Some(delay) = window.board.branches.next_refresh(Instant::now()) {
        ui.ctx().request_repaint_after(delay);
    }
    let mut sections = model::sections(window, std::time::SystemTime::now());
    let plan_started = Instant::now();
    roadmap::refresh_files(window, ui.ctx());
    roadmap::enrich(window, &mut sections, std::time::SystemTime::now());
    window.board.timing.plan_projection(plan_started.elapsed());
    let wall = std::time::SystemTime::now();
    let clock = Instant::now();
    sections.sort_by_key(|section| {
        let rank = section
            .entries
            .iter()
            .map(|entry| {
                let signal = window
                    .all_panes()
                    .find(|p| p.name == entry.pane)
                    .and_then(|p| p.signals.view(clock));
                observation::classify(entry.snapshot.document.as_deref(), signal, wall).rank()
            })
            .min()
            .unwrap_or(observation::Column::Idle.rank());
        (
            matches!(section.kind, model::Kind::Other),
            rank,
            section.name().to_owned(),
            format!("{:?}", section.key),
        )
    });
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

/// Observation is independent of whether the reader has the Board on screen.
pub(crate) fn observe(window: &mut crate::Window, ctx: &egui::Context) {
    if !window.board.enabled {
        return;
    }
    let started = Instant::now();
    let wall = std::time::SystemTime::now();
    let clock = Instant::now();
    let columns: Vec<_> = window
        .all_panes()
        .map(|p| {
            let snapshot = crate::panel::summary::snapshot(&p.panel_state, wall);
            (
                p.name,
                p.signals.incarnation(),
                observation::classify(snapshot.document.as_deref(), p.signals.view(clock), wall),
            )
        })
        .collect();
    let live: Vec<_> = columns.iter().map(|(pane, _, _)| *pane).collect();
    let mut transitions = 0;
    for (pane, epoch, column) in columns {
        let before = window.board.observations.since(pane);
        if column != observation::Column::Wait {
            window.board.observations.reset(pane);
        }
        window
            .board
            .observations
            .observe_epoch(pane, column, epoch, clock);
        transitions += usize::from(before != window.board.observations.since(pane));
    }
    window.board.observations.retain(&live);
    window.board.cards.retain(|pane, _| live.contains(pane));
    if window.board.open
        && let Some(delay) = window.board.observations.next_threshold(clock)
    {
        ctx.request_repaint_after(delay);
    }
    window
        .board
        .timing
        .observation(started.elapsed(), live.len(), transitions);
    window.timed_separately += window.board.timing.take_cost();
}
