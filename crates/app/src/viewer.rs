//! Read-only document views, independent of console and producer identity.
mod delivery;
mod diff;
mod load;
pub(crate) mod markdown;
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
    pending_send: Option<String>,
    context: Option<egui::Context>,
    tool_markdown: Option<bool>,
    open: bool,
    kind: Kind,
    path: String,
    requested: Option<(PathBuf, Kind)>,
    job: Option<(PathBuf, Kind, Job)>,
    loaded: Option<Loaded>,
    message: String,
    refresh_at: Option<Instant>,
    keyboard: bool,
    held: bool,
    focus_path: bool,
    blocked_frame: Option<u64>,
    events: Option<Vec<egui::Event>>,
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
            pending_send: None,
            context: None,
            tool_markdown: None,
            open: false,
            kind: Kind::Markdown,
            path: String::new(),
            requested: None,
            job: None,
            loaded: None,
            message: String::new(),
            refresh_at: None,
            keyboard: false,
            held: false,
            focus_path: false,
            blocked_frame: None,
            events: None,
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
        // A worker on an unavailable filesystem is abandoned on purpose, never joined by UI.
        // Keep its handle so repeated open/close cannot start unbounded concurrent readers.
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
    pub(crate) fn close_combined_frame(&mut self, other: &mut Self) {
        self.frame += std::mem::take(&mut other.frame);
        self.close_frame();
    }

    #[cfg(test)]
    pub(crate) fn publish(&self, to: &mut dyn std::io::Write) -> std::io::Result<()> {
        writeln!(
            to,
            "document viewers: frames={}, total={:?}",
            self.frames, self.total
        )
    }

    pub(crate) fn publish_combined(
        &self,
        other: &Self,
        to: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        writeln!(
            to,
            "document viewers: frames={}, total={:?}",
            self.frames + other.frames,
            self.total + other.total
        )
    }
}

pub(super) fn select_state(window: &mut crate::Window, markdown: bool) {
    if window.viewer.tool_markdown.unwrap_or(true) != markdown {
        std::mem::swap(&mut window.viewer, &mut window.viewer_other);
    }
    window.viewer.tool_markdown = Some(markdown);
    window.viewer_other.tool_markdown = Some(!markdown);
}

pub(crate) fn activate(window: &mut crate::Window, markdown: bool) {
    select_state(window, markdown);
    let tool = if markdown {
        crate::sidebar::Tool::Markdown
    } else {
        crate::sidebar::Tool::Git
    };
    crate::sidebar::select(window, tool);
    if window.viewer.origin.is_none() && window.viewer.path.is_empty() {
        origin::initialize(window, markdown);
    }
    let kind = if markdown {
        Kind::Markdown
    } else if window.viewer.kind == Kind::Markdown {
        Kind::Local
    } else {
        window.viewer.kind
    };
    window.viewer.open(kind);
    if let Some(ctx) = &window.viewer.context {
        ctx.request_repaint();
    }
}

pub(crate) fn suspend(window: &mut crate::Window) {
    for state in [&mut window.viewer, &mut window.viewer_other] {
        state.open = false;
        state.keyboard = false;
        state.held = false;
        state.refresh_at = None;
        state.events = None;
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
    window.viewer_other.context = Some(ui.ctx().clone());
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
    let started = Instant::now();
    let owned = (|| {
        for state in [&mut window.viewer, &mut window.viewer_other] {
            state.events = None;
        }
        if window.settings_editor.is_open() || window.pane_view.is_modal() {
            return false;
        }
        let markdown = match crate::sidebar::active(window) {
            Some(crate::sidebar::Tool::Markdown) => true,
            Some(crate::sidebar::Tool::Git) => false,
            _ => return false,
        };
        select_state(window, markdown);
        let Some(rect) = crate::tab_bar::geometry(window, ui.max_rect(), metrics).dock else {
            window.viewer.keyboard = false;
            window.viewer.held = false;
            window.viewer.refresh_at = None;
            return false;
        };
        if !window.viewer.open && !window.viewer.held {
            return false;
        }
        let ctx = ui.ctx().clone();
        window.viewer.context = Some(ctx.clone());
        let frame = ctx.cumulative_frame_nr();
        let over = ctx.input(|i| i.pointer.interact_pos().is_some_and(|p| rect.contains(p)));
        if ctx.input(|i| i.pointer.any_pressed()) && !window.viewer.held {
            window.viewer.keyboard = over;
            window.viewer.held = over;
        }
        let gesture_owned = window.viewer.held;
        if !ctx.input(|i| i.focused && i.pointer.any_down()) {
            window.viewer.held = false;
        }
        let pointer_activity =
            ctx.input(|i| i.pointer.any_pressed() || i.smooth_scroll_delta != egui::Vec2::ZERO);
        let was_open = window.viewer.open;
        let owned = window.viewer.keyboard
            || gesture_owned
            || (over && pointer_activity)
            || window.viewer.blocked_frame == Some(frame);
        if window.viewer.keyboard && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            window.viewer.close();
        }
        if owned || (was_open && !window.viewer.open) {
            window.viewer.events = Some(ctx.input(|i| i.events.clone()));
            window.viewer.blocked_frame = Some(frame);
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
        let dismissed = was_open && !window.viewer.open;
        if dismissed {
            crate::sidebar::close(window, &ctx);
        }
        owned || dismissed
    })();
    window.viewer.record(started.elapsed());
    window.timed_separately += window.viewer.take_cost();
    owned
}

/// Paint the active viewer after the console tree has reached its final geometry for this pass.
pub(crate) fn paint(window: &mut crate::Window, ui: &mut egui::Ui, dock: egui::Rect) {
    if window.settings_editor.is_open()
        || window.pane_view.is_modal()
        || window.chrome.guide_blocked(ui.ctx())
    {
        return;
    }
    let markdown = match crate::sidebar::active(window) {
        Some(crate::sidebar::Tool::Markdown) => true,
        Some(crate::sidebar::Tool::Git) => false,
        _ => return,
    };
    select_state(window, markdown);
    if !window.viewer.open {
        return;
    }
    let ctx = ui.ctx().clone();
    window.viewer.context = Some(ctx.clone());
    let started = Instant::now();
    if window.viewer.kind == Kind::Markdown
        && window.viewer.requested.is_some()
        && window.viewer.job.is_none()
        && window.viewer.refresh_at.is_none()
    {
        window.viewer.refresh_at = Some(Instant::now());
    }
    if window.viewer.requested.is_none()
        && !window.viewer.path.is_empty()
        && !window.viewer.editing_path
    {
        window.viewer.start(&ctx);
    }
    window.viewer.poll(&ctx);
    let unavailable = window
        .viewer
        .origin
        .as_ref()
        .is_none_or(|origin| !window.all_panes().any(|pane| pane.name == origin.pane));
    let filtered = ctx.input(|input| input.events.clone());
    if let Some(events) = window.viewer.events.take() {
        ctx.input_mut(|input| input.events = events);
    }
    view::show(&mut window.viewer, ui, dock, window.theme, unavailable);
    ctx.input_mut(|input| input.events = filtered);
    window.viewer.record(started.elapsed());
    window.timed_separately += window.viewer.take_cost();
}

pub(crate) use origin::{open_diff_for_pane, open_markdown_target};

#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod tests;

/// Explorer transitions use the ordinary close path, retaining unsent review comments.
#[cfg(test)]
pub(crate) fn hide(window: &mut crate::Window) {
    suspend(window);
}
pub(crate) fn is_open(window: &crate::Window) -> bool {
    window.viewer.open
}
