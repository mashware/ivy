//! Popup witnesses inspect rendered surfaces because each popup owns its UI style.
use super::*;

fn set(editor: &mut Editor, key: &str, value: &str) {
    editor
        .draft
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.key == key)
        .unwrap()
        .value = value.into();
}

fn fills(shape: &egui::epaint::Shape, out: &mut Vec<(egui::Rect, egui::Color32)>) {
    match shape {
        egui::epaint::Shape::Rect(r) => out.push((r.rect, r.fill)),
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                fills(shape, out);
            }
        }
        _ => {}
    }
}

fn rendered_text_color(shape: &egui::epaint::Shape) -> Option<(String, egui::Rect, egui::Color32)> {
    match shape {
        egui::epaint::Shape::Text(text) => Some((
            text.galley.text().to_owned(),
            text.galley.rect.translate(text.pos.to_vec2()),
            text.galley
                .job
                .sections
                .first()
                .map_or(egui::Color32::PLACEHOLDER, |section| section.format.color),
        )),
        egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(rendered_text_color),
        _ => None,
    }
}

fn assert_dialog_surface(editor: &mut Editor, ctx: &egui::Context) {
    let palette =
        palette::Palette::new(appearance::effective_theme(editor.draft.as_ref().unwrap()));
    let size = egui::vec2(1100.0, 850.0);
    let before = ctx.style_of(egui::Theme::Dark).visuals.panel_fill;
    paint(editor, ctx, size, vec![]);
    let (text, shapes) = scene(editor, ctx, size, vec![]);
    let mut rects = Vec::new();
    for shape in &shapes {
        fills(&shape.shape, &mut rects);
    }
    for label in [
        "Settings",
        "Ivy",
        "Save changes",
        "Changes apply when saved. Your running programs stay open.",
    ] {
        let at = text.iter().find(|(s, _)| s == label).unwrap().1.center();
        assert!(
            rects
                .iter()
                .any(|(rect, fill)| rect.contains(at) && *fill == palette.surface),
            "{label} must share draft surface"
        );
    }
    assert_eq!(
        ctx.style_of(egui::Theme::Dark).visuals.panel_fill,
        before,
        "dialog must not change context style"
    );
}

fn click_color_swatch(editor: &mut Editor, ctx: &egui::Context, label: &str) {
    let size = egui::vec2(1100.0, 850.0);
    paint(editor, ctx, size, vec![]);
    let text = paint(editor, ctx, size, vec![]);
    let center = text.iter().find(|(s, _)| s == label).unwrap().1.center();
    // The swatch sits directly above its caption in the color grid.
    paint(editor, ctx, size, click(center - egui::vec2(0.0, 29.0)));
}

#[test]
fn real_theme_selection_repaints_dialog_and_open_popup_without_saving() {
    let file = File::new();
    let original = file.read();
    let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
    editor.open(Some(file.0.clone()));
    let ctx = egui::Context::default();
    assert_dialog_surface(&mut editor, &ctx);
    click_label(&mut editor, &ctx, "Change");
    replace_search(&mut editor, &ctx, "github-light-default");
    click_label(&mut editor, &ctx, "GitHub Light Default");
    assert_eq!(draft_theme(&editor), "github-light-default");
    assert!(ctx.has_requested_repaint());
    assert_dialog_surface(&mut editor, &ctx);
    let p = palette::Palette::new(appearance::effective_theme(editor.draft.as_ref().unwrap()));
    let (text, shapes) = scene(&mut editor, &ctx, egui::vec2(1100.0, 850.0), vec![]);
    let at = text
        .iter()
        .find(|(s, _)| s == "github-light-default")
        .unwrap()
        .1
        .center();
    let mut rects = Vec::new();
    for shape in shapes {
        fills(&shape.shape, &mut rects);
    }
    assert!(
        rects.iter().any(|(r, c)| r.contains(at) && *c == p.surface),
        "open popup follows new draft"
    );
    assert_eq!(file.read(), original);
    assert!(!editor.is_applying());
}

#[test]
fn draft_colors_cancel_reopen_and_save_then_cancel_preserve_applied_theme_and_consoles() {
    use crate::tests::{area_of, one_frame_in, window_of_tabs};
    let file = File::new();
    let original = file.read();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    crate::settings::interpret(&original)
        .unwrap()
        .0
        .apply_to(&mut window);
    let original_theme = window.theme;
    let panes: Vec<_> = window.all_panes().map(|p| p.name).collect();
    window.settings_editor = Editor::resolved(crate::settings::Entry::Normal, None);
    window.settings_editor.open(Some(file.0.clone()));
    let ctx = egui::Context::default();
    for (key, value) in [
        ("theme", "github-light-default"),
        ("background", "#ffffff"),
        ("panel", "#ffffff"),
        ("foreground", "#ffffff"),
        ("focus", "#ffffff"),
    ] {
        set(&mut window.settings_editor, key, value);
    }
    assert_dialog_surface(&mut window.settings_editor, &ctx);
    for _ in 0..3 {
        one_frame_in(&mut window, &ctx, area_of(1200.0), vec![]);
    }
    assert_eq!(window.theme, original_theme);
    assert_eq!(file.read(), original);
    assert!(!window.settings_editor.is_applying());
    click_label(&mut window.settings_editor, &ctx, "Cancel");
    window.settings_editor.open(Some(file.0.clone()));
    assert_eq!(draft_theme(&window.settings_editor), "gruvbox-dark");
    set(&mut window.settings_editor, "theme", "github-light-default");
    click_label(&mut window.settings_editor, &ctx, "Save changes");
    for _ in 0..3 {
        one_frame_in(&mut window, &ctx, area_of(1200.0), vec![]);
    }
    let saved = window.theme;
    assert_eq!(saved, crate::theme::NamedTheme::GithubLightDefault.theme());
    let saved_bytes = file.read();
    set(&mut window.settings_editor, "theme", "tokyonight");
    assert_dialog_surface(&mut window.settings_editor, &ctx);
    click_label(&mut window.settings_editor, &ctx, "Cancel");
    assert_eq!(window.theme, saved);
    assert_eq!(file.read(), saved_bytes);
    window.settings_editor.open(Some(file.0.clone()));
    assert_eq!(draft_theme(&window.settings_editor), "github-light-default");
    assert_eq!(
        window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
        panes
    );
}

