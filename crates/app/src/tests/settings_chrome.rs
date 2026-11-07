//! Real-frame witnesses for gear/tab isolation and shared pane geometry.
#[cfg(test)]
mod tests {
    use super::super::*;

    fn click(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    #[test]
    fn settings_gear_cannot_switch_tabs_or_deliver_the_opening_frames_input() {
        for tabs in [1, 2] {
            let mut window = window_of_tabs((0..tabs).map(|_| ["sleep 30", "sleep 30"]));
            window.settings_editor =
                settings::editor::Editor::resolved(settings::Entry::Measuring, Some("closed"));
            let ctx = egui::Context::default();
            let area = area_of(1280.0);
            one_frame_in(&mut window, &ctx, area, Vec::new());
            let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
            let geometry = crate::tab_bar::geometry(&window, area, &metrics);
            let gear = window.settings_editor.entry_rect();
            assert_eq!(Some(gear), geometry.gear);
            assert!(geometry.cards.unwrap().right() <= gear.left());
            let active = window.active;
            let mut input = click(gear.center());
            input.push(egui::Event::Text("opening must stay in Settings".into()));
            input.push(egui::Event::Paste("clipboard".into()));
            one_frame_in(&mut window, &ctx, area, input);
            assert!(window.settings_editor.is_open());
            assert_eq!(window.active, active, "gear switched tabs");
            assert_eq!(window.tabs.len(), tabs);
            assert!(
                window
                    .all_panes_mut()
                    .all(|pane| pane.keystrokes.is_empty())
            );
        }
    }

    #[test]
    fn tab_card_clicks_still_change_tabs_beside_the_gear() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        window.settings_editor =
            settings::editor::Editor::resolved(settings::Entry::Measuring, Some("closed"));
        let ctx = egui::Context::default();
        let area = area_of(1280.0);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let geometry = crate::tab_bar::geometry(&window, area, &metrics);
        let at = crate::tab_bar::tab_rect(geometry.cards.unwrap(), 1, 2).center();
        one_frame_in(&mut window, &ctx, area, click(at));
        assert_eq!(window.active, 1);
        assert!(!window.settings_editor.is_open());
        assert!(
            window
                .all_panes_mut()
                .all(|pane| pane.keystrokes.is_empty())
        );
    }
}
