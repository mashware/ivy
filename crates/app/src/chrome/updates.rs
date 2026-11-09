//! Release discovery belongs to the application, never to a console's producer (D75).
mod feed;
mod service;

use crate::{Window, viewer::markdown::MarkdownDocument};
use eframe::egui::{self, Rect};
use std::time::Instant;

const INSTALLED_NOTES: &str = include_str!("../../../../resources/release-notes.md");

#[derive(Debug, Default)]
pub(super) struct Dialog {
    service: service::Service,
    open: bool,
    dismissed: bool,
    selected: Option<usize>,
    source: String,
    document: Option<MarkdownDocument>,
    blocked_frame: Option<u64>,
    held: bool,
}

impl Dialog {
    pub(super) fn start(&mut self, ctx: &egui::Context) {
        self.service.initialize(ctx);
    }
    pub(super) fn blocked(&self, ctx: &egui::Context) -> bool {
        self.open || self.held || self.blocked_frame == Some(ctx.cumulative_frame_nr())
    }

    fn notify(&self) -> bool {
        !self.dismissed && self.service.available().is_some()
    }
}

pub(super) fn open(window: &mut Window, ctx: &egui::Context, check: bool) {
    if window.settings_editor.is_open()
        || crate::summary::blocked(window, ctx)
        || window.pane_view.is_modal()
        || window.chrome.guide.blocked(ctx)
    {
        return;
    }
    window.chrome.menu = false;
    window.chrome.updates.open = true;
    window.chrome.updates.selected = None;
    window.chrome.updates.blocked_frame = Some(ctx.cumulative_frame_nr());
    if check {
        window.chrome.updates.service.check(ctx);
    }
    crate::summary::cancel_gestures(window, ctx);
    claim_input(ctx);
    ctx.request_repaint();
}

pub(super) fn indicator(window: &Window, ui: &egui::Ui, row: Rect) {
    if !window.chrome.updates.notify() {
        return;
    }
    let from_right = if row.width() < 300.0 {
        20.0
    } else if cfg!(target_os = "macos") {
        23.0
    } else {
        125.0
    };
    let point = egui::pos2(row.right() - from_right + 9.0, row.top() + 15.0);
    ui.painter_at(row).circle_filled(
        point,
        3.5,
        window.theme.color(term_core::TermColor::Indexed(3)),
    );
    if let Some(release) = window.chrome.updates.service.available() {
        ui.interact(
            Rect::from_center_size(point, egui::vec2(12.0, 12.0)),
            ui.id().with("update-indicator"),
            egui::Sense::hover(),
        )
        .on_hover_text(format!(
            "Ivy {} is available. Open the menu for release notes.",
            release.version
        ));
    }
}

fn claim_input(ctx: &egui::Context) {
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            matches!(
                event,
                egui::Event::ModifiersChanged(_)
                    | egui::Event::WindowFocused(_)
                    | egui::Event::PointerButton { pressed: false, .. }
            )
        })
    });
}

pub(super) fn begin_pass(window: &mut Window, ui: &mut egui::Ui) {
    if !window.chrome.enabled() {
        return;
    }
    let started = Instant::now();
    let ctx = ui.ctx().clone();
    let state = &mut window.chrome.updates;
    state.service.poll();
    if state.blocked(&ctx) {
        if state.open {
            show(state, &ctx, &mut window.desktop);
        }
        state.blocked_frame = Some(ctx.cumulative_frame_nr());
        state.held = ctx.input(|input| input.focused && input.pointer.any_down());
        window
            .header_input
            .block_for_settings(ctx.cumulative_frame_nr());
        claim_input(&ctx);
        ui.disable();
    }
    window.chrome.record_work(started.elapsed());
    window.timed_separately += window.chrome.take_cost();
}

fn show(state: &mut Dialog, ctx: &egui::Context, desktop: &mut crate::links::Desktop) {
    let available = ctx.content_rect().size();
    let mut close = false;
    egui::Modal::new(egui::Id::new("ivy-updates"))
        .frame(crate::settings::editor::view::modal_frame().inner_margin(16))
        .show(ctx, |ui| {
            crate::settings::editor::view::settings_style(ui);
            ui.set_width((available.x - 64.0).clamp(80.0, 640.0));
            ui.horizontal_top(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    close |= ui.button("×").on_hover_text("Close").clicked();
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.heading("What's new");
                        ui.label(format!("Installed: Ivy {}", env!("CARGO_PKG_VERSION")));
                    });
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("release-history")
                .max_height((available.y - 150.0).max(40.0))
                .show(ui, |ui| body(state, ui, desktop));
        });
    if close || ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        state.open = false;
        ctx.memory_mut(|memory| memory.stop_text_input());
        ctx.request_repaint();
    }
}

