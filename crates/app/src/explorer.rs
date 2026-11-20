//! A window-owned file surface with navigation owned by runtime console identity.
mod load;
mod model;
mod pattern;
mod search;
mod view;

use crate::{Window, painter::CellMetrics};
use eframe::egui;
use load::{Entry, EntryKind, Listing, Loader, Request};
use model::Navigation;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Instant,
};

const CACHE_BYTES: usize = 16 * 1024 * 1024;
const CACHE_COUNT: usize = 32;
const ROW_LIMIT: usize = 8192;

struct Cached {
    result: Result<Listing, String>,
    used: u64,
}

pub(crate) struct Explorer {
    visible: bool,
    context: Option<egui::Context>,
    navigation: HashMap<usize, Navigation>,
    cache: HashMap<Request, Cached>,
    loader: Loader,
    loading: Option<Request>,
    clock: u64,
    message: String,
    keyboard: bool,
    held: bool,
    resizing: bool,
    blocked_frame: Option<u64>,
    pointer_frame: Option<u64>,
    queued_open: bool,
    activate: bool,
    events: Option<Vec<egui::Event>>,
    viewport_rows: usize,
    row_stride: f32,
    /// The floating name search is open.
    searching: bool,
    search_index: usize,
    search_rect: Option<egui::Rect>,
    /// The console whose query the open search shows: the search closes when focus leaves it.
    search_pane: Option<usize>,
    /// The arrows moved the choice; the list scrolls it into view once.
    search_scroll: bool,
    focus_search: bool,
    pick: Option<search::Pick>,
}

impl Default for Explorer {
    fn default() -> Self {
        Self {
            visible: false,
            context: None,
            navigation: HashMap::new(),
            cache: HashMap::new(),
            loader: Loader::default(),
            loading: None,
            clock: 0,
            message: String::new(),
            keyboard: false,
            held: false,
            resizing: false,
            blocked_frame: None,
            pointer_frame: None,
            queued_open: false,
            activate: false,
            events: None,
            viewport_rows: 24,
            row_stride: 27.0,
            searching: false,
            search_index: 0,
            search_rect: None,
            search_pane: None,
            search_scroll: false,
            focus_search: false,
            pick: None,
        }
    }
}

impl Explorer {
    fn reconcile(&mut self, sources: &[(usize, Option<PathBuf>)]) {
        self.navigation
            .retain(|id, _| sources.iter().any(|(live, _)| live == id));
        for (id, source) in sources {
            self.navigation
                .entry(*id)
                .or_default()
                .observe(source.clone());
        }
        self.cache.retain(|request, _| {
            self.navigation.get(&request.pane).is_some_and(|nav| {
                nav.generation == request.generation && nav.epoch == request.epoch
            })
        });
        if self
            .loading
            .as_ref()
            .is_some_and(|request| !self.current(request))
        {
            self.loader.cancel();
            // The single worker remains alive even if the OS is blocked; its next request
            // replaces the pending mailbox rather than starting a replacement thread.
            self.loading = None;
        }
    }

    fn current(&self, request: &Request) -> bool {
        self.navigation.get(&request.pane).is_some_and(|nav| {
            nav.source.is_some()
                && nav.generation == request.generation
                && nav.epoch == request.epoch
        })
    }

    fn request(&self, pane: usize, path: PathBuf) -> Request {
        let nav = &self.navigation[&pane];
        Request {
            pane,
            generation: nav.generation,
            epoch: nav.epoch,
            path,
            query: nav.search_query().to_owned(),
            // A listing has no query, so its key never carries the flag: flipping it must not
            // make the tree's cached folders look like other requests.
            regex: nav.regex && !nav.search_query().is_empty(),
            reveal: false,
        }
    }

