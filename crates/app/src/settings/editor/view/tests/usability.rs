//! Geometry and input witnesses for the native usability corrections.
use super::*;

fn editor(text: &str) -> Editor {
    Editor {
        draft: Some(Document::new(text.into())),
        ..Default::default()
    }
}

fn visit(shape: &egui::epaint::Shape, f: &mut impl FnMut(&egui::epaint::Shape)) {
    f(shape);
    if let egui::epaint::Shape::Vec(shapes) = shape {
        for shape in shapes {
            visit(shape, f);
        }
    }
}

#[test]
fn size_and_family_have_equal_height_and_aligned_value_baselines() {
    let mut editor = editor("font=Fixture Family\nsize=49\n");
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let (text, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let font = editor.font_rect.unwrap();
    let unit = text.iter().find(|(s, _)| s == "pt").unwrap().1;
    let mut frames = Vec::new();
    for clipped in shapes {
        visit(&clipped.shape, &mut |shape| {
            if let egui::epaint::Shape::Rect(rect) = shape
                && rect.rect.contains(unit.center())
                && rect.rect.width() < 180.0
            {
                frames.push(rect.rect);
            }
        });
    }
    let number_frame = frames
        .into_iter()
        .min_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("size field frame");
    assert!(
        (number_frame.height() - font.height()).abs() <= 1.0,
        "font {font:?}, size {number_frame:?}"
    );
    let selected = text.iter().find(|(s, _)| s == "Fixture Family").unwrap().1;
    let number = text.iter().find(|(s, _)| s == "49").unwrap().1;
    assert!(
        (selected.center().y - number.center().y).abs() <= 2.0,
        "font {selected:?}, number {number:?}"
    );
}

#[test]
fn override_badge_is_compact_instead_of_taking_control_height() {
    let mut editor = editor("background=#123456\n");
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let (text, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let label = text.iter().find(|(s, _)| s == "1 override").unwrap().1;
    let mut badge = None;
    for clipped in shapes {
        visit(&clipped.shape, &mut |shape| {
            if let egui::epaint::Shape::Rect(rect) = shape
                && rect.stroke.color
                    == crate::theme::Roles::of(&appearance::effective_theme(
                        editor.draft.as_ref().unwrap(),
                    ))
                    .line2
                && rect.rect.contains(label.center())
            {
                badge = Some(rect.rect);
            }
        });
    }
    let badge = badge.expect("override badge border");
    assert!(badge.height() <= 24.0, "badge too tall: {badge:?}");
    assert!(
        badge.width() - label.width() <= 16.0,
        "badge has excessive side padding"
    );
}

#[test]
fn cursor_radio_circles_are_inside_their_clip_and_settings_text_is_readable() {
    let mut editor = editor("");
    editor.section = 1;
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let (text, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let mut checked = 0;
    let mut readable = false;
    for clipped in shapes {
        visit(&clipped.shape, &mut |shape| match shape {
            egui::epaint::Shape::Circle(circle)
                if circle.radius >= 5.0 && circle.radius <= 12.0 =>
            {
                if text.iter().any(|(label, rect)| {
                    [
                        "Follow the program",
                        "Blink regardless of the program",
                        "Never blink",
                    ]
                    .contains(&label.as_str())
                        && (rect.center().y - circle.center.y).abs() < 3.0
                        && circle.center.x < rect.left()
                        && rect.left() - circle.center.x < 30.0
                }) {
                    let bounds = egui::Rect::from_center_size(
                        circle.center,
                        egui::Vec2::splat(circle.radius * 2.0 + circle.stroke.width),
                    );
                    assert!(
                        clipped.clip_rect.contains_rect(bounds),
                        "radio clipped: {bounds:?} by {:?}",
                        clipped.clip_rect
                    );
                    checked += 1;
                }
            }
            egui::epaint::Shape::Text(text) if text.galley.text() == "Follow the program" => {
                readable = text
                    .galley
                    .job
                    .sections
                    .iter()
                    .all(|s| s.format.font_id.size >= 14.0);
            }
            _ => {}
        });
    }
    assert_eq!(checked, 3, "all three radio outlines must be examined");
    assert!(readable, "Settings body must use the larger readable type");
}

#[test]
fn shortcut_actions_have_full_height_targets_and_space_between_controls() {
    let mut editor = editor("");
    editor.section = 2;
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let (text, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let none = text.iter().find(|(s, _)| s == "None").unwrap().1;
    let default = text.iter().find(|(s, _)| s == "Default").unwrap().1;
    let mut rectangles = Vec::new();
    for clipped in shapes {
        visit(&clipped.shape, &mut |shape| {
            if let egui::epaint::Shape::Rect(rect) = shape {
                rectangles.push(rect.rect);
            }
        });
    }
    let control = |at: egui::Pos2| {
        rectangles
            .iter()
            .filter(|rect| rect.contains(at))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .copied()
            .unwrap()
    };
    let hint = text.iter().find(|(s, _)| s == "Ctrl+Shift+D").unwrap().1;
    let input = control(hint.center());
    assert!(
        hint.left() - input.left() >= 10.0,
        "shortcut text needs inner padding: {hint:?}, {input:?}"
    );
    assert!(
        (hint.center().y - input.center().y).abs() <= 2.0,
        "shortcut text must be vertically centered: {hint:?}, {input:?}"
    );
    assert!(
        (hint.center().y - none.center().y).abs() <= 2.0,
        "shortcut values and action labels must align"
    );
    let none = control(none.center());
    let default = control(default.center());
    assert!(
        none.height() >= 34.0 && default.height() >= 34.0,
        "shortcut targets too short: {none:?}, {default:?}"
    );
    assert!(
        default.left() - none.right() >= 8.0,
        "shortcut actions need horizontal breathing room"
    );
}

fn arrow(key: egui::Key) -> Vec<egui::Event> {
    vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]
}

#[test]
fn font_popup_arrows_change_only_the_draft_and_enter_keeps_settings_open() {
    let file = File::new();
    let original = file.read();
    let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
    editor.open(Some(file.0.clone()));
    editor.fonts = Some(vec!["Fixture Alpha".into(), "Fixture Beta".into()]);
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    paint(&mut editor, &ctx, size, vec![]);
    let rect = editor.font_rect.unwrap();
    paint(&mut editor, &ctx, size, click(rect.center()));
    assert!(egui::Popup::is_any_open(&ctx));
    for (key, expected) in [
        (egui::Key::ArrowDown, "Fixture Alpha"),
        (egui::Key::ArrowDown, "Fixture Beta"),
        (egui::Key::ArrowUp, "Fixture Alpha"),
    ] {
        paint(&mut editor, &ctx, size, arrow(key));
        assert_eq!(
            editor
                .draft
                .as_ref()
                .unwrap()
                .fields
                .iter()
                .find(|f| f.key == "font")
                .unwrap()
                .value,
            expected
        );
    }
    paint(&mut editor, &ctx, size, arrow(egui::Key::Enter));
    assert!(!egui::Popup::is_any_open(&ctx));
    assert!(editor.is_open());
    assert_eq!(file.read(), original);
}

#[test]
fn draft_size_changes_the_rendered_specimen_instead_of_a_fixed_sample() {
    let mut editor = editor("size=49\n");
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let (_, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let mut specimen = false;
    for clipped in shapes {
        visit(&clipped.shape, &mut |shape| {
            if let egui::epaint::Shape::Text(text) = shape
                && text.galley.text() == "Aa Bb Cc 0123456789   > ~/workspace"
            {
                assert!(
                    text.galley
                        .job
                        .sections
                        .iter()
                        .all(|s| s.format.font_id.size == 49.0)
                );
                assert!(text.galley.rect.height() > 40.0);
                specimen = true;
            }
        });
    }
    assert!(specimen, "the actual specimen must be painted");
}

fn rect_fills(shape: &egui::epaint::Shape, out: &mut Vec<(egui::Rect, egui::Color32)>) {
    match shape {
        egui::epaint::Shape::Rect(rect) => out.push((rect.rect, rect.fill)),
        egui::epaint::Shape::Vec(shapes) => shapes.iter().for_each(|s| rect_fills(s, out)),
        _ => {}
    }
}

/// Settings is painted with the roles of the theme it previews, written out — the navigation on
/// `panel`, the primary action on `accent` with its `on_accent` ink — and the desktop's light or
/// dark preference changes none of it (D98).
#[test]
fn settings_paints_the_previewed_roles_whatever_the_desktop_prefers() {
    let rgb = |hex: u32| egui::Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8);
    for preference in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        let file = File::new();
        let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
        editor.open(Some(file.0.clone()));
        // The fixture file names Gruvbox; preview Ivy Night, whose roles are written out below.
        editor
            .draft
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|f| f.key == "theme")
            .unwrap()
            .value = "ivy-night".into();
        let ctx = egui::Context::default();
        ctx.set_theme(preference);
        // Final colours, without the modal's time-dependent fade.
        ctx.all_styles_mut(|style| style.animation_time = 0.0);
        let size = egui::vec2(1100.0, 850.0);
        paint(&mut editor, &ctx, size, vec![]);
        let (text, shapes) = scene(&mut editor, &ctx, size, vec![]);
        let mut rects = Vec::new();
        for clipped in &shapes {
            rect_fills(&clipped.shape, &mut rects);
        }
        let fill_under = |label: &str| {
            let at = text
                .iter()
                .find(|(s, _)| s == label)
                .unwrap_or_else(|| panic!("{label}: {text:?}"))
                .1
                .center();
            rects
                .iter()
                // A frameless button paints an empty rect over its row; only real fills count.
                .filter(|(rect, fill)| rect.contains(at) && *fill != egui::Color32::TRANSPARENT)
                .min_by(|(a, _), (b, _)| a.area().total_cmp(&b.area()))
                .map(|(_, fill)| *fill)
        };
        // "Cursor" is a navigation row that is not selected: the navigation's own fill shows.
        assert_eq!(fill_under("Cursor"), Some(rgb(0x171d21)), "{preference:?}");
        assert_eq!(
            fill_under("Save and close"),
            Some(rgb(0x3cc384)),
            "{preference:?}"
        );
        let ink = shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Save and close" => {
                let color = t.galley.job.sections[0].format.color;
                Some(if color == egui::Color32::PLACEHOLDER {
                    t.fallback_color
                } else {
                    color
                })
            }
            _ => None,
        });
        assert_eq!(ink, Some(egui::Color32::BLACK), "{preference:?}");
    }
}

