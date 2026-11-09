//! Optional startup recovery never turns a secondary window into the snapshot writer.
use crate::{OpenWith, OpeningLayout, Window, session, settings::StartupRestore};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct PaneState {
    pub(crate) directory: Option<PathBuf>,
    pub(crate) failed: bool,
    pub(crate) resume: Option<panel_control::session::Identity>,
    pub(crate) resuming: bool,
    pub(crate) fresh_shell_required: bool,
    pub(crate) interacted: bool,
    pub(crate) session_status: Option<crate::ai_sessions::Status>,
    pub(crate) session_revision: u64,
    pub(crate) scope_unavailable: bool,
    pub(crate) session_directory: Option<PathBuf>,
    pub(crate) sessions: Option<crate::ai_sessions::Handle>,
    pub(crate) route: crate::resume::Route,
    pub(crate) wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
}

impl PaneState {
    pub(crate) fn take_over(&mut self) {
        if self.resuming {
            self.interacted = true;
        }
        if self.failed {
            self.failed = false;
            self.directory = None;
            self.resume = None;
            self.session_directory = None;
            self.resuming = false;
            self.fresh_shell_required = false;
            self.session_revision = 0;
        }
    }
}

pub(crate) struct State {
    mode: StartupRestore,
    primary: bool,
    measuring: bool,
    _ownership: Option<session::ownership::Ownership>,
    total: Duration,
    frames: usize,
    pending: Option<Duration>,
}

impl Default for State {
    fn default() -> Self {
        // Low-level constructors keep their historical persistence behavior. User startup always
        // replaces this with the settings-resolved policy before the first frame.
        Self {
            mode: StartupRestore::Layout,
            primary: true,
            measuring: true,
            _ownership: None,
            total: Duration::ZERO,
            frames: 0,
            pending: None,
        }
    }
}

impl State {
    fn record(&mut self, cost: Duration) {
        *self.pending.get_or_insert_default() += cost;
    }

    pub(crate) fn close_frame(&mut self) {
        if let Some(cost) = self.pending.take() {
            self.total += cost;
            self.frames += 1;
        }
    }

    pub(crate) fn captures_sessions(&self) -> bool {
        !self.measuring && self.primary && self.mode == StartupRestore::Sessions
    }

    pub(crate) fn can_save(&self) -> bool {
        self.primary && self.mode != StartupRestore::None
    }

    pub(crate) fn publish(&self, to: &mut dyn Write) -> std::io::Result<()> {
        writeln!(
            to,
            "restoration metadata per frame (cached local directories): mean_us={:.3}, frames={}",
            self.total.as_secs_f64() * 1e6 / self.frames.max(1) as f64,
            self.frames
        )
    }
}

pub(crate) fn valid_directory_text(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 4096
        && Path::new(text).is_absolute()
        && !text.chars().any(char::is_control)
}

/// The measurement path retains its seeded-layout semantics and overwrite witness.
pub(crate) fn prepare(
    persistence: &mut session::Persistence,
    path: Option<&Path>,
    mode: StartupRestore,
    measuring: bool,
    tabs: Option<&str>,
    panes: Option<&str>,
) -> (OpenWith, State) {
    if measuring {
        return (
            crate::open_with(tabs, panes, persistence.load()),
            State::default(),
        );
    }
    let ownership = path.and_then(Path::parent).and_then(|directory| {
        match session::ownership::Ownership::acquire(directory) {
            Ok(ownership) => Some(ownership),
            Err(error) => {
                eprintln!("Ivy cannot reserve its saved layout ({error}); this window opens without restoration or saving");
                None
            }
        }
    });
    let primary = ownership.as_ref().is_some_and(|lease| lease.is_primary());
    let saved = if primary && mode != StartupRestore::None {
        persistence.load()
    } else {
        None
    };
    let saved = match mode {
        StartupRestore::Sessions => saved,
        StartupRestore::Directories => saved.map(session::Validated::without_sessions),
        StartupRestore::None | StartupRestore::Layout => {
            saved.map(session::Validated::without_directories)
        }
    };
    let opening = match saved {
        Some(saved) => OpenWith::Restored(saved),
        None => OpenWith::Factory {
            tabs: 1,
            layout: OpeningLayout {
                columns: 1,
                rows: 1,
            },
        },
    };
    (
        opening,
        State {
            mode,
            primary,
            measuring: false,
            _ownership: ownership,
            total: Duration::ZERO,
            frames: 0,
            pending: None,
        },
    )
}

