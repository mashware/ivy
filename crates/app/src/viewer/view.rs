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
    maximized: bool,
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
                // Header and footer on the side-list surface, the document on the console's own
                // background: the kit's floating viewer.
                let roles = crate::ui::style::roles(parent.ctx(), &theme);
                // `panel` is a colour of its own the person may set; where the text cannot be read on
                // it the chrome stays on the document's background instead. The rules are the
                // person's `divider`, drawn toward the text where it would vanish into the chrome.
                let surface = roles.panel_or(roles.tx, roles.base);
                // The frame's sides run between the console and the document, both on `base`.
                let line = roles.apart(roles.line, &[surface, roles.base], surface, 0.16);
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
                style(&mut content, &theme);
                let (head, rest) =
                    inner.split_top_bottom_at_y((inner.top() + 62.0).min(inner.bottom()));
                header(state, &mut content, head, line, maximized);
                if !state.open {
                    return;
                }
                let (body, foot) =
                    rest.split_top_bottom_at_y((rest.bottom() - 38.0).max(rest.top()));
                footer(state, &mut content, foot, line, unavailable);
                let mut body_ui = content.new_child(
                    egui::UiBuilder::new()
                        .max_rect(body)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                body_ui.set_clip_rect(body);
                body_ui.painter().rect_filled(body, 0.0, roles.base);
                body_content(state, &mut body_ui, theme, unavailable);
            });
        });
}

fn style(ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    crate::ui::floating::style(ui, theme);
    let style = ui.style_mut();
    style.override_font_id = Some(FontId::proportional(13.0));
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 5.0);
}

/// Secondary text in the viewer's chrome: the path, the source console, the save note.
fn muted(ui: &egui::Ui) -> Color32 {
    ui.visuals().weak_text_color()
}

fn header(state: &mut State, ui: &mut egui::Ui, rect: egui::Rect, line: Color32, maximized: bool) {
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
                String::new()
            } else {
                "Current console".into()
            }
        });
    let title_y = rect.center().y - if path.is_empty() { 0.0 } else { 9.0 };
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 23.0, title_y),
        egui::vec2(16.0, 16.0),
    );
    crate::sidebar::icon(
        ui,
        icon_rect,
        if markdown {
            crate::sidebar::Tool::Markdown
        } else {
            crate::sidebar::Tool::Git
        },
        ui.visuals().widgets.inactive.fg_stroke.color,
    );
    if markdown {
        let target = crate::sidebar::maximize_rect(rect.expand(1.0));
        let label = if maximized { "Restore" } else { "Maximize" };
        let response = ui
            .interact(
                target,
                egui::Id::new("markdown-maximize"),
                egui::Sense::click(),
            )
            .on_hover_text(label);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(
                target,
                4,
                if response.is_pointer_button_down_on() {
                    ui.visuals().widgets.active.bg_fill
                } else {
                    ui.visuals().widgets.hovered.bg_fill
                },
            );
        }
        crate::header::integrated::paint_icon(
            ui.painter(),
            target.center(),
            crate::header::integrated::Target::Zoom,
            maximized,
            ui.visuals().widgets.inactive.fg_stroke.color,
            24.0,
        );
    }
    let right = (rect.right() - 54.0).max(rect.left() + 42.0);
    let title_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 42.0, title_y - 10.0),
        egui::pos2(right, title_y + 10.0),
    );
    let mut text_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(title_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    text_ui.set_min_height(20.0);
    text_ui.add(egui::Label::new(RichText::new(title).size(14.0)).truncate());
    if !path.is_empty() {
        let path_rect = egui::Rect::from_min_max(
            egui::pos2(title_rect.left(), title_y + 12.0),
            egui::pos2(right, rect.bottom() - 5.0),
        );
        let mut path_ui = ui.new_child(egui::UiBuilder::new().max_rect(path_rect));
        path_ui
            .add(egui::Label::new(RichText::new(&path).size(11.0).color(muted(ui))).truncate())
            .on_hover_text(path);
    }
}

