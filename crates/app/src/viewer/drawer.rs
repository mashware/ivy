//! The drawer's top strip: its source, the Changes, document and image tabs, the diff layout,
//! maximize and close. The host resolves clicks before frame tiling using the same layout
//! rectangles as the painting (D72).
use super::State;
use crate::sidebar::Tool;
use crate::theme::Roles;
use eframe::egui::{self, Color32, FontId, Rect, pos2, vec2};

pub(crate) const HEIGHT: f32 = 36.0;
const SOURCE: f32 = 220.0;
const CHANGES: f32 = 124.0;
const DOCUMENT: f32 = 180.0;
const IMAGE: f32 = 180.0;
const SEGMENT: f32 = 92.0;
const BUTTON: f32 = 24.0;
/// The comparison picker and the refresh button a Git tab keeps on the strip.
pub(super) const PICKER: f32 = 136.0;
const CONTROLS: f32 = PICKER + 4.0 + BUTTON;
const TAB_CLOSE: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StripAction {
    Changes,
    Document,
    Image,
    CloseChanges,
    CloseDocument,
    CloseImage,
    Unified,
    SideBySide,
    Maximize,
    Close,
}

/// What the strip shows, read from the two viewer states and the image.
pub(super) struct Tabs {
    /// The Changes tab, with its file count once a diff has loaded.
    pub(super) changes: Option<Option<usize>>,
    pub(super) document: Option<String>,
    pub(super) image: Option<String>,
    /// The drawer's tab the strip marks as chosen.
    pub(super) active: Option<Tool>,
    pub(super) side_by_side: bool,
}

pub(super) struct Strip {
    pub(super) rect: Rect,
    pub(super) source: Rect,
    pub(super) places: Vec<(StripAction, Rect)>,
    /// Where the Git tab's comparison picker and refresh button sit, when they fit.
    pub(super) controls: Option<Rect>,
    /// Something was left out for lack of room: the source's label, the Git controls, a tab or
    /// the diff toggle.
    pub(super) crowded: bool,
}

impl Strip {
    /// The strip inside the drawer's one-point frame: the host's hit test and the painting both
    /// lay it out from here.
    pub(super) fn framed(drawer: Rect, tabs: &Tabs) -> Self {
        Self::of(drawer.shrink(1.0), tabs)
    }

