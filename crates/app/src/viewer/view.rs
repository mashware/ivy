//! Floating review surface: fixed chrome around independently scrolling content.
use super::{GRIP, Kind, State};
use eframe::egui::{self, Color32, FontId, RichText};
use std::path::Path;

pub(super) fn show(
    state: &mut State,
    parent: &mut egui::Ui,
    full: egui::Rect,
    theme: crate::theme::Theme,
) {
    let rect = state.rect(full);
    egui::Area::new(egui::Id::new("document-viewer"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .movable(false)
        .show(parent.ctx(), |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            let surface = mix(theme.background(), theme.foreground(), 0.045);
            let line = mix(surface, theme.foreground(), 0.16);
            let frame = egui::Frame::new()
                .fill(surface)
                .corner_radius(9)
                .stroke(egui::Stroke::new(1.0, line))
                .shadow(egui::epaint::Shadow {
                    offset: [-10, 6],
                    blur: 32,
                    spread: 0,
                    color: Color32::from_black_alpha(90),
                });
            ui.painter().add(frame.paint(rect));
            let grip = egui::Rect::from_center_size(
                rect.left_center(),
                egui::vec2(GRIP * 2.0, rect.height()),
            );
            ui.interact(grip, egui::Id::new("viewer-divider"), egui::Sense::drag())
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            let inner = rect.shrink(1.0);
            let mut content = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            content.set_clip_rect(inner);
            style(&mut content, theme, surface, line);
            let (head, rest) =
                inner.split_top_bottom_at_y((inner.top() + 62.0).min(inner.bottom()));
            header(state, &mut content, head, theme, line);
            if !state.open {
                return;
            }
            let (body, foot) = rest.split_top_bottom_at_y((rest.bottom() - 38.0).max(rest.top()));
            footer(state, &mut content, foot, theme, line);
            let mut body_ui = content.new_child(
                egui::UiBuilder::new()
                    .max_rect(body)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            body_ui.set_clip_rect(body);
            body_content(state, &mut body_ui, theme);
        });
}

fn mix(a: Color32, b: Color32, fraction: f32) -> Color32 {
    let channel =
        |a: u8, b: u8| (f32::from(a) * (1.0 - fraction) + f32::from(b) * fraction).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

fn style(ui: &mut egui::Ui, theme: crate::theme::Theme, surface: Color32, line: Color32) {
    let style = ui.style_mut();
    let bg = theme.background();
    style.visuals = if u16::from(bg.r()) + u16::from(bg.g()) + u16::from(bg.b()) < 384 {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.override_font_id = Some(FontId::proportional(13.0));
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 5.0);
    style.visuals.override_text_color = Some(theme.foreground());
    style.visuals.panel_fill = surface;
    style.visuals.window_fill = surface;
    style.visuals.window_stroke = egui::Stroke::new(1.0, line);
    style.visuals.extreme_bg_color = theme.background();
    style.visuals.widgets.inactive.bg_fill = surface;
    style.visuals.widgets.inactive.weak_bg_fill = surface;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, line);
    style.visuals.widgets.inactive.corner_radius = 5.into();
    style.visuals.widgets.hovered.bg_fill = mix(surface, theme.foreground(), 0.08);
    style.visuals.widgets.hovered.weak_bg_fill = mix(surface, theme.foreground(), 0.08);
    style.visuals.selection.bg_fill = mix(surface, theme.foreground(), 0.09);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, theme.foreground());
}

fn header(
    state: &mut State,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    theme: crate::theme::Theme,
    line: Color32,
) {
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        egui::Stroke::new(1.0, line),
    );
    let markdown = state.kind == Kind::Markdown;
    let title = if markdown {
        state
            .requested
            .as_ref()
            .and_then(|(p, _)| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Markdown viewer".into())
    } else {
        "Review changes".into()
    };
    let path = state
        .requested
        .as_ref()
        .map(|(p, _)| p.display().to_string())
        .unwrap_or_else(|| {
            if markdown {
                "Markdown file".into()
            } else {
                "Current console".into()
            }
        });
    let icon_rect =
        egui::Rect::from_min_size(rect.min + egui::vec2(14.0, 21.0), egui::vec2(18.0, 20.0));
    ui.painter().rect_stroke(
        icon_rect.shrink2(egui::vec2(3.0, 1.0)),
        2,
        egui::Stroke::new(1.0, theme.foreground()),
        egui::StrokeKind::Inside,
    );
    for y in [7.0, 11.0, 15.0] {
        ui.painter().line_segment(
            [
                icon_rect.min + egui::vec2(6.0, y),
                icon_rect.min + egui::vec2(13.0, y),
            ],
            egui::Stroke::new(1.0, theme.foreground()),
        );
    }
    let text_rect = egui::Rect::from_min_max(
        rect.min + egui::vec2(42.0, 10.0),
        egui::pos2(
            (rect.right() - 92.0).max(rect.left() + 42.0),
            rect.bottom() - 8.0,
        ),
    );
    let mut text_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
    text_ui.spacing_mut().item_spacing.y = 2.0;
    let title_response = text_ui.add(
        egui::Label::new(RichText::new(title).size(14.0))
            .truncate()
            .sense(egui::Sense::click()),
    );
    if markdown && title_response.on_hover_text("Open another path").clicked() {
        state.editing_path = true;
        state.focus_path = true;
    }
    text_ui
        .add(
            egui::Label::new(RichText::new(&path).size(11.0).color(mix(
                theme.background(),
                theme.foreground(),
                0.6,
            )))
            .truncate(),
        )
        .on_hover_text(path);
    if icon_button(
        ui,
        egui::pos2(rect.right() - 40.0, rect.top() + 16.0),
        "Close",
    ) {
        state.close();
    }
    if icon_button(
        ui,
        egui::pos2(rect.right() - 76.0, rect.top() + 16.0),
        if state.expanded { "Restore" } else { "Expand" },
    ) {
        state.expanded = !state.expanded;
    }
}

fn icon_button(ui: &mut egui::Ui, pos: egui::Pos2, label: &str) -> bool {
    let rect = egui::Rect::from_min_size(pos, egui::vec2(28.0, 28.0));
    let response = ui
        .interact(
            rect,
            egui::Id::new(("viewer-icon", label)),
            egui::Sense::click(),
        )
        .on_hover_text(label);
    let scale = 17.0 / 24.0;
    let stroke = egui::Stroke::new(1.6 * scale, ui.visuals().text_color());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 5, ui.visuals().widgets.hovered.bg_fill);
    }
    let at = |x: f32, y: f32| rect.center() + egui::vec2(x, y) * scale;
    let line = |a: (f32, f32), b: (f32, f32)| {
        ui.painter()
            .line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    };
    match label {
        "Close" => {
            line((-6.0, -6.0), (6.0, 6.0));
            line((6.0, -6.0), (-6.0, 6.0));
        }
        "Restore" => {
            for (a, b) in [
                ((-3.0, -7.0), (-3.0, -9.0)),
                ((-3.0, -9.0), (9.0, -9.0)),
                ((9.0, -9.0), (9.0, 3.0)),
                ((9.0, 3.0), (7.0, 3.0)),
            ] {
                line(a, b);
            }
            ui.painter().rect_stroke(
                egui::Rect::from_min_max(at(-9.0, -3.0), at(3.0, 9.0)),
                scale,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        _ => {
            ui.painter().rect_stroke(
                egui::Rect::from_min_max(at(-7.0, -7.0), at(7.0, 7.0)),
                scale,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
    }
    response.clicked()
}

fn footer(
    state: &State,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    theme: crate::theme::Theme,
    line: Color32,
) {
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, line),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(13.0, 8.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let source = state
        .origin
        .as_ref()
        .map_or("No source console", |s| s.label.as_str());
    child.add(
        egui::Label::new(RichText::new(source).size(11.0).color(mix(
            theme.background(),
            theme.foreground(),
            0.6,
        )))
        .truncate(),
    );
    if state.kind == Kind::Markdown {
        child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("Updates on save").size(11.0).color(mix(
                theme.background(),
                theme.foreground(),
                0.6,
            )));
        });
    }
}

