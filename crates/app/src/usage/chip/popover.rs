//! The summary that opens under the chip: one tab per agent, its windows with use, reset and pace,
//! and a way into AI usage. It floats like a menu but is not modal, by decision (#283): it takes
//! no keys, claims the pointer over its own rectangle only and does not dim the consoles, since
//! only modal surfaces may. `summary_tests.rs` holds it to that.

use super::{Agent, Source, Tone, describe};
use crate::Window;
use eframe::egui::{self, Align2, FontId, Rect, Sense, Stroke};

const WIDTH: f32 = 360.0;
const MARGIN: f32 = 12.0;
const TAB: f32 = 26.0;
const LINE: f32 = 20.0;
const BUTTON: f32 = 28.0;

/// Paints the summary under `chip` while it is open; returns whether «Open AI usage» was chosen.
/// Without a chip this frame — a compact bar, or nothing enabled — it closes.
pub(crate) fn show(window: &mut Window, ctx: &egui::Context, chip: Option<Rect>) -> bool {
    let state = &mut window.usage.chip;
    state.summary = None;
    let (Some(chip), true, Some(claude), Some(codex)) =
        (chip, state.popover, state.claude, state.codex)
    else {
        state.popover = false;
        return false;
    };
    let tab = state
        .tab
        .unwrap_or(if claude.enabled() || !codex.enabled() {
            Agent::Claude
        } else {
            Agent::Codex
        });
    let source = match tab {
        Agent::Claude => claude,
        Agent::Codex => codex,
    };
    let lines = describe(tab, &source, chrono::Utc::now().timestamp(), &chrono::Local);
    let height = MARGIN * 3.0 + TAB + lines.len() as f32 * LINE + BUTTON;
    let screen = ctx.content_rect();
    let left = (chip.right() - WIDTH).clamp(screen.left(), (screen.right() - WIDTH).max(0.0));
    let rect = Rect::from_min_size(
        egui::pos2(left, chip.bottom() + 6.0),
        egui::vec2(WIDTH.min(screen.width()), height),
    );
    let roles = crate::ui::style::roles(ctx, &window.theme);
    let id = egui::Id::new("usage-chip-summary");
    let layer = egui::LayerId::new(egui::Order::Foreground, id);
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .movable(false)
        .constrain(false)
        .show(ctx, |registered| {
            registered.allocate_rect(rect, Sense::hover());
        });
    let ui = egui::Ui::new(
        ctx.clone(),
        id.with("content"),
        egui::UiBuilder::new().layer_id(layer).max_rect(rect),
    );
    let radius = crate::theme::metrics::RADIUS_FLOATING;
    ctx.layer_painter(layer)
        .add(roles.shadow.as_shape(rect, radius));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, radius, roles.raised);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, roles.line2),
        egui::StrokeKind::Inside,
    );
    let mut chosen = None;
    let mut x = rect.left() + MARGIN;
    let top = rect.top() + MARGIN;
    for agent in [Agent::Claude, Agent::Codex] {
        let label = agent.name();
        let width = 24.0 + label.len() as f32 * 8.0;
        let target = Rect::from_min_size(egui::pos2(x, top), egui::vec2(width, TAB));
        let response = ui.interact(target, id.with(label), Sense::CLICK);
        if agent == tab || response.hovered() {
            painter.rect_filled(target, crate::theme::metrics::RADIUS_CONTROL, roles.hover);
        }
        painter.circle_filled(
            egui::pos2(target.left() + 10.0, target.center().y),
            3.0,
            super::surface::dot(agent),
        );
        painter.text(
            egui::pos2(target.left() + 18.0, target.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(12.0),
            if agent == tab { roles.tx } else { roles.tx2 },
        );
        if response.clicked() {
            chosen = Some(agent);
        }
        x = target.right() + 4.0;
    }
    let mut y = top + TAB + MARGIN;
    for (text, tone) in &lines {
        painter.text(
            egui::pos2(rect.left() + MARGIN, y + LINE / 2.0),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(12.0),
            line_ink(*tone, &source, &roles),
        );
        y += LINE;
    }
    let button = Rect::from_min_size(
        egui::pos2(rect.left() + MARGIN, y + MARGIN),
        egui::vec2(120.0, BUTTON),
    );
    let response = ui.interact(button, id.with("open"), Sense::CLICK);
    let fill = if response.hovered() {
        roles.hover
    } else {
        roles.raised
    };
    painter.rect(
        button,
        crate::theme::metrics::RADIUS_CONTROL,
        fill,
        Stroke::new(1.0, roles.line2),
        egui::StrokeKind::Inside,
    );
    painter.text(
        button.center(),
        Align2::CENTER_CENTER,
        "Open AI usage",
        FontId::proportional(12.0),
        roles.tx,
    );
    let open_usage = response.clicked();
    let state = &mut window.usage.chip;
    if let Some(agent) = chosen {
        state.tab = Some(agent);
        ctx.request_repaint();
    }
    if open_usage {
        state.popover = false;
    } else {
        state.summary = Some(rect);
    }
    open_usage
}

fn line_ink(tone: Tone, source: &Source, roles: &crate::theme::Roles) -> egui::Color32 {
    match (tone, source) {
        (Tone::Normal, Source::Read(_)) => roles.tx,
        (tone, _) => super::surface::ink(tone, roles),
    }
}