/// Consume successful Settings saves before a due write or clean shutdown can use the old policy.
pub(crate) fn apply_saved_mode(window: &mut Window) {
    if window.restoration.measuring {
        return;
    }
    let Some(mode) = window.settings_editor.take_saved_startup_restore() else {
        return;
    };
    let before = Instant::now();
    if window.restoration.mode == StartupRestore::None
        && mode != StartupRestore::None
        && window.restoration.primary
    {
        let _ = window.persistence.load();
    }
    if mode != StartupRestore::Sessions {
        for pane in window.all_panes_mut() {
            crate::resume::set_capture(pane, false);
            if pane.restoration.failed && pane.restoration.resume.is_some() {
                pane.restoration.failed = pane
                    .restoration
                    .directory
                    .as_ref()
                    .is_some_and(|directory| !directory.is_dir());
            }
            pane.restoration.resume = None;
            pane.restoration.session_directory = None;
            pane.restoration.resuming = false;
            pane.restoration.fresh_shell_required = false;
        }
    }
    window.restoration.mode = mode;
    window.last_seen = None;
    window.persistence.save_due = None;
    if !mode.has_directories() {
        for pane in window.all_panes_mut() {
            pane.restoration.directory = None;
            pane.restoration.failed = false;
        }
    }
    let cost = before.elapsed();
    window.restoration.record(cost);
    window.timed_separately += cost;
}

pub(crate) fn refresh(window: &mut Window) {
    if window.restoration.measuring
        || !window.restoration.primary
        || !window.restoration.mode.has_directories()
    {
        return;
    }
    let before = Instant::now();
    let capture_sessions = window.restoration.mode == StartupRestore::Sessions;
    if capture_sessions {
        for pane in window.all_panes_mut() {
            crate::resume::set_capture(pane, !pane.restoration.failed);
            crate::resume::inspect_restored_exit(pane);
        }
    }
    let projects = &window.projects;
    for tab in &mut window.tabs {
        for pane in tab.panes_mut() {
            if !pane.restoration.failed {
                if capture_sessions {
                    crate::resume::capture(pane, None);
                    if pane.restoration.failed {
                        continue;
                    }
                }
                let unusable = pane.screen.working_directory_reported()
                    && pane.screen.working_directory().is_none();
                observe_directory(
                    &mut pane.restoration,
                    projects.directory(pane.name),
                    unusable,
                );
            }
        }
    }
    let cost = before.elapsed();
    window.restoration.record(cost);
    window.timed_separately += cost;
}

fn observe_directory(state: &mut PaneState, observed: Option<&Path>, explicitly_unusable: bool) {
    let observed = state.session_directory.as_deref().or(observed);
    if explicitly_unusable {
        state.directory = None;
    } else if let Some(path) = observed {
        if !path.to_str().is_some_and(valid_directory_text) {
            state.directory = None;
        } else if state.directory.as_deref() != Some(path) {
            state.directory = Some(path.to_path_buf());
        }
    }
}

