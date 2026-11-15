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
        let label = crate::pane_view::display_label(pane);
        paint(
            ui,
            strip.header,
            metrics,
            theme,
            Appearance {
                label: &label,
                agent: pane.signals.view(std::time::Instant::now()),
                title_size: strip.headers.title_size(),
                can_close: strip.button.is_drawn(),
                zoomed: strip.maximized == Some(pane.name),
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Label,
    Review,
    Panel,
    Zoom,
    Close,
}

fn targets(strip: egui::Rect, _m: &CellMetrics, can_close: bool) -> Vec<(Target, egui::Rect)> {
    let width = 25.0;
    let right = (strip.right() - super::CORNER_LEFT_TO_THE_GRAB).max(strip.left());
    let count = if can_close { 4 } else { 2 };
    let left = right - count as f32 * width;
    if left - strip.left() < 30.0 {
        return vec![(Target::Label, strip)];
    }
    let mut out = vec![(
        Target::Label,
        egui::Rect::from_min_max(strip.min, egui::pos2(left, strip.bottom())),
    )];
    for (i, target) in [Target::Review, Target::Panel, Target::Zoom, Target::Close]
        .into_iter()
        .filter(|target| !matches!(target, Target::Zoom | Target::Close) || can_close)
        .enumerate()
    {
        let x = left + i as f32 * width;
        out.push((
            target,
            egui::Rect::from_min_max(
                egui::pos2(x, strip.top()),
                egui::pos2(x + width, strip.bottom()),
            ),
        ));
    }
    out
}

pub(crate) fn hit(
    strip: egui::Rect,
    m: &CellMetrics,
    can_close: bool,
    at: egui::Pos2,
) -> Option<Target> {
    targets(strip, m, can_close)
        .into_iter()
        .find_map(|(target, rect)| {
            (rect.contains(at) && at.x < rect.right() && at.y < rect.bottom()).then_some(target)
        })
}

struct Appearance<'a> {
    agent: Option<crate::agent_signals::View>,
    label: &'a str,
    title_size: f32,
    can_close: bool,
    zoomed: bool,
    folded: bool,
    /// Whether this console is the one the window hands its keys to.
    focused: bool,
    project: Option<&'a crate::project_view::Badge>,
}

/// The stripe a project paints down the header's left edge.
const STRIPE: f32 = 3.0;

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

