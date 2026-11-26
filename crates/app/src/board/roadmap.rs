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
    pub(super) published_at: Option<SystemTime>,
    pub(super) path: Option<std::path::PathBuf>,
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

fn source_path(section: &Section, plan: &Plan) -> Option<std::path::PathBuf> {
    let source = std::path::Path::new(plan.source.as_deref()?);
    if source.is_absolute() {
        return Some(source.into());
    }
    if plan.source.as_deref()?.contains("://") {
        return None;
    }
    let crate::project::Key::Repository(common) = section.key.as_ref()? else {
        return None;
    };
    (common.file_name().is_some_and(|name| name == ".git"))
        .then(|| common.parent().unwrap_or(common).join(source))
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
                published_at: Some(published.published_at),
                path: source_path(section, &published.plan),
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
                    source: match file.plan.as_ref().and_then(|plan| plan.source.as_deref()) {
                        Some(source) => format!("from {} | {source}", file.path.display()),
                        None => format!("from {}", file.path.display()),
                    },
                    published_at: None,
                    path: Some(file.path.clone()),
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

    #[test]
    fn json_file_and_published_plan_have_the_same_content_and_placement() {
        use crate::project::{Identity, Key};
        use panel_control::{Answer, What};
        use std::{collections::BTreeMap, fs, sync::mpsc, time::Instant};

        struct Temp(std::path::PathBuf);
        impl Drop for Temp {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let mut bytes = [0; 8];
        getrandom::fill(&mut bytes).unwrap();
        let dir = Temp(
            std::env::temp_dir().join(format!("ivy-json-roadmap-{:x}", u64::from_ne_bytes(bytes))),
        );
        fs::create_dir(&dir.0).unwrap();
        let path = dir.0.join("roadmap.json");
        let document = r#"{
          "title":"Roadmap", "source":"Tracker",
          "milestones":[
            {"id":"H1","title":"First","items":[{"id":"A","text":"Done","mark":"done","ref":"R-1","link":"https://example.com/1"}]},
            {"id":"H2","title":"Second","items":[{"id":"B","text":"Current","mark":"current"}]},
            {"id":"H3","title":"Third","done_when":"everything works","depends_on":["H1","H2"],"items":[
              {"id":"C","text":"Pending","mark":"pending"},
              {"id":"D","text":"Waiting","mark":"wait"},
              {"id":"E","text":"Blocked","mark":"block"},
              {"id":"MT-16700","text":"Deliver","mark":"current","ref":"MT-16700","link":"https://example.com/issue"},
              "Plain item"]}
          ], "outside":["Review",{"id":"O","text":"Contract","mark":"done"},{"text":"Retained outside","mark":"future"}]
        }"#;
        fs::write(&path, document).unwrap();
        let ctx = eframe::egui::Context::default();
        let mut reader = crate::board::files::Reader::new(
            BTreeMap::from([("Sample".into(), path.clone())]),
            ctx.clone(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let snapshot = loop {
            let snapshots = reader.snapshots();
            if let Some(snapshot) = snapshots
                .into_iter()
                .find(|snapshot| snapshot.plan.is_some())
            {
                break snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "file reader did not load JSON within five seconds"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let file_plan = snapshot.plan.unwrap();
        let diagnostic = snapshot.error.expect("dropped mark is a file diagnostic");
        assert!(diagnostic.contains("future"));
        let identity = Identity {
            key: Key::Repository(dir.0.clone()),
            path: dir.0.clone(),
            name: "Sample".into(),
        };
        let (parsed, dropped) = panel_contract::plan::parse_document_noting(document).unwrap();
        let mut store = crate::board::plans::Store::at(None, false, ctx.clone());
        let (tx, rx) = mpsc::sync_channel(1);
        store.request(
            Ok(identity.clone()),
            What::PlanSet {
                document: document.into(),
                plan: parsed,
                dropped,
            },
            tx,
            &ctx,
            #[cfg(windows)]
            None,
        );
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            Answer::Text(_)
        ));
        let published = store.published(&identity.key).unwrap();
        assert_eq!(*file_plan, published.plan);
        for (milestone, deliverable, expected) in [
            ("H1", "A", Placement::Item(0, 0)),
            ("H2", "B", Placement::Item(1, 0)),
            ("H3", "MT-16700", Placement::Item(2, 3)),
        ] {
            let panel = panel_contract::parse_document(
                &serde_json::json!({"milestone":milestone,"deliverable":deliverable,"lines":[]})
                    .to_string(),
            )
            .unwrap();
            assert_eq!(placement(Some(&panel), &file_plan), expected);
        }
        assert_eq!(
            crate::board::milestones::after(&file_plan.milestones[2], &file_plan).as_deref(),
            Some("after H1 · First, H2 · Second")
        );
        assert_eq!(
            file_plan.milestones[2].done_when.as_deref(),
            Some("everything works")
        );
        let section = crate::board::model::Section {
            key: None,
            kind: crate::board::model::Kind::Repository {
                name: "Sample".into(),
            },
            entries: vec![],
            roadmap: Some(Resolved {
                plan: Some(file_plan.clone()),
                source: format!("from {}", path.display()),
                error: Some(diagnostic.clone()),
                published_at: None,
                path: Some(path.clone()),
            }),
            ink: None,
        };
        crate::fonts::install_bundled(&ctx);
        let roles = crate::theme::Roles::of(&crate::theme::Theme::default());
        let output = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
            ui.set_width(1800.0);
            crate::board::milestones::project_header(
                ui,
                &section,
                Default::default(),
                true,
                &roles,
            );
            crate::board::milestones::paint_plan(
                ui,
                &section,
                &file_plan,
                &roles,
                &mut |_, _, _, _, _| {},
            );
        });
        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                eframe::egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
        for expected in [
            format!("roadmap: {diagnostic}"),
            "Done when: everything works".into(),
            "after H1 · First, H2 · Second".into(),
            "Review".into(),
        ] {
            assert!(
                texts.contains(&expected),
                "missing {expected:?} in {texts:?}"
            );
        }
    }
}
