//! Pane creation shared by shortcuts and the header controls.

use crate::painter::CellMetrics;
use crate::tiling::{Orientation, Side, can_split};
use crate::{Pane, Window};
use eframe::egui;
use std::time::Instant;

impl Window {
    pub(crate) fn split_the_focused(
        &mut self,
        cut_along: Orientation,
        ctx: &egui::Context,
        whole_window: egui::Rect,
        metrics: &CellMetrics,
    ) {
        let before = Instant::now();
        #[cfg(test)]
        let before = crate::tests::OperationClock::start(before);
        split(
            self,
            self.active().focus,
            cut_along,
            ctx,
            whole_window,
            metrics,
        );
        self.timed_separately += before.elapsed();
    }

    #[cfg(test)]
    pub(crate) fn graft_the_new_one(
        &mut self,
        opened: term_core::Result<Pane>,
        cut_along: Orientation,
        ctx: &egui::Context,
    ) {
        graft(self, self.active().focus, opened, cut_along, ctx);
    }
}

/// Splits the named pane and gives focus to the newborn only after a successful graft.
///
/// **Focus passes to the new one because splitting is opening a console**, and whoever opens a
/// console wants to write in it; it is the same thing creating a tab does by putting it in
/// front. Staying on the old one would force a click to use what has just been asked for.
///
/// **If it does not fit, nothing is split and nothing is spent**: not a name, not a `fork`. A
/// child below the minimum cannot be used, so it is refused before launching a program.
///
/// **What it costs is declared in full**, and they are two things no pane is going to declare:
/// the tree walk to find the target, and the new program's `fork`/`exec`.
/// Without this, the frame's cost is shared out by subtraction and both would end up published
/// as interface framework cost — the figure D3 judges it with. The shortcut times this call;
/// a header press is already inside the mouse stopwatch and must not subtract it twice.
pub(crate) fn split(
    window: &mut Window,
    target: usize,
    cut_along: Orientation,
    ctx: &egui::Context,
    whole_window: egui::Rect,
    metrics: &CellMetrics,
) {
    // The bar is set aside with the tabs there are **now**: if the same batch of events has
    // just created one, the height left for the consoles is already a different one.
    let area = crate::tab_bar::geometry(window, whole_window, metrics).consoles;
    let target_area = window
        .active()
        .panes()
        .iter()
        .position(|pane| pane.name == target)
        .map(|at| {
            // The leaves and their rectangles are matched by position: the two walks go in the same
            // order, which is the invariant the whole painting already rests on.
            window.active().root.place(area, metrics.width()).parts[at]
        });

    // A target removed before its queued action is applied cannot open a replacement console.
    let Some(its_area) = target_area else {
        return;
    };
    if !can_split(its_area, cut_along, metrics, window.headers) {
        // **It is said out loud, because the chord has swallowed the key and nothing has
        // happened.** Without this line, in a narrow window the shortcut reads as broken: it
        // neither splits, nor reaches the program inside, nor leaves a trace of why. It is the
        // same as the sibling path already does —the console that could not be opened—, and for
        // now it is all that can be done: saying it on screen would need a way of warning that
        // this phase does not have.
        eprintln!(
            "it is not split: the pane cannot give both children their minimum. Make the window bigger \
                 or move the divider"
        );
        return;
    }

    let name = window.next_pane_name;
    let directory = window
        .active_mut()
        .panes_mut()
        .into_iter()
        .find(|pane| pane.name == target)
        .and_then(|pane| {
            // A remote/invalid report must not turn into the local ssh process's directory.
            if pane.screen.working_directory_reported() && pane.screen.working_directory().is_none()
            {
                return None;
            }
            // Native discovery sees a cd even before a shell publishes its next OSC 7 report.
            pane.pty.working_directory().or_else(|| {
                pane.screen
                    .working_directory()
                    .and_then(published_directory)
            })
        });
    if directory.is_none() {
        eprintln!(
            "the console's working directory is unavailable; splitting in the default directory"
        );
    }
    let opened = crate::opening::new_in_directory(
        ctx,
        name,
        window.control.read_activity(),
        directory.as_deref(),
    );
    // **The name is spent even if the program never gets to start**, just as when opening a
    // tab: whatever is opened afterwards must not be called the same as what never got to
    // exist, because the report may already have named the failed attempt on the error output.
    window.next_pane_name += 1;

    // **The stopwatch is not closed before the graft, and that line was drawn in the wrong
    // place.** Putting the leaf into the tree and rearming the watcher are the two
    // **expensive** parts of splitting —the second rebuilds the watcher over the panes of every
    // tab and makes them reread their `panel.json`—, and they happen inside the interface pass
    // without any pane declaring them. Closing it here, that work fell into «the rest of the
    // frame», which is what this project calls the interface framework cost: the figure D3
    // decides with.
    graft(window, target, opened, cut_along, ctx);
}