    fn poll(&mut self) {
        if let Some(reply) = self.loader.poll() {
            if self.loading.as_ref() == Some(&reply.request) {
                self.loading = None;
            }
            if self.current(&reply.request) {
                let mut key = reply.request.clone();
                if reply.request.reveal {
                    match &reply.result {
                        Ok(listing) => {
                            let root = listing
                                .root
                                .clone()
                                .expect("reveal reader returns its root");
                            let nav = self.navigation.get_mut(&key.pane).unwrap();
                            nav.enter(root.clone());
                            nav.selected = listing.selected.clone();
                            nav.reveal_selected = nav.selected.is_some();
                            self.message.clear();
                            key = self.request(key.pane, root);
                        }
                        Err(error) => {
                            self.message = error.clone();
                            return;
                        }
                    }
                }
                if let Ok(listing) = &reply.result {
                    let nav = self.navigation.get_mut(&key.pane).unwrap();
                    if key.query.is_empty()
                        && nav.selected.as_ref().is_some_and(|path| {
                            path.parent() == Some(key.path.as_path())
                                && !listing.entries.iter().any(|entry| &entry.path == path)
                        })
                    {
                        nav.selected = None;
                    }
                }
                self.clock = self.clock.wrapping_add(1);
                self.cache.insert(
                    key,
                    Cached {
                        result: reply.result,
                        used: self.clock,
                    },
                );
                self.evict();
            }
        }
    }