/// The console-title and tab-label sizes use the same 40-point point-size box as the font's size,
/// not a bare 55-point text field.
#[test]
fn title_sizes_use_the_point_size_box() {
    let mut editor = Editor {
        draft: Some(Document::new("theme=ivy-night\n".into())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|style| style.animation_time = 0.0);
    let size = egui::vec2(1100.0, 1200.0);
    paint(&mut editor, &ctx, size, vec![]);
    let (text, shapes) = scene(&mut editor, &ctx, size, vec![]);
    let mut rects = Vec::new();
    for clipped in &shapes {
        rect_fills(&clipped.shape, &mut rects);
    }
    let base = egui::Color32::from_rgb(0x13, 0x18, 0x1b);
    for hint in ["14", "12"] {
        let boxes = text
            .iter()
            .filter(|(s, _)| s == hint)
            .filter(|(_, at)| {
                rects.iter().any(|(r, fill)| {
                    *fill == base && r.size() == egui::vec2(113.0, 40.0) && r.contains(at.center())
                })
            })
            .count();
        let expected = if hint == "12" { 2 } else { 1 };
        assert_eq!(boxes, expected, "{hint}: {text:?}");
    }
}

/// A point-size box carries the focus ring while it has the keyboard, like a framed field.
#[test]
fn a_point_size_box_shows_when_it_has_the_keyboard() {
    let mut editor = Editor {
        draft: Some(Document::new("theme=ivy-night\n".into())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    ctx.all_styles_mut(|style| style.animation_time = 0.0);
    let size = egui::vec2(1100.0, 1200.0);
    let boxes = |shapes: &[egui::epaint::ClippedShape]| {
        let mut found = Vec::new();
        for clipped in shapes {
            match &clipped.shape {
                egui::epaint::Shape::Rect(r) if r.rect.size() == egui::vec2(113.0, 40.0) => {
                    found.push((r.rect, r.stroke.color))
                }
                _ => {}
            }
        }
        found
    };
    paint(&mut editor, &ctx, size, vec![]);
    let (_, still) = scene(&mut editor, &ctx, size, vec![]);
    let at_rest = boxes(&still);
    // The font size's box: the title-size boxes share `point_field` but sit below the dialog's
    // visible height at this size.
    let (target, rest_edge) = *at_rest
        .iter()
        .find(|(_, c)| *c != egui::Color32::TRANSPARENT)
        .expect("a point-size box");
    paint(&mut editor, &ctx, size, click(target.center()));
    let (_, focused) = scene(&mut editor, &ctx, size, vec![]);
    let edge = boxes(&focused)
        .into_iter()
        .filter(|(r, c)| *r == target && *c != egui::Color32::TRANSPARENT)
        .map(|(_, c)| c)
        .next_back()
        .expect("the box is still painted");
    assert_ne!(edge, rest_edge, "the focused box looks like the others");
}
