//! The drawer's document and review content.
use super::{Kind, State};
use eframe::egui::{self, FontId};
use std::path::Path;

/// The radius of the body's bottom corners, inside the drawer's one-point frame.
pub(super) const BODY_CORNER: u8 = crate::theme::metrics::RADIUS_CARD - 1;

pub(super) fn show(
    state: &mut State,
    parent: &mut egui::Ui,
    full: egui::Rect,
    theme: crate::theme::Theme,
    unavailable: bool,
    maximized: bool,
    (tabs, title, image): (
        &super::drawer::Tabs,
        &super::drawer::Title,
        Option<&mut super::image::Viewer>,
    ),
) {
    let namespace = if state.kind.is_document() {
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
                // The strip on the side-list surface, the document on the console's own
                // background: the kit's drawer (`DockB.dc.html`).
                let roles = crate::ui::style::roles(parent.ctx(), &theme);
                // `panel` is a colour of its own the person may set; where the text cannot be read on
                // it the chrome stays on the document's background instead. The border is drawn
                // toward the text where it would vanish into either surface.
                let surface = roles.panel_or(roles.tx, roles.base);
                let line = roles.apart(roles.line2, &[surface, roles.base], surface, 0.16);
                let frame = egui::Frame::new()
                    .fill(surface)
                    .corner_radius(crate::theme::metrics::RADIUS_CARD)
                    .stroke(egui::Stroke::new(1.0, line));
                ui.painter().add(frame.paint(rect));
                let grip = egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + 4.5),
                    egui::vec2(36.0, 3.0),
                );
                ui.painter().rect_filled(grip, 2, line);
                let inner = rect.shrink(1.0);
                let mut content = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(inner)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                content.set_clip_rect(inner);
                style(&mut content, &theme);
                let strip = super::drawer::Strip::framed(rect, tabs);
                let source = title.text();
                super::drawer::paint(&content, &strip, tabs, &roles, title, maximized);
                if let Some(rect) = strip.controls
                    && !state.kind.is_document()
                {
                    strip_controls(state, &mut content, rect, &theme);
                }
                for (action, place) in &strip.places {
                    let hint = super::drawer::hint(*action, maximized);
                    // A strip too narrow for all of itself says so where it can still be read.
                    let said = if strip.crowded && *action == super::drawer::StripAction::Maximize {
                        format!("{hint}: too narrow to show \"{source}\" and every tab")
                    } else {
                        hint.to_owned()
                    };
                    let response = content
                        .interact(
                            *place,
                            egui::Id::new(("drawer", hint)),
                            egui::Sense::hover(),
                        )
                        .on_hover_text(said);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, hint)
                    });
                }
                if !state.open {
                    return;
                }
                let body = egui::Rect::from_min_max(
                    egui::pos2(inner.left(), strip.rect.bottom()),
                    inner.max,
                );
                let mut body_ui = content.new_child(
                    egui::UiBuilder::new()
                        .max_rect(body)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                body_ui.set_clip_rect(body);
                body_ui.painter().rect_filled(
                    body,
                    egui::CornerRadius {
                        sw: BODY_CORNER,
                        se: BODY_CORNER,
                        ..Default::default()
                    },
                    roles.base,
                );
                match image {
                    Some(image) => image.show(&mut body_ui, theme, (state.space, state.keys)),
                    None => body_content(state, &mut body_ui, theme, unavailable),
                }
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

fn body_content(
    state: &mut State,
    ui: &mut egui::Ui,
    theme: crate::theme::Theme,
    unavailable: bool,
) {
    let document = state.kind.is_document();
    if document && state.requested.is_none() && state.job.is_none() && state.loaded.is_none() {
        egui::Frame::new().inner_margin(14).show(ui, |ui| {
            ui.label("Choose a file in Files.");
        });
    }
    // The comparison and refresh live on the strip; only a base to type or comments to clear
    // need a row of their own.
    if !document && (state.kind == Kind::AgainstBase || state.review.has_comments()) {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| controls(state, ui));
        ui.separator();
    }
    // The header stands whatever the file turned out to be: a binary still opens and copies.
    if state.kind == Kind::Code && Path::new(&state.path).is_absolute() {
        super::code_header::show(&mut state.code, Path::new(&state.path), ui, &theme);
    }
    if !state.message.is_empty() {
        ui.colored_label(
            theme.panel_color(Some(panel_contract::Mark::Block), None),
            &state.message,
        );
    }
    if state.job.is_some() && state.loaded.is_none() && state.message.is_empty() {
        ui.label("Loading…");
    }
    if let Some(loaded) = &state.loaded {
        if let Some(document) = &loaded.markdown {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(26, 22))
                .show(ui, |ui| document.show(ui));
        } else if let Some(document) = &loaded.code {
            let roles = crate::ui::style::roles(ui.ctx(), &theme);
            let palette = super::palette::Palette::of(&theme, &roles);
            super::code::show(document, &mut state.code, ui, &palette);
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
        Kind::Code => "File".into(),
        Kind::Local => "Local changes".into(),
        Kind::Unstaged => "Unstaged changes".into(),
        Kind::Staged => "Staged changes".into(),
        Kind::AgainstBase => format!("Compared with {base}"),
    }
}

