use crate::keyboard::{Action, Chord};
use eframe::egui::{self, Event, Key, Modifiers, Rect, pos2};

fn chord(key: Key, modifiers: Modifiers, repeat: bool) -> Vec<Event> {
    vec![
        Event::ModifiersChanged(modifiers),
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat,
            modifiers,
        },
        Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        },
        Event::ModifiersChanged(Modifiers::NONE),
    ]
}

fn frame(window: &mut crate::Window, ctx: &egui::Context, events: Vec<Event>) {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_max(pos2(0., 0.), pos2(1200., 800.))),
            events,
            ..Default::default()
        },
        |ui| {
            super::super::send_keys(window, ui.ctx());
        },
    )
    .drop_without_applying_deltas();
}

fn modifiers(ctrl: bool, alt: bool) -> Modifiers {
    Modifiers {
        ctrl,
        alt,
        shift: true,
        ..Modifiers::NONE
    }
}

#[test]
fn zoom_shortcut_restores_tree_ignores_repeat_and_locks_focus() {
    let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let tab = window.active().name;
    let initial = window.active().focus;
    let panes: Vec<_> = window.active().panes().iter().map(|p| p.name).collect();
    let area = Rect::from_min_max(pos2(0., 0.), pos2(1200., 800.));
    let before = window.active().root.place(area, 20.).parts;
    let mut held = chord(Key::M, modifiers(true, false), false);
    held.remove(2);
    frame(&mut window, &ctx, held);
    assert_eq!(window.pane_view.zoomed(tab), Some(initial));
    frame(
        &mut window,
        &ctx,
        chord(Key::M, modifiers(true, false), true),
    );
    frame(
        &mut window,
        &ctx,
        chord(Key::ArrowRight, modifiers(false, true), false),
    );
    assert_eq!(window.pane_view.zoomed(tab), Some(initial));
    assert_eq!(window.active().focus, initial);
    frame(
        &mut window,
        &ctx,
        chord(Key::M, modifiers(true, false), false),
    );
    assert_eq!(window.pane_view.zoomed(tab), None);
    assert_eq!(window.active().root.place(area, 20.).parts, before);
    assert_eq!(
        window
            .active()
            .panes()
            .iter()
            .map(|p| p.name)
            .collect::<Vec<_>>(),
        panes
    );
    frame(
        &mut window,
        &ctx,
        chord(Key::ArrowRight, modifiers(false, true), false),
    );
    assert_eq!(window.active().focus, panes[1]);
}

#[test]
fn rebound_and_disabled_navigation_on_three_real_panes() {
    let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_max(pos2(0., 0.), pos2(1200., 800.))),
            ..Default::default()
        },
        |ui| {
            let metrics = crate::painter::CellMetrics::measure(
                ui.ctx(),
                egui::FontId::monospace(window.font_size),
            );
            window.apply(
                crate::WindowRequest::Split(crate::Orientation::Columns),
                ui.ctx(),
                ui.max_rect(),
                &metrics,
            );
        },
    )
    .drop_without_applying_deltas();
    let panes: Vec<_> = window.active().panes().iter().map(|p| p.name).collect();
    assert_eq!(panes.len(), 3);
    window.active_mut().focus = panes[0];
    let custom = Chord {
        ctrl: false,
        alt: true,
        shift: true,
        key: Key::F12,
    };
    window.keymap.bind(Action::FocusRight, Some(custom));
    frame(
        &mut window,
        &ctx,
        chord(Key::ArrowRight, modifiers(false, true), false),
    );
    assert_eq!(window.active().focus, panes[0]);
    frame(
        &mut window,
        &ctx,
        chord(Key::F12, modifiers(false, true), false),
    );
    assert_eq!(window.active().focus, panes[1]);
    frame(
        &mut window,
        &ctx,
        chord(Key::F12, modifiers(false, true), false),
    );
    assert_eq!(window.active().focus, panes[2]);
    window.keymap.bind(Action::FocusRight, None);
    window.active_mut().focus = panes[0];
    frame(
        &mut window,
        &ctx,
        chord(Key::F12, modifiers(false, true), false),
    );
    assert_eq!(window.active().focus, panes[0]);
}

#[test]
fn maximize_with_one_pane_keeps_the_layout() {
    let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let closing = window.active().panes()[1].name;
    window.close_a_pane_called(closing, &ctx).unwrap();
    assert_eq!(window.active().panes().len(), 1);
    let tab = window.active().name;
    let focus = window.active().focus;
    frame(
        &mut window,
        &ctx,
        chord(Key::M, modifiers(true, false), false),
    );
    assert_eq!(window.pane_view.zoomed(tab), None);
    assert_eq!(window.active().focus, focus);
    assert_eq!(window.active().panes().len(), 1);
}
