//! The chip in the title bar and the state of the summary that opens under it. Neither is modal,
//! by decision (#283): a glance at the allowances must not stop anybody typing, and only modal
//! surfaces may dim the consoles.

use super::{Agent, Item, Source, Tone, items, templates, width};
use crate::{Window, theme::Roles};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke};

/// The chip's text size; the slot is measured with the same font it is painted with.
const FONT: f32 = 12.0;
const CLAUDE: Color32 = Color32::from_rgb(0xd9, 0x77, 0x57);
const CODEX: Color32 = Color32::from_rgb(0x10, 0xa3, 0x7f);

fn font() -> FontId {
    FontId::proportional(FONT)
}

pub(super) fn dot(agent: Agent) -> Color32 {
    match agent {
        Agent::Claude => CLAUDE,
        Agent::Codex => CODEX,
    }
}

pub(super) fn ink(tone: Tone, roles: &Roles) -> Color32 {
    match tone {
        Tone::Normal => roles.tx2,
        Tone::Amber => roles.wait,
        Tone::Red => roles.block,
        Tone::Stale => roles.tx3,
    }
}

/// What the chip shows this frame and the slot it was given.
#[derive(Default)]
pub(crate) struct Chip {
    pub(super) claude: Option<Source>,
    pub(super) codex: Option<Source>,
    items: Vec<Item>,
    /// The slot's width, from the widest text of the enabled items; zero when there is no chip.
    slot: f32,
    measured_for: Option<(bool, bool, u32)>,
    pub(super) popover: bool,
    pub(super) tab: Option<Agent>,
    /// Where the summary fell this frame, for the title bar's pointer claim.
    pub(super) summary: Option<Rect>,
    /// Readings a test hands the chip in place of the window's sources.
    #[cfg(test)]
    pub(super) forced: Option<(Source, Source)>,
}

impl Chip {
    pub(crate) fn slot(&self) -> f32 {
        self.slot
    }

    pub(crate) fn summary(&self) -> Option<Rect> {
        self.summary
    }

    pub(crate) fn close_summary(&mut self) {
        self.popover = false;
        self.summary = None;
    }

    /// Rebuilds the items from this frame's readings, and the slot only when the enabled set or
    /// the scale changed: the slot follows what the chip may say, never what it says now.
    pub(super) fn update(&mut self, ctx: &egui::Context, claude: Source, codex: Source, now: i64) {
        self.items = items(&claude, &codex, now, &chrono::Local);
        let key = (
            claude.enabled(),
            codex.enabled(),
            ctx.pixels_per_point().to_bits(),
        );
        if self.measured_for != Some(key) {
            let templates = templates(&claude, &codex);
            self.slot = width(&templates, &mut |text| measure(ctx, text));
            self.measured_for = Some(key);
        }
        if self.items.is_empty() {
            self.popover = false;
        } else if let Some(at) = super::next_change(&[claude, codex], now) {
            // A window at rest paints nothing, so the stale flip and a reset need their own frame.
            let wait = u64::try_from(at - now).unwrap_or_default();
            ctx.request_repaint_after(std::time::Duration::from_secs(wait));
        }
        self.claude = Some(claude);
        self.codex = Some(codex);
    }
}

fn measure(ctx: &egui::Context, text: &str) -> f32 {
    ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font(), Color32::WHITE)
            .size()
            .x
    })
}

/// Paints the chip on its slot; a click opens the summary under it, or closes it.
pub(crate) fn paint(window: &mut Window, ui: &egui::Ui, rect: Rect) {
    if paint_chip(window, ui, rect) {
        let state = &mut window.usage.chip;
        state.popover = !state.popover;
        ui.ctx().request_repaint();
    }
}

fn paint_chip(window: &Window, ui: &egui::Ui, rect: Rect) -> bool {
    let state = &window.usage.chip;
    if state.items.is_empty() || !rect.is_positive() {
        return false;
    }
    let roles = crate::ui::style::roles(ui.ctx(), &window.theme);
    let response = ui
        .interact(rect, ui.id().with("usage-chip"), Sense::CLICK)
        .on_hover_text("AI usage allowances");
    let painter = ui.painter_at(rect.expand(1.0));
    let fill = if response.hovered() || state.popover {
        roles.hover
    } else {
        Color32::TRANSPARENT
    };
    painter.rect(
        rect,
        6,
        fill,
        Stroke::new(1.0, roles.line2),
        egui::StrokeKind::Inside,
    );
    let y = rect.center().y;
    let mut x = rect.left() + super::PAD;
    for (index, item) in state.items.iter().enumerate() {
        if index > 0 {
            x += painter
                .text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_CENTER,
                    super::SEPARATOR,
                    font(),
                    roles.tx3,
                )
                .width();
        }
        if index == 0 || state.items[index - 1].agent != item.agent {
            painter.circle_filled(
                egui::pos2(x + super::DOT / 2.0, y),
                super::DOT / 2.0,
                dot(item.agent),
            );
            x += super::DOT + super::DOT_GAP;
        }
        x += painter
            .text(
                egui::pos2(x, y),
                egui::Align2::LEFT_CENTER,
                &item.text,
                font(),
                ink(item.tone, &roles),
            )
            .width();
    }
    response.clicked()
}
