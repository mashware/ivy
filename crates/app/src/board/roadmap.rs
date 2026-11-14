//! A roadmap is inert text and groups of console cards; no links or actions are activated here.
use super::model::Section;
use crate::{project::Key, theme::Theme};
use eframe::egui;
use panel_contract::{
    Mark, Panel,
    plan::{Milestone, Plan},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, SystemTime},
};

#[derive(Clone)]
pub(super) struct Resolved {
    pub(super) plan: Option<Arc<Plan>>,
    pub(super) source: String,
    pub(super) error: Option<String>,
}

#[derive(Default)]
pub(super) struct Reader {
    projects: HashMap<String, bool>,
    milestones: HashSet<(String, String)>,
    all_milestones: HashSet<String>,
}

fn project_key(section: &Section) -> String {
    match &section.key {
        Some(Key::Repository(path)) => format!("repository:{}", path.display()),
        Some(Key::Folder(path)) => format!("folder:{}", path.display()),
        None => format!("name:{}", section.name()),
    }
}

pub(super) fn window(plan: &Plan, done: usize, next: usize) -> std::ops::Range<usize> {
    let anchor = plan
        .milestones
        .iter()
        .position(|m| m.items.iter().any(|i| i.mark == Some(Mark::Current)))
        .or_else(|| {
            plan.milestones
                .iter()
                .position(|m| m.items.iter().any(|i| i.mark != Some(Mark::Done)))
        })
        .unwrap_or_else(|| plan.milestones.len().saturating_sub(1));
    anchor.saturating_sub(done)
        ..anchor
            .saturating_add(next)
            .saturating_add(1)
            .min(plan.milestones.len())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Placement {
    Item(usize, usize),
    Milestone(usize),
    Off,
}

pub(super) fn placement(panel: Option<&Panel>, plan: &Plan) -> Placement {
    let Some(panel) = panel else {
        return Placement::Off;
    };
    let Some(id) = &panel.milestone else {
        return Placement::Off;
    };
    let Some(m) = plan
        .milestones
        .iter()
        .position(|m| m.id.as_ref() == Some(id))
    else {
        return Placement::Off;
    };
    match &panel.deliverable {
        None => Placement::Milestone(m),
        Some(id) => plan.milestones[m]
            .items
            .iter()
            .position(|i| i.id.as_ref() == Some(id))
            .map_or(Placement::Off, |i| Placement::Item(m, i)),
    }
}

fn milestone_title(milestone: &Milestone) -> String {
    let done = milestone
        .items
        .iter()
        .filter(|i| i.mark == Some(Mark::Done))
        .count();
    let id = milestone.id.as_deref().unwrap_or("");
    format!(
        "{id} {} - {done}/{}",
        milestone.title,
        milestone.items.len()
    )
}
fn symbol(mark: Option<Mark>) -> &'static str {
    match mark {
        Some(Mark::Done) => "✓",
        Some(Mark::Current) => "▶",
        Some(Mark::Pending) => "○",
        Some(Mark::Wait) => "…",
        Some(Mark::Block) => "!",
        _ => " ",
    }
}
pub(super) fn published_source(published: &super::plans::Published, now: SystemTime) -> String {
    let seconds = now
        .duration_since(published.published_at)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let age = if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86400)
    };
    let source = published
        .plan
        .source
        .as_ref()
        .map(|s| format!(" | {s}"))
        .unwrap_or_default();
    format!("published {age} ago{source}")
}

pub(super) fn refresh_files(window: &mut crate::Window, ctx: &eframe::egui::Context) {
    if window.board.file_reload {
        window.board.files = if window.board.file_config.is_empty() {
            None
        } else {
            Some(super::files::Reader::new(
                window.board.file_config.clone(),
                ctx.clone(),
            ))
        };
        window.board.file_snapshots.clear();
        window.board.file_reload = false;
    }
    if let Some(reader) = &mut window.board.files {
        window.board.file_snapshots = reader.snapshots();
    }
}

