//! Shared paint and hit geometry for the integrated pane header.
use crate::{
    painter::CellMetrics,
    theme::{Theme, metrics},
};
use eframe::egui::{self, Color32};

/// Inputs shared by the historical and integrated strip painting regimes.
pub(crate) struct Strip {
    pub(crate) rect: egui::Rect,
    pub(crate) header: egui::Rect,
    pub(crate) headers: super::Headers,
    pub(crate) button: super::Button,
    pub(crate) integrated: bool,
    pub(crate) maximized: Option<usize>,
    pub(crate) now: std::time::SystemTime,
    /// Whether the window hands its keys to this console.
    pub(crate) focused: bool,
}

/// What a console's header calls it: the name somebody gave it; the title an agent set; otherwise
/// its foreground program — after its folder when no project names the place — rather than the
/// shell's `user@host:path` title, which the owner found said nothing (17 November 2026).
fn header_label(pane: &crate::Pane) -> std::borrow::Cow<'_, str> {
    let named = pane
        .label
        .as_deref()
        .is_some_and(|label| !label.trim().is_empty());
    let agent = pane.signals.view(std::time::Instant::now()).is_some();
    match &pane.program {
        Some((folder, program)) if !named && !agent => {
            let mut letters = program.chars();
            let program: String = letters
                .next()
                .map(|first| first.to_uppercase().chain(letters).collect())
                .unwrap_or_default();
            if pane.project.is_some() || folder.is_empty() {
                program.into()
            } else {
                format!("{folder} / {program}").into()
            }
        }
        _ => crate::pane_view::display_label(pane),
    }
}

/// Paint and time exactly one strip, preserving the existing accounting boundary.
pub(crate) fn paint_strip(
    ui: &egui::Ui,
    pane: &crate::Pane,
    metrics: &CellMetrics,
    theme: &Theme,
    strip: Strip,
) -> std::time::Duration {
    let started = std::time::Instant::now();
    if strip.integrated {
        let label = header_label(pane);
        let zoomed = strip.maximized == Some(pane.name);
        paint(
            ui,
            strip.header,
            theme,
            Appearance {
                name: pane.name,
                label: &label,
                agent: pane.signals.view(std::time::Instant::now()),
                title_size: strip.headers.title_size(),
                offer: Offer::of(
                    strip.rect,
                    metrics,
                    strip.headers,
                    strip.button.is_drawn(),
                    zoomed,
                ),
                zoomed,
                folded: pane.folded,
                focused: strip.focused,
                project: pane.project.as_ref(),
            },
        );
    } else {
        let _ = super::paint(
            &ui.painter_at(strip.header),
            strip.header,
            theme,
            metrics,
            strip.button,
        );
        let close =
            super::button_within(strip.header, metrics, strip.button).map(|target| target.rect());
        let enabled = [
            crate::tiling::can_split(
                strip.rect,
                crate::tiling::Orientation::Columns,
                metrics,
                strip.headers,
            ),
            crate::tiling::can_split(
                strip.rect,
                crate::tiling::Orientation::Rows,
                metrics,
                strip.headers,
            ),
            pane.panel_state.is_visible(strip.now),
        ];
        super::controls::Controls::within(strip.header, metrics, close).paint(
            ui,
            theme,
            enabled,
            pane.folded,
            ui.id().with(("header-controls", pane.name)),
        );
    }
    started.elapsed()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Target {
    Label,
    SplitRight,
    SplitDown,
    Review,
    Panel,
    Zoom,
    Close,
    /// Opens or closes the search on this console, whichever has the keyboard (D61).
    Search,
}

/// Which controls a header carries. Painting and pressing build it with this one constructor
/// (D99), each from its own inputs; `a_split_that_cannot_fit_is_not_offered…` presses where a
/// split is and is not painted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Offer {
    can_close: bool,
    split: [bool; 2],
}

impl Offer {
    /// `rect` is the pane's padded view rectangle, the one both passes receive. A maximized console
    /// offers no split: its view fills the area while the split judges the tree, and the titlebar's
    /// split already acts on it because it is the focused one (D66).
    pub(crate) fn of(
        rect: egui::Rect,
        metrics: &CellMetrics,
        headers: super::Headers,
        can_close: bool,
        zoomed: bool,
    ) -> Self {
        let fits =
            |along| !zoomed && crate::tiling::can_split_content(rect, along, metrics, headers);
        Self {
            can_close,
            split: [
                fits(crate::tiling::Orientation::Columns),
                fits(crate::tiling::Orientation::Rows),
            ],
        }
    }
}

const CELL: f32 = 25.0;
/// What the title keeps before any control is laid out beside it.
const TITLE_MINIMUM: f32 = 30.0;

/// Where each target sits, and the x of each rule between groups.
struct Layout {
    targets: Vec<(Target, egui::Rect)>,
    separators: Vec<f32>,
}

