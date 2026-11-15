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

fn assert_dialog_surface(editor: &mut Editor, ctx: &egui::Context) {
    let palette =
        palette::Palette::new(appearance::effective_theme(editor.draft.as_ref().unwrap()));
    let size = egui::vec2(1100.0, 850.0);
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
        egui::Visuals::dark().panel_fill,
        "dialog must not change context style"
    );
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
