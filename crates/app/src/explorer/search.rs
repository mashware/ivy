//! Files' name search as a floating list over the window (`Files.dc.html`, D72): the existing
//! worker finds the names below the displayed root; Enter reveals a match in the tree and
//! Ctrl+Enter opens it.
use super::{Entry, EntryKind, Explorer, Request, display};
use crate::theme::metrics::MENU_ROW_HEIGHT;
use eframe::egui;
use std::path::{Path, PathBuf};

const WIDTH: f32 = 360.0;
const ROWS: usize = 8;

/// What a person chose in the list.
pub(super) enum Pick {
    /// Show the match in the tree, its folders expanded, without opening it.
    Reveal(PathBuf),
    /// Open it: a Markdown file in the drawer; anything else is revealed.
    Open(Entry),
}

/// The matches for the pane's query, what to say instead, and the request to run when the
/// worker has not answered yet.
struct Matches {
    entries: Vec<Entry>,
    said: Option<String>,
    pending: Option<Request>,
}

fn matches(state: &mut Explorer, pane: usize) -> Matches {
    let nav = &state.navigation[&pane];
    let (Some(root), false) = (nav.root.clone(), nav.query.trim().is_empty()) else {
        return Matches {
            entries: Vec::new(),
            said: None,
            pending: None,
        };
    };
    let request = state.request(pane, root);
    state.clock = state.clock.wrapping_add(1);
    let Some(cached) = state.cache.get_mut(&request) else {
        return Matches {
            entries: Vec::new(),
            said: Some("Searching…".into()),
            pending: Some(request),
        };
    };
    cached.used = state.clock;
    match &cached.result {
        Ok(listing) if listing.entries.is_empty() => Matches {
            entries: Vec::new(),
            said: Some("No matching files or folders.".into()),
            pending: None,
        },
        Ok(listing) => Matches {
            entries: listing.entries.clone(),
            said: listing.notice.clone(),
            pending: None,
        },
        Err(error) => Matches {
            entries: Vec::new(),
            said: Some(error.clone()),
            pending: None,
        },
    }
}

/// Where the list floats: just below Files' header, overhanging the consoles, inside the window.
pub(super) fn place(files: egui::Rect, screen: egui::Rect, rows: usize) -> egui::Rect {
    let width = WIDTH.min((screen.right() - files.left() - 8.0).max(0.0));
    let height = 4.0 * 2.0 + 30.0 + 6.0 + rows.min(ROWS) as f32 * MENU_ROW_HEIGHT + 28.0;
    egui::Rect::from_min_size(
        egui::pos2(files.left() + 8.0, files.top() + 34.0),
        egui::vec2(width, height.min((screen.bottom() - files.top() - 40.0).max(0.0))),
    )
}

/// The keys the open list answers before the text field sees them.
pub(super) fn keys(state: &mut Explorer, pane: usize, ctx: &egui::Context) -> Option<Pick> {
    let found = matches(state, pane).entries;
    let pressed = |key| ctx.input(|i| i.key_pressed(key));
    if pressed(egui::Key::ArrowDown) {
        state.search_index = (state.search_index + 1).min(found.len().saturating_sub(1));
    }
    if pressed(egui::Key::ArrowUp) {
        state.search_index = state.search_index.saturating_sub(1);
    }
    if !pressed(egui::Key::Enter) {
        return None;
    }
    let entry = found.get(state.search_index)?.clone();
    if ctx.input(|i| i.modifiers.command) {
        Some(Pick::Open(entry))
    } else {
        Some(Pick::Reveal(entry.path))
    }
}

