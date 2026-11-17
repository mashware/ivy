//! Read-only live board of every console, grouped by project. Its state never owns a console or
//! producer authority.
mod bar;
mod branches;
mod cards;
#[path = "roadmap_file.rs"]
mod files;
mod filter;
mod input;
mod layout;
mod milestones;
mod model;
mod observation;
pub(crate) mod plans;
pub(crate) mod probe;
mod roadmap;
mod style;
mod view;

use eframe::egui;
use std::time::Instant;

pub(crate) struct State {
    pub(crate) plans: plans::Store,
    surface: view::ViewState,
    observations: observation::Observations,
    branches: branches::Branches,
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
    activate: Option<usize>,
    /// The cards of the last painting in reading order, with where each sat in the grid.
    order: Vec<layout::Slot>,
    scroll_selected: bool,
    blocked_frame: Option<u64>,
    awaiting_release: bool,
    pointer_owned: bool,
    keyboard_focus: bool,
    close_requested: bool,
    arrangement: filter::Arrangement,
    filter: filter::Filter,
    pub(crate) timing: crate::measure::Board,
}

impl Default for State {
    fn default() -> Self {
        Self {
            plans: plans::Store::default(),
            surface: Default::default(),
            observations: Default::default(),
            branches: Default::default(),
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
            activate: None,
            order: Vec::new(),
            scroll_selected: false,
            blocked_frame: None,
            awaiting_release: false,
            pointer_owned: false,
            keyboard_focus: false,
            close_requested: false,
            arrangement: Default::default(),
            filter: Default::default(),
            timing: crate::measure::Board::default(),
        }
    }
}

impl State {
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
        self.surface
            .painted
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, rect)| *rect)
    }
}

#[cfg(test)]
pub(crate) use input::toggle;

#[cfg(test)]
pub(crate) use view::HINT;

#[cfg(test)]
pub(crate) fn select_for_test(window: &mut crate::Window, pane: usize) {
    window.board.selected = Some(pane);
}
pub(crate) use input::{before_keys, blocked, cancel_gestures, dismiss_for_chrome, reanchor};

/// Called by the shared sidebar host when Board becomes the selected tool.
/// Keep the user's Board navigation and cached geometry across hide/reopen.
pub(crate) fn reveal(window: &mut crate::Window, ctx: &egui::Context) {
    if !window.board.enabled {
        return;
    }
    if !window.board.open {
        window.board.open = true;
        window.board.original = Some(window.active().focus);
        if window.board.selected.is_none() {
            window.board.selected = Some(window.active().focus);
            window.board.ordinal = 0;
            window.board.scroll_selected = true;
        }
        window.board.surface.pressed = None;
        window.board.pointer_owned = false;
        window.board.activate = None;
        window.board.keyboard_focus = true;
        window.board.blocked_frame = Some(ctx.cumulative_frame_nr());
    }
    ctx.request_repaint();
}

/// Release Board input ownership while leaving its global view state intact.
pub(crate) fn suspend(window: &mut crate::Window) {
    window.board.open = false;
    window.board.keyboard_focus = false;
    if window.board.pointer_owned {
        window.board.awaiting_release = true;
        window.board.pointer_owned = false;
    }
    window.board.surface.pressed = None;
    window.board.surface.child_pressed = None;
    window.board.activate = None;
}

/// Runs only after active and background panel refresh, preserving normal PTY geometry.
pub(crate) fn paint(window: &mut crate::Window, ui: &mut egui::Ui, area: egui::Rect) {
    observe(window, ui.ctx());
    if !window.board.enabled || !window.board.open {
        return;
    }
    let started = Instant::now();
    if let Some(target) = window
        .board
        .surface
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
    let signals: std::collections::HashMap<_, _> = window
        .all_panes()
        .filter_map(|pane| pane.signals.view(clock).map(|signal| (pane.name, signal)))
        .collect();
    sections.sort_by_key(|section| {
        let rank = section
            .entries
            .iter()
            .map(|entry| {
                let signal = signals.get(&entry.pane).copied();
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
    let claiming = sections
        .iter()
        .flat_map(|s| &s.entries)
        .filter(|entry| {
            let signal = signals.get(&entry.pane).copied();
            filter::claims(observation::classify(entry.snapshot.document.as_deref(), signal, wall))
        })
        .count();
    let totals = bar::Totals {
        consoles: sections.iter().map(|s| s.entries.len()).sum(),
        projects: sections
            .iter()
            .filter(|s| matches!(s.kind, model::Kind::Repository { .. }))
            .count(),
        claiming,
    };
    filter::apply(&mut sections, &window.board.filter, |entry| {
        let signal = signals.get(&entry.pane).copied();
        observation::classify(entry.snapshot.document.as_deref(), signal, wall)
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
    view::paint(window, ui, area, &sections, &signals, wall, totals);
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
    crate::sidebar::close(window, ctx);
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
    window
        .board
        .surface
        .cards
        .retain(|pane, _| live.contains(pane));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_selection_and_scroll_survive_shared_sidebar_switches() {
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]; 4]);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 650.0));
        let frame = |window: &mut crate::Window, events| {
            crate::tests::one_frame_in(window, &ctx, area, events);
        };
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Board);
        frame(&mut window, vec![]);
        let initial_selection = window.board.selected();
        frame(
            &mut window,
            vec![egui::Event::Key {
                key: egui::Key::End,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(&mut window, vec![]);
        let selected = window.board.selected().expect("Board has a selection");
        assert_ne!(
            Some(selected),
            initial_selection,
            "End did not change Board selection"
        );
        let selected_rect = window
            .board
            .card_rect(selected)
            .expect("selected card visible");
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Files);
        crate::sidebar::select(&mut window, crate::sidebar::Tool::Board);
        frame(&mut window, vec![]);
        assert_eq!(window.board.selected(), Some(selected));
        assert_eq!(window.board.card_rect(selected), Some(selected_rect));
    }
}
