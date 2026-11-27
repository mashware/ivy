use eframe::egui;

pub(super) const HINT_HEIGHT: f32 = 26.0;
pub(crate) const HINT: &str = "Enter go to console  ·  Arrows move between cards  ·  Esc close";

/// The keys, named once at the foot of the Board instead of in every card.
fn movement_hint(keymap: &crate::keyboard::Keymap) -> String {
    use crate::keyboard::{Action, Chord};
    let directions = [
        (Action::BoardLeft, egui::Key::ArrowLeft, "left"),
        (Action::BoardRight, egui::Key::ArrowRight, "right"),
        (Action::BoardUp, egui::Key::ArrowUp, "up"),
        (Action::BoardDown, egui::Key::ArrowDown, "down"),
    ];
    if directions.iter().all(|(action, key, _)| {
        keymap.chord_for(*action)
            == Some(Chord {
                ctrl: false,
                shift: false,
                alt: false,
                key: *key,
            })
    }) {
        return HINT.to_owned();
    }
    let bindings = directions
        .iter()
        .filter_map(|(action, _, direction)| {
            keymap.chord_for(*action).map(|chord| {
                let mut name = String::new();
                if chord.ctrl {
                    name.push_str("Ctrl+");
                }
                if chord.alt {
                    name.push_str("Alt+");
                }
                if chord.shift {
                    name.push_str("Shift+");
                }
                name.push_str(chord.key.name());
                format!("{name} {direction}")
            })
        })
        .collect::<Vec<_>>()
        .join(", ");
    if bindings.is_empty() {
        "Enter go to console  ·  Esc close".to_owned()
    } else {
        format!("Enter go to console  ·  {bindings}  ·  Esc close")
    }
}

fn hint_label(full: &str, full_width: f32, room: f32) -> (&str, bool) {
    if full_width <= room {
        (full, false)
    } else {
        ("Keys (hover)", true)
    }
}

pub(super) fn paint_hint(
    ui: &mut egui::Ui,
    refused: Option<&str>,
    roles: &crate::theme::Roles,
    keymap: &crate::keyboard::Keymap,
) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), HINT_HEIGHT),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 0.0, roles.chrome);
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, roles.line),
    );
    // A refusal takes the hint's place: the two on one line overlap in a narrow window.
    let (text, ink) = match refused {
        Some(reason) => (
            format!("This arrangement will not be remembered: {reason}"),
            roles.wait,
        ),
        None => (movement_hint(keymap), roles.tx3),
    };
    let font = egui::FontId::proportional(11.5);
    let width = ui
        .painter()
        .layout_no_wrap(text.clone(), font.clone(), ink)
        .size()
        .x;
    let (label, clipped) = hint_label(&text, width, (rect.width() - 32.0).max(0.0));
    if clipped {
        response.on_hover_text(&text);
    }
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .text(
            egui::pos2(rect.left() + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            font,
            ink,
        );
}

#[cfg(test)]
mod hint_tests {
    use super::*;
    use crate::keyboard::{Action, Chord, Keymap};

    #[test]
    fn movement_hint_follows_rebound_and_disabled_directions() {
        let mut map = Keymap::default();
        assert_eq!(movement_hint(&map), HINT);
        map.bind(
            Action::BoardRight,
            Some(Chord {
                ctrl: false,
                alt: false,
                shift: false,
                key: egui::Key::L,
            }),
        );
        map.bind(Action::BoardLeft, None);
        let hint = movement_hint(&map);
        assert!(hint.contains("L right"));
        assert!(!hint.contains("Arrows"));
        assert!(!hint.contains("left"));
        for action in [Action::BoardRight, Action::BoardUp, Action::BoardDown] {
            map.bind(action, None);
        }
        assert_eq!(movement_hint(&map), "Enter go to console  ·  Esc close");
    }

    #[test]
    fn clipped_hint_uses_a_discoverable_compact_label() {
        let full = "Enter go to console · Ctrl+Shift+Right right · Esc close";
        assert_eq!(hint_label(full, 400.0, 100.0), ("Keys (hover)", true));
        assert_eq!(hint_label(full, 400.0, 400.0), (full, false));
    }
}
