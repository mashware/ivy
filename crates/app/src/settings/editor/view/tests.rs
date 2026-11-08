//! Headless interaction witnesses; desktop appearance remains a separate check.
use super::*;
use crate::settings::document::Document;

fn text_shapes(shape: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::epaint::Shape::Text(text) => out.push((
            text.galley.text().to_owned(),
            text.galley.rect.translate(text.pos.to_vec2()),
        )),
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                text_shapes(shape, out);
            }
        }
        _ => {}
    }
}
fn paint(
    editor: &mut Editor,
    ctx: &egui::Context,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Rect)> {
    scene(editor, ctx, size, events).0
}
fn scene(
    editor: &mut Editor,
    ctx: &egui::Context,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> (Vec<(String, egui::Rect)>, Vec<egui::epaint::ClippedShape>) {
    let mut fonts = crate::fonts::Discoverer::default();
    let output = ctx.run_ui(
        egui::RawInput {
            time: Some(ctx.cumulative_frame_nr() as f64),
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| show(editor, ui.ctx(), &mut fonts),
    );
    let mut text = Vec::new();
    for shape in &output.shapes {
        text_shapes(&shape.shape, &mut text);
    }
    let shapes = output.shapes.clone();
    output.drop_without_applying_deltas();
    (text, shapes)
}
fn click(pos: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

struct File(std::path::PathBuf);
impl File {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "settings-footer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, "# keep\ntheme=gruvbox-dark\n").unwrap();
        Self(path)
    }
    fn read(&self) -> String {
        std::fs::read_to_string(&self.0).unwrap()
    }
}
impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn draft_theme(editor: &Editor) -> &str {
    &editor
        .draft
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .find(|f| f.key == "theme")
        .unwrap()
        .value
}
fn theme_button(editor: &Editor) -> String {
    let title = crate::theme::NamedTheme::from_written(draft_theme(editor))
        .map_or("Built-in theme (inherit)", |t| t.title());
    title.to_owned()
}
fn click_label(editor: &mut Editor, ctx: &egui::Context, label: &str) {
    let size = egui::vec2(1100.0, 850.0);
    let label = if label == "Change" {
        theme_button(editor)
    } else {
        label.to_owned()
    };
    paint(editor, ctx, size, vec![]);
    let text = paint(editor, ctx, size, vec![]);
    let at = text
        .iter()
        .find(|(s, _)| s == &label)
        .unwrap_or_else(|| panic!("missing {label}: {text:?}"))
        .1
        .center();
    paint(editor, ctx, size, click(at));
}

#[test]
fn actual_footer_buttons_persist_and_only_save_and_close_dismisses_settings() {
    for (button, stays_open) in [("Save changes", true), ("Save and close", false)] {
        let file = File::new();
        let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
        editor.open(Some(file.0.clone()));
        editor
            .draft
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|f| f.key == "theme")
            .unwrap()
            .value = "tokyonight".into();
        let ctx = egui::Context::default();
        click_label(&mut editor, &ctx, button);
        assert_eq!(
            crate::settings::interpret(&file.read()).unwrap().0.theme,
            Some(crate::theme::NamedTheme::TokyoNight),
            "{button}"
        );
        assert!(file.read().starts_with("# keep\n"));
        assert_eq!(editor.is_open(), stays_open, "{button}");
        assert_eq!(editor.message, "Saved. Applying settings...");
    }
}

#[test]
fn actual_save_and_close_conflict_preserves_open_draft_and_reports_error() {
    let file = File::new();
    let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
    editor.open(Some(file.0.clone()));
    editor
        .draft
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.key == "theme")
        .unwrap()
        .value = "tokyonight".into();
    std::fs::write(&file.0, "# external\nsize=19\n").unwrap();
    let ctx = egui::Context::default();
    click_label(&mut editor, &ctx, "Save and close");
    assert!(editor.is_open());
    assert_eq!(draft_theme(&editor), "tokyonight");
    assert!(editor.message.contains("changed outside"));
    assert_eq!(file.read(), "# external\nsize=19\n");
}

