//! The drawer's top strip: its source, the Changes and document tabs, the diff layout, maximize and
//! close. Its places are fixed widths so the host can resolve a click before the frame's tiling
//! with the same rectangles the painting uses (D72).
use super::State;
use crate::theme::Roles;
use eframe::egui::{self, Color32, FontId, Rect, pos2, vec2};

pub(crate) const HEIGHT: f32 = 36.0;
const SOURCE: f32 = 220.0;
const CHANGES: f32 = 112.0;
const DOCUMENT: f32 = 168.0;
const SEGMENT: f32 = 92.0;
const BUTTON: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StripAction {
    Changes,
    Document,
    Unified,
    SideBySide,
    Maximize,
    Close,
}

/// What the strip shows, read from the two viewer states.
pub(super) struct Tabs {
    pub(super) changes: Option<usize>,
    pub(super) document: Option<String>,
    pub(super) markdown: bool,
    pub(super) side_by_side: bool,
}

pub(super) struct Strip {
    pub(super) rect: Rect,
    pub(super) source: Rect,
    pub(super) places: Vec<(StripAction, Rect)>,
}

impl Strip {
    pub(super) fn of(drawer: Rect, tabs: &Tabs) -> Self {
        let rect = Rect::from_min_size(drawer.min, vec2(drawer.width(), HEIGHT.min(drawer.height())));
        let middle = |width: f32, x: f32| {
            Rect::from_min_size(pos2(x, rect.center().y - width / 2.0), vec2(width, width))
        };
        let tab = |x: f32, width: f32| Rect::from_min_max(pos2(x, rect.top()), pos2(x + width, rect.bottom()));
        let source = tab(rect.left() + 8.0, SOURCE.min((rect.width() - 8.0).max(0.0)));
        let mut places = Vec::new();
        let mut x = source.right() + 4.0;
        if tabs.changes.is_some() {
            places.push((StripAction::Changes, tab(x, CHANGES)));
            x += CHANGES;
        }
        if tabs.document.is_some() {
            places.push((StripAction::Document, tab(x, DOCUMENT)));
            x += DOCUMENT;
        }
        let close = middle(BUTTON, rect.right() - 8.0 - BUTTON);
        let maximize = middle(BUTTON, close.left() - 2.0 - BUTTON);
        places.push((StripAction::Maximize, maximize));
        places.push((StripAction::Close, close));
        if !tabs.markdown {
            let right = maximize.left() - 8.0;
            let height = 20.0;
            let top = rect.center().y - height / 2.0;
            let side = Rect::from_min_size(pos2(right - 2.0 - SEGMENT, top), vec2(SEGMENT, height));
            let unified =
                Rect::from_min_size(pos2(side.left() - 2.0 - SEGMENT, top), vec2(SEGMENT, height));
            if unified.left() > x {
                places.push((StripAction::Unified, unified));
                places.push((StripAction::SideBySide, side));
            }
        }
        // A narrow drawer keeps maximize and close; a tab that would run under them is dropped.
        let reserved = maximize.left();
        places.retain(|(action, place)| {
            matches!(action, StripAction::Maximize | StripAction::Close) || place.right() <= reserved
        });
        Self {
            rect,
            source,
            places,
        }
    }

    pub(super) fn at(&self, pos: egui::Pos2) -> Option<StripAction> {
        self.places
            .iter()
            .find(|(_, place)| place.contains(pos))
            .map(|(action, _)| *action)
    }
}

pub(super) fn tabs(window: &crate::Window) -> Tabs {
    let markdown = crate::sidebar::active(window) == Some(crate::sidebar::Tool::Markdown);
    let (shown, other) = (&window.viewer, &window.viewer_other);
    let is_markdown = |state: &State| state.tool_markdown.unwrap_or(true);
    let git = [shown, other].into_iter().find(|s| !is_markdown(s));
    let document = [shown, other].into_iter().find(|s| is_markdown(s));
    Tabs {
        changes: git.filter(|s| s.origin.is_some() || !markdown).map(|s| {
            s.loaded
                .as_ref()
                .and_then(|loaded| loaded.diff.as_ref())
                .map_or(0, |diff| diff.files.len())
        }),
        document: document
            .and_then(|s| s.requested.as_ref().map(|(path, _)| path))
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .or_else(|| markdown.then(|| "Markdown".to_owned())),
        markdown,
        side_by_side: git.is_some_and(|s| s.review.side_by_side()),
    }
}

fn selected(action: StripAction, tabs: &Tabs, maximized: bool) -> bool {
    match action {
        StripAction::Changes => !tabs.markdown,
        StripAction::Document => tabs.markdown,
        StripAction::Unified => !tabs.side_by_side,
        StripAction::SideBySide => tabs.side_by_side,
        StripAction::Maximize => maximized,
        StripAction::Close => false,
    }
}

