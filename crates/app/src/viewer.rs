//! Read-only document views, independent of console and producer identity.
mod code;
mod code_header;
mod delivery;
mod diff;
mod drawer;
mod focus;
mod highlight;
pub(crate) mod image;
mod load;
pub(crate) mod markdown;
mod origin;
mod palette;
mod review;
mod selection_scroll;
mod view;

use crate::sidebar::Tool;
pub(crate) use drawer::StripAction;
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
    code: code::CodeState,
    message: String,
    refresh_at: Option<Instant>,
    keyboard: bool,
    /// Whether the Space held now was pressed while the drawer held the keys.
    space: bool,
    /// Whether the drawer holds the keys this pass, once a press has handed them over.
    keys: bool,
    held: bool,
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
            code: code::CodeState::default(),
            message: String::new(),
            refresh_at: None,
            keyboard: false,
            space: false,
            keys: false,
            held: false,
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
    #[cfg(test)]
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
        self.code.reset();
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
                        if !self.kind.is_document()
                            || self
                                .loaded
                                .as_ref()
                                .is_none_or(|old| old.text != loaded.text)
                        {
                            // A selection's offsets belong to the text it was made on.
                            self.code.reset();
                            self.loaded = Some(loaded);
                        }
                    }
                    Err(why) => self.message = why,
                }
                self.refresh_at = self.kind.is_document().then(|| Instant::now() + REFRESH);
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
        crate::sidebar::Tool::Document
    } else {
        crate::sidebar::Tool::Git
    };
    crate::sidebar::select(window, tool);
    if window.viewer.origin.is_none() && window.viewer.path.is_empty() {
        origin::initialize(window, markdown);
    }
    // Reopening the Document tab keeps what it shows: Markdown or code alike.
    let kind = if markdown {
        if window.viewer.kind.is_document() {
            window.viewer.kind
        } else {
            Kind::Markdown
        }
    } else if window.viewer.kind.is_document() {
        Kind::Local
    } else {
        window.viewer.kind
    };
    window.viewer.open(kind);
    if let Some(ctx) = &window.viewer.context {
        ctx.request_repaint();
    }
}

/// The drawer place under `pos`, as the strip is laid out over `dock` this frame.
pub(crate) fn strip_action(
    window: &crate::Window,
    dock: egui::Rect,
    pos: egui::Pos2,
) -> Option<StripAction> {
    drawer::Strip::framed(dock, &drawer::tabs(window)).at(pos)
}

/// Where the strip paints `action` over `dock`, for tests that click it as a person would.
#[cfg(test)]
pub(crate) fn strip_place(
    window: &crate::Window,
    dock: egui::Rect,
    action: StripAction,
) -> Option<egui::Pos2> {
    drawer::Strip::framed(dock, &drawer::tabs(window))
        .places
        .iter()
        .find(|(which, _)| *which == action)
        .map(|(_, place)| place.center())
}

pub(crate) fn apply_strip_action(
    window: &mut crate::Window,
    action: StripAction,
    ctx: &egui::Context,
) {
    match action {
        StripAction::Changes => activate(window, false),
        StripAction::Document => activate(window, true),
        StripAction::Image => origin::show_image(window),
        StripAction::Unified | StripAction::SideBySide => {
            for state in [&mut window.viewer, &mut window.viewer_other] {
                if state.tool_markdown == Some(false) {
                    state
                        .review
                        .set_side_by_side(action == StripAction::SideBySide);
                }
            }
        }
        StripAction::CloseChanges => origin::close_tab(window, Tool::Git, ctx),
        StripAction::CloseDocument => origin::close_tab(window, Tool::Document, ctx),
        StripAction::CloseImage => origin::close_tab(window, Tool::Image, ctx),
        StripAction::Maximize => crate::sidebar::toggle_maximize(window),
        StripAction::Close => crate::sidebar::close(window, ctx),
    }
    ctx.request_repaint();
}

/// Leave the keys to Files, which has just opened beside the drawer.
pub(crate) fn blur(window: &mut crate::Window) {
    window.viewer.keyboard = false;
}