    fn evict(&mut self) {
        // Traversal touches the root before its children. Keep that anchor while evicting
        // derived listings, otherwise pressure makes the next frame reload the entire tree.
        let recent_pane = self
            .cache
            .iter()
            .max_by_key(|(_, value)| value.used)
            .map(|(key, _)| key.pane);
        let root = recent_pane.and_then(|pane| {
            self.navigation
                .get(&pane)
                .and_then(|nav| nav.root.clone())
                .map(|path| (pane, path))
        });
        while self.cache.len() > CACHE_COUNT
            || self
                .cache
                .values()
                .map(|cached| cached.result.as_ref().map_or(0, |listing| listing.bytes))
                .sum::<usize>()
                > CACHE_BYTES
        {
            let Some(oldest) = self
                .cache
                .iter()
                .filter(|(key, _)| {
                    root.as_ref()
                        .is_none_or(|(pane, path)| key.pane != *pane || &key.path != path)
                })
                .min_by_key(|(_, cached)| cached.used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.cache.remove(&oldest);
        }
    }

    fn rect(&self, area: egui::Rect) -> egui::Rect {
        area
    }

    pub(crate) fn visible(&self) -> bool {
        self.visible
    }

    fn hide(&mut self) {
        self.visible = false;
        self.keyboard = false;
        self.resizing = false;
        self.searching = false;
        self.search_rect = None;
    }

    /// Open the floating name search with its field focused.
    fn open_search(&mut self, pane: Option<usize>) {
        self.searching = true;
        self.search_pane = pane;
        self.focus_search = true;
        self.keyboard = true;
        self.search_index = 0;
    }

    fn close_search(&mut self, pane: Option<usize>) {
        if let Some(nav) = pane.and_then(|pane| self.navigation.get_mut(&pane)) {
            nav.query.clear();
        }
        self.searching = false;
        self.search_rect = None;
        self.search_pane = None;
    }
}

fn active(window: &Window) -> Option<usize> {
    window
        .active()
        .where_the_focus_is()
        .map(|index| window.active().panes()[index].name)
}

pub(crate) fn suspend(window: &mut Window) {
    window.explorer.hide();
    window.explorer.held = false;
    window.explorer.queued_open = false;
}

/// Leave the keys to another surface that has just opened; Files stays shown.
pub(crate) fn blur(window: &mut Window) {
    window.explorer.keyboard = false;
}

/// Where the floating search list lies, painted over the dividers and the drawer: what is under
/// it does not take a press.
pub(crate) fn list_rect(window: &Window) -> Option<egui::Rect> {
    let state = &window.explorer;
    state.search_rect.filter(|_| state.searching)
}

pub(crate) fn reveal(window: &mut Window) {
    window.explorer.visible = true;
    window.explorer.keyboard = true;
}

fn reconcile(window: &mut Window) {
    let sources: Vec<_> = window
        .all_panes()
        .map(|pane| {
            (
                pane.name,
                window
                    .projects
                    .directory(pane.name)
                    .filter(|p| p.is_absolute())
                    .map(Path::to_path_buf),
            )
        })
        .collect();
    window.explorer.reconcile(&sources);
    window.explorer.poll();
}

/// Capture the gesture before console dispatch; resolve the displayed pane after focus changes.
pub(crate) fn before_keys(
    window: &mut Window,
    ui: &mut egui::Ui,
    metrics: &CellMetrics,
    blocked: bool,
) -> (bool, bool) {
    let started = Instant::now();
    let result = (|| {
        reconcile(window);
        let ctx = ui.ctx();
        window.explorer.context = Some(ctx.clone());
        if window.explorer.queued_open {
            window.explorer.queued_open = false;
            window.explorer.visible = true;
            window.explorer.keyboard = true;
        }
        if blocked || window.settings_editor.is_open() || window.chrome.guide_blocked(ctx) {
            return (false, false);
        }
        let target = active(window);
        let state = &mut window.explorer;
        state.events = None;
        let frame = ctx.cumulative_frame_nr();
        if !state.visible && !state.held && state.blocked_frame != Some(frame) {
            return (false, false);
        }
        let Some(area) = crate::tab_bar::geometry(window, ui.max_rect(), metrics).files else {
            window.explorer.keyboard = false;
            window.explorer.held = false;
            return (false, false);
        };
        let state = &mut window.explorer;
        if state.searching && state.search_pane != target {
            let left = state.search_pane;
            state.close_search(left);
        }
        let rect = state.rect(area);
        // The floating search overhangs the consoles; a press on it is Files', not theirs.
        let list = state.search_rect.filter(|_| state.searching);
        let over = state.visible
            && ctx.input(|i| {
                i.pointer
                    .interact_pos()
                    .is_some_and(|p| rect.contains(p) || list.is_some_and(|r| r.contains(p)))
            });
        if ctx.input(|i| i.pointer.any_pressed()) && !state.held {
            state.keyboard = over;
            state.held = over;
        }
        let gesture = state.held;
        if !ctx.input(|i| i.focused && i.pointer.any_down()) {
            state.held = false;
            state.resizing = false;
        }
        let pointer =
            ctx.input(|i| i.pointer.any_pressed() || i.smooth_scroll_delta != egui::Vec2::ZERO);
        let pointer_owned = gesture || (over && pointer) || state.pointer_frame == Some(frame);
        let owned =
            state.keyboard || gesture || (over && pointer) || state.blocked_frame == Some(frame);
        if state.visible && state.keyboard {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                // Escape leaves the search before it closes Files.
                if state.searching {
                    state.close_search(target);
                } else {
                    state.hide();
                }
            } else if state.searching {
                if let Some(pane) = target {
                    state.pick = search::keys(state, pane, ctx).or(state.pick.take());
                }
            } else if let Some(pane) = target {
                view::keys(state, pane, ctx);
            }
        }
        if owned {
            state.events = Some(ctx.input(|i| i.events.clone()));
            state.blocked_frame = Some(frame);
            window.modifiers = ctx.input(|i| {
                if i.focused {
                    i.modifiers
                } else {
                    egui::Modifiers::NONE
                }
            });
            if pointer_owned {
                state.pointer_frame = Some(frame);
                window.header_input.block_for_settings(frame);
            }
            crate::sidebar::consume_input(window, ctx);
        }
        (owned, pointer_owned)
    })();
    window.sidebar.record(started.elapsed());
    window.timed_separately += window.sidebar.take_cost();
    result
}