pub(super) fn enrich(window: &crate::Window, sections: &mut Vec<Section>, now: SystemTime) {
    for file in &window.board.file_snapshots {
        let key = file.identity.as_ref().map(|identity| identity.key.clone());
        if key.is_some() && sections.iter().any(|s| s.key == key) {
            continue;
        }
        sections.push(Section {
            key,
            kind: super::model::Kind::Repository {
                name: file.name.clone(),
                ink: window.theme.focus(),
            },
            entries: Vec::new(),
            roadmap: None,
        });
    }
    sections.sort_by_key(|s| matches!(s.kind, super::model::Kind::Other));
    for section in sections {
        section.roadmap = if let Some(published) = section
            .key
            .as_ref()
            .and_then(|key| window.board.plans.published(key))
        {
            Some(Resolved {
                plan: Some(Arc::new(published.plan.clone())),
                source: published_source(&published, now),
                error: None,
            })
        } else {
            window
                .board
                .file_snapshots
                .iter()
                .find(|file| {
                    if let Some(key) = &section.key {
                        file.identity.as_ref().is_some_and(|i| &i.key == key)
                    } else {
                        file.name == section.name()
                    }
                })
                .map(|file| Resolved {
                    plan: file.plan.clone(),
                    source: format!("from {}", file.path.display()),
                    error: file.error.clone(),
                })
        };
    }
}

/// Reader choices are local to the board and never change the published plan.
pub(super) fn paint_controls(ui: &mut egui::Ui, sections: &[Section], reader: &mut Reader) {
    ui.horizontal(|ui| {
        if ui.button("Open all roadmaps").clicked() {
            for section in sections
                .iter()
                .filter(|s| s.roadmap.is_some() || !fallback(s).is_empty())
            {
                reader.projects.insert(project_key(section), true);
            }
        }
        if ui.button("Close all roadmaps").clicked() {
            for section in sections
                .iter()
                .filter(|s| s.roadmap.is_some() || !fallback(s).is_empty())
            {
                reader.projects.insert(project_key(section), false);
            }
        }
    });
}

fn done(m: &Milestone) -> bool {
    !m.items.is_empty() && m.items.iter().all(|i| i.mark == Some(Mark::Done))
}

fn started(m: &Milestone, attached: bool) -> bool {
    attached
        || (!done(m)
            && m.items.iter().any(|i| {
                matches!(
                    i.mark,
                    Some(Mark::Done | Mark::Current | Mark::Wait | Mark::Block)
                )
            }))
}

fn visible_items(m: &Milestone, attached: &[bool], all: bool) -> Vec<usize> {
    let mut open = 0;
    m.items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let is_open = item.mark != Some(Mark::Done);
            if is_open {
                open += 1;
            }
            (all || attached.get(i).copied().unwrap_or(false) || (is_open && open <= 5))
                .then_some(i)
        })
        .collect()
}

fn chips(
    ui: &mut egui::Ui,
    section: &Section,
    positions: &[Placement],
    wanted: &Placement,
    selected: &mut Option<usize>,
) {
    for (entry, position) in section.entries.iter().zip(positions) {
        if position == wanted {
            let id = egui::Id::new(("board-roadmap-console", ui.ctx().viewport_id(), entry.pane));
            if ui
                .scope_builder(egui::UiBuilder::new().id(id), |ui| {
                    ui.small_button(format!("{} · {}", entry.tab_label, entry.console_label))
                        .clicked()
                })
                .inner
            {
                *selected = Some(entry.pane);
            }
        }
    }
}

fn fallback(section: &Section) -> Vec<(&str, &panel_contract::Line)> {
    section
        .entries
        .iter()
        .flat_map(|entry| {
            entry.snapshot.document.iter().flat_map(move |panel| {
                panel
                    .lines
                    .iter()
                    .filter(|line| line.style == Some(panel_contract::Style::Section))
                    .map(move |line| (entry.console_label.as_str(), line))
            })
        })
        .collect()
}

pub(super) fn paint_project(
    ui: &mut egui::Ui,
    section: &Section,
    reader: &mut Reader,
    window: (usize, usize),
    theme: &Theme,
) -> Option<usize> {
    let id = egui::Id::new((
        "board-roadmap-project",
        ui.ctx().viewport_id(),
        project_key(section),
    ));
    ui.scope_builder(egui::UiBuilder::new().id(id), |ui| {
        paint_project_contents(ui, section, reader, window, theme)
    })
    .inner
}

