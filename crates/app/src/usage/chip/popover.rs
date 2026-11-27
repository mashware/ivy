//! The summary that opens under the chip: one tab per agent with a small bar of its 5-hour use, a
//! header with when it was read, each window with its use, reset and pace, and a way into AI
//! usage. It floats like a menu but is not modal, by decision (#283): it takes no keys, claims
//! the pointer over its own rectangle only and does not dim the consoles, since only modal
//! surfaces may. `summary_tests.rs` holds it to that.

use super::summary::{Body, Panel, Row, describe};
use super::surface::{dot, ink};
use super::{Agent, FIVE_HOURS, STALE_AFTER, Source, Tone, tone_of};
use crate::{Window, theme::Roles};
use eframe::egui::{self, Align2, Color32, FontId, Painter, Rect, Sense, Stroke, pos2, vec2};

const WIDTH: f32 = 380.0;
/// The tab row's padding and the gap between its two tabs.
const TABS_PAD: f32 = 10.0;
const TAB_HEIGHT: f32 = 44.0;
/// The sections' side and vertical padding.
const SIDE: f32 = 16.0;
const SECTION: f32 = 12.0;
const LINE: f32 = 16.0;
/// A window's block: name, bar, figures and, where known, pace.
const BAR: f32 = 6.0;
const GAP: f32 = 6.0;
const BETWEEN: f32 = 14.0;
const FOOTER_ROW: f32 = 36.0;
const FOOTER_PAD: f32 = 6.0;

fn proportional(size: f32) -> FontId {
    FontId::proportional(size)
}

/// The header's height: name, «Updated …» and the note when there is one.
fn header_height(panel: &Panel) -> f32 {
    let lines =
        20.0 + if panel.updated.is_some() {
            2.0 + LINE
        } else {
            0.0
        } + if panel.note.is_some() {
            2.0 + LINE
        } else {
            0.0
        };
    4.0 + lines + SECTION
}

fn row_height(row: &Row) -> f32 {
    LINE + GAP + BAR + GAP + LINE + if row.pace.is_some() { 4.0 + LINE } else { 0.0 }
}

fn body_height(panel: &Panel) -> f32 {
    let inner = match &panel.body {
        Body::Message { hint, .. } => LINE + if hint.is_some() { 4.0 + LINE } else { 0.0 },
        Body::Windows(rows) => {
            rows.iter().map(row_height).sum::<f32>() + BETWEEN * rows.len().saturating_sub(1) as f32
        }
    };
    2.0 * SECTION + inner
}

