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

pub(super) fn open(window: &mut Window, ctx: &egui::Context, check: bool, board_handoff: bool) {
    if window.settings_editor.is_open()
        || (crate::board::blocked(window, ctx) && !board_handoff)
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
    crate::board::cancel_gestures(window, ctx);
    claim_input(ctx);
    ctx.request_repaint();
}

pub(super) fn indicator(window: &Window, ui: &egui::Ui, row: Rect) {
    if !window.chrome.updates.notify() {
        return;
    }
    use super::strip;
    // Over the menu button's top-right corner, wherever this platform and width put the menu.
    let from_right = if strip::is_compact(row.width()) {
        strip::COMPACT_MENU / 2.0
    } else {
        strip::RIGHT_MARGIN + (strip::right_buttons() - 3) as f32 * strip::BUTTON + 17.0
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
            show(state, &ctx, &mut window.desktop, &window.theme);
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

fn show(
    state: &mut Dialog,
    ctx: &egui::Context,
    desktop: &mut crate::links::Desktop,
    theme: &crate::theme::Theme,
) {
    let available = ctx.content_rect().size();
    let mut close = false;
    egui::Modal::new(egui::Id::new("ivy-updates"))
        .frame(
            crate::ui::floating::dialog_frame(&crate::ui::style::roles(ctx, theme))
                .inner_margin(16),
        )
        .show(ctx, |ui| {
            crate::ui::floating::style(ui, theme);
            // The chrome's dialogs read at Settings' measures (D67's native review).
            crate::settings::editor::view::settings_geometry(ui.style_mut());
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

    /// The dialog floats on the theme's `raised` role, whatever egui was told about the desktop,
    /// and keeps Settings' measures (D98).
    #[test]
    fn the_updates_dialog_floats_on_raised_at_settings_measures() {
        for preference in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
            let mut dialog = Dialog::default();
            dialog.service.fixture(vec![release("99.0.0")]);
            let ctx = egui::Context::default();
            ctx.set_theme(preference);
            // Final colours, without the modal's time-dependent fade multiplying alpha.
            ctx.all_styles_mut(|style| style.animation_time = 0.0);
            let mut desktop = crate::links::Desktop::Recorded(Vec::new());
            let mut shapes = Vec::new();
            for _ in 0..2 {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1000.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |_ui| {
                        show(
                            &mut dialog,
                            &ctx,
                            &mut desktop,
                            &crate::theme::Theme::default(),
                        )
                    },
                );
                shapes = output.shapes.clone();
                output.drop_without_applying_deltas();
            }
            fn big_raised(shape: &egui::Shape) -> bool {
                match shape {
                    egui::Shape::Rect(r) => {
                        r.fill == egui::Color32::from_rgb(0x20, 0x24, 0x27)
                            && r.rect.width() > 300.0
                            && r.rect.height() > 80.0
                    }
                    egui::Shape::Vec(shapes) => shapes.iter().any(big_raised),
                    _ => false,
                }
            }
            assert!(
                shapes.iter().any(|s| big_raised(&s.shape)),
                "{preference:?}: the dialog is not on raised"
            );
            // Settings' 25-point heading, not the 20 of the floating preset.
            assert!(
                shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t)
                    if t.galley.text() == "What's new"
                        && t.galley.job.sections.iter().all(|x| x.format.font_id.size == 25.0))),
                "{preference:?}: the heading lost Settings' measure"
            );
        }
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
                |_ui| show(dialog, ctx, desktop, &crate::theme::Theme::default()),
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
