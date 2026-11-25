use super::*;

fn pointer_button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn a_stationary_edge_drag_scrolls_and_copies_beyond_the_initial_viewport() {
    for wrap in [false, true] {
        let document = CodeDocument::parse(Path::new("selection.sql"), &file(200));
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut state = CodeState {
            wrap,
            ..Default::default()
        };
        frame(&ctx, &document, &mut state, vec![]);
        let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
        let start = texts
            .iter()
            .find(|(s, _, _)| s == "let x2 = 2;")
            .unwrap()
            .1
            .left_center();
        let down = egui::pos2(start.x + 50.0, 330.0);
        for events in [
            vec![egui::Event::PointerMoved(start)],
            vec![pointer_button(start, true)],
            vec![egui::Event::PointerMoved(start + egui::vec2(8.0, 0.0))],
            vec![egui::Event::PointerMoved(down)],
        ] {
            frame(&ctx, &document, &mut state, events);
        }
        for _ in 0..120 {
            frame(&ctx, &document, &mut state, vec![]);
        }
        let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
        assert!(
            !texts.iter().any(|(s, _, _)| s == "let x2 = 2;"),
            "viewport did not advance, wrap={wrap}"
        );
        frame(
            &ctx,
            &document,
            &mut state,
            vec![pointer_button(down, false)],
        );
        let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
        let copied = &copied[0];
        assert!(
            copied.starts_with("let x2 = 2;\n"),
            "anchor moved: {copied}"
        );
        assert!(
            copied.lines().count() > 30,
            "copied only {} lines",
            copied.lines().count()
        );

        // Start a fresh selection in the now-scrolled document and hold above its top edge.
        let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
        let start = texts
            .iter()
            .find(|(s, rect, _)| {
                s.starts_with("let x") && rect.center().y > 60.0 && rect.center().y < 250.0
            })
            .unwrap()
            .1
            .left_center();
        let up = egui::pos2(start.x + 30.0, -30.0);
        for events in [
            vec![egui::Event::PointerMoved(start)],
            vec![pointer_button(start, true)],
            vec![egui::Event::PointerMoved(up)],
        ] {
            frame(&ctx, &document, &mut state, events);
        }
        for _ in 0..180 {
            frame(&ctx, &document, &mut state, vec![]);
        }
        let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
        assert!(
            texts.iter().any(|(s, _, _)| s == "let x1 = 1;"),
            "upward drag never returned to the top"
        );
        frame(&ctx, &document, &mut state, vec![pointer_button(up, false)]);
        let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
        assert!(copied[0].starts_with("let x1 = 1;"));
    }
}

fn navigation_output(
    ctx: &egui::Context,
    document: &CodeDocument,
    state: &mut CodeState,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600.0, 300.0),
            )),
            events,
            ..Default::default()
        },
        |ui| show(document, state, ui, &palette()),
    )
}

#[test]
fn overflow_scrollbars_stay_painted_at_rest_and_drag_the_document() {
    let text = (0..200)
        .map(|n| format!("row{n} {}\n", "wide ".repeat(100)))
        .collect::<String>();
    let document = CodeDocument::parse(Path::new("scroll.json"), &text);
    for wrap in [false, true] {
        let ctx = egui::Context::default();
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        let mut state = CodeState {
            wrap,
            ..Default::default()
        };
        for _ in 0..6 {
            navigation_output(&ctx, &document, &mut state, vec![]).drop_without_applying_deltas();
        }
        let out = navigation_output(&ctx, &document, &mut state, vec![]);
        let bars: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Rect(r)
                    if r.fill.a() > 0
                        && r.rect.width() <= 12.0
                        && r.rect.height() >= 12.0
                        && r.rect.height() < 290.0
                        && r.rect.left() > 580.0 =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();
        let vertical = *bars
            .first()
            .expect("a visible vertical scrollbar handle at rest");
        let horizontal = out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill.a() > 0 && r.rect.height() <= 12.0 && r.rect.width() >= 12.0 && r.rect.width() < 580.0 && r.rect.top() > 280.0));
        assert_eq!(horizontal, !wrap, "horizontal overflow follows wrapping");
        out.drop_without_applying_deltas();
        let from = vertical.center();
        let to = egui::pos2(from.x, 220.0);
        for events in press_drag_release(from, to) {
            navigation_output(&ctx, &document, &mut state, events).drop_without_applying_deltas();
        }
        let out = navigation_output(&ctx, &document, &mut state, vec![]);
        assert!(
            !out.shapes.iter().any(
                |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().starts_with("row0 "))
            ),
            "dragging the visible handle must move the document"
        );
        out.drop_without_applying_deltas();
    }
}

