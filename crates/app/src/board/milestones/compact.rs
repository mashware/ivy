//! A literal excerpt; line marks choose presentation, never the console's attention.
use crate::{
    board::{cards, model::Entry, observation::Column},
    theme::Roles,
};
use eframe::egui::{self, FontId, Rect, Stroke, pos2, vec2};
use panel_contract::{Mark, Style};
use std::time::{Instant, SystemTime};

pub(super) fn excerpt(panel: &panel_contract::Panel) -> Vec<&panel_contract::Line> {
    let mut lines: Vec<_> = panel
        .lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.style != Some(Style::Title) && !line.text.trim().is_empty())
        .collect();
    lines.sort_by_key(|(i, line)| (!matches!(line.mark, Some(Mark::Wait | Mark::Block)), *i));
    lines.into_iter().take(3).map(|(_, line)| line).collect()
}
#[allow(clippy::too_many_arguments)]
pub(in crate::board) fn paint(
    ui: &mut egui::Ui,
    entry: &Entry,
    column: Column,
    backed: bool,
    since: Option<Instant>,
    branch: Option<&str>,
    selected: bool,
    roles: &Roles,
    timing: &mut crate::measure::Board,
) -> Rect {
    let started = Instant::now();
    let width = ui.available_width().max(1.0);
    let doc = entry.snapshot.document.as_deref();
    let stale = doc.is_some_and(|d| d.is_stale_at(SystemTime::now()));
    let rows = doc.map(excerpt).unwrap_or_default();
    let warning = entry.snapshot.outside
        || entry.snapshot.warning.is_some()
        || (column == Column::Wait && !backed);
    let claim = crate::board::filter::claims(column);
    let height = 74.0
        + if claim { 24.0 } else { 0.0 }
        + rows.len() as f32 * 38.0
        + if warning { 32.0 } else { 0.0 }
        + if stale { 18.0 } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
    timing.content_layout(started.elapsed());
    let started = Instant::now();
    let hovered = ui.rect_contains_pointer(rect);
    ui.painter().rect(
        rect,
        8,
        if hovered { roles.hover } else { roles.panel },
        Stroke::new(1.0, if selected { roles.accent } else { roles.line }),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(10.0);
    let color = match column {
        Column::Wait => roles.wait,
        Column::Block => roles.block,
        Column::Done => roles.done,
        _ => roles.tx3,
    };
    let ink = |c: egui::Color32| c;
    let mut y = inner.top();
    if claim {
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, vec2(width, 28.0)),
            8,
            color.gamma_multiply(0.15),
        );
        let label = if column == Column::Done {
            "Finished · unseen"
        } else {
            column.name()
        };
        let class = match column {
            Column::Wait => panel_contract::Attention::Wait,
            Column::Block => panel_contract::Attention::Block,
            _ => panel_contract::Attention::Done,
        };
        crate::ui::claim_pill::paint(
            ui.painter(),
            Rect::from_min_size(pos2(inner.left(), y - 3.0), vec2(18.0, 18.0)),
            crate::panel::AttentionState { class, backed },
            roles,
        );
        let age = since.map(|since| format!("{} min", since.elapsed().as_secs() / 60));
        let age_width = if age.is_some() { 46.0 } else { 0.0 };
        super::strip::text(
            ui,
            Rect::from_min_size(
                pos2(inner.left() + 24.0, y),
                vec2((inner.width() - 24.0 - age_width).max(0.0), 18.0),
            ),
            label,
            FontId::proportional(12.0),
            ink(color),
            1,
        );
        if let Some(age) = age {
            super::strip::text(
                ui,
                Rect::from_min_size(
                    pos2(inner.right() - age_width, y + 1.0),
                    vec2(age_width, 16.0),
                ),
                &age,
                FontId::proportional(11.0),
                roles.tx3,
                1,
            );
            ui.interact(
                rect,
                ui.id().with((entry.pane, "compact-age")),
                egui::Sense::hover(),
            )
            .on_hover_text(format!(
                "Observed {age}; time before observation is unknown"
            ));
        }
        y += 24.0;
    }
    super::strip::text(
        ui,
        Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 34.0)),
        cards::title(entry),
        FontId::monospace(12.5),
        ink(roles.tx),
        2,
    );
    y += 36.0;
    for line in &rows {
        let color = match line.mark {
            Some(Mark::Wait) => roles.wait,
            Some(Mark::Block) => roles.block,
            _ => roles.tx2,
        };
        let label = line.reference.as_deref().unwrap_or("");
        let prefix = if label.is_empty() {
            String::new()
        } else {
            format!("{label}  ")
        };
        super::strip::text(
            ui,
            Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 36.0)),
            &format!("{prefix}{}", line.text),
            FontId::monospace(11.5),
            ink(color),
            2,
        );
        y += 38.0;
    }
    if warning {
        let message = entry
            .snapshot
            .warning
            .as_deref()
            .unwrap_or(if entry.snapshot.outside {
                "published from a process outside this console"
            } else {
                "Waiting state is unconfirmed"
            });
        super::strip::text(
            ui,
            Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 30.0)),
            message,
            FontId::proportional(11.0),
            ink(roles.wait),
            2,
        );
        y += 32.0;
    }
    if stale {
        let age = doc
            .and_then(|d| d.written_at)
            .and_then(|at| SystemTime::now().duration_since(at).ok())
            .map(|d| format!("Last state {} h ago", d.as_secs() / 3600))
            .unwrap_or_else(|| "Last state is old".into());
        super::strip::text(
            ui,
            Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 18.0)),
            &age,
            FontId::proportional(11.0),
            roles.tx3,
            1,
        );
    }
    if doc.is_none() {
        cards::dashed(ui.painter(), rect, roles.line2);
        super::strip::text(
            ui,
            Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 18.0)),
            "No published state · Connect agent",
            FontId::proportional(11.0),
            roles.tx3,
            1,
        );
    }
    let phase = doc.and_then(|d| d.phase.as_deref()).unwrap_or("");
    let footer = Rect::from_min_size(
        pos2(inner.left(), rect.bottom() - 23.0),
        vec2(inner.width(), 16.0),
    );
    if branch.is_some() {
        crate::ui::icons::paint(
            ui.painter(),
            footer.min + vec2(6.0, 7.0),
            crate::ui::icons::Icon::Branch,
            12.0,
            ink(roles.tx3),
        );
    }
    let phase_width = (phase.chars().count() as f32 * 6.5 + 10.0).min(footer.width() * 0.4);
    super::strip::text(
        ui,
        Rect::from_min_max(
            footer.min + vec2(18.0, 0.0),
            footer.max - vec2(phase_width, 0.0),
        ),
        &branch.map(str::to_owned).unwrap_or_default(),
        FontId::monospace(11.0),
        ink(roles.tx3),
        1,
    );
    super::strip::text(
        ui,
        Rect::from_min_size(
            pos2(footer.right() - phase_width, footer.top()),
            vec2(phase_width, 16.0),
        ),
        phase,
        FontId::proportional(11.0),
        ink(roles.tx3),
        1,
    );
    if stale {
        ui.painter()
            .rect_filled(rect, 8, roles.base.gamma_multiply(0.45));
    }
    if selected {
        ui.painter().rect_stroke(
            rect,
            8,
            Stroke::new(2.0, roles.accent),
            egui::StrokeKind::Inside,
        );
    }
    ui.interact(
        rect,
        ui.id().with((entry.pane, "compact-details")),
        egui::Sense::hover(),
    )
    .on_hover_text(format!(
        "{}\n{} · {}\nGo to console",
        cards::clean(cards::title(entry)),
        cards::clean(&entry.console_label),
        cards::clean(&entry.tab_label)
    ));
    timing.content_rows(rows.len());
    timing.content_paint(started.elapsed());
    rect
}
