//! Shared paint and hit geometry for the integrated pane header.
use crate::{painter::CellMetrics, theme::Theme};
use eframe::egui;

/// Inputs shared by the historical and integrated strip painting regimes.
pub(crate) struct Strip {
    pub(crate) rect: egui::Rect,
    pub(crate) header: egui::Rect,
    pub(crate) headers: super::Headers,
    pub(crate) button: super::Button,
    pub(crate) integrated: bool,
    pub(crate) maximized: Option<usize>,
    pub(crate) now: std::time::SystemTime,
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
                number: pane.name + 1,
                title_size: strip.headers.title_size(),
                can_close: strip.button.is_drawn(),
                zoomed: strip.maximized == Some(pane.name),
                folded: pane.folded,
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
    let count = if can_close { 4 } else { 3 };
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
        .filter(|target| *target != Target::Close || can_close)
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
    number: usize,
    title_size: f32,
    can_close: bool,
    zoomed: bool,
    folded: bool,
    project: Option<&'a crate::project_view::Badge>,
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
        number,
        title_size,
        can_close,
        zoomed,
        folded,
        project,
    } = appearance;
    let painter = ui.painter().with_clip_rect(strip.intersect(ui.clip_rect()));
    let colors = project.map(|project| crate::theme::project_header_colors(theme, project.ink));
    let foreground = colors.as_ref().map_or(theme.foreground(), |c| c.foreground);
    let secondary = if project.is_some() {
        foreground
    } else {
        foreground.gamma_multiply(0.65)
    };
    if let Some(colors) = &colors {
        paint_gradient(&painter, strip, strip, colors.left, colors.right);
    } else {
        painter.rect_filled(strip, 0.0, theme.panel_background());
    }
    if let Some(project) = project {
        project
            .painted
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
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
        if response.hovered() {
            if let Some(colors) = &colors {
                paint_gradient(
                    &painter.with_clip_rect(rect),
                    strip,
                    rect,
                    colors.hover_left,
                    colors.hover_right,
                );
            } else {
                painter.rect_filled(rect, 2.0, theme.focus().gamma_multiply(0.2));
            }
        }
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
        if target == Target::Label {
            let left = rect.left_center() + egui::vec2(11.0, 0.0);
            let number_text = format!("{number:02}");
            let number_galley =
                painter.layout_no_wrap(number_text, egui::FontId::proportional(10.0), secondary);
            let offset = number_galley.size().x + 7.0;
            painter.with_clip_rect(rect).galley(
                left - egui::vec2(0.0, number_galley.size().y / 2.0),
                number_galley,
                foreground,
            );
            let mut title_left = left + egui::vec2(offset, 0.0);
            if let Some(agent) = agent {
                let logo = egui::Rect::from_center_size(
                    title_left + egui::vec2(8.0, 0.0),
                    egui::Vec2::splat(16.0),
                )
                .intersect(rect);
                if logo.is_positive() {
                    if project.is_some() {
                        // A transparent brand mark can match a configured project accent exactly.
                        painter.with_clip_rect(rect).rect_filled(
                            logo.expand(2.0),
                            2.0,
                            theme.panel_background(),
                        );
                    }
                    crate::agent_signals::logos::paint(ui, logo, agent);
                }
                title_left.x += 23.0;
                if agent.waiting {
                    let marker = painter.layout_no_wrap(
                        "?".into(),
                        egui::FontId::proportional(title_size),
                        theme
                            .color(term_core::TermColor::Indexed(3))
                            .gamma_multiply(if agent.backed { 1.0 } else { 0.45 }),
                    );
                    let position = title_left - egui::vec2(0.0, marker.size().y / 2.0);
                    if project.is_some() {
                        // The semantic yellow can also be this project's header color.
                        painter.with_clip_rect(rect).rect_filled(
                            egui::Rect::from_min_size(position, marker.size()).expand(2.0),
                            2.0,
                            theme.panel_background(),
                        );
                    }
                    painter
                        .with_clip_rect(rect)
                        .galley(position, marker, foreground);
                    title_left.x += title_size * 0.8;
                }
            }
            if let Some(project) = project {
                let available = (rect.right() - title_left.x - 8.0).max(0.0);
                let galley = painter.layout_no_wrap(
                    project.name.clone(),
                    egui::FontId::proportional(title_size),
                    foreground,
                );
                let width = galley.size().x.min(available * 0.45).min(140.0);
                let name_rect = egui::Rect::from_min_size(
                    egui::pos2(title_left.x, rect.center().y - galley.size().y / 2.0),
                    egui::vec2(width, galley.size().y),
                );
                if width > 0.0 {
                    painter.with_clip_rect(name_rect).galley(
                        name_rect.left_center() - egui::vec2(0.0, galley.size().y / 2.0),
                        galley,
                        foreground,
                    );
                    let separator = painter.with_clip_rect(rect).text(
                        egui::pos2(name_rect.right(), rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        " | ",
                        egui::FontId::proportional(title_size),
                        secondary,
                    );
                    title_left.x = separator.right();
                }
            }
            crate::header::title_text::paint(
                &painter.with_clip_rect(rect),
                title_left,
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(title_size),
                foreground,
            );
            continue;
        }
        paint_icon(&painter, rect.center(), target, zoomed, foreground);
    }
}