/// Paints the summary under `chip` while it is open; returns whether «Open AI usage» was chosen.
/// Without a chip this frame — a compact bar, or nothing enabled — it closes.
pub(crate) fn show(window: &mut Window, ctx: &egui::Context, chip: Option<Rect>) -> bool {
    let state = &mut window.usage.chip;
    state.summary = None;
    let (Some(chip), true, Some(claude), Some(codex)) = (
        chip,
        state.popover,
        state.claude.clone(),
        state.codex.clone(),
    ) else {
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
    let now = chrono::Utc::now().timestamp();
    let source = match tab {
        Agent::Claude => &claude,
        Agent::Codex => &codex,
    };
    let panel = describe(tab, source, now, &chrono::Local);
    let height = 2.0 * TABS_PAD
        + TAB_HEIGHT
        + header_height(&panel)
        + 1.0
        + body_height(&panel)
        + 1.0
        + 2.0 * FOOTER_PAD
        + FOOTER_ROW;
    let screen = ctx.content_rect();
    let left = (chip.right() - WIDTH).clamp(screen.left(), (screen.right() - WIDTH).max(0.0));
    let rect = Rect::from_min_size(
        pos2(left, chip.bottom() + 6.0),
        vec2(WIDTH.min(screen.width()), height),
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
    let paint = Paint {
        painter: &painter,
        roles: &roles,
        rect,
    };
    let mut chosen = None;
    let tab_width = (rect.width() - 2.0 * TABS_PAD - GAP) / 2.0;
    for (index, (agent, source)) in [(Agent::Claude, &claude), (Agent::Codex, &codex)]
        .into_iter()
        .enumerate()
    {
        let target = Rect::from_min_size(
            pos2(
                rect.left() + TABS_PAD + index as f32 * (tab_width + GAP),
                rect.top() + TABS_PAD,
            ),
            vec2(tab_width, TAB_HEIGHT),
        );
        let response = ui.interact(target, id.with(agent.name()), Sense::CLICK);
        let mini = mini_bar(source, now);
        paint.tab(
            target,
            agent,
            agent == tab,
            response.hovered(),
            mini,
            window,
        );
        if response.clicked() {
            chosen = Some(agent);
        }
    }
    let mut y = rect.top() + 2.0 * TABS_PAD + TAB_HEIGHT;
    y = paint.header(y, tab, &panel);
    paint.rule(y);
    y = paint.body(y + 1.0, tab, &panel, window);
    paint.rule(y);
    let row = Rect::from_min_size(
        pos2(rect.left() + FOOTER_PAD, y + 1.0 + FOOTER_PAD),
        vec2(rect.width() - 2.0 * FOOTER_PAD, FOOTER_ROW),
    );
    let response = ui.interact(row, id.with("open"), Sense::CLICK);
    paint.footer(row, response.hovered());
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

/// A tab's small bar: its 5-hour use and tone, from a current reading only.
fn mini_bar(source: &Source, now: i64) -> Option<(f64, Tone)> {
    let reading = source.reading()?;
    if now - reading.observed_at > STALE_AFTER {
        return None;
    }
    let window = reading
        .window(FIVE_HOURS)
        .filter(|window| !window.expired)?;
    let used = window.used?;
    Some((used, tone_of(used)))
}

struct Paint<'a> {
    painter: &'a Painter,
    roles: &'a Roles,
    rect: Rect,
}

impl Paint<'_> {
    fn text(&self, at: egui::Pos2, align: Align2, text: &str, font: FontId, color: Color32) {
        self.painter.text(at, align, text, font, color);
    }

    fn rule(&self, y: f32) {
        self.painter.line_segment(
            [
                pos2(self.rect.left(), y + 0.5),
                pos2(self.rect.right(), y + 0.5),
            ],
            Stroke::new(1.0, self.roles.line),
        );
    }

    /// A bar of `height` across `span`, filled to `used` percent in `fill`.
    fn bar(&self, span: Rect, used: Option<f64>, fill: Color32) {
        let radius = span.height() / 2.0;
        self.painter.rect_filled(span, radius, self.roles.line);
        if let Some(used) = used {
            let share = (used.clamp(0.0, 100.0) / 100.0) as f32;
            let filled = Rect::from_min_size(span.min, vec2(span.width() * share, span.height()));
            self.painter.rect_filled(filled, radius, fill);
        }
    }

    fn fill(&self, tone: Tone, agent: Agent, window: &Window) -> Color32 {
        ink(tone, self.roles, dot(agent, &window.theme))
    }

    fn tab(
        &self,
        target: Rect,
        agent: Agent,
        on: bool,
        hovered: bool,
        mini: Option<(f64, Tone)>,
        window: &Window,
    ) {
        if on || hovered {
            self.painter.rect_filled(
                target,
                crate::theme::metrics::RADIUS_CONTROL,
                self.roles.hover,
            );
        }
        let label = agent.name();
        let font = proportional(13.0);
        let width = self
            .painter
            .layout_no_wrap(label.into(), font.clone(), self.roles.tx)
            .size()
            .x;
        let start = target.center().x - (8.0 + 6.0 + width) / 2.0;
        let line = target.top() + 8.0 + 8.0;
        self.painter
            .circle_filled(pos2(start + 4.0, line), 4.0, dot(agent, &window.theme));
        let color = if on { self.roles.tx } else { self.roles.tx2 };
        self.text(
            pos2(start + 14.0, line),
            Align2::LEFT_CENTER,
            label,
            font,
            color,
        );
        let span = Rect::from_min_size(
            pos2(target.left() + 10.0, target.bottom() - 8.0 - 3.0),
            vec2(target.width() - 20.0, 3.0),
        );
        let (used, tone) = mini.map_or((None, Tone::Stale), |(used, tone)| (Some(used), tone));
        self.bar(span, used, self.fill(tone, agent, window));
    }

    fn header(&self, top: f32, agent: Agent, panel: &Panel) -> f32 {
        let left = self.rect.left() + SIDE;
        let mut y = top + 4.0;
        self.text(
            pos2(left, y + 10.0),
            Align2::LEFT_CENTER,
            agent.name(),
            proportional(15.0),
            self.roles.tx,
        );
        y += 20.0;
        if let Some(updated) = &panel.updated {
            y += 2.0;
            self.text(
                pos2(left, y + LINE / 2.0),
                Align2::LEFT_CENTER,
                updated,
                proportional(12.0),
                self.roles.tx2,
            );
            y += LINE;
        }
        if let Some(note) = &panel.note {
            y += 2.0;
            self.text(
                pos2(left, y + LINE / 2.0),
                Align2::LEFT_CENTER,
                note,
                proportional(12.0),
                self.roles.wait,
            );
            y += LINE;
        }
        y + SECTION
    }

    fn body(&self, top: f32, agent: Agent, panel: &Panel, window: &Window) -> f32 {
        let left = self.rect.left() + SIDE;
        let right = self.rect.right() - SIDE;
        let mut y = top + SECTION;
        match &panel.body {
            Body::Message { text, hint, tone } => {
                self.text(
                    pos2(left, y + LINE / 2.0),
                    Align2::LEFT_CENTER,
                    text,
                    proportional(13.0),
                    ink(*tone, self.roles, self.roles.tx),
                );
                y += LINE;
                if let Some(hint) = hint {
                    y += 4.0;
                    self.text(
                        pos2(left, y + LINE / 2.0),
                        Align2::LEFT_CENTER,
                        hint,
                        proportional(12.0),
                        self.roles.tx2,
                    );
                    y += LINE;
                }
            }
            Body::Windows(rows) => {
                for (index, row) in rows.iter().enumerate() {
                    if index > 0 {
                        y += BETWEEN;
                    }
                    self.text(
                        pos2(left, y + LINE / 2.0),
                        Align2::LEFT_CENTER,
                        &row.name,
                        proportional(13.0),
                        self.roles.tx,
                    );
                    y += LINE + GAP;
                    let span = Rect::from_min_max(pos2(left, y), pos2(right, y + BAR));
                    self.bar(span, row.used, self.fill(row.tone, agent, window));
                    y += BAR + GAP;
                    self.text(
                        pos2(left, y + LINE / 2.0),
                        Align2::LEFT_CENTER,
                        &row.used_text,
                        super::surface::font(),
                        ink(row.tone, self.roles, self.roles.tx),
                    );
                    if let Some(reset) = &row.reset {
                        self.text(
                            pos2(right, y + LINE / 2.0),
                            Align2::RIGHT_CENTER,
                            reset,
                            proportional(12.0),
                            self.roles.tx2,
                        );
                    }
                    y += LINE;
                    if let Some(pace) = &row.pace {
                        y += 4.0;
                        self.text(
                            pos2(left, y + LINE / 2.0),
                            Align2::LEFT_CENTER,
                            pace,
                            proportional(12.0),
                            self.roles.tx2,
                        );
                        y += LINE;
                    }
                }
            }
        }
        y + SECTION
    }

    /// «Open AI usage» as a menu row, with the chart the design gives it.
    fn footer(&self, row: Rect, hovered: bool) {
        if hovered {
            self.painter
                .rect_filled(row, crate::theme::metrics::RADIUS_CONTROL, self.roles.hover);
        }
        let x = row.left() + 10.0;
        let base = row.center().y + 5.0;
        let stroke = Stroke::new(1.5, self.roles.tx2);
        for (offset, height) in [(3.0, 5.0), (8.0, 10.0), (13.0, 7.0)] {
            self.painter.line_segment(
                [pos2(x + offset, base), pos2(x + offset, base - height)],
                stroke,
            );
        }
        self.text(
            pos2(x + 16.0 + 10.0, row.center().y),
            Align2::LEFT_CENTER,
            "Open AI usage",
            proportional(13.0),
            self.roles.tx,
        );
    }
}
