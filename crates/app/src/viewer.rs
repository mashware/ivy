//! Read-only document views, independent of console and producer identity.
mod delivery;
mod diff;
mod load;
mod markdown;
mod origin;
mod review;
mod view;

use eframe::egui;
use load::{Job, Kind, Loaded};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

const REFRESH: Duration = Duration::from_secs(1);
const GRIP: f32 = 6.0;

pub(super) struct Source {
    pane: usize,
    label: String,
    cwd: Option<PathBuf>,
}

pub(crate) struct State {
    origin: Option<Source>,
    review: review::Review,
    base: String,
    requested_base: String,
    generation: u64,
    job_generation: u64,
    job_base: String,
    editing_path: bool,
    expanded: bool,
    pending_send: Option<String>,
    context: Option<egui::Context>,
    open: bool,
    kind: Kind,
    width: f32,
    path: String,
    requested: Option<(PathBuf, Kind)>,
    job: Option<(PathBuf, Kind, Job)>,
    loaded: Option<Loaded>,
    message: String,
    refresh_at: Option<Instant>,
    keyboard: bool,
    held: bool,
    resizing: bool,
    focus_path: bool,
    blocked_frame: Option<u64>,
    cost: Duration,
    frame: Duration,
    total: Duration,
    frames: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            origin: None,
            review: review::Review::default(),
            base: "main".into(),
            requested_base: "main".into(),
            generation: 0,
            job_generation: 0,
            job_base: "main".into(),
            editing_path: true,
            expanded: false,
            pending_send: None,
            context: None,
            open: false,
            kind: Kind::Markdown,
            width: 800.0,
            path: String::new(),
            requested: None,
            job: None,
            loaded: None,
            message: String::new(),
            refresh_at: None,
            keyboard: false,
            held: false,
            resizing: false,
            focus_path: false,
            blocked_frame: None,
            cost: Duration::ZERO,
            frame: Duration::ZERO,
            total: Duration::ZERO,
            frames: 0,
        }
    }
}

impl State {
    pub(crate) fn open_markdown(&mut self) {
        self.open(Kind::Markdown);
    }
    pub(crate) fn open_diff(&mut self) {
        self.open(Kind::Local);
    }

    fn open(&mut self, kind: Kind) {
        if self.kind != kind {
            self.requested = None;
            self.loaded = None;
            self.path.clear();
            self.message.clear();
        }
        self.kind = kind;
        self.open = true;
        self.keyboard = true;
        self.focus_path = true;
        if self.requested.is_some() {
            self.refresh_at = Some(Instant::now());
        }
    }

    fn close(&mut self) {
        self.open = false;
        self.keyboard = false;
        self.refresh_at = None;
        self.resizing = false;
        // A worker on an unavailable filesystem is abandoned on purpose, never joined by UI.
        // Keep its handle so repeated open/close cannot start unbounded concurrent readers.
    }

    fn rect(&self, area: egui::Rect) -> egui::Rect {
        let inset = 8.0_f32.min(area.width().min(area.height()) * 0.5);
        let area = area.shrink(inset);
        let width = if self.expanded {
            area.width()
        } else {
            self.width.min(area.width())
        };
        egui::Rect::from_min_max(egui::pos2(area.right() - width, area.top()), area.max)
    }

