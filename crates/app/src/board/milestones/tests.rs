#![cfg(test)]
use super::*;
use super::{
    projection::{Class, Projection},
    state::{Action, Identity, Key, Reader, Target},
};
use crate::board::observation::Column;
use std::sync::Arc;

fn fixture() -> Plan {
    panel_contract::plan::parse_document_noting(include_str!(
        "../../../tests/fixtures/board-milestones/plan.json"
    ))
    .unwrap()
    .0
}
fn entry(pane: usize, m: Option<&str>, d: Option<&str>, attention: Option<&str>) -> Entry {
    Entry {pane,console_label:"Per-organization AEAT seal with lifecycle".into(),tab_label:"Billing".into(),snapshot:crate::panel::summary::Snapshot {document:Some(Arc::new(panel_contract::parse_document(&serde_json::json!({"milestone":m,"deliverable":d,"attention":attention,"lines":[{"text":"Task title","style":"title"},{"text":"Literal content"}]}).to_string()).unwrap())),warning:None,attention:None,outside:false}}
}
fn column(e: &Entry) -> Column {
    crate::board::observation::classify(
        e.snapshot.document.as_deref(),
        None,
        std::time::SystemTime::now(),
    )
}
fn section(entries: Vec<Entry>, plan: Plan) -> Section {
    Section {
        key: None,
        kind: crate::board::model::Kind::Repository {
            name: "TaxPlatform".into(),
        },
        entries,
        roadmap: Some(crate::board::roadmap::Resolved {
            plan: Some(Arc::new(plan)),
            source: "published 2h ago".into(),
            error: None,
            published_at: None,
            path: None,
        }),
        ink: None,
    }
}
#[test]
fn taxplatform_classification_uses_references_not_titles_or_partial_progress() {
    let plan = fixture();
    let entries = vec![
        entry(1, Some("H2"), Some("AEAT-SEAL"), None),
        entry(2, Some("H3"), Some("TERRITORIES"), Some("wait")),
    ];
    let placed = place(&entries, &plan);
    assert_eq!(placed.milestones[2].items[17][0].pane, 1);
    assert!(placed.off.is_empty());
    let result = Projection::new(&plan, &placed, &column);
    assert_eq!(
        result.classes,
        vec![
            Class::Done,
            Class::Done,
            Class::Active,
            Class::Active,
            Class::Claim(Column::Wait),
            Class::Later,
            Class::Ready,
            Class::Later,
            Class::Later,
            Class::Later,
            Class::Ready,
            Class::Later,
            Class::Later,
            Class::Later
        ]
    );
    assert_eq!(result.missing[7], ["H2 (17/18)", "H5 (0/14)"]);
    let unlinked = vec![entry(3, None, None, None)];
    assert_eq!(place(&unlinked, &plan).off[0].pane, 3);
}
#[test]
fn claim_priority_done_attachment_empty_milestone_and_cycles_are_explicit() {
    let plan=panel_contract::plan::parse_document_noting(r#"{"milestones":[{"id":"A","title":"A","depends_on":["B"],"items":[{"mark":"pending","text":"x"}]},{"id":"B","title":"B","depends_on":["A"],"items":[{"mark":"done","text":"x"}]},{"id":"C","title":"C","depends_on":["missing"],"items":[{"text":"x"}]},{"id":"E","title":"empty"}]}"#).unwrap().0;
    let result = Projection::new(&plan, &place(&[], &plan), &column);
    assert_eq!(
        result.classes,
        [Class::Later, Class::Done, Class::Later, Class::Done]
    );
    assert_eq!(result.diagnostics.len(), 3);
    assert_eq!(
        projection::claim([Column::Done, Column::Block, Column::Wait]),
        Some(Column::Wait)
    );
    let entries = vec![entry(1, Some("B"), None, Some("wait"))];
    assert_eq!(
        Projection::new(&plan, &place(&entries, &plan), &column).classes[1],
        Class::Done
    );
}
#[test]
fn selective_items_include_all_attachments_and_only_two_pending_after_last_relevant() {
    let mut plan = fixture();
    let m = &mut plan.milestones[4];
    m.items[6].mark = Some(Mark::Current);
    let mut attached = vec![false; m.items.len()];
    attached[5] = true;
    assert_eq!(
        projection::visible(m, &attached, false, false),
        [5, 6, 7, 8]
    );
    assert_eq!(projection::visible(m, &attached, false, true), [5]);
    assert_eq!(projection::visible(m, &attached, true, false).len(), 10);
    assert_eq!(
        projection::visible(&plan.milestones[6], &[], false, false),
        [0, 1]
    );
}
#[test]
fn strip_actions_show_all_class_changes_and_reopen_keep_reader_owned_choices() {
    let mut reader = Reader::default();
    let key = Key {
        project: "p".into(),
        milestone: "H6".into(),
    };
    reader.reconcile("p", &[("H6".into(), Class::Later)]);
    reader.apply(&Action::ToggleColumn(key.clone()));
    assert!(reader.project("p").opened.contains(&Identity::from("H6")));
    reader.apply(&Action::ShowAll(true));
    reader.apply(&Action::ShowAll(false));
    assert!(reader.project("p").opened.contains(&Identity::from("H6")));
    reader.reconcile("p", &[("H6".into(), Class::Done)]);
    assert!(!reader.project("p").full.contains(&Identity::from("H6")));
    assert!(reader.project("p").opened.contains(&Identity::from("H6")));
    reader.apply(&Action::CloseColumn(key.clone()));
    assert!(!reader.project("p").opened.contains(&Identity::from("H6")));
    reader.reconcile("p", &[("H6".into(), Class::Ready)]);
    reader.apply(&Action::ToggleColumn(key.clone()));
    assert!(reader.project("p").full.contains(&Identity::from("H6")));
    reader.apply(&Action::ToggleColumn(key));
    assert!(!reader.project("p").full.contains(&Identity::from("H6")));
}
#[test]
fn compact_excerpt_is_literal_bounded_and_prioritizes_published_marks() {
    let panel=panel_contract::parse_document(r#"{"lines":[{"text":"Title","style":"title"},"first",{"text":"Do not interpret this","mark":"wait"},"third","fourth"]}"#).unwrap();
    let lines = compact::excerpt(&panel);
    assert_eq!(
        lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
        ["Do not interpret this", "first", "third"]
    );
}
#[test]
fn widths_preserve_collapsed_width_and_use_horizontal_overflow() {
    let widths = projection::widths(500.0, &[(false, true), (true, false), (false, false)], 1);
    assert_eq!(widths, [240.0, 190.0, 240.0]);
    let widths = projection::widths(1440.0, &[(false, true), (true, false), (false, false)], 1);
    assert_eq!(widths[1], 190.0);
    assert!(widths[0] > widths[2]);
    assert!((widths.iter().sum::<f32>() + 64.0 - 1440.0).abs() < 0.1);
}
fn render(
    ctx: &egui::Context,
    section: &Section,
    reader: &mut Reader,
    filter: &crate::board::filter::Filter,
) -> Vec<egui::epaint::ClippedShape> {
    let roles = Roles::of(&crate::theme::Theme::default());
    reader.hits.clear();
    reader.targets.clear();
    let out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1440.0, 900.0))),
            ..Default::default()
        },
        |ui| {
            let plan = section.roadmap.as_ref().unwrap().plan.as_ref().unwrap();
            let canvas = ui.max_rect();
            paint_plan_with(
                ui,
                section,
                plan,
                &roles,
                reader,
                filter,
                &column,
                &mut |ui, e, _, _, width| {
                    assert!(width >= 200.0);
                    let rect = compact::paint(
                        ui,
                        e,
                        column(e),
                        true,
                        None,
                        Some("main"),
                        false,
                        &roles,
                        &mut crate::measure::Board::default(),
                    );
                    assert!(rect.width() <= width + 0.01);
                    let visible = rect.intersect(ui.clip_rect());
                    assert!(!visible.is_positive() || canvas.contains_rect(visible));
                },
            );
            assert!(ui.min_rect().width() <= 1440.0);
        },
    );
    let shapes = out.shapes.clone();
    out.drop_without_applying_deltas();
    shapes
}
#[test]
fn fourteen_chips_stay_on_one_line_filters_do_not_change_strip_and_h6_opens() {
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let mut reader = Reader::default();
    let section = section(
        vec![
            entry(1, Some("H2"), Some("AEAT-SEAL"), None),
            entry(2, Some("H3"), Some("TERRITORIES"), Some("wait")),
        ],
        fixture(),
    );
    render(&ctx, &section, &mut reader, &Default::default());
    let chips: Vec<_> = reader
        .targets
        .iter()
        .filter(|(t, _)| matches!(t, Target::Strip(_)))
        .collect();
    assert_eq!(chips.len(), 14);
    assert!(chips.iter().all(|(_, r)| r.top() == chips[0].1.top()));
    assert!(
        chips
            .windows(2)
            .all(|pair| pair[0].1.right() <= pair[1].1.left())
    );
    assert!(
        chips
            .iter()
            .all(|(_, r)| r.left() >= 0.0 && r.right() <= 1440.0)
    );
    let strip: Vec<_> = chips.iter().map(|(_, r)| *r).collect();
    render(
        &ctx,
        &section,
        &mut reader,
        &crate::board::filter::Filter {
            query: "nothing matches".into(),
            ..Default::default()
        },
    );
    assert_eq!(
        reader
            .targets
            .iter()
            .filter(|(t, _)| matches!(t, Target::Strip(_)))
            .map(|(_, r)| *r)
            .collect::<Vec<_>>(),
        strip
    );
    let k = strip::key(
        &project_key(&section),
        section.roadmap.as_ref().unwrap().plan.as_ref().unwrap(),
        7,
    );
    reader.apply(&Action::ToggleColumn(k.clone()));
    let shapes = render(&ctx, &section, &mut reader, &Default::default());
    assert!(
        reader
            .targets
            .iter()
            .any(|(t, _)| *t == Target::Header(k.clone()))
    );
    assert!(shapes.iter().any(
        |s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains("waiting on H2"))
    ));
}

