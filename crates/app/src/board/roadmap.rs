//! Where a project's roadmap comes from — a published plan or a configured file — and where a
//! console sits in it. The plan is inert text: nothing here opens a link or runs an action.
use super::model::Section;
use panel_contract::{Panel, plan::Plan};
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

#[derive(Clone)]
pub(super) struct Resolved {
    pub(super) plan: Option<Arc<Plan>>,
    pub(super) source: String,
    pub(super) error: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> Plan {
        panel_contract::plan::parse_document_noting(r#"{"milestones":[{"id":"empty","title":"Empty"},{"id":"old","title":"Old","items":[{"text":"old","mark":"pending"}]},{"id":"H3","title":"Now","items":[{"id":"D","text":"work","mark":"current"},{"text":"unknown","mark":"future"}]}]}"#).unwrap().0
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
}