#[test]
fn gray_overrides_keep_hovered_theme_selector_ink_contrasted_on_its_rendered_fill() {
    let mut editor = Editor {
        draft: Some(crate::settings::document::Document::new(
            "theme=github-light-default\nbackground=#777777\npanel=#777777\nforeground=#777777\nfocus=#777777\ndivider=#777777\n".into(),
        )),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    click_label(&mut editor, &ctx, "Change");
    let labels = paint(&mut editor, &ctx, size, vec![]);
    let target = labels
        .iter()
        .find(|(text, _)| text == "Tokyo Night")
        .unwrap()
        .1
        .center();
    let (_, shapes) = scene(
        &mut editor,
        &ctx,
        size,
        vec![egui::Event::PointerMoved(target)],
    );
    let mut rects = Vec::new();
    let mut label = None;
    for clipped in &shapes {
        fills(&clipped.shape, &mut rects);
        if let Some(found) = rendered_text_color(&clipped.shape)
            && found.0 == "Tokyo Night"
        {
            label = Some(found);
        }
    }
    let (_, text_rect, ink) = label.expect("rendered theme selector label");
    let center = text_rect.center();
    let palette =
        palette::Palette::new(appearance::effective_theme(editor.draft.as_ref().unwrap()));
    let fill = rects
        .iter()
        .filter(|(rect, _)| rect.contains(center))
        .min_by(|(a, _), (b, _)| a.area().total_cmp(&b.area()))
        .map(|(_, color)| *color)
        .expect("rendered selector fill");
    assert_ne!(
        fill, palette.surface,
        "pointer should render the row hover state"
    );
    assert!(
        crate::theme::project_ink_contrast(ink, fill) >= 4.5,
        "selector ink {ink:?} on hover fill {fill:?}"
    );
}

#[test]
fn font_and_color_popups_render_the_current_light_and_dark_surface() {
    for theme in ["github-light-default", "tokyonight"] {
        for popup in ["font", "color"] {
            let mut editor = Editor {
                draft: Some(crate::settings::document::Document::new(format!(
                    "theme={theme}\n"
                ))),
                ..Default::default()
            };
            editor.fonts = Some(vec!["Fixture Family".into()]);
            let ctx = egui::Context::default();
            let size = egui::vec2(1100.0, 850.0);
            match popup {
                "font" => click_label(&mut editor, &ctx, "Built-in default"),
                "color" => {
                    click_label(&mut editor, &ctx, "Customize colors");
                    click_color_swatch(&mut editor, &ctx, "Background");
                }
                _ => unreachable!(),
            }
            assert!(egui::Popup::is_any_open(&ctx), "{popup} popup should open");
            let palette =
                palette::Palette::new(appearance::effective_theme(editor.draft.as_ref().unwrap()));
            let (text, shapes) = scene(&mut editor, &ctx, size, vec![]);
            let target_label = if popup == "font" {
                "Fixture Family"
            } else {
                "Background colour"
            };
            let target = text
                .iter()
                .find(|(s, _)| s == target_label)
                .unwrap_or_else(|| panic!("missing {target_label}: {text:?}"))
                .1
                .center();
            let mut rects = Vec::new();
            for clipped in &shapes {
                fills(&clipped.shape, &mut rects);
            }
            assert!(
                rects
                    .iter()
                    .any(|(rect, fill)| rect.contains(target) && *fill == palette.surface),
                "{theme} {popup} popup should inherit its palette surface"
            );
        }
    }
}

#[test]
fn color_popup_accepts_exact_hex_edit_without_dismissing() {
    let mut editor = Editor {
        draft: Some(crate::settings::document::Document::new(
            "theme=github-light-default\n".into(),
        )),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    click_label(&mut editor, &ctx, "Customize colors");
    click_color_swatch(&mut editor, &ctx, "Background");
    assert!(egui::Popup::is_any_open(&ctx));
    let (text, _) = scene(&mut editor, &ctx, size, vec![]);
    let hint = text
        .iter()
        .find(|(label, _)| label.starts_with("#"))
        .unwrap_or_else(|| panic!("expected inherited hex hint in popup: {text:?}"));
    let field = hint.1.center();
    scene(&mut editor, &ctx, size, click(field));
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    scene(
        &mut editor,
        &ctx,
        size,
        vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers,
            },
            egui::Event::Text("#123456".into()),
        ],
    );
    assert!(
        egui::Popup::is_any_open(&ctx),
        "editing a popup field must keep the popup open"
    );
    assert_eq!(
        editor
            .draft
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .find(|field| field.key == "background")
            .unwrap()
            .value,
        "#123456"
    );
}
