//! Caption-group preferences belong to the reader, independently of the published plan.
use super::state::{Action, Reader};
use crate::theme::Roles;
use eframe::egui::{self, Rect, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(in crate::board) enum Group {
    Active,
    Ready,
    Outside,
    Opened,
}
impl Group {
    pub(super) const ALL: [Self; 4] = [Self::Active, Self::Ready, Self::Outside, Self::Opened];
    pub(super) fn label(self) -> &'static str {
        [
            "In progress",
            "Ready to start",
            "Off the plan",
            "Opened by you",
        ][self as usize]
    }
}
#[derive(Default)]
pub(super) struct Layout {
    pub(super) widths: [Option<f32>; 4],
    pub(super) hidden: [bool; 4],
    pub(super) maximized: Option<Group>,
}
impl Layout {
    pub(super) fn hide(&mut self, group: Group) {
        self.hidden[group as usize] = true;
        if self.maximized == Some(group) {
            self.maximized = None;
        }
    }
    pub(super) fn show(&mut self, group: Group) {
        self.hidden[group as usize] = false;
        self.maximized = None;
    }
    pub(super) fn toggle_maximized(&mut self, group: Group) {
        self.maximized = if self.maximized == Some(group) {
            None
        } else {
            Some(group)
        };
    }
    pub(super) fn resize(&mut self, drag: &Drag, x: f32) {
        let d = &drag.divider;
        let delta = (x - drag.x).clamp(d.minima[0] - d.widths[0], d.widths[1] - d.minima[1]);
        self.widths[d.groups[0] as usize] = Some(d.widths[0] + delta);
        self.widths[d.groups[1] as usize] = Some(d.widths[1] - delta);
    }
    pub(super) fn shows(&self, group: Group) -> bool {
        self.maximized
            .map_or(!self.hidden[group as usize], |g| g == group)
    }
}
#[derive(Clone, Debug)]
pub(in crate::board) struct Divider {
    pub(in crate::board) rect: Rect,
    pub(super) project: String,
    pub(super) groups: [Group; 2],
    pub(super) widths: [f32; 2],
    pub(super) minima: [f32; 2],
}
#[derive(Clone)]
pub(super) struct Drag {
    pub(super) divider: Divider,
    pub(super) x: f32,
}

pub(super) fn minimum(columns: &[(bool, bool)]) -> f32 {
    columns
        .iter()
        .map(|(collapsed, _)| if *collapsed { 190.0 } else { 240.0 })
        .sum::<f32>()
        + 20.0 * columns.len().saturating_sub(1) as f32
}

pub(super) fn widths(available: f32, columns: &[(usize, bool, bool)], layout: &Layout) -> Vec<f32> {
    let separators = columns.windows(2).filter(|p| p[0].0 != p[1].0).count();
    let sizes: Vec<_> = columns.iter().map(|c| (c.1, c.2)).collect();
    let mut result = super::projection::widths(available, &sizes, separators);
    for group in Group::ALL {
        let indices: Vec<_> = columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.0 == group as usize)
            .map(|(i, _)| i)
            .collect();
        if indices.is_empty() {
            continue;
        }
        let size: Vec<_> = indices.iter().map(|&i| sizes[i]).collect();
        let requested = if layout.maximized == Some(group) {
            Some(available)
        } else {
            layout.widths[group as usize]
        };
        let Some(requested) = requested else {
            continue;
        };
        let envelope = requested.max(minimum(&size));
        let mut allocated = super::projection::widths(envelope, &size, 0);
        if size.iter().all(|c| c.0) {
            let surplus = (envelope - minimum(&size)).max(0.0) / size.len() as f32;
            allocated.iter_mut().for_each(|w| *w += surplus);
        }
        for (&i, width) in indices.iter().zip(allocated) {
            result[i] = width;
        }
    }
    result
}

pub(super) fn caption(
    ui: &mut egui::Ui,
    rect: Rect,
    project: &str,
    group: Group,
    maximized: bool,
    reader: &mut Reader,
    roles: &Roles,
) {
    let label = Rect::from_min_max(rect.min, rect.max - vec2(48.0, 0.0));
    super::strip::text(
        ui,
        label,
        &group.label().to_uppercase(),
        egui::FontId::proportional(11.0),
        roles.tx3,
        1,
    );
    for (n, icon, hint, action) in [
        (
            0.0,
            crate::ui::icons::Icon::Panel,
            "Hide section",
            Action::HideGroup(project.into(), group),
        ),
        (
            1.0,
            if maximized {
                crate::ui::icons::Icon::Restore
            } else {
                crate::ui::icons::Icon::Maximize
            },
            if maximized {
                "Restore layout"
            } else {
                "Maximize section"
            },
            Action::MaximizeGroup(project.into(), group),
        ),
    ] {
        let hit = Rect::from_min_size(
            pos2(rect.right() - 46.0 + n * 24.0, rect.top()),
            vec2(22.0, 22.0),
        );
        let hover = ui.rect_contains_pointer(hit);
        if hover {
            ui.painter().rect_filled(hit, 4, roles.hover);
        }
        if hover && ui.input(|i| i.pointer.primary_down()) {
            ui.painter().rect_filled(hit, 4, roles.sel);
        }
        crate::ui::icons::paint(
            ui.painter(),
            hit.center(),
            icon,
            14.0,
            if hover { roles.tx } else { roles.tx3 },
        );
        reader.hits.push((hit.intersect(ui.clip_rect()), action));
        ui.interact(
            hit,
            ui.id()
                .with(("group-control", project, group as usize, n as usize)),
            egui::Sense::hover(),
        )
        .on_hover_text(hint);
    }
}

pub(super) fn recovery(ui: &mut egui::Ui, project: &str, reader: &mut Reader, roles: &Roles) {
    let p = reader.project(project);
    let hidden = p.sections.hidden;
    let maximized = p.sections.maximized;
    if !hidden.iter().any(|h| *h) && maximized.is_none() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        if maximized.is_some() {
            recovery_button(
                ui,
                "Restore layout",
                Action::RestoreLayout(project.into()),
                reader,
                roles,
            );
        }
        for group in Group::ALL {
            if hidden[group as usize] {
                recovery_button(
                    ui,
                    &format!("Show {}", group.label()),
                    Action::ShowGroup(project.into(), group),
                    reader,
                    roles,
                );
            }
        }
    });
    ui.add_space(4.0);
}
fn recovery_button(
    ui: &mut egui::Ui,
    label: &str,
    action: Action,
    reader: &mut Reader,
    roles: &Roles,
) {
    let font = egui::FontId::proportional(11.5);
    let width = ui
        .fonts_mut(|f| f.layout_no_wrap(label.into(), font.clone(), roles.tx2))
        .size()
        .x
        + 16.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, 24.0), egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        4,
        if ui.rect_contains_pointer(rect) && ui.input(|i| i.pointer.primary_down()) {
            roles.sel
        } else if ui.rect_contains_pointer(rect) {
            roles.hover
        } else {
            roles.raised
        },
    );
    super::strip::text(ui, rect.shrink2(vec2(8.0, 4.0)), label, font, roles.tx2, 1);
    reader.hits.push((rect.intersect(ui.clip_rect()), action));
}
