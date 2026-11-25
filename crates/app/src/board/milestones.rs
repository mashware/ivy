mod columns;
pub(super) mod compact;
mod details;
mod projection;
pub(super) mod sections;
pub(super) mod state;
mod strip;
// «By milestone»: each project's roadmap as columns, a console's card under the deliverable its
// panel declares, and «Off the plan» for whatever declares none (D6: the place is never deduced).
use super::{
    model::{Entry, Section},
    roadmap::{Placement, placement},
};
use crate::theme::Roles;
use eframe::egui::{self, Align2, FontId, Rect, Stroke, pos2, vec2};
use panel_contract::{
    Mark,
    plan::{Milestone, Plan},
};

/// The responsive grid for projects without a roadmap.
const MIN_COLUMN: f32 = 260.0;
const GAP: f32 = 8.0;

/// One milestone with the consoles placed in it.
pub(super) struct Placed<'a> {
    /// Per deliverable, in the plan's order, the consoles that declared it.
    pub(super) items: Vec<Vec<&'a Entry>>,
    /// Consoles that declared the milestone and no deliverable the plan lists under it.
    pub(super) loose: Vec<&'a Entry>,
}

pub(super) struct Columns<'a> {
    pub(super) milestones: Vec<Placed<'a>>,
    pub(super) off: Vec<&'a Entry>,
}

pub(super) fn place<'a>(entries: &'a [Entry], plan: &Plan) -> Columns<'a> {
    let mut milestones: Vec<Placed> = plan
        .milestones
        .iter()
        .map(|m| Placed {
            items: vec![Vec::new(); m.items.len()],
            loose: Vec::new(),
        })
        .collect();
    let mut off = Vec::new();
    for entry in entries {
        match placement(entry.snapshot.document.as_deref(), plan) {
            Placement::Item(m, i) => milestones[m].items[i].push(entry),
            Placement::Milestone(m) => milestones[m].loose.push(entry),
            Placement::Off => off.push(entry),
        }
    }
    Columns { milestones, off }
}

/// «after Q4 · Refunds»: each dependency id resolved to the milestone it names, an unknown id kept
/// as written so a typo in the plan stays visible instead of vanishing.
pub(super) fn after(milestone: &Milestone, plan: &Plan) -> Option<String> {
    if milestone.depends_on.is_empty() {
        return None;
    }
    let names: Vec<String> = milestone
        .depends_on
        .iter()
        .map(|id| {
            plan.milestones
                .iter()
                .find(|m| m.id.as_deref() == Some(id.as_str()))
                .map_or_else(|| id.clone(), |m| format!("{id} · {}", m.title))
        })
        .collect();
    Some(format!("after {}", super::cards::clean(&names.join(", "))))
}

pub(super) fn progress(milestone: &Milestone) -> (usize, usize) {
    let done = milestone
        .items
        .iter()
        .filter(|i| i.mark == Some(Mark::Done))
        .count();
    (done, milestone.items.len())
}

/// How many columns fit side by side, and how wide each one is.
pub(super) fn grid(width: f32, columns: usize) -> (usize, f32) {
    let fit = (((width + GAP) / (MIN_COLUMN + GAP)).floor() as usize).clamp(1, columns.clamp(1, 4));
    (
        fit,
        ((width - GAP * (fit - 1) as f32) / fit as f32).max(1.0),
    )
}

/// What paints a card: the Board's own painter, handed in so this module never owns navigation.
pub(super) type CardPainter<'p> = dyn FnMut(&mut egui::Ui, &Entry, usize, usize, f32) + 'p;

pub(super) use columns::paint as paint_plan_with;

#[cfg(test)]
pub(super) fn paint_plan(
    ui: &mut egui::Ui,
    section: &Section,
    plan: &Plan,
    roles: &Roles,
    paint_card: &mut CardPainter,
) -> usize {
    let mut reader = state::Reader::default();
    reader.apply(&state::Action::ShowAll(true));
    paint_plan_with(
        ui,
        section,
        plan,
        roles,
        &mut reader,
        &super::filter::Filter::default(),
        &|entry| {
            super::observation::classify(
                entry.snapshot.document.as_deref(),
                None,
                std::time::SystemTime::now(),
            )
        },
        paint_card,
    )
}

pub(super) fn project_key(section: &Section) -> String {
    section.key.as_ref().map_or_else(
        || format!("name:{}", section.name()),
        |key| format!("{key:?}"),
    )
}