/// Puts the console that has just been opened next to its target, or leaves the window as
/// it was if it could not be opened.
///
/// **It receives the attempt already made instead of making it**, just like
/// [`Window::bring_to_front`] with the tabs and for the same reason: opening a console only
/// fails when the system gives no more pseudoterminals or descriptors, and that cannot be
/// provoked from a test without touching the whole process. Splitting it here is what makes it
/// possible to assert what the case promises — that a failure leaves no half pane, does not
/// move the focus and rearms nothing.
fn graft(
    window: &mut Window,
    target: usize,
    opened: term_core::Result<Pane>,
    cut_along: Orientation,
    ctx: &egui::Context,
) {
    let new = match opened {
        Ok(new) => new,
        Err(failure) => {
            eprintln!("the pane could not be split ({failure})");
            return;
        }
    };

    // **The name focus will go to is taken from the console being grafted**, and not from a
    // parameter saying which one it is. The two would be worth the same as long as the caller
    // took them from the same place, but the day they come apart the focus would point at a
    // pane that does not exist: the keys would reach no console at all —they would be lost in
    // silence—, the highlight would not be painted, and the next splitting chord would find
    // nothing to split.
    let name = new.name;
    let tab = window.active_mut();
    if let Some(leftover) = tab.root.split(target, Side::behind(cut_along), new) {
        // **It cannot happen**: the caller has just found the target among the leaves of this
        // very tab, and nothing has touched the tree in between. It is said out loud instead of
        // kept quiet because the console really did get opened, and on dropping it here its
        // program dies and its directory disappears: without this line, a shell opened and
        // closed in the same frame would leave no trace at all.
        drop(leftover);
        eprintln!("the pane was not found when splitting; nothing has been split");
        return;
    }

    tab.focus = name;
    // Without rearming, the watcher knows nothing about the directory of the one that has just
    // been born and its panel would stay frozen with nothing saying so. It is the same as
    // opening a tab does.
    window.rearm_the_watcher(ctx);
}

/// OSC 7 uses a URI path; Windows drive paths arrive with a leading slash.
fn published_directory(path: &std::path::Path) -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let path = path.to_str()?;
        Some(std::path::PathBuf::from(
            path.strip_prefix('/').unwrap_or(path),
        ))
    }
    #[cfg(not(windows))]
    {
        Some(path.to_path_buf())
    }
}

