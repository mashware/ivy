#[cfg(test)]
mod tests {
    use crate::{Window, egui, settings, tests};
    use term_core::{Cell, Half};

    fn frame(window: &mut Window, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<String> {
        let output = painted_frame(window, ctx, events);
        let copied = output
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
        copied
    }

    fn painted_frame(
        window: &mut Window,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut backend = eframe::Frame::_new_kittest();
        eframe::App::logic(window, ctx, &mut backend);
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(tests::area_of(1200.0)),
                events,
                ..Default::default()
            },
            |ui| eframe::App::ui(window, ui, &mut backend),
        )
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    // Inspect submitted text, rather than trusting runtime settings or cumulative counters.
    fn text_galleys(output: &egui::FullOutput) -> Vec<std::sync::Arc<egui::Galley>> {
        fn collect(shape: &egui::Shape, result: &mut Vec<std::sync::Arc<egui::Galley>>) {
            match shape {
                egui::Shape::Text(text) => result.push(std::sync::Arc::clone(&text.galley)),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, result);
                    }
                }
                _ => {}
            }
        }
        let mut result = Vec::new();
        for clipped in &output.shapes {
            collect(&clipped.shape, &mut result);
        }
        result
    }

    fn assert_current_run(
        window: &mut Window,
        ctx: &egui::Context,
        text: &str,
        size: f32,
    ) -> (f32, Vec<egui::Pos2>) {
        let output = painted_frame(window, ctx, vec![]);
        let galleys = text_galleys(&output);
        output.drop_without_applying_deltas();
        let runs: Vec<_> = galleys
            .iter()
            .filter(|galley| galley.text() == text)
            .collect();
        assert_eq!(
            runs.len(),
            1,
            "exactly one current terminal run must be painted at size {size} scale {}: {:?}",
            ctx.pixels_per_point(),
            galleys.iter().map(|g| g.text()).collect::<Vec<_>>()
        );
        let run = runs[0];
        assert_eq!(run.job.sections[0].format.font_id.size, size);
        let cell = crate::painter::CellMetrics::measure(ctx, egui::FontId::monospace(size));
        assert!(
            (run.rows[0].size.x - text.len() as f32 * cell.width()).abs()
                < 1.0 / ctx.pixels_per_point(),
            "painted run width {} must follow current cell step {} at size {size} and scale {}",
            run.rows[0].size.x,
            cell.width(),
            ctx.pixels_per_point()
        );
        let vertices: Vec<_> = run.rows[0]
            .visuals
            .mesh
            .vertices
            .iter()
            .map(|v| v.pos)
            .collect();
        assert!(
            !vertices.is_empty(),
            "current drawing must carry actual ink geometry"
        );
        (run.rows[0].size.x, vertices)
    }

    #[test]
    fn same_window_replaces_ligature_drawing_after_size_scale_face_and_reset() {
        let ctx = egui::Context::default();
        let mut window = tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let fixture = || {
            egui::FontData::from_static(include_bytes!(
                "../tests/fixtures/ligatures/JetBrainsMono-Regular.ttf"
            ))
        };
        crate::fonts::use_measurement_face(&mut window.discoverer, fixture(), &ctx);
        let apply = |window: &mut Window, size| {
            settings::Settings {
                ligatures: Some(true),
                font_size: Some(size),
                ..Default::default()
            }
            .apply_to(window)
        };
        apply(&mut window, 14.7);
        frame(&mut window, &ctx, vec![]);
        frame(&mut window, &ctx, vec![]);
        let text = "a!=b=>c==d===e";
        window.active_mut().panes_mut()[0]
            .screen
            .advance(format!("\x1b[2J\x1b[H{text}\x1b[?25l").as_bytes());
        let original = assert_current_run(&mut window, &ctx, text, 14.7);
        apply(&mut window, 17.3);
        let resized = assert_current_run(&mut window, &ctx, text, 17.3);
        assert_ne!(original, resized, "size change retained stale drawing");
        ctx.set_pixels_per_point(1.5);
        frame(&mut window, &ctx, vec![]);
        let scaled = assert_current_run(&mut window, &ctx, text, 17.3);
        assert_eq!(ctx.pixels_per_point(), 1.5);
        assert_ne!(
            resized, scaled,
            "device-scale change retained stale drawing"
        );

        // A visibly different registration under the same family name must replace old ink.
        let mut replacement = fixture();
        replacement.tweak.y_offset = 2.0;
        crate::fonts::use_measurement_face(&mut window.discoverer, replacement, &ctx);
        assert!(crate::fonts::ligature_family(&window.discoverer, &ctx).is_none());
        let registered = assert_current_run(&mut window, &ctx, text, 17.3);
        assert_eq!(
            scaled.0, registered.0,
            "visual face tweak must retain cell width"
        );
        assert_ne!(
            scaled.1, registered.1,
            "new selected face retained stale ink geometry"
        );
        assert!(crate::fonts::ligature_family(&window.discoverer, &ctx).is_some());

        let reset = crate::fonts::PreparedSelection::prepare(&mut window.discoverer, None).unwrap();
        reset.queue(&window.discoverer, &ctx);
        frame(&mut window, &ctx, vec![]);
        reset.activate(&mut window.discoverer);
        let before = window.discoverer.ligature_cost();
        let output = painted_frame(&mut window, &ctx, vec![]);
        let galleys = text_galleys(&output);
        output.drop_without_applying_deltas();
        assert!(
            galleys.iter().all(|galley| galley.text() != text),
            "reset reused the selected face's whole-run mesh"
        );
        assert!(
            galleys
                .iter()
                .all(|galley| galley.job.sections.iter().all(|section| section
                    .format
                    .font_id
                    .family
                    != egui::FontFamily::Name("terminal-ligature-face".into()))),
            "reset painted a stale selected family"
        );
        for c in text.chars() {
            assert!(
                galleys.iter().any(|galley| galley.text() == c.to_string()),
                "reset must paint the original source cells"
            );
        }
        let after = window.discoverer.ligature_cost();
        assert_eq!(after.shaped_runs, before.shaped_runs);
        assert!(after.fallback_runs > before.fallback_runs);
        assert!(!window.discoverer.has_searched_the_machine());
    }

    #[test]
    fn actual_window_shapes_but_copy_search_and_geometry_keep_the_source_cells() {
        let ctx = egui::Context::default();
        let mut window = tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        crate::fonts::use_measurement_face(
            &mut window.discoverer,
            egui::FontData::from_static(include_bytes!(
                "../tests/fixtures/ligatures/JetBrainsMono-Regular.ttf"
            )),
            &ctx,
        );
        settings::Settings {
            ligatures: Some(true),
            font_size: Some(14.7),
            ..Default::default()
        }
        .apply_to(&mut window);
        frame(&mut window, &ctx, vec![]);
        frame(&mut window, &ctx, vec![]);
        let text = "a!=b=>c==d===e";
        window.active_mut().panes_mut()[0]
            .screen
            .advance(format!("\x1b[2J\x1b[H{text}\x1b[?25l").as_bytes());
        frame(&mut window, &ctx, vec![]);
        let on = window.discoverer.ligature_cost();
        assert!(
            on.shaped_runs > 0,
            "real window did not reach enabled painting"
        );
        let grid = window.active().panes()[0].screen.view().size();
        let pane = &mut window.active_mut().panes_mut()[0];
        pane.screen.start_selection(Cell {
            row: 0,
            col: 0,
            half: Half::Left,
        });
        pane.screen.extend_selection(Cell {
            row: 0,
            col: text.len() as u16 - 1,
            half: Half::Right,
        });
        let modifiers = egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        assert_eq!(
            frame(
                &mut window,
                &ctx,
                vec![egui::Event::ModifiersChanged(modifiers), egui::Event::Copy]
            ),
            [text]
        );
        frame(&mut window, &ctx, vec![key(egui::Key::F, modifiers)]);
        frame(
            &mut window,
            &ctx,
            vec![
                egui::Event::Text("!=".into()),
                key(egui::Key::Enter, egui::Modifiers::NONE),
            ],
        );
        assert_eq!(
            window.active().panes()[0].screen.selected_text().as_deref(),
            Some("!=")
        );
        frame(
            &mut window,
            &ctx,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        settings::Settings {
            font_size: Some(14.7),
            ..Default::default()
        }
        .apply_to(&mut window);
        let before = window.discoverer.ligature_cost();
        frame(&mut window, &ctx, vec![]);
        assert_eq!(
            window.discoverer.ligature_cost(),
            before,
            "off performed ligature work"
        );
        assert_eq!(window.active().panes()[0].screen.view().size(), grid);
        assert!(!window.discoverer.has_searched_the_machine());
    }
}