/// A final native observation covers navigation immediately before closing, without an idle timer.
pub(crate) fn capture_on_exit(window: &mut Window) {
    apply_saved_mode(window);
    if window.restoration.measuring
        || !window.restoration.can_save()
        || !window.restoration.mode.has_directories()
    {
        return;
    }
    for pane in window.all_panes_mut() {
        if pane.restoration.resuming && pane.pty.exit_success() == Some(false) {
            pane.restoration.failed = true;
            pane.restoration.fresh_shell_required = true;
            pane.restoration.resuming = false;
        }
    }
    let snapshots = if window.restoration.mode == StartupRestore::Sessions {
        let handles: Vec<_> = window
            .all_panes()
            .filter_map(|pane| pane.restoration.sessions.as_ref())
            .collect();
        crate::ai_sessions::freeze_all(&handles, Duration::from_millis(1250))
    } else {
        Vec::new()
    };
    let mut snapshots = snapshots.into_iter();
    let capture_sessions = window.restoration.mode == StartupRestore::Sessions;
    for pane in window.all_panes_mut() {
        let snapshot = pane
            .restoration
            .sessions
            .as_ref()
            .and_then(|_| snapshots.next());
        if pane.restoration.failed {
            continue;
        }
        if capture_sessions {
            crate::resume::capture(pane, snapshot);
            if pane.restoration.failed {
                continue;
            }
        }
        let unusable =
            pane.screen.working_directory_reported() && pane.screen.working_directory().is_none();
        let directory = pane
            .pty
            .working_directory()
            .or_else(|| pane.screen.working_directory().map(local_directory));
        observe_directory(&mut pane.restoration, directory.as_deref(), unusable);
        if capture_sessions
            && pane.restoration.resuming
            && pane
                .restoration
                .resume
                .as_ref()
                .is_some_and(|identity| identity.client != panel_control::session::Client::Codex)
        {
            pane.restoration.resume = None;
        }
    }
}

fn local_directory(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        PathBuf::from(text.strip_prefix('/').unwrap_or(&text))
    }
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
}

