use eframe::egui::Rect;

#[cfg(test)]
mod interaction_tests;

#[derive(Clone, Copy)]
pub(super) enum Direction {
    Left,
    Right,
    Up,
    Down,
}

pub(super) fn neighbor(rects: &[Rect], current: usize, direction: Direction) -> Option<usize> {
    let origin = *rects.get(current)?;
    let mut best: Option<(usize, f32, f32, f32)> = None;
    for (index, rect) in rects.iter().copied().enumerate() {
        if index == current {
            continue;
        }
        let (gap, overlap, center) = match direction {
            Direction::Left => (
                origin.left() - rect.right(),
                origin.bottom().min(rect.bottom()) - origin.top().max(rect.top()),
                (origin.center().y - rect.center().y).abs(),
            ),
            Direction::Right => (
                rect.left() - origin.right(),
                origin.bottom().min(rect.bottom()) - origin.top().max(rect.top()),
                (origin.center().y - rect.center().y).abs(),
            ),
            Direction::Up => (
                origin.top() - rect.bottom(),
                origin.right().min(rect.right()) - origin.left().max(rect.left()),
                (origin.center().x - rect.center().x).abs(),
            ),
            Direction::Down => (
                rect.top() - origin.bottom(),
                origin.right().min(rect.right()) - origin.left().max(rect.left()),
                (origin.center().x - rect.center().x).abs(),
            ),
        };
        if gap < 0.0 || overlap <= 0.0 {
            continue;
        }
        if best.is_none_or(|(_, g, o, c)| {
            gap < g || (gap == g && (overlap > o || (overlap == o && center < c)))
        }) {
            best = Some((index, gap, overlap, center));
        }
    }
    best.map(|(index, _, _, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Rect, pos2};
    #[test]
    fn picks_facing_edge_then_overlap_and_keeps_edges() {
        let r = |x1, y1, x2, y2| Rect::from_min_max(pos2(x1, y1), pos2(x2, y2));
        let parts = [
            r(0., 0., 100., 100.),
            r(102., 0., 200., 40.),
            r(102., 42., 200., 100.),
            r(202., 0., 300., 100.),
        ];
        assert_eq!(neighbor(&parts, 0, Direction::Right), Some(2));
        assert_eq!(neighbor(&parts, 0, Direction::Left), None);
        assert_eq!(neighbor(&parts, 1, Direction::Down), Some(2));
        assert_eq!(neighbor(&parts, 2, Direction::Up), Some(1));
    }

    #[test]
    fn ties_choose_center_then_layout_order() {
        let r = |x1, y1, x2, y2| Rect::from_min_max(pos2(x1, y1), pos2(x2, y2));
        let parts = [
            r(0., 0., 100., 100.),
            r(102., -50., 200., 50.),
            r(102., 25., 200., 75.),
            r(102., 25., 200., 75.),
        ];
        assert_eq!(neighbor(&parts, 0, Direction::Right), Some(2));
    }

    #[test]
    fn arrow_cannot_claim_following_text_as_its_own() {
        let modifiers = eframe::egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        let key = |key| eframe::egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        assert!(!super::super::may_emit_text(
            &key(eframe::egui::Key::ArrowRight),
            modifiers
        ));
        assert!(super::super::may_emit_text(
            &key(eframe::egui::Key::L),
            modifiers
        ));
    }

    #[test]
    fn tab_count_is_refreshed_after_navigation_applies_earlier_requests() {
        use eframe::egui::{self, Event, Key, Modifiers};
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let original = window.active().name;
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let tab = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        let nav = Modifiers {
            alt: true,
            shift: true,
            ..Modifiers::NONE
        };
        let key = |key, modifiers| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_max(pos2(0., 0.), pos2(1200., 800.))),
            events: vec![
                Event::ModifiersChanged(tab),
                key(Key::T, tab),
                Event::ModifiersChanged(nav),
                key(Key::ArrowRight, nav),
                Event::ModifiersChanged(tab),
                key(Key::ArrowLeft, tab),
            ],
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            let requests = super::super::send_keys(&mut window, ui.ctx());
            assert_eq!(window.tabs.len(), 2);
            assert!(requests.is_empty());
            assert_eq!(window.active().name, original);
        })
        .drop_without_applying_deltas();
    }

    #[test]
    fn bytes_follow_each_boundary_including_a_tab_switch() {
        use eframe::egui::{self, Event, Key, Modifiers};
        use std::time::Duration;
        let sinks: Vec<_> = (0..4)
            .map(|_| panel_contract::PanelDir::create().unwrap())
            .collect();
        let paths: Vec<_> = sinks
            .iter()
            .map(|dir| dir.path().join("received"))
            .collect();
        let scripts: Vec<_> = sinks
            .iter()
            .map(|dir| {
                let folder = dir.path().display().to_string();
                #[cfg(unix)]
                let folder = folder.replace('\'', "'\\''");
                format!("cd '{folder}' && exec cat > 'received'")
            })
            .collect();
        let mut window = crate::tests::window_of_tabs([
            [scripts[0].as_str(), scripts[1].as_str()],
            [scripts[2].as_str(), scripts[3].as_str()],
        ]);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while paths.iter().any(|path| !path.exists()) {
            for pane in window.all_panes_mut() {
                pane.pump(Default::default());
            }
            assert!(
                std::time::Instant::now() < deadline,
                "receivers did not open: {paths:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let arrow = |key, modifiers| Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        };
        let nav = Modifiers {
            alt: true,
            shift: true,
            ..Modifiers::NONE
        };
        let tab = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        window.keymap.bind(
            crate::keyboard::Action::FocusLeft,
            Some(crate::keyboard::Chord {
                ctrl: false,
                shift: true,
                alt: true,
                key: Key::Comma,
            }),
        );
        let events = vec![
            Event::Text("a".into()),
            arrow(Key::Enter, Modifiers::NONE),
            Event::ModifiersChanged(nav),
            arrow(Key::ArrowRight, nav),
            Event::ModifiersChanged(Modifiers::NONE),
            Event::Text("b".into()),
            arrow(Key::Enter, Modifiers::NONE),
            Event::ModifiersChanged(tab),
            arrow(Key::ArrowRight, tab),
            Event::ModifiersChanged(nav),
            arrow(Key::ArrowRight, nav),
            arrow(Key::Comma, nav),
            Event::Text("<".into()),
            Event::ModifiersChanged(Modifiers::NONE),
            Event::Text("c".into()),
            arrow(Key::Enter, Modifiers::NONE),
            Event::ModifiersChanged(nav),
            arrow(Key::ArrowRight, nav),
            Event::ModifiersChanged(Modifiers::NONE),
            Event::Text("d".into()),
            arrow(Key::Enter, Modifiers::NONE),
        ];
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_max(pos2(0., 0.), pos2(1200., 800.))),
            events,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            let _ = super::super::send_keys(&mut window, ui.ctx());
        })
        .drop_without_applying_deltas();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        for (path, expected) in paths.iter().zip([b"a\n", b"b\n", b"c\n", b"d\n"]) {
            loop {
                let got = std::fs::read(path).unwrap_or_default();
                if got == expected {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "{path:?}: expected {expected:?}, got {got:?}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
