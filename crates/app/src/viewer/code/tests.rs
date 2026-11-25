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
    frame_of(ctx, document, state, events, egui::vec2(600.0, 300.0))
}

fn frame_of(
    ctx: &egui::Context,
    document: &CodeDocument,
    state: &mut CodeState,
    events: Vec<egui::Event>,
    size: egui::Vec2,
) -> (Vec<(String, egui::Rect, egui::Color32)>, Vec<String>) {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
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
        ..CodeState::default()
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

/// Every shape the view painted over `document`, after one settling pass.
fn shapes(
    document: &CodeDocument,
    state: &mut CodeState,
    events: Vec<egui::Event>,
) -> Vec<egui::Shape> {
    let ctx = egui::Context::default();
    frame(&ctx, document, state, vec![]);
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0));
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(area),
            events,
            ..Default::default()
        },
        |ui| show(document, state, ui, &palette()),
    );
    let shapes = output.shapes.iter().map(|c| c.shape.clone()).collect();
    output.drop_without_applying_deltas();
    shapes
}

/// What the view paints is coloured by token: read off the painted text, not the layout helper.
#[test]
fn the_painted_line_carries_the_token_colours() {
    let document = CodeDocument::parse(Path::new("/r/a.rs"), "fn main() { 42 }\n");
    let colours = palette();
    let painted = shapes(&document, &mut CodeState::default(), vec![]);
    let colour_of = |piece: &str| {
        painted.iter().find_map(|shape| match shape {
            egui::Shape::Text(text) if text.galley.text().starts_with("fn") => {
                let job = &text.galley.job;
                job.sections
                    .iter()
                    .find(|s| job.text[s.byte_range.start.0..s.byte_range.end.0].trim() == piece)
                    .map(|s| s.format.color)
            }
            _ => None,
        })
    };
    assert_eq!(colour_of("fn"), Some(colours.keyword));
    assert_eq!(colour_of("42"), Some(colours.number));
}

/// A marked stretch is painted behind its text in the selection colour.
#[test]
fn a_selection_is_painted_behind_the_text() {
    let document = CodeDocument::parse(Path::new("/r/a.rs"), &file(3));
    let colours = palette();
    let mut state = CodeState::default();
    let filled = |shapes: &[egui::Shape]| {
        shapes
            .iter()
            .any(|shape| matches!(shape, egui::Shape::Rect(r) if r.fill == colours.selection))
    };
    assert!(!filled(&shapes(&document, &mut state, vec![])));
    state.select((0, 4), (1, 6));
    assert!(filled(&shapes(&document, &mut state, vec![])));
}

/// A drag that starts and ends inside lines copies from there to there, not whole lines.
#[test]
fn a_drag_from_mid_line_copies_only_what_it_crossed() {
    let document = CodeDocument::parse(Path::new("/r/a.rs"), &file(6));
    let ctx = egui::Context::default();
    let mut state = CodeState::default();
    frame(&ctx, &document, &mut state, vec![]);
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let rect = |text: &str| texts.iter().find(|(t, _, _)| t == text).unwrap().1;
    let (two, four) = (rect("let x2 = 2;"), rect("let x4 = 4;"));
    let cell = two.width() / "let x2 = 2;".chars().count() as f32;
    // From the fifth character of line 2 to the seventh of line 4.
    let from = egui::pos2(two.left() + cell * 4.0 + 1.0, two.center().y);
    let to = egui::pos2(four.left() + cell * 6.0 + 1.0, four.center().y);
    for events in press_drag_release(from, to) {
        frame(&ctx, &document, &mut state, events);
    }
    let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
    assert_eq!(copied, vec!["x2 = 2;\nlet x3 = 3;\nlet x4".to_owned()]);
}

/// A line too long to lay out is painted cut, but copying it gives the whole line.
#[test]
fn a_truncated_line_copies_whole() {
    let long = "x".repeat(highlight::LINE_LIMIT + 10);
    let document = CodeDocument::parse(Path::new("/r/a.txt"), &format!("{long}\nshort\n"));
    let ctx = egui::Context::default();
    let mut state = CodeState::default();
    state.select((0, 0), (1, 5));
    let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
    assert_eq!(copied, vec![format!("{long}\nshort")]);
}