pub(crate) fn open_leaf(
    ctx: &eframe::egui::Context,
    leaf: &session::StoredLeaf,
    program: &str,
    name: usize,
    reads: crate::control::ReadActivity,
) -> term_core::Result<crate::Pane> {
    if leaf.resume.is_some() {
        match crate::resume::try_open(ctx, leaf, name, reads.clone()) {
            Ok(pane) => return Ok(pane),
            Err(error) => {
                eprintln!(
                    "The saved AI session could not be started ({error}); opening a fresh console"
                );
                let mut shell = leaf.clone();
                shell.resume = None;
                let mut pane = open_leaf(ctx, &shell, program, name, reads)?;
                pane.restoration.resume = leaf.resume.clone();
                pane.restoration.failed = true;
                return Ok(pane);
            }
        }
    }
    let directory = leaf.cwd.as_deref().filter(|path| path.is_dir());
    let opened = crate::opening::launch_pane_in(
        ctx,
        program,
        &[],
        crate::panel_or_none(panel_contract::PanelDir::create()),
        name,
        reads.clone(),
        directory,
    );
    let (mut pane, failed) = match opened {
        Ok(pane) => (pane, leaf.cwd.is_some() && directory.is_none()),
        Err(_) if directory.is_some() => (
            crate::opening::launch_pane_in(
                ctx,
                program,
                &[],
                crate::panel_or_none(panel_contract::PanelDir::create()),
                name,
                reads,
                None,
            )?,
            true,
        ),
        Err(error) => return Err(error),
    };
    pane.restoration.directory = leaf.cwd.clone();
    pane.restoration.failed = failed;
    Ok(pane)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{StoredDocument, StoredLeaf, StoredNode, StoredTab};

    struct Fixture {
        directory: PathBuf,
        snapshot: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let mut id = [0u8; 8];
            getrandom::fill(&mut id).unwrap();
            let directory =
                std::env::temp_dir().join(format!("ivy-restore-{:x}", u64::from_le_bytes(id)));
            std::fs::create_dir(&directory).unwrap();
            let snapshot = directory.join("layout.json");
            Self {
                directory,
                snapshot,
            }
        }
        fn seed(&self, cwd: Option<PathBuf>) -> Vec<u8> {
            let document = StoredDocument {
                v: session::VERSION,
                tabs: (0..2)
                    .map(|_| StoredTab {
                        label: None,
                        root: StoredNode::Leaf(StoredLeaf {
                            resume: None,
                            panel_width: 37,
                            folded: true,
                            label: Some("Saved console".into()),
                            cwd: cwd.clone(),
                        }),
                    })
                    .collect(),
            };
            let bytes = session::serialize(&document).unwrap().into_bytes();
            std::fs::write(&self.snapshot, &bytes).unwrap();
            bytes
        }
        fn prepare(&self, mode: StartupRestore) -> (OpenWith, State, session::Persistence) {
            let mut persistence = session::Persistence::at(Some(self.snapshot.clone()));
            let (opening, state) = prepare(
                &mut persistence,
                Some(&self.snapshot),
                mode,
                false,
                None,
                None,
            );
            (opening, state, persistence)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn startup_none_opens_one_console_and_cannot_replace_the_existing_snapshot() {
        let fixture = Fixture::new();
        let bytes = fixture.seed(Some(fixture.directory.clone()));
        let (opening, state, persistence) = fixture.prepare(StartupRestore::None);
        assert!(
            matches!(&opening, OpenWith::Factory { tabs: 1, layout } if layout.columns == 1 && layout.rows == 1)
        );
        assert!(
            persistence.load_time().is_none(),
            "disabled startup must not read the saved document"
        );
        let mut window =
            Window::opening(&Default::default(), opening, "/bin/sh", None, persistence).unwrap();
        window.restoration = state;
        window.active_mut().panes_mut()[0].label = Some("A change in the fresh window".into());
        window.check_whether_the_layout_moved();
        window.save_now();
        assert_eq!(std::fs::read(&fixture.snapshot).unwrap(), bytes);
        assert_eq!(window.tabs.len(), 1);
        assert_eq!(window.all_panes().count(), 1);
    }

    #[test]
    fn layout_restores_geometry_but_discards_saved_directories() {
        let fixture = Fixture::new();
        fixture.seed(Some(fixture.directory.clone()));
        let (opening, state, persistence) = fixture.prepare(StartupRestore::Layout);
        let mut window =
            Window::opening(&Default::default(), opening, "/bin/sh", None, persistence).unwrap();
        window.restoration = state;
        assert_eq!(window.tabs.len(), 2);
        for pane in window.all_panes() {
            assert_eq!(pane.panel_width, 37);
            assert!(pane.folded);
            assert_eq!(pane.label.as_deref(), Some("Saved console"));
            assert!(pane.restoration.directory.is_none());
        }
        let ids: std::collections::HashSet<_> = window
            .all_panes()
            .map(|pane| pane.panel.as_ref().unwrap().id().to_owned())
            .collect();
        assert_eq!(ids.len(), 2);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn directory_restore_sets_the_real_child_cwd_without_console_input() {
        let fixture = Fixture::new();
        fixture.seed(Some(fixture.directory.clone()));
        let (opening, state, persistence) = fixture.prepare(StartupRestore::Directories);
        let mut window =
            Window::opening(&Default::default(), opening, "/bin/sh", None, persistence).unwrap();
        window.restoration = state;
        for pane in window.all_panes_mut() {
            let deadline = Instant::now() + Duration::from_secs(2);
            while pane.pty.working_directory().as_deref() != Some(fixture.directory.as_path()) {
                assert!(
                    Instant::now() < deadline,
                    "restored process did not enter its saved directory"
                );
                std::thread::yield_now();
            }
            assert!(pane.keystrokes.is_empty());
            assert!(!pane.restoration.failed);
        }
    }

    #[test]
    fn missing_directory_is_visible_and_survives_an_unrelated_save_until_takeover() {
        let fixture = Fixture::new();
        let missing = fixture.directory.join("removed");
        fixture.seed(Some(missing.clone()));
        let (opening, state, persistence) = fixture.prepare(StartupRestore::Directories);
        let mut window =
            Window::opening(&Default::default(), opening, "/bin/sh", None, persistence).unwrap();
        window.restoration = state;
        assert!(matches!(
            crate::painter::notices::what_the_silence_is(window.active().panes()[0]),
            Some(crate::painter::notices::Silence::RestoreDirectory)
        ));
        window.active_mut().panes_mut()[0].label = Some("Renamed while recovering".into());
        window.check_whether_the_layout_moved();
        window.save_now();
        let saved = session::read(&fixture.snapshot).unwrap();
        assert!(
            matches!(&saved.tabs()[0].root, StoredNode::Leaf(leaf) if leaf.cwd.as_ref() == Some(&missing))
        );
        window.active_mut().panes_mut()[0].hand_it_back_to_the_person();
        window.check_whether_the_layout_moved();
        window.save_now();
        let saved = session::read(&fixture.snapshot).unwrap();
        assert!(matches!(&saved.tabs()[0].root, StoredNode::Leaf(leaf) if leaf.cwd.is_none()));
        assert!(!window.active().panes()[0].restoration.failed);
    }

    #[test]
    fn startup_child() {
        use std::io::{Read, Write};
        let Ok(snapshot) = std::env::var("IVY_STARTUP_TEST_SNAPSHOT") else {
            return;
        };
        let mut gate = [0u8; 1];
        std::io::stdin().read_exact(&mut gate).unwrap();
        let snapshot = PathBuf::from(snapshot);
        let mut persistence = session::Persistence::at(Some(snapshot.clone()));
        let (opening, _state) = prepare(
            &mut persistence,
            Some(&snapshot),
            StartupRestore::Layout,
            false,
            None,
            None,
        );
        println!(
            "STARTUP_RESTORED={}",
            matches!(opening, OpenWith::Restored(_))
        );
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_exact(&mut gate).unwrap();
    }

    struct StartupChild {
        child: std::process::Child,
        input: Option<std::process::ChildStdin>,
        output: std::io::BufReader<std::process::ChildStdout>,
    }
    impl StartupChild {
        fn spawn(snapshot: &Path) -> Self {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "restoration::tests::startup_child",
                    "--exact",
                    "--nocapture",
                ])
                .env("IVY_STARTUP_TEST_SNAPSHOT", snapshot)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let input = child.stdin.take();
            let output = std::io::BufReader::new(child.stdout.take().unwrap());
            Self {
                child,
                input,
                output,
            }
        }
        fn restored(&mut self) -> bool {
            use std::io::{BufRead, Write};
            self.input.as_mut().unwrap().write_all(b"s").unwrap();
            loop {
                let mut line = String::new();
                assert!(
                    self.output.read_line(&mut line).unwrap() > 0,
                    "startup helper exited before answering"
                );
                if let Some((_, value)) = line.split_once("STARTUP_RESTORED=") {
                    return value.trim() == "true";
                }
            }
        }
        fn close(&mut self) {
            use std::io::Write;
            self.input.take().unwrap().write_all(b"x").unwrap();
            assert!(self.child.wait().unwrap().success());
        }
    }
    impl Drop for StartupChild {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[test]
    fn secondary_start_cannot_restore_even_after_the_primary_exits() {
        let fixture = Fixture::new();
        fixture.seed(None);
        let mut primary = StartupChild::spawn(&fixture.snapshot);
        assert!(primary.restored());
        let mut secondary = StartupChild::spawn(&fixture.snapshot);
        assert!(!secondary.restored());
        primary.close();
        let mut third = StartupChild::spawn(&fixture.snapshot);
        assert!(!third.restored());
        secondary.close();
        third.close();
        let mut next = StartupChild::spawn(&fixture.snapshot);
        assert!(next.restored());
        next.close();
    }

    #[test]
    fn saved_directory_shape_rejects_relative_control_and_oversize_paths() {
        for invalid in [
            "relative".to_owned(),
            "/tmp/bad\nname".to_owned(),
            format!("/{}", "a".repeat(4096)),
        ] {
            let fixture = Fixture::new();
            fixture.seed(Some(PathBuf::from(invalid)));
            assert!(session::read(&fixture.snapshot).is_err());
        }
    }

    #[test]
    fn oversized_serialized_snapshot_never_replaces_the_previous_file() {
        let fixture = Fixture::new();
        let original = fixture.seed(None);
        let directory = PathBuf::from(format!("/{}", "a".repeat(3000)));
        let doc = StoredDocument {
            v: session::VERSION,
            tabs: (0..session::MAX_LEAVES)
                .map(|_| StoredTab {
                    label: None,
                    root: StoredNode::Leaf(StoredLeaf {
                        resume: None,
                        panel_width: 37,
                        folded: false,
                        label: None,
                        cwd: Some(directory.clone()),
                    }),
                })
                .collect(),
        };
        let validated = session::validate(doc).unwrap();
        let mut persistence = session::Persistence::at(Some(fixture.snapshot.clone()));
        assert!(matches!(
            persistence.save(&validated),
            Err(session::SaveFailure::DoesntFit)
        ));
        assert_eq!(std::fs::read(&fixture.snapshot).unwrap(), original);
        assert_eq!(
            std::fs::read_dir(&fixture.directory).unwrap().count(),
            1,
            "oversize must not create a temporary"
        );
    }

    #[test]
    fn verified_client_directory_wins_over_wrapper_cwd_but_not_remote_veto() {
        let declared = PathBuf::from("/tmp/client-project");
        let wrapper = Path::new("/tmp/launch-wrapper");
        let mut state = PaneState {
            session_directory: Some(declared.clone()),
            ..Default::default()
        };
        observe_directory(&mut state, Some(wrapper), false);
        assert_eq!(state.directory, Some(declared));
        observe_directory(&mut state, Some(wrapper), true);
        assert!(state.directory.is_none());
    }

    #[test]
    fn explicit_invalid_directory_clears_old_context_but_missing_observation_preserves_it() {
        let previous = PathBuf::from("/tmp/previous-project");
        let mut state = PaneState {
            directory: Some(previous.clone()),
            ..Default::default()
        };
        observe_directory(&mut state, None, false);
        assert_eq!(state.directory, Some(previous));
        let mut screen = term_core::Screen::new(crate::INITIAL_GRID);
        screen.advance(b"\x1b]7;file:///tmp/a%C2%85b\x07");
        let declared = screen
            .working_directory()
            .expect("OSC 7 accepts this explicit local path");
        observe_directory(&mut state, Some(declared), false);
        assert!(
            state.directory.is_none(),
            "an invalid new path must not retain an old project"
        );
    }

    #[test]
    fn long_missing_directory_keeps_a_visible_notice_without_losing_its_full_target() {
        let ctx = eframe::egui::Context::default();
        let path = PathBuf::from(format!("/tmp/{}", "a/".repeat(2000)));
        let leaf = StoredLeaf {
            resume: None,
            panel_width: 37,
            folded: false,
            label: None,
            cwd: Some(path.clone()),
        };
        let pane = open_leaf(&ctx, &leaf, "/bin/sh", 0, Default::default()).unwrap();
        assert!(pane.restoration.failed);
        ctx.run_ui(Default::default(), |ui| {
            let metrics = crate::painter::CellMetrics::measure(
                ui.ctx(),
                eframe::egui::FontId::monospace(14.0),
            );
            let area = eframe::egui::Rect::from_min_size(
                eframe::egui::Pos2::ZERO,
                eframe::egui::vec2(1200.0, 760.0),
            );
            let boxes = crate::painter::notices::the_boxes(
                ui.painter(),
                &pane,
                &crate::theme::Theme::default(),
                &metrics,
                area,
            );
            assert!(
                boxes.silence.is_some(),
                "an accepted long path must not hide the fallback explanation"
            );
        })
        .drop_without_applying_deltas();
        assert_eq!(
            pane.restoration.directory,
            Some(path),
            "only display text may be shortened"
        );
    }

    #[test]
    fn repeated_capture_passes_close_one_actual_frame_sample() {
        let fixture = Fixture::new();
        let (opening, state, persistence) = fixture.prepare(StartupRestore::Directories);
        let mut window =
            Window::opening(&Default::default(), opening, "/bin/sh", None, persistence).unwrap();
        window.restoration = state;
        refresh(&mut window);
        refresh(&mut window);
        assert_eq!(window.restoration.frames, 0, "passes are not frame samples");
        window.close_the_frame(Duration::from_secs(1));
        assert_eq!(window.restoration.frames, 1);
        let first = window.restoration.total;
        window.close_the_frame(Duration::from_secs(1));
        assert_eq!(
            window.restoration.frames, 1,
            "a frame with no new capture is not another sample"
        );
        assert_eq!(window.restoration.total, first);
    }

    #[test]
    fn legacy_measurement_restores_seeded_layout_despite_the_default_none_mode() {
        let fixture = Fixture::new();
        fixture.seed(None);
        let mut persistence = session::Persistence::at(Some(fixture.snapshot.clone()));
        persistence.forbid_overwrite("measurement");
        let (opening, state) = prepare(
            &mut persistence,
            Some(&fixture.snapshot),
            StartupRestore::None,
            true,
            None,
            None,
        );
        assert!(matches!(opening, OpenWith::Restored(_)));
        assert!(state.can_save());
        assert!(!fixture.directory.join("window-leases").exists());
    }
}