/// A project with no roadmap: its cards in one wrapping row, under «no roadmap».
pub(super) fn paint_loose(
    ui: &mut egui::Ui,
    section: &Section,
    paint_card: &mut CardPainter,
) -> usize {
    let (fit, width) = grid(ui.available_width(), section.entries.len());
    let mut rows = 0;
    for chunk in section.entries.chunks(fit.max(1)) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for (column, entry) in chunk.iter().enumerate() {
                ui.allocate_ui_with_layout(
                    vec2(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width);
                        paint_card(ui, entry, rows, column, width);
                    },
                );
            }
        });
        rows += 1;
    }
    rows
}

/// The rule under a project, so its columns do not run into the next project's header.
pub(super) fn rule(ui: &mut egui::Ui, roles: &Roles) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, roles.line),
    );
}

/// How many of a project's consoles sit in each claiming column (D18's three claims).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Claims {
    pub(super) wait: usize,
    pub(super) block: usize,
    pub(super) done: usize,
}

/// A project's name in its header ink, its consoles, how many sit in each claiming column, and
/// where its roadmap came from — or why it could not be read. Returns the source target so the
/// Board can route an explicit local-file click through its read-only document viewer.
pub(super) fn project_header(
    ui: &mut egui::Ui,
    section: &Section,
    claims: Claims,
    with_roadmap: bool,
    roles: &Roles,
) -> Option<Rect> {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), egui::Sense::hover());
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let y = rect.center().y;
    let square = Rect::from_center_size(pos2(rect.left() + 9.0, y), vec2(10.0, 10.0));
    painter.rect_filled(square, 2.0, section.ink.unwrap_or(roles.tx3));
    let mut x = square.right() + 8.0;
    let name = FontId::new(14.0, crate::fonts::ui_weight(ui.ctx(), 600));
    x += painter
        .text(
            pos2(x, y),
            Align2::LEFT_CENTER,
            super::cards::clean(section.name()),
            name,
            roles.tx,
        )
        .width()
        + 10.0;
    let consoles = section.entries.len();
    let mut detail = format!(
        "{consoles} {}",
        if consoles == 1 { "console" } else { "consoles" }
    );
    // «By state» shows no roadmap, so it says nothing about one; «Other consoles» can never have one.
    let with_roadmap =
        with_roadmap && matches!(section.kind, super::model::Kind::Repository { .. });
    let error = section
        .roadmap
        .as_ref()
        .and_then(|r| r.error.as_deref())
        .filter(|_| with_roadmap);
    if with_roadmap && section.roadmap.as_ref().is_none_or(|r| r.plan.is_none()) && error.is_none()
    {
        detail.push_str(" · no roadmap");
    }
    x += painter
        .text(
            pos2(x, y),
            Align2::LEFT_CENTER,
            detail,
            FontId::proportional(12.0),
            roles.tx3,
        )
        .width()
        + 12.0;
    use panel_contract::Attention;
    for (count, class) in [
        (claims.wait, Attention::Wait),
        (claims.block, Attention::Block),
        (claims.done, Attention::Done),
    ] {
        if count == 0 {
            continue;
        }
        let pill = Rect::from_min_size(pos2(x, y - 9.0), vec2(crate::ui::claim_pill::SIZE, 18.0));
        crate::ui::claim_pill::paint(
            &painter,
            pill,
            crate::panel::AttentionState {
                class,
                backed: true,
            },
            roles,
        );
        x = pill.right() + 4.0;
        x += painter
            .text(
                pos2(x, y),
                Align2::LEFT_CENTER,
                count.to_string(),
                FontId::monospace(11.5),
                roles.tx2,
            )
            .width()
            + 10.0;
    }
    // A usable plan can carry diagnostics about fields the parser dropped.
    if let Some(error) = error {
        let label = if section.roadmap.as_ref().is_some_and(|r| r.plan.is_some()) {
            "roadmap"
        } else {
            "roadmap unreadable"
        };
        painter.text(
            pos2(rect.right() - 4.0, y),
            Align2::RIGHT_CENTER,
            format!("{label}: {}", super::cards::clean(error)),
            FontId::proportional(11.5),
            roles.block,
        );
    } else if let Some(source) = section
        .roadmap
        .as_ref()
        .filter(|r| with_roadmap && r.plan.is_some())
    {
        let source_rect = Rect::from_min_max(
            pos2(rect.right() - (rect.width() * 0.36).min(420.0), rect.top()),
            rect.max,
        );
        let old = source.published_at.is_some_and(|at| {
            std::time::SystemTime::now()
                .duration_since(at)
                .unwrap_or_default()
                .as_secs()
                > 86400
        });
        let label = if source.path.is_some() {
            format!("{} ↗", source.source)
        } else {
            source.source.clone()
        };
        strip::text(
            ui,
            source_rect,
            &label,
            FontId::monospace(11.0),
            if old { roles.wait } else { roles.tx3 },
            1,
        );
        ui.interact(
            source_rect,
            ui.id().with(("roadmap-source", project_key(section))),
            egui::Sense::hover(),
        )
        .on_hover_text(format!(
            "{}\n{}",
            source.source,
            if source.path.is_some() {
                "Open roadmap"
            } else {
                "Local roadmap source is unavailable"
            }
        ));
        let target = source_rect.intersect(ui.clip_rect());
        return target.is_positive().then_some(target);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Plan {
        panel_contract::plan::parse_document_noting(
            r#"{"milestones":[
                {"id":"Q4","title":"Refunds","items":[{"id":"R1","text":"Partial refunds","mark":"current"},{"id":"R2","text":"Audit log"}]},
                {"id":"Q5","title":"Invoices v2","depends_on":["Q4","Q9"],"items":[{"id":"I1","text":"PDF","mark":"done"},{"text":"Tax lines"}]}
            ]}"#,
        )
        .unwrap()
        .0
    }
    fn entry(pane: usize, milestone: Option<&str>, deliverable: Option<&str>) -> Entry {
        let mut doc = serde_json::json!({"lines": ["x"]});
        if let Some(m) = milestone {
            doc["milestone"] = m.into();
        }
        if let Some(d) = deliverable {
            doc["deliverable"] = d.into();
        }
        Entry {
            pane,
            console_label: format!("c{pane}"),
            tab_label: "t".into(),
            snapshot: crate::panel::summary::Snapshot {
                document: Some(std::sync::Arc::new(
                    panel_contract::parse_document(&doc.to_string()).unwrap(),
                )),
                warning: None,
                attention: None,
                outside: false,
            },
        }
    }
    fn panes(entries: &[&Entry]) -> Vec<usize> {
        entries.iter().map(|e| e.pane).collect()
    }

    #[test]
    fn a_console_sits_under_the_deliverable_it_declares_and_nothing_is_deduced() {
        let entries = vec![
            entry(1, Some("Q4"), Some("R2")),
            entry(2, Some("Q5"), None),
            entry(3, None, None),
            entry(4, Some("Q4"), Some("nope")),
            entry(5, Some("Q7"), Some("R1")),
        ];
        let columns = place(&entries, &plan());
        assert_eq!(panes(&columns.milestones[0].items[1]), [1]);
        assert!(columns.milestones[0].items[0].is_empty());
        assert_eq!(panes(&columns.milestones[1].loose), [2]);
        assert_eq!(
            panes(&columns.milestones[0].loose),
            [4],
            "unknown deliverable stays at milestone level"
        );
        assert_eq!(
            panes(&columns.off),
            [3, 5],
            "no milestone, or one the plan lacks, is off the plan"
        );
    }

    #[test]
    fn dependencies_read_as_titles_and_an_unknown_id_stays_literal() {
        let plan = plan();
        assert_eq!(after(&plan.milestones[0], &plan), None);
        assert_eq!(
            after(&plan.milestones[1], &plan).as_deref(),
            Some("after Q4 · Refunds, Q9")
        );
        assert_eq!(progress(&plan.milestones[1]), (1, 2));
    }

    #[test]
    fn columns_wrap_before_they_get_narrower_than_a_readable_card() {
        assert_eq!(grid(1200.0, 3).0, 3);
        assert_eq!(grid(1200.0, 9).0, 4);
        let (fit, width) = grid(500.0, 3);
        assert_eq!(fit, 1);
        assert_eq!(width, 500.0);
    }

    fn section(entries: Vec<Entry>, plan: Option<Plan>, error: Option<&str>) -> Section {
        Section {
            key: None,
            kind: crate::board::model::Kind::Repository {
                name: "billing".into(),
            },
            entries,
            roadmap: (plan.is_some() || error.is_some()).then(|| crate::board::roadmap::Resolved {
                plan: plan.map(std::sync::Arc::new),
                source: "ROADMAP.md".into(),
                error: error.map(str::to_owned),
                published_at: None,
                path: None,
            }),
            ink: None,
        }
    }

    /// Runs one painting and returns what the card painter was asked: `(pane, row, column)`.
    fn laid_out(width: f32, section: &Section, plan: &Plan) -> Vec<(usize, usize, usize)> {
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let roles = Roles::of(&crate::theme::Theme::default());
        let mut asked = Vec::new();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_width(width);
            let mut card = |ui: &mut egui::Ui, entry: &Entry, row: usize, column: usize, _: f32| {
                ui.add_space(40.0);
                asked.push((entry.pane, row, column));
            };
            paint_plan(ui, section, plan, &roles, &mut card);
        })
        .drop_without_applying_deltas();
        asked
    }

    #[test]
    fn milestone_columns_keep_their_horizontal_identity_when_the_row_overflows() {
        let plan = panel_contract::plan::parse_document_noting(
            r#"{"milestones":[{"id":"A","title":"A","items":[]},{"id":"B","title":"B","items":[]},{"id":"C","title":"C","items":[]}]}"#,
        )
        .unwrap()
        .0;
        let entries = vec![entry(1, Some("A"), None), entry(3, Some("C"), None)];
        let section = section(entries, None, None);
        // The row overflows horizontally; C remains the third column.
        let asked = laid_out(600.0, &section, &plan);
        let a = asked.iter().find(|(p, ..)| *p == 1).unwrap();
        let c = asked.iter().find(|(p, ..)| *p == 3).unwrap();
        assert_eq!(a.2, 0);
        assert_eq!(c.2, 2, "C keeps its horizontal column");
        assert_eq!(c.1, a.1, "columns share the same navigation row");
    }

    #[test]
    fn a_card_lands_under_the_deliverable_it_declares_not_merely_in_its_milestone() {
        // Given in the opposite order to the plan's: loose first, then R2, then R1.
        let entries = vec![
            entry(0, Some("Q4"), None),
            entry(1, Some("Q4"), Some("R2")),
            entry(2, Some("Q4"), Some("R1")),
        ];
        let section = section(entries, None, None);
        let asked = laid_out(1200.0, &section, &plan());
        // Milestone-level cards precede items, which remain in roadmap order.
        let order: Vec<usize> = asked.iter().map(|(p, ..)| *p).collect();
        assert_eq!(order, [0, 2, 1]);
        assert_eq!(
            progress(&plan().milestones[0]),
            (0, 2),
            "current is not done"
        );
    }

    fn header_texts(section: &Section) -> Vec<String> {
        header_texts_with(section, true)
    }

    fn header_texts_with(section: &Section, with_roadmap: bool) -> Vec<String> {
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let roles = Roles::of(&crate::theme::Theme::default());
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            project_header(
                ui,
                section,
                Claims {
                    wait: 2,
                    block: 0,
                    done: 1,
                },
                with_roadmap,
                &roles,
            );
        });
        let texts = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
        texts
    }

    #[test]
    fn the_project_header_names_its_source_its_claims_or_why_its_roadmap_failed() {
        let texts = header_texts(&section(vec![entry(1, None, None)], Some(plan()), None));
        for expected in ["billing", "1 console", "2", "1", "ROADMAP.md"] {
            assert!(
                texts.iter().any(|t| t == expected),
                "missing {expected} in {texts:?}"
            );
        }
        let texts = header_texts(&section(vec![], None, None));
        assert!(texts.iter().any(|t| t == "0 consoles · no roadmap"));
        let texts = header_texts(&section(vec![], None, Some("not UTF-8")));
        assert!(texts.iter().any(|t| t == "roadmap unreadable: not UTF-8"));
        assert!(
            !texts.iter().any(|t| t.contains("no roadmap")),
            "an error is not an absence"
        );
    }

    #[test]
    fn a_milestones_criterion_and_the_plans_outside_work_are_painted() {
        let plan = panel_contract::plan::parse_document_noting(
            r#"{"milestones":[{"id":"Q4","title":"Refunds","done_when":"refunds settle in one day","items":[{"text":"Webhooks"}]}],"outside":[{"text":"Sign the Stripe contract","mark":"pending"}]}"#,
        )
        .unwrap()
        .0;
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let roles = Roles::of(&crate::theme::Theme::default());
        let section = section(vec![], None, None);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_width(1200.0);
            paint_plan(ui, &section, &plan, &roles, &mut |_, _, _, _, _| {});
        });
        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
        for expected in [
            "Done when: refunds settle in one day",
            "Sign the Stripe contract",
        ] {
            assert!(
                texts.iter().any(|t| t == expected),
                "missing {expected} in {texts:?}"
            );
        }
    }

    #[test]
    fn by_state_headers_and_other_consoles_say_nothing_about_a_roadmap() {
        let says = |texts: Vec<String>, needle: &str| texts.iter().any(|t| t.contains(needle));
        let with_plan = section(vec![entry(1, None, None)], Some(plan()), None);
        assert!(!says(header_texts_with(&with_plan, false), "ROADMAP.md"));
        let without = section(vec![entry(1, None, None)], None, None);
        assert!(!says(header_texts_with(&without, false), "no roadmap"));
        let unreadable = section(vec![entry(1, None, None)], None, Some("not UTF-8"));
        assert!(!says(
            header_texts_with(&unreadable, false),
            "roadmap unreadable"
        ));
        let mut other = section(vec![entry(1, None, None)], None, None);
        other.kind = crate::board::model::Kind::Other;
        assert!(!says(header_texts_with(&other, true), "no roadmap"));
    }
}

#[cfg(test)]
#[path = "milestones/tests.rs"]
mod specification_tests;