/// A drag along one truncated line, past where its painting ends, takes the line whole; a drag
/// that stops inside the painting takes only what it crossed.
#[test]
fn a_drag_past_the_painted_end_of_a_truncated_line_copies_it_whole() {
    let long = "x".repeat(highlight::LINE_LIMIT + 10);
    let document = CodeDocument::parse(Path::new("/r/a.txt"), &format!("{long}\nshort\n"));
    // Wide enough to show the whole painted line and the mark after it.
    let size = egui::vec2(200_000.0, 300.0);
    let drag = |to_x: Option<f32>| {
        let ctx = egui::Context::default();
        let mut state = CodeState::default();
        frame_of(&ctx, &document, &mut state, vec![], size);
        let (texts, _) = frame_of(&ctx, &document, &mut state, vec![], size);
        let line = texts
            .iter()
            .find(|(t, _, _)| t.len() == highlight::LINE_LIMIT)
            .expect("the truncated line is painted")
            .1;
        let to = egui::pos2(to_x.unwrap_or(line.right() + 20.0), line.center().y);
        for events in press_drag_release(line.left_center(), to) {
            frame_of(&ctx, &document, &mut state, events, size);
        }
        let (_, copied) = frame_of(&ctx, &document, &mut state, vec![egui::Event::Copy], size);
        (copied, line)
    };
    let (copied, line) = drag(None);
    assert_eq!(copied, vec![long.clone()]);
    let cell = line.width() / highlight::LINE_LIMIT as f32;
    let (copied, _) = drag(Some(line.left() + cell * 3.0 + 1.0));
    assert_eq!(copied, vec!["xxx".to_owned()]);
}

/// Offsets that no longer fall on a character of the text are brought back to one, never sliced.
#[test]
fn a_place_inside_a_character_is_never_sliced() {
    let document = CodeDocument::parse(Path::new("/r/a.txt"), "añbc\n");
    let copied = document.text_between(Place { line: 0, byte: 0 }, Place { line: 0, byte: 2 });
    assert_eq!(copied, "a");
    assert_eq!(char_cursor("añbc", 2).index.0, 1);
}

/// How far the view can scroll sideways in one pass over `document`: asked to go further than
/// any line, the scroll area stops at the extent the view gave it.
fn scroll_width(ctx: &egui::Context, document: &CodeDocument, state: &mut CodeState) -> f32 {
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0));
    let mut width = 0.0;
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(area),
            ..Default::default()
        },
        |ui| {
            let id = ui.make_persistent_id(egui::IdSalt::new(("code-content", &document.path)));
            let mut scroll = egui::scroll_area::State::load(ui.ctx(), id).unwrap_or_default();
            scroll.offset.x = 1.0e7;
            scroll.store(ui.ctx(), id);
            show(document, state, ui, &palette());
            width = egui::scroll_area::State::load(ui.ctx(), id)
                .expect("the code view keeps a scroll area")
                .offset
                .x;
        },
    );
    output.drop_without_applying_deltas();
    width
}

/// A new version of the same file keeps the sideways extent painting measured, so a rewrite does
/// not pull the scroll back; another file starts from its own lines.
#[test]
fn a_refresh_of_the_same_file_keeps_its_width_and_another_file_does_not() {
    let wide = format!("{}\n", "w".repeat(300));
    let ctx = egui::Context::default();
    let mut state = CodeState::default();
    let first = CodeDocument::parse(Path::new("/r/a.txt"), &wide);
    scroll_width(&ctx, &first, &mut state);
    let before = scroll_width(&ctx, &first, &mut state);
    let rewritten = CodeDocument::parse(Path::new("/r/a.txt"), "short\n");
    state.reset();
    scroll_width(&ctx, &rewritten, &mut state);
    let kept = scroll_width(&ctx, &rewritten, &mut state);
    assert!((kept - before).abs() < 1.0, "{kept} after {before}");
    let other = CodeDocument::parse(Path::new("/r/b.txt"), "short\n");
    state.reset();
    scroll_width(&ctx, &other, &mut state);
    let fresh = scroll_width(&ctx, &other, &mut state);
    assert!(fresh < before - 100.0, "{fresh} after {before}");
}

