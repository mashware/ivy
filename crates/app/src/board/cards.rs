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
    /// A press on the Board's top bar, which belongs to no card.
    Bar(super::bar::Action),
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
            Command::Bar(_) => return,
        }
        self.cache = None;
    }
}
pub(super) fn color(column: Column, theme: &Theme) -> Color32 {
    match column {
        Column::Wait => theme.color(term_core::TermColor::Indexed(3)),
        Column::Block => theme.color(term_core::TermColor::Indexed(1)),
        // ANSI blue as D93's owner chose, not `focus`: since D94 focus is ANSI green, which would
        // paint Working the colour of Finished.
        Column::Working => theme.color(term_core::TermColor::Indexed(4)),
        Column::Done => theme.color(term_core::TermColor::Indexed(2)),
        Column::Idle => theme.divider(),
    }
}
pub(super) fn title(entry: &Entry) -> &str {
    entry
        .snapshot
        .document
        .as_deref()
        .and_then(|d| {
            d.lines
                .iter()
                .find(|l| l.style == Some(panel_contract::Style::Title))
        })
        .map_or(entry.console_label.as_str(), |l| l.text.as_str())
}

pub(super) fn clean(text: &str) -> String {
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
        Heat::Warm => theme.background().lerp_to_gamma(ink, 20.0 / 255.0),
        Heat::Hot => theme.background().lerp_to_gamma(ink, 41.0 / 255.0),
    };
    let stroke = egui::Stroke::new(1.0, if selected { theme.focus() } else { ink });
    let frame = egui::Frame::new()
        .fill(background)
        .stroke(stroke)
        .corner_radius(8)
        .inner_margin(10);
    let response = frame.show(ui, |ui| {
        ui.style_mut().interaction.selectable_labels = false;
        ui.set_width(ui.available_width());
        let doc = entry.snapshot.document.as_deref();
        ui.horizontal(|ui| {
            let phase = doc.and_then(|d| d.phase.as_deref());
            let available = ui.available_width();
            let phase_width = phase.map_or(0.0, |s| {
                (s.chars().count() as f32 * 7.3)
                    .min(72.0)
                    .min(available * 0.25)
            });
            let navigation_width = 16.0_f32.min(available * 0.1);
            let title_width =
                (available - phase_width - navigation_width - 2.0 * ui.spacing().item_spacing.x)
                    .max(1.0);
            ui.allocate_ui_with_layout(
                egui::vec2(title_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_width(title_width);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(clean(title(entry))).size(13.0).strong(),
                        )
                        .truncate(),
                    )
                    .on_hover_text(format!(
                        "{}\n{} · {}",
                        clean(title(entry)),
                        clean(&entry.console_label),
                        clean(&entry.tab_label)
                    ));
                },
            );
            if let Some(phase) = phase {
                ui.add_sized(
                    egui::vec2(phase_width, 20.0),
                    egui::Label::new(egui::RichText::new(clean(phase)).color(theme.focus()))
                        .truncate()
                        .halign(egui::Align::Min),
                )
                .on_hover_text(clean(phase));
            }
            super::style::console_navigation(ui, navigation_width, theme);
        });
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
        if let Some(doc) = doc
            && !compact
        {
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
                .on_hover_cursor(egui::CursorIcon::PointingHand)
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
                let r = ui
                    .add(
                        egui::Label::new(egui::RichText::new(label).color(theme.focus()))
                            .sense(egui::Sense::hover()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if ui.clip_rect().contains_rect(r.rect) {
                    child_hits.push((r.rect, Command::Expand(entry.pane)));
                }
            }
        }
        let has_toggle = doc.is_some() && matches!(column, Column::Done | Column::Idle);
        if branch.is_some() || doc.is_none() || has_toggle {
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_toggle {
                        let r = ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(if compact {
                                        "Show panel"
                                    } else {
                                        "Hide panel"
                                    })
                                    .size(11.0)
                                    .color(theme.focus()),
                                )
                                .sense(egui::Sense::hover()),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if ui.clip_rect().contains_rect(r.rect) {
                            child_hits.push((r.rect, Command::Compact(entry.pane)));
                        }
                    }
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 18.0),
                        egui::Sense::hover(),
                    );
                    if let Some(branch) = branch {
                        super::style::branch(ui, rect, &clean(branch), theme);
                    } else if doc.is_none() {
                        ui.put(
                            rect,
                            egui::Label::new(
                                egui::RichText::new("publishes nothing")
                                    .size(11.0)
                                    .color(super::style::dim(theme)),
                            )
                            .halign(egui::Align::Max),
                        );
                    }
                });
            });
        }
    });
    let rect = response.response.rect;
    if let Some(age) = age {
        ui.interact(
            rect,
            ui.id().with((entry.pane, "observed-age")),
            egui::Sense::hover(),
        )
        .on_hover_text(format!(
            "observed wait {} min; time before observation is unknown",
            age.as_secs() / 60
        ));
    }
    let hovered = ui.rect_contains_pointer(rect);
    if selected || warmth == Heat::Hot || hovered {
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

#[cfg(test)]
mod tests {
    use super::*;
    /// Working stays the owner's blue and Finished green (D93), written out: since D94 the focus is
    /// green, so a column taken from it would paint working consoles the colour of finished ones.
    #[test]
    fn working_and_finished_columns_keep_their_own_colours() {
        let theme = Theme::default();
        assert_eq!(
            color(Column::Working, &theme),
            Color32::from_rgb(0x72, 0xB7, 0xF2)
        );
        assert_eq!(
            color(Column::Done, &theme),
            Color32::from_rgb(0x3C, 0xC3, 0x84)
        );
    }
    #[test]
    fn short_and_long_titles_start_at_the_same_left_padding() {
        let mut offsets = Vec::new();
        let mut neighbors = Vec::new();
        for title in [
            "[DEMO] Export CSV",
            "[DEMO] A much longer title that must be truncated within the card",
        ] {
            let ctx = egui::Context::default();
            let entry = Entry {
                pane: 1, console_label:"Console".into(), tab_label:"Tab".into(),
                snapshot: crate::panel::summary::Snapshot {
                    document:Some(std::sync::Arc::new(panel_contract::parse_document(&serde_json::json!({"phase":"design","lines":[{"text":title,"style":"title"}]}).to_string()).unwrap())),
                    warning:None,attention:None,outside:false,
                },
            };
            let mut card = Rect::NOTHING;
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.set_width(400.0);
                super::super::style::apply(ui, &Theme::default());
                card = paint(
                    ui,
                    &entry,
                    Column::Wait,
                    true,
                    None,
                    Some("main"),
                    false,
                    &mut Reader::default(),
                    &mut crate::fonts::Discoverer::default(),
                    &Theme::default(),
                    &mut crate::measure::Board::default(),
                    &mut Vec::new(),
                );
            });
            let start = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == title => {
                        Some(t.pos.x + t.galley.pos_from_cursor(egui::text::CCursor::new(0)).left())
                    }
                    _ => None,
                })
                .expect("card title painted");
            offsets.push(start - card.left());
            let position = |needle: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Text(t) if t.galley.text() == needle => Some(
                            t.pos.x + t.galley.pos_from_cursor(egui::text::CCursor::new(0)).left(),
                        ),
                        _ => None,
                    })
                    .expect("header field painted")
            };
            let navigation = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::LineSegment { points, stroke }
                        if stroke.color == Theme::default().focus()
                            && points[0].y == points[1].y
                            && points[0].y < card.top() + 30.0 =>
                    {
                        Some((points[0].x + points[1].x) / 2.0)
                    }
                    _ => None,
                })
                .expect("console navigation arrow painted");
            neighbors.push((position("design"), navigation));
            output.drop_without_applying_deltas();
        }
        assert!(
            (offsets[0] - offsets[1]).abs() < 0.5,
            "short/long title left offsets differ: {offsets:?}"
        );
        assert!(
            (neighbors[0].0 - neighbors[1].0).abs() < 0.5
                && (neighbors[0].1 - neighbors[1].1).abs() < 0.5,
            "phase/navigation shifted with title length: {neighbors:?}"
        );
    }

    #[test]
    fn long_phases_and_navigation_fit_the_narrow_finished_and_idle_columns() {
        let widths = super::super::layout::column_widths(1344.0, 8.0);
        for (column, width) in [(Column::Done, widths[3]), (Column::Idle, widths[4])] {
            let ctx = egui::Context::default();
            let entry = Entry {
                pane: 1,
                console_label: "Console".into(),
                tab_label: "Tab".into(),
                snapshot: crate::panel::summary::Snapshot {
                    document: Some(std::sync::Arc::new(panel_contract::parse_document(r#"{"phase":"implementation","lines":[{"text":"A long producer title for a narrow column","style":"title"}]}"#).unwrap())),
                    warning: None,
                    attention: None,
                    outside: false,
                },
            };
            ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.set_width(width);
                super::super::style::apply(ui, &Theme::default());
                let rect = paint(
                    ui,
                    &entry,
                    column,
                    true,
                    None,
                    Some("main"),
                    false,
                    &mut Reader::default(),
                    &mut crate::fonts::Discoverer::default(),
                    &Theme::default(),
                    &mut crate::measure::Board::default(),
                    &mut Vec::new(),
                );
                assert!(
                    rect.width() <= width + 0.1,
                    "{column:?} expanded from {width} to {}",
                    rect.width()
                );
            })
            .drop_without_applying_deltas();
        }
    }
}
