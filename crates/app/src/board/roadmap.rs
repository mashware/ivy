//! A roadmap is inert text and groups of console cards; no links or actions are activated here.
mod presentation;
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

pub(super) fn project_open(reader: &Reader, section: &Section) -> bool {
    reader
        .projects
        .get(&project_key(section))
        .copied()
        .unwrap_or(false)
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
            .map_or(Placement::Milestone(m), |i| Placement::Item(m, i)),
    }
}

#[cfg(test)]
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
            },
            entries: Vec::new(),
            roadmap: None,
            ink: None,
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

fn done(m: &Milestone) -> bool {
    !m.items.is_empty() && m.items.iter().all(|i| i.mark == Some(Mark::Done))
}
fn started(m: &Milestone, attached: bool) -> bool {
    if done(m) {
        return attached;
    }
    attached
        || m.items.iter().any(|i| {
            matches!(
                i.mark,
                Some(Mark::Done | Mark::Current | Mark::Wait | Mark::Block)
            )
        })
}
fn visible_items(m: &Milestone, attached: &[bool], all: bool) -> Vec<usize> {
    let mut open = 0;
    m.items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            if all || attached.get(i).copied().unwrap_or(false) {
                return Some(i);
            }
            if item.mark == Some(Mark::Done) {
                return None;
            }
            open += 1;
            (open <= 5).then_some(i)
        })
        .collect()
}
fn progress(m: &Milestone) -> (usize, usize) {
    (
        m.items
            .iter()
            .filter(|i| i.mark == Some(Mark::Done))
            .count(),
        m.items.len(),
    )
}
fn milestone_key(project: &str, m: &Milestone, index: usize) -> (String, String) {
    (
        project.to_owned(),
        m.id.clone()
            .unwrap_or_else(|| format!("#{index}:{}", m.title)),
    )
}
struct Surface<'a> {
    theme: &'a Theme,
    signals: &'a HashMap<usize, crate::agent_signals::View>,
    now: SystemTime,
    board_totals: Option<(usize, usize)>,
}
fn chip(
    ui: &mut egui::Ui,
    entry: &super::model::Entry,
    selected: &mut Option<usize>,
    surface: &Surface<'_>,
) {
    let column = super::observation::classify(
        entry.snapshot.document.as_deref(),
        surface.signals.get(&entry.pane).copied(),
        surface.now,
    );
    let title = super::cards::clean(super::cards::title(entry).split(" · ").next().unwrap_or(""));
    let id = egui::Id::new(("board-roadmap-console", ui.ctx().viewport_id(), entry.pane));
    if ui
        .scope_builder(egui::UiBuilder::new().id(id), |ui| {
            super::style::console_chip(ui, column, &title, surface.theme)
                .on_hover_text(format!(
                    "{}\nGo to {} · {}",
                    title, entry.console_label, entry.tab_label
                ))
                .clicked()
        })
        .inner
    {
        *selected = Some(entry.pane);
    }
}
fn chips(
    ui: &mut egui::Ui,
    section: &Section,
    positions: &[Placement],
    wanted: &Placement,
    selected: &mut Option<usize>,
    surface: &Surface<'_>,
) {
    for (entry, position) in section.entries.iter().zip(positions) {
        if position == wanted && (*wanted != Placement::Off || entry.snapshot.document.is_some()) {
            chip(ui, entry, selected, surface);
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
                    .map(move |line| (super::cards::title(entry), line))
            })
        })
        .collect()
}

