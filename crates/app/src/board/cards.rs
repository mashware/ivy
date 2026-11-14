//! Each Board card owns its reading state and the targets its last painting disclosed.
use super::{
    model::Entry,
    observation::{Column, Heat, heat},
};
use crate::{
    painter::CellMetrics,
    panel::body::{self, Body},
    theme::Theme,
};
use eframe::egui::{self, Color32, Rect};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub(super) enum Command {
    Body(usize, body::Action),
    Expand(usize),
    Compact(usize),
}
#[derive(Default)]
pub(super) struct Reader {
    body: body::Reader,
    full: bool,
    opened: bool,
    cache: Option<(
        std::sync::Arc<panel_contract::Panel>,
        usize,
        Theme,
        u64,
        bool,
        Body,
    )>,
}
impl Reader {
    pub(super) fn apply(&mut self, command: &Command) {
        match command {
            Command::Body(_, action) => self.body.apply(action),
            Command::Expand(_) => self.full = !self.full,
            Command::Compact(_) => self.opened = !self.opened,
        }
        self.cache = None;
    }
}
pub(super) fn color(column: Column, theme: &Theme) -> Color32 {
    match column {
        Column::Wait => theme.attention_color(panel_contract::Attention::Wait, true),
        Column::Block => theme.attention_color(panel_contract::Attention::Block, true),
        Column::Working => theme.focus(),
        Column::Done => theme.attention_color(panel_contract::Attention::Done, true),
        Column::Idle => theme.divider(),
    }
}
fn clean(text: &str) -> String {
    text.chars()
        .map(|c| {
            if panel_contract::is_shown_as_itself(c) {
                c
            } else {
                ' '
            }
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    ui: &mut egui::Ui,
    entry: &Entry,
    column: Column,
    backed: bool,
    since: Option<Instant>,
    branch: Option<&str>,
    selected: bool,
    reader: &mut Reader,
    discoverer: &mut crate::fonts::Discoverer,
    theme: &Theme,
    timing: &mut crate::measure::Board,
    child_hits: &mut Vec<(Rect, Command)>,
) -> Rect {
    let ink = color(column, theme).gamma_multiply(if backed { 1.0 } else { 0.45 });
    let age = since.map(|at| Instant::now().saturating_duration_since(at));
    let warmth = age.map(heat).unwrap_or(Heat::Normal);
    let background = match warmth {
        Heat::Normal => theme.panel_background(),
        Heat::Warm => theme.panel_background().lerp_to_gamma(ink, 0.08),
        Heat::Hot => theme.panel_background().lerp_to_gamma(ink, 0.16),
    };
    let stroke = egui::Stroke::new(1.0, if selected { theme.focus() } else { ink });
    let frame = egui::Frame::new()
        .fill(background)
        .stroke(stroke)
        .corner_radius(8)
        .inner_margin(10);
    let response = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        let doc = entry.snapshot.document.as_deref();
        let title = doc
            .and_then(|d| {
                d.lines
                    .iter()
                    .find(|l| l.style == Some(panel_contract::Style::Title))
            })
            .map_or(entry.console_label.as_str(), |l| l.text.as_str());
        ui.horizontal(|ui| {
            let phase = doc.and_then(|d| d.phase.as_deref());
            let phase_width = if phase.is_some() {
                ui.available_width().min(110.0) * 0.4
            } else {
                0.0
            };
            let title_width =
                (ui.available_width() - phase_width - ui.spacing().item_spacing.x).max(1.0);
            ui.add_sized(
                egui::vec2(title_width, 20.0),
                egui::Label::new(egui::RichText::new(clean(title)).strong()).truncate(),
            )
            .on_hover_text(clean(title));
            if let Some(phase) = phase {
                ui.add_sized(
                    egui::vec2(phase_width, 20.0),
                    egui::Label::new(egui::RichText::new(clean(phase)).color(theme.focus()))
                        .truncate(),
                )
                .on_hover_text(clean(phase));
            }
        });
        if let Some(attention) = entry.snapshot.attention {
            let message = match attention.class {
                panel_contract::Attention::Wait => "it is waiting for you",
                panel_contract::Attention::Block => "stuck on something else, not you",
                panel_contract::Attention::Done => "it finished and nobody looked",
            };
            ui.colored_label(
                theme.attention_color(attention.class, attention.backed),
                format!(
                    "{} {message}",
                    crate::panel::attention_symbol(attention.class)
                ),
            );
        }
        if column == Column::Wait && !backed {
            ui.weak("Waiting state is unconfirmed");
        }
        let compact = matches!(column, Column::Done | Column::Idle) && !reader.opened;
        if entry.snapshot.outside {
            ui.colored_label(
                theme.panel_color(None, Some(panel_contract::Style::Warn)),
                "published from a process outside this console",
            );
        }
        if let Some(warning) = &entry.snapshot.warning {
            ui.colored_label(
                theme.panel_color(None, Some(panel_contract::Style::Warn)),
                clean(warning),
            );
        }
        if let Some(doc) = doc {
            if !compact {
                let metrics =
                    CellMetrics::measure(ui.ctx(), egui::TextStyle::Monospace.resolve(ui.style()))
                        .for_panel(ui.ctx());
                let width = (ui.available_width() / metrics.width()).max(1.0) as usize;
                let key = entry.snapshot.document.as_ref().unwrap();
                let started = Instant::now();
                if reader.cache.as_ref().is_none_or(|(id, w, t, r, full, _)| {
                    !std::sync::Arc::ptr_eq(id, key)
                        || *w != width
                        || t != theme
                        || *r != reader.body.revision()
                        || *full != reader.full
                }) {
                    reader.cache = Some((
                        key.clone(),
                        width,
                        *theme,
                        reader.body.revision(),
                        reader.full,
                        Body::layout(doc, &reader.body, width, theme, reader.full),
                    ));
                }
                timing.content_layout(started.elapsed());
                let body = &reader.cache.as_ref().unwrap().5;
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), body.height(&metrics)),
                    egui::Sense::hover(),
                );
                let started = Instant::now();
                let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                for (rect, action) in body.paint(&painter, rect, &metrics, theme, discoverer) {
                    let hint = match &action {
                        body::Action::Link(target) => target.as_str(),
                        body::Action::Fold(..) => "Toggle section",
                    };
                    ui.interact(
                        rect,
                        ui.id().with((entry.pane, rect.min.y.to_bits())),
                        egui::Sense::hover(),
                    )
                    .on_hover_text(hint);
                    child_hits.push((rect, Command::Body(entry.pane, action)));
                }
                timing.content_rows(body.rows());
                timing.content_paint(started.elapsed());
                if body.hidden > 0 || reader.full {
                    let label = if reader.full {
                        "Show less".to_owned()
                    } else {
                        format!("Show all {} lines", doc.lines.len())
                    };
                    let r = ui.add(
                        egui::Label::new(egui::RichText::new(label).color(theme.focus()))
                            .sense(egui::Sense::hover()),
                    );
                    if ui.clip_rect().contains_rect(r.rect) {
                        child_hits.push((r.rect, Command::Expand(entry.pane)));
                    }
                }
            }
        } else {
            ui.weak("publishes nothing");
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if let Some(branch) = branch {
                ui.weak(format!("⎇ {}", clean(branch)));
            }
            if let Some(age) = age {
                ui.weak(format!("observed wait {} min", age.as_secs() / 60));
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "{} · {}",
                        clean(&entry.console_label),
                        clean(&entry.tab_label)
                    ))
                    .weak(),
                )
                .truncate(),
            );
            if doc.is_some() && matches!(column, Column::Done | Column::Idle) {
                let r = ui.add(
                    egui::Label::new(
                        egui::RichText::new(if compact { "Show panel" } else { "Hide panel" })
                            .color(theme.focus()),
                    )
                    .sense(egui::Sense::hover()),
                );
                if ui.clip_rect().contains_rect(r.rect) {
                    child_hits.push((r.rect, Command::Compact(entry.pane)));
                }
            }
        });
    });
    let rect = response.response.rect;
    if selected || warmth == Heat::Hot || ui.rect_contains_pointer(rect) {
        ui.painter().rect_stroke(
            rect,
            8.0,
            egui::Stroke::new(
                if selected || warmth == Heat::Hot {
                    2.0
                } else {
                    1.0
                },
                if selected { theme.focus() } else { ink },
            ),
            egui::StrokeKind::Inside,
        );
    }
    rect
}

pub(super) fn age_refresh(since: Option<Instant>, now: Instant) -> Option<Duration> {
    since.map(|at| {
        Duration::from_secs(60)
            - Duration::from_secs(now.saturating_duration_since(at).as_secs() % 60)
    })
}
