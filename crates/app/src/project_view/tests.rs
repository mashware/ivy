use super::*;
use crate::tests::window_of_tabs;
use crate::theme::project_ink_contrast as contrast;

#[test]
fn default_theme_produces_distinguishable_contrasting_inks() {
    let theme = Theme::default();
    let colors = crate::theme::project_accents(&theme);
    assert!(
        colors.len() >= 2,
        "default theme needs at least two project colors"
    );
    for (i, ink) in colors.iter().enumerate() {
        assert!(
            contrast(*ink, theme.panel_background()) >= 3.0,
            "insufficient contrast: {ink:?}"
        );
        for other in &colors[..i] {
            assert!(
                separated(*ink, *other),
                "indistinguishable inks: {ink:?}, {other:?}"
            );
        }
    }
}

#[test]
fn monochrome_palette_has_finite_distinct_candidates() {
    let colors = crate::settings::Colors {
        palette: [Some(Color32::GRAY); 16],
        ..Default::default()
    };
    let theme = Theme::default().with_colors(&colors);
    let inks = crate::theme::project_accents(&theme);
    assert!(
        inks.len() <= 1,
        "duplicate source colors must not promise distinct accents"
    );
    assert!(
        inks.iter()
            .all(|ink| contrast(*ink, theme.panel_background()) >= 3.0)
    );
}

#[test]
fn colliding_names_are_disambiguated_by_shared_identity_paths() {
    let identity = |path: &str| Identity {
        key: Key::Repository(path.into()),
        name: "same".into(),
        path: path.into(),
    };
    let first = identity("/one/same");
    let second = identity("/two/same");
    let all = vec![(0, first.clone()), (1, second.clone())];
    assert_ne!(badge_name(&first, &all), badge_name(&second, &all));
    assert_eq!(badge_name(&first, &all), "one/same");
    assert_eq!(badge_name(&first, &[(0, first.clone())]), "same");
}