fn body_content(state: &mut State, ui: &mut egui::Ui, theme: crate::theme::Theme) {
    let markdown = state.kind == Kind::Markdown;
    if markdown && (state.editing_path || (state.requested.is_none() && state.job.is_none())) {
        egui::Frame::new()
            .inner_margin(14)
            .show(ui, |ui| path_form(state, ui));
    }
    if !markdown {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| controls(state, ui));
        ui.separator();
    }
    if !state.message.is_empty() {
        ui.colored_label(
            theme.panel_color(Some(panel_contract::Mark::Block), None),
            &state.message,
        );
    }
    if state.job.is_some() && state.loaded.is_none() {
        ui.label("Loading…");
    }
    if let Some(loaded) = &state.loaded {
        if let Some(document) = &loaded.markdown {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(26, 22))
                .show(ui, |ui| document.show(ui));
        } else if let Some(document) = &loaded.diff {
            let label = state
                .origin
                .as_ref()
                .map_or("unknown console", |s| s.label.as_str());
            let label = format!("{label} - {}", comparison(state.kind, &state.base));
            state.review.set_theme(theme);
            state.pending_send = state.review.show(
                ui,
                document,
                &label,
                state
                    .requested
                    .as_ref()
                    .map_or(Path::new(&state.path), |(path, _)| path.as_path()),
            );
        }
    }
}

