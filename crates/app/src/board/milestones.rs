//! «By milestone»: each project's roadmap as columns, a console's card under the deliverable its
//! panel declares, and «Off the plan» for whatever declares none (D6: the place is never deduced).
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

/// Below this a column's cards stop being readable, so the columns wrap onto another row.
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
    let fit = (((width + GAP) / (MIN_COLUMN + GAP)).floor() as usize).clamp(1, columns.max(1));
    (
        fit,
        ((width - GAP * (fit - 1) as f32) / fit as f32).max(1.0),
    )
}

/// What paints a card: the Board's own painter, handed in so this module never owns navigation.
pub(super) type CardPainter<'p> = dyn FnMut(&mut egui::Ui, &Entry, usize, usize, f32) + 'p;

fn column_header(
    ui: &mut egui::Ui,
    width: f32,
    title: &str,
    progress: Option<(usize, usize)>,
    tally: usize,
    after: Option<&str>,
    roles: &Roles,
) {
    let height = if after.is_some() { 52.0 } else { 36.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    let font = FontId::new(13.0, crate::fonts::ui_weight(ui.ctx(), 600));
    // A milestone counts deliverables done; «Off the plan» has none, so it counts its consoles.
    let counted = progress.map_or_else(|| tally.to_string(), |(done, all)| format!("{done}/{all}"));
    let count_at = pos2(rect.right() - 4.0, rect.top() + 12.0);
    let count_width = painter
        .text(
            count_at,
            Align2::RIGHT_CENTER,
            counted,
            FontId::monospace(11.0),
            roles.tx3,
        )
        .width();
    let clip = Rect::from_min_max(rect.min, pos2(count_at.x - count_width - 8.0, rect.max.y));
    painter
        .with_clip_rect(clip.intersect(painter.clip_rect()))
        .text(
            pos2(rect.left() + 4.0, rect.top() + 12.0),
            Align2::LEFT_CENTER,
            title,
            font,
            roles.tx,
        );
    let track = Rect::from_min_size(
        pos2(rect.left() + 4.0, rect.top() + 26.0),
        vec2(width - 8.0, 3.0),
    );
    if progress.is_some() {
        painter.rect_filled(track, 1.5, roles.line);
    }
    if let Some((done, all)) = progress.filter(|(_, all)| *all > 0) {
        let filled = track.width() * done as f32 / all as f32;
        painter.rect_filled(
            Rect::from_min_size(track.min, vec2(filled, 3.0)),
            1.5,
            roles.accent,
        );
    }
    if let Some(after) = after {
        painter
            .with_clip_rect(rect.intersect(painter.clip_rect()))
            .text(
                pos2(rect.left() + 4.0, rect.top() + 42.0),
                Align2::LEFT_CENTER,
                after,
                FontId::proportional(11.5),
                roles.tx3,
            );
    }
}

/// One muted line under a milestone's header, cut at the column's edge.
fn note(ui: &mut egui::Ui, width: f32, text: &str, roles: &Roles) {
    let (row, _) = ui.allocate_exact_size(vec2(width, 18.0), egui::Sense::hover());
    ui.painter()
        .with_clip_rect(row.intersect(ui.clip_rect()))
        .text(
            pos2(row.left() + 4.0, row.center().y),
            Align2::LEFT_CENTER,
            super::cards::clean(text),
            FontId::proportional(11.5),
            roles.tx3,
        );
}

fn deliverable(ui: &mut egui::Ui, width: f32, text: &str, mark: Option<Mark>, roles: &Roles) {
    let (row, _) = ui.allocate_exact_size(vec2(width, 22.0), egui::Sense::hover());
    let (mark, ink, text_ink) = match mark {
        Some(Mark::Done) => (Mark::Done, roles.accent, roles.tx3),
        Some(Mark::Current) => (Mark::Current, roles.accent, roles.tx),
        Some(Mark::Block) => (Mark::Block, roles.block, roles.tx),
        Some(Mark::Wait) => (Mark::Wait, roles.wait, roles.tx),
        _ => (Mark::Pending, roles.tx3, roles.tx2),
    };
    let glyph = Rect::from_center_size(pos2(row.left() + 10.0, row.center().y), vec2(12.0, 16.0));
    crate::panel::body::paint_mark(ui.painter(), glyph, mark, ink, FontId::monospace(12.0));
    ui.painter()
        .with_clip_rect(row.intersect(ui.clip_rect()))
        .text(
            pos2(row.left() + 22.0, row.center().y),
            Align2::LEFT_CENTER,
            super::cards::clean(text),
            FontId::proportional(12.5),
            text_ink,
        );
}

/// A project whose plan is laid out as columns. `paint_card(ui, entry, row, column, width)` paints
/// one card at the next free spot; this returns how many navigation rows the project used.
pub(super) fn paint_plan(
    ui: &mut egui::Ui,
    section: &Section,
    plan: &Plan,
    roles: &Roles,
    paint_card: &mut CardPainter,
) -> usize {
    let placed = place(&section.entries, plan);
    let columns = plan.milestones.len() + 1;
    let (fit, width) = grid(ui.available_width(), columns);
    let mut rows_used = 0;
    let mut first = 0;
    while first < columns {
        let last = (first + fit).min(columns);
        let mut tallest = 0;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for index in first..last {
                ui.allocate_ui_with_layout(
                    vec2(width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(width);
                        let mut row = rows_used;
                        // The grid column on screen, not the milestone's index: ↑/↓ move between
                        // wrapped rows by it.
                        let column = index - first;
                        if let Some(milestone) = plan.milestones.get(index) {
                            let title = match &milestone.id {
                                Some(id) => format!("{id} · {}", milestone.title),
                                None => milestone.title.clone(),
                            };
                            let after = after(milestone, plan);
                            column_header(
                                ui,
                                width,
                                &super::cards::clean(&title),
                                Some(progress(milestone)),
                                0,
                                after.as_deref(),
                                roles,
                            );
                            if let Some(criterion) = &milestone.done_when {
                                note(ui, width, &format!("Done when: {criterion}"), roles);
                            }
                            for (item, consoles) in
                                milestone.items.iter().zip(&placed.milestones[index].items)
                            {
                                deliverable(ui, width, &item.text, item.mark, roles);
                                for entry in consoles {
                                    paint_card(ui, entry, row, column, width);
                                    row += 1;
                                }
                            }
                            for entry in &placed.milestones[index].loose {
                                paint_card(ui, entry, row, column, width);
                                row += 1;
                            }
                        } else {
                            let count = placed.off.len();
                            column_header(ui, width, "Off the plan", None, count, None, roles);
                            // The plan's own work outside its milestones (a contract, a review) is
                            // the plan's, not a console's: listed here, above the consoles.
                            for item in &plan.outside {
                                deliverable(ui, width, &item.text, item.mark, roles);
                            }
                            if placed.off.is_empty() && plan.outside.is_empty() {
                                ui.weak("—");
                            }
                            for entry in &placed.off {
                                paint_card(ui, entry, row, column, width);
                                row += 1;
                            }
                        }
                        tallest = tallest.max(row - rows_used);
                    },
                );
            }
        });
        rows_used += tallest.max(1);
        first = last;
    }
    rows_used
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
/// where its roadmap came from — or why it could not be read. The source is named, not linked: it
/// is a file path or a publication, and the Board opens only web addresses (`links::WebTarget`).
pub(super) fn project_header(
    ui: &mut egui::Ui,
    section: &Section,
    claims: Claims,
    with_roadmap: bool,
    roles: &Roles,
) {
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
    // An unreadable roadmap says why instead of passing for no roadmap.
    if let Some(error) = error {
        painter.text(
            pos2(rect.right() - 4.0, y),
            Align2::RIGHT_CENTER,
            format!("roadmap unreadable: {}", super::cards::clean(error)),
            FontId::proportional(11.5),
            roles.block,
        );
    } else if let Some(source) = section
        .roadmap
        .as_ref()
        .filter(|r| with_roadmap && r.plan.is_some())
    {
        painter.text(
            pos2(rect.right() - 4.0, y),
            Align2::RIGHT_CENTER,
            super::cards::clean(&source.source),
            FontId::monospace(11.0),
            roles.tx3,
        );
    }
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
    fn wrapped_milestones_restart_their_grid_column_so_up_and_down_stay_in_line() {
        let plan = panel_contract::plan::parse_document_noting(
            r#"{"milestones":[{"id":"A","title":"A","items":[]},{"id":"B","title":"B","items":[]},{"id":"C","title":"C","items":[]}]}"#,
        )
        .unwrap()
        .0;
        let entries = vec![entry(1, Some("A"), None), entry(3, Some("C"), None)];
        let section = section(entries, None, None);
        // Two columns fit: A and B on the first row, C and Off the plan on the second.
        let asked = laid_out(600.0, &section, &plan);
        let a = asked.iter().find(|(p, ..)| *p == 1).unwrap();
        let c = asked.iter().find(|(p, ..)| *p == 3).unwrap();
        assert_eq!(a.2, 0);
        assert_eq!(
            c.2, 0,
            "C is the first column of its row, not the third column overall"
        );
        assert!(c.1 > a.1, "C sits on a later navigation row");
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
        // Painted in the plan's order: under R1, under R2, then the loose card after them all.
        let order: Vec<usize> = asked.iter().map(|(p, ..)| *p).collect();
        assert_eq!(order, [2, 1, 0]);
        assert_eq!(
            progress(&plan().milestones[0]),
            (0, 2),
            "current is not done"
        );
    }

    fn header_texts(section: &Section) -> Vec<String> {
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
                true,
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
        let ctx = egui::Context::default();
        crate::fonts::install_bundled(&ctx);
        let roles = Roles::of(&crate::theme::Theme::default());
        let painted = |section: &Section, with_roadmap: bool| {
            let output = ctx.run_ui(egui::RawInput::default(), |ui| {
                project_header(ui, section, Claims::default(), with_roadmap, &roles);
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
            texts
        };
        let with_plan = section(vec![entry(1, None, None)], Some(plan()), None);
        assert!(!painted(&with_plan, false).iter().any(|t| t == "ROADMAP.md"));
        let mut other = section(vec![entry(1, None, None)], None, None);
        other.kind = crate::board::model::Kind::Other;
        assert!(
            !painted(&other, true)
                .iter()
                .any(|t| t.contains("no roadmap"))
        );
    }
}
