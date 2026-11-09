//! Explicit console identity and directory for a review session.
use super::{Source, State};
use crate::Window;
use std::path::PathBuf;

fn source(window: &mut Window, name: usize) -> Option<Source> {
    let pane = window.all_panes_mut().find(|pane| pane.name == name)?;
    let cwd = if pane.screen.working_directory_reported() {
        pane.screen.working_directory().map(ToOwned::to_owned)
    } else {
        pane.pty.working_directory()
    };
    Some(Source {
        pane: name,
        label: crate::pane_view::display_label(pane).into_owned(),
        cwd,
    })
}

fn pin(state: &mut State, source: Source) -> bool {
    if state.review.has_comments() {
        state.message = "Send or clear the review comments before opening another document.".into();
        state.open = true;
        return false;
    }
    if state
        .origin
        .as_ref()
        .is_none_or(|old| old.pane != source.pane)
    {
        state.loaded = None;
        state.requested = None;
        state.path.clear();
        state.message.clear();
        state.refresh_at = None;
        state.editing_path = true;
    }
    state.origin = Some(source);
    true
}

pub(crate) fn open(window: &mut Window, markdown: bool) {
    let Some(index) = window.active().where_the_focus_is() else {
        return;
    };
    let name = window.active().panes()[index].name;
    if !markdown {
        open_diff_for_pane(window, name);
        return;
    }
    let Some(source) = source(window, name) else {
        return;
    };
    if !pin(&mut window.viewer, source) {
        return;
    }
    window.viewer.open_markdown();
    if let Some(ctx) = &window.viewer.context {
        ctx.request_repaint();
    }
}

pub(crate) fn open_diff_for_pane(window: &mut Window, name: usize) {
    if window
        .viewer
        .origin
        .as_ref()
        .is_some_and(|source| source.pane == name)
        && window.viewer.review.has_comments()
    {
        window.viewer.open = true;
        window.viewer.keyboard = true;
    } else {
        let Some(source) = source(window, name) else {
            return;
        };
        let cwd = source.cwd.clone();
        if !pin(&mut window.viewer, source) {
            return;
        }
        window.viewer.open_diff();
        window.viewer.editing_path = false;
        if let Some(cwd) = cwd {
            window.viewer.path = cwd.to_string_lossy().into_owned();
            if let Some(ctx) = window.viewer.context.clone() {
                window.viewer.start(&ctx);
            }
        } else {
            window.viewer.path.clear();
            window.viewer.loaded = None;
            window.viewer.requested = None;
            window.viewer.refresh_at = None;
            window.viewer.message = "The working directory of this console is unavailable.".into();
        }
    }
    if let Some(ctx) = &window.viewer.context {
        ctx.request_repaint();
    }
}