fn path_form(state: &mut State, ui: &mut egui::Ui) {
    let editable = !state.review.has_comments();
    ui.label("Markdown file");
    let response = ui.add_enabled(
        editable,
        egui::TextEdit::singleline(&mut state.path)
            .id(egui::Id::new("viewer-path"))
            .desired_width(f32::INFINITY),
    );
    if state.focus_path {
        response.request_focus();
        state.focus_path = false;
    }
    let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if (ui
        .add_enabled(editable, egui::Button::new("Open"))
        .clicked()
        || enter)
        && editable
    {
        state.editing_path = false;
        state.start(ui.ctx());
    }
}

fn comparison(kind: Kind, base: &str) -> String {
    match kind {
        Kind::Markdown => "Markdown".into(),
        Kind::Local => "Local changes".into(),
        Kind::Unstaged => "Unstaged changes".into(),
        Kind::Staged => "Staged changes".into(),
        Kind::AgainstBase => format!("Compared with {base}"),
    }
}

fn controls(state: &mut State, ui: &mut egui::Ui) {
    let editable = !state.review.has_comments();
    let mut load = false;
    ui.horizontal_wrapped(|ui| {
        let popup_style = egui::style::StyleModifier::from(ui.style().as_ref().clone());
        egui::ComboBox::from_id_salt("viewer-comparison")
            .popup_style(popup_style)
            .selected_text(comparison(state.kind, &state.base))
            .show_ui(ui, |ui| {
                ui.add_enabled_ui(editable, |ui| {
                    for (kind, label) in [
                        (Kind::Local, "Local changes"),
                        (Kind::Unstaged, "Unstaged changes"),
                        (Kind::Staged, "Staged changes"),
                        (Kind::AgainstBase, "Against base"),
                    ] {
                        load |= ui.selectable_value(&mut state.kind, kind, label).changed()
                            && !state.path.is_empty();
                    }
                });
            });
        if let Some(document) = state
            .loaded
            .as_ref()
            .and_then(|loaded| loaded.diff.as_ref())
        {
            state.review.toolbar(ui, document);
        }
        if ui
            .add_enabled(
                editable && state.requested.is_some() && state.job.is_none(),
                egui::Button::new("Refresh"),
            )
            .clicked()
        {
            state.refresh_at = Some(std::time::Instant::now());
        }
        if !editable && ui.button("Clear comments").clicked() {
            state.review.clear();
        }
    });
    if state.kind == Kind::AgainstBase {
        ui.horizontal(|ui| {
            ui.label("Base");
            ui.add_enabled(
                editable,
                egui::TextEdit::singleline(&mut state.base).desired_width(120.0),
            );
            load |= ui
                .add_enabled(editable, egui::Button::new("Compare"))
                .clicked();
        });
    }
    if load && editable {
        state.editing_path = false;
        state.start(ui.ctx());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximizing_shows_restore_glyph_and_restoring_shows_maximize_glyph() {
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0));
        let mut state = State::default();
        state.open_markdown();
        let icon = egui::Rect::from_min_size(
            state.rect(area).min + egui::vec2(state.rect(area).width() - 76.0, 16.0),
            egui::vec2(28.0, 28.0),
        );
        let pos = icon.center();
        let run = |state: &mut State, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(area),
                    events,
                    ..Default::default()
                },
                |ui| show(state, ui, area, crate::theme::Theme::default()),
            )
        };
        fn count(shape: &egui::Shape, icon: egui::Rect) -> usize {
            match shape {
                egui::Shape::LineSegment { points, .. } => {
                    usize::from(points.iter().all(|point| icon.contains(*point)))
                }
                egui::Shape::Vec(shapes) => shapes.iter().map(|shape| count(shape, icon)).sum(),
                _ => 0,
            }
        }
        let lines = |state: &mut State| {
            let output = run(state, vec![egui::Event::PointerMoved(pos)]);
            let result: usize = output
                .shapes
                .iter()
                .map(|shape| count(&shape.shape, icon))
                .sum();
            output.drop_without_applying_deltas();
            result
        };
        run(&mut state, vec![]).drop_without_applying_deltas();
        assert_eq!(lines(&mut state), 0);
        for expanded in [true, false] {
            for pressed in [true, false] {
                run(
                    &mut state,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                )
                .drop_without_applying_deltas();
            }
            assert_eq!(state.expanded, expanded);
            assert_eq!(
                lines(&mut state),
                if expanded { 4 } else { 0 },
                "the painted icon did not follow the maximized state"
            );
        }
    }

    #[test]
    fn narrow_overlay_keeps_header_and_comparison_controls_visible() {
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 480.0));
        let mut state = State::default();
        state.open_diff();
        let run = |state: &mut State, events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(area),
                    events,
                    ..Default::default()
                },
                |ui| show(state, ui, area, crate::theme::Theme::default()),
            )
        };
        run(&mut state, vec![]).drop_without_applying_deltas();
        let output = run(&mut state, vec![]);
        let mut labels = Vec::new();
        fn collect(
            shape: &egui::Shape,
            clip: egui::Rect,
            labels: &mut Vec<(String, egui::Pos2, egui::Rect)>,
        ) {
            match shape {
                egui::Shape::Text(t) => {
                    labels.push((t.galley.text().into(), t.pos + t.galley.size() * 0.5, clip))
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, clip, labels);
                    }
                }
                _ => {}
            }
        }
        for shape in &output.shapes {
            collect(&shape.shape, shape.clip_rect, &mut labels);
        }
        output.drop_without_applying_deltas();
        for label in ["Review changes", "Local changes", "No source console"] {
            let (_, point, clip) = labels
                .iter()
                .find(|(text, _, _)| text == label)
                .unwrap_or_else(|| panic!("missing {label}: {labels:?}"));
            assert!(clip.contains(*point), "clipped {label} at {point:?}");
        }
        let rect = state.rect(area);
        let point = egui::pos2(rect.right() - 25.0, rect.top() + 30.0);
        for pressed in [true, false] {
            run(
                &mut state,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            )
            .drop_without_applying_deltas();
        }
        assert!(
            !state.open,
            "the compact close control did not dismiss the viewer"
        );
    }
}