fn layout(strip: egui::Rect, offer: Offer) -> Layout {
    let right = (strip.right() - super::CORNER_LEFT_TO_THE_GRAB).max(strip.left());
    let mut base = vec![Target::Search, Target::Review, Target::Panel];
    if offer.can_close {
        base.extend([Target::Zoom, Target::Close]);
    }
    let room = right - strip.left() - TITLE_MINIMUM;
    if room < base.len() as f32 * CELL {
        return Layout {
            targets: vec![(Target::Label, strip)],
            separators: Vec::new(),
        };
    }
    // The splits are the first to go when the header is narrow: they are also in the titlebar.
    let splits: Vec<Target> = [Target::SplitRight, Target::SplitDown]
        .into_iter()
        .zip(offer.split)
        .filter_map(|(target, offered)| offered.then_some(target))
        .take(((room / CELL) as usize).saturating_sub(base.len()))
        .collect();
    let order: Vec<Target> = splits.iter().copied().chain(base).collect();
    let left = right - order.len() as f32 * CELL;
    let cell = |i: usize| {
        let x = left + i as f32 * CELL;
        egui::Rect::from_min_max(
            egui::pos2(x, strip.top()),
            egui::pos2(x + CELL, strip.bottom()),
        )
    };
    let mut separators = Vec::new();
    if !splits.is_empty() {
        separators.push(left + splits.len() as f32 * CELL);
    }
    if offer.can_close {
        separators.push(right - 2.0 * CELL);
    }
    let mut targets = vec![(
        Target::Label,
        egui::Rect::from_min_max(strip.min, egui::pos2(left, strip.bottom())),
    )];
    targets.extend(
        order
            .into_iter()
            .enumerate()
            .map(|(i, target)| (target, cell(i))),
    );
    Layout {
        targets,
        separators,
    }
}

/// Which cell of the header lies under `at`. Whether a control there may be pressed is the
/// caller's question ([`revealed`]): one that was not seen is the title's (D99).
pub(crate) fn hit(strip: egui::Rect, offer: Offer, at: egui::Pos2) -> Option<Target> {
    let inside =
        |rect: egui::Rect| rect.contains(at) && at.x < rect.right() && at.y < rect.bottom();
    layout(strip, offer)
        .targets
        .into_iter()
        .find_map(|(target, rect)| inside(rect).then_some(target))
}

fn painted_id(name: usize) -> egui::Id {
    egui::Id::new(("integrated-header-revealed", name))
}

/// Whether the controls of console `name`'s header were painted in the pass before this one.
///
/// **A frame number and not a flag**: a console that stops being painted — a hidden sibling, a tab
/// in the background, or one the pointer left a moment ago — keeps its last number, which is then
/// too old to count. A press in the same pass that brought the pointer onto the header finds the
/// previous pass's number, not its own, because the pointer is handled before the panes are painted;
/// `a_hidden_close_cannot_be_pressed_until_it_was_painted` fails if that order is ever reversed.
pub(crate) fn revealed(ctx: &egui::Context, name: usize) -> bool {
    let now = ctx.cumulative_frame_nr();
    ctx.data(|data| data.get_temp::<u64>(painted_id(name)))
        .is_some_and(|painted| painted + 1 == now)
}

struct Appearance<'a> {
    /// The console's name, which keys the record of the pass its controls were painted in.
    name: usize,
    agent: Option<crate::agent_signals::View>,
    label: &'a str,
    title_size: f32,
    offer: Offer,
    zoomed: bool,
    folded: bool,
    /// Whether this console is the one the window hands its keys to.
    focused: bool,
    project: Option<&'a crate::project_view::Badge>,
}

/// `ink` brought toward `toward` a tenth at a time until it reads at `contrast` on `surface`, and
/// black or white, whichever reads better, when even `toward` does not. The roles' text is chosen
/// against the plain panel and chrome, and a header is tinted with its project's ink.
fn readable(ink: Color32, surface: Color32, toward: Color32, contrast: f32) -> Color32 {
    use crate::theme::{mix, project_ink_contrast};
    (0..=10)
        .map(|step| mix(ink, toward, step as f32 / 10.0))
        .find(|candidate| project_ink_contrast(*candidate, surface) >= contrast)
        .unwrap_or_else(|| {
            if project_ink_contrast(Color32::WHITE, surface)
                >= project_ink_contrast(Color32::BLACK, surface)
            {
                Color32::WHITE
            } else {
                Color32::BLACK
            }
        })
}

/// What a header paints its surface and its words in.
struct Colors {
    surface: Color32,
    /// The project's name: its ink, readable at 3:1 like any mark.
    name: Option<Color32>,
    /// The task, readable at 4.5:1 like any text.
    text: Color32,
    separator: Color32,
}

fn colors(roles: &crate::theme::Roles, ink: Option<Color32>, focused: bool) -> Colors {
    // The whole header carries a low tint of its project's ink over the stripe: a stripe alone was
    // reported almost invisible (#48), and the design kit tints the header 18 % when focused.
    let ground = if focused { roles.panel } else { roles.chrome };
    let text = if focused { roles.tx } else { roles.tx2 };
    let Some(ink) = ink else {
        // A role surface, where the roles' own text rules already hold (D94, D59).
        return Colors {
            surface: ground,
            name: None,
            text,
            separator: roles.tx3,
        };
    };
    let surface = crate::theme::mix(ground, ink, if focused { 0.18 } else { 0.14 });
    Colors {
        surface,
        name: Some(readable(ink, surface, roles.tx, 3.0)),
        text: readable(text, surface, roles.tx, 4.5),
        separator: readable(roles.tx3, surface, roles.tx, 4.5),
    }
}