#[test]
fn actual_save_and_close_applies_pending_theme_to_surviving_consoles() {
    use crate::tests::{area_of, one_frame_in, window_of_tabs};
    let file = File::new();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    window.settings_editor = Editor::resolved(crate::settings::Entry::Normal, None);
    window.settings_editor.open(Some(file.0.clone()));
    window
        .settings_editor
        .draft
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|f| f.key == "theme")
        .unwrap()
        .value = "tokyonight".into();
    let panes: Vec<_> = window.all_panes().map(|p| p.name).collect();
    let old = window.theme.background();
    let ctx = egui::Context::default();
    click_label(&mut window.settings_editor, &ctx, "Save and close");
    assert!(!window.settings_editor.is_open());
    assert_eq!(
        window.theme.background(),
        old,
        "save only queues runtime application"
    );
    for _ in 0..3 {
        one_frame_in(&mut window, &ctx, area_of(1200.0), vec![]);
    }
    assert_eq!(
        window.theme.background(),
        crate::theme::NamedTheme::TokyoNight.theme().background()
    );
    assert_eq!(
        window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
        panes
    );
    assert_eq!(window.settings_editor.message, "Saved and applied.");
}

fn replace_search(editor: &mut Editor, ctx: &egui::Context, value: &str) {
    let label = if editor.picker_query.is_empty() {
        "Search themes...".to_owned()
    } else {
        editor.picker_query.clone()
    };
    click_label(editor, ctx, &label);
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    paint(
        editor,
        ctx,
        egui::vec2(1100.0, 850.0),
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
            egui::Event::Text(value.into()),
        ],
    );
    assert_eq!(
        editor.picker_query, value,
        "text input must reach the real search control"
    );
}