    fn start(&mut self, ctx: &egui::Context) {
        let path = PathBuf::from(&self.path);
        if self.path.is_empty() {
            self.message = "Choose a file or repository path first.".into();
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.requested_base = self.base.clone();
        self.requested = Some((path, self.kind));
        self.loaded = None;
        self.message.clear();
        self.refresh_at = Some(Instant::now());
        self.poll(ctx);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Some((path, kind, job)) = &self.job
            && let Some(result) = job.poll()
        {
            let current = self
                .requested
                .as_ref()
                .is_some_and(|r| r == &(path.clone(), *kind))
                && self.requested_base == self.job_base
                && self.generation == self.job_generation;
            if current {
                match result {
                    Ok(loaded) => {
                        self.message.clear();
                        if self.kind != Kind::Markdown
                            || self
                                .loaded
                                .as_ref()
                                .is_none_or(|old| old.text != loaded.text)
                        {
                            self.loaded = Some(loaded);
                        }
                    }
                    Err(why) => self.message = why,
                }
                self.refresh_at = (self.kind == Kind::Markdown).then(|| Instant::now() + REFRESH);
            }
            self.job = None;
        }
        if self.open
            && self.job.is_none()
            && self.refresh_at.is_some_and(|at| at <= Instant::now())
            && let Some((path, kind)) = &self.requested
        {
            match Job::start_with_base(
                path.clone(),
                *kind,
                self.requested_base.clone(),
                ctx.clone(),
            ) {
                Ok(job) => {
                    self.job_base = self.requested_base.clone();
                    self.job_generation = self.generation;
                    self.job = Some((path.clone(), *kind, job));
                }
                Err(why) => {
                    self.message = why;
                    self.refresh_at = None;
                }
            }
        }
        if self.open {
            if self.job.is_some() {
                ctx.request_repaint_after(Duration::from_millis(100));
            } else if let Some(at) = self.refresh_at {
                ctx.request_repaint_after(at.saturating_duration_since(Instant::now()));
            }
        }
    }

    fn record(&mut self, spent: Duration) {
        self.cost += spent;
        self.frame += spent;
    }
    pub(crate) fn take_cost(&mut self) -> Duration {
        std::mem::take(&mut self.cost)
    }
    pub(crate) fn close_frame(&mut self) {
        if !self.frame.is_zero() {
            self.frames += 1;
            self.total += std::mem::take(&mut self.frame);
        }
    }
    pub(crate) fn publish(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        writeln!(
            to,
            "document viewers: frames={}, total={:?}",
            self.frames, self.total
        )
    }
}

pub(crate) fn keys(
    window: &mut crate::Window,
    ui: &mut egui::Ui,
    metrics: &crate::painter::CellMetrics,
    blocked: bool,
) -> (bool, Vec<crate::WindowRequest>) {
    let blocked = blocked || window.chrome.guide_blocked(ui.ctx());
    window.viewer.context = Some(ui.ctx().clone());
    let owned = !blocked && before_keys(window, ui, metrics);
    delivery::deliver(window, ui.ctx());
    let requests = if blocked || owned {
        Vec::new()
    } else {
        crate::settings::editor::before_keys(window, ui, metrics)
    };
    (owned, requests)
}

/// The viewer gets the original input before the console dispatcher sees any of it.
pub(crate) fn before_keys(
    window: &mut crate::Window,
    ui: &mut egui::Ui,
    metrics: &crate::painter::CellMetrics,
) -> bool {
    if window.settings_editor.is_open() || window.summary.is_open() || window.pane_view.is_modal() {
        return false;
    }
    if !window.viewer.open
        && !window.viewer.held
        && window.viewer.blocked_frame != Some(ui.ctx().cumulative_frame_nr())
    {
        return false;
    }
    let started = Instant::now();
    let area = crate::tab_bar::geometry(window, ui.max_rect(), metrics).consoles;
    let full = area;
    let state = &mut window.viewer;
    let ctx = ui.ctx().clone();
    let frame = ctx.cumulative_frame_nr();
    let was_open = state.open;
    let rect = state.rect(full);
    let over = ctx.input(|i| {
        i.pointer
            .interact_pos()
            .is_some_and(|p| rect.expand(GRIP).contains(p))
    });
    if ctx.input(|i| i.pointer.any_pressed()) && !state.held {
        state.keyboard = over;
        state.held = over;
        state.resizing = over
            && ctx.input(|i| {
                i.pointer
                    .interact_pos()
                    .is_some_and(|p| (p.x - rect.left()).abs() <= GRIP)
            });
    }
    let gesture_owned = state.held;
    if state.resizing
        && let Some(p) = ctx.input(|i| i.pointer.interact_pos())
    {
        state.width =
            (full.right() - 8.0 - p.x).clamp(280.0_f32.min(full.width()), full.width().max(280.0));
    }
    if !ctx.input(|i| i.focused && i.pointer.any_down()) {
        state.held = false;
        state.resizing = false;
    }
    let pointer_activity = ctx.input(|i| {
        i.pointer.any_pressed()
            || i.pointer.any_released()
            || i.smooth_scroll_delta != egui::Vec2::ZERO
    });
    let owned = state.keyboard
        || gesture_owned
        || (over && pointer_activity)
        || state.blocked_frame == Some(frame);
    if was_open {
        state.poll(&ctx);
        view::show(state, ui, full, window.theme);
        if state.keyboard && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            state.close();
        }
    }
    if owned || (was_open && !state.open) {
        state.blocked_frame = Some(frame);
        window.modifiers = ctx.input(|i| {
            if i.focused {
                i.modifiers
            } else {
                egui::Modifiers::NONE
            }
        });
        window.header_input.block_for_settings(frame);
        ctx.input_mut(|i| {
            i.events.retain(|e| {
                matches!(
                    e,
                    egui::Event::ModifiersChanged(_) | egui::Event::WindowFocused(_)
                )
            })
        });
    }
    state.record(started.elapsed());
    window.timed_separately += state.take_cost();
    owned || (was_open && !state.open)
}

pub(crate) use origin::{open, open_diff_for_pane, open_markdown_target};

#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod tests;
