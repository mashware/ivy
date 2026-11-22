//! Explicit console identity and directory for a review session.
use super::{Source, State};
use crate::Window;
use crate::sidebar::Tool;
use std::path::PathBuf;

fn source(window: &mut Window, name: usize) -> Option<Source> {
    let pane = window.all_panes_mut().find(|pane| pane.name == name)?;
    let cwd =
        if pane.screen.working_directory_reported() && pane.screen.working_directory().is_none() {
            // A remote/invalid report must not become the local ssh process's directory.
            None
        } else {
            // Native discovery sees a cd before the shell publishes its next local report.
            pane.pty
                .working_directory()
                .or_else(|| pane.screen.working_directory().map(ToOwned::to_owned))
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

pub(super) fn initialize(window: &mut Window, markdown: bool) {
    let Some(index) = window.active().where_the_focus_is() else {
        return;
    };
    let name = window.active().panes()[index].name;
    if markdown {
        if let Some(source) = source(window, name) {
            window.viewer.origin = Some(source);
        }
    } else {
        open_diff_for_pane(window, name);
    }
}

pub(crate) fn open_diff_for_pane(window: &mut Window, name: usize) {
    super::select_state(window, false);
    if window
        .viewer
        .origin
        .as_ref()
        .is_some_and(|source| source.pane == name)
        && window.viewer.review.has_comments()
    {
        crate::sidebar::select(window, crate::sidebar::Tool::Git);
        window.viewer.open = true;
        window.viewer.keyboard = true;
    } else {
        let Some(source) = source(window, name) else {
            return;
        };
        let cwd = source.cwd.clone();
        crate::sidebar::select(window, crate::sidebar::Tool::Git);
        if !pin(&mut window.viewer, source) {
            window.viewer.keyboard = true;
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

pub(crate) fn open_document_target(
    window: &mut Window,
    pane: usize,
    path: PathBuf,
    _line: Option<u32>,
) -> bool {
    super::select_state(window, true);
    let Some(source) = source(window, pane) else {
        return false;
    };
    if !pin(&mut window.viewer, source) {
        return false;
    }
    crate::sidebar::select(window, crate::sidebar::Tool::Document);
    // Files follows the focused console, so the folder a relative path is counted from is the
    // one Files showed when the file was opened, not whatever it shows later.
    window.viewer.code.header.root = crate::explorer::root_of(window, pane);
    window.viewer.open(super::Kind::of_document(&path));
    window.viewer.path = path.to_string_lossy().into_owned();
    window.viewer.editing_path = false;
    if path.is_relative() {
        window.viewer.message = "This console has not published its working directory. Open an absolute path or enable OSC 7 in the shell.".into();
        window.viewer.loaded = None;
        window.viewer.requested = None;
        window.viewer.refresh_at = None;
    } else if let Some(ctx) = window.viewer.context.clone() {
        window.viewer.start(&ctx);
    }
    if let Some(ctx) = &window.viewer.context {
        ctx.request_repaint();
    }
    true
}

/// Forgets what `state` showed, keeping which tab it is, the window's context and the drawer's
/// own input flags: the image tab takes its input through the document state's, so closing the
/// document beside it is not closing the drawer. Moving to another tab is left to suspend them.
pub(super) fn forget(state: &mut super::State) {
    *state = super::State {
        context: state.context.take(),
        tool_markdown: state.tool_markdown,
        open: state.open,
        keyboard: state.keyboard,
        space: state.space,
        held: state.held,
        blocked_frame: state.blocked_frame,
        ..super::State::default()
    };
}

/// Shows the drawer's image tab, which takes the keyboard. The drawer's open, keyboard and
/// gesture flags stay the document state's, so the image takes input exactly as a document would.
pub(crate) fn show_image(window: &mut Window) {
    super::select_state(window, true);
    crate::sidebar::select(window, crate::sidebar::Tool::Image);
    window.viewer.open = true;
    window.viewer.keyboard = true;
    if let Some(ctx) = &window.viewer.context {
        ctx.request_repaint();
    }
}

/// Whether the image of the console `pane` carries marks or notes not yet sent, which would be
/// lost: if so it is shown, says so, and keeps them, as unsent review comments keep their review.
/// Another console's marks are not its business.
pub(super) fn keeps_marks(window: &mut Window, pane: usize) -> bool {
    let Some(slot) = super::image::slot_of(&mut window.tabs, pane).filter(|s| s.has_marks()) else {
        return false;
    };
    slot.add(UNSENT_MARKS);
    show_image(window);
    true
}

const UNSENT_MARKS: &str =
    "Send or clear the marks on the image before closing it or opening another image.";

/// Why a document did not open: the review's unsent comments.
pub(crate) const KEPT_DOCUMENT: &str =
    "Send or clear the review comments before opening another document.";

/// Whether the drawer is on its image tab.
pub(super) fn showing_image(window: &Window) -> bool {
    crate::sidebar::active(window) == Some(crate::sidebar::Tool::Image)
}

/// The focused console, while the image tab is on and that console has no image.
pub(super) fn empty_image_tab(window: &Window) -> Option<&crate::Pane> {
    if !showing_image(window) || super::image::focused(window).is_some_and(|s| s.shown()) {
        return None;
    }
    super::image::focused_pane(window)
}

/// The console the shown document was opened from.
pub(super) fn shown_origin(window: &Window) -> Option<&super::Source> {
    if showing_image(window) {
        super::image::focused(window)?.origin()
    } else {
        window.viewer.origin.as_ref()
    }
}

/// Why an image was not opened.
#[derive(Debug, PartialEq)]
pub(crate) enum NotOpened {
    /// The console it was printed in is gone.
    Gone,
    /// The console's image has marks not yet sent; the drawer shows it and says so.
    Marks,
}

impl std::fmt::Display for NotOpened {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Self::Gone => "The console this image belongs to has closed.",
            Self::Marks => UNSENT_MARKS,
        })
    }
}

/// Opens an image in the slot of the console it was printed in, focuses that console and shows
/// the drawer's image tab. A relative path is read from that console's directory, as the review
/// reads its repository. Refused when that console is gone or its image keeps unsent marks; the
/// refusals of the image itself, a file that does not decode among them, are painted in the drawer.
pub(crate) fn open_image_target(
    window: &mut Window,
    pane: usize,
    path: PathBuf,
) -> Result<(), NotOpened> {
    let Some(source) = source(window, pane) else {
        return Err(NotOpened::Gone);
    };
    focus(window, pane);
    if keeps_marks(window, pane) {
        return Err(NotOpened::Marks);
    }
    let path = match &source.cwd {
        Some(cwd) if path.is_relative() => cwd.join(path),
        _ => path,
    };
    let Some(slot) = super::image::slot_of(&mut window.tabs, pane) else {
        return Err(NotOpened::Gone);
    };
    window.image.open(slot, source, path, false);
    show_image(window);
    if let Some(ctx) = window.viewer.context.clone()
        && let Some(slot) = super::image::slot_of(&mut window.tabs, pane)
    {
        slot.poll(&ctx);
    }
    Ok(())
}

/// What an agent's open answers when the drawer shows the image: it opened on it, or its Image
/// tab was already on show for that console (D120). The answer comes before the worker has
/// decoded the file, and a truncated or corrupt PNG or JPEG begins like a whole one, so it says
/// where a file that cannot be shown is explained instead of promising pixels (owner, D120).
pub(crate) const SHOWN_TO_THE_PERSON: &str = "opened and shown in the drawer of this console, which the person is working in; if the drawer cannot show it, it says why there";

/// What an agent's open answers when nothing moved and the mark says it (D120). Its last clause
/// is the one `SHOWN_TO_THE_PERSON` ends with, for the same reason.
pub(crate) const MARKED_FOR_THE_PERSON: &str = "opened; the person is not looking at this console's drawer, so its tab and header mark a new image until they do; if the drawer cannot show it, it says why there";

/// Why an agent's open was refused over the person's unsent marks, worded for the agent.
pub(crate) const MARKS_NOT_SENT: &str = "the image open in this console has marks the person has not sent; nothing was opened. Ask them to send or clear the marks, then call image_open again";

/// The drawer's geometry, for telling whether an opened drawer has room in this frame.
pub(crate) struct Room<'a> {
    pub(crate) ctx: &'a eframe::egui::Context,
    pub(crate) whole_window: eframe::egui::Rect,
    pub(crate) metrics: &'a crate::painter::CellMetrics,
}

/// Opens `path` in the slot of the console `pane`, for that console's own agent (D120).
///
/// The person is not moved: where the Image tab already shows that console the new image simply
/// takes its place, and otherwise the drawer opens on it only where the person is working in that
/// console and nothing else holds the screen or the keys. The keyboard stays where it was. Refused
/// over that console's unsent marks, with nothing shown and nothing said in the drawer, because the
/// agent is the one asking. The file itself is not read here: the worker that decodes it refuses a
/// path through a link first, which a person's own open is spared.
pub(crate) fn open_for_an_agent(
    window: &mut Window,
    pane: usize,
    path: PathBuf,
    room: &Room<'_>,
) -> Result<&'static str, String> {
    let gone = || "the console that asked is gone".to_owned();
    let source = source(window, pane).ok_or_else(gone)?;
    let slot = super::image::slot_of(&mut window.tabs, pane).ok_or_else(gone)?;
    if slot.has_marks() {
        return Err(MARKS_NOT_SENT.to_owned());
    }
    window.image.open(slot, source, path, true);
    // Set even where the drawer shows it at once: one rule, and the frame that paints it takes
    // the mark down.
    slot.mark_unseen();
    if !in_front(window, pane, room.ctx) {
        return Ok(MARKED_FOR_THE_PERSON);
    }
    // The Image tab on show already shows this console's image: nothing has to move for the new
    // one to be seen, the keys and a maximized drawer included.
    if !showing_image(window) {
        // Room is asked before switching: a window too short for the drawer keeps the consoles
        // whole, and switching anyway would replace the person's Document or Changes for nothing.
        let room_for_it =
            crate::tab_bar::drawer_has_room(window, room.whole_window, room.metrics, Tool::Image);
        if !free_to_switch(window) || !room_for_it {
            return Ok(MARKED_FOR_THE_PERSON);
        }
        super::select_state(window, true);
        crate::sidebar::select(window, Tool::Image);
        window.viewer.open = true;
    }
    room.ctx.request_repaint();
    // Selected is not shown: an Image tab that was already selected in a window too short for the
    // drawer is still out of sight.
    let geometry = crate::tab_bar::geometry(window, room.whole_window, room.metrics);
    Ok(if geometry.dock.is_some() {
        SHOWN_TO_THE_PERSON
    } else {
        MARKED_FOR_THE_PERSON
    })
}

