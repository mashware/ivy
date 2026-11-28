//! The chip in the title bar and the state of the summary that opens under it. Neither is modal,
//! by decision (#283): a glance at the allowances must not stop anybody typing, and only modal
//! surfaces may dim the consoles.

use super::{Agent, Item, Part, Source, Tone, items, templates, width};
use crate::{
    Window,
    theme::{Roles, Theme},
};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke};

/// The chip's text size; the slot is measured with the same font it is painted with.
const FONT: f32 = 12.0;
/// Claude's own clay; Codex takes the theme's blue, so it follows the palette.
const CLAUDE: Color32 = Color32::from_rgb(0xd9, 0x77, 0x57);

pub(super) fn font() -> FontId {
    FontId::monospace(FONT)
}

pub(super) fn dot(agent: Agent, theme: &Theme) -> Color32 {
    match agent {
        Agent::Claude => CLAUDE,
        Agent::Codex => theme.color(term_core::TermColor::Indexed(4)),
    }
}

/// A tone's ink for text; `normal` is what a figure in good standing is painted with.
pub(super) fn ink(tone: Tone, roles: &Roles, normal: Color32) -> Color32 {
    match tone {
        Tone::Normal => normal,
        Tone::Amber => roles.wait,
        Tone::Red | Tone::Failed => roles.block,
        Tone::Stale | Tone::Unavailable => roles.tx3,
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
    /// The live chip's width, which the box hugs inside the slot.
    live: f32,
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
        self.tab = None;
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
        self.live = width(&self.items, &mut |text| measure(ctx, text));
        if self.items.is_empty() {
            self.close_summary();
        } else if let Some(at) = super::next_change(&[&claude, &codex], now) {
            // A window at rest paints nothing, so the stale flip and a reset need their own frame.
            let wait = u64::try_from(at - now).unwrap_or_default();
            ctx.request_repaint_after(std::time::Duration::from_secs(wait));
        }
        self.claude = Some(claude);
        self.codex = Some(codex);
    }

    /// The box the chip paints in `slot`: as wide as its text, against the slot's right edge so
    /// it sits by the Board.
    pub(crate) fn hugged(&self, slot: Rect) -> Rect {
        let width = self.live.min(slot.width());
        Rect::from_min_max(egui::pos2(slot.right() - width, slot.top()), slot.max)
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
pub(crate) fn paint(window: &mut Window, ui: &egui::Ui, slot: Rect) {
    if paint_chip(window, ui, slot) {
        let state = &mut window.usage.chip;
        state.popover = !state.popover;
        if !state.popover {
            state.tab = None;
        }
        ui.ctx().request_repaint();
    }
}

fn paint_chip(window: &Window, ui: &egui::Ui, slot: Rect) -> bool {
    let state = &window.usage.chip;
    let rect = state.hugged(slot);
    if state.items.is_empty() || !rect.is_positive() {
        return false;
    }
    let roles = crate::ui::style::roles(ui.ctx(), &window.theme);
    let response = ui
        .interact(rect, ui.id().with("usage-chip"), Sense::CLICK)
        .on_hover_text("AI usage allowances");
    let painter = ui.painter_at(rect.expand(1.0));
    let (fill, border) = if state.popover {
        (roles.hover, roles.line2)
    } else if response.hovered() {
        (roles.hover, roles.line)
    } else {
        (Color32::TRANSPARENT, roles.line)
    };
    painter.rect(
        rect,
        6,
        fill,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    let y = rect.center().y;
    let mut x = rect.left() + super::PAD;
    let text = |x: &mut f32, text: &str, color: Color32| {
        *x += painter
            .text(
                egui::pos2(*x, y),
                egui::Align2::LEFT_CENTER,
                text,
                font(),
                color,
            )
            .width();
    };
    for (index, item) in state.items.iter().enumerate() {
        if index > 0 {
            x += super::ITEM_GAP;
            text(&mut x, super::SEPARATOR, roles.line2);
            x += super::ITEM_GAP;
        }
        if index == 0 || state.items[index - 1].agent != item.agent {
            painter.circle_filled(
                egui::pos2(x + super::DOT / 2.0, y),
                super::DOT / 2.0,
                dot(item.agent, &window.theme),
            );
            x += super::DOT + super::DOT_GAP;
        }
        // In good standing the labels are dim and the figures plain; amber and red colour the
        // whole item, and a stale one is grey throughout.
        for (piece, part) in item.parts() {
            let color = match (item.tone, part) {
                (Tone::Normal, Part::Value) => roles.tx,
                (Tone::Normal, _) => roles.tx2,
                (tone, _) => ink(tone, &roles, roles.tx),
            };
            text(&mut x, piece, color);
        }
    }
    response.clicked()
}
