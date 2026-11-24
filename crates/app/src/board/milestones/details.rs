use super::{
    progress,
    projection::Class,
    state::{Action, Key, Reader, Target},
    strip,
};
use crate::{board::observation::Column, theme::Roles};
use eframe::egui::{self, FontId, Rect, Stroke, pos2, vec2};
use panel_contract::{
    Mark,
    plan::{Milestone, Plan},
};
#[allow(clippy::too_many_arguments)]
pub(super) fn header(
    ui: &mut egui::Ui,
    m: &Milestone,
    plan: &Plan,
    class: Class,
    missing: &[String],
    key: &Key,
    full: bool,
    collapsed: bool,
    closable: bool,
    reader: &mut Reader,
    roles: &Roles,
) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 36.0), egui::Sense::hover());
    let title = match &m.id {
        Some(id) if id != &m.title => format!("{id} · {}", m.title),
        _ => m.title.clone(),
    };
    let (a, b) = progress(m);
    let count = format!("{a}/{b}");
    let count_width = count.len() as f32 * 7.0 + 8.0;
    if full || reader.focus.as_ref() == Some(&Target::Header(key.clone())) {
        ui.painter().rect_filled(rect, 6, roles.sel);
    }
    crate::ui::icons::paint(
        ui.painter(),
        rect.min + vec2(6.0, 9.0),
        crate::ui::icons::Icon::Caret { open: !collapsed },
        12.0,
        roles.tx3,
    );
    let right = rect.right() - if closable { 20.0 } else { 0.0 };
    strip::text(
        ui,
        Rect::from_min_max(
            rect.min + vec2(18.0, 0.0),
            pos2(
                right - count_width - if class == Class::Ready { 40.0 } else { 0.0 },
                rect.top() + 20.0,
            ),
        ),
        &title,
        FontId::new(13.0, crate::fonts::ui_weight(ui.ctx(), 600)),
        roles.tx,
        1,
    );
    strip::text(
        ui,
        Rect::from_min_size(
            pos2(right - count_width, rect.top()),
            vec2(count_width, 18.0),
        ),
        &count,
        FontId::monospace(11.0),
        roles.tx2,
        1,
    );
    if class == Class::Ready {
        let badge = Rect::from_min_size(
            pos2(right - count_width - 38.0, rect.top()),
            vec2(34.0, 17.0),
        );
        ui.painter().rect_filled(badge, 4, roles.accent_soft);
        strip::text(
            ui,
            badge.shrink2(vec2(2.0, 0.0)),
            "ready",
            FontId::proportional(10.5),
            roles.accent,
            1,
        );
    }
    if closable {
        let close = Rect::from_min_size(pos2(rect.right() - 18.0, rect.top()), vec2(18.0, 20.0));
        strip::text(ui, close, "×", FontId::proportional(13.0), roles.tx2, 1);
        reader.hits.push((
            close.intersect(ui.clip_rect()),
            Action::CloseColumn(key.clone()),
        ));
    }
    let track = Rect::from_min_size(rect.min + vec2(0.0, 27.0), vec2(width, 3.0));
    ui.painter().rect_filled(track, 1.5, roles.line);
    if b > 0 {
        ui.painter().rect_filled(
            Rect::from_min_size(track.min, vec2(width * a as f32 / b as f32, 3.0)),
            1.5,
            if class == Class::Ready {
                roles.tx3
            } else {
                roles.accent
            },
        );
    }
    reader.hits.push((
        rect.intersect(ui.clip_rect()),
        Action::ToggleItems(key.clone()),
    ));
    reader.targets.push((Target::Header(key.clone()), rect));
    if reader.reveal_focus == Some(Target::Header(key.clone())) {
        ui.scroll_to_rect(rect, None);
        reader.reveal_focus = None;
    }
    ui.interact(
        rect,
        ui.id().with(("milestone-header", key)),
        egui::Sense::hover(),
    )
    .on_hover_text(&title);
    let after = if class == Class::Later && !missing.is_empty() {
        Some(format!("waiting on {}", missing.join(", ")))
    } else {
        super::after(m, plan)
    };
    if let Some(after) = after {
        let (rect, _) = ui.allocate_exact_size(vec2(width, 20.0), egui::Sense::hover());
        strip::text(
            ui,
            rect,
            &after,
            FontId::proportional(11.0),
            if class == Class::Later {
                roles.wait
            } else {
                roles.tx3
            },
            1,
        );
    }
    if full && let Some(criterion) = &m.done_when {
        let (rect, _) = ui.allocate_exact_size(vec2(width, 34.0), egui::Sense::hover());
        strip::text(
            ui,
            rect,
            &format!("Done when: {criterion}"),
            FontId::proportional(11.5),
            roles.tx2,
            2,
        );
    }
    ui.separator();
    ui.add_space(4.0);
}
pub(super) fn button(
    ui: &mut egui::Ui,
    label: &str,
    action: Action,
    reader: &mut Reader,
    roles: &Roles,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), egui::Sense::hover());
    let text = label.trim_end_matches(['▾', '▴', '▸', ' ']);
    strip::text(ui, rect, text, FontId::proportional(12.0), roles.tx2, 1);
    let width = ui
        .fonts_mut(|f| f.layout_no_wrap(text.into(), FontId::proportional(12.0), roles.tx2))
        .size()
        .x;
    let icon = if label.ends_with('▴') {
        crate::ui::icons::Icon::CaretUp
    } else {
        crate::ui::icons::Icon::Caret {
            open: !label.ends_with('▸'),
        }
    };
    crate::ui::icons::paint(
        ui.painter(),
        pos2(
            (rect.left() + width + 10.0).min(rect.right() - 6.0),
            rect.top() + 8.0,
        ),
        icon,
        12.0,
        roles.tx3,
    );
    reader.hits.push((rect.intersect(ui.clip_rect()), action));
}
#[allow(clippy::too_many_arguments)]
pub(super) fn item_row(
    ui: &mut egui::Ui,
    text: &str,
    mark: Option<Mark>,
    claim: Option<Column>,
    key: &Key,
    index: usize,
    reader: &mut Reader,
    roles: &Roles,
) {
    let mut job = egui::text::LayoutJob::simple(
        super::super::cards::clean(text),
        FontId::proportional(12.5),
        roles.tx2,
        (ui.available_width() - 22.0).max(1.0),
    );
    job.wrap.max_rows = 2;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let height = galley.size().y.max(22.0);
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
    let (mark, ink) = match claim {
        Some(Column::Wait) => (Mark::Wait, roles.wait),
        Some(Column::Block) => (Mark::Block, roles.block),
        Some(Column::Done) => (Mark::Done, roles.done),
        _ => match mark {
            Some(Mark::Done) => (Mark::Done, roles.tx3),
            Some(Mark::Current) => (Mark::Current, roles.accent),
            Some(Mark::Wait) => (Mark::Wait, roles.wait),
            Some(Mark::Block) => (Mark::Block, roles.block),
            _ => (Mark::Pending, roles.tx3),
        },
    };
    if let Some(class) = match claim {
        Some(Column::Wait) => Some(panel_contract::Attention::Wait),
        Some(Column::Block) => Some(panel_contract::Attention::Block),
        Some(Column::Done) => Some(panel_contract::Attention::Done),
        _ => None,
    } {
        crate::ui::claim_pill::paint(
            ui.painter(),
            Rect::from_min_size(rect.min, vec2(16.0, 18.0)),
            crate::panel::AttentionState {
                class,
                backed: true,
            },
            roles,
        );
    } else if mark == Mark::Pending {
        ui.painter()
            .circle_stroke(rect.min + vec2(6.0, 8.0), 3.0, Stroke::new(1.0, roles.tx3));
    } else {
        crate::panel::body::paint_mark(
            ui.painter(),
            Rect::from_min_size(rect.min, vec2(14.0, 18.0)),
            mark,
            ink,
            FontId::monospace(12.0),
        );
    }
    strip::text(
        ui,
        Rect::from_min_max(rect.min + vec2(22.0, 0.0), rect.max),
        text,
        FontId::proportional(12.5),
        if mark == Mark::Done {
            roles.tx3
        } else if claim.is_some() || mark == Mark::Current {
            roles.tx
        } else {
            roles.tx2
        },
        2,
    );
    if reader.focus == Some(Target::Item(key.clone(), index)) {
        ui.painter().rect_stroke(
            rect,
            2,
            Stroke::new(1.0, roles.accent),
            egui::StrokeKind::Inside,
        );
    }
    reader
        .targets
        .push((Target::Item(key.clone(), index), rect));
    if reader.reveal_focus == Some(Target::Item(key.clone(), index)) {
        ui.scroll_to_rect(rect, None);
        reader.reveal_focus = None;
    }
    ui.interact(
        rect,
        ui.id().with((key, index, "item")),
        egui::Sense::hover(),
    )
    .on_hover_text(text);
}

pub(super) fn off_header(
    ui: &mut egui::Ui,
    count: usize,
    key: &Key,
    full: bool,
    reader: &mut Reader,
    roles: &Roles,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), egui::Sense::hover());
    crate::ui::icons::paint(
        ui.painter(),
        rect.min + vec2(6.0, 9.0),
        crate::ui::icons::Icon::Caret { open: full },
        12.0,
        roles.tx3,
    );
    strip::text(
        ui,
        Rect::from_min_max(rect.min + vec2(18.0, 0.0), rect.max),
        &format!("Off the plan  {count}"),
        FontId::proportional(13.0),
        roles.tx,
        1,
    );
    reader.hits.push((
        rect.intersect(ui.clip_rect()),
        Action::ToggleItems(key.clone()),
    ));
    reader.targets.push((Target::Header(key.clone()), rect));
}