fn paint_gradient(
    painter: &egui::Painter,
    strip: egui::Rect,
    clip: egui::Rect,
    left: egui::Color32,
    right: egui::Color32,
) {
    let mut mesh = egui::Mesh::default();
    for (pos, color) in [
        (strip.left_top(), left),
        (strip.right_top(), right),
        (strip.right_bottom(), right),
        (strip.left_bottom(), left),
    ] {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter
        .with_clip_rect(clip.intersect(painter.clip_rect()))
        .add(mesh);
}

/// The prototype's 24-unit SVG paths, drawn inside a 13-point icon box.
fn paint_icon(
    painter: &egui::Painter,
    center: egui::Pos2,
    target: Target,
    zoomed: bool,
    color: egui::Color32,
) {
    let scale = 13.0 / 24.0;
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
                3.0 * 16.0 + 16.0 + 33.0 + 5.0,
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
                    number: 1,
                    title_size: 32.0,
                    can_close: true,
                    zoomed: false,
                    folded: false,
                    project: Some(&badge),
                },
            );
        });
        output.textures_delta.clear();
        for label in ["Backend", " | ", "Review changes"] {
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

    #[test]
    fn project_color_fills_the_whole_header_and_retains_the_console_title() {
        let ctx = egui::Context::default();
        let ink = egui::Color32::from_rgb(67, 192, 113);
        let theme = Theme::default().with_colors(&crate::settings::Colors {
            foreground: Some(egui::Color32::from_rgb(210, 225, 240)),
            panel: Some(egui::Color32::BLACK),
            ..Default::default()
        });
        let badge = crate::project_view::Badge {
            key: crate::project::Key::Repository("/work/backend".into()),
            name: "Backend".into(),
            details: "/work/backend".into(),
            ink,
            painted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        for width in [500.0, 185.0, 35.0] {
            let strip = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 33.0));
            let metrics = CellMetrics::of(8.0, 16.0);
            let label_rect = targets(strip, &metrics, true)[0].1;
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint(
                    ui,
                    strip,
                    &metrics,
                    &theme,
                    Appearance {
                        agent: None,
                        label: "Fix login",
                        number: 1,
                        title_size: 12.0,
                        can_close: true,
                        zoomed: false,
                        folded: false,
                        project: Some(&badge),
                    },
                );
            });
            out.textures_delta.clear();
            assert!(
                out.shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Mesh(mesh)
                    if mesh.calc_bounds() == strip && mesh.vertices.len() == 4
                    && mesh.vertices[0].color == egui::Color32::from_rgb(19, 54, 32)
                    && mesh.vertices[1].color == egui::Color32::from_rgb(4, 12, 7)
                    && mesh.vertices[2].color == mesh.vertices[1].color
                    && mesh.vertices[3].color == mesh.vertices[0].color
                    && mesh.indices == [0, 1, 2, 0, 2, 3])),
                "project color must cover the full header, including the controls"
            );
            let text: Vec<_> = out
                .shapes
                .iter()
                .filter_map(|s| {
                    if let egui::Shape::Text(text) = &s.shape {
                        assert!(s.clip_rect.right() <= label_rect.right());
                        if text.galley.job.text == "Backend" {
                            assert_ne!(ink, theme.foreground());
                            assert!(
                                text.galley.job.sections.iter().all(|section| {
                                    section.format.color == egui::Color32::WHITE
                                }),
                                "project text must contrast with the full colored header"
                            );
                        }
                        Some(text.galley.job.text.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            if width == 500.0 {
                assert!(text.contains(&"Backend"));
                assert!(text.contains(&" | "));
                assert!(text.contains(&"Fix login"));
            }
            out.drop_without_applying_deltas();
        }
        assert!(badge.painted.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn project_headers_render_contrasting_hover_and_back_the_yellow_waiting_marker() {
        let theme = Theme::default();
        let yellow = theme.color(term_core::TermColor::Indexed(3));
        for fill in [egui::Color32::BLACK, egui::Color32::WHITE, yellow] {
            let colors = crate::theme::project_header_colors(&theme, fill);
            let ink = colors.foreground;
            let ctx = egui::Context::default();
            let strip = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(500.0, 33.0));
            let metrics = CellMetrics::of(8.0, 16.0);
            let control = targets(strip, &metrics, true)[1].1;
            let badge = crate::project_view::Badge {
                key: crate::project::Key::Repository("/work/backend".into()),
                name: "Backend".into(),
                details: "/work/backend".into(),
                ink: fill,
                painted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            };
            for backed in [true, false] {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        events: vec![egui::Event::PointerMoved(control.center())],
                        ..Default::default()
                    },
                    |ui| {
                        paint(
                            ui,
                            strip,
                            &metrics,
                            &theme,
                            Appearance {
                                agent: Some(crate::agent_signals::View {
                                    client: crate::agent_signals::Client::Codex,
                                    busy: false,
                                    waiting: true,
                                    backed,
                                }),
                                label: "Fix login",
                                number: 1,
                                title_size: 12.0,
                                can_close: true,
                                zoomed: false,
                                folded: false,
                                project: Some(&badge),
                            },
                        )
                    },
                );
                output.textures_delta.clear();
                assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Mesh(mesh) if mesh.calc_bounds() == strip && mesh.vertices[0].color == colors.left && mesh.vertices[1].color == colors.right)));
                let mut text_count = 0;
                for (index, shape) in output.shapes.iter().enumerate() {
                    match &shape.shape {
                        egui::Shape::Text(text) => {
                            text_count += 1;
                            if text.galley.text() == "?" {
                                let bounds =
                                    egui::Rect::from_min_size(text.pos, text.galley.size());
                                assert!(output.shapes[..index].iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == theme.panel_background() && r.rect.contains_rect(bounds))), "waiting marker needs its neutral backing before it is drawn");
                                assert!(text.galley.job.sections.iter().all(|s| s.format.color
                                    == yellow.gamma_multiply(if backed { 1.0 } else { 0.45 })));
                            } else {
                                assert!(
                                    text.galley
                                        .job
                                        .sections
                                        .iter()
                                        .all(|s| s.format.color == ink),
                                    "ordinary header text must use opaque contrast-selected ink"
                                );
                            }
                        }
                        egui::Shape::LineSegment { stroke, .. } => assert_eq!(stroke.color, ink),
                        _ => {}
                    }
                }
                assert!(text_count >= 5);
                if backed {
                    // Interaction rectangles are known on the next pass too.
                    output.drop_without_applying_deltas();
                    continue;
                }
                assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Mesh(mesh) if s.clip_rect == control && mesh.calc_bounds() == strip && mesh.vertices[0].color == colors.hover_left && mesh.vertices[1].color == colors.hover_right)), "hover must preserve the full-header gradient's direction and use contrasting shades");
                output.drop_without_applying_deltas();
            }
        }
    }

    #[test]
    fn claude_logo_keeps_neutral_backing_and_signal_opacity_on_matching_project_fill() {
        let theme = Theme::default();
        let badge = crate::project_view::Badge {
            key: crate::project::Key::Repository("/work/backend".into()),
            name: "Backend".into(),
            details: "/work/backend".into(),
            ink: egui::Color32::from_rgb(0xD9, 0x77, 0x57),
            painted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
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
                        egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(500.0, 33.0)),
                        &CellMetrics::of(8.0, 16.0),
                        &theme,
                        Appearance {
                            agent: Some(crate::agent_signals::View {
                                client: crate::agent_signals::Client::Claude,
                                busy,
                                waiting: false,
                                backed,
                            }),
                            label: "Fix login",
                            number: 1,
                            title_size: 12.0,
                            can_close: true,
                            zoomed: false,
                            folded: false,
                            project: Some(&badge),
                        },
                    );
                },
            );
            let installed_textures: Vec<_> = output.textures_delta.set.keys().copied().collect();
            output.textures_delta.clear();
            let images: Vec<_> = output
                .shapes
                .iter()
                .enumerate()
                .filter_map(|(index, shape)| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                        Some((index, mesh))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                images.len(),
                1,
                "the installed Claude logo must actually be painted"
            );
            let (index, mesh) = images[0];
            assert!(installed_textures.contains(&mesh.texture_id));
            let bounds = mesh.calc_bounds();
            assert_eq!(bounds.size(), egui::Vec2::splat(16.0));
            assert!(
                output.shapes[..index].iter().any(|shape| matches!(
                    &shape.shape,
                    egui::Shape::Rect(rect)
                        if rect.fill == theme.panel_background() && rect.rect.contains_rect(bounds)
                )),
                "the real transparent logo needs neutral backing before its image is drawn"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .all(|vertex| { vertex.color == egui::Color32::WHITE.gamma_multiply(opacity) }),
                "fresh, busy and stale logo opacity must survive the project fill"
            );
            output.drop_without_applying_deltas();
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
