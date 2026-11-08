//! Saved documents cross into rendering only at a coherent begin-pass.
use super::Editor;
use crate::{Window, fonts::PreparedSelection, settings::Settings};
use eframe::egui;
use std::time::Instant;

#[derive(Default)]
pub(super) struct Application {
    pub family: Option<String>,
    pending: Option<Pending>,
    pub saved_frame: Option<(u64, bool)>,
    prepared: usize,
    failed: usize,
    queued: usize,
    committed: usize,
    font_activations: usize,
}

struct Pending {
    settings: Settings,
    font: Option<PreparedSelection>,
    queued_pass: Option<u64>,
}

fn same_family(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

pub(super) fn save(editor: &mut Editor, fonts: &mut crate::fonts::Discoverer) -> bool {
    let started = Instant::now();
    let children_before = editor.timing.work;
    let result = prepare_and_save(editor, fonts);
    let children = editor.timing.work.saturating_sub(children_before);
    editor
        .timing
        .record_work(2, started.elapsed().saturating_sub(children));
    match result {
        Ok(()) => true,
        Err(error) => {
            editor.message = error;
            false
        }
    }
}

fn prepare_and_save(
    editor: &mut Editor,
    fonts: &mut crate::fonts::Discoverer,
) -> Result<(), String> {
    if editor.measuring {
        return Err("Measurement: configuration I/O is disabled.".into());
    }
    let draft = editor.draft.as_ref().ok_or("Settings is not open.")?;
    if !draft.errors().is_empty() {
        return Err("Correct the invalid settings before saving.".into());
    }
    if editor.store.is_none() {
        return Err("This configuration cannot be saved through Settings.".into());
    }
    let text = draft.text();
    let (settings, _) = super::super::interpret(&text).map_err(|e| e.to_string())?;
    let font = if same_family(
        editor.application.family.as_deref(),
        settings.font.as_deref(),
    ) {
        None
    } else {
        let started = Instant::now();
        editor.application.prepared += 1;
        let prepared = PreparedSelection::prepare(fonts, settings.font.as_deref());
        editor.timing.record_work(5, started.elapsed());
        match prepared {
            Ok(prepared) => Some(prepared),
            Err(error) => {
                editor.application.failed += 1;
                return Err(match error {
                    crate::fonts::SelectionError::NotInstalled => {
                        "The selected font is not installed.".into()
                    }
                    crate::fonts::SelectionError::Unusable => {
                        "The selected font's files cannot be read.".into()
                    }
                    crate::fonts::SelectionError::NoFontsOnTheMachine => {
                        "No system fonts are available.".into()
                    }
                });
            }
        }
    };
    editor
        .store
        .as_mut()
        .expect("checked the store")
        .save(&text)?;
    editor.application.pending = Some(Pending {
        settings,
        font,
        queued_pass: None,
    });
    editor.message = "Saved. Applying settings...".into();
    Ok(())
}

/// Called before CellMetrics; a callback in the queuing pass is not a new begin-pass.
pub(crate) fn begin_pass(window: &mut Window, ctx: &egui::Context) {
    if !window.settings_editor.enabled {
        return;
    }
    if window.settings_editor.probe.is_some() {
        let probe_started = Instant::now();
        let children_before = window.settings_editor.timing.work;
        run_probe(window, ctx);
        let children = window
            .settings_editor
            .timing
            .work
            .saturating_sub(children_before);
        window
            .settings_editor
            .timing
            .record_work(8, probe_started.elapsed().saturating_sub(children));
    }
    let started = Instant::now();
    let ready = window
        .settings_editor
        .application
        .pending
        .as_ref()
        .is_some_and(|p| {
            p.queued_pass
                .is_some_and(|queued| queued < ctx.cumulative_pass_nr())
        });
    window
        .settings_editor
        .timing
        .record_work(4, started.elapsed());
    if ready {
        let started = Instant::now();
        let pending = window
            .settings_editor
            .application
            .pending
            .take()
            .expect("ready outcome");
        if let Some(font) = pending.font {
            font.activate(&mut window.discoverer);
            window.settings_editor.application.font_activations += 1;
        }
        pending.settings.apply_to(window);
        window.settings_editor.application.family = pending.settings.font;
        window.settings_editor.application.committed += 1;
        window.settings_editor.message = if window.settings_editor.measuring {
            "Transient settings applied; configuration I/O remains disabled."
        } else {
            "Saved and applied."
        }
        .into();
        // The next logic round must announce the grid this UI pass is about to measure, even idle.
        ctx.request_repaint();
        window
            .settings_editor
            .timing
            .record_work(6, started.elapsed());
    }
    window.timed_separately += window.settings_editor.take_cost();
}

/// Finish ordinary fallback discovery before the final selected-family definitions are queued.
pub(crate) fn resolve_fonts(window: &mut Window, ctx: &egui::Context) {
    let started = Instant::now();
    let round = window.discoverer.resolve(ctx);
    if round.did_work() {
        let cost = started.elapsed();
        window.timed_separately += cost;
        window
            .discovery
            .record(cost, round.searched, round.registered);
    }
    let Some(pending) = window.settings_editor.application.pending.as_mut() else {
        return;
    };
    if pending.queued_pass.is_some() {
        return;
    }
    let started = Instant::now();
    if let Some(font) = &pending.font {
        font.queue(&window.discoverer, ctx);
        window.settings_editor.application.queued += 1;
    }
    pending.queued_pass = Some(ctx.cumulative_pass_nr());
    ctx.request_repaint();
    window
        .settings_editor
        .timing
        .record_work(5, started.elapsed());
    window.timed_separately += window.settings_editor.take_cost();
}

pub(super) fn publish(
    application: &Application,
    to: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    writeln!(
        to,
        "settings runtime: prepared={}, failed={}, font requests={}, font activations={}, commits={}",
        application.prepared,
        application.failed,
        application.queued,
        application.font_activations,
        application.committed
    )
}
fn prepare_probe(
    window: &mut Window,
    probe: &mut super::probe::Probe,
    category: usize,
    name: Option<&str>,
) -> Result<PreparedSelection, crate::fonts::SelectionError> {
    let started = Instant::now();
    let result = PreparedSelection::prepare(&mut window.discoverer, name);
    let elapsed = started.elapsed();
    probe.preparation[category] = Some(elapsed);
    window.settings_editor.timing.record_work(5, elapsed);
    result
}

/// A mixed whole-frame probe; font-engine work is not given an invented isolated duration.
fn run_probe(window: &mut Window, ctx: &egui::Context) {
    let Some(mut probe) = window.settings_editor.probe.take() else {
        return;
    };
    if ctx.cumulative_frame_nr() < 3 || window.settings_editor.application.pending.is_some() {
        window.settings_editor.probe = Some(probe);
        return;
    }
    let panes: Vec<_> = window
        .all_panes()
        .map(|p| (p.name, p.screen.size(), p.measured))
        .collect();
    if panes.len() != 4 || panes.iter().any(|(_, actual, measured)| actual != measured) {
        window.settings_editor.probe = Some(probe);
        return;
    }
    let background = egui::Color32::from_rgb(7, 31, 53);
    let mut settings = Settings {
        blink: Some(false),
        ..Settings::default()
    };
    settings.colors.background = Some(background);
    let mut font = None;
    match probe.stage {
        0 => {
            probe.baseline = Some(panes.iter().map(|(id, size, _)| (*id, *size)).collect());
            if probe.family.is_none() {
                probe.family = std::env::var("MEASURE_SETTINGS_FAMILY")
                    .ok()
                    .filter(|s| !s.is_empty());
            }
        }
        1 => {
            probe.color = window.theme.background() == background;
            if !probe.color {
                window.settings_editor.probe = Some(probe);
                return;
            }
            settings.font_size = Some(22.0);
        }
        2 => {
            probe.observed = panes.len();
            probe.changed = panes
                .iter()
                .filter(|(id, size, _)| {
                    probe
                        .baseline
                        .as_ref()
                        .and_then(|b| b.get(id))
                        .is_some_and(|old| old != size)
                })
                .count();
            if probe.changed != 4 {
                window.settings_editor.probe = Some(probe);
                return;
            }
            settings.font_size = Some(22.0);
            // Separate negative preparation from the cold requested-family preparation.
            window.settings_editor.application.prepared += 1;
            let bogus = "agent-terminal-probe-no-such-family-67184";
            probe.failure = prepare_probe(window, &mut probe, 0, Some(bogus)).is_err();
            if probe.failure {
                window.settings_editor.application.failed += 1;
            }
            let Some(family) = probe.family.clone() else {
                probe.stage = 255;
                window.settings_editor.probe = Some(probe);
                return;
            };
            window.settings_editor.application.prepared += 1;
            match prepare_probe(window, &mut probe, 1, Some(&family)) {
                Ok(value) => {
                    probe.family = value.chosen_name();
                    font = Some(value);
                    settings.font = probe.family.clone();
                }
                Err(_) => {
                    window.settings_editor.application.failed += 1;
                    probe.stage = 255;
                    window.settings_editor.probe = Some(probe);
                    return;
                }
            }
        }
        3 | 6 => {
            let Some(family) = probe.family.clone() else {
                window.settings_editor.probe = Some(probe);
                return;
            };
            let active = ctx.fonts_mut(|fonts| {
                let first = fonts
                    .definitions()
                    .families
                    .get(&egui::FontFamily::Monospace)
                    .and_then(|f| f.first())
                    .cloned();
                let galley = fonts.layout_no_wrap(
                    "Mm".into(),
                    egui::FontId::monospace(22.0),
                    egui::Color32::WHITE,
                );
                first.is_some_and(|f| f.eq_ignore_ascii_case(&format!("chosen:{family}")))
                    && galley.size().x > 0.0
            });
            if !active {
                window.settings_editor.probe = Some(probe);
                return;
            }
            if probe.stage == 3 {
                probe.font_active = true;
            } else {
                probe.warm = true;
                window.settings_editor.probe = Some(probe);
                return;
            }
            settings.font_size = Some(22.0);
            settings.font = Some(family);
        }
        4 => {
            // No queue or preparation was needed for the unchanged-family commit.
            probe.unchanged = window.settings_editor.application.queued == 1;
            if !probe.unchanged {
                window.settings_editor.probe = Some(probe);
                return;
            }
            settings.font_size = Some(22.0);
            window.settings_editor.application.prepared += 1;
            font = prepare_probe(window, &mut probe, 2, None).ok();
        }
        5 => {
            probe.reset = ctx.fonts_mut(|fonts| {
                let first = fonts
                    .definitions()
                    .families
                    .get(&egui::FontFamily::Monospace)
                    .and_then(|f| f.first())
                    .cloned();
                let galley = fonts.layout_no_wrap(
                    "Mm".into(),
                    egui::FontId::monospace(22.0),
                    egui::Color32::WHITE,
                );
                first.is_some_and(|f| !f.starts_with("chosen:")) && galley.size().x > 0.0
            });
            if !probe.reset {
                window.settings_editor.probe = Some(probe);
                return;
            }
            settings.font_size = Some(22.0);
            let family = probe.family.clone().expect("font was previously selected");
            window.settings_editor.application.prepared += 1;
            match prepare_probe(window, &mut probe, 3, Some(&family)) {
                Ok(value) => {
                    probe.family = value.chosen_name();
                    font = Some(value);
                    settings.font = probe.family.clone();
                }
                Err(_) => {
                    window.settings_editor.application.failed += 1;
                    probe.stage = 255;
                    window.settings_editor.probe = Some(probe);
                    return;
                }
            }
        }
        _ => {
            window.settings_editor.probe = Some(probe);
            return;
        }
    }
    probe.stage += 1;
    window.settings_editor.application.pending = Some(Pending {
        settings,
        font,
        queued_pass: None,
    });
    window.settings_editor.probe = Some(probe);
    ctx.request_repaint();
}

#[cfg(test)]
pub(crate) use tests::reference_apply_report;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{area_of, one_frame_in, window_of_tabs};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct File(PathBuf);
    impl File {
        fn new(text: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "runtime-settings-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::write(&path, text).unwrap();
            Self(path)
        }
        fn read(&self) -> String {
            fs::read_to_string(&self.0).unwrap()
        }
    }
    impl Drop for File {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    fn window(file: &File) -> Window {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        window.settings_editor = Editor::resolved(crate::settings::Entry::Normal, None);
        window.settings_editor.open(Some(file.0.clone()));
        window.discoverer = crate::fonts::fixture_discoverer();
        window
    }
    fn set(window: &mut Window, key: &str, value: &str) {
        window
            .settings_editor
            .draft
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|field| field.key == key)
            .unwrap()
            .value = value.into();
    }
    fn frame(window: &mut Window, ctx: &egui::Context) {
        one_frame_in(window, ctx, area_of(1200.0), Vec::new());
    }
    fn save(window: &mut Window) {
        window.settings_editor.save(&mut window.discoverer);
    }
    fn metrics(ctx: &egui::Context) -> egui::Vec2 {
        ctx.fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(
                    "M".into(),
                    egui::FontId::monospace(14.0),
                    egui::Color32::WHITE,
                )
                .size()
        })
    }
    #[test]
    fn ligatures_save_cancel_and_reset_preserve_programs_grid_and_lazy_fonts() {
        let file = File::new("# retain\nsize=14\n");
        let ctx = egui::Context::default();
        let mut window = window(&file);
        window.discoverer = crate::fonts::Discoverer::default();
        window.tabs[0]
            .panes_mut()
            .into_iter()
            .next()
            .unwrap()
            .screen
            .advance(b"a!=b=>c");
        frame(&mut window, &ctx);
        frame(&mut window, &ctx);
        let before: Vec<_> = window
            .all_panes()
            .map(|p| (p.name, p.screen.size()))
            .collect();
        assert!(!window.ligatures);
        assert_eq!(window.discoverer.ligature_cost().eligible_runs, 0);
        set(&mut window, "font.ligatures", "on");
        frame(&mut window, &ctx);
        assert!(!window.ligatures, "a draft must not affect live drawing");
        assert_eq!(file.read(), "# retain\nsize=14\n");
        save(&mut window);
        assert!(!window.ligatures, "Save waits for the coherent begin-pass");
        frame(&mut window, &ctx);
        frame(&mut window, &ctx);
        assert!(window.ligatures, "Save must enable live ligature drawing");
        assert!(
            window.discoverer.ligature_cost().eligible_runs > 0,
            "the saved preference must reach actual console painting"
        );
        assert!(
            window.discoverer.ligature_cost().fallback_runs > 0,
            "no chosen font must retain normal drawing without discovery"
        );
        assert!(file.read().contains("font.ligatures = on"));
        assert!(!window.discoverer.has_searched_the_machine());
        assert_eq!(window.settings_editor.application.prepared, 0);
        assert_eq!(window.settings_editor.application.font_activations, 0);
        set(&mut window, "font.ligatures", "off");
        one_frame_in(
            &mut window,
            &ctx,
            area_of(1200.0),
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        frame(&mut window, &ctx);
        assert!(!window.settings_editor.is_open());
        assert!(window.ligatures, "Cancel must retain saved live drawing");
        assert!(file.read().contains("font.ligatures = on"));
        window.settings_editor.open(Some(file.0.clone()));
        window.settings_editor.draft.as_mut().unwrap().defaults();
        assert!(window.ligatures, "reset is staged until Save");
        save(&mut window);
        frame(&mut window, &ctx);
        frame(&mut window, &ctx);
        assert!(!window.ligatures, "reset must disable live drawing");
        assert_eq!(file.read(), "# retain\n");
        let after_reset = window.discoverer.ligature_cost().eligible_runs;
        frame(&mut window, &ctx);
        assert_eq!(
            window.discoverer.ligature_cost().eligible_runs,
            after_reset,
            "off must stop ligature processing in the painter"
        );
        assert!(!window.discoverer.has_searched_the_machine());
        assert_eq!(
            before,
            window
                .all_panes()
                .map(|p| (p.name, p.screen.size()))
                .collect::<Vec<_>>()
        );
        assert!(window.all_panes().all(|p| p.pty.ending().is_none()));
    }

    #[test]
    fn repeated_save_in_the_same_frame_preserves_success_and_next_frame_checks_conflicts() {
        let file = File::new("size=14\n");
        let ctx = egui::Context::default();
        let mut window = window(&file);
        frame(&mut window, &ctx);
        set(&mut window, "size", "24");
        window
            .settings_editor
            .save_in_frame(&mut window.discoverer, &ctx);
        assert_eq!(
            crate::settings::interpret(&file.read())
                .unwrap()
                .0
                .font_size,
            Some(24.0)
        );
        fs::write(&file.0, "size=19\n").unwrap();
        window
            .settings_editor
            .save_in_frame(&mut window.discoverer, &ctx);
        assert_eq!(
            window.settings_editor.message,
            "Saved. Applying settings..."
        );
        assert_eq!(file.read(), "size=19\n");
        frame(&mut window, &ctx);
        window
            .settings_editor
            .save_in_frame(&mut window.discoverer, &ctx);
        assert!(window.settings_editor.message.contains("changed outside"));
        assert_eq!(file.read(), "size=19\n");
        frame(&mut window, &ctx);
        assert_eq!(window.font_size, 24.0);
        assert_eq!(window.settings_editor.application.committed, 1);
    }
    #[test]
    fn failed_save_remains_failed_during_the_same_frame() {
        let file = File::new("size=14\n");
        let ctx = egui::Context::default();
        let mut window = window(&file);
        frame(&mut window, &ctx);
        set(&mut window, "size", "invalid");
        assert!(
            !window
                .settings_editor
                .save_in_frame(&mut window.discoverer, &ctx)
        );
        set(&mut window, "size", "24");
        assert!(
            !window
                .settings_editor
                .save_in_frame(&mut window.discoverer, &ctx)
        );
        assert_eq!(file.read(), "size=14\n");
        assert!(window.settings_editor.draft.is_some());
    }
    #[test]
    fn closing_after_saved_font_discards_later_edits_but_keeps_pending_activation() {
        let file = File::new("size=14\n");
        let ctx = egui::Context::default();
        let mut window = window(&file);
        frame(&mut window, &ctx);
        let builtin = metrics(&ctx);
        set(&mut window, "font", "Ubuntu");
        set(&mut window, "size", "24");
        save(&mut window);
        let saved = file.read();
        set(&mut window, "size", "32");
        one_frame_in(
            &mut window,
            &ctx,
            area_of(1200.0),
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!window.settings_editor.is_open());
        assert_ne!(window.font_size, 24.0);
        assert_eq!(metrics(&ctx), builtin);
        frame(&mut window, &ctx);
        assert_eq!(file.read(), saved);
        assert_eq!(window.font_size, 24.0);
        assert_ne!(metrics(&ctx), builtin);
        assert_eq!(
            window.settings_editor.application.family.as_deref(),
            Some("Ubuntu")
        );
        assert_eq!(window.settings_editor.application.committed, 1);
    }
    #[test]
    fn save_commits_once_after_the_next_begin_pass_and_resizes_all_quiet_consoles() {
        let file = File::new("size=14\n");
        let ctx = egui::Context::default();
        let mut window = window(&file);
        frame(&mut window, &ctx);
        frame(&mut window, &ctx);
        for tab in &mut window.tabs {
            for pane in tab.panes_mut() {
                let panel = crate::PanelDir::create().expect("panel identity fixture");
                fs::write(
                    panel.path().join("panel.json"),
                    r#"{"attention":"wait","lines":["published before Save"]}"#,
                )
                .unwrap();
                pane.panel_state.refresh(panel.path());
                pane.panel = Some(panel);
                for n in 0..80 {
                    pane.screen.advance(format!("history-{n}\r\n").as_bytes());
                }
            }
        }
        let before: Vec<_> = window
            .tabs
            .iter()
            .flat_map(|tab| tab.panes())
            .map(|pane| {
                (
                    pane.name,
                    pane.screen.size(),
                    pane.panel.as_ref().unwrap().path().to_path_buf(),
                    pane.panel_state
                        .attention(std::time::SystemTime::now())
                        .unwrap(),
                )
            })
            .collect();
        set(&mut window, "size", "28");
        save(&mut window);
        assert_eq!(window.settings_editor.application.committed, 0);
        assert_ne!(window.font_size, 28.0);
        frame(&mut window, &ctx);
        assert_eq!(window.settings_editor.application.committed, 0);
        frame(&mut window, &ctx);
        assert_eq!(window.settings_editor.application.committed, 1);
        frame(&mut window, &ctx);
        frame(&mut window, &ctx);
        assert_eq!(window.settings_editor.application.committed, 1);
        for ((name, old, panel_path, claim), pane) in before
            .iter()
            .zip(window.tabs.iter().flat_map(|tab| tab.panes()))
        {
            assert_eq!(*name, pane.name);
            assert_eq!(pane.panel.as_ref().unwrap().path(), panel_path);
            assert_eq!(
                pane.panel_state.attention(std::time::SystemTime::now()),
                Some(*claim)
            );
            assert!(
                fs::read_to_string(panel_path.join("panel.json"))
                    .unwrap()
                    .contains("published before Save")
            );
            assert!(pane.pty.ending().is_none(), "program output has not ended");
            assert!(pane.screen.scrollback_text().contains("history-0"));
            let new = pane.screen.size();
            assert!(
                new.cols < old.cols && new.rows < old.rows,
                "{old:?} -> {new:?}"
            );
        }
        assert_eq!(
            crate::settings::interpret(&file.read())
                .unwrap()
                .0
                .font_size,
            Some(28.0)
        );
    }
    #[test]
    fn failed_font_preparation_and_external_conflict_preserve_file_runtime_and_draft() {
        let file = File::new("size=14\n");
        let ctx = egui::Context::default();
        let mut window = window(&file);
        frame(&mut window, &ctx);
        let before = window.font_size;
        set(&mut window, "size", "20");
        set(&mut window, "font", "Missing fixture");
        save(&mut window);
        assert!(window.settings_editor.message.contains("not installed"));
        assert_eq!(file.read(), "size=14\n");
        assert_eq!(window.font_size, before);
        assert!(
            window
                .settings_editor
                .draft
                .as_ref()
                .unwrap()
                .text()
                .contains("Missing fixture")
        );
        set(&mut window, "font", "Ubuntu");
        fs::write(&file.0, "size=19\n").unwrap();
        save(&mut window);
        assert!(window.settings_editor.message.contains("changed outside"));
        frame(&mut window, &ctx);
        frame(&mut window, &ctx);
        assert_eq!(file.read(), "size=19\n");
        assert_eq!(window.font_size, before);
        assert_eq!(window.settings_editor.application.committed, 0);
        assert!(
            window
                .settings_editor
                .draft
                .as_ref()
                .unwrap()
                .text()
                .contains("Ubuntu")
        );
    }
    #[test]
    fn selected_fixture_changes_real_metrics_and_defaults_restore_them() {
        let file = File::new("");
        let ctx = egui::Context::default();
        let mut window = window(&file);
        frame(&mut window, &ctx);
        let builtin = metrics(&ctx);
        set(&mut window, "font", "Ubuntu");
        save(&mut window);
        frame(&mut window, &ctx);
        assert_eq!(metrics(&ctx), builtin);
        frame(&mut window, &ctx);
        assert_ne!(metrics(&ctx), builtin);
        window.settings_editor.draft.as_mut().unwrap().defaults();
        save(&mut window);
        frame(&mut window, &ctx);
        frame(&mut window, &ctx);
        assert_eq!(metrics(&ctx), builtin);
        assert_eq!(window.settings_editor.application.committed, 2);
    }
    #[test]
    fn unchanged_missing_startup_family_does_not_block_unrelated_save() {
        let file = File::new("font=Missing fixture\nsize=14\n");
        let ctx = egui::Context::default();
        let mut window = window(&file);
        window.settings_editor.application.family = Some("Missing fixture".into());
        frame(&mut window, &ctx);
        set(&mut window, "size", "20");
        save(&mut window);
        assert_eq!(window.settings_editor.application.prepared, 0);
        frame(&mut window, &ctx);
        frame(&mut window, &ctx);
        assert_eq!(window.font_size, 20.0);
        assert_eq!(window.settings_editor.application.committed, 1);
        assert!(file.read().contains("font=Missing fixture"));
    }
    #[test]
    fn transient_probe_observes_all_stages_without_touching_configuration() {
        let _ = reference_apply_report();
    }

    pub(crate) fn reference_apply_report() -> String {
        let file = File::new("# unchanged\nsize=14\n");
        let mut window = window(&file);
        window.settings_editor = Editor::resolved(crate::settings::Entry::Measuring, Some("apply"));
        window.settings_editor.probe.as_mut().unwrap().family = Some("Ubuntu".into());
        crate::settings::to_use(crate::settings::Entry::Measuring, None)
            .0
            .apply_to(&mut window);
        let ctx = egui::Context::default();
        for _ in 0..32 {
            frame(&mut window, &ctx);
            assert_eq!(
                window.blink.ceiling,
                Some(false),
                "every probe stage retains the measurement veto"
            );
        }
        assert!(!window.settings_editor.message.contains("Saved"));
        assert_eq!(window.settings_editor.application.font_activations, 3);
        let app = &window.settings_editor.application;
        assert_eq!(
            (app.prepared, app.failed, app.queued, app.committed),
            (4, 1, 3, 6)
        );
        let probe = window.settings_editor.probe.as_ref().unwrap();
        assert!(
            probe.color
                && probe.font_active
                && probe.unchanged
                && probe.reset
                && probe.warm
                && probe.failure
        );
        assert_eq!((probe.changed, probe.observed), (4, 4));
        assert_eq!(file.read(), "# unchanged\nsize=14\n");
        assert!(window.settings_editor.store.is_none());
        let mut report = Vec::new();
        window.settings_editor.publish(&mut report).unwrap();
        let report = String::from_utf8(report).unwrap();
        assert!(report.contains("settings persistence: vetoed"));
        report
    }
}