pub(super) fn paint(
    ui: &egui::Ui,
    strip: &Strip,
    tabs: &Tabs,
    roles: &Roles,
    source: &str,
    maximized: bool,
) {
    let painter = ui.painter();
    let hover = ui.ctx().input(|i| i.pointer.hover_pos());
    painter.line_segment(
        [strip.rect.left_bottom(), strip.rect.right_bottom()],
        egui::Stroke::new(1.0, roles.line),
    );
    let font = FontId::proportional(12.5);
    let text = |at: egui::Pos2, align: egui::Align2, label: &str, color: Color32, width: f32| {
        let galley = painter.layout(
            label.to_owned(),
            font.clone(),
            color,
            f32::INFINITY,
        );
        let galley = if galley.size().x > width {
            let mut job = egui::text::LayoutJob::simple_singleline(
                label.to_owned(),
                font.clone(),
                color,
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(width);
            painter.layout_job(job)
        } else {
            galley
        };
        let rect = align.anchor_size(at, galley.size());
        painter.galley(rect.min, galley, color);
        rect
    };
    let stripe = Rect::from_center_size(
        pos2(strip.source.left() + 2.0, strip.source.center().y),
        vec2(3.0, 14.0),
    );
    painter.rect_filled(stripe, 2, roles.accent);
    text(
        pos2(stripe.right() + 6.0, strip.source.center().y),
        egui::Align2::LEFT_CENTER,
        source,
        roles.tx2,
        (strip.source.right() - stripe.right() - 14.0).max(0.0),
    );
    painter.line_segment(
        [
            pos2(strip.source.right(), strip.rect.center().y - 10.0),
            pos2(strip.source.right(), strip.rect.center().y + 10.0),
        ],
        egui::Stroke::new(1.0, roles.line),
    );
    for (action, place) in &strip.places {
        let chosen = selected(*action, tabs, maximized);
        let hovered = hover.is_some_and(|p| place.contains(p));
        match action {
            StripAction::Changes | StripAction::Document => {
                let ink = if chosen { roles.tx } else { roles.tx2 };
                let label = match action {
                    StripAction::Changes => "Changes".to_owned(),
                    _ => tabs.document.clone().unwrap_or_default(),
                };
                let count = match action {
                    StripAction::Changes => tabs.changes.map(|n| n.to_string()),
                    _ => None,
                };
                let room = place.width() - 20.0 - count.as_ref().map_or(0.0, |_| 26.0);
                let written = text(
                    pos2(place.left() + 10.0, place.center().y),
                    egui::Align2::LEFT_CENTER,
                    &label,
                    if hovered { roles.tx } else { ink },
                    room.max(0.0),
                );
                if let Some(count) = count {
                    let chip = Rect::from_min_size(
                        pos2(written.right() + 6.0, place.center().y - 8.0),
                        vec2(20.0, 16.0),
                    );
                    painter.rect_filled(chip, 4, roles.hover);
                    painter.text(
                        chip.center(),
                        egui::Align2::CENTER_CENTER,
                        count,
                        FontId::proportional(10.5),
                        roles.tx2,
                    );
                }
                if chosen {
                    painter.rect_filled(
                        Rect::from_min_max(pos2(place.left(), place.bottom() - 2.0), place.max),
                        0,
                        roles.accent,
                    );
                }
            }
            StripAction::Unified | StripAction::SideBySide => {
                if *action == StripAction::Unified {
                    let group = place.union(
                        strip
                            .places
                            .iter()
                            .find(|(a, _)| *a == StripAction::SideBySide)
                            .map_or(*place, |(_, r)| *r),
                    );
                    painter.rect(
                        group.expand(2.0),
                        6,
                        roles.base,
                        egui::Stroke::new(1.0, roles.line2),
                        egui::StrokeKind::Inside,
                    );
                }
                if chosen || hovered {
                    painter.rect_filled(*place, 4, roles.hover);
                }
                painter.text(
                    place.center(),
                    egui::Align2::CENTER_CENTER,
                    if *action == StripAction::Unified {
                        "Unified"
                    } else {
                        "Side by side"
                    },
                    FontId::proportional(12.0),
                    if chosen { roles.tx } else { roles.tx2 },
                );
            }
            StripAction::Maximize | StripAction::Close => {
                if hovered {
                    painter.rect_filled(*place, 5, roles.hover);
                }
                let target = if *action == StripAction::Maximize {
                    crate::header::integrated::Target::Zoom
                } else {
                    crate::header::integrated::Target::Close
                };
                crate::header::integrated::paint_icon(
                    painter,
                    place.center(),
                    target,
                    maximized,
                    roles.tx2,
                    20.0,
                );
            }
        }
    }
}

/// The hover text each place carries for assistive technology and the pointer.
pub(super) fn hint(action: StripAction, maximized: bool) -> &'static str {
    match action {
        StripAction::Changes => "Review changes",
        StripAction::Document => "Markdown document",
        StripAction::Unified => "Unified diff",
        StripAction::SideBySide => "Side-by-side diff",
        StripAction::Maximize if maximized => "Restore drawer",
        StripAction::Maximize => "Maximize drawer",
        StripAction::Close => "Close drawer",
    }
}
