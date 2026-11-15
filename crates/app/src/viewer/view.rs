//! Docked document and review content.
use super::{Kind, State};
use eframe::egui::{self, Color32, FontId, RichText};
use std::path::Path;

pub(super) fn show(
    state: &mut State,
    parent: &mut egui::Ui,
    full: egui::Rect,
    theme: crate::theme::Theme,
    unavailable: bool,
) {
    let namespace = if state.kind == Kind::Markdown {
        "markdown-viewer"
    } else {
        "git-viewer"
    };
    egui::Area::new(egui::Id::new(("document-viewer", namespace)))
        .fade_in(false)
        .constrain(false)
        .fixed_pos(full.min)
        .movable(false)
        .order(egui::Order::Foreground)
        .show(parent.ctx(), |parent| {
            parent.set_min_size(full.size());
            parent.set_max_size(full.size());
            parent.set_clip_rect(full);
            parent.push_id(namespace, |parent| {
                let rect = full;
                let mut ui = parent.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                ui.set_clip_rect(rect);
                ui.set_min_size(rect.size());
                ui.set_max_size(rect.size());
                let surface = mix(theme.background(), theme.foreground(), 0.045);
                let line = mix(surface, theme.foreground(), 0.16);
                let frame = egui::Frame::new()
                    .fill(surface)
                    .corner_radius(0)
                    .stroke(egui::Stroke::new(1.0, line));
                ui.painter().add(frame.paint(rect));
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
                let (body, foot) =
                    rest.split_top_bottom_at_y((rest.bottom() - 38.0).max(rest.top()));
                footer(state, &mut content, foot, theme, line, unavailable);
                let mut body_ui = content.new_child(
                    egui::UiBuilder::new()
                        .max_rect(body)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                body_ui.set_clip_rect(body);
                body_content(state, &mut body_ui, theme, unavailable);
            });
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
}

fn footer(
    state: &State,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    theme: crate::theme::Theme,
    line: Color32,
    unavailable: bool,
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
    let source = if unavailable {
        format!("{source} - unavailable")
    } else {
        source.to_owned()
    };
    child.add(
        egui::Label::new(RichText::new(&source).size(11.0).color(mix(
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

fn body_content(
    state: &mut State,
    ui: &mut egui::Ui,
    theme: crate::theme::Theme,
    unavailable: bool,
) {
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
                !unavailable,
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
    fn dock_content_stays_within_the_supplied_rectangle() {
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::new(620.0, 0.0), egui::vec2(360.0, 480.0));
        let mut state = State::default();
        state.open_diff();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 600.0));
        let frame = |state: &mut State| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| show(state, ui, area, crate::theme::Theme::default(), true),
            )
        };
        frame(&mut state).drop_without_applying_deltas();
        let output = frame(&mut state);
        fn inspect(
            shape: &egui::Shape,
            area: egui::Rect,
            labels: &mut Vec<(String, egui::Pos2, egui::Rect)>,
        ) {
            match shape {
                egui::Shape::Text(text) => {
                    labels.push((
                        text.galley.text().to_owned(),
                        text.pos + text.galley.size() * 0.5,
                        area,
                    ));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        inspect(shape, area, labels);
                    }
                }
                _ => {}
            }
        }
        let mut labels = Vec::new();
        for shape in &output.shapes {
            inspect(&shape.shape, shape.clip_rect, &mut labels);
        }
        output.drop_without_applying_deltas();
        for expected in [
            "Review changes",
            "Local changes",
            "No source console - unavailable",
        ] {
            let (_, position, clip) = labels
                .iter()
                .find(|(label, _, _)| label == expected)
                .unwrap_or_else(|| panic!("missing dock label {expected:?}: {labels:?}"));
            assert!(area.contains(*position), "{expected:?} escaped {area:?}");
            assert!(
                clip.contains(*position),
                "{expected:?} escaped clip {clip:?}"
            );
        }
    }
}
