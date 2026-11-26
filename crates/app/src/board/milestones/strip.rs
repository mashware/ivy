use super::{
    progress,
    projection::{Class, Projection},
    state::{Action, Identity, Key, Reader, Target},
};
use crate::theme::Roles;
use eframe::egui::{self, Color32, FontId, Rect, Stroke, pos2, vec2};
use panel_contract::plan::Plan;

pub(super) fn text(
    ui: &egui::Ui,
    rect: Rect,
    value: &str,
    font: FontId,
    ink: Color32,
    rows: usize,
) {
    let mut job = egui::text::LayoutJob::simple(
        super::super::cards::clean(value),
        font,
        ink,
        rect.width().max(0.0),
    );
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = rows == 1;
    job.wrap.overflow_character = Some('…');
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(rect.min, galley, ink);
}
pub(super) fn ink(class: Class, roles: &Roles) -> Color32 {
    match class {
        Class::Claim(crate::board::observation::Column::Wait) => roles.wait,
        Class::Claim(crate::board::observation::Column::Block) => roles.block,
        Class::Claim(_) => roles.done,
        Class::Active | Class::Done => roles.accent,
        Class::Ready => roles.tx3,
        Class::Later => roles.line2,
    }
}
pub(super) fn key(project: &str, plan: &Plan, i: usize) -> Key {
    Key {
        project: project.into(),
        milestone: plan.milestones[i]
            .id
            .clone()
            .map(Identity::Declared)
            .unwrap_or_else(|| Identity::Anonymous(i, plan.milestones[i].title.clone())),
    }
}

pub(super) fn paint(
    ui: &mut egui::Ui,
    project: &str,
    plan: &Plan,
    projection: &Projection,
    reader: &mut Reader,
    roles: &Roles,
) {
    let count = plan.milestones.len();
    if count == 0 {
        return;
    }
    let width = ((ui.available_width() - 4.0 * (count - 1) as f32) / count as f32).max(0.0);
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), egui::Sense::hover());
    for (i, m) in plan.milestones.iter().enumerate() {
        let rect = Rect::from_min_size(
            pos2(strip.left() + i as f32 * (width + 4.0), strip.top()),
            vec2(width, 48.0),
        );
        let k = key(project, plan, i);
        let class = projection.classes[i];
        let color = ink(class, roles);
        let p = reader.project(project);
        let opened = p.show_all || p.full.contains(&k.milestone) || p.opened.contains(&k.milestone);
        let fill = if opened {
            roles.sel
        } else {
            match class {
                Class::Done => roles.accent_soft,
                Class::Active => roles.raised,
                Class::Claim(_) => color.gamma_multiply(0.15),
                _ => Color32::TRANSPARENT,
            }
        };
        let hovered = ui.rect_contains_pointer(rect);
        ui.painter().rect(
            rect,
            6,
            if hovered { roles.hover } else { fill },
            Stroke::new(
                1.0,
                if matches!(class, Class::Done | Class::Later) {
                    Color32::TRANSPARENT
                } else if class == Class::Ready {
                    roles.line2
                } else {
                    color
                },
            ),
            egui::StrokeKind::Inside,
        );
        if class == Class::Later {
            super::super::cards::dashed(ui.painter(), rect, roles.line);
        }
        let prefix = match class {
            Class::Done => Some(crate::ui::icons::Icon::Check),
            _ => None,
        };
        let mark = match class {
            Class::Claim(crate::board::observation::Column::Wait) => {
                Some(panel_contract::Attention::Wait)
            }
            Class::Claim(crate::board::observation::Column::Block) => {
                Some(panel_contract::Attention::Block)
            }
            Class::Claim(_) => Some(panel_contract::Attention::Done),
            _ => None,
        };
        let mut left = rect.left() + 8.0;
        if let Some(icon) = prefix {
            crate::ui::icons::paint(
                ui.painter(),
                pos2(left + 5.0, rect.top() + 12.0),
                icon,
                11.0,
                color,
            );
            left += 14.0;
        }
        if let Some(class) = mark {
            crate::ui::claim_pill::paint(
                ui.painter(),
                Rect::from_min_size(pos2(left, rect.top() + 4.0), vec2(16.0, 16.0)),
                crate::panel::AttentionState {
                    class,
                    backed: true,
                },
                roles,
            );
            left += 20.0;
        }
        let title = match &m.id {
            Some(id) if id != &m.title => format!("{id} {}", m.title),
            _ => m.title.clone(),
        };
        text(
            ui,
            Rect::from_min_max(pos2(left, rect.top() + 6.0), rect.max - vec2(8.0, 23.0)),
            &title,
            FontId::proportional(11.5),
            if class == Class::Later {
                roles.tx3
            } else {
                roles.tx
            },
            1,
        );
        let (a, b) = progress(m);
        let count = format!("{a}/{b}");
        let number_width = (count.len() as f32 * 6.5).min((width - 12.0).max(0.0));
        text(
            ui,
            Rect::from_min_size(
                pos2(rect.right() - 8.0 - number_width, rect.bottom() - 18.0),
                vec2(number_width, 14.0),
            ),
            &count,
            FontId::monospace(10.5),
            roles.tx3,
            1,
        );
        let bar = Rect::from_min_size(
            pos2(rect.left() + 8.0, rect.bottom() - 12.0),
            vec2((width - number_width - 24.0).max(0.0), 3.0),
        );
        ui.painter().rect_filled(bar, 1.5, roles.line);
        if b > 0 {
            ui.painter().rect_filled(
                Rect::from_min_size(bar.min, vec2(bar.width() * a as f32 / b as f32, 3.0)),
                1.5,
                color,
            );
        }
        if opened {
            ui.painter().rect_filled(
                Rect::from_min_size(rect.left_bottom() - vec2(0.0, 2.0), vec2(width, 2.0)),
                0,
                roles.accent,
            );
        }
        if reader.focus.as_ref() == Some(&Target::Strip(k.clone())) {
            ui.painter().rect_stroke(
                rect,
                6,
                Stroke::new(2.0, roles.accent),
                egui::StrokeKind::Inside,
            );
        }
        let mut hint = format!("{}\n{a}/{b}", m.title);
        if let Some(after) = super::after(m, plan) {
            hint.push_str(&format!("\n{after}"));
        }
        if !projection.missing[i].is_empty() {
            hint.push_str(&format!(
                "\nwaiting on {}",
                projection.missing[i].join(", ")
            ));
        }
        ui.interact(
            rect,
            ui.id().with(("milestone-strip", &k)),
            egui::Sense::hover(),
        )
        .on_hover_text(hint)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
        reader.hits.push((
            rect.intersect(ui.clip_rect()),
            Action::ToggleColumn(k.clone()),
        ));
        if reader.reveal_focus == Some(Target::Strip(k.clone())) {
            ui.scroll_to_rect(rect, None);
            reader.reveal_focus = None;
        }
        reader.targets.push((Target::Strip(k), rect));
    }
    let (legend, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), 32.0), egui::Sense::hover());
    text(
        ui,
        legend,
        "done · green border = in progress · amber = claiming you · grey border = ready to start · dashed = waiting on dependencies · click a milestone to open it below",
        FontId::proportional(11.0),
        roles.tx3,
        1,
    );
}