pub(crate) fn show(window: &mut Window, ui: &mut egui::Ui, area: egui::Rect) {
    if window.settings_editor.is_open()
        || window.board.is_open()
        || window.pane_view.is_modal()
        || window.chrome.guide_blocked(ui.ctx())
    {
        return;
    }
    let started = Instant::now();
    (|| {
        reconcile(window);
        if !window.explorer.visible {
            return;
        }
        let target = active(window);
        let filtered = ui.ctx().input(|input| input.events.clone());
        if let Some(events) = window.explorer.events.take() {
            ui.ctx().input_mut(|input| input.events = events);
        }
        let mut open = view::show(&mut window.explorer, ui, area, target, window.theme);
        let state = &mut window.explorer;
        if let Some(pane) = target.filter(|_| state.searching && state.visible) {
            let clicked = search::show(state, ui.ctx(), pane, area, &window.theme);
            match clicked.or(state.pick.take()) {
                Some(search::Pick::Reveal(path)) => search::reveal(state, pane, &path),
                Some(search::Pick::Open(entry)) => {
                    search::reveal(state, pane, &entry.path);
                    open = open.or(view::activate(state, pane, &entry));
                }
                None => {}
            }
        } else {
            state.search_rect = None;
            state.pick = None;
        }
        ui.ctx().input_mut(|input| input.events = filtered);
        if let Some((pane, path)) = open {
            if crate::viewer::open_document_target(window, pane, path, None) {
                ui.ctx().request_repaint();
            } else {
                window.explorer.message =
                    "Send or clear the review comments before opening another document.".into();
            }
        }
    })();
    window.sidebar.record(started.elapsed());
    window.timed_separately += window.sidebar.take_cost();
}

/// The folder Files shows for `pane` right now, if Files has ever listed one for it.
pub(crate) fn root_of(window: &Window, pane: usize) -> Option<PathBuf> {
    window.explorer.navigation.get(&pane)?.root.clone()
}

/// The row Files has selected for `pane`, and whether it is an expanded folder.
#[cfg(test)]
pub(crate) fn selected_of(window: &Window, pane: usize) -> Option<(PathBuf, bool)> {
    let nav = window.explorer.navigation.get(&pane)?;
    let selected = nav.selected.clone()?;
    let expanded = nav.expanded.contains(&selected);
    Some((selected, expanded))
}

pub(crate) fn open_path(window: &mut Window, pane: usize, path: PathBuf) {
    let Some(tab) = window
        .tabs
        .iter()
        .position(|tab| tab.panes().iter().any(|p| p.name == pane))
    else {
        return;
    };
    window.active = tab;
    window.active_mut().focus = pane;
    if !crate::sidebar::files_visible(window) {
        crate::sidebar::select(window, crate::sidebar::Tool::Files);
    }
    reveal(window);
    reconcile(window);
    let state = &mut window.explorer;
    let Some(nav) = state
        .navigation
        .get_mut(&pane)
        .filter(|nav| nav.source.is_some())
    else {
        state.message = "Local directory unavailable or unsupported.".into();
        return;
    };
    let path = if path.is_absolute() {
        path
    } else {
        nav.source.as_ref().unwrap().join(path)
    };
    nav.query.clear();
    nav.refresh();
    let mut request = state.request(pane, path);
    request.reveal = true;
    state.loading = Some(request.clone());
    // The sidebar retains the window context even when Files has not been painted yet.
    let ctx = state.context.clone().unwrap_or_default();
    state.loader.submit(request, &ctx);
    ctx.request_repaint();
}

/// Show `folder` in the source console's Files: inside the tree it already shows, expanded,
/// selected and scrolled into view, keeping its root; a folder outside that root opens as one.
pub(crate) fn reveal_folder(window: &mut Window, pane: usize, folder: PathBuf) {
    let inside = window
        .explorer
        .navigation
        .get(&pane)
        .and_then(|nav| nav.root.as_ref())
        .is_some_and(|root| folder.starts_with(root));
    if !inside {
        open_path(window, pane, folder);
        return;
    }
    let Some(tab) = window
        .tabs
        .iter()
        .position(|tab| tab.panes().iter().any(|p| p.name == pane))
    else {
        return;
    };
    window.active = tab;
    window.active_mut().focus = pane;
    if !crate::sidebar::files_visible(window) {
        crate::sidebar::select(window, crate::sidebar::Tool::Files);
    }
    reveal(window);
    let state = &mut window.explorer;
    search::reveal(state, pane, &folder);
    if let Some(nav) = state.navigation.get_mut(&pane) {
        nav.expanded.insert(folder);
    }
    if let Some(ctx) = &state.context {
        ctx.request_repaint();
    }
}

pub(crate) fn display(path: &Path) -> String {
    path.to_string_lossy().chars().flat_map(|c| {
        if c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}') {
            c.escape_unicode().collect::<Vec<_>>()
        } else { vec![c] }
    }).collect()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod interaction_tests;
