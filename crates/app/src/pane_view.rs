//! Transient presentation state; it never changes the stored layout tree.

use std::collections::HashMap;

use eframe::egui;

use crate::tiling::Tiling;

pub(crate) fn display_label(pane: &crate::Pane) -> std::borrow::Cow<'_, str> {
    pane.label
        .as_deref()
        .filter(|label| !label.trim().is_empty())
        .or_else(|| pane.screen.terminal_title())
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| format!("Console {}", pane.name + 1).into())
}

#[derive(Clone, Copy)]
enum RenameTarget {
    Pane { tab: usize, pane: usize },
    Tab { tab: usize },
}

#[derive(Default)]
pub(crate) struct State {
    zoom: HashMap<usize, usize>,
    rename: Option<(RenameTarget, String)>,
    menu: Option<(usize, usize, bool, egui::Pos2)>,
    blocked_frame: Option<u64>,
    awaiting_release: bool,
}

impl State {
    pub(crate) fn is_modal(&self) -> bool {
        self.rename.is_some() || self.menu.is_some() || self.awaiting_release
    }

    pub(crate) fn is_renaming(&self) -> bool {
        self.rename.is_some()
    }
    pub(crate) fn open_menu(&mut self, tab: usize, pane: usize, can_close: bool, at: egui::Pos2) {
        self.menu = Some((tab, pane, can_close, at));
    }
    pub(crate) fn zoomed(&self, tab: usize) -> Option<usize> {
        self.zoom.get(&tab).copied()
    }

    pub(crate) fn toggle(&mut self, tab: usize, pane: usize) {
        if self.zoomed(tab) == Some(pane) {
            self.clear(tab);
        } else {
            self.zoom.insert(tab, pane);
        }
    }

    pub(crate) fn clear(&mut self, tab: usize) {
        self.zoom.remove(&tab);
    }

    pub(crate) fn begin_rename(&mut self, tab: usize, pane: usize, initial: String) {
        self.rename = Some((RenameTarget::Pane { tab, pane }, initial));
    }

    pub(crate) fn begin_tab_rename(&mut self, tab: usize, initial: String) {
        self.rename = Some((RenameTarget::Tab { tab }, initial));
    }

    pub(crate) fn reconcile(&mut self, tab: usize, focus: usize, panes: &[usize]) {
        if self
            .zoomed(tab)
            .is_some_and(|pane| pane != focus || !panes.contains(&pane))
        {
            self.clear(tab);
        }
    }

    /// Keep index alignment while leaving hidden panes out of the visible geometry.
    pub(crate) fn view_tiling(
        &self,
        tab: usize,
        panes: &[usize],
        mut tiling: Tiling,
        area: egui::Rect,
    ) -> Tiling {
        if let Some(index) = self
            .zoomed(tab)
            .and_then(|pane| panes.iter().position(|id| *id == pane))
        {
            tiling.parts.fill(egui::Rect::NOTHING);
            if let Some(rect) = tiling.parts.get_mut(index) {
                *rect = area;
                tiling.dividers.clear();
            }
        }
        tiling
    }
}

pub(crate) fn reconcile(window: &mut crate::Window) {
    if !window.chrome.enabled() {
        return;
    }
    let started = std::time::Instant::now();
    let tab = window.active();
    let name = tab.name;
    let focus = tab.focus;
    let panes: Vec<_> = tab.panes().iter().map(|pane| pane.name).collect();
    window.pane_view.reconcile(name, focus, &panes);
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
}

pub(crate) fn view_tiling(
    window: &crate::Window,
    area: egui::Rect,
    metrics: &crate::painter::CellMetrics,
) -> Tiling {
    let tab = window.active();
    if !window.chrome.enabled() {
        return tab.root.place(area, metrics.width());
    }
    let panes: Vec<_> = tab.panes().iter().map(|pane| pane.name).collect();
    let mut view = window.pane_view.view_tiling(
        tab.name,
        &panes,
        tab.root.place(area, metrics.width()),
        area,
    );
    for rect in &mut view.parts {
        if *rect != egui::Rect::NOTHING {
            *rect = padded_rect(*rect, true);
        }
    }
    view
}