pub(crate) fn open_markdown_target(
    window: &mut Window,
    pane: usize,
    path: PathBuf,
    _line: Option<u32>,
) {
    let Some(source) = source(window, pane) else {
        return;
    };
    if !pin(&mut window.viewer, source) {
        return;
    }
    window.viewer.open_markdown();
    window.viewer.path = path.to_string_lossy().into_owned();
    window.viewer.editing_path = false;
    if path.is_relative() {
        window.viewer.message = "This console has not published its working directory. Open an absolute Markdown path or enable OSC 7 in the shell.".into();
        window.viewer.loaded = None;
        window.viewer.requested = None;
        window.viewer.refresh_at = None;
    } else if let Some(ctx) = window.viewer.context.clone() {
        window.viewer.start(&ctx);
    }
    if let Some(ctx) = &window.viewer.context {
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{one_frame_in, window_of_tabs};
    use eframe::egui;
    #[cfg(target_os = "linux")]
    struct Repository(PathBuf);
    #[cfg(target_os = "linux")]
    impl Drop for Repository {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn review_uses_live_worktree_changes_after_a_stale_local_directory_report() {
        use std::{
            fs,
            process::Command,
            time::{Duration, Instant},
        };
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let root =
            std::env::temp_dir().join(format!("viewer-worktree-{}", u64::from_ne_bytes(nonce)));
        fs::create_dir(&root).unwrap();
        let _repository = Repository(root.clone());
        let main = root.join("main");
        fs::create_dir(&main).unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&main)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        fs::write(main.join("tracked"), "base\n").unwrap();
        git(&["add", "tracked"]);
        git(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "base",
        ]);
        let linked = root.join("linked");
        git(&["worktree", "add", "-qb", "linked", linked.to_str().unwrap()]);
        fs::write(main.join("tracked"), "MAIN_ONLY_CHANGE\n").unwrap();
        fs::write(linked.join("tracked"), "WORKTREE_ONLY_CHANGE\n").unwrap();
        let script = format!("cd '{}' && sleep 30", linked.display());
        let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
        let name = window.active().panes()[0].name;
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        let deadline = Instant::now() + Duration::from_secs(5);
        while window
            .pane_called(name)
            .unwrap()
            .pty
            .working_directory()
            .as_ref()
            != Some(&linked)
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        one_frame_in(&mut window, &ctx, area, vec![]);
        window
            .pane_called(name)
            .unwrap()
            .screen
            .advance(format!("\x1b]7;file://{}\x07", main.display()).as_bytes());
        open_diff_for_pane(&mut window, name);
        loop {
            window.viewer.poll(&ctx);
            if let Some(loaded) = &window.viewer.loaded {
                assert!(
                    loaded.text.contains("+WORKTREE_ONLY_CHANGE"),
                    "{}",
                    loaded.text
                );
                assert!(!loaded.text.contains("MAIN_ONLY_CHANGE"), "{}", loaded.text);
                assert!(!loaded.diff.as_ref().unwrap().files.is_empty());
                break;
            }
            assert!(Instant::now() < deadline, "{}", window.viewer.message);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn review_opens_live_console_repository_without_shell_directory_reports() {
        use std::{
            fs,
            process::Command,
            time::{Duration, Instant},
        };
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path =
            std::env::temp_dir().join(format!("viewer-pane-cwd-{}", u64::from_ne_bytes(nonce)));
        fs::create_dir(&path).unwrap();
        let _repository = Repository(path.clone());
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        fs::write(path.join("from-this-console.txt"), "new file").unwrap();
        let script = format!("cd '{}' && sleep 30", path.display());
        let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
        let name = window.active().panes()[0].name;
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        one_frame_in(&mut window, &ctx, area, vec![]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while source(&mut window, name).and_then(|s| s.cwd).as_ref() != Some(&path) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !window
                .pane_called(name)
                .unwrap()
                .screen
                .working_directory_reported()
        );
        open_diff_for_pane(&mut window, name);
        loop {
            window.viewer.poll(&ctx);
            if let Some(doc) = window.viewer.loaded.as_ref().and_then(|l| l.diff.as_ref()) {
                assert!(
                    doc.files
                        .iter()
                        .any(|file| file.path == std::path::Path::new("from-this-console.txt"))
                );
                break;
            }
            assert!(Instant::now() < deadline, "{}", window.viewer.message);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!window.viewer.editing_path);
    }

    #[test]
    fn opening_a_path_pins_its_console_and_preserves_the_grid() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        one_frame_in(&mut window, &ctx, area, vec![]);
        let before: Vec<_> = window.all_panes().map(|p| (p.name, p.measured)).collect();
        let name = before[1].0;
        open_markdown_target(&mut window, name, PathBuf::from("relative.md"), None);
        assert_eq!(window.viewer.origin.as_ref().unwrap().pane, name);
        assert!(window.viewer.message.contains("working directory"));
        assert!(window.viewer.requested.is_none());
        one_frame_in(&mut window, &ctx, area, vec![]);
        assert_eq!(
            window
                .all_panes()
                .map(|p| (p.name, p.measured))
                .collect::<Vec<_>>(),
            before
        );
        window.viewer.width = 900.0;
        one_frame_in(&mut window, &ctx, area, vec![]);
        assert_eq!(
            window
                .all_panes()
                .map(|p| (p.name, p.measured))
                .collect::<Vec<_>>(),
            before
        );
    }
    #[test]
    fn changing_console_clears_previous_document() {
        let mut state = State::default();
        assert!(pin(
            &mut state,
            Source {
                pane: 1,
                label: "first".into(),
                cwd: None
            }
        ));
        state.open_markdown();
        state.path = "/tmp/old.md".into();
        state.requested = Some((PathBuf::from("/tmp/old.md"), super::super::Kind::Markdown));
        state.loaded = Some(super::super::Loaded {
            text: "old".into(),
            markdown: Some(super::super::markdown::MarkdownDocument::parse("old").unwrap()),
            diff: None,
        });
        assert!(pin(
            &mut state,
            Source {
                pane: 2,
                label: "second".into(),
                cwd: None
            }
        ));
        state.open_markdown();
        assert_eq!(state.origin.as_ref().unwrap().pane, 2);
        assert!(state.loaded.is_none() && state.requested.is_none() && state.path.is_empty());
    }
}