/// Wrap lines is off until asked for; on, a long line folds inside the 600-point view and the
/// lines after it move down to make room; off again, it is one row that scrolls sideways.
#[test]
fn wrap_lines_folds_long_lines_to_the_view_and_off_restores_sideways_scrolling() {
    // Words of fifty letters: one per row, so the rows are more than the length alone says.
    let long = format!("{} ", "x".repeat(50)).repeat(12);
    let text = format!("{}\nafter\n", long.trim_end());
    let document = CodeDocument::parse(Path::new("/r/a.txt"), &text);
    let ctx = egui::Context::default();
    let mut state = CodeState::default();
    let settled = |state: &mut CodeState| {
        frame(&ctx, &document, state, vec![]);
        let (texts, _) = frame(&ctx, &document, state, vec![]);
        let rect = |wanted: &str| {
            texts
                .iter()
                .find(|(t, _, _)| t == wanted)
                .unwrap_or_else(|| panic!("{wanted:?} not painted: {texts:?}"))
                .1
        };
        (rect(long.trim_end()), rect("after"))
    };
    let (line, after) = settled(&mut state);
    assert!(line.right() > 600.0 && after.height() >= line.height() - 0.5);
    assert!(scroll_width(&ctx, &document, &mut state) > 0.0);

    state.wrap = true;
    let (line, after) = settled(&mut state);
    assert!(line.right() <= 600.0, "{line:?}");
    assert!(line.height() > 3.0 * after.height(), "{line:?}");
    assert!(
        after.top() >= line.bottom() - 0.5,
        "{after:?} overlaps {line:?}"
    );

    // Ticked after scrolling right, nothing stays scrolled: the folded text starts at the gutter.
    assert_eq!(scroll_width(&ctx, &document, &mut state), 0.0);
    let (scrolled, _) = settled(&mut state);
    assert_eq!(scrolled.left(), line.left());

    state.wrap = false;
    let (line, after) = settled(&mut state);
    assert!(line.right() > 600.0 && line.height() < 2.0 * after.height());
    assert!(after.top() < line.bottom() + after.height());
    assert!(scroll_width(&ctx, &document, &mut state) > 0.0);
}

/// Marking still works when a line is folded: a drag from the start of a wrapped line to its
/// second row copies what it crossed, and the marked rows are painted on both.
#[test]
fn a_drag_across_a_wrapped_line_copies_what_it_crossed() {
    let long = (0..120)
        .map(|n| format!("w{n:03}"))
        .collect::<Vec<_>>()
        .join(" ");
    let document = CodeDocument::parse(Path::new("/r/a.txt"), &format!("{long}\n"));
    let ctx = egui::Context::default();
    let mut state = CodeState {
        wrap: true,
        ..CodeState::default()
    };
    frame(&ctx, &document, &mut state, vec![]);
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let line = texts.iter().find(|(t, _, _)| *t == long).unwrap().1;
    // The line number is one row tall.
    let row = texts.iter().find(|(t, _, _)| t == "1").unwrap().1.height();
    assert!(line.height() > 2.0 * row, "{line:?} is not wrapped");
    // From the start of the first row to a little way into the second.
    let from = egui::pos2(line.left() + 1.0, line.top() + row * 0.5);
    let to = egui::pos2(line.left() + 12.0, line.top() + row * 1.5);
    for events in press_drag_release(from, to) {
        frame(&ctx, &document, &mut state, events);
    }
    let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
    let copied = copied.first().cloned().unwrap_or_default();
    assert!(copied.starts_with("w000 w001"), "{copied:?}");
    assert!(copied.len() < long.len() / 2, "{copied:?}");
    let marked = shapes(&document, &mut state, vec![])
        .iter()
        .filter(|shape| matches!(shape, egui::Shape::Rect(r) if r.fill == palette().selection))
        .count();
    assert!(marked >= 2, "{marked} marked rows");
}

/// Before any line is placed, the places answer instead of panicking: everything is at the top.
#[test]
fn heights_answer_before_anything_is_placed() {
    let heights = rows::Heights::default();
    assert_eq!(heights.top(0), 0.0);
    assert_eq!(heights.top(7), 0.0);
    assert_eq!(heights.line_at(120.0), 0);
    assert_eq!(heights.total(), 0.0);
}

mod navigation;
