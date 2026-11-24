//! Disclosure belongs to the reader and survives hiding Board, not application restarts.
use super::projection::Class;
use eframe::egui::{self, Rect};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::board) struct Key {
    pub project: String,
    pub milestone: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::board) enum Action {
    Strip(Key),
    Toggle(Key),
    Close(Key),
    ShowAll(bool),
}
#[derive(Default)]
pub(super) struct Project {
    pub(super) opened: HashSet<String>,
    pub(super) full: HashSet<String>,
    pub(super) show_all: bool,
    classes: HashMap<String, Class>,
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
    pub(in crate::board) zone: u8,
    pub(super) diagnostics: HashSet<(String, String)>,
    all: bool,
}
impl Reader {
    pub(super) fn project(&mut self, key: &str) -> &mut Project {
        self.projects.entry(key.into()).or_insert_with(|| Project {
            show_all: self.all,
            ..Default::default()
        })
    }
    pub(super) fn reconcile(&mut self, project: &str, classes: &[(String, Class)]) {
        let reader = self.project(project);
        reader
            .full
            .retain(|id| id == "@off" || classes.iter().any(|(key, _)| key == id));
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
    pub(in crate::board) fn show_all(&self) -> bool {
        self.all
    }
    pub(in crate::board) fn apply(&mut self, action: &Action) {
        match action {
            Action::ShowAll(value) => {
                self.all = *value;
                for p in self.projects.values_mut() {
                    p.show_all = *value;
                }
            }
            Action::Strip(key) => {
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
            Action::Toggle(key) => {
                let p = self.project(&key.project);
                if !p.full.remove(&key.milestone) {
                    p.full.insert(key.milestone.clone());
                }
            }
            Action::Close(key) => {
                let p = self.project(&key.project);
                p.opened.remove(&key.milestone);
                p.full.remove(&key.milestone);
            }
        }
    }
    pub(in crate::board) fn keyboard(
        &mut self,
        key: egui::Key,
        shift: bool,
    ) -> (bool, Option<usize>) {
        if key == egui::Key::Tab {
            self.zone = if shift {
                (self.zone + 2) % 3
            } else {
                (self.zone + 1) % 3
            };
            self.focus = self
                .targets
                .iter()
                .find(|(target, _)| zone(target) == self.zone)
                .map(|(t, _)| t.clone());
            self.reveal_focus = self.focus.clone();
            return (true, None);
        }
        if self.zone == 2 {
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
                    Target::Strip(k) => self.apply(&Action::Strip(k.clone())),
                    Target::Header(k) => self.apply(&Action::Toggle(k.clone())),
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
fn zone(target: &Target) -> u8 {
    match target {
        Target::Strip(_) => 0,
        _ => 1,
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
            zone: 2,
            diagnostics: HashSet::new(),
            all: false,
        }
    }
}