#[cfg(all(test, unix))]
mod tests {
    //! The newborn shell writes pwd to a file: terminal echo cannot forge the assertion.
    use super::*;
    use crate::tests::window_of_tabs;
    use std::{fs, path::PathBuf, time::Duration};

    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new() -> Self {
            let mut nonce = [0u8; 8];
            getrandom::fill(&mut nonce).unwrap();
            let path =
                std::env::temp_dir().join(format!("ivy-split-{}", u64::from_ne_bytes(nonce)));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn directory(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).unwrap();
            path
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn wait(mut ready: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready() {
            assert!(Instant::now() < deadline, "directory operation timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn area() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0))
    }
    fn assert_newborn_pwd(window: &mut Window, sandbox: &Sandbox, expected: &std::path::Path) {
        let output = sandbox.0.join("pwd.txt");
        let name = window.active().focus;
        window
            .all_panes_mut()
            .find(|p| p.name == name)
            .unwrap()
            .pty
            .write(format!("command pwd > '{}'\n", output.display()).as_bytes())
            .unwrap();
        wait(|| {
            fs::read_to_string(&output).is_ok_and(|s| s.trim_end() == expected.to_string_lossy())
        });
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn split_directory_tracks_cd_for_both_orientations() {
        for direction in [Orientation::Columns, Orientation::Rows] {
            let sandbox = Sandbox::new();
            let first = sandbox.directory("first");
            let second = sandbox.directory("second space;$dollar");
            let ready = sandbox.0.join("ready");
            let change = sandbox.0.join("change");
            let changed = sandbox.0.join("changed");
            let script = format!(
                "cd '{}' && touch '{}' && while ! test -f '{}'; do sleep 0.01; done; cd '{}' && touch '{}' && exec sleep 30",
                first.display(),
                ready.display(),
                change.display(),
                second.display(),
                changed.display()
            );
            let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
            wait(|| ready.exists());
            let original = std::env::current_dir().unwrap();
            // A stale, valid local report must not override the live cd on Linux.
            window.active_mut().panes_mut()[0]
                .screen
                .advance(format!("\x1b]7;file://{}\x07", first.display()).as_bytes());
            fs::write(&change, "").unwrap();
            wait(|| changed.exists());
            let ctx = egui::Context::default();
            window.split_the_focused(direction, &ctx, area(), &CellMetrics::of(8.0, 16.0));
            assert_eq!(window.active().panes().len(), 3);
            assert_newborn_pwd(&mut window, &sandbox, &second);
            assert_eq!(std::env::current_dir().unwrap(), original);
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn split_directory_uses_named_toolbar_target_instead_of_focus() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("clicked console");
        let ready = sandbox.0.join("ready");
        let script = format!(
            "cd '{}' && touch '{}' && exec sleep 30",
            source.display(),
            ready.display()
        );
        let mut window = window_of_tabs([["sleep 30", script.as_str()]]);
        wait(|| ready.exists());
        let target = window.active().panes()[1].name;
        assert_ne!(window.active().focus, target);
        split(
            &mut window,
            target,
            Orientation::Rows,
            &egui::Context::default(),
            area(),
            &CellMetrics::of(8.0, 16.0),
        );
        assert_eq!(window.active().panes().len(), 3);
        assert_newborn_pwd(&mut window, &sandbox, &source);
    }

    #[test]
    fn split_directory_osc_report_survives_source_exit_and_deleted_path_refuses_split() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("published directory");
        let mut window = window_of_tabs([["exit 0", "sleep 30"]]);
        wait(|| {
            window.active_mut().panes_mut()[0]
                .pty
                .working_directory()
                .is_none()
        });
        window.active_mut().panes_mut()[0]
            .screen
            .advance(format!("\x1b]7;file://{}\x07", source.display()).as_bytes());
        let target = window.active().focus;
        split(
            &mut window,
            target,
            Orientation::Columns,
            &egui::Context::default(),
            area(),
            &CellMetrics::of(8.0, 16.0),
        );
        assert_eq!(window.active().panes().len(), 3);
        assert_newborn_pwd(&mut window, &sandbox, &source);
        fs::remove_dir(&source).unwrap();
        let before = window.active().focus;
        split(
            &mut window,
            target,
            Orientation::Rows,
            &egui::Context::default(),
            area(),
            &CellMetrics::of(8.0, 16.0),
        );
        assert_eq!(window.active().panes().len(), 3);
        assert_eq!(window.active().focus, before);
    }

    #[test]
    fn split_directory_unknown_after_source_exit_uses_ordinary_launch_directory() {
        let sandbox = Sandbox::new();
        let output = sandbox.0.join("ordinary-pwd.txt");
        let ctx = egui::Context::default();
        // Observe the ordinary user-shell launch, including its own startup configuration.
        let mut ordinary =
            Pane::new_counting_reads(&ctx, 90, crate::control::ReadActivity::default()).unwrap();
        ordinary
            .pty
            .write(format!("command pwd > '{}'\n", output.display()).as_bytes())
            .unwrap();
        wait(|| fs::read_to_string(&output).is_ok_and(|s| !s.trim().is_empty()));
        let expected = PathBuf::from(fs::read_to_string(&output).unwrap().trim_end());
        let mut window = window_of_tabs([["exit 0", "sleep 30"]]);
        wait(|| {
            window.active_mut().panes_mut()[0]
                .pty
                .working_directory()
                .is_none()
        });
        assert!(
            !window.active().panes()[0]
                .screen
                .working_directory_reported()
        );
        window.split_the_focused(Orientation::Rows, &ctx, area(), &CellMetrics::of(8.0, 16.0));
        assert_eq!(window.active().panes().len(), 3);
        assert_newborn_pwd(&mut window, &sandbox, &expected);
    }

    #[test]
    fn split_directory_fallback_shell_receives_the_same_directory() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("fallback directory");
        let pane = crate::opening::launch_pane_in(
            &egui::Context::default(),
            "/no-such-ivy-shell",
            &[],
            None,
            0,
            crate::control::ReadActivity::default(),
            Some(&source),
        )
        .unwrap();
        let tab = crate::Tab::with_panes(
            vec![pane],
            &crate::OpeningLayout {
                columns: 1,
                rows: 1,
            },
            0,
        );
        let mut window = Window::with_these_tabs(
            &egui::Context::default(),
            vec![tab],
            None,
            crate::control::WindowActivity::default(),
        )
        .unwrap();
        assert_newborn_pwd(&mut window, &sandbox, &source);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn split_directory_foreign_report_vetoes_native_and_previous_local_directory() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("local ssh directory");
        let ready = sandbox.0.join("ready");
        let script = format!(
            "cd '{}' && touch '{}' && exec sleep 30",
            source.display(),
            ready.display()
        );
        let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
        wait(|| ready.exists());
        window.active_mut().panes_mut()[0].screen.advance(
            format!(
                "\x1b]7;file://{}\x07\x1b]7;file://remote.invalid/some/project\x07",
                source.display()
            )
            .as_bytes(),
        );
        window.split_the_focused(
            Orientation::Columns,
            &egui::Context::default(),
            area(),
            &CellMetrics::of(8.0, 16.0),
        );
        assert_eq!(window.active().panes().len(), 3);
        let output = sandbox.0.join("remote-pwd.txt");
        let name = window.active().focus;
        window
            .all_panes_mut()
            .find(|p| p.name == name)
            .unwrap()
            .pty
            .write(format!("command pwd > '{}'\n", output.display()).as_bytes())
            .unwrap();
        wait(|| fs::read_to_string(&output).is_ok_and(|s| !s.trim().is_empty()));
        let actual = fs::read_to_string(&output).unwrap();
        assert_ne!(actual.trim_end(), source.to_string_lossy());
        assert_ne!(actual.trim_end(), "/some/project");
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    #[test]
    fn drive_uri_directory_becomes_an_absolute_native_path() {
        let path =
            super::published_directory(std::path::Path::new("/C:/work/project one")).unwrap();
        assert_eq!(path, std::path::PathBuf::from("C:/work/project one"));
        assert!(path.is_absolute());
    }
}