/// Whether the console `pane` is the focused one of the tab on show, with nothing over the drawer
/// that would hide it. The title bar's modals are asked through `chrome::is_modal`, the same rule
/// that dims the consoles, so its menu counts as well as its dialogs.
fn in_front(window: &Window, pane: usize, ctx: &eframe::egui::Context) -> bool {
    let tab = window.active();
    tab.focus == pane
        && tab.panes().iter().any(|p| p.name == pane)
        && !window.settings_editor.is_open()
        && !window.pane_view.is_modal()
        && !crate::chrome::is_modal(window, ctx)
}

/// Whether the drawer may be switched to the Image tab: switching takes away what the Board or a
/// maximized drawer shows, and the keys from the drawer or Files if they hold them (D120).
fn free_to_switch(window: &Window) -> bool {
    !matches!(crate::sidebar::active(window), Some(Tool::Board))
        && !window.sidebar.maximized
        && !window.viewer.keyboard
        && !window.viewer_other.keyboard
        && !window.explorer.holds_keys()
}

/// Gives the console `pane` the keys, showing its tab: the image tab shows the focused console's.
pub(super) fn focus(window: &mut Window, pane: usize) {
    if let Some(tab) = window
        .tabs
        .iter()
        .position(|tab| tab.panes().iter().any(|p| p.name == pane))
    {
        window.active = tab;
        window.tabs[tab].focus = pane;
    }
}