/// The Git tab's comparison picker and refresh button, on the strip (`GitDiff.dc.html` has no row
/// for them; the owner kept both, compacted, on 17 November 2026).
fn strip_controls(
    state: &mut State,
    ui: &mut egui::Ui,
    rect: egui::Rect,
    theme: &crate::theme::Theme,
) {
    let editable = !state.review.has_comments();
    let mut load = false;
    let picker =
        egui::Rect::from_min_size(rect.min, egui::vec2(super::drawer::PICKER, rect.height()));
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(picker)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().interact_size.y = rect.height();
    row.spacing_mut().button_padding = egui::vec2(8.0, 2.0);
    let popup_style = egui::style::StyleModifier::from(row.style().as_ref().clone());
    egui::ComboBox::from_id_salt("viewer-comparison")
        .popup_style(popup_style)
        .width(super::drawer::PICKER - 16.0)
        .selected_text(comparison(state.kind, &state.base))
        .show_ui(&mut row, |ui| {
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
    let refresh = egui::Rect::from_min_size(
        egui::pos2(rect.right() - rect.height(), rect.top()),
        egui::vec2(rect.height(), rect.height()),
    );
    let enabled = editable && state.requested.is_some() && state.job.is_none();
    let response = ui
        .interact(
            refresh,
            egui::Id::new("drawer-refresh"),
            if enabled {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            },
        )
        .on_hover_text("Refresh the changes");
    let roles = crate::ui::style::roles(ui.ctx(), theme);
    if response.hovered() && enabled {
        ui.painter().rect_filled(refresh, 5, roles.hover);
    }
    crate::ui::icons::paint(
        ui.painter(),
        refresh.center(),
        crate::ui::icons::Icon::Refresh,
        13.0,
        if enabled { roles.tx2 } else { roles.tx3 },
    );
    if response.clicked() {
        state.refresh_at = Some(std::time::Instant::now());
    }
    if load && editable {
        state.editing_path = false;
        state.start(ui.ctx());
    }
}

/// The row a comparison against a typed base, or unsent comments, still need.
fn controls(state: &mut State, ui: &mut egui::Ui) {
    let editable = !state.review.has_comments();
    let mut load = false;
    ui.horizontal(|ui| {
        if state.kind == Kind::AgainstBase {
            ui.label("Base");
            ui.add_enabled(
                editable,
                egui::TextEdit::singleline(&mut state.base).desired_width(120.0),
            );
            load |= ui
                .add_enabled(editable, egui::Button::new("Compare"))
                .clicked();
        }
        if !editable && ui.button("Clear comments").clicked() {
            state.review.clear();
        }
    });
    if load && editable {
        state.editing_path = false;
        state.start(ui.ctx());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;

    fn tabs(markdown: bool) -> super::super::drawer::Tabs {
        super::super::drawer::Tabs {
            changes: Some(Some(3)),
            document: Some("PLAN.md".into()),
            markdown,
            side_by_side: false,
        }
    }

    fn title() -> super::super::drawer::Title {
        super::super::drawer::Title {
            project: Some(("billing-api".into(), Color32::from_rgb(0x43, 0xC0, 0x71))),
            folder: "billing".into(),
            unavailable: false,
        }
    }

    fn texts(shape: &egui::Shape, found: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text) => found.push((
                text.galley.text().to_owned(),
                text.galley.rect.translate(text.pos.to_vec2()),
            )),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| texts(s, found)),
            _ => {}
        }
    }

    /// The strip reads as the artboard: source, then the tabs with the count, the layout toggle
    /// for a diff, and maximize and close at the right, all on one row.
    #[test]
    fn the_drawer_strip_lays_its_tabs_and_controls_on_one_row() {
        use super::super::drawer::{HEIGHT, Strip, StripAction};
        let drawer = egui::Rect::from_min_size(egui::pos2(270.0, 400.0), egui::vec2(1000.0, 360.0));
        let strip = Strip::of(drawer, &tabs(false));
        let order: Vec<_> = strip.places.iter().map(|(action, _)| *action).collect();
        for action in [
            StripAction::Changes,
            StripAction::Document,
            StripAction::Unified,
            StripAction::SideBySide,
            StripAction::Maximize,
            StripAction::Close,
        ] {
            assert!(order.contains(&action), "{action:?} missing: {order:?}");
        }
        let place = |action| strip.places.iter().find(|(a, _)| *a == action).unwrap().1;
        assert!(place(StripAction::Changes).right() <= place(StripAction::Document).left());
        assert!(place(StripAction::SideBySide).right() < place(StripAction::Maximize).left());
        assert!(place(StripAction::Maximize).right() < place(StripAction::Close).left());
        assert!(place(StripAction::Close).right() <= drawer.right());
        for (_, rect) in &strip.places {
            assert!((rect.center().y - (drawer.top() + HEIGHT / 2.0)).abs() < 0.5);
        }
        let markdown = Strip::of(drawer, &tabs(true));
        assert!(
            !markdown
                .places
                .iter()
                .any(|(a, _)| matches!(a, StripAction::Unified | StripAction::SideBySide)),
            "a document has no diff layout to choose"
        );
        let narrow = Strip::of(
            egui::Rect::from_min_size(drawer.min, egui::vec2(300.0, 360.0)),
            &tabs(false),
        );
        let kept: Vec<_> = narrow.places.iter().map(|(a, _)| *a).collect();
        assert!(kept.contains(&StripAction::Maximize) && kept.contains(&StripAction::Close));
        let maximize = narrow
            .places
            .iter()
            .find(|(a, _)| *a == StripAction::Maximize)
            .unwrap()
            .1;
        assert!(
            narrow.places.iter().all(|(a, r)| matches!(
                a,
                StripAction::Maximize | StripAction::Close
            ) || r.right() <= maximize.left()),
            "a tab runs under the drawer's own buttons"
        );
        let ctx = egui::Context::default();
        let output = ctx.run_ui(Default::default(), |ui| {
            let roles = crate::ui::style::roles(ui.ctx(), &crate::theme::Theme::default());
            super::super::drawer::paint(ui, &strip, &tabs(false), &roles, &title(), false);
        });
        let mut found = Vec::new();
        for shape in &output.shapes {
            texts(&shape.shape, &mut found);
        }
        output.drop_without_applying_deltas();
        for (label, action) in [
            ("Changes", StripAction::Changes),
            ("3", StripAction::Changes),
            ("PLAN.md", StripAction::Document),
            ("Unified", StripAction::Unified),
            ("Side by side", StripAction::SideBySide),
        ] {
            let (_, rect) = found
                .iter()
                .find(|(text, _)| text == label)
                .unwrap_or_else(|| panic!("{label} is not painted: {found:?}"));
            assert!(
                place(action).contains(rect.center()),
                "{label} outside its place"
            );
        }
        assert!(found.iter().any(|(text, _)| text == "billing-api"));
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
                        (&tabs(true), &title(), None),
                    )
                },
            );
            let mut found = Vec::new();
            for shape in &output.shapes {
                labels(&shape.shape, &mut found);
            }
            output.drop_without_applying_deltas();
            if pass == 1 {
                assert!(found.iter().any(|text| text == "Choose a file in Files."));
                assert!(!found.iter().any(|text| text == "Open"));
                assert!(!found.iter().any(|text| text == "Markdown file"));
            }
        }
    }

    /// The drawer's chrome sits on `panel` and the document on `base`; a `panel` the text cannot be
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
                    |ui| {
                        show(
                            &mut state,
                            ui,
                            area,
                            theme,
                            true,
                            false,
                            (&tabs(false), &title(), None),
                        )
                    },
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
        let area =
            egui::Rect::from_min_size(egui::Pos2::new(300.0, 0.0), egui::vec2(1080.0, 480.0));
        let mut state = State::default();
        state.open_diff();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 600.0));
        let gone = super::super::drawer::Title {
            project: None,
            folder: "No source console".into(),
            unavailable: true,
        };
        let frame = |state: &mut State| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| {
                    show(
                        state,
                        ui,
                        area,
                        crate::theme::Theme::default(),
                        true,
                        false,
                        (&tabs(false), &gone, None),
                    )
                },
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
            "Changes",
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
