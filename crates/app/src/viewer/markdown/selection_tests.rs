use super::*;
use egui::{Event, OutputCommand, Pos2, Rect};

struct Probe {
    document: MarkdownDocument,
    context: egui::Context,
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
            |ui| self.document.show(ui),
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
        assert!(copied[0].starts_with("Witness 001 alpha bravo charlie\n"));
        assert!(copied[0].lines().count() > 12, "selection did not extend");
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