#[test]
fn real_search_events_match_case_insensitive_names_ids_and_preserve_empty_selection() {
    let file = File::new();
    let original = file.read();
    let mut editor = Editor::resolved(crate::settings::Entry::Normal, None);
    editor.open(Some(file.0.clone()));
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    click_label(&mut editor, &ctx, "Change");
    replace_search(&mut editor, &ctx, "tOkYo NiGhT");
    let text = paint(&mut editor, &ctx, size, vec![]);
    assert!(text.iter().any(|(s, _)| s == "Tokyo Night"));
    assert!(!text.iter().any(|(s, _)| s == "Nord"));
    replace_search(&mut editor, &ctx, "tOkYoNiGhT");
    let text = paint(&mut editor, &ctx, size, vec![]);
    assert!(text.iter().any(|(s, _)| s == "Tokyo Night"));
    click_label(&mut editor, &ctx, "Tokyo Night");
    assert_eq!(draft_theme(&editor), "tokyonight");
    replace_search(&mut editor, &ctx, "no-such-palette-327");
    let text = paint(&mut editor, &ctx, size, vec![]);
    assert!(text.iter().any(|(s, _)| s == "No matching themes."));
    paint(
        &mut editor,
        &ctx,
        size,
        vec![egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(draft_theme(&editor), "tokyonight");
    assert_eq!(file.read(), original, "browsing must not save");
    assert!(editor.message.is_empty());
}

#[test]
fn real_filter_clicks_restrict_results_and_all_restores_dark_and_light() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    click_label(&mut editor, &ctx, "Change");
    replace_search(&mut editor, &ctx, "gItHuB");
    let titles = [
        ("GitHub Dark Default", true),
        ("GitHub Light Default", false),
    ];
    for (filter, dark) in [("Dark", Some(true)), ("Light", Some(false)), ("All", None)] {
        click_label(&mut editor, &ctx, filter);
        let text = paint(&mut editor, &ctx, size, vec![]);
        for &(title, is_dark) in &titles {
            assert_eq!(
                text.iter().any(|(s, _)| s == title),
                dark.is_none_or(|d| d == is_dark),
                "{filter}: {title}"
            );
        }
        assert_eq!(
            draft_theme(&editor),
            "",
            "filtering alone preserves the draft"
        );
    }
}
#[test]
fn opening_appearance_keeps_every_override_inherited_and_fonts_lazy() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    for label in ["Built-in theme (inherit)", "Theme & colors", "Typography"] {
        assert!(text.iter().any(|(s, _)| s == label), "missing {label}");
    }
    assert!(
        !text.iter().any(|(s, _)| s == "15"),
        "ANSI palette must start collapsed"
    );
    assert!(
        editor
            .draft
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .all(|f| f.value.is_empty())
    );
    assert!(editor.fonts.is_none());
    assert_eq!(editor.font_requests, 0);
}
#[test]
fn narrow_short_dialog_keeps_footer_inside_the_window() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(460.0, 430.0);
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    for label in [
        "Restore defaults",
        "Cancel",
        "Save changes",
        "Save and close",
    ] {
        let (_, rect) = text
            .iter()
            .find(|(s, _)| s == label)
            .unwrap_or_else(|| panic!("missing {label}: {text:?}"));
        assert!(
            rect.min.x >= 0.0 && rect.max.x <= size.x && rect.min.y >= 0.0 && rect.max.y <= size.y,
            "{label}: {rect:?}"
        );
    }
}
#[test]
fn clicking_theme_sample_changes_only_the_draft() {
    let mut editor = Editor {
        draft: Some(Document::new("# preserved\n".into())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    let change = text
        .iter()
        .find(|(s, _)| s == &theme_button(&editor))
        .unwrap()
        .1
        .center();
    paint(&mut editor, &ctx, size, click(change));
    let text = paint(&mut editor, &ctx, size, vec![]);
    let center = text
        .iter()
        .find(|(s, _)| s == "Tokyo Night")
        .unwrap()
        .1
        .center();
    paint(&mut editor, &ctx, size, click(center));
    let text = editor.draft.as_ref().unwrap().text();
    assert!(text.contains("theme = tokyonight"));
    assert!(text.starts_with("# preserved\n"));
    assert!(editor.store.is_none());
    assert!(editor.message.is_empty());
}

#[test]
fn wide_dialog_keeps_heading_and_footer_inside_the_window_across_passes() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(800.0, 600.0);
    for pass in 0..6 {
        let text = paint(&mut editor, &ctx, size, vec![]);
        if pass == 0 {
            continue;
        }
        for label in [
            "Settings",
            "Appearance",
            "Save changes",
            "Save and close",
            "Cancel",
            "Restore defaults",
        ] {
            let matching: Vec<_> = text.iter().filter(|(s, _)| s == label).collect();
            assert!(
                !matching.is_empty(),
                "pass {pass}: missing {label}: {text:?}"
            );
            for (_, rect) in matching {
                assert!(
                    rect.min.x >= 0.0
                        && rect.max.x <= size.x
                        && rect.min.y >= 0.0
                        && rect.max.y <= size.y,
                    "pass {pass}, {label}: {rect:?}"
                );
            }
        }
    }
}

#[test]
fn escape_closes_picker_before_settings() {
    let mut editor = Editor {
        draft: Some(Document::new(String::new())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    click_label(&mut editor, &ctx, "Change");
    assert!(egui::Popup::is_any_open(&ctx));
    let escape = || {
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    };
    paint(&mut editor, &ctx, size, escape());
    assert!(!egui::Popup::is_any_open(&ctx));
    assert!(editor.draft.is_some());
    paint(
        &mut editor,
        &ctx,
        size,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    paint(&mut editor, &ctx, size, escape());
    assert!(editor.draft.is_none());
}

#[test]
fn filtered_arrow_navigation_starts_at_first_result() {
    let mut editor = Editor {
        draft: Some(Document::new("theme = gruvbox-dark\n".into())),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let size = egui::vec2(1100.0, 850.0);
    paint(&mut editor, &ctx, size, vec![]);
    let text = paint(&mut editor, &ctx, size, vec![]);
    let change = text
        .iter()
        .find(|(s, _)| s == &theme_button(&editor))
        .unwrap()
        .1
        .center();
    paint(&mut editor, &ctx, size, click(change));
    editor.picker_query = "tokyo".into();
    paint(&mut editor, &ctx, size, vec![]);
    paint(
        &mut editor,
        &ctx,
        size,
        vec![egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(
        editor
            .draft
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .find(|f| f.key == "theme")
            .unwrap()
            .value,
        "tokyonight"
    );
}

mod layout;

mod usability;