#[test]
fn remote_directory_veto_clears_existing_badge_and_accounts_work() {
    let root = std::env::temp_dir().join(format!(
        "ivy-project-view-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::create_dir_all(root.join("b")).unwrap();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    configure(
        &mut window,
        crate::settings::Entry::Normal,
        Some(root.clone()),
    );
    let pane = window.active().panes()[0].name;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        refresh(&mut window, &ctx);
        if window.pane_called(pane).unwrap().project.is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "project resolver did not publish a badge"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    window.projects.probe = None;
    window
        .pane_called(pane)
        .unwrap()
        .screen
        .advance(b"\x1b]7;file://remote.invalid/project\x07");
    let before = window.timed_separately;
    refresh(&mut window, &ctx);
    assert!(
        window.pane_called(pane).unwrap().project.is_none(),
        "remote report must veto native cwd and clear the old badge"
    );
    assert_eq!(window.projects.sources.get(&pane), Some(&None));
    assert!(window.timed_separately > before);
    assert!(window.projects.pending > Duration::ZERO);
    window.projects.close_frame();
    let mut report = Vec::new();
    window.projects.publish(&mut report).unwrap();
    let report = String::from_utf8(report).unwrap();
    assert!(report.contains("project accents: enabled=true"));
    assert!(report.contains(
        "project identity per frame (native metadata + bookkeeping, excludes background Git)"
    ));
    assert!(report.contains("frames=1"));
    drop(window);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn no_candidate_is_offered_when_theme_cannot_supply_contrast() {
    let colors = crate::settings::Colors {
        palette: [Some(Color32::GRAY); 16],
        foreground: Some(Color32::GRAY),
        panel: Some(Color32::GRAY),
        ..Default::default()
    };
    let theme = Theme::default().with_colors(&colors);
    assert!(
        crate::theme::project_accents(&theme).is_empty(),
        "an unreadable ink is not a suitable project color"
    );
}

#[test]
fn historical_measurements_do_not_observe_projects() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    configure(&mut window, crate::settings::Entry::Measuring, None);
    refresh(&mut window, &egui::Context::default());
    assert!(!window.projects.enabled);
    assert!(window.projects.sources.is_empty());
    assert_eq!(window.projects.observed, 0);
    assert_eq!(window.projects.pending, Duration::ZERO);
}

#[cfg(target_os = "linux")]
#[test]
fn fresh_directory_report_replaces_an_older_native_sample_without_extra_polling() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    configure(&mut window, crate::settings::Entry::Normal, None);
    let old = PathBuf::from("/old-native-observation");
    window
        .projects
        .native
        .insert(0, (Instant::now(), Some(old.clone())));
    window.projects.reports.insert(0, 0);
    window
        .pane_called(0)
        .unwrap()
        .screen
        .advance(b"\x1b]7;file://localhost/tmp\x07");
    window
        .pane_called(1)
        .unwrap()
        .screen
        .advance(b"\x1b]7;file://remote.invalid/project\x07");
    for _ in 0..2 {
        refresh(&mut window, &ctx);
        assert_eq!(
            window.projects.sources.get(&0),
            Some(&Some(PathBuf::from("/tmp")))
        );
        assert_eq!(
            window.projects.observed, 0,
            "a report must not force an extra native query"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn repeating_a_directory_report_supersedes_an_intervening_native_directory() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    configure(&mut window, crate::settings::Entry::Normal, None);
    let report = b"\x1b]7;file://localhost/tmp\x07";
    window.pane_called(0).unwrap().screen.advance(report);
    let generation = window
        .pane_called(0)
        .unwrap()
        .screen
        .working_directory_generation();
    window.projects.reports.insert(0, generation);
    window.projects.native.insert(
        0,
        (
            Instant::now(),
            Some(PathBuf::from("/intervening-native-directory")),
        ),
    );
    window.pane_called(0).unwrap().screen.advance(report);
    window
        .pane_called(1)
        .unwrap()
        .screen
        .advance(b"\x1b]7;file://remote.invalid/project\x07");
    refresh(&mut window, &ctx);
    assert_eq!(
        window.projects.sources.get(&0),
        Some(&Some(PathBuf::from("/tmp")))
    );
    assert_eq!(window.projects.observed, 0);

    window.projects.native.insert(
        0,
        (
            Instant::now(),
            Some(PathBuf::from("/recent-native-directory")),
        ),
    );
    window.pane_called(0).unwrap().screen.advance(b"\x1bc");
    refresh(&mut window, &ctx);
    assert_eq!(
        window.projects.sources.get(&0),
        Some(&Some(PathBuf::from("/recent-native-directory")))
    );
}

#[cfg(target_os = "linux")]
#[test]
fn project_measurement_counts_current_distinct_panes_instead_of_badge_rebuilds() {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    configure(&mut window, crate::settings::Entry::Normal, None);
    window
        .all_panes_mut()
        .find(|p| p.name == 3)
        .unwrap()
        .screen
        .advance(b"\x1b]7;file://remote.invalid/project\x07");
    let deadline = Instant::now() + Duration::from_secs(5);
    while window.projects.resolved != 3 {
        refresh(&mut window, &ctx);
        assert!(
            Instant::now() < deadline,
            "three local consoles must resolve"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    for foreground in [Color32::WHITE, Color32::LIGHT_GRAY, Color32::GRAY] {
        window.theme = Theme::default().with_colors(&crate::settings::Colors {
            foreground: Some(foreground),
            ..Default::default()
        });
        refresh(&mut window, &ctx);
        assert_eq!(
            window.projects.resolved, 3,
            "rebuilding three badges must not count extra consoles"
        );
        assert_eq!(
            window
                .projects
                .painted
                .values()
                .filter(|p| p.load(Ordering::Relaxed))
                .count(),
            0
        );
        for _ in 0..2 {
            for badge in window.all_panes().filter_map(|p| p.project.as_ref()) {
                badge.painted.store(true, Ordering::Relaxed);
            }
        }
        let mut output = Vec::new();
        window.projects.publish(&mut output).unwrap();
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("resolved=3, painted=3")
        );
    }
}

#[test]
fn real_frame_loop_resolves_and_paints_project_badges() {
    let root = std::env::temp_dir().join(format!("ivy-project-frame-{}", std::process::id()));
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::create_dir_all(root.join("b")).unwrap();
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    configure(
        &mut window,
        crate::settings::Entry::Normal,
        Some(root.clone()),
    );
    window.chrome.enable();
    window.headers = crate::header::Headers::PAINTED.integrated(true);
    let mut frame = eframe::Frame::_new_kittest();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        eframe::App::logic(&mut window, &ctx, &mut frame);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| eframe::App::ui(&mut window, ui, &mut frame),
        );
        output.drop_without_applying_deltas();
        if window.projects.resolved == 2
            && window
                .projects
                .painted
                .values()
                .all(|marker| marker.load(Ordering::Relaxed))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the actual frame loop did not resolve and paint both projects"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(window.projects.painted.len(), 2);
    assert!(window.projects.pending > Duration::ZERO);
    drop(window);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn projects_keep_identity_and_accent_across_tabs_focus_and_theme_changes() {
    let root = std::env::temp_dir().join(format!(
        "ivy-project-integration-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    for name in ["a", "b"] {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "-q"])
                .arg(&dir)
                .status()
                .unwrap()
                .success()
        );
    }
    let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
    let ctx = egui::Context::default();
    configure(
        &mut window,
        crate::settings::Entry::Normal,
        Some(root.clone()),
    );
    window.all_panes_mut().find(|p| p.name == 0).unwrap().label = Some("Manual console".into());
    window
        .pane_called(1)
        .unwrap()
        .screen
        .advance(b"\x1b]2;Program title\x07");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        refresh(&mut window, &ctx);
        if window.all_panes().all(|pane| pane.project.is_some()) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "project resolver did not publish all badges"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let badges = |window: &mut Window| -> Vec<(String, Color32)> {
        (0..4)
            .map(|id| {
                let badge = window
                    .all_panes_mut()
                    .find(|p| p.name == id)
                    .unwrap()
                    .project
                    .as_ref()
                    .unwrap();
                (badge.name.clone(), badge.ink)
            })
            .collect()
    };
    let initial = badges(&mut window);
    assert_eq!(initial[0], initial[2]);
    assert_eq!(initial[1], initial[3]);
    assert_ne!(initial[0].0, initial[1].0);
    assert_ne!(initial[0].1, initial[1].1);
    assert_eq!(
        crate::pane_view::display_label(window.all_panes_mut().find(|p| p.name == 0).unwrap()),
        "Manual console"
    );
    assert_eq!(
        crate::pane_view::display_label(window.all_panes_mut().find(|p| p.name == 1).unwrap()),
        "Program title"
    );

    window.active = 1;
    window.active_mut().focus = 3;
    refresh(&mut window, &ctx);
    assert_eq!(
        badges(&mut window),
        initial,
        "focus must not change project accents"
    );

    let mut palette = [Some(Color32::from_rgb(255, 32, 32)); 16];
    palette[2] = Some(Color32::from_rgb(32, 255, 32));
    let overrides = crate::settings::Colors {
        palette,
        foreground: Some(Color32::WHITE),
        panel: Some(Color32::BLACK),
        ..Default::default()
    };
    window.theme = Theme::default().with_colors(&overrides);
    refresh(&mut window, &ctx);
    let changed = badges(&mut window);
    for id in 0..4 {
        assert_eq!(changed[id].0, initial[id].0);
    }
    assert_eq!(changed[0], changed[2]);
    assert_eq!(changed[1], changed[3]);
    assert_ne!(changed[0].1, changed[1].1);
    assert_ne!((changed[0].1, changed[1].1), (initial[0].1, initial[1].1));
    assert_eq!(
        crate::pane_view::display_label(window.all_panes_mut().find(|p| p.name == 0).unwrap()),
        "Manual console"
    );
    assert_eq!(
        crate::pane_view::display_label(window.all_panes_mut().find(|p| p.name == 1).unwrap()),
        "Program title"
    );

    let mut idle_delay = Duration::ZERO;
    for _ in 0..4 {
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            refresh(&mut window, ui.ctx())
        });
        idle_delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        output.drop_without_applying_deltas();
    }
    assert_eq!(
        idle_delay,
        Duration::MAX,
        "resolved project metadata must not arm an idle repaint"
    );

    window.tabs.pop();
    window.active = 0;
    refresh(&mut window, &ctx);
    let survivor_a = window
        .all_panes_mut()
        .find(|p| p.name == 0)
        .unwrap()
        .project
        .as_ref()
        .unwrap()
        .ink;
    let survivor_b = window
        .all_panes_mut()
        .find(|p| p.name == 1)
        .unwrap()
        .project
        .as_ref()
        .unwrap()
        .ink;
    assert_eq!((survivor_a, survivor_b), (changed[0].1, changed[1].1));
    drop(window);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn exhausted_palette_keeps_distinct_project_names_in_actual_headers() {
    let root = std::env::temp_dir().join(format!("ivy-project-exhaustion-{}", std::process::id()));
    for name in ["a", "b"] {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "-q"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
    }
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    window.theme = Theme::default().with_colors(&crate::settings::Colors {
        palette: [Some(Color32::WHITE); 16],
        foreground: Some(Color32::WHITE),
        panel: Some(Color32::BLACK),
        ..Default::default()
    });
    assert_eq!(
        crate::theme::project_accents(&window.theme),
        vec![Color32::WHITE]
    );
    configure(
        &mut window,
        crate::settings::Entry::Normal,
        Some(root.clone()),
    );
    window.chrome.enable();
    window.headers = crate::header::Headers::PAINTED.integrated(true);
    let ctx = egui::Context::default();
    let mut frame = eframe::Frame::_new_kittest();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        eframe::App::logic(&mut window, &ctx, &mut frame);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| eframe::App::ui(&mut window, ui, &mut frame),
        );
        let ready = window.projects.resolved == 2;
        if ready {
            let badges: Vec<_> = window
                .all_panes()
                .map(|p| p.project.as_ref().unwrap())
                .collect();
            assert_eq!(badges[0].ink, Color32::WHITE);
            assert_eq!(badges[1].ink, Color32::WHITE);
            assert_ne!(badges[0].name, badges[1].name);
            for badge in badges {
                assert!(
                    output.shapes.iter().any(|shape| matches!(
                        &shape.shape,
                        egui::Shape::Text(text) if text.galley.job.text == badge.name
                    )),
                    "project name must remain painted after color exhaustion"
                );
            }
            assert_eq!(output.shapes.iter().filter(|shape| matches!(
                &shape.shape,
                egui::Shape::Rect(rect) if rect.rect.width() == 3.0 && rect.fill == Color32::WHITE
            )).count(), 2, "both actual header strips must use the resulting ink");
        }
        output.drop_without_applying_deltas();
        if ready {
            break;
        }
        assert!(Instant::now() < deadline, "both repositories must resolve");
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(window);
    std::fs::remove_dir_all(root).unwrap();
}