fn footer(state: &State, ui: &mut egui::Ui, rect: egui::Rect, line: Color32, unavailable: bool) {
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
    child.add(egui::Label::new(RichText::new(&source).size(11.0).color(muted(ui))).truncate());
    if state.kind == Kind::Markdown {
        child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("Updates on save").size(11.0).color(muted(ui)));
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
    if markdown && state.requested.is_none() && state.job.is_none() && state.loaded.is_none() {
        egui::Frame::new().inner_margin(14).show(ui, |ui| {
            ui.label("Choose a Markdown file in Files.");
        });
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
    fn markdown_header_centers_title_on_its_icon_and_draws_a_readable_zoom_control() {
        fn inspect(
            shape: &egui::Shape,
            title: &mut Option<f32>,
            icon: &mut Option<f32>,
            zoom: &mut Option<(f32, f32)>,
        ) {
            match shape {
                egui::Shape::Text(text) if text.galley.text() == "Markdown viewer" => {
                    *title = Some(text.pos.y + text.galley.rect.center().y)
                }
                egui::Shape::Rect(rect)
                    if rect.rect.center().x < 100.0 && (rect.rect.width() - 16.0).abs() < 0.1 =>
                {
                    *icon = Some(rect.rect.center().y)
                }
                egui::Shape::Rect(rect) if rect.rect.center().x > 400.0 => {
                    *zoom = Some((rect.rect.width(), rect.rect.center().y))
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        inspect(shape, title, icon, zoom);
                    }
                }
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(480.0, 62.0));
        let mut state = State::default();
        let output = ctx.run_ui(Default::default(), |ui| {
            header(&mut state, ui, area, Color32::GRAY, false)
        });
        let (mut title, mut icon, mut zoom) = (None, None, None);
        for shape in &output.shapes {
            inspect(&shape.shape, &mut title, &mut icon, &mut zoom);
        }
        output.drop_without_applying_deltas();
        assert!(
            (title.expect("title painted") - icon.expect("document icon painted")).abs() < 1.0,
            "title and icon are on different rows"
        );
        assert!(
            (title.unwrap() - zoom.expect("zoom outline painted").1).abs() < 1.0,
            "zoom control is not centered on the header row"
        );
        assert!(
            zoom.expect("zoom outline painted").0 >= 14.0,
            "zoom glyph is too small for the document header"
        );
    }

    #[test]
    fn empty_markdown_directs_selection_to_files_without_an_open_form() {
        fn labels(shape: &egui::Shape, found: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) => found.push(text.galley.text().to_owned()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        labels(shape, found);
                    }
                }
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(480.0, 600.0));
        let mut state = State::default();
        state.open_markdown();
        for pass in 0..2 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(area),
                    ..Default::default()
                },
                |ui| {
                    show(
                        &mut state,
                        ui,
                        area,
                        crate::theme::Theme::default(),
                        false,
                        false,
                    )
                },
            );
            let mut found = Vec::new();
            for shape in &output.shapes {
                labels(&shape.shape, &mut found);
            }
            output.drop_without_applying_deltas();
            if pass == 1 {
                assert!(
                    found
                        .iter()
                        .any(|text| text == "Choose a Markdown file in Files.")
                );
                assert!(!found.iter().any(|text| text == "Open"));
                assert!(!found.iter().any(|text| text == "Markdown file"));
            }
        }
    }

    /// The viewer's chrome sits on `panel` and the document on `base`; a `panel` the text cannot be
    /// read on leaves the chrome on `base` instead (D98).
    #[test]
    fn the_viewer_frames_the_document_with_the_theme_roles() {
        fn fills(shape: &egui::Shape, out: &mut Vec<(egui::Rect, Color32)>) {
            match shape {
                egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| fills(s, out)),
                _ => {}
            }
        }
        let rgb = |hex: u32| Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
        let area = egui::Rect::from_min_size(egui::pos2(620.0, 0.0), egui::vec2(360.0, 480.0));
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 600.0));
        let unreadable = {
            let (settings, _) = crate::settings::interpret("panel=#e4e8e2").unwrap();
            crate::theme::Theme::default().with_colors(&settings.colors)
        };
        // A divider set to the console background would hide the frame's sides against the
        // document and the console; it is drawn toward the text instead.
        let hidden_divider = {
            let (settings, _) = crate::settings::interpret("divider=#13181b").unwrap();
            crate::theme::Theme::default().with_colors(&settings.colors)
        };
        let roles = crate::theme::Roles::of(&hidden_divider);
        assert!(
            crate::theme::project_ink_contrast(
                roles.apart(roles.line, &[roles.panel, roles.base], roles.panel, 0.16),
                roles.base
            ) >= 1.03
        );
        for (theme, chrome) in [
            (crate::theme::Theme::default(), rgb(0x171d21)),
            (unreadable, rgb(0x13181b)),
            (hidden_divider, rgb(0x171d21)),
        ] {
            let ctx = egui::Context::default();
            let mut state = State::default();
            state.open_diff();
            let mut shapes = Vec::new();
            for _ in 0..2 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(screen),
                        ..Default::default()
                    },
                    |ui| show(&mut state, ui, area, theme, true, false),
                );
                shapes = output.shapes.clone();
                output.drop_without_applying_deltas();
            }
            let mut rects = Vec::new();
            for clipped in &shapes {
                fills(&clipped.shape, &mut rects);
            }
            assert!(
                rects
                    .iter()
                    .any(|(r, c)| r.contains_rect(area) && *c == chrome),
                "chrome {chrome:?}: {rects:?}"
            );
            let stroke = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Rect(r) if r.rect.contains_rect(area) && r.fill == chrome => {
                        Some(r.stroke.color)
                    }
                    _ => None,
                })
                .expect("the frame is painted");
            assert!(
                crate::theme::project_ink_contrast(stroke, rgb(0x13181b)) >= 1.03,
                "the frame's sides vanish into the document"
            );
            assert!(
                rects.iter().any(|(r, c)| *c == rgb(0x13181b)
                    && r.height() > 300.0
                    && !r.contains_rect(area)),
                "the document is not on base"
            );
        }
    }

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
                |ui| show(state, ui, area, crate::theme::Theme::default(), true, false),
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