#[test]
fn keyboard_tab_arrows_and_activation_keep_stable_targets() {
    let mut reader = Reader::default();
    let k = Key {
        project: "p".into(),
        milestone: "H6".into(),
    };
    let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(100.0, 30.0));
    reader.reconcile("p", &[("H6".into(), Class::Later)]);
    reader.targets = vec![
        (Target::Strip(k.clone()), rect),
        (Target::Header(k.clone()), rect),
        (Target::Item(k.clone(), 0), rect),
        (Target::Card(7), rect),
    ];
    assert!(reader.keyboard(egui::Key::Tab, false).0);
    assert_eq!(reader.focus, Some(Target::Strip(k.clone())));
    reader.keyboard(egui::Key::Space, false);
    assert!(reader.project("p").opened.contains(&Identity::from("H6")));
    reader.keyboard(egui::Key::Tab, false);
    reader.keyboard(egui::Key::ArrowDown, false);
    assert_eq!(reader.focus, Some(Target::Item(k, 0)));
    reader.keyboard(egui::Key::ArrowDown, false);
    assert_eq!(reader.keyboard(egui::Key::Enter, false), (true, Some(7)));
    reader.keyboard(egui::Key::Tab, false);
    assert_eq!(reader.keyboard(egui::Key::ArrowRight, false), (false, None));
}

