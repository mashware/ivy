//! The row above a file in the code view: where it lives, and what can be done with it.
use super::code::CodeState;
use crate::opener;
use crate::ui::icons::{FileKind, Icon};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

/// What Open's tooltip says when the file opens with its application, and when it is shown in the
/// file manager instead.
const OPENS: &str = "Open with the default application";
const REVEALS: &str = "Show in the file manager: only documents are opened from here";
const UNSAFE: &str = "Not copied: the path has characters that do not show as themselves";

/// The open file once its links are followed, found off the window's thread.
struct Resolved {
    /// Where the file really is; nothing when it cannot be found.
    canonical: Option<PathBuf>,
    /// The Files folder it was opened from, links followed the same way.
    root: Option<PathBuf>,
    /// Whether Open hands it to its application rather than showing it.
    launches: bool,
}

#[derive(Default)]
pub(crate) struct Header {
    /// The Files folder of the source console when the file was opened. Files follows the focused
    /// console, so the folder a relative path counts from is taken then, not read later.
    pub(super) root: Option<PathBuf>,
    asked: Option<(PathBuf, Option<PathBuf>)>,
    answer: Option<Receiver<Resolved>>,
    resolved: Option<Resolved>,
    desktop: opener::Desktop,
    /// A folder of the path somebody clicked, for Files to show once the drawer is painted.
    reveal: Option<PathBuf>,
}

impl Header {
    /// Ask again where the file lives whenever the file or its root changed.
    fn refresh(&mut self, path: &Path, ctx: &egui::Context) {
        let asking = (path.to_owned(), self.root.clone());
        if self.asked.as_ref() != Some(&asking) {
            self.asked = Some(asking.clone());
            self.resolved = None;
            let (send, answer) = mpsc::channel();
            let ctx = ctx.clone();
            let spawned = std::thread::Builder::new()
                .name("file header".into())
                .spawn(move || {
                    let _ = send.send(resolve(&asking.0, asking.1.as_deref()));
                    ctx.request_repaint();
                });
            match spawned {
                Ok(_) => self.answer = Some(answer),
                Err(_) => self.resolved = Some(resolve_nothing()),
            }
        }
        if let Some(resolved) = self.answer.as_ref().and_then(|a| a.try_recv().ok()) {
            self.resolved = Some(resolved);
            self.answer = None;
        }
    }
}

fn resolve(path: &Path, root: Option<&Path>) -> Resolved {
    let canonical = opener::canonical(path).ok();
    Resolved {
        launches: canonical.as_deref().is_some_and(opener::may_launch),
        root: root.and_then(|root| opener::canonical(root).ok()),
        canonical,
    }
}

fn resolve_nothing() -> Resolved {
    Resolved {
        canonical: None,
        root: None,
        launches: false,
    }
}

/// `path` as seen from `root`; nothing when it lies outside or either is unknown.
pub(super) fn relative<'a>(path: &'a Path, root: Option<&Path>) -> Option<&'a Path> {
    path.strip_prefix(root?)
        .ok()
        .filter(|rest| !rest.as_os_str().is_empty())
}

/// The header row for the file at `path`, as Files gave it.
pub(super) fn show(
    code: &mut CodeState,
    path: &Path,
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
) {
    let header = &mut code.header;
    header.refresh(path, ui.ctx());
    let resolved = header.resolved.as_ref();
    let absolute = resolved
        .and_then(|r| r.canonical.clone())
        .unwrap_or_else(|| path.to_owned());
    let root = resolved.and_then(|r| r.root.clone());
    let inside = resolved
        .and_then(|r| relative(r.canonical.as_deref()?, root.as_deref()))
        .map(Path::to_owned);
    let launches = resolved.is_some_and(|r| r.launches);
    let roles = crate::ui::style::roles(ui.ctx(), theme);
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let more = ui
                        .add(egui::Button::new("").min_size(egui::vec2(24.0, 24.0)))
                        .on_hover_text("More");
                    let ink = ui.style().interact(&more).fg_stroke.color;
                    crate::ui::icons::paint(
                        ui.painter(),
                        more.rect.center(),
                        Icon::Caret { open: true },
                        12.0,
                        ink,
                    );
                    egui::Popup::menu(&more)
                        .frame(crate::ui::floating::menu_frame(&roles))
                        .show(|ui| {
                            let copy = Copies {
                                absolute: &absolute,
                                inside: inside.as_deref(),
                                root: root.as_deref(),
                                resolved: resolved.is_some(),
                            };
                            menu(ui, &mut header.desktop, &mut code.wrap, path, &copy);
                        });
                    let open =
                        ui.button("Open")
                            .on_hover_text(if launches { OPENS } else { REVEALS });
                    if open.clicked() {
                        header.desktop.open(path);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let crumbs = Crumbs {
                            absolute: &absolute,
                            root: root.as_deref(),
                            inside: inside.as_deref(),
                        };
                        if let Some(folder) = crumbs.show(ui, theme, &roles) {
                            header.reveal = Some(folder);
                        }
                    });
                });
            });
        });
    ui.separator();
}