    pub(super) fn of(drawer: Rect, tabs: &Tabs) -> Self {
        let rect = Rect::from_min_size(
            drawer.min,
            vec2(drawer.width(), HEIGHT.min(drawer.height())),
        );
        let middle = |width: f32, x: f32| {
            Rect::from_min_size(pos2(x, rect.center().y - width / 2.0), vec2(width, width))
        };
        let tab = |x: f32, width: f32| {
            Rect::from_min_max(pos2(x, rect.top()), pos2(x + width, rect.bottom()))
        };
        let close = middle(BUTTON, rect.right() - 8.0 - BUTTON);
        let maximize = middle(BUTTON, close.left() - 2.0 - BUTTON);
        let tabs_width = tabs.changes.map_or(0.0, |_| CHANGES)
            + tabs.document.as_ref().map_or(0.0, |_| DOCUMENT)
            + tabs.image.as_ref().map_or(0.0, |_| IMAGE);
        // What gives way first in a narrow drawer: the source's label, then the Git controls, then
        // the diff's layout toggle, which has no other home; the tabs and maximize and close last.
        let available = maximize.left() - 8.0 - rect.left() - 8.0 - 4.0 - tabs_width;
        let git = tabs.active == Some(Tool::Git);
        let toggle = git && available >= 2.0 * SEGMENT + 2.0 + 8.0;
        let controls = toggle && available >= 2.0 * SEGMENT + 2.0 + 8.0 + CONTROLS + 8.0;
        let right_items = if toggle {
            2.0 * SEGMENT + 2.0 + 8.0
        } else {
            0.0
        } + if controls { CONTROLS + 8.0 } else { 0.0 };
        let room = available - right_items;
        let source = tab(
            rect.left() + 8.0,
            if room >= 60.0 { room.min(SOURCE) } else { 0.0 },
        );
        let mut places = Vec::new();
        let mut crowded = source.width() == 0.0 || (git && !controls);
        let mut x = source.right() + 4.0;
        let closer = |tab: Rect| middle(TAB_CLOSE, tab.right() - 6.0 - TAB_CLOSE);
        let candidates = [
            (
                tabs.changes.is_some(),
                Tool::Git,
                CHANGES,
                StripAction::Changes,
                StripAction::CloseChanges,
            ),
            (
                tabs.document.is_some(),
                Tool::Document,
                DOCUMENT,
                StripAction::Document,
                StripAction::CloseDocument,
            ),
            (
                tabs.image.is_some(),
                Tool::Image,
                IMAGE,
                StripAction::Image,
                StripAction::CloseImage,
            ),
        ];
        let active = candidates
            .iter()
            .position(|(present, tool, ..)| *present && Some(*tool) == tabs.active);
        let mut remaining = (maximize.left() - x).max(0.0);
        let mut widths = [0.0; 3];
        // Reserve the selected tab before giving space to inactive tabs, then paint in the usual
        // order. Even a shortened tab needs room for its complete close target and side padding.
        if let Some(active) = active {
            let width = candidates[active].2.min(remaining);
            if width >= TAB_CLOSE + 12.0 {
                widths[active] = width;
                remaining -= width;
            }
        }
        for (i, &(present, _, width, ..)) in candidates.iter().enumerate() {
            if present && Some(i) != active && width <= remaining {
                widths[i] = width;
                remaining -= width;
            }
        }
        for (i, &(present, _, width, action, close_action)) in candidates.iter().enumerate() {
            crowded |= present && widths[i] < width;
            if widths[i] > 0.0 {
                let place = tab(x, widths[i]);
                places.push((close_action, closer(place)));
                places.push((action, place));
                x += widths[i];
            }
        }
        places.push((StripAction::Maximize, maximize));
        places.push((StripAction::Close, close));
        let height = 22.0;
        let top = rect.center().y - height / 2.0;
        let mut right = maximize.left() - 8.0;
        if toggle {
            let side = Rect::from_min_size(pos2(right - SEGMENT, top), vec2(SEGMENT, height));
            let unified = Rect::from_min_size(
                pos2(side.left() - 2.0 - SEGMENT, top),
                vec2(SEGMENT, height),
            );
            places.push((StripAction::Unified, unified));
            places.push((StripAction::SideBySide, side));
            right = unified.left() - 8.0;
        }
        let controls = controls
            .then(|| Rect::from_min_size(pos2(right - CONTROLS, top), vec2(CONTROLS, height)));
        Self {
            rect,
            source,
            places,
            controls,
            crowded,
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
    let active = crate::sidebar::active(window);
    let markdown = active == Some(Tool::Document);
    let (shown, other) = (&window.viewer, &window.viewer_other);
    let is_markdown = |state: &State| state.tool_markdown.unwrap_or(true);
    let git = [shown, other].into_iter().find(|s| !is_markdown(s));
    let document = [shown, other].into_iter().find(|s| is_markdown(s));
    Tabs {
        changes: git
            .filter(|s| s.origin.is_some() || active == Some(Tool::Git))
            .map(|s| {
                s.loaded
                    .as_ref()
                    .and_then(|loaded| loaded.diff.as_ref())
                    .map(|diff| diff.files.len())
            }),
        document: document
            .and_then(|s| s.requested.as_ref().map(|(path, _)| path))
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .or_else(|| markdown.then(|| "File".to_owned())),
        image: super::image::focused(window)
            .and_then(super::image::Slot::file_name)
            .or_else(|| (active == Some(Tool::Image)).then(|| "Image".to_owned())),
        active,
        side_by_side: git.is_some_and(|s| s.review.side_by_side()),
    }
}

fn selected(action: StripAction, tabs: &Tabs, maximized: bool) -> bool {
    match action {
        StripAction::Changes => tabs.active == Some(Tool::Git),
        StripAction::Document => tabs.active == Some(Tool::Document),
        StripAction::Image => tabs.active == Some(Tool::Image),
        StripAction::Unified => !tabs.side_by_side,
        StripAction::SideBySide => tabs.side_by_side,
        StripAction::Maximize => maximized,
        StripAction::Close
        | StripAction::CloseChanges
        | StripAction::CloseDocument
        | StripAction::CloseImage => false,
    }
}

/// What the strip names its source with: the console's project in its ink, or the folder.
pub(super) struct Title {
    pub(super) project: Option<(String, Color32)>,
    pub(super) folder: String,
    pub(super) unavailable: bool,
}

impl Title {
    pub(super) fn text(&self) -> String {
        let name = self
            .project
            .as_ref()
            .map_or(self.folder.as_str(), |(name, _)| name.as_str());
        if self.unavailable {
            format!("{name} - unavailable")
        } else {
            name.to_owned()
        }
    }
}

/// Text cut with an ellipsis where it would run past `width`.
fn text(
    painter: &egui::Painter,
    at: egui::Pos2,
    align: egui::Align2,
    label: &str,
    font: FontId,
    color: Color32,
    width: f32,
) -> Rect {
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(0.0));
    let galley = painter.layout_job(job);
    let rect = align.anchor_size(at, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

pub(super) fn paint(
    ui: &egui::Ui,
    strip: &Strip,
    tabs: &Tabs,
    roles: &Roles,
    title: &Title,
    maximized: bool,
) {
    use crate::ui::icons::{Icon, paint as icon};
    let painter = ui.painter();
    let hover = ui.ctx().input(|i| i.pointer.hover_pos());
    painter.line_segment(
        [strip.rect.left_bottom(), strip.rect.right_bottom()],
        egui::Stroke::new(1.0, roles.line),
    );
    if strip.source.width() > 0.0 {
        let ink = title.project.as_ref().map_or(roles.accent, |(_, ink)| *ink);
        let stripe = Rect::from_center_size(
            pos2(strip.source.left() + 1.5, strip.source.center().y),
            vec2(3.0, 14.0),
        );
        painter.rect_filled(stripe, 2, ink);
        let font = FontId::new(12.5, crate::fonts::ui_weight(ui.ctx(), 600));
        text(
            painter,
            pos2(stripe.right() + 7.0, strip.source.center().y),
            egui::Align2::LEFT_CENTER,
            &title.text(),
            font,
            if title.project.is_some() {
                ink
            } else {
                roles.tx2
            },
            strip.source.right() - stripe.right() - 16.0,
        );
        painter.line_segment(
            [
                pos2(strip.source.right(), strip.rect.center().y - 10.0),
                pos2(strip.source.right(), strip.rect.center().y + 10.0),
            ],
            egui::Stroke::new(1.0, roles.line),
        );
    }
    if let Some(group) = segment(strip) {
        painter.rect(
            group.expand(2.0),
            6,
            roles.base,
            egui::Stroke::new(1.0, roles.line2),
            egui::StrokeKind::Inside,
        );
    }
    for (action, place) in &strip.places {
        let chosen = selected(*action, tabs, maximized);
        let hovered = hover.is_some_and(|p| place.contains(p));
        match action {
            StripAction::Changes | StripAction::Document | StripAction::Image => {
                let closing = strip.places.iter().any(|(close, at)| {
                    matches!(
                        close,
                        StripAction::CloseChanges
                            | StripAction::CloseDocument
                            | StripAction::CloseImage
                    ) && place.contains(at.center())
                        && hover.is_some_and(|p| at.contains(p))
                });
                tab(
                    painter,
                    *action,
                    *place,
                    tabs,
                    roles,
                    chosen,
                    hovered && !closing,
                )
            }
            StripAction::CloseChanges | StripAction::CloseDocument | StripAction::CloseImage => {
                if hovered {
                    painter.rect_filled(*place, 4, roles.hover);
                }
                icon(
                    painter,
                    place.center(),
                    Icon::Close,
                    10.0,
                    if hovered { roles.tx } else { roles.tx3 },
                );
            }
            StripAction::Unified | StripAction::SideBySide => {
                if chosen {
                    painter.rect_filled(*place, 4, roles.raised);
                } else if hovered {
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
                let shape = match action {
                    StripAction::Maximize if maximized => Icon::Collapse,
                    StripAction::Maximize => Icon::Expand,
                    _ => Icon::Close,
                };
                icon(
                    painter,
                    place.center(),
                    shape,
                    12.0,
                    if hovered { roles.tx } else { roles.tx2 },
                );
            }
        }
    }
}

/// The Unified / Side by side pair, framed as one segmented control.
fn segment(strip: &Strip) -> Option<Rect> {
    let place = |which| {
        strip
            .places
            .iter()
            .find(|(action, _)| *action == which)
            .map(|(_, r)| *r)
    };
    Some(place(StripAction::Unified)?.union(place(StripAction::SideBySide)?))
}

fn tab(
    painter: &egui::Painter,
    action: StripAction,
    place: Rect,
    tabs: &Tabs,
    roles: &Roles,
    chosen: bool,
    hovered: bool,
) {
    let (label, count) = match action {
        StripAction::Changes => (
            "Changes".to_owned(),
            tabs.changes.flatten().map(|n| n.to_string()),
        ),
        StripAction::Image => (tabs.image.clone().unwrap_or_default(), None),
        _ => (tabs.document.clone().unwrap_or_default(), None),
    };
    let count = count.filter(|_| place.width() >= 20.0 + TAB_CLOSE + 6.0 + 26.0);
    let room = place.width() - 20.0 - TAB_CLOSE - 6.0 - count.as_ref().map_or(0.0, |_| 26.0);
    let label_bounds = Rect::from_min_size(
        pos2(place.left() + 10.0, place.top()),
        vec2(room.max(0.0), place.height()),
    );
    // Egui can return an ellipsis wider than the requested width. Its visible edge, rather than
    // that galley's natural edge, is where the count starts on a compressed tab.
    let written = text(
        &painter.with_clip_rect(label_bounds),
        pos2(place.left() + 10.0, place.center().y),
        egui::Align2::LEFT_CENTER,
        &label,
        FontId::proportional(12.5),
        if chosen || hovered {
            roles.tx
        } else {
            roles.tx2
        },
        room,
    );
    if let Some(count) = count {
        let chip = Rect::from_min_size(
            pos2(
                written.right().min(label_bounds.right()) + 6.0,
                place.center().y - 8.0,
            ),
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

/// The hover text each place carries for assistive technology and the pointer.
pub(super) fn hint(action: StripAction, maximized: bool) -> &'static str {
    match action {
        StripAction::Changes => "Review changes",
        StripAction::Document => "File",
        StripAction::Image => "Image",
        StripAction::CloseChanges => "Close the review",
        StripAction::CloseDocument => "Close the document",
        StripAction::CloseImage => "Close the image",
        StripAction::Unified => "Unified diff",
        StripAction::SideBySide => "Side-by-side diff",
        StripAction::Maximize if maximized => "Restore drawer",
        StripAction::Maximize => "Maximize drawer",
        StripAction::Close => "Close drawer",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewer::StripAction;

    const TAB_PAIRS: [(Tool, StripAction, StripAction); 3] = [
        (Tool::Git, StripAction::Changes, StripAction::CloseChanges),
        (
            Tool::Document,
            StripAction::Document,
            StripAction::CloseDocument,
        ),
        (Tool::Image, StripAction::Image, StripAction::CloseImage),
    ];

    fn three_tabs(active: Tool) -> Tabs {
        Tabs {
            changes: Some(Some(3)),
            document: Some("plan.md".into()),
            image: Some("shot.png".into()),
            active: Some(active),
            side_by_side: false,
        }
    }

    fn strip(width: f32, tabs: &Tabs) -> Strip {
        Strip::framed(
            Rect::from_min_size(pos2(20.0, 30.0), vec2(width, 370.0)),
            tabs,
        )
    }

    fn place(strip: &Strip, action: StripAction) -> Option<Rect> {
        strip
            .places
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, r)| *r)
    }

    fn assert_paired_hit_targets(strip: &Strip) {
        let maximize = place(strip, StripAction::Maximize).unwrap();
        for (_, tab, close) in TAB_PAIRS {
            match (place(strip, tab), place(strip, close)) {
                (Some(body), Some(button)) => {
                    assert!(strip.rect.contains_rect(body), "{tab:?} escapes the strip");
                    assert!(body.right() <= maximize.left(), "{tab:?} overlaps controls");
                    assert!(body.contains_rect(button), "{close:?} escapes its tab");
                    assert_eq!(strip.at(button.center()), Some(close));
                    assert_eq!(
                        strip.at(pos2(body.left() + 2.0, body.center().y)),
                        Some(tab)
                    );
                }
                (None, None) => {}
                _ => panic!("{tab:?} and {close:?} must appear together"),
            }
        }
    }

    #[test]
    fn selected_tab_stays_visible_and_hittable_in_a_narrow_drawer() {
        for width in [300.0, 450.0, 550.0] {
            for (active, tab, _) in TAB_PAIRS {
                let strip = strip(width, &three_tabs(active));
                assert!(place(&strip, tab).is_some(), "{active:?} hidden at {width}");
                assert_paired_hit_targets(&strip);
            }
        }
    }

    #[test]
    fn omitted_tabs_leave_no_orphan_close_targets() {
        for width in [300.0, 450.0, 550.0] {
            for (active, _, _) in TAB_PAIRS {
                assert_paired_hit_targets(&strip(width, &three_tabs(active)));
            }
        }
    }

    #[test]
    fn selected_tab_compresses_when_its_normal_width_does_not_fit() {
        for (active, tab, _) in TAB_PAIRS {
            let strip = strip(180.0, &three_tabs(active));
            let body = place(&strip, tab).expect("selected tab must fit by compressing");
            let normal = if active == Tool::Git {
                CHANGES
            } else {
                DOCUMENT
            };
            assert!(body.width() > TAB_CLOSE && body.width() < normal);
            assert_paired_hit_targets(&strip);
            assert!(strip.crowded);
        }
    }

    #[test]
    fn wide_strip_keeps_tab_order_and_each_close_target() {
        for (active, _, _) in TAB_PAIRS {
            let strip = strip(1400.0, &three_tabs(active));
            let bodies: Vec<_> = TAB_PAIRS
                .iter()
                .map(|(_, tab, _)| place(&strip, *tab).expect("wide drawer keeps every tab"))
                .collect();
            assert!(
                bodies
                    .windows(2)
                    .all(|pair| pair[0].right() <= pair[1].left())
            );
            assert_paired_hit_targets(&strip);
        }
    }

    #[test]
    fn painted_tab_content_stays_before_its_close_target() {
        let roles = Roles::of(&crate::theme::Theme::default());
        let mut overlaps = Vec::new();
        for width in [100.0, 140.0, 142.0, 150.0, 300.0] {
            let tabs = three_tabs(Tool::Git);
            let strip = strip(width, &tabs);
            let body = place(&strip, StripAction::Changes).unwrap();
            let close = place(&strip, StripAction::CloseChanges).unwrap();
            let mut output = egui::Context::default().run_ui(Default::default(), |ui| {
                super::tab(
                    ui.painter(),
                    StripAction::Changes,
                    body,
                    &tabs,
                    &roles,
                    true,
                    false,
                );
            });
            output.textures_delta.clear();
            let mut texts = 0;
            let mut chips = 0;
            for shape in output.shapes {
                let bounds = match shape.shape {
                    egui::Shape::Text(text) => {
                        texts += 1;
                        text.galley.rect.translate(text.pos.to_vec2())
                    }
                    egui::Shape::Rect(rect) if rect.fill == roles.hover => {
                        chips += 1;
                        rect.rect
                    }
                    _ => continue,
                };
                let visible = bounds.intersect(shape.clip_rect);
                if visible.is_positive() && visible.right() > close.left() {
                    overlaps.push((width, visible, close));
                }
            }
            if width == 300.0 {
                assert_eq!(texts, 2, "ordinary tab paints its label and count");
                assert_eq!(chips, 1, "ordinary tab paints its count chip");
            }
        }
        assert!(
            overlaps.is_empty(),
            "tab content overlaps close targets: {overlaps:?}"
        );
    }

    /// With an image beside the document and the review, the strip lays out all three tabs, each
    /// with its close button, at 718 points: the width the review's narrow-strip test holds the
    /// strip to, chosen as roughly what a 1000-point window with Files open left the drawer.
    /// Narrower, a strip that leaves any of them out says so.
    #[test]
    fn a_strip_with_three_tabs_lays_them_all_out_or_says_it_could_not() {
        let tabs = Tabs {
            changes: Some(Some(3)),
            document: Some("plan.md".into()),
            image: Some("shot.png".into()),
            active: Some(crate::sidebar::Tool::Image),
            side_by_side: false,
        };
        let has = |strip: &Strip, action| strip.places.iter().any(|(a, _)| *a == action);
        let tabs_and_closes = [
            StripAction::Changes,
            StripAction::Document,
            StripAction::Image,
            StripAction::CloseChanges,
            StripAction::CloseDocument,
            StripAction::CloseImage,
        ];
        let strip = |width: f32| {
            Strip::framed(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 370.0)),
                &tabs,
            )
        };
        let laid = strip(718.0);
        for action in tabs_and_closes {
            assert!(has(&laid, action), "{action:?} left out at 718 points");
        }
        for width in [300.0, 450.0, 550.0] {
            let narrow = strip(width);
            if tabs_and_closes.iter().any(|&action| !has(&narrow, action)) {
                assert!(
                    narrow.crowded,
                    "a tab left out at {width} points was not said"
                );
            }
        }
        assert!(
            tabs_and_closes
                .iter()
                .any(|&action| !has(&strip(300.0), action))
        );
    }
}