#[test]
fn capture_milestone_reference_when_requested() {
    let Ok(directory) = std::env::var("IVY_BOARD_CAPTURE") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    for named in [
        crate::theme::NamedTheme::IvyNight,
        crate::theme::NamedTheme::IvyDay,
        crate::theme::NamedTheme::Nord,
    ] {
        for opened in [false, true] {
            let ctx = egui::Context::default();
            crate::fonts::install_bundled(&ctx);
            let theme = named.theme();
            let roles = Roles::of(&theme);
            let mut reader = Reader::default();
            let mut active = entry(2, Some("H3"), Some("TERRITORIES"), Some("wait"));
            active.snapshot.document=Some(Arc::new(panel_contract::parse_document(r#"{"milestone":"H3","deliverable":"TERRITORIES","attention":"wait","phase":"design","lines":[{"text":"Special European territories and their tax details","style":"title"},{"text":"Migration or approval for territory tax rates?","mark":"wait","ref":"Decision"},{"text":"Expand the jurisdiction catalog","ref":"Now"}]}"#).unwrap()));
            let section = section(
                vec![entry(1, Some("H2"), Some("AEAT-SEAL"), None), active],
                fixture(),
            );
            if opened {
                let plan = section.roadmap.as_ref().unwrap().plan.as_ref().unwrap();
                reader.reconcile(
                    &project_key(&section),
                    &plan
                        .milestones
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            (
                                strip::key(&project_key(&section), plan, i).milestone,
                                Class::Later,
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                reader.apply(&Action::ToggleColumn(strip::key(
                    &project_key(&section),
                    plan,
                    7,
                )));
            }
            let mut textures = std::collections::HashMap::new();
            let mut meshes = Vec::new();
            for _ in 0..3 {
                reader.hits.clear();
                reader.targets.clear();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            vec2(1440.0, 900.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        crate::board::style::apply(ui, &theme);
                        ui.painter().rect_filled(ui.max_rect(), 0, roles.base);
                        crate::board::bar::paint(
                            ui,
                            1440.0,
                            crate::board::bar::Totals {
                                consoles: 2,
                                projects: 1,
                                claiming: 1,
                            },
                            crate::board::filter::Arrangement::ByMilestone,
                            &Default::default(),
                            Some(false),
                            &roles,
                            &mut Vec::new(),
                        );
                        egui::Frame::new().inner_margin(16).show(ui, |ui| {
                            project_header(
                                ui,
                                &section,
                                Claims {
                                    wait: 1,
                                    ..Default::default()
                                },
                                true,
                                &roles,
                            );
                            let plan = section.roadmap.as_ref().unwrap().plan.as_ref().unwrap();
                            paint_plan_with(
                                ui,
                                &section,
                                plan,
                                &roles,
                                &mut reader,
                                &Default::default(),
                                &column,
                                &mut |ui, e, _, _, _| {
                                    compact::paint(
                                        ui,
                                        e,
                                        column(e),
                                        true,
                                        None,
                                        Some("master"),
                                        e.pane == 2,
                                        &roles,
                                        &mut crate::measure::Board::default(),
                                    );
                                },
                            );
                        });
                    },
                );
                for (id, deltas) in &output.textures_delta.set {
                    for delta in deltas {
                        let egui::ImageData::Color(image) = &delta.image;
                        let key = format!("{id:?}");
                        let buffer = textures.entry(key).or_insert_with(|| {
                            image::RgbaImage::new(image.width() as u32, image.height() as u32)
                        });
                        let [x, y] = delta.pos.unwrap_or([0, 0]);
                        for iy in 0..image.height() {
                            for ix in 0..image.width() {
                                buffer.put_pixel(
                                    (x + ix) as u32,
                                    (y + iy) as u32,
                                    image::Rgba(image.pixels[iy * image.width() + ix].to_array()),
                                );
                            }
                        }
                    }
                }
                meshes=ctx.tessellate(std::mem::take(&mut output.shapes),1.0).into_iter().filter_map(|p| {
                    let egui::epaint::Primitive::Mesh(mesh)=p.primitive else {return None};
                    Some(serde_json::json!({"clip":[p.clip_rect.min.x,p.clip_rect.min.y,p.clip_rect.max.x,p.clip_rect.max.y],"texture":format!("{:?}",mesh.texture_id),"indices":mesh.indices,"vertices":mesh.vertices.iter().map(|v|[v.pos.x,v.pos.y,v.uv.x,v.uv.y,v.color.r() as f32,v.color.g() as f32,v.color.b() as f32,v.color.a() as f32]).collect::<Vec<_>>()}))
                }).collect();
                output.drop_without_applying_deltas();
            }
            let name = format!("{}-{}", named.id(), if opened { "h6" } else { "default" });
            for (id, image) in textures {
                image
                    .save(directory.join(format!("{name}-{id}.png")))
                    .unwrap();
            }
            std::fs::write(
                directory.join(format!("{name}.json")),
                serde_json::to_vec(&meshes).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
fn show_all_paints_the_entire_roadmap_then_restores_the_compact_groups() {
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let mut reader = Reader::default();
    let section = section(
        vec![
            entry(1, Some("H2"), Some("AEAT-SEAL"), None),
            entry(2, Some("H3"), Some("TERRITORIES"), Some("wait")),
        ],
        fixture(),
    );
    render(&ctx, &section, &mut reader, &Default::default());
    let headers = |r: &Reader| {
        r.targets
            .iter()
            .filter_map(|(t, _)| match t {
                Target::Header(k) if k.milestone != Identity::Outside => Some(k.milestone.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let before = headers(&reader);
    assert_eq!(before.len(), 5);
    reader.apply(&Action::ShowAll(true));
    render(&ctx, &section, &mut reader, &Default::default());
    assert_eq!(
        headers(&reader),
        fixture()
            .milestones
            .iter()
            .map(|m| Identity::Declared(m.id.clone().unwrap()))
            .collect::<Vec<_>>()
    );
    reader.apply(&Action::ShowAll(false));
    render(&ctx, &section, &mut reader, &Default::default());
    assert_eq!(headers(&reader), before);
}

#[test]
fn outside_expansion_survives_projection_and_current_outside_items_are_visible() {
    let mut plan = fixture();
    plan.outside = (0..6)
        .map(|i| panel_contract::plan::Item {
            text: format!("Outside item {i}"),
            mark: Some(if i == 4 { Mark::Current } else { Mark::Pending }),
            ..Default::default()
        })
        .collect();
    let section = section(Vec::new(), plan);
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let mut reader = Reader::default();
    render(&ctx, &section, &mut reader, &Default::default());
    let outside = |r: &Reader| {
        r.targets
            .iter()
            .filter_map(|(t, _)| match t {
                Target::Item(k, i) if k.milestone == Identity::Outside => Some(*i),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(outside(&reader), [4, 5]);
    let key = Key {
        project: project_key(&section),
        milestone: Identity::Outside,
    };
    reader.apply(&Action::ToggleItems(key));
    render(&ctx, &section, &mut reader, &Default::default());
    assert_eq!(outside(&reader), [0, 1, 2, 3, 4, 5]);
}

#[test]
fn a_completed_acyclic_prerequisite_of_a_cycle_does_not_block_its_dependents() {
    let plan=panel_contract::plan::parse_document_noting(r#"{"milestones":[{"id":"A","title":"A","depends_on":["B"],"items":[{"text":"a"}]},{"id":"B","title":"B","depends_on":["A"],"items":[{"text":"b"}]},{"id":"C","title":"C","depends_on":["A"],"items":[{"text":"c","mark":"done"}]},{"id":"D","title":"D","depends_on":["C"],"items":[{"text":"d"}]}]}"#).unwrap().0;
    let result = Projection::new(&plan, &place(&[], &plan), &column);
    assert_eq!(
        result.classes,
        [Class::Later, Class::Later, Class::Done, Class::Ready]
    );
    assert!(result.missing[3].is_empty());
}
#[test]
fn producer_ids_cannot_alias_anonymous_or_outside_reader_columns() {
    let plan=panel_contract::plan::parse_document_noting(r##"{"milestones":[{"title":"Same title"},{"id":"#0:Same title","title":"Declared title"},{"id":"@off","title":"Named outside","items":[{"text":"named task","mark":"current"}]}],"outside":["one","two","three","four"]}"##).unwrap().0;
    let section = section(Vec::new(), plan.clone());
    let project = project_key(&section);
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let mut reader = Reader::default();
    render(&ctx, &section, &mut reader, &Default::default());
    let anonymous = strip::key(&project, &plan, 0);
    let declared = strip::key(&project, &plan, 1);
    reader.apply(&Action::ToggleColumn(anonymous.clone()));
    render(&ctx, &section, &mut reader, &Default::default());
    assert!(
        reader
            .targets
            .iter()
            .any(|(t, _)| *t == Target::Header(anonymous.clone()))
    );
    assert!(
        !reader
            .targets
            .iter()
            .any(|(t, _)| *t == Target::Header(declared.clone()))
    );
    reader.apply(&Action::ToggleItems(strip::key(&project, &plan, 2)));
    render(&ctx, &section, &mut reader, &Default::default());
    let outside = |r: &Reader| {
        r.targets
            .iter()
            .filter(|(t, _)| matches!(t,Target::Item(k,_) if k.milestone==Identity::Outside))
            .count()
    };
    assert_eq!(outside(&reader), 2);
    reader.apply(&Action::ToggleItems(Key {
        project,
        milestone: Identity::Outside,
    }));
    render(&ctx, &section, &mut reader, &Default::default());
    assert_eq!(outside(&reader), 4);
}
#[test]
fn roadmap_source_hits_are_limited_to_the_visible_header() {
    let mut section = section(Vec::new(), fixture());
    section.roadmap.as_mut().unwrap().path = Some(std::env::temp_dir().join("roadmap.md"));
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.set_width(700.0);
        let start = ui.next_widget_position();
        let clip = Rect::from_min_size(start, vec2(700.0, 14.0));
        ui.set_clip_rect(clip);
        let hit = project_header(
            ui,
            &section,
            Claims::default(),
            true,
            &Roles::of(&crate::theme::Theme::default()),
        )
        .unwrap();
        assert!(clip.contains_rect(hit));
        assert!(!hit.contains(start + vec2(600.0, 28.0)));
        assert!(
            project_header(
                ui,
                &section,
                Claims::default(),
                true,
                &Roles::of(&crate::theme::Theme::default())
            )
            .is_none()
        );
    })
    .drop_without_applying_deltas();
}

#[test]
fn group_separators_end_before_the_next_project() {
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let mut reader = Reader::default();
    let plan = fixture();
    let section = section(Vec::new(), plan.clone());
    let roles = Roles::of(&crate::theme::Theme::default());
    let mut bottom = 0.0;
    let output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1440.0, 900.0))),
            ..Default::default()
        },
        |ui| {
            paint_plan_with(
                ui,
                &section,
                &plan,
                &roles,
                &mut reader,
                &Default::default(),
                &column,
                &mut |_, _, _, _, _| {},
            );
            bottom = ui.next_widget_position().y;
            ui.label("Next project witness");
        },
    );
    let vertical: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::LineSegment { points, stroke }
                if points[0].x == points[1].x && stroke.color == roles.line =>
            {
                Some(points)
            }
            _ => None,
        })
        .collect();
    assert!(!vertical.is_empty());
    assert!(
        vertical
            .iter()
            .all(|points| points.iter().all(|p| p.y <= bottom)),
        "separator crosses the next project at {bottom}"
    );
    output.drop_without_applying_deltas();
}

#[test]
fn card_filters_keep_item_targets_and_claiming_columns_yield_to_show_all() {
    let section = section(
        vec![
            entry(1, Some("H2"), Some("AEAT-SEAL"), None),
            entry(2, Some("H3"), Some("TERRITORIES"), Some("wait")),
            entry(3, None, None, Some("block")),
            entry(4, None, None, None),
        ],
        fixture(),
    );
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let mut reader = Reader::default();
    render(&ctx, &section, &mut reader, &Default::default());
    let items = |reader: &Reader| {
        reader
            .targets
            .iter()
            .filter_map(|(target, _)| matches!(target, Target::Item(..)).then_some(target.clone()))
            .collect::<Vec<_>>()
    };
    let before = items(&reader);
    assert!(!before.is_empty());
    render(
        &ctx,
        &section,
        &mut reader,
        &crate::board::filter::Filter {
            query: "no console has this label".into(),
            ..Default::default()
        },
    );
    assert_eq!(
        items(&reader),
        before,
        "text narrowing changed roadmap items"
    );
    assert!(
        !reader
            .targets
            .iter()
            .any(|(target, _)| matches!(target, Target::Card(_)))
    );

    let claiming = crate::board::filter::Filter {
        claiming_only: true,
        ..Default::default()
    };
    render(&ctx, &section, &mut reader, &claiming);
    let headers = |reader: &Reader| {
        reader
            .targets
            .iter()
            .filter_map(|(target, _)| match target {
                Target::Header(key) => Some(key.milestone.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(headers(&reader), [Identity::from("H3"), Identity::Outside]);
    let cards = |reader: &Reader| {
        reader
            .targets
            .iter()
            .filter_map(|(target, _)| match target {
                Target::Card(pane) => Some(*pane),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(cards(&reader), [2, 3]);
    assert_eq!(
        reader
            .targets
            .iter()
            .filter(|(target, _)| matches!(target, Target::Strip(_)))
            .count(),
        14
    );
    reader.apply(&Action::ShowAll(true));
    render(&ctx, &section, &mut reader, &claiming);
    assert_eq!(
        headers(&reader).len(),
        15,
        "Show all must restore every milestone plus off-plan"
    );
    assert_eq!(
        cards(&reader),
        [2, 3],
        "Show all must preserve the claiming card filter"
    );
}

#[test]
fn all_done_message_keeps_unmatched_consoles_before_outside_items() {
    let plan = panel_contract::plan::parse_document_noting(
        r#"{"milestones":[{"id":"H0","title":"Complete","items":[{"text":"Done task","mark":"done"}]}],"outside":[{"text":"Company certificate","mark":"pending"}]}"#,
    ).unwrap().0;
    let section = section(vec![entry(8, None, None, None)], plan);
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let mut reader = Reader::default();
    let shapes = render(&ctx, &section, &mut reader, &Default::default());
    assert!(
        shapes
            .iter()
            .any(|shape| matches!(&shape.shape, egui::Shape::Text(text)
        if text.galley.text() == "All milestones done · Show all to review"))
    );
    let card = reader
        .targets
        .iter()
        .position(|(target, _)| *target == Target::Card(8))
        .unwrap();
    let outside = reader
        .targets
        .iter()
        .position(|(target, _)| {
            matches!(target,
        Target::Item(key, 0) if key.milestone == Identity::Outside)
        })
        .unwrap();
    assert!(
        card < outside,
        "outside work preceded the unmatched console"
    );
    assert!(!reader.targets.iter().any(|(target, _)| matches!(target,
        Target::Header(key) if key.milestone == Identity::from("H0"))));
}

#[test]
fn hidden_summary_counts_only_omitted_items_and_full_expansion_has_none() {
    let items = (0..10_000)
        .map(|i| panel_contract::plan::Item {
            text: format!("Task {i}"),
            mark: Some(if i % 2 == 0 {
                Mark::Done
            } else {
                Mark::Pending
            }),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        projection::hidden_counts(&items, &[1, 5000, 9999], false),
        (4998, 4999)
    );
    assert_eq!(projection::hidden_counts(&items, &[], false), (5000, 5000));
    assert_eq!(
        projection::hidden_counts(&items, &(0..items.len()).collect::<Vec<_>>(), true),
        (0, 0)
    );
}

#[test]
fn diagnostics_emit_once_per_current_plan_snapshot_not_once_per_session() {
    let mut reader = Reader::default();
    let mut a = fixture();
    a.milestones[0].depends_on = vec!["unknown-A".into()];
    let mut b = a.clone();
    b.milestones[0].depends_on = vec!["unknown-B".into()];
    let messages = |plan: &Plan| Projection::new(plan, &place(&[], plan), &column).diagnostics;
    let da = messages(&a);
    let db = messages(&b);
    assert_eq!(reader.new_diagnostics("p", &a, &da), da);
    assert!(reader.new_diagnostics("p", &a.clone(), &da).is_empty());
    assert_eq!(
        reader.new_diagnostics("q", &a, &da),
        da,
        "projects deduplicate independently"
    );
    assert_eq!(reader.new_diagnostics("p", &b, &db), db);
    assert!(reader.new_diagnostics("p", &b, &db).is_empty());
    assert_eq!(
        reader.new_diagnostics("p", &a, &da),
        da,
        "returning snapshot must not inherit obsolete warnings"
    );
}

#[test]
fn publication_age_over_a_day_uses_wait_ink_while_fresh_age_is_muted() {
    let ctx = egui::Context::default();
    crate::fonts::install_bundled(&ctx);
    let roles = Roles::of(&crate::theme::Theme::default());
    for (hours, expected) in [(2, roles.tx3), (25, roles.wait)] {
        let mut section = section(Vec::new(), fixture());
        let source = section.roadmap.as_mut().unwrap();
        source.source = format!("published {hours}h ago");
        source.published_at =
            Some(std::time::SystemTime::now() - std::time::Duration::from_secs(hours * 3600));
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_width(1440.0);
            project_header(ui, &section, Claims::default(), true, &roles);
        });
        let actual = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == format!("published {hours}h ago") => {
                Some(text.fallback_color)
            }
            _ => None,
        });
        output.drop_without_applying_deltas();
        assert_eq!(actual, Some(expected));
    }
}
