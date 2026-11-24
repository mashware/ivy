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
    Milestone(super::milestones::state::Action),
    Source(usize, std::path::PathBuf),
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
            Command::Bar(_) | Command::Milestone(_) | Command::Source(..) => return,
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
    roles: &crate::theme::Roles,
    timing: &mut crate::measure::Board,
    child_hits: &mut Vec<(Rect, Command)>,
) -> Rect {
    let ink = color(column, theme).gamma_multiply(if backed { 1.0 } else { 0.45 });
    let age = since.map(|at| Instant::now().saturating_duration_since(at));
    let warmth = age.map(heat).unwrap_or(Heat::Normal);
    let background = match warmth {
        Heat::Normal => roles.panel,
        Heat::Warm => roles.panel.lerp_to_gamma(ink, 20.0 / 255.0),
        Heat::Hot => roles.panel.lerp_to_gamma(ink, 41.0 / 255.0),
    };
    let doc = entry.snapshot.document.as_deref();
    // No published state is a dashed outline, painted after the frame: egui cannot dash a rounded
    // stroke, and the frame's own solid one would read as a published card. Its stroke stays one
    // point wide and transparent, because a frame grows by its stroke and selecting the card would
    // otherwise move every card below it.
    let stroke = egui::Stroke::new(
        1.0,
        match (selected, doc) {
            (true, _) => roles.accent,
            (false, Some(_)) => roles.line,
            (false, None) => Color32::TRANSPARENT,
        },
    );
    let frame = egui::Frame::new()
        .fill(background)
        .stroke(stroke)
        .corner_radius(crate::theme::metrics::RADIUS_CARD)
        .inner_margin(10);
    let response = frame.show(ui, |ui| {
        ui.style_mut().interaction.selectable_labels = false;
        ui.set_width(ui.available_width());
        state_row(ui, column, backed, age, theme, roles);
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
                // Only the part on screen: since the top bar and the key hint live inside the
                // Board's area, a row half under them would otherwise take their presses.
                let visible = rect.intersect(ui.clip_rect());
                if visible.is_positive() {
                    child_hits.push((visible, Command::Body(entry.pane, action)));
                }
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
    if doc.is_none() && !selected {
        dashed(ui.painter(), rect, roles.line2);
    }
    // An old state nobody refreshes is veiled rather than recoloured, so its claim still reads.
    if doc.is_some_and(|d| d.is_stale_at(std::time::SystemTime::now())) && !selected {
        ui.painter().rect_filled(
            rect,
            crate::theme::metrics::RADIUS_CARD,
            roles.base.gamma_multiply(0.45),
        );
    }
    if selected || warmth == Heat::Hot || hovered {
        ui.painter().rect_stroke(
            rect,
            crate::theme::metrics::RADIUS_CARD,
            egui::Stroke::new(
                if selected || warmth == Heat::Hot {
                    2.0
                } else {
                    1.0
                },
                if selected {
                    roles.accent
                } else if hovered && warmth != Heat::Hot {
                    roles.line2
                } else {
                    ink
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    rect
}

/// The card's effective state — the column «By state» would put it in — so «By milestone», which
/// has no state columns, still says which console waits. A claim (D18) is the kit's pill; working
/// and idle are the column's own mark. The age is the Board's observation, said as one (D93).
fn state_row(
    ui: &mut egui::Ui,
    column: Column,
    backed: bool,
    age: Option<Duration>,
    theme: &Theme,
    roles: &crate::theme::Roles,
) {
    use panel_contract::Attention;
    let (row, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), egui::Sense::hover());
    let slot = Rect::from_min_size(
        row.min,
        egui::vec2(crate::ui::claim_pill::SIZE, row.height()),
    );
    let class = match column {
        Column::Wait => Some((Attention::Wait, roles.wait)),
        Column::Block => Some((Attention::Block, roles.block)),
        Column::Done => Some((Attention::Done, roles.done)),
        Column::Working | Column::Idle => None,
    };
    let ink = match class {
        Some((class, ink)) => {
            let claim = crate::panel::AttentionState { class, backed };
            crate::ui::claim_pill::paint(ui.painter(), slot, claim, roles);
            if backed { ink } else { roles.tx3 }
        }
        None => {
            let ink = if column == Column::Working {
                color(column, theme)
            } else {
                roles.tx3
            };
            super::style::column_mark(ui.painter(), slot, column, ink);
            ink
        }
    };
    let font = egui::FontId::new(12.0, crate::fonts::ui_weight(ui.ctx(), 500));
    ui.painter().text(
        egui::pos2(slot.right() + 6.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        column.name(),
        font,
        ink,
    );
    if let Some(age) = age {
        ui.painter().text(
            egui::pos2(row.right(), row.center().y),
            egui::Align2::RIGHT_CENTER,
            format!("observed {} min", age.as_secs() / 60),
            egui::FontId::proportional(11.5),
            roles.tx3,
        );
    }
}

pub(super) fn dashed(painter: &egui::Painter, rect: Rect, color: Color32) {
    let r = rect.shrink(0.5);
    let inset = f32::from(crate::theme::metrics::RADIUS_CARD);
    let stroke = egui::Stroke::new(1.0, color);
    for (from, to) in [
        (
            egui::pos2(r.left() + inset, r.top()),
            egui::pos2(r.right() - inset, r.top()),
        ),
        (
            egui::pos2(r.left() + inset, r.bottom()),
            egui::pos2(r.right() - inset, r.bottom()),
        ),
        (
            egui::pos2(r.left(), r.top() + inset),
            egui::pos2(r.left(), r.bottom() - inset),
        ),
        (
            egui::pos2(r.right(), r.top() + inset),
            egui::pos2(r.right(), r.bottom() - inset),
        ),
    ] {
        painter.extend(egui::Shape::dashed_line(&[from, to], stroke, 5.0, 4.0));
    }
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
            crate::fonts::install_icons_only(&ctx);
            ctx.run_ui(egui::RawInput::default(), |_| {})
                .drop_without_applying_deltas();
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
                    &crate::theme::Roles::of(&Theme::default()),
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
                .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
                .find_map(|(glyph, color, square)| {
                    (glyph == egui_phosphor::regular::ARROW_UP_RIGHT
                        && color == Theme::default().focus()
                        && square.center().y < card.top() + 54.0)
                        .then_some(square.center().x)
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
                    &crate::theme::Roles::of(&Theme::default()),
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
    fn painted(
        doc: Option<&str>,
        claim: Option<panel_contract::Attention>,
        selected: bool,
    ) -> (Vec<egui::epaint::ClippedShape>, Rect) {
        painted_panel(
            doc.map(|d| panel_contract::parse_document(d).unwrap()),
            claim,
            selected,
        )
    }
    fn painted_panel(
        doc: Option<panel_contract::Panel>,
        claim: Option<panel_contract::Attention>,
        selected: bool,
    ) -> (Vec<egui::epaint::ClippedShape>, Rect) {
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let entry = Entry {
            pane: 1,
            console_label: "Console".into(),
            tab_label: "Tab".into(),
            snapshot: crate::panel::summary::Snapshot {
                document: doc.map(std::sync::Arc::new),
                warning: None,
                attention: claim.map(|class| crate::panel::AttentionState {
                    class,
                    backed: true,
                }),
                outside: false,
            },
        };
        let mut card = Rect::NOTHING;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_width(320.0);
            card = paint(
                ui,
                &entry,
                Column::Wait,
                true,
                None,
                None,
                selected,
                &mut Reader::default(),
                &mut crate::fonts::Discoverer::default(),
                &Theme::default(),
                &crate::theme::Roles::of(&Theme::default()),
                &mut crate::measure::Board::default(),
                &mut Vec::new(),
            );
        });
        let shapes = output.shapes.clone();
        output.drop_without_applying_deltas();
        (shapes, card)
    }
    fn dashes(shapes: &[egui::epaint::ClippedShape], color: Color32) -> usize {
        shapes
            .iter()
            .filter(|s| matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == color))
            .count()
    }
    const DOC: &str =
        r#"{"attention":"wait","lines":[{"text":"Refund webhooks","style":"title"}]}"#;

    #[test]
    fn a_console_with_no_published_state_is_dashed_and_a_published_one_is_not() {
        let roles = crate::theme::Roles::of(&Theme::default());
        let (shapes, _) = painted(None, None, false);
        assert!(dashes(&shapes, roles.line2) >= 8, "four dashed sides");
        let (shapes, _) = painted(Some(DOC), None, false);
        assert_eq!(dashes(&shapes, roles.line2), 0);
        // And no solid outline under the dashes, which would read as a published card.
        let (shapes, card) = painted(None, None, false);
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
            if r.rect.contains_rect(card.shrink(1.0)) && r.stroke.width > 0.0
                && r.stroke.color == roles.line))
        );
        // And no solid outline under the dashes, which would read as a published card.
        let (shapes, card) = painted(None, None, false);
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
            if r.rect.contains_rect(card.shrink(1.0)) && r.stroke.width > 0.0
                && r.stroke.color == roles.line))
        );
    }

    #[test]
    fn the_selected_card_carries_the_accent_border_at_two_points() {
        let roles = crate::theme::Roles::of(&Theme::default());
        let accent = |shapes: &[egui::epaint::ClippedShape], card: Rect| {
            shapes.iter().any(|s| {
                matches!(&s.shape, egui::Shape::Rect(r)
                if r.rect == card && r.stroke.color == roles.accent && r.stroke.width == 2.0)
            })
        };
        let (shapes, card) = painted(Some(DOC), None, true);
        assert!(accent(&shapes, card));
        let (shapes, card) = painted(Some(DOC), None, false);
        assert!(!accent(&shapes, card));
    }

    #[test]
    fn a_declared_wait_reads_as_its_pill_and_words_in_the_wait_role() {
        let roles = crate::theme::Roles::of(&Theme::default());
        let (shapes, card) = painted(Some(DOC), Some(panel_contract::Attention::Wait), false);
        let words = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Waiting for you" => Some(t),
                _ => None,
            })
            .expect("claim words painted");
        assert_eq!(words.fallback_color, roles.wait);
        assert!(card.contains(words.pos));
        assert!(
            shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
            if r.fill == roles.wait && r.rect.width() == crate::ui::claim_pill::SIZE))
        );
    }

    #[test]
    fn an_old_state_nobody_refreshes_is_veiled_and_a_fresh_one_is_not() {
        let roles = crate::theme::Roles::of(&Theme::default());
        let veiled = |doc: panel_contract::Panel| {
            let (shapes, card) = painted_panel(Some(doc), None, false);
            shapes.iter().any(|s| {
                matches!(&s.shape, egui::Shape::Rect(r)
                if r.rect == card && r.fill == roles.base.gamma_multiply(0.45))
            })
        };
        let fresh = panel_contract::parse_document(DOC).unwrap();
        let mut stale = fresh.clone();
        stale.written_at =
            Some(std::time::SystemTime::now() - stale.stale_after - Duration::from_secs(1));
        assert!(veiled(stale));
        assert!(!veiled(fresh));
    }
}
