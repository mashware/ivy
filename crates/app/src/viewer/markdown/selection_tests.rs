use super::*;
use egui::{Event, OutputCommand, Pos2, Rect};

struct Probe {
    document: MarkdownDocument,
    context: egui::Context,
    keys: std::cell::Cell<bool>,
}

impl Probe {
    fn new() -> Self {
        let p = Self {
            document: MarkdownDocument::parse(
                &(0..160)
                    .map(|i| format!("Witness {i:03} alpha bravo charlie\n\n"))
                    .collect::<String>(),
            )
            .unwrap(),
            context: egui::Context::default(),
            keys: std::cell::Cell::new(true),
        };
        for _ in 0..5 {
            p.frame(vec![]);
        }
        p
    }

    fn frame(&self, events: Vec<Event>) -> (Vec<(String, Rect)>, Vec<String>) {
        fn visit(shape: &egui::Shape, clip: Rect, texts: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => {
                    let rect = t.galley.rect.translate(t.pos.to_vec2());
                    if clip.intersects(rect) {
                        texts.push((t.galley.text().into(), rect));
                    }
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        visit(shape, clip, texts);
                    }
                }
                _ => {}
            }
        }
        let out = self.output(events);
        let mut texts = Vec::new();
        for s in &out.shapes {
            visit(&s.shape, s.clip_rect, &mut texts);
        }
        let copied = out
            .platform_output
            .commands
            .iter()
            .filter_map(|c| {
                if let OutputCommand::CopyText(text) = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect();
        out.drop_without_applying_deltas();
        (texts, copied)
    }

    fn output(&self, events: Vec<Event>) -> egui::FullOutput {
        self.context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 300.0))),
                events,
                ..Default::default()
            },
            |ui| self.document.show(ui, self.keys.get()),
        )
    }

    fn scrollbar_thumb(&self) -> Pos2 {
        let out = self.output(vec![]);
        let thumb = out.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.rect.left() > 580.0
                    && rect.rect.height() > 5.0
                    && rect.rect.height() < 100.0
                    && rect.fill.a() > 0 =>
            {
                Some(rect.rect.center())
            }
            _ => None,
        });
        out.drop_without_applying_deltas();
        thumb.expect("the overflowing document paints a scrollbar thumb")
    }

    fn select(&self) -> Pos2 {
        let (texts, _) = self.frame(vec![]);
        let rect = texts
            .iter()
            .find(|(s, _)| s.starts_with("Witness 001"))
            .unwrap()
            .1;
        let from = rect.left_center() + egui::vec2(2., 0.);
        let to = rect.right_center() - egui::vec2(2., 0.);
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        self.frame(vec![Event::PointerMoved(to)]);
        to
    }

    fn assert_copy_extends_beyond_start(&self, to: Pos2) {
        self.frame(vec![button(to, false)]);
        let copied = self.frame(vec![Event::Copy]).1;
        assert_eq!(copied.len(), 1);
        assert!(copied[0].starts_with("Witness 001 alpha bravo charlie\n\n"));
        assert!(
            copied[0].split("\n\n").count() > 8,
            "selection did not extend"
        );
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: egui::Modifiers::NONE,
    }
}

