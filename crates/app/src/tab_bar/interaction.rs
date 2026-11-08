//! Pointer interactions for the integrated tab strip.
use super::{CLOSE_MIN_WIDTH, CellMetrics, Window, WindowRequest, add_tab_rect, compact_tab_rect};
use eframe::egui;
use std::time::Instant;

pub(super) fn handle(
    window: &mut Window,
    ui: &egui::Ui,
    tab_bar: egui::Rect,
    metrics: &CellMetrics,
) {
    let started = Instant::now();
    let count = window.tabs.len();
    for which_tab in 0..count {
        let card = compact_tab_rect(tab_bar, which_tab, count);
        let close = egui::Rect::from_min_max(
            egui::pos2((card.right() - 25.0).max(card.left()), card.top()),
            card.right_bottom(),
        );
        let can_close = count > 1 && card.width() >= CLOSE_MIN_WIDTH;
        if can_close
            && ui
                .interact(
                    close,
                    ui.id().with(("compact-tab-close", which_tab)),
                    egui::Sense::CLICK,
                )
                .on_hover_text("Close tab")
                .clicked()
        {
            window.chrome.record_work(started.elapsed());
            window.timed_separately += window.chrome.take_cost();
            window.apply(
                WindowRequest::Go(which_tab as isize - window.active as isize),
                ui.ctx(),
                ui.max_rect(),
                metrics,
            );
            window.apply(WindowRequest::CloseTab, ui.ctx(), ui.max_rect(), metrics);
            return;
        }
        let body = egui::Rect::from_min_max(
            card.min,
            egui::pos2(
                if can_close {
                    close.left()
                } else {
                    card.right()
                },
                card.bottom(),
            ),
        );
        let response = ui
            .interact(
                body,
                ui.id().with(("compact-tab", which_tab)),
                egui::Sense::CLICK,
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
            window.chrome.record_work(started.elapsed());
            window.timed_separately += window.chrome.take_cost();
            window.apply(
                WindowRequest::Go(which_tab as isize - window.active as isize),
                ui.ctx(),
                ui.max_rect(),
                metrics,
            );
            return;
        }
    }

    let add = add_tab_rect(tab_bar, count);
    if add.width() >= 20.0
        && ui
            .interact(add, ui.id().with("compact-tab-add"), egui::Sense::CLICK)
            .on_hover_text("New tab")
            .clicked()
    {
        window.chrome.record_work(started.elapsed());
        window.timed_separately += window.chrome.take_cost();
        window.apply(WindowRequest::NewTab, ui.ctx(), ui.max_rect(), metrics);
        return;
    }
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
}