fn paint(ui: &egui::Ui, strip: egui::Rect, theme: &Theme, appearance: Appearance<'_>) {
    let Appearance {
        name,
        agent,
        label,
        title_size,
        offer,
        zoomed,
        folded,
        focused,
        project,
    } = appearance;
    let roles = crate::ui::style::roles(ui.ctx(), theme);
    let painter = ui.painter().with_clip_rect(strip.intersect(ui.clip_rect()));
    let colors = colors(&roles, project.map(|project| project.ink), focused);
    let surface = colors.surface;
    painter.rect_filled(
        strip,
        egui::CornerRadius {
            nw: metrics::RADIUS_CARD,
            ne: metrics::RADIUS_CARD,
            sw: 0,
            se: 0,
        },
        surface,
    );
    let ink = colors.name;
    if let Some(project) = project {
        project
            .painted
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let text = colors.text;
    let font = egui::FontId::proportional(title_size);
    let name_font = egui::FontId::new(title_size, crate::fonts::ui_weight(ui.ctx(), 600));
    let layout = layout(strip, offer);
    // Only the focused console shows its controls all the time; another shows them while the
    // pointer is over its header. Hidden ones keep their cells, so the title never moves (D99).
    let shown = focused || ui.rect_contains_pointer(strip);
    if shown {
        let frame = ui.ctx().cumulative_frame_nr();
        let previous = ui.ctx().data_mut(|data| {
            let previous = data.get_temp::<u64>(painted_id(name));
            data.insert_temp(painted_id(name), frame);
            previous
        });
        // egui hit-tests a pass against the widgets of the one before, where these did not exist:
        // one more pass gives the control under a still pointer its hover and its tooltip.
        if previous.is_none_or(|painted| painted + 1 < frame) {
            ui.ctx().request_repaint();
        }
        for x in &layout.separators {
            painter.line_segment(
                [
                    egui::pos2(*x, strip.center().y - 7.0),
                    egui::pos2(*x, strip.center().y + 7.0),
                ],
                // The ink of the title's own « / » separator: the border role the titlebar uses was
                // reported barely visible on a header's tinted surface (18 November 2026).
                egui::Stroke::new(1.0, colors.separator),
            );
        }
    }
    for (target, rect) in layout.targets {
        if target != Target::Label && !shown {
            continue;
        }
        let tooltip = match target {
            Target::Label => "Double-click to rename",
            Target::SplitRight => "Split right",
            Target::SplitDown => "Split down",
            Target::Review => "Review changes",
            Target::Panel if folded => "Show information panel",
            Target::Panel => "Hide information panel",
            Target::Zoom if zoomed => "Restore console",
            Target::Zoom => "Maximize console",
            Target::Close => "Close console",
            Target::Search => "Search this console",
        };
        let response = ui.interact(
            rect,
            ui.id().with((
                "integrated-header",
                strip.min.x.to_bits(),
                strip.min.y.to_bits(),
                target as u8,
            )),
            egui::Sense::hover(),
        );
        let hovered = response.hovered();
        if target == Target::Label
            && let Some(project) = project
        {
            response.on_hover_text(format!(
                "{}\n{}\nDouble-click to rename console",
                project.details, label
            ));
        } else {
            response.on_hover_text(tooltip);
        }
        if target != Target::Label {
            if hovered {
                painter.rect_filled(
                    egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(22.0))
                        .intersect(rect),
                    metrics::RADIUS_CHIP,
                    roles.hover,
                );
            }
            // Closing is the one control that destroys something, so it stays quiet until reached.
            let icon = match target {
                Target::Close if !hovered => roles.tx3,
                _ if hovered => roles.tx,
                _ => roles.tx2,
            };
            let size = if target == Target::Review { 13.0 } else { 12.0 };
            paint_icon(&painter, rect.center(), target, zoomed, icon, size);
            continue;
        }
        let clipped = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
        let mut left = egui::pos2(rect.left() + 8.0, rect.center().y);
        if let Some(agent) = agent {
            let logo =
                egui::Rect::from_center_size(left + egui::vec2(8.0, 0.0), egui::Vec2::splat(16.0))
                    .intersect(rect);
            if logo.is_positive() {
                crate::agent_signals::logos::paint(ui, logo, agent);
            }
            left.x += 23.0;
            if agent.waiting {
                let marker = clipped.text(
                    left,
                    egui::Align2::LEFT_CENTER,
                    "?",
                    font.clone(),
                    roles
                        .wait
                        .gamma_multiply(if agent.backed { 1.0 } else { 0.45 }),
                );
                left.x = marker.right() + 6.0;
            }
        }
        if let (Some(project), Some(ink)) = (project, ink) {
            let available = (rect.right() - left.x - 8.0).max(0.0);
            let galley = painter.layout_no_wrap(project.name.clone(), name_font.clone(), ink);
            let width = galley.size().x.min(available * 0.45).min(140.0);
            if width > 0.0 {
                let name = egui::Rect::from_min_size(
                    egui::pos2(left.x, rect.center().y - galley.size().y / 2.0),
                    egui::vec2(width, galley.size().y),
                );
                painter
                    .with_clip_rect(name.intersect(clipped.clip_rect()))
                    .galley(name.min, galley, ink);
                let separator = clipped.text(
                    egui::pos2(name.right(), rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    " / ",
                    font.clone(),
                    colors.separator,
                );
                left.x = separator.right();
            }
        }
        crate::header::title_text::paint(
            &clipped,
            left,
            egui::Align2::LEFT_CENTER,
            label,
            font.clone(),
            text,
        );
    }
}

/// The header's icons from the interface's set (D103) in a `size`-point square: the splits, the
/// branch for the review, the panel, diagonal arrows to maximize and back, and the cross.
pub(crate) fn paint_icon(
    painter: &egui::Painter,
    center: egui::Pos2,
    target: Target,
    zoomed: bool,
    color: egui::Color32,
    size: f32,
) {
    use crate::ui::icons::{Icon, paint};
    let icon = match target {
        Target::SplitRight => Icon::SplitRight,
        Target::SplitDown => Icon::SplitDown,
        Target::Review => Icon::Branch,
        Target::Panel => Icon::Panel,
        Target::Zoom if zoomed => Icon::Collapse,
        Target::Zoom => Icon::Expand,
        Target::Close => Icon::Close,
        Target::Search => Icon::Search,
        Target::Label => unreachable!(),
    };
    paint(painter, center, icon, size, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header offering no split: the set every test before D99 was written against.
    fn no_splits(can_close: bool) -> Offer {
        Offer {
            can_close,
            split: [false; 2],
        }
    }

    fn targets(strip: egui::Rect, _m: &CellMetrics, can_close: bool) -> Vec<(Target, egui::Rect)> {
        layout(strip, no_splits(can_close)).targets
    }

    #[test]
    fn a_console_without_an_agent_is_named_by_its_folder_and_program() {
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let pane = &mut *window.active_mut().panes_mut()[0];
        pane.program = Some(("tax-platform".into(), "bash".into()));
        assert_eq!(header_label(pane), "tax-platform / Bash");
        pane.project = Some(crate::project_view::Badge {
            key: crate::project::Key::Repository("/work/tax".into()),
            name: "TaxPlatform".into(),
            details: "/work/tax".into(),
            ink: Color32::WHITE,
            painted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        assert_eq!(
            header_label(pane),
            "Bash",
            "the project already names the place"
        );
        pane.label = Some("deploy".into());
        assert_eq!(header_label(pane), "deploy", "a name somebody gave wins");
        pane.label = None;
        pane.program = None;
        assert_eq!(header_label(pane), crate::pane_view::display_label(pane));
    }

    #[test]
    fn pane_icons_are_the_set_s_glyphs_and_have_no_overflow_control() {
        let ctx = crate::ui::icons::installed_for_tests(1.0);
        let painted = [
            (Target::Close, false, egui_phosphor::regular::X),
            (
                Target::Zoom,
                false,
                egui_phosphor::regular::ARROWS_OUT_SIMPLE,
            ),
            (Target::Zoom, true, egui_phosphor::regular::ARROWS_IN_SIMPLE),
            (Target::Review, false, egui_phosphor::regular::GIT_BRANCH),
            (Target::Panel, false, egui_phosphor::regular::SIDEBAR_SIMPLE),
        ];
        for (target, zoomed, glyph) in painted {
            let out = ctx.run_ui(Default::default(), |ui| {
                paint_icon(
                    ui.painter(),
                    egui::pos2(30.0, 30.0),
                    target,
                    zoomed,
                    egui::Color32::WHITE,
                    13.0,
                );
            });
            let texts: Vec<_> = out
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                    _ => None,
                })
                .collect();
            assert_eq!(texts, vec![glyph.to_owned()], "{target:?} zoomed {zoomed}");
            assert_eq!(out.shapes.len(), 1, "{target:?}: nothing drawn by hand");
            out.drop_without_applying_deltas();
        }
        let targets = targets(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 33.0)),
            &CellMetrics::of(8.0, 16.0),
            true,
        );
        assert_eq!(
            targets.iter().map(|(t, _)| *t as u8).collect::<Vec<_>>(),
            vec![
                Target::Label as u8,
                Target::Review as u8,
                Target::Panel as u8,
                Target::Zoom as u8,
                Target::Close as u8
            ]
        );
    }

    #[test]
    fn integrated_split_threshold_keeps_twenty_columns_and_three_rows() {
        let m = CellMetrics::of(8.0, 16.0);
        let headers = super::super::Headers::PAINTED.integrated(true);
        for (axis, minimum) in [
            (crate::tiling::Orientation::Columns, 20.0 * 8.0 + 16.0 + 5.0),
            (
                crate::tiling::Orientation::Rows,
                3.0 * 16.0 + 16.0 + 28.0 + 5.0,
            ),
        ] {
            let threshold = 2.0 * minimum + crate::DIVIDER_THICKNESS;
            let size = |length| match axis {
                crate::tiling::Orientation::Columns => egui::vec2(length, 800.0),
                crate::tiling::Orientation::Rows => egui::vec2(800.0, length),
            };
            assert!(!crate::tiling::can_split(
                egui::Rect::from_min_size(egui::Pos2::ZERO, size(threshold - 1.0)),
                axis,
                &m,
                headers
            ));
            assert!(crate::tiling::can_split(
                egui::Rect::from_min_size(egui::Pos2::ZERO, size(threshold)),
                axis,
                &m,
                headers
            ));
            for length in [threshold - 1.0, threshold] {
                let outer = egui::Rect::from_min_size(egui::Pos2::ZERO, size(length));
                let content = crate::pane_view::padded_rect(outer, true);
                assert_eq!(
                    crate::tiling::can_split_content(content, axis, &m, headers),
                    crate::tiling::can_split(outer, axis, &m, headers),
                    "header menu and shortcut disagree at {length}"
                );
            }
        }
    }

    #[test]
    fn enlarged_project_prefix_uses_the_title_size_and_full_line_height() {
        let ctx = egui::Context::default();
        let badge = crate::project_view::Badge {
            key: crate::project::Key::Repository("/work/backend".into()),
            name: "Backend".into(),
            details: "/work/backend".into(),
            ink: egui::Color32::GREEN,
            painted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let height =
                ui.fonts_mut(|fonts| fonts.row_height(&egui::FontId::proportional(32.0))) + 12.0;
            let strip = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, height));
            paint(
                ui,
                strip,
                &Theme::default(),
                Appearance {
                    agent: None,
                    label: "Review changes",
                    title_size: 32.0,
                    name: 0,
                    offer: no_splits(true),
                    zoomed: false,
                    folded: false,
                    focused: true,
                    project: Some(&badge),
                },
            );
        });
        output.textures_delta.clear();
        for label in ["Backend", " / ", "Review changes"] {
            let (clipped, text) = output
                .shapes
                .iter()
                .find_map(|clipped| match &clipped.shape {
                    egui::Shape::Text(text) if text.galley.text() == label => Some((clipped, text)),
                    _ => None,
                })
                .expect("both parts of the project header must be painted");
            assert!(
                text.galley
                    .job
                    .sections
                    .iter()
                    .all(|section| section.format.font_id.size == 32.0)
            );
            assert!(
                clipped.clip_rect.height() >= text.galley.size().y,
                "project text was vertically clipped"
            );
        }
    }

    fn rgb(hex: u32) -> egui::Color32 {
        egui::Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }

    fn badge(ink: egui::Color32) -> crate::project_view::Badge {
        crate::project_view::Badge {
            key: crate::project::Key::Repository("/work/backend".into()),
            name: "Backend".into(),
            details: "/work/backend".into(),
            ink,
            painted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn appearance<'a>(
        project: Option<&'a crate::project_view::Badge>,
        focused: bool,
        agent: Option<crate::agent_signals::View>,
    ) -> Appearance<'a> {
        Appearance {
            agent,
            label: "Fix login",
            title_size: 12.0,
            name: 0,
            offer: no_splits(true),
            zoomed: false,
            folded: false,
            focused,
            project,
        }
    }

    fn text_color(shapes: &[egui::epaint::ClippedShape], text: &str) -> Option<egui::Color32> {
        shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                Some(t.galley.job.sections[0].format.color)
            }
            _ => None,
        })
    }

    /// **Literal Ivy Night colours, written out**: the focused header sits on the panel and the
    /// others on the chrome, each tinted with the project's ink; the project is a 3-point stripe
    /// and its name in its own ink, beside
    /// the task in the text role of the surface — and no gradient and no «01» any more.
    #[test]
    fn project_stripe_and_ink_replace_the_gradient_and_focus_picks_the_surface() {
        let ctx = egui::Context::default();
        let ink = rgb(0x43c071);
        let badge = badge(ink);
        let strip = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(500.0, 28.0));
        let metrics = CellMetrics::of(8.0, 16.0);
        let label_rect = targets(strip, &metrics, true)[0].1;
        // The panel #171D21 with 18 % of the ink when focused; the chrome #0F1315 with 14 % when not.
        // The separator, muted text #8E9390, is brought back until it reads at 4.5:1 on the focused
        // tint; on the darker unfocused one it already does.
        for (focused, surface, text, separator) in [
            (true, rgb(0x1f3a2f), rgb(0xe4e8e2), rgb(0x9fa4a0)),
            (false, rgb(0x162b22), rgb(0xa5aaa6), rgb(0x8e9390)),
        ] {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint(
                    ui,
                    strip,
                    &Theme::default(),
                    appearance(Some(&badge), focused, None),
                );
            });
            out.textures_delta.clear();
            assert!(
                out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                    if r.rect == strip && r.fill == surface)),
                "focused={focused}: the header is not on its surface"
            );
            // The owner removed the stripe on 17 November 2026: the tint and the ink name stay.
            assert!(
                !out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                    if r.rect.width() == 3.0 && r.fill == ink)),
                "the project stripe is back"
            );
            assert!(
                !out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Mesh(_))),
                "the old gradient is still painted"
            );
            assert_eq!(text_color(&out.shapes, "Backend"), Some(ink));
            assert_eq!(text_color(&out.shapes, " / "), Some(separator));
            assert_eq!(text_color(&out.shapes, "Fix login"), Some(text));
            assert_eq!(text_color(&out.shapes, "01"), None);
            for s in &out.shapes {
                if matches!(s.shape, egui::Shape::Text(_)) {
                    assert!(s.clip_rect.right() <= label_rect.right());
                }
            }
            out.drop_without_applying_deltas();
        }
        assert!(badge.painted.load(std::sync::atomic::Ordering::Relaxed));
        // Without a project there is no stripe and the task starts after the same inset.
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint(ui, strip, &Theme::default(), appearance(None, true, None));
        });
        out.textures_delta.clear();
        assert!(
            !out.shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
            if r.rect.width() == 3.0))
        );
        assert_eq!(text_color(&out.shapes, "Fix login"), Some(rgb(0xe4e8e2)));
        out.drop_without_applying_deltas();
    }

    /// A project ink that would not read on the chrome is brought toward the text until it does.
    #[test]
    fn an_unreadable_project_ink_is_brought_toward_the_text() {
        let chrome = rgb(0x0f1315);
        let tx = rgb(0xe4e8e2);
        assert_eq!(readable(rgb(0x43c071), chrome, tx, 3.0), rgb(0x43c071));
        let dark = readable(rgb(0x202020), chrome, tx, 3.0);
        assert_ne!(dark, rgb(0x202020));
        assert!(crate::theme::project_ink_contrast(dark, chrome) >= 3.0);
    }

    /// Every factory theme, with each of its project colours and with inks too dark and too light
    /// for it, focused and not: the task and the separator read at 4.5:1 on the tinted header and
    /// the project's name at 3:1.
    #[test]
    fn project_headers_keep_their_text_readable_across_the_catalog() {
        use crate::theme::{NamedTheme, Roles, project_accents, project_ink_contrast};
        for &named in NamedTheme::ALL {
            let theme = named.theme();
            let roles = Roles::of(&theme);
            let mut inks: Vec<Option<egui::Color32>> =
                project_accents(&theme).into_iter().map(Some).collect();
            inks.extend([Some(rgb(0x202020)), Some(rgb(0xf0f0f0))]);
            for ink in inks {
                for focused in [true, false] {
                    let c = colors(&roles, ink, focused);
                    let label = format!("{named:?} {ink:?} focused={focused}");
                    assert!(
                        project_ink_contrast(c.text, c.surface) >= 4.5,
                        "{label}: task"
                    );
                    assert!(
                        project_ink_contrast(c.separator, c.surface) >= 4.5,
                        "{label}: separator"
                    );
                    if let Some(name) = c.name {
                        assert!(
                            project_ink_contrast(name, c.surface) >= 3.0,
                            "{label}: name"
                        );
                    }
                }
            }
        }
    }

    /// An unfocused header with an ink too dark for its tinted surface paints the name in a
    /// colour brought toward the text, which reads at 3:1 there.
    #[test]
    fn a_dark_project_ink_is_lifted_on_the_painted_header() {
        let ctx = egui::Context::default();
        let ink = rgb(0x202020);
        let badge = badge(ink);
        let strip = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(500.0, 28.0));
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint(
                ui,
                strip,
                &Theme::default(),
                appearance(Some(&badge), false, None),
            );
        });
        out.textures_delta.clear();
        let surface = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.rect == strip => Some(r.fill),
                _ => None,
            })
            .expect("the header surface is painted");
        let name = text_color(&out.shapes, "Backend").expect("the project name is painted");
        assert_ne!(name, ink);
        assert!(crate::theme::project_ink_contrast(name, surface) >= 3.0);
        out.drop_without_applying_deltas();
    }

    /// Controls hover on the hover surface in the text colour; close stays muted until reached;
    /// the waiting marker is the wait role, dimmed when nobody holds the claim up.
    #[test]
    fn header_controls_hover_on_the_kit_surface_and_close_stays_quiet_until_reached() {
        let strip = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(500.0, 28.0));
        let metrics = CellMetrics::of(8.0, 16.0);
        let review = targets(strip, &metrics, true)[1].1;
        let close = targets(strip, &metrics, true)[4].1;
        for backed in [true, false] {
            let ctx = crate::ui::icons::installed_for_tests(1.0);
            let run = |events| {
                ctx.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        paint(
                            ui,
                            strip,
                            &Theme::default(),
                            appearance(
                                Some(&badge(rgb(0x43c071))),
                                true,
                                Some(crate::agent_signals::View {
                                    client: crate::agent_signals::Client::Codex,
                                    busy: false,
                                    done: false,
                                    waiting: true,
                                    backed,
                                }),
                            ),
                        )
                    },
                )
            };
            run(vec![egui::Event::PointerMoved(close.center())]).drop_without_applying_deltas();
            let mut reached = run(vec![]);
            reached.textures_delta.clear();
            let reached_close: Vec<_> = reached
                .shapes
                .iter()
                .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
                .filter(|(_, _, square)| close.contains(square.center()))
                .map(|(_, color, _)| color)
                .collect();
            assert!(
                !reached_close.is_empty() && reached_close.iter().all(|c| *c == rgb(0xe4e8e2)),
                "a reached close is not in the text colour: {reached_close:?}"
            );
            reached.drop_without_applying_deltas();
            run(vec![egui::Event::PointerMoved(review.center())]).drop_without_applying_deltas();
            let mut out = run(vec![]);
            out.textures_delta.clear();
            let hover = egui::Rect::from_center_size(review.center(), egui::Vec2::splat(22.0))
                .intersect(review);
            assert!(
                out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                if r.rect == hover && r.fill == rgb(0x262b2d)))
            );
            let strokes_in = |rect: egui::Rect| -> Vec<egui::Color32> {
                out.shapes
                    .iter()
                    .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
                    .filter(|(_, _, square)| rect.contains(square.center()))
                    .map(|(_, color, _)| color)
                    .collect()
            };
            let hovered = strokes_in(review);
            assert!(!hovered.is_empty() && hovered.iter().all(|c| *c == rgb(0xe4e8e2)));
            let quiet = strokes_in(close);
            assert!(!quiet.is_empty() && quiet.iter().all(|c| *c == rgb(0x8e9390)));
            assert_eq!(
                text_color(&out.shapes, "?"),
                Some(rgb(0xf2b84b).gamma_multiply(if backed { 1.0 } else { 0.45 }))
            );
            out.drop_without_applying_deltas();
        }
    }

    #[test]
    fn agent_logo_keeps_its_signal_opacity_in_the_header() {
        let theme = Theme::default();
        let badge = badge(rgb(0xd97757));
        for (backed, busy, opacity) in [(true, false, 1.0), (true, true, 0.8), (false, true, 0.45)]
        {
            let ctx = egui::Context::default();
            crate::agent_signals::logos::install(&ctx);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    time: Some(0.0),
                    focused: true,
                    ..Default::default()
                },
                |ui| {
                    paint(
                        ui,
                        egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(500.0, 28.0)),
                        &theme,
                        appearance(
                            Some(&badge),
                            true,
                            Some(crate::agent_signals::View {
                                client: crate::agent_signals::Client::Claude,
                                busy,
                                done: false,
                                waiting: false,
                                backed,
                            }),
                        ),
                    );
                },
            );
            let installed: Vec<_> = output.textures_delta.set.keys().copied().collect();
            output.textures_delta.clear();
            let images: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                        Some(mesh)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(images.len(), 1, "the installed Claude logo must be painted");
            assert!(installed.contains(&images[0].texture_id));
            assert_eq!(images[0].calc_bounds().size(), egui::Vec2::splat(16.0));
            assert!(
                images[0]
                    .vertices
                    .iter()
                    .all(|vertex| vertex.color == egui::Color32::WHITE.gamma_multiply(opacity)),
                "fresh, busy and stale logo opacity must survive"
            );
            output.drop_without_applying_deltas();
        }
    }

    #[test]
    fn singleton_header_omits_zoom_and_reclaims_its_target_width() {
        let m = CellMetrics::of(8.0, 16.0);
        for width in [35.0, 100.0, 150.0, 500.0] {
            let strip = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 33.0));
            let items = targets(strip, &m, false);
            assert!(
                items
                    .iter()
                    .all(|(target, _)| !matches!(target, Target::Zoom | Target::Close))
            );
            for (_, rect) in targets(strip, &m, true)
                .into_iter()
                .filter(|(t, _)| *t == Target::Zoom)
            {
                assert!(!matches!(
                    hit(strip, no_splits(false), rect.center()),
                    Some(Target::Zoom)
                ));
            }
            if width >= 100.0 {
                assert_eq!(items.len(), 3);
            }
            if width == 500.0 {
                assert_eq!(
                    items[0].1.width() - targets(strip, &m, true)[0].1.width(),
                    50.0
                );
            }
        }
    }

    fn kinds(strip: egui::Rect, offer: Offer) -> Vec<Target> {
        layout(strip, offer)
            .targets
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    fn strip_of(width: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(width, 28.0))
    }

    /// D99's three groups, in order, with a rule on each boundary between them.
    #[test]
    fn a_header_lays_out_its_three_groups_with_a_rule_between_each() {
        let strip = strip_of(500.0);
        let both = |can_close| Offer {
            can_close,
            split: [true, true],
        };
        let full = layout(strip, both(true));
        assert_eq!(
            full.targets.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
            vec![
                Target::Label,
                Target::SplitRight,
                Target::SplitDown,
                Target::Review,
                Target::Panel,
                Target::Zoom,
                Target::Close
            ]
        );
        let at = |t| full.targets.iter().find(|(x, _)| *x == t).unwrap().1;
        assert_eq!(
            full.separators,
            vec![at(Target::SplitDown).right(), at(Target::Panel).right()]
        );
        assert_eq!(at(Target::SplitDown).right(), at(Target::Review).left());
        assert_eq!(at(Target::Panel).right(), at(Target::Zoom).left());
        let lone = layout(strip, both(false));
        assert_eq!(
            lone.targets.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
            vec![
                Target::Label,
                Target::SplitRight,
                Target::SplitDown,
                Target::Review,
                Target::Panel
            ]
        );
        assert_eq!(lone.separators.len(), 1);
    }

    /// The title keeps 30 points: the base comes first, a split only after it, and below the base
    /// the header is the title alone. Strip widths with the 6-point corner, at ±1 point.
    #[test]
    fn a_narrow_header_drops_its_splits_before_the_rest() {
        for (can_close, label_only, base, with_split) in
            [(true, 135.0, 136.0, 161.0), (false, 85.0, 86.0, 111.0)]
        {
            let offer = Offer {
                can_close,
                split: [false, true],
            };
            assert_eq!(kinds(strip_of(label_only), offer), vec![Target::Label]);
            let base_set = kinds(strip_of(base), offer);
            assert!(base_set.contains(&Target::Review) && base_set.contains(&Target::Panel));
            assert!(!base_set.contains(&Target::SplitDown));
            assert!(!kinds(strip_of(with_split - 1.0), offer).contains(&Target::SplitDown));
            assert!(kinds(strip_of(with_split), offer).contains(&Target::SplitDown));
        }
    }

    /// The offer is the split's own predicate on the padded rectangle, at its threshold, and a
    /// maximized console offers none.
    #[test]
    fn the_offer_follows_the_split_threshold_and_a_maximized_console_offers_none() {
        let m = CellMetrics::of(8.0, 16.0);
        let headers = super::super::Headers::PAINTED.integrated(true);
        let minimum = 3.0 * 16.0 + 16.0 + 28.0 + 5.0;
        let threshold = 2.0 * minimum + crate::DIVIDER_THICKNESS;
        let content = |height| {
            crate::pane_view::padded_rect(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, height)),
                true,
            )
        };
        let strip = strip_of(300.0);
        let below = Offer::of(content(threshold - 1.0), &m, headers, true, false);
        let at = Offer::of(content(threshold), &m, headers, true, false);
        assert!(!kinds(strip, below).contains(&Target::SplitDown));
        assert!(kinds(strip, at).contains(&Target::SplitDown));
        let zoomed = Offer::of(content(threshold + 400.0), &m, headers, true, true);
        assert!(
            !kinds(strip_of(900.0), zoomed)
                .iter()
                .any(|t| matches!(t, Target::SplitRight | Target::SplitDown))
        );
    }

    /// The pass in which a header's controls first appear asks for exactly one more, so the control
    /// under a still pointer gets its hover; a header that stays shown asks for none.
    #[test]
    fn controls_appearing_ask_for_one_more_pass_and_staying_asks_for_none() {
        let ctx = egui::Context::default();
        let strip = strip_of(500.0);
        let pass = |focused: bool| {
            let out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let mut look = appearance(None, focused, None);
                look.name = 7;
                paint(ui, strip, &Theme::default(), look);
            });
            let asked = out
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .is_some_and(|v| v.repaint_delay == std::time::Duration::ZERO);
            out.drop_without_applying_deltas();
            asked
        };
        pass(false);
        pass(false);
        assert!(!pass(false), "a hidden header asked for a pass");
        assert!(
            pass(true),
            "controls that just appeared did not ask for one more pass"
        );
        // egui answers one request with the pass it asked for, which still reports it; after that
        // a header that stays shown must leave the window at rest.
        pass(true);
        assert!(
            !pass(true),
            "a header that stays shown keeps waking the window"
        );
        assert!(
            !pass(true),
            "a header that stays shown keeps waking the window"
        );
    }

    /// Every control cell answers its own target, and nothing outside the cells does.
    #[test]
    fn each_control_cell_answers_its_own_target() {
        let strip = strip_of(500.0);
        let offer = Offer {
            can_close: true,
            split: [true, true],
        };
        for (target, rect) in layout(strip, offer).targets {
            assert_eq!(hit(strip, offer, rect.center()), Some(target));
        }
        assert_eq!(
            hit(
                strip,
                offer,
                egui::pos2(strip.left() - 1.0, strip.center().y)
            ),
            None
        );
    }

    #[test]
    fn narrow_header_keeps_a_reachable_label_without_overflow_buttons() {
        let m = CellMetrics::of(8.0, 16.0);
        let strip = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(35.0, 22.0));
        let items = targets(strip, &m, true);
        assert_eq!(items.len(), 1);
        assert!(matches!(
            hit(strip, no_splits(true), items[0].1.center()),
            Some(Target::Label)
        ));
    }
}