/// Keep hidden panes on the same grid as the padded visible panes.
pub(crate) fn padded_rect(rect: egui::Rect, integrated: bool) -> egui::Rect {
    if !integrated {
        return rect;
    }
    let inset = crate::header::Headers::PAINTED
        .integrated(true)
        .pane_inset()
        .min(rect.width().max(0.0) / 2.0)
        .min(rect.height().max(0.0) / 2.0);
    rect.shrink(inset)
}

/// Paint the focus and pane outlines over the contents without changing their hit geometry.
pub(crate) fn separator_color(window: &crate::Window) -> egui::Color32 {
    if window.chrome.enabled() {
        window.theme.background()
    } else {
        window.theme.divider()
    }
}

pub(crate) fn paint_borders(window: &mut crate::Window, ui: &egui::Ui, rects: &[egui::Rect]) {
    let integrated = window.chrome.enabled();
    let started = integrated.then(std::time::Instant::now);
    let focused = window.active().where_the_focus_is();
    for (index, rect) in rects.iter().copied().enumerate() {
        if rect == egui::Rect::NOTHING || (!integrated && focused != Some(index)) {
            continue;
        }
        let color = if focused == Some(index) {
            window.theme.focus()
        } else {
            window.theme.chrome_outline()
        };
        ui.painter().rect_stroke(
            rect,
            if integrated { 5.0 } else { 0.0 },
            egui::Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(started) = started {
        window.chrome.record_work(started.elapsed());
        window.timed_separately += window.chrome.take_cost();
    }
}

fn menu_item(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let button = egui::Button::new((label, egui::Atom::grow()))
        .frame_when_inactive(false)
        .min_size(egui::vec2(ui.available_width(), 28.0));
    ui.add_enabled(enabled, button).clicked()
}

fn split_buttons(
    ui: &mut egui::Ui,
    theme: crate::theme::Theme,
    enabled: [bool; 2],
) -> Option<usize> {
    let width = (ui.available_width() - 2.0).max(0.0) / 2.0;
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (index, label) in ["Split right", "Split down"].into_iter().enumerate() {
            let response = ui.add_enabled(
                enabled[index],
                egui::Button::new("").min_size(egui::vec2(width, 30.0)),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled[index], label)
            });
            crate::chrome::paint_button_icon(
                ui.painter(),
                response.rect,
                "",
                label,
                if enabled[index] {
                    theme.foreground()
                } else {
                    theme.foreground().gamma_multiply(0.35)
                },
            );
            if response.clicked() {
                chosen = Some(index);
            }
            response.on_hover_text(label);
        }
    });
    chosen
}

