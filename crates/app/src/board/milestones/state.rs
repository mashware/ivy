//! Disclosure belongs to the reader and survives hiding Board, not application restarts.
use super::{
    projection::Class,
    sections::{self, Group},
};
use eframe::egui::{self, Rect};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::board) enum Identity {
    Declared(String),
    Anonymous(usize, String),
    Outside,
}
impl From<&str> for Identity {
    fn from(value: &str) -> Self {
        Self::Declared(value.into())
    }
}
impl From<String> for Identity {
    fn from(value: String) -> Self {
        Self::Declared(value)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::board) enum FocusZone {
    Strip,
    Columns,
    #[default]
    Cards,
}
impl FocusZone {
    fn next(self, backward: bool) -> Self {
        match (self, backward) {
            (Self::Strip, false) | (Self::Cards, true) => Self::Columns,
            (Self::Columns, false) | (Self::Strip, true) => Self::Cards,
            _ => Self::Strip,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::board) struct Key {
    pub project: String,
    pub milestone: Identity,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::board) enum Action {
    ToggleColumn(Key),
    ToggleItems(Key),
    CloseColumn(Key),
    ShowAll(bool),
    HideGroup(String, Group),
    ShowGroup(String, Group),
    MaximizeGroup(String, Group),
    RestoreLayout(String),
}
#[derive(Default)]
pub(super) struct Project {
    pub(super) opened: HashSet<Identity>,
    pub(super) full: HashSet<Identity>,
    pub(super) show_all: bool,
    pub(super) sections: sections::Layout,
    classes: HashMap<Identity, Class>,
    diagnostic_plan: Option<panel_contract::plan::Plan>,
    diagnostics: HashSet<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::board) enum Target {
    Strip(Key),
    Header(Key),
    Item(Key, usize),
    Card(usize),
}
pub(in crate::board) struct Reader {
    pub(super) projects: HashMap<String, Project>,
    pub(in crate::board) hits: Vec<(Rect, Action)>,
    pub(in crate::board) targets: Vec<(Target, Rect)>,
    pub(in crate::board) focus: Option<Target>,
    pub(super) reveal_focus: Option<Target>,
    pub(in crate::board) reveal: Option<Key>,
    pub(in crate::board) reveal_pane: Option<usize>,
    pub(in crate::board) zone: FocusZone,
    pub(in crate::board) dividers: Vec<sections::Divider>,
    drag: Option<sections::Drag>,
    all: bool,
}
impl Reader {
    pub(super) fn project(&mut self, key: &str) -> &mut Project {
        self.projects.entry(key.into()).or_insert_with(|| Project {
            show_all: self.all,
            ..Default::default()
        })
    }
    pub(super) fn reconcile(&mut self, project: &str, classes: &[(Identity, Class)]) {
        let reader = self.project(project);
        reader
            .full
            .retain(|id| *id == Identity::Outside || classes.iter().any(|(key, _)| key == id));
        reader
            .opened
            .retain(|id| classes.iter().any(|(key, _)| key == id));
        for (id, class) in classes {
            if reader.classes.get(id).is_some_and(|old| old != class) {
                reader.full.remove(id);
            }
        }
        reader.classes = classes.iter().cloned().collect();
    }
    /// Keep warnings only for the current bounded plan snapshot, not every plan seen this session.
    pub(super) fn new_diagnostics(
        &mut self,
        project: &str,
        plan: &panel_contract::plan::Plan,
        diagnostics: &[String],
    ) -> Vec<String> {
        let reader = self.project(project);
        if reader.diagnostic_plan.as_ref() != Some(plan) {
            reader.diagnostics.clear();
            reader.diagnostic_plan = Some(plan.clone());
        }
        diagnostics
            .iter()
            .filter(|message| reader.diagnostics.insert((*message).clone()))
            .cloned()
            .collect()
    }
    pub(in crate::board) fn show_all(&self) -> bool {
        self.all
    }
    pub(in crate::board) fn apply(&mut self, action: &Action) {
        match action {
            Action::HideGroup(project, group) => {
                let p = self.project(project);
                p.sections.hidden[*group as usize] = true;
                if p.sections.maximized == Some(*group) {
                    p.sections.maximized = None;
                }
                self.cancel_resize();
            }
            Action::ShowGroup(project, group) => {
                let p = self.project(project);
                p.sections.hidden[*group as usize] = false;
                p.sections.maximized = None;
                self.cancel_resize();
            }
            Action::MaximizeGroup(project, group) => {
                let p = self.project(project);
                p.sections.maximized = if p.sections.maximized == Some(*group) {
                    None
                } else {
                    Some(*group)
                };
                self.cancel_resize();
            }
            Action::RestoreLayout(project) => {
                self.project(project).sections.maximized = None;
                self.cancel_resize();
            }
            Action::ShowAll(value) => {
                self.cancel_resize();
                self.all = *value;
                for p in self.projects.values_mut() {
                    p.show_all = *value;
                }
            }
            Action::ToggleColumn(key) => {
                let p = self.project(&key.project);
                let visible = p
                    .classes
                    .get(&key.milestone)
                    .is_some_and(|c| c.active() || *c == Class::Ready);
                if visible {
                    if !p.full.remove(&key.milestone) {
                        p.full.insert(key.milestone.clone());
                        p.opened.insert(key.milestone.clone());
                    } else {
                        p.opened.remove(&key.milestone);
                    }
                } else if !p.opened.remove(&key.milestone) {
                    p.opened.insert(key.milestone.clone());
                    p.full.insert(key.milestone.clone());
                } else {
                    p.full.remove(&key.milestone);
                }
                self.reveal = Some(key.clone());
            }
            Action::ToggleItems(key) => {
                let p = self.project(&key.project);
                if !p.full.remove(&key.milestone) {
                    p.full.insert(key.milestone.clone());
                }
            }
            Action::CloseColumn(key) => {
                let p = self.project(&key.project);
                p.opened.remove(&key.milestone);
                p.full.remove(&key.milestone);
            }
        }
    }
    pub(in crate::board) fn begin_resize(&mut self, pos: egui::Pos2) -> bool {
        let Some(divider) = self.dividers.iter().find(|d| d.rect.contains(pos)).cloned() else {
            return false;
        };
        self.drag = Some(sections::Drag { divider, x: pos.x });
        true
    }
    pub(in crate::board) fn resizing(&self) -> bool {
        self.drag.is_some()
    }
    pub(in crate::board) fn resize(&mut self, pos: egui::Pos2) {
        let Some(drag) = self.drag.clone() else {
            return;
        };
        let d = &drag.divider;
        let delta = (pos.x - drag.x).clamp(d.minima[0] - d.widths[0], d.widths[1] - d.minima[1]);
        let layout = &mut self.project(&d.project).sections;
        layout.widths[d.groups[0] as usize] = Some(d.widths[0] + delta);
        layout.widths[d.groups[1] as usize] = Some(d.widths[1] - delta);
    }
    pub(in crate::board) fn cancel_resize(&mut self) {
        self.drag = None;
    }
    pub(in crate::board) fn finish_resize_frame(&mut self) {
        if let Some(drag) = &self.drag
            && !self.dividers.iter().any(|d| {
                d.project == drag.divider.project
                    && d.groups == drag.divider.groups
                    && d.minima == drag.divider.minima
            })
        {
            self.cancel_resize();
        }
    }
    pub(in crate::board) fn keyboard(
        &mut self,
        key: egui::Key,
        shift: bool,
    ) -> (bool, Option<usize>) {
        if key == egui::Key::Tab {
            self.zone = self.zone.next(shift);
            self.focus = self
                .targets
                .iter()
                .find(|(target, _)| zone(target) == self.zone)
                .map(|(t, _)| t.clone());
            self.reveal_focus = self.focus.clone();
            return (true, None);
        }
        if self.zone == FocusZone::Cards {
            return (false, None);
        }
        let targets: Vec<_> = self
            .targets
            .iter()
            .filter(|(t, _)| zone(t) == self.zone)
            .map(|(t, _)| t.clone())
            .collect();
        if targets.is_empty() {
            return (false, None);
        }
        let index = targets
            .iter()
            .position(|t| Some(t) == self.focus.as_ref())
            .unwrap_or(0);
        let next = match key {
            egui::Key::ArrowLeft | egui::Key::ArrowUp => index.saturating_sub(1),
            egui::Key::ArrowRight | egui::Key::ArrowDown => (index + 1).min(targets.len() - 1),
            egui::Key::Home => 0,
            egui::Key::End => targets.len() - 1,
            egui::Key::Enter | egui::Key::Space => {
                match &targets[index] {
                    Target::Strip(k) => self.apply(&Action::ToggleColumn(k.clone())),
                    Target::Header(k) => self.apply(&Action::ToggleItems(k.clone())),
                    Target::Card(pane) => return (true, Some(*pane)),
                    Target::Item(..) => {}
                }
                return (true, None);
            }
            _ => return (false, None),
        };
        self.focus = Some(targets[next].clone());
        self.reveal_focus = self.focus.clone();
        match &targets[next] {
            Target::Strip(k) | Target::Header(k) | Target::Item(k, _) => {
                self.reveal = Some(k.clone())
            }
            Target::Card(_) => {}
        }
        (true, None)
    }
}
fn zone(target: &Target) -> FocusZone {
    match target {
        Target::Strip(_) => FocusZone::Strip,
        _ => FocusZone::Columns,
    }
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            projects: HashMap::new(),
            hits: Vec::new(),
            targets: Vec::new(),
            focus: None,
            reveal_focus: None,
            reveal: None,
            reveal_pane: None,
            zone: FocusZone::Cards,
            dividers: Vec::new(),
            drag: None,
            all: false,
        }
    }
}