/// Close one tab of the drawer: what it showed is forgotten, the drawer moves to another tab the
/// strip shows, preferring Document, then Changes, then Image, and with no other tab it closes. A review
/// with unsent comments and an image with unsent marks are kept and say why.
pub(super) fn close_tab(window: &mut Window, closed: Tool, ctx: &eframe::egui::Context) {
    let tabs = super::drawer::tabs(window);
    let next = [
        (Tool::Document, tabs.document.is_some()),
        (Tool::Git, tabs.changes.is_some()),
        (Tool::Image, tabs.image.is_some()),
    ]
    .into_iter()
    .find(|&(tool, shown)| shown && tool != closed)
    .map(|(tool, _)| tool);
    if closed == Tool::Image {
        let focus = window.active().focus;
        if keeps_marks(window, focus) {
            return;
        }
        if let Some(slot) = super::image::slot_of(&mut window.tabs, focus)
            && slot.clear()
        {
            ctx.request_repaint();
        }
    } else {
        let markdown = closed == Tool::Document;
        let state = if window.viewer.tool_markdown.unwrap_or(true) == markdown {
            &mut window.viewer
        } else {
            &mut window.viewer_other
        };
        if state.review.has_comments() {
            state.message = "Send or clear the review comments before closing the review.".into();
            return;
        }
        forget(state);
    }
    if crate::sidebar::active(window) == Some(closed) {
        match next {
            Some(Tool::Image) => show_image(window),
            Some(tool) => super::activate(window, tool == Tool::Document),
            None => crate::sidebar::close(window, ctx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{one_frame_in, window_of_tabs};
    use eframe::egui;

    #[test]
    fn markdown_and_git_banks_keep_independent_paths_and_loaded_content() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        window.viewer.path = "/docs/guide.md".into();
        window.viewer.loaded = Some(super::super::Loaded {
            text: "markdown body".into(),
            markdown: Some(
                crate::viewer::markdown::MarkdownDocument::parse("markdown body").unwrap(),
            ),
            code: None,
            diff: None,
        });

        super::super::select_state(&mut window, false);
        window.viewer.kind = super::super::Kind::AgainstBase;
        window.viewer.path = "/repo".into();
        window.viewer.base = "release".into();
        window.viewer.loaded = Some(super::super::Loaded {
            text: "git diff".into(),
            markdown: None,
            code: None,
            diff: None,
        });

        super::super::select_state(&mut window, true);
        assert_eq!(window.viewer.path, "/docs/guide.md");
        assert_eq!(window.viewer.loaded.as_ref().unwrap().text, "markdown body");
        super::super::select_state(&mut window, false);
        assert_eq!(window.viewer.path, "/repo");
        assert_eq!(window.viewer.base, "release");
        assert_eq!(window.viewer.loaded.as_ref().unwrap().text, "git diff");
    }
    #[cfg(target_os = "linux")]
    struct Repository(PathBuf);
    #[cfg(target_os = "linux")]
    impl Drop for Repository {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    crate::windows_unix_omission!(
        review_uses_live_worktree_changes_after_a_stale_local_directory_report,
        "unix-only: exercises live Linux worktree state and shell directory reports"
    );

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

    crate::windows_unix_omission!(
        review_opens_live_console_repository_without_shell_directory_reports,
        "unix-only: exercises live Linux worktree state and shell directory reports"
    );

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
    fn opening_a_path_pins_its_console_without_changing_pane_identity() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        one_frame_in(&mut window, &ctx, area, vec![]);
        let names: Vec<_> = window.all_panes().map(|p| p.name).collect();
        let name = names[1];
        open_document_target(&mut window, name, PathBuf::from("relative.md"), None);
        assert_eq!(window.viewer.origin.as_ref().unwrap().pane, name);
        assert!(window.viewer.message.contains("working directory"));
        assert!(window.viewer.requested.is_none());
        one_frame_in(&mut window, &ctx, area, vec![]);
        assert_eq!(
            window.all_panes().map(|p| p.name).collect::<Vec<_>>(),
            names
        );
    }
    /// The image is pinned to the console it was printed in, and it and a Markdown document no
    /// longer push each other out of the drawer: each keeps a tab of its own.
    #[test]
    fn an_image_and_a_markdown_document_keep_tabs_of_their_own() {
        use crate::sidebar::{Tool, active};
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        one_frame_in(&mut window, &ctx, area, vec![]);
        let names: Vec<_> = window.all_panes().map(|p| p.name).collect();
        let strip = |window: &Window| {
            let tabs = super::super::drawer::tabs(window);
            (tabs.document, tabs.image)
        };
        // Absolute on every platform: a bare `/docs/guide.md` has no drive on Windows, so it is
        // read as relative to the console and the document never opens.
        let root = std::env::temp_dir();
        let guide = root.join("guide.md");
        open_document_target(&mut window, names[0], guide.clone(), None);
        open_image_target(&mut window, names[1], root.join("screen.png")).unwrap();
        assert_eq!(
            super::super::image::focused(&window)
                .unwrap()
                .origin()
                .unwrap()
                .pane,
            names[1]
        );
        assert_eq!(active(&window), Some(Tool::Image));
        assert_eq!(
            strip(&window),
            (Some("guide.md".into()), Some("screen.png".into()))
        );
        assert_eq!(window.viewer.path, guide.to_string_lossy());
        open_document_target(&mut window, names[0], root.join("other.md"), None);
        assert_eq!(active(&window), Some(Tool::Document));
        assert_eq!(
            strip(&window),
            (Some("other.md".into()), Some("screen.png".into()))
        );
        assert_eq!(
            open_image_target(&mut window, 9999, root.join("x.png")),
            Err(NotOpened::Gone)
        );
        assert_eq!(strip(&window).1, Some("screen.png".into()));
    }

    /// A relative image path is read from the console it was printed in, as its review is; a
    /// console whose directory is remote has none to read it from, and says so.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_relative_image_path_is_read_from_its_consoles_directory() {
        use std::time::{Duration, Instant};
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path =
            std::env::temp_dir().join(format!("viewer-image-cwd-{}", u64::from_ne_bytes(nonce)));
        std::fs::create_dir(&path).unwrap();
        let _folder = Repository(path.clone());
        ::image::RgbaImage::new(24, 12)
            .save_with_format(path.join("shot.png"), ::image::ImageFormat::Png)
            .unwrap();
        let script = format!("cd '{}' && sleep 30", path.display());
        let mut window = window_of_tabs([[script.as_str(), "sleep 30"]]);
        let names: Vec<_> = window.active().panes().iter().map(|p| p.name).collect();
        let ctx = egui::Context::default();
        let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0));
        one_frame_in(&mut window, &ctx, area, vec![]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while source(&mut window, names[0]).and_then(|s| s.cwd).as_ref() != Some(&path) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        open_image_target(&mut window, names[0], PathBuf::from("shot.png")).unwrap();
        assert_eq!(
            crate::viewer::shown_image(&window),
            Some((names[0], path.join("shot.png")))
        );
        while !super::super::image::focused(&window).unwrap().loaded() {
            one_frame_in(&mut window, &ctx, area, vec![]);
            assert!(
                Instant::now() < deadline,
                "{:?}",
                super::super::image::focused(&window).unwrap().message()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        window
            .pane_called(names[1])
            .unwrap()
            .screen
            .advance(b"\x1b]7;file://elsewhere.invalid/home\x07");
        assert_eq!(source(&mut window, names[1]).unwrap().cwd, None);
        open_image_target(&mut window, names[1], PathBuf::from("shot.png")).unwrap();
        assert!(
            super::super::image::focused(&window)
                .unwrap()
                .message()
                .is_some_and(|why| why.contains("working directory"))
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
            code: None,
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
