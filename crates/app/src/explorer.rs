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
    viewport_rows: usize,
    row_stride: f32,
}

impl Default for Explorer {
    fn default() -> Self {
        Self {
            visible: false,
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
        }
    }

    fn poll(&mut self) {
        if let Some(reply) = self.loader.poll() {
            if self.loading.as_ref() == Some(&reply.request) {
                self.loading = None;
            }
            if self.current(&reply.request) {
                if let Ok(listing) = &reply.result {
                    let nav = self.navigation.get_mut(&reply.request.pane).unwrap();
                    if nav.selected.as_ref().is_some_and(|path| {
                        path.parent() == Some(reply.request.path.as_path())
                            && !listing.entries.iter().any(|entry| &entry.path == path)
                    }) {
                        nav.selected = None;
                    }
                }
                self.clock = self.clock.wrapping_add(1);
                self.cache.insert(
                    reply.request,
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
    reconcile(window);
    let ctx = ui.ctx();
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
    let over =
        state.visible && ctx.input(|i| i.pointer.interact_pos().is_some_and(|p| rect.contains(p)));
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
            state.hide();
        } else if let Some(pane) = target {
            view::keys(state, pane, ctx);
        }
    }
    if owned {
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
    reconcile(window);
    if !window.explorer.visible {
        return;
    }
    let started = Instant::now();
    let target = active(window);
    let label = target
        .and_then(|id| window.all_panes().find(|p| p.name == id))
        .map(|pane| crate::pane_view::display_label(pane).into_owned());
    let open = view::show(
        &mut window.explorer,
        ui,
        area,
        target,
        label.as_deref(),
        window.theme,
    );
    if !window.explorer.visible {
        crate::sidebar::close(window, ui.ctx());
    }
    if let Some((pane, path)) = open {
        if crate::viewer::open_markdown_target(window, pane, path, None) {
            window.explorer.hide();
        } else {
            window.explorer.message =
                "Send or clear the review comments before opening another document.".into();
        }
    }
    window.timed_separately += started.elapsed();
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