fn paint_project_contents(
    ui: &mut egui::Ui,
    section: &Section,
    reader: &mut Reader,
    window: (usize, usize),
    theme: &Theme,
) -> Option<usize> {
    let key = project_key(section);
    let mut selected = None;
    let resolved = section.roadmap.as_ref();
    let fallback = fallback(section);
    let mut is_open = reader.projects.get(&key).copied().unwrap_or(false);
    let horizontal_offset = (ui.clip_rect().left() - ui.max_rect().left()).max(0.0);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(horizontal_offset);
        ui.colored_label(
            section.ink().unwrap_or(theme.focus()),
            egui::RichText::new(section.name()).strong(),
        );
        ui.label(format!("{} consoles", section.entries.len()));
        if let Some(roadmap) = resolved {
            if let Some(plan) = &roadmap.plan {
                let total: usize = plan.milestones.iter().map(|m| m.items.len()).sum();
                let completed: usize = plan
                    .milestones
                    .iter()
                    .flat_map(|m| &m.items)
                    .filter(|i| i.mark == Some(Mark::Done))
                    .count();
                ui.label(format!(
                    "{} · {completed}/{total} done",
                    plan.title.as_deref().unwrap_or("Roadmap")
                ));
            } else {
                ui.label(format!("No plan · {} sections", fallback.len()));
            }
            let notice = match &roadmap.error {
                Some(error) => format!("{}\n{}", roadmap.source, error),
                None => roadmap.source.clone(),
            };
            if ui
                .small_button(if is_open {
                    "▾ Roadmap"
                } else {
                    "▸ Roadmap"
                })
                .on_hover_text(notice)
                .clicked()
            {
                is_open = !is_open;
                reader.projects.insert(key.clone(), is_open);
            }
        } else if !fallback.is_empty() {
            ui.label(format!("No plan · {} sections", fallback.len()));
            if ui
                .small_button(if is_open {
                    "▾ Sections"
                } else {
                    "▸ Sections"
                })
                .clicked()
            {
                is_open = !is_open;
                reader.projects.insert(key.clone(), is_open);
            }
        } else {
            ui.label("No plan");
        }
    });
    if is_open && resolved.is_none_or(|r| r.plan.is_none()) && !fallback.is_empty() {
        ui.label("No plan published · console sections");
        for (console, line) in fallback {
            ui.horizontal_wrapped(|ui| {
                ui.label(console);
                ui.strong(&line.text);
                if let Some(summary) = &line.summary {
                    ui.label(summary);
                }
            });
        }
    }
    let Some(roadmap) = resolved.filter(|_| is_open) else {
        return selected;
    };
    ui.small(&roadmap.source);
    if let Some(error) = &roadmap.error {
        ui.label(error);
    }
    let Some(plan) = &roadmap.plan else {
        return selected;
    };
    if let Some(title) = &plan.title {
        ui.strong(title);
    }
    let positions: Vec<_> = section
        .entries
        .iter()
        .map(|e| placement(e.snapshot.document.as_deref(), plan))
        .collect();
    let attached: Vec<bool> = (0..plan.milestones.len())
        .map(|m| {
            positions
                .iter()
                .any(|p| matches!(p, Placement::Item(n, _) | Placement::Milestone(n) if *n == m))
        })
        .collect();
    let prefix = plan
        .milestones
        .iter()
        .enumerate()
        .take_while(|(i, m)| done(m) && !attached[*i])
        .count();
    if prefix > 0 {
        ui.label(format!("✓ {prefix} completed milestones"));
    }
    let range = super::roadmap::window(plan, window.0, window.1);
    let all = reader.all_milestones.contains(&key);
    for (mi, milestone) in plan.milestones.iter().enumerate() {
        if mi < prefix && !all {
            continue;
        }
        let active = started(milestone, attached[mi]);
        if !all && !range.contains(&mi) && !active {
            continue;
        }
        if !active && !all {
            ui.label(format!("○ {}", milestone_title(milestone)));
            continue;
        }
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.strong(milestone_title(milestone));
            if let Some(criterion) = &milestone.done_when {
                ui.label(format!("Done when: {criterion}"));
            }
            if !milestone.depends_on.is_empty() {
                ui.label(format!("Depends on: {}", milestone.depends_on.join(", ")));
            }
            ui.horizontal_wrapped(|ui| {
                chips(
                    ui,
                    section,
                    &positions,
                    &Placement::Milestone(mi),
                    &mut selected,
                )
            });
            let item_attached: Vec<_> = (0..milestone.items.len())
                .map(|i| positions.contains(&Placement::Item(mi, i)))
                .collect();
            let mk = (
                key.clone(),
                milestone
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("#{mi}:{}", milestone.title)),
            );
            let expanded = reader.milestones.contains(&mk);
            let visible = visible_items(milestone, &item_attached, expanded);
            let hidden_done = milestone
                .items
                .iter()
                .enumerate()
                .filter(|(i, item)| item.mark == Some(Mark::Done) && !visible.contains(i))
                .count();
            if hidden_done > 0 {
                ui.small(format!("✓ {hidden_done} completed"));
            }
            for &i in &visible {
                let item = &milestone.items[i];
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "{} {} {}",
                        symbol(item.mark),
                        item.text,
                        item.reference.as_deref().unwrap_or("")
                    ));
                    chips(
                        ui,
                        section,
                        &positions,
                        &Placement::Item(mi, i),
                        &mut selected,
                    );
                });
            }
            if (visible.len() < milestone.items.len() || expanded)
                && ui
                    .small_button(if expanded { "Show less" } else { "Show all" })
                    .clicked()
            {
                if expanded {
                    reader.milestones.remove(&mk);
                } else {
                    reader.milestones.insert(mk);
                }
            }
        });
    }
    let hidden_details = prefix > 0
        || plan
            .milestones
            .iter()
            .enumerate()
            .any(|(i, m)| !started(m, attached[i]));
    if (hidden_details || all)
        && ui
            .small_button(if all {
                "Show less milestones"
            } else {
                "Show all milestones"
            })
            .clicked()
    {
        if all {
            reader.all_milestones.remove(&key);
        } else {
            reader.all_milestones.insert(key.clone());
        }
    }
    if !plan.outside.is_empty() {
        ui.strong("Outside the plan");
        for item in &plan.outside {
            ui.label(format!("{} {}", symbol(item.mark), item.text));
        }
    }
    if positions.contains(&Placement::Off) {
        ui.label("Off the plan");
        ui.horizontal_wrapped(|ui| chips(ui, section, &positions, &Placement::Off, &mut selected));
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> Plan {
        panel_contract::plan::parse_document_noting(r#"{"milestones":[{"id":"empty","title":"Empty"},{"id":"old","title":"Old","items":[{"text":"old","mark":"pending"}]},{"id":"H3","title":"Now","items":[{"id":"D","text":"work","mark":"current"},{"text":"unknown","mark":"future"}]}]}"#).unwrap().0
    }
    fn test_section(plan: Plan, panes: &[usize]) -> Section {
        Section {
            key: None,
            kind: super::super::model::Kind::Other,
            entries: panes
                .iter()
                .map(|pane| super::super::model::Entry {
                    pane: *pane,
                    console_label: format!("Console {pane}"),
                    tab_label: "Tab".into(),
                    snapshot: crate::panel::summary::Snapshot {
                        document: None,
                        warning: None,
                        attention: None,
                        outside: false,
                    },
                })
                .collect(),
            roadmap: Some(Resolved {
                plan: Some(Arc::new(plan)),
                source: "fixture".into(),
                error: None,
            }),
        }
    }
    fn press(pos: egui::Pos2, down: bool) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: down,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    fn ui_frame(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        paint: impl FnMut(&mut egui::Ui),
    ) -> Vec<egui::epaint::ClippedShape> {
        let output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            paint,
        );
        let shapes = output.shapes.clone();
        output.drop_without_applying_deltas();
        shapes
    }
    #[test]
    fn a_small_pending_plan_exposes_all_deliverable_details() {
        let plan=panel_contract::plan::parse_document_noting(r#"{"milestones":[{"id":"M","title":"Pending","done_when":"criterion witness","items":[{"text":"deliverable witness","mark":"pending"}]}]}"#).unwrap().0;
        let section = test_section(plan, &[]);
        let ctx = egui::Context::default();
        let mut reader = Reader::default();
        reader.projects.insert(project_key(&section), true);
        let draw = |reader: &mut Reader, events| {
            ui_frame(&ctx, events, |ui| {
                let _ = paint_project(ui, &section, reader, (2, 3), &Theme::default());
            })
        };
        let shapes = draw(&mut reader, vec![]);
        let at = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Show all milestones" => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .expect("pending milestone has no disclosure");
        draw(&mut reader, press(at, true));
        draw(&mut reader, press(at, false));
        let shapes = draw(&mut reader, vec![]);
        for expected in ["deliverable witness", "criterion witness"] {
            assert!(
                shapes.iter().any(
                    |s| matches!(&s.shape,egui::Shape::Text(t)if t.galley.text().contains(expected))
                ),
                "unreachable {expected}"
            );
        }
    }
    #[test]
    fn reordering_roadmap_console_chips_cannot_retarget_a_press() {
        let mut section = test_section(Plan::default(), &[1, 2]);
        let ctx = egui::Context::default();
        let mut selected = None;
        let mut draw = |section: &Section, events| {
            ui_frame(&ctx, events, |ui| {
                chips(
                    ui,
                    section,
                    &[Placement::Off, Placement::Off],
                    &Placement::Off,
                    &mut selected,
                );
            })
        };
        let shapes = draw(&section, vec![]);
        let at = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text().contains("Console 1") => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .unwrap();
        draw(&section, press(at, true));
        section.entries.reverse();
        draw(&section, press(at, false));
        assert_eq!(
            selected,
            Some(1),
            "release selected another console after reorder"
        );
    }

    #[test]
    fn declared_current_anchors_before_pending_and_unknown_is_not_done() {
        let plan = plan();
        assert_eq!(window(&plan, 0, 0), 2..3);
        assert_eq!(window(&plan, usize::MAX, usize::MAX), 0..3);
        assert!(milestone_title(&plan.milestones[2]).ends_with("0/2"));
        assert!(milestone_title(&plan.milestones[0]).ends_with("0/0"));
    }
    #[test]
    fn milestone_selection_keeps_attached_done_and_limits_open_items() {
        let milestone = Milestone {
            items: (0..9)
                .map(|i| panel_contract::plan::Item {
                    text: i.to_string(),
                    mark: Some(if i < 2 { Mark::Done } else { Mark::Pending }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let mut attached = vec![false; 9];
        attached[0] = true;
        attached[8] = true;
        assert_eq!(
            visible_items(&milestone, &attached, false),
            vec![0, 2, 3, 4, 5, 6, 8]
        );
        assert_eq!(
            visible_items(&milestone, &attached, true),
            (0..9).collect::<Vec<_>>()
        );
        assert!(started(&milestone, false));
        assert!(started(&Milestone::default(), true));
        assert!(!started(&Milestone::default(), false));
    }
    #[test]
    fn fallback_keeps_only_section_headings_and_their_summaries() {
        let panel = panel_contract::parse_document(r#"{"lines":[{"text":"Normal"},{"text":"Build","style":"section","summary":"3 left"}]}"#).unwrap();
        let section = Section {
            key: None,
            kind: super::super::model::Kind::Other,
            entries: vec![super::super::model::Entry {
                pane: 1,
                console_label: "Shell".into(),
                tab_label: "Tab".into(),
                snapshot: crate::panel::summary::Snapshot {
                    document: Some(Arc::new(panel)),
                    warning: None,
                    attention: None,
                    outside: false,
                },
            }],
            roadmap: None,
        };
        let headings = fallback(&section);
        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].0, "Shell");
        assert_eq!(headings[0].1.text, "Build");
        assert_eq!(headings[0].1.summary.as_deref(), Some("3 left"));
    }
    #[test]
    fn roadmap_reader_starts_folded_and_tracks_independent_choices() {
        let mut reader = Reader::default();
        assert!(!reader.projects.get("a").copied().unwrap_or(false));
        reader.projects.insert("a".into(), true);
        reader.milestones.insert(("a".into(), "m".into()));
        reader.all_milestones.insert("b".into());
        assert!(reader.projects["a"]);
        assert!(!reader.projects.get("b").copied().unwrap_or(false));
        assert!(reader.milestones.contains(&("a".into(), "m".into())));
        assert!(!reader.milestones.contains(&("b".into(), "m".into())));
    }
    #[test]
    fn placement_requires_matching_milestone_and_item_or_milestone_alone() {
        let p = |text| panel_contract::parse_document(text).unwrap();
        let plan = plan();
        assert_eq!(
            placement(
                Some(&p(r#"{"milestone":"H3","deliverable":"D","lines":[]}"#)),
                &plan
            ),
            Placement::Item(2, 0)
        );
        assert_eq!(
            placement(Some(&p(r#"{"milestone":"H3","lines":[]}"#)), &plan),
            Placement::Milestone(2)
        );
        for text in [
            r#"{"deliverable":"D","lines":[]}"#,
            r#"{"milestone":"H3","deliverable":"missing","lines":[]}"#,
            r#"{"milestone":"missing","lines":[]}"#,
        ] {
            assert_eq!(placement(Some(&p(text)), &plan), Placement::Off);
        }
    }
}