pub(crate) fn release_gesture(window: &mut crate::Window) {
    window.viewer.held = false;
}

pub(crate) fn suspend(window: &mut crate::Window) {
    for state in [&mut window.viewer, &mut window.viewer_other] {
        state.open = false;
        state.keyboard = false;
        state.held = false;
        state.space = false;
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
    if blocked {
        window.viewer.space = false;
        window.viewer_other.space = false;
        window.viewer.keys = false;
    }
    let owned = !blocked && before_keys(window, ui, metrics);
    delivery::before_keys(window, ui.ctx());
    let requests = if blocked || owned {
        Vec::new()
    } else {
        crate::settings::editor::before_keys(window, ui, metrics)
    };
    delivery::deliver(window, ui.ctx());
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
        // Every return below leaves Space not the drawer's unless it is worked out again, so a
        // Space held across a closed drawer cannot pan when it comes back.
        let held = window.viewer.space || window.viewer_other.space;
        for state in [&mut window.viewer, &mut window.viewer_other] {
            state.events = None;
            // Nor the keys: whatever pass returns early, the drawer did not hold them in it.
            state.space = false;
            state.keys = false;
        }
        if window.settings_editor.is_open() || window.pane_view.is_modal() {
            return false;
        }
        let markdown = match crate::sidebar::active(window) {
            Some(Tool::Document | Tool::Image) => true,
            Some(Tool::Git) => false,
            _ => return false,
        };
        select_state(window, markdown);
        let geometry = crate::tab_bar::geometry(window, ui.max_rect(), metrics);
        let Some(rect) = geometry.dock else {
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
        // Read before a press below hands the keys over: a Space a console was given stays its own.
        let holds = window.viewer.keyboard || window.sidebar.maximized;
        window.viewer.space = ctx.input(|i| image::space_is_ours(i, holds, held));
        // Console keys are dispatched before its egui click response can run. Transfer focus
        // at the drawer's handoff so even text batched with this press has the right recipient.
        let returned = holds
            && !window.viewer.held
            && !window.sidebar.maximized
            && focus::return_to_console(window, &ctx, geometry.consoles, metrics);
        if ctx.input(|i| i.pointer.any_pressed()) && !window.viewer.held {
            window.viewer.keyboard = over && !returned;
            window.viewer.held = over && !returned;
        }
        window.viewer.keys = window.viewer.keyboard || window.sidebar.maximized;
        let gesture_owned = window.viewer.held;
        if !ctx.input(|i| i.focused && i.pointer.any_down()) {
            window.viewer.held = false;
        }
        // egui reports Ctrl+wheel as a zoom and not as a scroll, so the image's zoom gesture
        // would otherwise leave the drawer without its input.
        let pointer_activity = ctx.input(|i| {
            i.pointer.any_pressed()
                || i.smooth_scroll_delta != egui::Vec2::ZERO
                || i.zoom_delta() != 1.0
        });
        let was_open = window.viewer.open;
        let reading_keys = window.sidebar.maximized
            && ctx.input(|i| {
                i.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key { .. }
                            | egui::Event::Text(_)
                            | egui::Event::Paste(_)
                            | egui::Event::Copy
                            | egui::Event::Cut
                            | egui::Event::Ime(_)
                    )
                })
            });
        let owned = reading_keys
            || window.viewer.keyboard
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
            crate::sidebar::consume_input(window, &ctx);
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
        Some(Tool::Document | Tool::Image) => true,
        Some(Tool::Git) => false,
        _ => return,
    };
    select_state(window, markdown);
    if !window.viewer.open {
        return;
    }
    let ctx = ui.ctx().clone();
    window.viewer.context = Some(ctx.clone());
    let started = Instant::now();
    let image = origin::showing_image(window);
    if !image
        && window.viewer.kind.is_document()
        && window.viewer.requested.is_some()
        && window.viewer.job.is_none()
        && window.viewer.refresh_at.is_none()
    {
        window.viewer.refresh_at = Some(Instant::now());
    }
    if image {
        if let Some(slot) = image::focused_mut(&mut window.tabs, window.active) {
            slot.poll(&ctx);
        }
    } else {
        if window.viewer.requested.is_none()
            && !window.viewer.path.is_empty()
            && !window.viewer.editing_path
        {
            window.viewer.start(&ctx);
        }
        window.viewer.poll(&ctx);
    }
    // An image tab with nothing to show is about the focused console, which is there.
    let empty = image && image::focused(window).is_none_or(|slot| !slot.shown());
    let unavailable = !empty
        && origin::shown_origin(window)
            .is_none_or(|origin| !window.all_panes().any(|pane| pane.name == origin.pane));
    let filtered = ctx.input(|input| input.events.clone());
    if let Some(events) = window.viewer.events.take() {
        ctx.input_mut(|input| input.events = events);
    }
    image::send::aim(window);
    let tabs = drawer::tabs(window);
    let title = title(window, unavailable);
    view::show(
        &mut window.viewer,
        ui,
        dock,
        window.theme,
        unavailable,
        window.sidebar.maximized,
        (
            &tabs,
            &title,
            if image {
                view::Body::Image(
                    window.tabs.get(window.active).map(|tab| tab.focus),
                    image::focused_mut(&mut window.tabs, window.active)
                        .map(|slot| (&mut window.image, slot)),
                )
            } else {
                view::Body::Document
            },
        ),
    );
    ctx.input_mut(|input| input.events = filtered);
    code_header::reveal_in_files(window);
    window.viewer.record(started.elapsed());
    window.timed_separately += window.viewer.take_cost();
}

