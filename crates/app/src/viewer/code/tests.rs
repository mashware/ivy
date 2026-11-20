use super::*;

fn palette() -> Palette {
    let theme = crate::theme::Theme::default();
    Palette::of(&theme, &crate::theme::Roles::of(&theme))
}

/// Run the view over `document` in a 600×300 area, feeding `events`, and return what it painted
/// and what it asked the desktop to copy.
fn frame(
    ctx: &egui::Context,
    document: &CodeDocument,
    state: &mut CodeState,
    events: Vec<egui::Event>,
) -> (Vec<(String, egui::Rect, egui::Color32)>, Vec<String>) {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0));
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(area),
            events,
            ..Default::default()
        },
        |ui| show(document, state, ui, &palette()),
    );
    let mut texts = Vec::new();
    for clipped in &output.shapes {
        if let egui::Shape::Text(text) = &clipped.shape {
            let color = text
                .galley
                .job
                .sections
                .first()
                .map_or(text.fallback_color, |s| s.format.color);
            texts.push((
                text.galley.text().to_owned(),
                text.galley.rect.translate(text.pos.to_vec2()),
                color,
            ));
        }
    }
    let copied = output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .collect();
    output.drop_without_applying_deltas();
    (texts, copied)
}

fn file(lines: usize) -> String {
    (1..=lines).map(|n| format!("let x{n} = {n};\n")).collect()
}

#[test]
fn every_line_gets_its_number_right_aligned_in_the_gutter() {
    let document = CodeDocument::parse(Path::new("/r/a.rs"), &file(12));
    let ctx = egui::Context::default();
    let mut state = CodeState::default();
    frame(&ctx, &document, &mut state, vec![]);
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let number = |n: usize| {
        texts
            .iter()
            .find(|(t, _, _)| *t == n.to_string())
            .unwrap_or_else(|| panic!("line number {n} not painted: {texts:?}"))
            .1
    };
    for n in 1..=12 {
        assert!(
            (number(n).right() - number(12).right()).abs() < 0.5,
            "line {n}"
        );
    }
    for n in 1..12 {
        assert!(
            number(n).top() < number(n + 1).top(),
            "line {n} is not above {}",
            n + 1
        );
    }
    // The text of each line sits to the right of the numbers, in the theme's colours.
    let line = texts
        .iter()
        .find(|(t, _, _)| t == "let x1 = 1;")
        .expect("line 1 painted");
    assert!(line.1.left() > number(12).right());
}

#[test]
fn colours_follow_the_tokens() {
    let document =
        CodeDocument::parse(Path::new("/r/a.rs"), "fn main() { \"text\"; 42 } // note\n");
    let colours = palette();
    let font = FontId::monospace(13.0);
    let laid = job(&document.lines[0], &font, &colours);
    let colour_of = |piece: &str| {
        laid.sections
            .iter()
            .find(|s| laid.text[s.byte_range.start.0..s.byte_range.end.0].trim() == piece)
            .unwrap_or_else(|| panic!("{piece} not a section: {:?}", laid.text))
            .format
            .color
    };
    assert_eq!(colour_of("fn"), colours.keyword);
    assert_eq!(colour_of("\"text\""), colours.string);
    assert_eq!(colour_of("42"), colours.number);
    assert_eq!(colour_of("// note"), colours.comment);
    let plain = CodeDocument::parse(Path::new("/r/a.txt"), "fn \"x\" 42 // y\n");
    let plain = job(&plain.lines[0], &font, &colours);
    assert!(
        plain
            .sections
            .iter()
            .all(|s| s.format.color == colours.text)
    );
}

fn press_drag_release(from: egui::Pos2, to: egui::Pos2) -> Vec<Vec<egui::Event>> {
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![
        vec![egui::Event::PointerMoved(from)],
        vec![button(from, true)],
        vec![egui::Event::PointerMoved(from + egui::vec2(4.0, 0.0))],
        vec![egui::Event::PointerMoved(to)],
        vec![button(to, false)],
    ]
}

/// Marking across lines with the mouse and copying gives the file's text: never a line number.
#[test]
fn a_dragged_selection_copies_the_text_without_line_numbers() {
    let document = CodeDocument::parse(Path::new("/r/a.rs"), &file(12));
    let ctx = egui::Context::default();
    let mut state = CodeState::default();
    frame(&ctx, &document, &mut state, vec![]);
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let rect = |text: &str| texts.iter().find(|(t, _, _)| t == text).unwrap().1;
    let (two, four) = (rect("let x2 = 2;"), rect("let x4 = 4;"));
    for events in press_drag_release(
        two.left_center(),
        four.right_center() + egui::vec2(-1.0, 0.0),
    ) {
        frame(&ctx, &document, &mut state, events);
    }
    let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
    assert_eq!(
        copied,
        vec!["let x2 = 2;\nlet x3 = 3;\nlet x4 = 4;".to_owned()]
    );
}

/// The whole file copies, though only a screenful is laid out in any frame.
#[test]
fn select_all_copies_more_than_a_screenful() {
    let text = file(400);
    let document = CodeDocument::parse(Path::new("/r/a.rs"), &text);
    let ctx = egui::Context::default();
    let mut state = CodeState::default();
    frame(&ctx, &document, &mut state, vec![]);
    let select_all = egui::Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    let (painted, _) = frame(&ctx, &document, &mut state, vec![select_all]);
    assert!(
        !painted.iter().any(|(t, _, _)| t == "let x400 = 400;"),
        "the fixture must not fit on one screen"
    );
    let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
    assert_eq!(copied, vec![text.trim_end().to_owned()]);
}

#[test]
fn a_click_clears_the_selection_and_nothing_is_copied() {
    let document = CodeDocument::parse(Path::new("/r/a.rs"), &file(3));
    let ctx = egui::Context::default();
    let mut state = CodeState {
        selection: Some((Place::default(), document.end())),
    };
    frame(&ctx, &document, &mut state, vec![]);
    let at = egui::pos2(200.0, 20.0);
    for events in [
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        }],
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    ] {
        frame(&ctx, &document, &mut state, events);
    }
    let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
    assert!(copied.is_empty(), "{copied:?}");
}