/// What the two Copy items would copy, and why one of them cannot.
struct Copies<'a> {
    absolute: &'a Path,
    inside: Option<&'a Path>,
    root: Option<&'a Path>,
    resolved: bool,
}

fn menu(
    ui: &mut egui::Ui,
    desktop: &mut opener::Desktop,
    wrap: &mut bool,
    path: &Path,
    copy: &Copies,
) {
    if ui.button("Open in File Manager").clicked() {
        desktop.reveal(path);
    }
    let absolute = copyable(Some(copy.absolute));
    let item = ui
        .add_enabled(absolute.is_ok(), egui::Button::new("Copy absolute path"))
        .on_disabled_hover_text(absolute.clone().err().unwrap_or_default());
    if item.clicked()
        && let Ok(text) = absolute
    {
        ui.ctx().copy_text(text);
    }
    let relative = match (copy.inside, copy.root) {
        (Some(inside), _) => copyable(Some(inside)),
        _ if !copy.resolved => Err("Still finding where this file lives".to_owned()),
        (None, Some(root)) => Err(format!(
            "Outside the Files folder it was opened from ({})",
            crate::explorer::display(root)
        )),
        (None, None) => Err("Not opened from a Files folder".to_owned()),
    };
    let item = ui
        .add_enabled(relative.is_ok(), egui::Button::new("Copy relative path"))
        .on_disabled_hover_text(relative.clone().err().unwrap_or_default());
    if item.clicked()
        && let Ok(text) = relative
    {
        ui.ctx().copy_text(text);
    }
    ui.checkbox(wrap, "Wrap lines");
}

/// The text a Copy item puts on the clipboard, literally, or why it puts nothing: a line break
/// would run on paste and a direction override would paste something else than it showed.
fn copyable(path: Option<&Path>) -> Result<String, String> {
    let text = path
        .and_then(Path::to_str)
        .ok_or_else(|| UNSAFE.to_owned())?;
    if opener::shown_as_itself(text) {
        Ok(text.to_owned())
    } else {
        Err(UNSAFE.to_owned())
    }
}

/// The path as folders from the Files root down to the file, or whole when it lies outside.
struct Crumbs<'a> {
    absolute: &'a Path,
    root: Option<&'a Path>,
    inside: Option<&'a Path>,
}

impl Crumbs<'_> {
    /// Paint the path; the folder somebody clicked, if any.
    fn show(
        &self,
        ui: &mut egui::Ui,
        theme: &crate::theme::Theme,
        roles: &crate::theme::Roles,
    ) -> Option<PathBuf> {
        ui.spacing_mut().item_spacing.x = 4.0;
        let mut clicked = None;
        let name = |path: &Path| {
            crate::explorer::display(Path::new(path.file_name().unwrap_or(path.as_os_str())))
        };
        if let (Some(root), Some(inside)) = (self.root, self.inside) {
            let mut folder = root.to_owned();
            let parents: Vec<_> = inside.parent().into_iter().flat_map(Path::iter).collect();
            for (n, step) in std::iter::once(None)
                .chain(parents.into_iter().map(Some))
                .enumerate()
            {
                if let Some(step) = step {
                    folder.push(step);
                }
                if n > 0 {
                    ui.label(egui::RichText::new("›").color(roles.tx3));
                }
                let link = ui
                    .add(egui::Label::new(name(&folder)).sense(egui::Sense::click()))
                    .on_hover_text(format!(
                        "Show {} in Files",
                        crate::explorer::display(&folder)
                    ))
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if link.clicked() {
                    clicked = Some(folder.clone());
                }
            }
            ui.label(egui::RichText::new("›").color(roles.tx3));
        }
        let kind = FileKind::of(self.absolute);
        let (icon, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
        let grounds = crate::ui::icons::row_grounds(roles.base, roles.base, roles.base);
        crate::ui::icons::paint(
            ui.painter(),
            icon.center(),
            Icon::File(kind),
            14.0,
            kind.color(theme, roles, &grounds),
        );
        let last = if self.inside.is_some() {
            name(self.absolute)
        } else {
            crate::explorer::display(self.absolute)
        };
        ui.add(egui::Label::new(last).truncate())
            .on_hover_text(crate::explorer::display(self.absolute));
        clicked
    }
}

/// Show in Files the folder a click on the path asked for, from the console the file came from.
pub(super) fn reveal_in_files(window: &mut crate::Window) {
    let Some(folder) = window.viewer.code.header.reveal.take() else {
        return;
    };
    if let Some(pane) = window.viewer.origin.as_ref().map(|origin| origin.pane) {
        crate::explorer::reveal_folder(window, pane, folder);
    }
}

/// What Copy relative path would give for the open document, links followed; nothing if it lies
/// outside the folder it was opened from.
#[cfg(test)]
pub(crate) fn document_relative_path(window: &crate::Window) -> Option<PathBuf> {
    let state = &window.viewer;
    let resolved = resolve(Path::new(&state.path), state.code.header.root.as_deref());
    relative(resolved.canonical.as_deref()?, resolved.root.as_deref()).map(Path::to_owned)
}

#[cfg(test)]
mod tests;
