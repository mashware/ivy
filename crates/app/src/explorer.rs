//! A window-owned file surface with navigation owned by runtime console identity.
mod load;
mod model;
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
            query: nav.query.trim().to_owned(),
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

    fn hide(&mut self) {
        self.visible = false;
        self.keyboard = false;
        self.resizing = false;
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
        let Some(area) = crate::tab_bar::geometry(window, ui.max_rect(), metrics).dock else {
            window.explorer.keyboard = false;
            window.explorer.held = false;
            return (false, false);
        };
        let state = &mut window.explorer;
        let rect = state.rect(area);
        let over = state.visible
            && ctx.input(|i| i.pointer.interact_pos().is_some_and(|p| rect.contains(p)));
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
                if let Some(nav) = target
                    .and_then(|pane| state.navigation.get_mut(&pane))
                    .filter(|nav| !nav.query.is_empty())
                {
                    nav.query.clear();
                    nav.scroll = 0.0;
                    nav.refresh();
                } else {
                    state.hide();
                }
            } else if let Some(pane) = target
                && !ctx.memory(|memory| {
                    memory.has_focus(egui::Id::new(("explorer-name-search", pane)))
                })
            {
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
            ctx.input_mut(|i| {
                i.events.retain(|e| {
                    matches!(
                        e,
                        egui::Event::ModifiersChanged(_) | egui::Event::WindowFocused(_)
                    )
                })
            });
        }
        if !window.explorer.visible
            && crate::sidebar::active(window) == Some(crate::sidebar::Tool::Files)
        {
            crate::sidebar::close(window, ctx);
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
        || crate::viewer::is_open(window)
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
        let open = view::show(&mut window.explorer, ui, area, target, window.theme);
        ui.ctx().input_mut(|input| input.events = filtered);
        if !window.explorer.visible {
            crate::sidebar::close(window, ui.ctx());
        }
        if let Some((pane, path)) = open {
            if crate::viewer::open_markdown_target(window, pane, path, None) {
                window.explorer.hide();
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
    crate::sidebar::select(window, crate::sidebar::Tool::Files);
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

fn display(path: &Path) -> String {
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