#[cfg(test)]
pub(super) fn paint_project(
    ui: &mut egui::Ui,
    section: &Section,
    reader: &mut Reader,
    window: (usize, usize),
    theme: &Theme,
) -> Option<usize> {
    paint_project_with_signals(
        ui,
        section,
        reader,
        window,
        theme,
        &HashMap::new(),
        SystemTime::now(),
        None,
    )
}
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_project_with_signals(
    ui: &mut egui::Ui,
    section: &Section,
    reader: &mut Reader,
    window: (usize, usize),
    theme: &Theme,
    signals: &HashMap<usize, crate::agent_signals::View>,
    now: SystemTime,
    board_totals: Option<(usize, usize)>,
) -> Option<usize> {
    let id = egui::Id::new((
        "board-roadmap-project",
        ui.ctx().viewport_id(),
        project_key(section),
    ));
    let surface = Surface {
        theme,
        signals,
        now,
        board_totals,
    };
    ui.scope_builder(egui::UiBuilder::new().id(id), |ui| {
        presentation::paint_project_contents(ui, section, reader, window, &surface)
    })
    .inner
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
            ink: None,
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
    fn multiple_long_attachments_stay_inside_their_milestone_column() {
        let plan = panel_contract::plan::parse_document_noting(r#"{"milestones":[{"id":"M","title":"Now","items":[{"id":"D","text":"Deliverable","mark":"current"}]}]}"#).unwrap().0;
        let mut section = test_section(plan, &[1, 2, 3]);
        for entry in &mut section.entries {
            entry.snapshot.document = Some(Arc::new(
                panel_contract::parse_document(
                    &serde_json::json!({
                        "milestone":"M", "deliverable":"D", "lines":[{"text": "Long attached title ".repeat(80), "style":"title"}]
                    })
                    .to_string(),
                )
                .unwrap(),
            ));
        }
        let ctx = egui::Context::default();
        let mut reader = Reader::default();
        reader.projects.insert(project_key(&section), true);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_width(440.0);
            super::super::style::apply(ui, &Theme::default());
            let left = ui.min_rect().left();
            let _ = paint_project(ui, &section, &mut reader, (2, 3), &Theme::default());
            assert!(
                ui.min_rect().right() <= left + 440.1,
                "roadmap expanded to {}",
                ui.min_rect().width()
            );
        })
        .drop_without_applying_deltas();
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
                egui::Shape::Text(t) if t.galley.text().contains("M Pending 0/1") => {
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
        for entry in &mut section.entries {
            entry.snapshot.document = Some(Arc::new(
                panel_contract::parse_document(r#"{"lines":[]}"#).unwrap(),
            ));
        }
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
                    &Surface {
                        theme: &Theme::default(),
                        signals: &HashMap::new(),
                        now: SystemTime::now(),
                        board_totals: None,
                    },
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
    fn reordering_milestones_cannot_retarget_a_disclosure_press() {
        let mut plan = Plan::default();
        for id in ["a", "b"] {
            plan.milestones.push(Milestone {
                id: Some(id.into()),
                title: "Same shape".into(),
                items: (0..7)
                    .map(|i| panel_contract::plan::Item {
                        text: format!("Item {i}"),
                        mark: Some(Mark::Current),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            });
        }
        let mut section = test_section(plan, &[]);
        let ctx = egui::Context::default();
        let mut reader = Reader::default();
        reader.projects.insert(project_key(&section), true);
        let mut draw = |section: &Section, events| {
            ui_frame(&ctx, events, |ui| {
                let _ = paint_project(ui, section, &mut reader, (2, 3), &Theme::default());
            })
        };
        let shapes = draw(&section, vec![]);
        let at = shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text().contains("Show all 7") => {
                    Some(t.pos + t.galley.size() / 2.0)
                }
                _ => None,
            })
            .expect("milestone disclosure");
        draw(&section, press(at, true));
        Arc::make_mut(section.roadmap.as_mut().unwrap().plan.as_mut().unwrap())
            .milestones
            .reverse();
        draw(&section, press(at, false));
        assert!(
            reader
                .milestones
                .contains(&(project_key(&section), "a".into()))
        );
        assert!(
            !reader
                .milestones
                .contains(&(project_key(&section), "b".into()))
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
        attached[2] = true;
        assert_eq!(
            visible_items(&milestone, &attached, false),
            vec![0, 2, 3, 4, 5, 6, 7, 8],
            "attached unfinished items must not consume the five other open-item slots"
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
            ink: None,
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
        assert_eq!(
            placement(
                Some(&p(
                    r#"{"milestone":"H3","deliverable":"missing","lines":[]}"#
                )),
                &plan
            ),
            Placement::Milestone(2)
        );
        for text in [
            r#"{"deliverable":"D","lines":[]}"#,
            r#"{"milestone":"missing","lines":[]}"#,
        ] {
            assert_eq!(placement(Some(&p(text)), &plan), Placement::Off);
        }
    }

    fn text_at(shapes: &[egui::epaint::ClippedShape], needle: &str) -> Option<egui::Pos2> {
        shapes.iter().find_map(|s| match &s.shape {
            egui::Shape::Text(t) => {
                let start = t.galley.text().find(needle)?;
                let at = t
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(
                        t.galley.text()[..start].chars().count() + needle.chars().count() / 2,
                    ))
                    .center()
                    + t.pos.to_vec2();
                s.clip_rect.contains(at).then_some(at)
            }
            _ => None,
        })
    }

    #[test]
    fn exact_reference_plan_preserves_all_items_and_paints_three_active_cards_and_every_outside_item()
     {
        let (plan, _) = panel_contract::plan::parse_document_noting(include_str!(
            "../../tests/fixtures/board-reference/plan.json"
        ))
        .unwrap();
        assert_eq!(
            plan.milestones
                .iter()
                .map(|m| m.items.len())
                .collect::<Vec<_>>(),
            [4, 4, 3, 3]
        );
        assert_eq!(
            plan.milestones
                .iter()
                .map(progress)
                .fold((0, 0), |(d, n), (x, y)| (d + x, n + y)),
            (4, 14)
        );
        assert_eq!(plan.outside.len(), 3);
        let mut section = test_section(plan, &[1, 2, 3, 4, 5]);
        let docs = [
            include_str!("../../tests/fixtures/board-reference/t2g.panel.json"),
            include_str!("../../tests/fixtures/board-reference/t2h.panel.json"),
            include_str!("../../tests/fixtures/board-reference/t2j.panel.json"),
            include_str!("../../tests/fixtures/board-reference/t2k.panel.json"),
            include_str!("../../tests/fixtures/board-reference/t2c.panel.json"),
        ];
        for (entry, doc) in section.entries.iter_mut().zip(docs) {
            entry.snapshot.document = Some(Arc::new(panel_contract::parse_document(doc).unwrap()));
        }
        let ctx = egui::Context::default();
        let mut reader = Reader::default();
        let draw = |reader: &mut Reader, events| {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1500.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.set_width(1500.0);
                    super::super::style::apply(ui, &Theme::default());
                    let _ = paint_project(ui, &section, reader, (2, 3), &Theme::default());
                },
            );
            let shapes = output.shapes.clone();
            output.drop_without_applying_deltas();
            shapes
        };
        let folded = draw(&mut reader, vec![]);
        assert!(text_at(&folded, "4/14").is_some());
        assert!(text_at(&folded, "now M1, M2, M3").is_some());
        assert!(text_at(&folded, "0/4 milestones").is_none());
        let open = text_at(&folded, "Roadmap").unwrap();
        draw(&mut reader, press(open, true));
        draw(&mut reader, press(open, false));
        let expanded = draw(&mut reader, vec![]);
        for text in [
            "0/4 milestones · 4/14 deliverables",
            "Outside the code · 2 pending:",
            "Revisión legal del RGPD",
            "Contrato con Stripe",
            "Auditoría de seguridad",
            "From docs/roadmap.md",
            "after M1, M3",
            "Notificaciones sin duplicados",
            "Informes programados por email",
        ] {
            assert!(
                text_at(&expanded, text).is_some(),
                "reference information omitted {text:?}"
            );
        }
        let titles = [
            "Bandeja multi-perfil",
            "Facturación en Stripe v3",
            "Informes",
        ]
        .map(|t| text_at(&expanded, t).unwrap());
        assert!(
            titles
                .windows(2)
                .all(|p| p[0].x < p[1].x && (p[0].y - p[1].y).abs() < 1.0),
            "active milestones must share one row: {titles:?}"
        );
        let text_edge = |needle: &str, end: bool| {
            expanded
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) => {
                        if end && t.galley.text() != needle {
                            return None;
                        }
                        let start = t.galley.text().find(needle)?;
                        let cursor = t.galley.text()[..start].chars().count()
                            + if end { needle.chars().count() } else { 0 };
                        Some(
                            t.pos.x
                                + t.galley
                                    .pos_from_cursor(egui::text::CCursor::new(cursor))
                                    .left(),
                        )
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {needle}"))
        };
        for (id, title) in [
            ("M1", "Bandeja multi-perfil"),
            ("M2", "Facturación en Stripe v3"),
            ("M3", "Informes"),
        ] {
            let gap = text_edge(title, false) - text_edge(id, true);
            assert!(
                (4.0..=12.0).contains(&gap),
                "{id} title separated from its ID by {gap}"
            );
        }
        for items in [
            vec![
                "Bandeja y búsqueda por perfil",
                "Página de ajustes",
                "Notificaciones sin duplicados",
            ],
            vec!["Pruebas en sandbox", "Corte en producción"],
            vec![
                "Exportar a CSV",
                "Gráficas de actividad",
                "Informes programados por email",
            ],
        ] {
            let lefts: Vec<_> = items.iter().map(|text| text_edge(text, false)).collect();
            assert!(
                lefts.windows(2).all(|p| (p[0] - p[1]).abs() < 0.5),
                "deliverable left edges differ: {items:?} {lefts:?}"
            );
        }
        let blocked = text_at(&expanded, "Pruebas en sandbox").unwrap();
        let block_left = text_edge("Pruebas en sandbox", false);
        assert!(expanded.iter().any(|s|matches!(&s.shape,egui::Shape::Circle(c) if c.stroke.color==Theme::default().color(term_core::TermColor::Indexed(1)) && c.center.x<block_left && c.center.x>block_left-18.0 && (c.center.y-blocked.y).abs()<9.0)),"blocked deliverable must use the same red/orange mark as its blocked console");
        let card_widths: Vec<_> = titles
            .iter()
            .map(|at| {
                expanded
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Rect(r)
                            if r.rect.contains(*at)
                                && r.stroke.color == Theme::default().focus()
                                && r.rect.height() > 50.0 =>
                        {
                            Some(r.rect.width())
                        }
                        _ => None,
                    })
                    .next()
                    .expect("milestone frame")
            })
            .collect();

        assert!(expanded.iter().filter(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect.height()==4.0 && r.rect.width()==60.0)).count()>=3,"missing per-milestone progress tracks");
        assert_eq!(expanded.iter().filter(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains("2 done"))).count(),2,"summary must count attached done items as well as hidden done items");
        for shape in &expanded {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text().contains("2 done")
            {
                let near = egui::Rect::from_min_max(
                    text.pos - egui::vec2(18.0, 1.0),
                    text.pos + egui::vec2(0.0, 18.0),
                );
                let strokes=expanded.iter().filter(|s|matches!(&s.shape,egui::Shape::LineSegment{points,stroke} if stroke.color==Theme::default().focus() && points[0].x!=points[1].x && points[0].y!=points[1].y && near.contains(points[0]) && near.contains(points[1]))).count();
                assert_eq!(
                    strokes, 2,
                    "done summary needs a visible checkmark without relying on font coverage"
                );
            }
        }

        for forbidden in ["Aprobar", "sí, adelante"] {
            assert!(
                text_at(&expanded, forbidden).is_none(),
                "producer command leaked into roadmap controls"
            );
        }
        let close = text_at(&expanded, "Roadmap").unwrap();
        draw(&mut reader, press(close, true));
        draw(&mut reader, press(close, false));
        let folded = draw(&mut reader, vec![]);
        assert!(text_at(&folded, "0/4 milestones").is_none());
        assert!(text_at(&folded, "Roadmap").is_some());
        let individual = text_at(&folded, "Roadmap").unwrap();
        draw(&mut reader, press(individual, true));
        draw(&mut reader, press(individual, false));
        assert!(
            text_at(&draw(&mut reader, vec![]), "0/4 milestones").is_some(),
            "individual project disclosure failed"
        );
        assert!(
            card_widths.windows(2).all(|p| (p[0] - p[1]).abs() < 1.0),
            "milestone cards differ in width: {card_widths:?}"
        );
    }
}
