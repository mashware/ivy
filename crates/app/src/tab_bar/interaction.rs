//! Pointer interactions for the integrated tab strip.
use super::{CellMetrics, Window, WindowRequest, feedback, title};
use eframe::egui;
use std::time::Instant;

pub(super) fn handle(
    window: &mut Window,
    ui: &egui::Ui,
    tab_bar: egui::Rect,
    metrics: &CellMetrics,
    blocked: bool,
) {
    let started = Instant::now();
    let count = window.tabs.len();
    let mut action = None;
    let mut rename = None;
    // The add button stays after the last card, so the card it creates lands under the pointer:
    // the second half of a double click on the add button is not a double click on that card.
    let added = ui.id().with("tab-add-clicked");
    let after_add = ui.ctx().data(|d| d.get_temp::<bool>(added)).is_some();
    let mut clicked_a_card = false;
    for (which_tab, _, layout) in title::cards(window, ui, tab_bar) {
        let name = window.tabs[which_tab].name;
        if let Some(close) = layout.close
            && feedback::interact(ui, close, feedback::id(ui, name, "close"))
                .on_hover_text("Close tab")
                .clicked()
        {
            action = Some((which_tab, true));
        }
        let response = feedback::interact(ui, layout.body, feedback::id(ui, name, "body"))
            .on_hover_text("Double-click or right-click to rename tab");
        clicked_a_card |= response.clicked();
        let doubled = response.double_clicked() && !after_add;
        if !blocked && (doubled || response.secondary_clicked()) {
            rename = Some(which_tab);
        } else if response.clicked() {
            action = Some((which_tab, false));
        }
    }
    if let Some(which_tab) = rename {
        let tab = &window.tabs[which_tab];
        let initial = tab
            .label
            .clone()
            .unwrap_or_else(|| format!("Terminal {}", tab.name + 1));
        window.pane_view.begin_tab_rename(tab.name, initial);
    }

    let new_tab = title::add_target(tab_bar, count).is_some_and(|add| {
        feedback::interact(ui, add, ui.id().with("compact-tab-add"))
            .on_hover_text("New tab")
            .clicked()
    });
    if new_tab {
        ui.ctx().data_mut(|d| d.insert_temp(added, true));
    } else if clicked_a_card {
        ui.ctx().data_mut(|d| d.remove::<bool>(added));
    }
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
    if blocked {
        return;
    }
    if let Some((which, close)) = action {
        window.apply(
            WindowRequest::Go(which as isize - window.active as isize),
            ui.ctx(),
            ui.max_rect(),
            metrics,
        );
        if close {
            window.apply(WindowRequest::CloseTab, ui.ctx(), ui.max_rect(), metrics);
        }
    } else if new_tab {
        window.apply(WindowRequest::NewTab, ui.ctx(), ui.max_rect(), metrics);
    }
    if action.is_some() || new_tab {
        crate::board::reanchor(window);
    }
}
