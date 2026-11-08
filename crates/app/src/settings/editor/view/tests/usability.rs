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
                && rect.stroke.color == egui::Color32::from_rgb(91, 81, 61)
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