/// The strip's name for what the drawer shows: the source console's project, or the folder it
/// reads; the terminal's own title says nothing a person needs here.
fn title(window: &crate::Window, unavailable: bool) -> drawer::Title {
    // An empty image tab names the focused console, which is there by definition.
    if let Some(pane) = origin::empty_image_tab(window) {
        return drawer::Title {
            project: pane
                .project
                .as_ref()
                .map(|badge| (badge.name.clone(), badge.ink)),
            folder: pane
                .program
                .as_ref()
                .map(|(folder, _)| folder.clone())
                .filter(|folder| !folder.is_empty())
                .unwrap_or_else(|| crate::pane_view::display_label(pane).into_owned()),
            unavailable: false,
        };
    }
    let origin = origin::shown_origin(window);
    let project = origin
        .and_then(|origin| window.all_panes().find(|pane| pane.name == origin.pane))
        .and_then(|pane| pane.project.as_ref())
        .map(|badge| (badge.name.clone(), badge.ink));
    let folder = window
        .viewer
        .requested
        .as_ref()
        .filter(|(_, kind)| !kind.is_document())
        .map(|(path, _)| path.clone())
        .or_else(|| origin.and_then(|origin| origin.cwd.clone()))
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "No source console".to_owned());
    drawer::Title {
        project,
        folder,
        unavailable,
    }
}

#[cfg(test)]
pub(crate) use code_header::document_relative_path;
pub(crate) use image::is_image;
pub(crate) use origin::{
    KEPT_DOCUMENT, NotOpened, Room, open_diff_for_pane, open_document_target, open_for_an_agent,
    open_image_target, show_image,
};

#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod tests;

/// Explorer transitions use the ordinary close path, retaining unsent review comments.
#[cfg(test)]
pub(crate) fn hide(window: &mut crate::Window) {
    suspend(window);
}
/// The image the image tab shows, the focused console's, with the console it is pinned to.
#[cfg(test)]
pub(crate) fn shown_image(window: &crate::Window) -> Option<(usize, PathBuf)> {
    let slot = image::focused(window)?;
    Some((slot.origin()?.pane, slot.path()?.to_owned()))
}
#[cfg(test)]
pub(crate) fn is_open(window: &crate::Window) -> bool {
    window.viewer.open
}
/// Whether the drawer was asked to show `path` as code.
#[cfg(test)]
pub(crate) fn shows_as_code(window: &crate::Window, path: &std::path::Path) -> bool {
    window.viewer.requested.as_ref() == Some(&(path.to_owned(), Kind::Code))
}