fn paint(
    ui: &egui::Ui,
    strip: egui::Rect,
    m: &CellMetrics,
    theme: &Theme,
    appearance: Appearance<'_>,
) {
    let Appearance {
        agent,
        label,
        title_size,
        can_close,
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
        painter.rect_filled(
            egui::Rect::from_min_max(strip.min, egui::pos2(strip.left() + STRIPE, strip.bottom())),
            0.0,
            project.ink,
        );
        project
            .painted
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let text = colors.text;
    let font = egui::FontId::proportional(title_size);
    let name_font = egui::FontId::new(title_size, crate::fonts::ui_weight(ui.ctx(), 600));
    for (target, rect) in targets(strip, m, can_close) {
        let tooltip = match target {
            Target::Label => "Double-click to rename",
            Target::Review => "Review changes",
            Target::Panel if folded => "Show information panel",
            Target::Panel => "Hide information panel",
            Target::Zoom if zoomed => "Restore console",
            Target::Zoom => "Maximize console",
            Target::Close => "Close console",
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
            paint_icon(&painter, rect.center(), target, zoomed, icon, 13.0);
            continue;
        }
        let clipped = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
        let mut left = egui::pos2(rect.left() + STRIPE + 8.0, rect.center().y);
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

/// The prototype's 24-unit SVG paths, scaled to the caller's icon box.
pub(crate) fn paint_icon(
    painter: &egui::Painter,
    center: egui::Pos2,
    target: Target,
    zoomed: bool,
    color: egui::Color32,
    size: f32,
) {
    let scale = size / 24.0;
    let at = |x, y| center + egui::vec2(x, y) * scale;
    let stroke = egui::Stroke::new(1.6 * scale, color);
    let line = |a, b| {
        painter.line_segment([a, b], stroke);
    };
    let outline = |a, b| {
        painter.rect_stroke(
            egui::Rect::from_min_max(a, b),
            scale,
            stroke,
            egui::StrokeKind::Middle,
        );
    };
    match target {
        Target::Review => {
            line(at(-5.0, -5.0), at(-5.0, 5.0));
            line(at(5.0, -5.0), at(5.0, -1.0));
            line(at(5.0, -1.0), at(-5.0, 3.0));
            for p in [at(-5.0, -6.0), at(-5.0, 6.0), at(5.0, -6.0)] {
                painter.circle_filled(p, 2.0 * scale, color);
            }
        }
        Target::Panel => {
            outline(at(-9.0, -8.0), at(9.0, 8.0));
            line(at(3.0, -8.0), at(3.0, 8.0));
        }
        Target::Zoom if zoomed => {
            for (a, b) in [
                ((-3.0, -7.0), (-3.0, -9.0)),
                ((-3.0, -9.0), (9.0, -9.0)),
                ((9.0, -9.0), (9.0, 3.0)),
                ((9.0, 3.0), (7.0, 3.0)),
            ] {
                line(at(a.0, a.1), at(b.0, b.1));
            }
            outline(at(-9.0, -3.0), at(3.0, 9.0));
        }
        Target::Zoom => {
            outline(at(-7.0, -7.0), at(7.0, 7.0));
        }
        Target::Close => {
            line(at(-6.0, -6.0), at(6.0, 6.0));
            line(at(6.0, -6.0), at(-6.0, 6.0));
        }
        Target::Label => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_icons_match_the_prototype_paths_and_have_no_overflow_control() {
        let ctx = egui::Context::default();
        let out = ctx.run_ui(Default::default(), |ui| {
            paint_icon(
                ui.painter(),
                egui::pos2(30.0, 30.0),
                Target::Close,
                false,
                egui::Color32::WHITE,
                13.0,
            );
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
        });
        let lines: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::LineSegment { points, stroke } => Some((points, stroke)),
                _ => None,
            })
            .collect();
        assert_eq!(lines.len(), 2);
        for (points, stroke) in lines {
            assert!(((points[1].x - points[0].x).abs() - 6.5).abs() < 0.001);
            assert!((stroke.width - 13.0 * 1.6 / 24.0).abs() < 0.001);
        }
        out.drop_without_applying_deltas();
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
                &CellMetrics::of(8.0, 16.0),
                &Theme::default(),
                Appearance {
                    agent: None,
                    label: "Review changes",
                    title_size: 32.0,
                    can_close: true,
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
            can_close: true,
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
                    &metrics,
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
            let stripe = egui::Rect::from_min_size(strip.min, egui::vec2(3.0, 28.0));
            assert!(
                out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                    if r.rect == stripe && r.fill == ink)),
                "the project stripe is missing"
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
            paint(
                ui,
                strip,
                &metrics,
                &Theme::default(),
                appearance(None, true, None),
            );
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
                &CellMetrics::of(8.0, 16.0),
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
            let ctx = egui::Context::default();
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
                            &metrics,
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
                .filter_map(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke } if close.contains(points[0]) => {
                        Some(stroke.color)
                    }
                    _ => None,
                })
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
                    .filter_map(|s| match &s.shape {
                        egui::Shape::LineSegment { points, stroke } if rect.contains(points[0]) => {
                            Some(stroke.color)
                        }
                        _ => None,
                    })
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
                        &CellMetrics::of(8.0, 16.0),
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
                    hit(strip, &m, false, rect.center()),
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

    #[test]
    fn narrow_header_keeps_a_reachable_label_without_overflow_buttons() {
        let m = CellMetrics::of(8.0, 16.0);
        let strip = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(35.0, 22.0));
        let items = targets(strip, &m, true);
        assert_eq!(items.len(), 1);
        assert!(matches!(
            hit(strip, &m, true, items[0].1.center()),
            Some(Target::Label)
        ));
    }
}