/// Returns true for every frame owned by the editor, including its dismissal.
pub(crate) fn before_keys(
    window: &mut crate::Window,
    ui: &mut egui::Ui,
    metrics: &crate::painter::CellMetrics,
) -> bool {
    let frame = ui.ctx().cumulative_frame_nr();
    if window.pane_view.blocked_frame == Some(frame) {
        return true;
    }
    if !window.pane_view.is_renaming()
        && window.pane_view.menu.is_none()
        && !window.pane_view.awaiting_release
    {
        return false;
    }
    let started = std::time::Instant::now();
    window.pane_view.blocked_frame = Some(frame);
    if let Some((tab, pane, can_close, at)) = window.pane_view.menu.take() {
        let mut action = None;
        let area = crate::tab_bar::geometry(window, ui.max_rect(), metrics).consoles;
        let axes = [
            crate::tiling::Orientation::Columns,
            crate::tiling::Orientation::Rows,
        ];
        let enabled = window
            .active()
            .panes()
            .iter()
            .position(|p| p.name == pane)
            .map(|index| {
                let rect = window.active().root.place(area, metrics.width()).parts[index];
                axes.map(|axis| crate::tiling::can_split(rect, axis, metrics, window.headers))
            })
            .unwrap_or([false; 2]);
        let theme = window.theme;
        let selected = window
            .pane_called(pane)
            .and_then(|pane| pane.screen.selected_text());
        let viewport = ui.ctx().viewport_rect();
        let menu = egui::Area::new(egui::Id::new("console-context-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .constrain_to(ui.ctx().viewport_rect())
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .fill(theme.panel_background())
                    .stroke(egui::Stroke::new(1.0, theme.divider()))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height((viewport.height() - 20.0).max(1.0))
                            .show(ui, |ui| {
                                crate::settings::editor::view::style(ui, theme);
                                ui.set_width(190.0_f32.min((viewport.width() - 20.0).max(1.0)));
                                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                                ui.visuals_mut().widgets.hovered.weak_bg_fill =
                                    theme.divider().gamma_multiply(0.55);
                                ui.visuals_mut().widgets.active.weak_bg_fill = theme.divider();
                                if let Some(index) = split_buttons(ui, theme, enabled) {
                                    action = Some(5 + index);
                                }
                                ui.separator();
                                if menu_item(ui, "Copy", selected.is_some()) {
                                    action = Some(8);
                                }
                                if menu_item(ui, "Paste", true) {
                                    action = Some(9);
                                }
                                ui.separator();
                                if menu_item(ui, "Rename", true) {
                                    action = Some(0);
                                }
                                if menu_item(ui, "Review changes", true) {
                                    action = Some(7);
                                }
                                if menu_item(ui, "Toggle information panel", true) {
                                    action = Some(1);
                                }
                                if menu_item(ui, "Maximize / restore", true) {
                                    action = Some(2);
                                }
                                if menu_item(ui, "Close console", can_close) {
                                    action = Some(3);
                                }
                                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    action = Some(4);
                                }
                            });
                    });
            });
        if ui.input(|input| {
            input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| !menu.response.rect.contains(pos))
        }) {
            action = Some(4);
        }
        match action {
            Some(8) => {
                if let Some(text) = selected {
                    ui.ctx().copy_text(text);
                }
            }
            Some(9) => {
                window.clipboard.request(
                    pane,
                    crate::clipboard::Source::Clipboard,
                    std::time::Instant::now(),
                );
            }
            Some(0) => {
                let initial = window
                    .pane_called(pane)
                    .map(|p| display_label(p).into_owned())
                    .unwrap_or_else(|| format!("Console {}", pane + 1));
                window.pane_view.begin_rename(tab, pane, initial);
            }
            Some(1) => {
                if let Some(p) = window.pane_called(pane)
                    && p.panel_state.is_visible(std::time::SystemTime::now())
                {
                    p.folded = !p.folded;
                }
            }
            Some(2) => {
                window.active_mut().focus = pane;
                window.pane_view.toggle(tab, pane);
            }
            Some(3) => {
                window.pane_view.clear(tab);
                if let Err(error) = window.close_a_pane_called(pane, ui.ctx()) {
                    eprintln!("the button could not close its console: {error}");
                }
            }
            Some(7) => {
                crate::viewer::open_diff_for_pane(window, pane);
            }
            Some(index @ (5 | 6)) => {
                window.pane_view.clear(tab);
                crate::header::split_named(
                    window,
                    pane,
                    axes[index - 5],
                    ui.ctx(),
                    ui.max_rect(),
                    metrics,
                );
            }
            Some(_) => {}
            None => window.pane_view.menu = Some((tab, pane, can_close, at)),
        }
        if action.is_some() {
            window.pane_view.awaiting_release = ui.ctx().input(|i| i.pointer.any_down());
            ui.ctx().request_repaint();
        }
        window.header_input.block_for_settings(frame);
        ui.ctx().input_mut(|i| {
            i.events.retain(|e| {
                matches!(
                    e,
                    egui::Event::ModifiersChanged(_)
                        | egui::Event::WindowFocused(_)
                        | egui::Event::PointerButton { pressed: false, .. }
                )
            })
        });
        window.chrome.record_work(started.elapsed());
        window.timed_separately += window.chrome.take_cost();
        return true;
    }
    let Some((target, mut draft)) = window.pane_view.rename.take() else {
        window.pane_view.awaiting_release = ui.ctx().input(|i| i.pointer.any_down());
        window.header_input.block_for_settings(frame);
        ui.ctx().input_mut(|i| {
            i.events.retain(|e| {
                matches!(
                    e,
                    egui::Event::ModifiersChanged(_) | egui::Event::WindowFocused(_)
                )
            })
        });
        window.chrome.record_work(started.elapsed());
        window.timed_separately += window.chrome.take_cost();
        return true;
    };
    let mut save = false;
    let mut cancel = false;
    let mut follow_title = false;
    let theme = window.theme;
    let title = match target {
        RenameTarget::Pane { .. } => "Rename console",
        RenameTarget::Tab { .. } => "Rename tab",
    };
    egui::Window::new(title)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .constrain_to(ui.ctx().viewport_rect())
        .movable(false)
        .default_width(380.0)
        .frame(
            egui::Frame::window(ui.style())
                .inner_margin(20.0)
                .fill(theme.panel_background())
                .stroke(egui::Stroke::new(1.0, theme.divider())),
        )
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            crate::settings::editor::view::style(ui, theme);
            ui.spacing_mut().button_padding = egui::vec2(12.0, 7.0);
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(12.0));
            ui.label(egui::RichText::new(title).size(16.0));
            ui.add_space(12.0);
            if matches!(target, RenameTarget::Pane { .. }) {
                ui.label("A manual name overrides the terminal title. Leave empty to follow it.");
            }
            let response = ui.add(
                egui::TextEdit::singleline(&mut draft)
                    .font(egui::FontId::proportional(13.0))
                    .margin(egui::Margin::symmetric(10, 9))
                    .min_size(egui::vec2(0.0, 36.0))
                    .desired_width(ui.available_width()),
            );
            response.request_focus();
            if !crate::session::valid_label(&draft) {
                ui.label("Use at most 128 characters, without control characters.");
            }
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                save = ui.button("Save").clicked();
                cancel = ui.button("Cancel").clicked();
                if matches!(target, RenameTarget::Pane { .. }) {
                    follow_title = ui.button("Use terminal title").clicked();
                }
            });
            save |= ui.input(|i| i.key_pressed(egui::Key::Enter));
            cancel |= ui.input(|i| i.key_pressed(egui::Key::Escape));
        });
    if follow_title || save && crate::session::valid_label(&draft) {
        let label = draft.trim();
        let label = (!follow_title && !label.is_empty()).then(|| label.to_owned());
        match target {
            RenameTarget::Pane { tab, pane } => {
                if let Some(pane) = window
                    .tabs
                    .iter_mut()
                    .find(|t| t.name == tab)
                    .and_then(|t| t.pane_called(pane))
                {
                    pane.label = label;
                }
            }
            RenameTarget::Tab { tab } => {
                if let Some(tab) = window.tabs.iter_mut().find(|t| t.name == tab) {
                    tab.label = label;
                }
            }
        }
        window.pane_view.awaiting_release = ui.ctx().input(|i| i.pointer.any_down());
    } else if !cancel {
        window.pane_view.rename = Some((target, draft));
    } else {
        window.pane_view.awaiting_release = ui.ctx().input(|i| i.pointer.any_down());
    }
    window
        .header_input
        .block_for_settings(ui.ctx().cumulative_frame_nr());
    ui.ctx().input_mut(|i| {
        i.events.retain(|e| {
            matches!(
                e,
                egui::Event::ModifiersChanged(_)
                    | egui::Event::WindowFocused(_)
                    | egui::Event::PointerButton { pressed: false, .. }
            )
        })
    });
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::valid_label;

    #[test]
    fn zoom_changes_only_the_view_and_reconciles_focus() {
        let mut state = State::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 300.0));
        let parts = vec![area, area];
        state.toggle(4, 9);
        let result = state.view_tiling(
            4,
            &[8, 9],
            Tiling {
                parts,
                dividers: vec![],
            },
            area,
        );
        assert_eq!(result.parts, vec![egui::Rect::NOTHING, area]);
        state.reconcile(4, 8, &[8, 9]);
        assert_eq!(state.zoomed(4), None);
    }

    #[test]
    fn label_bounds_count_scalars_and_reject_controls() {
        assert!(valid_label(&"界".repeat(128)));
        assert!(!valid_label(&"界".repeat(129)));
        assert!(!valid_label("unsafe\nlabel"));
        let restored: crate::session::StoredLeaf =
            serde_json::from_str(r#"{"panel_width":36,"folded":false,"label":"Review"}"#).unwrap();
        assert_eq!(restored.label.as_deref(), Some("Review"));
        assert!(
            serde_json::from_str::<crate::session::StoredLeaf>(
                r#"{"panel_width":36,"folded":false,"label":"bad\nname"}"#
            )
            .is_err()
        );
        let old: crate::session::StoredLeaf =
            serde_json::from_str(r#"{"panel_width":36,"folded":false}"#).unwrap();
        assert_eq!(old.label, None);
    }
}