/// Paint the list and its field; returns what a click chose.
pub(super) fn show(
    state: &mut Explorer,
    ctx: &egui::Context,
    pane: usize,
    files: egui::Rect,
    theme: &crate::theme::Theme,
) -> Option<Pick> {
    let found = matches(state, pane);
    if state.loading.is_none()
        && let Some(request) = found.pending.clone()
    {
        state.loader.submit(request.clone(), ctx);
        state.loading = Some(request);
    }
    state.search_index = state
        .search_index
        .min(found.entries.len().saturating_sub(1));
    let rect = place(files, ctx.content_rect(), found.entries.len().max(1));
    state.search_rect = Some(rect);
    let roles = crate::ui::style::roles(ctx, theme);
    let root = state.navigation[&pane].root.clone().unwrap_or_default();
    let mut pick = None;
    egui::Area::new(egui::Id::new(("explorer-search", pane)))
        .order(egui::Order::Tooltip)
        .fixed_pos(rect.min)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            crate::ui::floating::style(ui, theme);
            ui.set_clip_rect(rect.expand(12.0));
            crate::ui::floating::menu_frame(&roles).show(ui, |ui| {
                let inner = rect.width() - 2.0 * 4.0 - 2.0;
                ui.set_width(inner);
                let nav = state.navigation.get_mut(&pane).unwrap();
                let mut query = nav.query.clone();
                let field = ui.add(
                    egui::TextEdit::singleline(&mut query)
                        .id(egui::Id::new(("explorer-name-search", pane)))
                        .hint_text("Search files and folders")
                        .char_limit(256)
                        .desired_width(inner),
                );
                if std::mem::take(&mut state.focus_search) {
                    field.request_focus();
                }
                if field.changed() {
                    nav.query = query;
                    nav.refresh();
                    state.search_index = 0;
                    state.message.clear();
                }
                let rows = found.entries.len().min(ROWS);
                egui::ScrollArea::vertical()
                    .id_salt(("explorer-search-results", pane))
                    .max_height(rows as f32 * MENU_ROW_HEIGHT)
                    .auto_shrink([false, true])
                    .show_rows(ui, MENU_ROW_HEIGHT, found.entries.len(), |ui, range| {
                        for index in range {
                            let entry = &found.entries[index];
                            let chosen = index == state.search_index;
                            if let Some(choice) = row(ui, entry, &root, chosen, &roles) {
                                state.search_index = index;
                                pick = Some(choice);
                            }
                        }
                    });
                let count = match &found.said {
                    Some(said) if found.entries.is_empty() => said.clone(),
                    Some(said) => format!("{} · {said}", count(found.entries.len())),
                    None => count(found.entries.len()),
                };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(count).size(11.0).color(roles.tx3));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Enter reveals · Ctrl+Enter opens")
                                .size(11.0)
                                .color(roles.tx3),
                        );
                    });
                });
            });
        });
    pick
}

fn count(n: usize) -> String {
    match n {
        1 => "1 match".into(),
        n => format!("{n} matches"),
    }
}

fn row(
    ui: &mut egui::Ui,
    entry: &Entry,
    root: &Path,
    chosen: bool,
    roles: &crate::theme::Roles,
) -> Option<Pick> {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), MENU_ROW_HEIGHT),
        egui::Sense::click(),
    );
    if chosen {
        ui.painter().rect_filled(rect, 6, roles.sel);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 6, roles.hover);
    }
    let directory = entry.kind == EntryKind::Directory;
    let icon = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 8.0, rect.center().y - 8.0),
        egui::vec2(16.0, 16.0),
    );
    crate::sidebar::paint_file_icon(
        ui.painter(),
        icon,
        directory,
        egui::Stroke::new(1.0, if directory { roles.accent } else { roles.tx2 }),
    );
    let name = display(Path::new(entry.path.file_name().unwrap_or_default()));
    let folder = entry
        .path
        .parent()
        .and_then(|parent| parent.strip_prefix(root).ok())
        .map(|parent| {
            let shown = display(parent);
            if shown.is_empty() {
                "./".to_owned()
            } else {
                format!("{shown}/")
            }
        })
        .unwrap_or_default();
    let font = egui::FontId::proportional(12.5);
    let name_galley = ui.painter().layout_no_wrap(name, font.clone(), roles.tx);
    let at = egui::pos2(icon.right() + 8.0, rect.center().y - name_galley.size().y / 2.0);
    let name_right = at.x + name_galley.size().x;
    ui.painter().galley(at, name_galley, roles.tx);
    ui.painter().text(
        egui::pos2(name_right + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        folder,
        egui::FontId::proportional(11.0),
        roles.tx3,
    );
    let response = response.on_hover_text(display(&entry.path));
    response.clicked().then(|| {
        if ui.input(|i| i.modifiers.command) {
            Pick::Open(entry.clone())
        } else {
            Pick::Reveal(entry.path.clone())
        }
    })
}

/// Show `path` in the tree: every folder between the root and it expanded, it selected and
/// scrolled into view. The root stays where it is.
pub(super) fn reveal(state: &mut Explorer, pane: usize, path: &Path) {
    let Some(nav) = state.navigation.get_mut(&pane) else {
        return;
    };
    let Some(root) = nav.root.clone() else {
        return;
    };
    let mut folder = path.parent();
    while let Some(parent) = folder.filter(|parent| parent.starts_with(&root) && *parent != root) {
        nav.expanded.insert(parent.to_path_buf());
        folder = parent.parent();
    }
    nav.selected = Some(path.to_path_buf());
    nav.reveal_selected = true;
    nav.query.clear();
    nav.refresh();
    state.searching = false;
    state.search_rect = None;
}
