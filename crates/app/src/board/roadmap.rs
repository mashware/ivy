//! A roadmap is inert text and groups of console cards; no links or actions are activated here.
use super::model::{Band, Section};
use panel_contract::{
    Mark, Panel,
    plan::{Milestone, Plan},
};
use std::time::{Duration, SystemTime};

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

pub(super) fn project(
    section: &mut Section,
    plan: &Plan,
    source: String,
    done: usize,
    next: usize,
) {
    let entries = std::mem::take(&mut section.entries);
    let mut remaining: Vec<_> = entries.into_iter().map(Some).collect();
    let positions: Vec<_> = remaining
        .iter()
        .map(|e| {
            placement(
                e.as_ref().and_then(|e| e.snapshot.document.as_deref()),
                plan,
            )
        })
        .collect();
    section.bands.clear();
    section.bands.push(Band {
        title: None,
        count: 0,
    });
    band(section, source);
    if let Some(title) = &plan.title {
        band(section, title.clone());
    }
    if plan.milestones.is_empty() {
        band(section, "the published plan has no milestones".into());
    }
    let shown = window(plan, done, next);
    for (m, milestone) in plan.milestones.iter().enumerate() {
        // Cards remain reachable even when their milestone is outside the roadmap's text window.
        let has_cards = positions
            .iter()
            .any(|p| matches!(p, Placement::Item(n, _) | Placement::Milestone(n) if *n == m));
        if !shown.contains(&m) && !has_cards {
            continue;
        }
        band(section, milestone_title(milestone));
        take_cards(
            section,
            &mut remaining,
            &positions,
            &Placement::Milestone(m),
        );
        if shown.contains(&m) {
            if let Some(criterion) = &milestone.done_when {
                band(section, format!("Done when: {criterion}"));
            }
            if !milestone.depends_on.is_empty() {
                band(
                    section,
                    format!("Depends on: {}", milestone.depends_on.join(", ")),
                );
            }
        }
        for (i, item) in milestone.items.iter().enumerate() {
            if !shown.contains(&m) && !positions.contains(&Placement::Item(m, i)) {
                continue;
            }
            let reference = item.reference.as_deref().unwrap_or("");
            band(
                section,
                format!("  {} {} {}", symbol(item.mark), item.text, reference),
            );
            take_cards(section, &mut remaining, &positions, &Placement::Item(m, i));
        }
    }
    if !plan.outside.is_empty() {
        band(section, "Outside the plan".into());
        for item in &plan.outside {
            band(section, format!("  {} {}", symbol(item.mark), item.text));
        }
    }
    if remaining.iter().any(Option::is_some) {
        band(section, "Off the plan".into());
        for entry in remaining.into_iter().flatten() {
            section.entries.push(entry);
            section.bands.last_mut().unwrap().count += 1;
        }
    }
}
fn band(section: &mut Section, title: String) {
    section.bands.push(Band {
        title: Some(title),
        count: 0,
    });
}
fn take_cards(
    section: &mut Section,
    remaining: &mut [Option<super::model::Entry>],
    positions: &[Placement],
    wanted: &Placement,
) {
    for (entry, position) in remaining.iter_mut().zip(positions) {
        if position == wanted
            && let Some(entry) = entry.take()
        {
            section.entries.push(entry);
            section.bands.last_mut().unwrap().count += 1;
        }
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
            bands: vec![Band {
                title: None,
                count: 0,
            }],
        });
    }
    sections.sort_by_key(|s| matches!(s.kind, super::model::Kind::Other));
    let (done, next) = window.board.roadmap_window;
    for section in sections {
        if let Some(published) = section
            .key
            .as_ref()
            .and_then(|key| window.board.plans.published(key))
        {
            project(
                section,
                &published.plan,
                published_source(&published, now),
                done,
                next,
            );
        } else if let Some(file) = window.board.file_snapshots.iter().find(|file| {
            if let Some(key) = &section.key {
                file.identity.as_ref().is_some_and(|i| &i.key == key)
            } else {
                file.name == section.name()
            }
        }) {
            let source = format!("from {}", file.path.display());
            if let Some(plan) = &file.plan {
                project(section, plan, source, done, next);
            } else {
                section.bands = vec![
                    Band {
                        title: None,
                        count: 0,
                    },
                    Band {
                        title: Some(source),
                        count: 0,
                    },
                    Band {
                        title: file.error.clone(),
                        count: 0,
                    },
                    Band {
                        title: Some("Off the plan".into()),
                        count: section.entries.len(),
                    },
                ];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> Plan {
        panel_contract::plan::parse_document_noting(r#"{"milestones":[{"id":"empty","title":"Empty"},{"id":"old","title":"Old","items":[{"text":"old","mark":"pending"}]},{"id":"H3","title":"Now","items":[{"id":"D","text":"work","mark":"current"},{"text":"unknown","mark":"future"}]}]}"#).unwrap().0
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
