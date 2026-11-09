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
}

impl PaneState {
    pub(crate) fn take_over(&mut self) {
        if self.failed {
            self.failed = false;
            self.directory = None;
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
        }
    }
}

impl State {
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

fn savable(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| path.to_str().is_some_and(valid_directory_text))
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
    let saved = if mode == StartupRestore::Directories {
        saved
    } else {
        saved.map(session::Validated::without_directories)
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
        },
    )
}

pub(crate) fn refresh(window: &mut Window) {
    if window.restoration.measuring {
        return;
    }
    let before = Instant::now();
    if let Some(mode) = window.settings_editor.take_saved_startup_restore() {
        if window.restoration.mode == StartupRestore::None
            && mode != StartupRestore::None
            && window.restoration.primary
        {
            // Disabled startup did not read the snapshot; learn a newer-version veto before writing.
            let _ = window.persistence.load();
        }
        window.restoration.mode = mode;
        window.last_seen = None;
    }
    if window.restoration.mode != StartupRestore::Directories {
        for pane in window.all_panes_mut() {
            pane.restoration.directory = None;
            pane.restoration.failed = false;
        }
    } else if window.restoration.primary {
        let sources: std::collections::HashMap<_, _> = window
            .all_panes()
            .map(|pane| (pane.name, window.projects.directory(pane.name)))
            .collect();
        for pane in window.all_panes_mut() {
            if !pane.restoration.failed {
                if pane.screen.working_directory_reported()
                    && pane.screen.working_directory().is_none()
                {
                    pane.restoration.directory = None;
                } else if let Some(directory) = sources
                    .get(&pane.name)
                    .and_then(|directory| savable(directory.clone()))
                {
                    pane.restoration.directory = Some(directory);
                }
            }
        }
    }
    let cost = before.elapsed();
    window.restoration.total += cost;
    window.restoration.frames += 1;
    window.timed_separately += cost;
}

/// A final native observation covers navigation immediately before closing, without an idle timer.
pub(crate) fn capture_on_exit(window: &mut Window) {
    if window.restoration.measuring
        || !window.restoration.can_save()
        || window.restoration.mode != StartupRestore::Directories
    {
        return;
    }
    for pane in window.all_panes_mut() {
        if pane.restoration.failed {
            continue;
        }
        let unusable =
            pane.screen.working_directory_reported() && pane.screen.working_directory().is_none();
        if unusable {
            pane.restoration.directory = None;
        } else if let Some(directory) = savable(
            pane.pty
                .working_directory()
                .or_else(|| pane.screen.working_directory().map(local_directory)),
        ) {
            pane.restoration.directory = Some(directory);
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
    fn secondary_start_cannot_restore_even_after_the_primary_exits() {
        let fixture = Fixture::new();
        fixture.seed(None);
        let (first, primary, _) = fixture.prepare(StartupRestore::Layout);
        assert!(matches!(first, OpenWith::Restored(_)));
        assert!(primary.can_save());
        let (second, secondary, _) = fixture.prepare(StartupRestore::Directories);
        assert!(matches!(second, OpenWith::Factory { tabs: 1, .. }));
        assert!(!secondary.can_save());
        drop(primary);
        let (third, third_state, _) = fixture.prepare(StartupRestore::Layout);
        assert!(matches!(third, OpenWith::Factory { tabs: 1, .. }));
        assert!(!third_state.can_save());
        drop(secondary);
        drop(third_state);
        let (next, next_state, _) = fixture.prepare(StartupRestore::Layout);
        assert!(matches!(next, OpenWith::Restored(_)));
        assert!(next_state.can_save());
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
