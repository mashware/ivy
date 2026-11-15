//! Pointer interactions for the integrated tab strip.
use super::{CellMetrics, Window, WindowRequest, add_tab_rect, compact_tab_rect, feedback, layout};
use eframe::egui;
use std::time::{Instant, SystemTime};

pub(super) fn handle(
    window: &mut Window,
    ui: &egui::Ui,
    tab_bar: egui::Rect,
    metrics: &CellMetrics,
) {
    let started = Instant::now();
    let count = window.tabs.len();
    let mut action = None;
    for which_tab in 0..count {
        let card = compact_tab_rect(tab_bar, which_tab, count);
        let layout = layout::Layout::new(
            card,
            window.chrome.title_sizes.tab_size(),
            window.tabs[which_tab]
                .attention(SystemTime::now())
                .is_some(),
            count > 1,
        );
        if let Some(close) = layout.close
            && feedback::interact(
                ui,
                close,
                feedback::id(ui, window.tabs[which_tab].name, "close"),
            )
            .on_hover_text("Close tab")
            .clicked()
        {
            action = Some((which_tab, true));
        }
        let response = feedback::interact(
            ui,
            layout.body,
            feedback::id(ui, window.tabs[which_tab].name, "body"),
        )
        .on_hover_text("Double-click or right-click to rename tab");
        if response.double_clicked() || response.secondary_clicked() {
            let tab = &window.tabs[which_tab];
            let initial = tab
                .label
                .clone()
                .unwrap_or_else(|| format!("Terminal {}", tab.name + 1));
            window.pane_view.begin_tab_rename(tab.name, initial);
        } else if response.clicked() {
            action = Some((which_tab, false));
        }
    }

    let add = add_tab_rect(tab_bar, count);
    let new_tab = add.width() >= 20.0
        && feedback::interact(ui, add, ui.id().with("compact-tab-add"))
            .on_hover_text("New tab")
            .clicked();
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
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
