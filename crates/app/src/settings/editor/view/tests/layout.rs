//! Approved Settings layout and real-interaction regression witnesses.
use super::*;

fn set_field(editor: &mut Editor, key: &str, value: &str) {
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
fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
#[test]
fn customized_count_includes_all_twenty_two_slots_even_invalid_draft_values() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    for field in &mut editor.draft.as_mut().unwrap().fields {
        if field.key.starts_with("color.")
            || ["background", "foreground", "cursor", "divider", "focus"]
                .contains(&field.key.as_str())
        {
            field.value = "#123456".into();
        }
    }
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    assert!(text.iter().any(|(s, _)| s == "Ivy"));
    assert!(
        text.iter()
            .any(|(s, _)| s.ends_with("21 colors customized"))
    );
    set_field(&mut editor, "panel", "invalid");
    let text = paint(&mut editor, &ctx, size, vec![]);
    assert!(
        text.iter()
            .any(|(s, _)| s.ends_with("22 colors customized"))
    );
    assert!(text.iter().any(|(s, _)| s == "22 overrides"));
    assert!(text.iter().any(|(s, _)| s == "1 error"));
    assert!(
        !text.iter().any(|(s, _)| s == "Reset colors to theme"),
        "colors start collapsed"
    );
}
#[test]
fn expanding_colors_and_reset_clear_only_overrides() {
    let mut editor = Editor { draft: Some(Document::new("theme=tokyonight\nfont=Keep Family\nsize=18\nkey.new-tab=none\nbackground=#123456\ncolor.15=#abcdef\n".into())), ..Default::default() };
    let ctx = egui::Context::default();
    ctx.style_mut_of(egui::Theme::Dark, |s| s.animation_time = 0.0);
    click_label(&mut editor, &ctx, "Customize colors");
    let text = paint(&mut editor, &ctx, egui::vec2(1100.0, 850.0), vec![]);
    for label in [
        "Background",
        "Text",
        "Divider",
        "Focus",
        "Panel",
        "ANSI palette: 16 colours",
    ] {
        assert!(text.iter().any(|(s, _)| s == label), "missing {label}");
    }
    assert!(!text.iter().any(|(s, _)| s == "15"));
    click_label(&mut editor, &ctx, "ANSI palette: 16 colours");
    let text = paint(&mut editor, &ctx, egui::vec2(1100.0, 850.0), vec![]);
    for index in 0..16 {
        assert!(
            text.iter().any(|(s, _)| s == &index.to_string()),
            "missing ANSI {index}"
        );
    }
    click_label(&mut editor, &ctx, "Reset colors to theme");
    let draft = editor.draft.as_ref().unwrap();
    assert!(
        draft
            .fields
            .iter()
            .filter(|f| f.key.starts_with("color.")
                || [
                    "background",
                    "foreground",
                    "cursor",
                    "divider",
                    "focus",
                    "panel"
                ]
                .contains(&f.key.as_str()))
            .all(|f| f.value.is_empty())
    );
    for (key, value) in [
        ("theme", "tokyonight"),
        ("font", "Keep Family"),
        ("size", "18"),
        ("key.new-tab", "none"),
    ] {
        assert_eq!(
            draft.fields.iter().find(|f| f.key == key).unwrap().value,
            value
        );
    }
    let text = paint(&mut editor, &ctx, egui::vec2(1100.0, 850.0), vec![]);
    assert!(
        text.iter()
            .any(|(s, _)| s == "Colors inherit from this theme.")
    );
}
fn shape_rects(shape: &egui::epaint::Shape, result: &mut Vec<(egui::Rect, egui::Color32)>) {
    match shape {
        egui::epaint::Shape::Rect(rect) => result.push((rect.rect, rect.fill)),
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                shape_rects(shape, result);
            }
        }
        _ => {}
    }
}
#[test]
fn only_save_and_close_has_the_primary_accent_fill() {
    let file = File::new();
    let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
    editor.open(Some(file.0.clone()));
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let (text, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let mut rects = Vec::new();
    for clipped in shapes {
        shape_rects(&clipped.shape, &mut rects);
    }
    for (label, primary) in [("Save and close", true), ("Save changes", false)] {
        let at = text.iter().find(|(s, _)| s == label).unwrap().1.center();
        let fills: Vec<_> = rects
            .iter()
            .filter(|(rect, _)| rect.contains(at))
            .map(|(_, fill)| *fill)
            .collect();
        assert_eq!(fills.contains(&ACCENT), primary, "{label}: {fills:?}");
    }
}
fn visible_text(
    shapes: &[egui::epaint::ClippedShape],
    size: egui::Vec2,
) -> Vec<(String, egui::Rect)> {
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
    let mut visible = Vec::new();
    for clipped in shapes {
        let mut labels = Vec::new();
        text_shapes(&clipped.shape, &mut labels);
        visible.extend(labels.into_iter().filter(|(_, rect)| {
            clipped.clip_rect.expand(0.5).contains_rect(*rect) && viewport.contains_rect(*rect)
        }));
    }
    visible
}
fn wheel(at: egui::Pos2, delta: f32) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            delta: egui::vec2(0.0, delta),
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
fn reach_label(
    editor: &mut Editor,
    ctx: &egui::Context,
    size: egui::Vec2,
    label: &str,
) -> egui::Rect {
    let at = egui::pos2(size.x * 0.7, size.y * 0.5);
    scene(editor, ctx, size, wheel(at, 10000.0));
    for _ in 0..30 {
        let (_, shapes) = scene(editor, ctx, size, vec![]);
        if let Some((_, rect)) = visible_text(&shapes, size).iter().find(|(s, _)| s == label) {
            return *rect;
        }
        scene(editor, ctx, size, wheel(at, -60.0));
    }
    let (text, shapes) = scene(editor, ctx, size, vec![]);
    let clips: Vec<_> = shapes
        .iter()
        .filter_map(|clipped| {
            let mut labels = Vec::new();
            text_shapes(&clipped.shape, &mut labels);
            labels
                .iter()
                .any(|(s, _)| s == label)
                .then_some(clipped.clip_rect)
        })
        .collect();
    panic!(
        "{label} must be visibly reachable at {size:?}; matching={:?}; clips={clips:?}",
        text.iter().filter(|(s, _)| s == label).collect::<Vec<_>>()
    );
}
#[test]
fn viewport_resize_keeps_body_editable_and_footer_reachable_with_notices() {
    let file = File::new();
    let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
    editor.open(Some(file.0.clone()));
    let ctx = egui::Context::default();
    for state in 0..3 {
        if state > 0 {
            editor.message =
                "The configuration changed outside Settings. Cancel and reopen to reload it. "
                    .repeat(10);
        }
        if state == 2 {
            set_field(&mut editor, "size", "invalid");
            set_field(&mut editor, "background", "red");
        }
        for size in [
            egui::vec2(1100.0, 850.0),
            egui::vec2(800.0, 600.0),
            egui::vec2(460.0, 430.0),
            egui::vec2(460.0, 360.0),
            egui::vec2(460.0, 220.0),
            egui::vec2(1100.0, 850.0),
        ] {
            for _ in 0..3 {
                scene(&mut editor, &ctx, size, vec![]);
            }
            // A body radio must accept a real click, even when fixed regions cannot fit together.
            editor.section = 1;
            let target = reach_label(&mut editor, &ctx, size, "Never blink");
            scene(&mut editor, &ctx, size, click(target.center()));
            assert_eq!(
                editor
                    .draft
                    .as_ref()
                    .unwrap()
                    .fields
                    .iter()
                    .find(|f| f.key == "cursor.blink")
                    .unwrap()
                    .value,
                "off"
            );
            set_field(&mut editor, "cursor.blink", "");
            for label in [
                "Cancel",
                "Save changes",
                "Save and close",
                "Restore defaults",
                "Changes apply when saved. Your running programs stay open.",
            ] {
                reach_label(&mut editor, &ctx, size, label);
            }
            if state == 2 {
                reach_label(
                    &mut editor,
                    &ctx,
                    size,
                    "2 setting errors: fix them before saving.",
                );
            }
            let (_, shapes) = scene(&mut editor, &ctx, size, vec![]);
            let mut rectangles = Vec::new();
            for clipped in &shapes {
                shape_rects(&clipped.shape, &mut rectangles);
            }
            let modal = rectangles
                .iter()
                .find(|(_, fill)| *fill == egui::Color32::from_rgb(25, 29, 38))
                .expect("modal background")
                .0;
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(modal),
                "modal exceeds {size:?}: {modal:?}"
            );
            editor.section = 0;
        }
    }
}
#[test]
fn all_popup_rows_reach_light_and_dark_palettes_through_visible_scroll_input() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    click_label(&mut editor, &ctx, "Change");
    click_label(&mut editor, &ctx, "All");
    let (_, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let visible = visible_text(&shapes, size);
    let dark = visible
        .iter()
        .find(|(s, _)| s == "Tokyo Night")
        .expect("All initially exposes a dark row")
        .1;
    let at = dark.center();
    let light = visible
        .iter()
        .find(|(s, _)| s == "Catppuccin Latte")
        .expect("All must show the first light palette fully inside its clip without scrolling")
        .1;
    scene(&mut editor, &ctx, size, click(light.center()));
    assert_eq!(draft_theme(&editor), "catppuccin-latte");
    let mut reached = None;
    for _ in 0..15 {
        let (_, shapes) = scene(&mut editor, &ctx, size, vec![]);
        if let Some((_, rect)) = visible_text(&shapes, size)
            .iter()
            .find(|(s, _)| s == "Rosé Pine Moon")
        {
            reached = Some(*rect);
            break;
        }
        scene(&mut editor, &ctx, size, wheel(at, -48.0));
    }
    let last = reached.expect("All must expose its last row after scrolling");
    scene(&mut editor, &ctx, size, click(last.center()));
    assert_eq!(draft_theme(&editor), "rose-pine-moon");
    assert!(editor.store.is_none());
    // Scroll back through the same popup and select the dark result again.
    for _ in 0..10 {
        scene(&mut editor, &ctx, size, wheel(at, 48.0));
    }
    let (_, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let visible = visible_text(&shapes, size);
    let dark = visible
        .iter()
        .find(|(s, _)| s == "Tokyo Night")
        .expect("All dark row remains reachable")
        .1;
    scene(&mut editor, &ctx, size, click(dark.center()));
    assert_eq!(draft_theme(&editor), "tokyonight");
}

#[test]
fn short_window_footer_cancel_accepts_click_after_scrolling() {
    let file = File::new();
    let original = file.read();
    let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
    editor.open(Some(file.0.clone()));
    set_field(&mut editor, "size", "invalid");
    editor.message = "The configuration changed outside Settings.".repeat(10);
    let ctx = egui::Context::default();
    let size = egui::vec2(460.0, 220.0);
    let cancel = reach_label(&mut editor, &ctx, size, "Cancel");
    scene(&mut editor, &ctx, size, click(cancel.center()));
    assert!(!editor.is_open());
    assert_eq!(file.read(), original);
}

#[test]
fn theme_font_and_color_popup_escape_preserves_settings_before_second_escape_discards() {
    let size = egui::vec2(1100.0, 850.0);
    for popup in ["theme", "font", "color"] {
        let file = File::new();
        let original = file.read();
        let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
        editor.open(Some(file.0.clone()));
        set_field(&mut editor, "size", "18");
        let ctx = egui::Context::default();
        ctx.style_mut_of(egui::Theme::Dark, |s| s.animation_time = 0.0);
        match popup {
            "theme" => click_label(&mut editor, &ctx, "Change"),
            "font" => {
                assert!(editor.fonts.is_none());
                // Use a deterministic existing cache; opening behavior must not enumerate again.
                editor.fonts = Some(vec!["Fixture Family".into()]);
                click_label(&mut editor, &ctx, "Built-in default");
                assert_eq!(editor.font_requests, 1);
            }
            "color" => {
                click_label(&mut editor, &ctx, "Customize colors");
                let text = paint(&mut editor, &ctx, size, vec![]);
                let label = text.iter().find(|(s, _)| s == "Background").unwrap().1;
                paint(
                    &mut editor,
                    &ctx,
                    size,
                    click(egui::pos2(label.min.x + 28.0, label.min.y - 24.0)),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            egui::Popup::is_any_open(&ctx),
            "{popup} popup must actually open"
        );
        paint(
            &mut editor,
            &ctx,
            size,
            vec![key_event(egui::Key::Escape, true)],
        );
        assert!(
            !egui::Popup::is_any_open(&ctx),
            "{popup} should close first"
        );
        assert!(editor.is_open(), "{popup} dismissal must preserve Settings");
        assert_eq!(
            editor
                .draft
                .as_ref()
                .unwrap()
                .fields
                .iter()
                .find(|f| f.key == "size")
                .unwrap()
                .value,
            "18"
        );
        assert_eq!(file.read(), original);
        paint(
            &mut editor,
            &ctx,
            size,
            vec![key_event(egui::Key::Escape, false)],
        );
        paint(
            &mut editor,
            &ctx,
            size,
            vec![key_event(egui::Key::Escape, true)],
        );
        assert!(!editor.is_open());
        assert_eq!(file.read(), original);
    }
}
#[test]
fn typography_reset_and_cursor_shortcuts_remain_native_and_staged() {
    let mut editor = Editor {
        draft: Some(Document::new("font=Fixture Family\nsize=20\n".into())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    assert_eq!(
        text.iter().filter(|(s, _)| s == "Fixture Family").count(),
        1
    );
    assert!(editor.fonts.is_none());
    assert_eq!(editor.font_requests, 0);
    assert!(text.iter().any(|(s, _)| s == "Programming ligatures"));
    click_label(&mut editor, &ctx, "Programming ligatures");
    assert!(
        editor
            .draft
            .as_ref()
            .unwrap()
            .text()
            .contains("font.ligatures = on")
    );
    assert!(editor.fonts.is_none());
    assert_eq!(editor.font_requests, 0);
    click_label(&mut editor, &ctx, "Reset");
    assert!(
        !editor
            .draft
            .as_ref()
            .unwrap()
            .text()
            .contains("font.ligatures")
    );
    for key in ["font", "size", "font.ligatures"] {
        assert!(
            editor
                .draft
                .as_ref()
                .unwrap()
                .fields
                .iter()
                .find(|f| f.key == key)
                .unwrap()
                .value
                .is_empty()
        );
    }
    click_label(&mut editor, &ctx, "Cursor");
    click_label(&mut editor, &ctx, "Never blink");
    assert_eq!(
        editor
            .draft
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .find(|f| f.key == "cursor.blink")
            .unwrap()
            .value,
        "off"
    );
    click_label(&mut editor, &ctx, "Shortcuts");
    let text = paint(&mut editor, &ctx, size, vec![]);
    assert!(text.iter().any(|(s, _)| s == "None"));
    click_label(&mut editor, &ctx, "None");
    assert!(
        editor
            .draft
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .any(|f| f.key.starts_with("key.") && f.value == "none")
    );
}