fn body(state: &mut Dialog, ui: &mut egui::Ui, desktop: &mut crate::links::Desktop) {
    if let Some(release) = state.service.available() {
        ui.strong(format!("Ivy {} is available", release.version));
        let target = crate::links::WebTarget::parse(&release.url);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Read release notes").clicked() {
                state.selected = Some(0);
            }
            if ui
                .button("View download")
                .on_hover_text(&release.url)
                .clicked()
                && let Some(target) = &target
            {
                desktop.open(target);
            }
            if ui.button("Dismiss notification").clicked() {
                state.dismissed = true;
            }
        });
    }
    ui.label(state.service.status());
    if ui
        .add_enabled(
            !state.service.checking(),
            egui::Button::new("Check for updates"),
        )
        .clicked()
    {
        state.service.check(ui.ctx());
    }
    ui.separator();
    let selected = state
        .selected
        .and_then(|index| state.service.releases().get(index))
        .map(|release| release.name.as_str())
        .unwrap_or("Installed version");
    egui::ComboBox::from_id_salt("release-version")
        .selected_text(selected)
        .width(ui.available_width().min(580.0))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut state.selected, None, "Installed version");
            for (index, release) in state.service.releases().iter().enumerate() {
                ui.selectable_value(
                    &mut state.selected,
                    Some(index),
                    format!("{} — {}", release.version, release.name),
                );
            }
        });
    let source = state
        .selected
        .and_then(|index| state.service.releases().get(index))
        .map(|release| release.notes.as_str())
        .unwrap_or(INSTALLED_NOTES);
    if state.source != source {
        state.document = MarkdownDocument::parse(source).ok();
        state.source = source.to_owned();
    }
    if let Some(document) = &state.document {
        document.show(ui);
    } else {
        ui.label("These release notes cannot be displayed. Open the release page to read them.");
    }
    if let Some(release) = state
        .selected
        .and_then(|index| state.service.releases().get(index))
        && ui
            .button("Open release page")
            .on_hover_text(&release.url)
            .clicked()
        && let Some(target) = crate::links::WebTarget::parse(&release.url)
    {
        desktop.open(&target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: &str) -> feed::Release {
        feed::Release {
            version: version.parse().unwrap(),
            name: format!("Ivy {version}"),
            notes: "## New feature\n\nYou can read release notes.".into(),
            url: format!("https://github.com/mashware/ivy/releases/tag/v{version}"),
        }
    }

    #[test]
    fn only_newer_releases_notify_and_dismissing_keeps_notes_available() {
        let mut dialog = Dialog::default();
        dialog
            .service
            .fixture(vec![release(env!("CARGO_PKG_VERSION"))]);
        assert!(!dialog.notify());
        dialog.service.fixture(vec![release("99.0.0")]);
        assert!(dialog.notify());
        dialog.dismissed = true;
        assert!(!dialog.notify());
        assert_eq!(
            dialog.service.available().unwrap().version.to_string(),
            "99.0.0"
        );
        assert_eq!(dialog.service.releases().len(), 1);
    }

    #[test]
    fn integrated_measurement_start_does_not_start_a_request() {
        let mut chrome = super::super::Chrome::default();
        chrome.start(&egui::Context::default(), false);
        assert!(chrome.enabled());
        assert!(!chrome.updates.service.checking());
        assert!(chrome.updates.service.releases().is_empty());
    }

    #[test]
    fn newer_release_is_inert_until_the_download_button_is_clicked() {
        fn render(
            dialog: &mut Dialog,
            ctx: &egui::Context,
            desktop: &mut crate::links::Desktop,
            events: Vec<egui::Event>,
        ) -> Vec<egui::epaint::ClippedShape> {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |_ui| show(dialog, ctx, desktop),
            );
            let shapes = output.shapes.clone();
            output.drop_without_applying_deltas();
            shapes
        }
        let mut dialog = Dialog::default();
        dialog.service.fixture(vec![release("99.0.0")]);
        let ctx = egui::Context::default();
        let mut desktop = crate::links::Desktop::Recorded(Vec::new());
        render(&mut dialog, &ctx, &mut desktop, vec![]);
        let shapes = render(&mut dialog, &ctx, &mut desktop, vec![]);
        let crate::links::Desktop::Recorded(opened) = &desktop else {
            unreachable!()
        };
        assert!(opened.is_empty());
        let at = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "View download" => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            })
            .expect("download button is visible");
        render(
            &mut dialog,
            &ctx,
            &mut desktop,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        render(
            &mut dialog,
            &ctx,
            &mut desktop,
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let crate::links::Desktop::Recorded(opened) = desktop else {
            unreachable!()
        };
        assert_eq!(
            opened,
            ["https://github.com/mashware/ivy/releases/tag/v99.0.0"]
        );
    }
}