#[test]
fn held_edge_drag_requests_frames_until_release_or_document_boundary() {
    let document = CodeDocument::parse(Path::new("repaint.txt"), &file(200));
    let ctx = egui::Context::default();
    ctx.global_style_mut(|style| style.animation_time = 0.0);
    let mut state = CodeState::default();
    for _ in 0..6 {
        frame(&ctx, &document, &mut state, vec![]);
    }
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let start = texts
        .iter()
        .find(|(s, _, _)| s == "let x2 = 2;")
        .unwrap()
        .1
        .left_center();
    let down = egui::pos2(start.x, 380.0);
    let delay = |state: &mut CodeState, events| {
        let output = navigation_output(&ctx, &document, state, events);
        let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        output.drop_without_applying_deltas();
        delay
    };
    for events in [
        vec![egui::Event::PointerMoved(start)],
        vec![pointer_button(start, true)],
        vec![egui::Event::PointerMoved(down)],
    ] {
        delay(&mut state, events);
    }
    // Drain input-triggered frames: only the stationary drag can keep these repainting.
    for _ in 0..8 {
        assert_eq!(delay(&mut state, vec![]), std::time::Duration::ZERO);
    }
    delay(&mut state, vec![pointer_button(down, false)]);
    for _ in 0..8 {
        delay(&mut state, vec![]);
    }
    assert_eq!(
        delay(&mut state, vec![]),
        std::time::Duration::MAX,
        "released selection keeps waking an idle window"
    );

    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let start = texts
        .iter()
        .find(|(s, rect, _)| s.starts_with("let x") && (60.0..240.0).contains(&rect.center().y))
        .unwrap()
        .1
        .left_center();
    let up = egui::pos2(start.x, -100.0);
    for events in [
        vec![egui::Event::PointerMoved(start)],
        vec![pointer_button(start, true)],
        vec![egui::Event::PointerMoved(up)],
    ] {
        delay(&mut state, events);
    }
    for _ in 0..240 {
        delay(&mut state, vec![]);
    }
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    assert!(
        texts.iter().any(|(s, _, _)| s == "let x1 = 1;"),
        "held drag must actually reach the upper boundary"
    );
    assert_eq!(
        delay(&mut state, vec![]),
        std::time::Duration::MAX,
        "holding at the document boundary keeps waking an idle window"
    );
    delay(&mut state, vec![pointer_button(up, false)]);
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let start = texts
        .iter()
        .find(|(s, _, _)| s == "let x2 = 2;")
        .unwrap()
        .1
        .left_center();
    for events in [
        vec![egui::Event::PointerMoved(start)],
        vec![pointer_button(start, true)],
        vec![egui::Event::PointerMoved(down)],
    ] {
        delay(&mut state, events);
    }
    for _ in 0..400 {
        delay(&mut state, vec![]);
    }
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    assert!(
        texts.iter().any(|(s, _, _)| s == "let x200 = 200;"),
        "held drag must reach the lower boundary"
    );
    assert_eq!(
        delay(&mut state, vec![]),
        std::time::Duration::MAX,
        "holding below the last line keeps waking an idle window"
    );
}

#[test]
fn edge_drag_through_multirow_lines_keeps_the_original_copy_anchor() {
    let lines: Vec<_> = (0..80)
        .map(|line| {
            format!(
                "line{line:02} {}",
                (0..90)
                    .map(|word| format!("w{word:03}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect();
    let document = CodeDocument::parse(Path::new("wrapped.txt"), &lines.join("\n"));
    let ctx = egui::Context::default();
    ctx.global_style_mut(|style| style.animation_time = 0.0);
    let mut state = CodeState {
        wrap: true,
        ..Default::default()
    };
    for _ in 0..8 {
        frame(&ctx, &document, &mut state, vec![]);
    }
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let line = texts.iter().find(|(s, _, _)| s == &lines[0]).unwrap().1;
    let row_height = texts.iter().find(|(s, _, _)| s == "1").unwrap().1.height();
    assert!(
        line.height() > row_height * 3.0,
        "fixture must wrap into multiple visual rows"
    );
    let start = egui::pos2(line.left(), line.top() + row_height / 2.0);
    let down = egui::pos2(start.x + 40.0, 380.0);
    for events in [
        vec![egui::Event::PointerMoved(start)],
        vec![pointer_button(start, true)],
        vec![egui::Event::PointerMoved(down)],
    ] {
        frame(&ctx, &document, &mut state, events);
    }
    for _ in 0..160 {
        frame(&ctx, &document, &mut state, vec![]);
    }
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    assert!(
        !texts.iter().any(|(s, _, _)| s == &lines[0]),
        "viewport must leave the anchored source line"
    );
    frame(
        &ctx,
        &document,
        &mut state,
        vec![pointer_button(down, false)],
    );
    let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
    let copied = copied.first().expect("drag copied document text");
    assert!(
        copied.starts_with(&format!("{}\n{}\n", lines[0], lines[1])),
        "wrapped rows must copy as whole source lines with the original anchor"
    );
    assert!(
        lines.join("\n").starts_with(copied),
        "copy must be the exact document prefix, without visual-row breaks or gutter numbers"
    );
    assert!(
        copied.lines().count() > 8,
        "drag did not traverse enough wrapped source lines"
    );
}

#[test]
fn the_horizontal_scrollbar_handle_moves_the_painted_text_sideways() {
    let text = format!("start {} end", "wide ".repeat(100));
    let document = CodeDocument::parse(Path::new("horizontal.txt"), &text);
    let ctx = egui::Context::default();
    ctx.global_style_mut(|style| style.animation_time = 0.0);
    let mut state = CodeState::default();
    for _ in 0..8 {
        frame(&ctx, &document, &mut state, vec![]);
    }
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let before = texts.iter().find(|(s, _, _)| s == &text).unwrap().1.left();
    let output = navigation_output(&ctx, &document, &mut state, vec![]);
    let handle = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Rect(r)
                if r.fill.a() > 0
                    && r.rect.height() <= 12.0
                    && r.rect.width() >= 12.0
                    && r.rect.width() < 580.0
                    && r.rect.top() > 280.0 =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("horizontal overflow has a painted handle");
    output.drop_without_applying_deltas();
    for events in press_drag_release(handle.center(), handle.center() + egui::vec2(180.0, 0.0)) {
        frame(&ctx, &document, &mut state, events);
    }
    let (texts, _) = frame(&ctx, &document, &mut state, vec![]);
    let after = texts.iter().find(|(s, _, _)| s == &text).unwrap().1.left();
    assert!(
        after < before - 100.0,
        "dragging horizontal handle must translate the painted document: {before} -> {after}"
    );
    let (_, copied) = frame(&ctx, &document, &mut state, vec![egui::Event::Copy]);
    assert!(copied.is_empty(), "scrollbar drag must not select text");
}
