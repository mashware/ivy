//! Official client marks, decoded once; busy state changes only their opacity.
use super::{Client, View};
use eframe::egui;
use std::time::{Duration, Instant};

fn key(client: Client) -> egui::Id {
    egui::Id::new(("agent-logo", client as u8))
}
pub(crate) fn install(ctx: &egui::Context) {
    for (client, bytes) in [
        (
            Client::Claude,
            include_bytes!("../../../../resources/agent-logos/claude.png").as_slice(),
        ),
        (
            Client::Codex,
            include_bytes!("../../../../resources/agent-logos/codex.png").as_slice(),
        ),
        (
            Client::OpenCode,
            include_bytes!("../../../../resources/agent-logos/opencode.png").as_slice(),
        ),
    ] {
        let icon =
            eframe::icon_data::from_png_bytes(bytes).expect("bundled client logo must decode");
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [icon.width as usize, icon.height as usize],
            &icon.rgba,
        );
        let texture = ctx.load_texture(client.name(), image, egui::TextureOptions::LINEAR);
        ctx.data_mut(|data| data.insert_temp(key(client), texture));
    }
}
pub(crate) fn animation_delay(view: View, visible: bool, focused: bool) -> Option<Duration> {
    (view.busy && view.backed && visible && focused).then_some(Duration::from_millis(100))
}
pub(crate) fn paint(ui: &egui::Ui, rect: egui::Rect, view: View) {
    let painter = ui.painter_at(rect);
    let active_viewport = ui
        .ctx()
        .input(|i| i.focused && i.viewport().minimized != Some(true));
    let visible = ui.is_rect_visible(rect);
    let alpha = if animation_delay(view, visible, active_viewport).is_some() {
        let time = ui.ctx().input(|i| i.time);
        0.6 + 0.4 * ((time * std::f64::consts::TAU / 1.2).sin() * 0.5 + 0.5) as f32
    } else if view.backed {
        1.0
    } else {
        0.45
    };
    if let Some(delay) = animation_delay(view, visible, active_viewport) {
        ui.ctx().request_repaint_after(delay);
    }
    if let Some(texture) = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(key(view.client)))
    {
        // The official OpenCode favicon includes a wide square background around its 4:5 mark.
        // Sample the mark itself and preserve its proportions inside the shared logo slot.
        let (image_rect, uv) = if view.client == Client::OpenCode {
            (
                egui::Rect::from_center_size(
                    rect.center(),
                    egui::vec2(rect.width() * 0.8, rect.height()),
                ),
                egui::Rect::from_min_max(egui::pos2(0.25, 0.1875), egui::pos2(0.75, 0.8125)),
            )
        } else {
            (
                rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            )
        };
        painter.image(
            texture.id(),
            image_rect,
            uv,
            egui::Color32::WHITE.gamma_multiply(alpha),
        );
    }
    ui.interact(
        rect,
        key(view.client).with((rect.min.x.to_bits(), rect.min.y.to_bits())),
        egui::Sense::hover(),
    )
    .on_hover_text(format!(
        "{} · {}",
        view.client.name(),
        if !view.backed {
            "Last declaration is unconfirmed"
        } else if view.waiting {
            "Waiting for your input"
        } else if view.busy {
            "Working"
        } else {
            "No declared activity"
        }
    ));
}
pub(crate) fn arm_stale(window: &crate::Window, ctx: &egui::Context) {
    let now = Instant::now();
    for tab in &window.tabs {
        tab.root.walk(&mut |pane| {
            if let Some(delay) = pane.signals.stale_in(now) {
                ctx.request_repaint_after(delay);
            }
        });
    }
}