fn wheel() -> Event {
    Event::MouseWheel {
        delta: egui::vec2(0., -600.),
        unit: egui::MouseWheelUnit::Point,
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn selection_scroll_markdown_edge_drag_advances() {
    let p = Probe::new();
    let to = p.select();
    let below = Pos2::new(to.x, 325.0);
    p.frame(vec![Event::PointerMoved(below)]);
    for _ in 0..60 {
        p.frame(vec![]);
    }
    assert!(
        !p.frame(vec![])
            .0
            .iter()
            .any(|(s, _)| s.starts_with("Witness 001")),
        "holding a selection past the bottom did not scroll"
    );
    p.assert_copy_extends_beyond_start(below);
}

#[test]
fn selection_scroll_markdown_wheel_during_selection_advances() {
    let p = Probe::new();
    let to = p.select();
    for _ in 0..12 {
        p.frame(vec![wheel()]);
    }
    assert!(
        !p.frame(vec![])
            .0
            .iter()
            .any(|(s, _)| s.starts_with("Witness 001")),
        "the wheel did not scroll while the selection button was held"
    );
    p.assert_copy_extends_beyond_start(to);
}

#[test]
fn selection_scroll_markdown_copy_survives_wheeling_after_release() {
    let p = Probe::new();
    let to = p.select();
    p.frame(vec![button(to, false)]);
    let initial = p.frame(vec![Event::Copy]).1;
    assert!(
        initial.iter().any(|s| s.contains("Witness 001 alpha")),
        "selection setup: {initial:?}"
    );
    for _ in 0..12 {
        p.frame(vec![wheel()]);
    }
    assert!(
        !p.frame(vec![])
            .0
            .iter()
            .any(|(s, _)| s.starts_with("Witness 001")),
        "the viewport did not move"
    );
    let after = p.frame(vec![Event::Copy]).1;
    assert_eq!(initial, after, "scrolling cleared or changed the selection");
}

#[test]
fn selection_scroll_markdown_copy_survives_scrollbar_thumb_drag_after_release() {
    let p = Probe::new();
    p.context
        .global_style_mut(|style| style.spacing.scroll = egui::style::ScrollStyle::solid());
    for _ in 0..5 {
        p.frame(vec![]);
    }
    let to = p.select();
    p.frame(vec![button(to, false)]);
    let initial = p.frame(vec![Event::Copy]).1;
    assert!(
        initial.iter().any(|s| s.contains("Witness 001 alpha")),
        "selection setup: {initial:?}"
    );
    let thumb = p.scrollbar_thumb();
    let down = Pos2::new(thumb.x, 225.0);
    p.frame(vec![Event::PointerMoved(thumb)]);
    p.frame(vec![button(thumb, true)]);
    p.frame(vec![Event::PointerMoved(down)]);
    p.frame(vec![button(down, false)]);
    for _ in 0..30 {
        p.frame(vec![]);
    }
    assert!(
        !p.frame(vec![])
            .0
            .iter()
            .any(|(s, _)| s.starts_with("Witness 001")),
        "the scrollbar did not move the viewport"
    );
    assert_eq!(
        initial,
        p.frame(vec![Event::Copy]).1,
        "scrollbar cleared or changed the selection"
    );
}

#[test]
fn selection_scroll_markdown_double_click_still_selects_a_word() {
    let p = Probe::new();
    let out = p.output(vec![]);
    let word = out
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().starts_with("Witness 001") => Some(
                text.pos
                    + text
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(14))
                        .center()
                        .to_vec2(),
            ),
            _ => None,
        })
        .expect("paragraph appears in the viewport");
    out.drop_without_applying_deltas();
    p.frame(vec![Event::PointerMoved(word)]);
    for _ in 0..2 {
        p.frame(vec![button(word, true)]);
        p.frame(vec![button(word, false)]);
    }
    assert_eq!(p.frame(vec![Event::Copy]).1, ["alpha"]);
}

#[test]
fn selection_scroll_markdown_copy_requires_keyboard_ownership() {
    let p = Probe::new();
    let to = p.select();
    p.frame(vec![button(to, false)]);
    p.keys.set(false);
    assert!(p.frame(vec![Event::Copy]).1.is_empty());
    p.keys.set(true);
    assert_eq!(
        p.frame(vec![Event::Copy]).1,
        ["Witness 001 alpha bravo charlie"]
    );
}

#[test]
fn selection_scroll_markdown_touch_drag_pans_the_document() {
    let p = Probe::new();
    let from = Pos2::new(85.0, 220.0);
    let to = Pos2::new(85.0, 50.0);
    let touch = |phase, pos| Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(1),
        phase,
        pos,
        force: None,
    };
    p.frame(vec![touch(egui::TouchPhase::Start, from)]);
    p.frame(vec![
        touch(egui::TouchPhase::End, from),
        Event::PointerMoved(from),
    ]);
    p.frame(vec![
        touch(egui::TouchPhase::Start, from),
        button(from, true),
    ]);
    p.frame(vec![
        touch(egui::TouchPhase::Move, to),
        Event::PointerMoved(to),
    ]);
    p.frame(vec![touch(egui::TouchPhase::End, to), button(to, false)]);
    let (visible, copied) = p.frame(vec![Event::Copy]);
    assert!(
        !visible
            .iter()
            .any(|(text, _)| text.starts_with("Witness 001"))
    );
    assert!(copied.is_empty(), "a touch scroll must not select text");
}

#[test]
fn selection_scroll_markdown_keyboard_scrolls_a_multiline_block() {
    let mut p = Probe::new();
    p.document =
        MarkdownDocument::parse(&"Multiline text for keyboard selection.  \n".repeat(60)).unwrap();
    for _ in 0..5 {
        p.frame(vec![]);
    }
    let rect = p.frame(vec![]).0[0].1;
    let original_top = rect.top();
    let start = rect.left_top() + egui::vec2(12.0, 8.0);
    p.frame(vec![Event::PointerMoved(start)]);
    p.frame(vec![button(start, true)]);
    p.frame(vec![button(start, false)]);
    for _ in 0..40 {
        p.frame(vec![Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        }]);
    }
    let (visible, copied) = p.frame(vec![Event::Copy]);
    assert!(
        visible[0].1.top() < original_top - 30.0,
        "the multiline block did not scroll"
    );
    assert!(
        copied[0].lines().count() > 30,
        "the keyboard selection did not extend"
    );
}
