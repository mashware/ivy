//! Paste: a screenshot the desktop's own tool left on the clipboard, written as a PNG into a
//! console's own folder and opened in the drawer pinned to that console (D117).
use crate::clipboard::{Pasted, Refused};
use crate::viewer::load::{DECODE_LIMIT, IMAGE_LIMIT, Job, spawn, take};
use crate::viewer::origin;
use eframe::egui;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// What the empty viewer says. The chord is the platform's: Cmd on macOS.
pub(crate) const HINT: &str = if cfg!(target_os = "macos") {
    "Copy a screenshot and press Paste screenshot again, or press Cmd+V here."
} else {
    "Copy a screenshot and press Paste screenshot again, or press Ctrl+V here."
};
pub(crate) const NO_IMAGE: &str = "The clipboard holds no image.";
pub(crate) const NO_FOLDER: &str = "This console has no folder to keep the screenshot in.";
pub(crate) const EXPIRED: &str = "The clipboard did not answer in time.";
const BUSY: &str = "A paste is already on its way.";
const CLOSED: &str = "This console has closed.";

/// Who started a paste: the header's button opens the viewer for its console; the chord over a
/// picture already shown only says what went wrong, and leaves the picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum By {
    Button,
    Key,
}

/// A paste from the read to the opened image; one at a time.
#[derive(Default)]
pub(crate) struct Pasting {
    /// The paste chord, taken by the viewer while it painted; the input pass asks for it.
    pub(super) key: bool,
    /// Whether the chord that asked is still held: egui-winit turns every repeat of a held chord
    /// into another text paste, repeat flag dropped, so the end of a hold is when its modifier
    /// (or the Paste key) is let go.
    held: bool,
    /// Who started the clipboard read outstanding; the clipboard answers every read it took.
    asked: Option<By>,
    writing: Option<Writing>,
}

/// A screenshot being encoded and written into a console's folder.
struct Writing {
    pane: usize,
    by: By,
    folder: PathBuf,
    job: Job<PathBuf>,
}

impl Pasting {
    fn busy(&self) -> bool {
        self.asked.is_some() || self.writing.is_some()
    }

    pub(crate) fn pending(&self) -> bool {
        self.key || self.writing.is_some()
    }
}

/// What the empty viewer says, when no image was pasted into it.
pub(crate) fn empty(viewer: &super::Viewer) -> Option<&str> {
    match &viewer.document.as_ref()?.load {
        super::Load::Empty(text) => Some(text),
        _ => None,
    }
}

/// Replaces whatever image was shown with an empty viewer for `origin`'s console, saying `text`.
pub(in crate::viewer) fn pin(
    viewer: &mut super::Viewer,
    origin: crate::viewer::Source,
    text: String,
) {
    viewer.start(origin, PathBuf::new(), super::Load::Empty(text));
}

/// The paste chord is the viewer's only while the drawer holds the keys — a console that holds
/// them leaves the chord in the input, its own (`a_paste_into_a_console_is_not_taken_by_the_viewer`)
/// — and no note is being typed, which keeps it for its own text. Taken here, it is asked for in
/// the next input pass, once per hold.
pub(super) fn take_chord(viewer: &mut super::Viewer, ui: &mut egui::Ui, keys: bool) {
    let free = keys && !viewer.send.busy() && !ui.ctx().text_edit_focused();
    if free && ui.input_mut(take_key) {
        if !viewer.paste.held {
            viewer.paste.key = true;
            ui.ctx().request_repaint();
        }
        viewer.paste.held = true;
    } else if ui.input(|i| !i.modifiers.command && !i.key_down(egui::Key::Paste)) {
        viewer.paste.held = false;
    }
}

/// Takes the paste chord out of `input`: as the text paste egui-winit makes of every paste chord
/// when the clipboard holds text, or as the key it lets through — the vendored copy does — when it
/// holds none, which is when a screenshot is there: Ctrl+V or Ctrl+Shift+V (Cmd on macOS), the
/// Paste key, and Shift+Insert on Windows, where Win+Shift+S leaves only an image. A held chord's
/// repeats are not new pastes.
fn take_key(input: &mut egui::InputState) -> bool {
    let before = input.events.len();
    input.events.retain(|event| {
        let egui::Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers: m,
            ..
        } = event
        else {
            return !matches!(event, egui::Event::Paste(_));
        };
        let insert = cfg!(windows) && m.shift && !m.ctrl && !m.alt && !m.command;
        !match key {
            egui::Key::V => m.command && !m.alt,
            egui::Key::Paste => true,
            egui::Key::Insert => insert,
            _ => false,
        }
    });
    input.events.len() != before
}

/// A paste for the console `pane`. Refused, before the clipboard is read, when the image shown
/// keeps unsent marks, another paste is on its way, the console has closed, it has no folder to
/// keep the file in, or the clipboard is busy or unavailable.
pub(crate) fn ask(window: &mut crate::Window, pane: usize, by: By) {
    if origin::keeps_marks(window) {
        return;
    }
    // Beside what is shown: pinning another console's empty viewer would be undone the moment
    // the paste on its way answers for its own.
    if window.image.paste.busy() {
        window.image.say(BUSY);
        return origin::reveal_image(window);
    }
    let Some(console) = window.all_panes().find(|p| p.name == pane) else {
        return closed(&mut window.image);
    };
    if console.panel.is_none() {
        return tell(window, pane, by, Said::Final(NO_FOLDER));
    }
    match window.clipboard.request_image(pane, Instant::now()) {
        Ok(()) => window.image.paste.asked = Some(by),
        Err(Refused::Busy) => tell(window, pane, by, Said::Retry(Refused::Busy.says())),
        Err(why) => tell(window, pane, by, Said::Final(why.says())),
    }
}

/// Says that the viewer's console has closed where the drawer paints it: the empty viewer's own
/// text, a notice beside a picture, or — for an image that failed or is still loading, which paint
/// no notice — standard error alone.
fn closed(viewer: &mut super::Viewer) {
    match viewer.document.as_mut().map(|d| &mut d.load) {
        Some(super::Load::Empty(text)) => *text = CLOSED.to_owned(),
        Some(super::Load::Ready { .. }) => viewer.say(CLOSED),
        _ => eprintln!("A paste was asked for a console that has closed"),
    }
}

/// What the clipboard answered for the console `pane`.
pub(crate) fn received(
    window: &mut crate::Window,
    pane: usize,
    pasted: Pasted,
    ctx: &egui::Context,
) {
    // Only `ask` starts an image read, and it records who did; the clipboard answers each read it
    // took once (`an_expired_read_is_not_answered_again_when_the_worker_stops`).
    let by = window.image.paste.asked.take();
    debug_assert!(by.is_some(), "an image read nobody asked for");
    let by = by.unwrap_or(By::Button);
    match pasted {
        Pasted::Image(image) => {
            if let Err(why) = write(window, pane, by, image, ctx) {
                tell(window, pane, by, Said::Final(&why));
            }
        }
        Pasted::Absent if by == By::Button => tell(window, pane, by, Said::Hint),
        Pasted::Absent => tell(window, pane, by, Said::Retry(NO_IMAGE)),
        Pasted::Failed(why) => {
            let said = format!("The clipboard could not be read: {why}.");
            tell(window, pane, by, Said::Retry(&said));
        }
        Pasted::Unavailable(why) => {
            let said = format!("The clipboard is unavailable ({why}); restart Ivy to paste.");
            tell(window, pane, by, Said::Final(&said));
        }
        Pasted::Expired => tell(window, pane, by, Said::Retry(EXPIRED)),
    }
}

/// Refuses an image the viewer would refuse anyway, before a file is left for it, then starts the
/// worker that encodes and writes it.
fn write(
    window: &mut crate::Window,
    pane: usize,
    by: By,
    image: image::RgbaImage,
    ctx: &egui::Context,
) -> Result<(), String> {
    let console = window.all_panes().find(|p| p.name == pane).ok_or(CLOSED)?;
    let folder = console.panel.as_ref().map(|dir| dir.path().to_owned());
    let folder = folder.ok_or(NO_FOLDER)?;
    if decodes_too_large(image.width(), image.height()) {
        return Err(too_large(DECODE_LIMIT));
    }
    let into = folder.clone();
    let job = spawn(ctx.clone(), move || {
        let png = encode(&image, IMAGE_LIMIT)?;
        write_new(&into, &png)
    })?;
    window.image.paste.writing = Some(Writing {
        pane,
        by,
        folder,
        job,
    });
    Ok(())
}

/// Whether the viewer's decode would refuse an image this size: it decodes to RGBA, four bytes a
/// pixel, under the same limit.
fn decodes_too_large(width: u32, height: u32) -> bool {
    u64::from(width) * u64::from(height) * 4 > DECODE_LIMIT
}

/// The screenshot as a PNG, refused over `limit` bytes as an opened image's file would be.
fn encode(image: &image::RgbaImage, limit: usize) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    image
        .write_with_encoder(image::codecs::png::PngEncoder::new(&mut png))
        .map_err(|e| format!("The screenshot could not be encoded: {e}"))?;
    if png.len() > limit {
        return Err(too_large(limit as u64));
    }
    Ok(png)
}

fn too_large(limit: u64) -> String {
    format!(
        "The clipboard image is too large to open (limit: {} MiB).",
        limit / (1024 * 1024)
    )
}

/// Writes `png` into `folder` under the first `pasted-<n>.png` nothing there takes, a dangling
/// link included: `create_new` refuses them all, and a name taken between the look and the create
/// moves on to the next. On Unix readable by its owner only, as Send's copies are (D113).
fn write_new(folder: &Path, png: &[u8]) -> Result<PathBuf, String> {
    for n in 1.. {
        let path = folder.join(format!("pasted-{n}.png"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(format!(
                    "The screenshot could not be created at {}: {e}",
                    path.display()
                ));
            }
        };
        return match file.write_all(png).and_then(|()| file.flush()) {
            Ok(()) => Ok(path),
            Err(e) => {
                drop(file);
                let left = match std::fs::remove_file(&path) {
                    Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                        format!("; the half-written file could not be removed: {error}")
                    }
                    _ => String::new(),
                };
                Err(format!(
                    "The screenshot could not be written at {}: {e}{left}",
                    path.display()
                ))
            }
        };
    }
    unreachable!("some number is free")
}

/// Takes a written screenshot to the drawer, from the window's input pass, and asks for the paste
/// the viewer's chord asked for, for the console the viewer is pinned to.
pub(crate) fn poll(window: &mut crate::Window, ctx: &egui::Context) {
    if std::mem::take(&mut window.image.paste.key)
        && let Some(pane) = window.image.origin().map(|o| o.pane)
    {
        ask(window, pane, By::Key);
    }
    let Some(writing) = &window.image.paste.writing else {
        return;
    };
    let Some(written) = take(&writing.job) else {
        ctx.request_repaint_after(Duration::from_millis(50));
        return;
    };
    let Writing {
        pane, by, folder, ..
    } = window.image.paste.writing.take().expect("a paste");
    let gone = !window.all_panes().any(|p| p.name == pane);
    let path = match written {
        Ok(path) if gone => return forsake(&path),
        Ok(path) => path,
        Err(why) if gone => return forsake_folder(&folder, Some(&why)),
        Err(why) => return tell(window, pane, by, Said::Final(&why)),
    };
    // The console removes it when it closes, with Send's copies.
    if let Some(console) = window.all_panes_mut().find(|p| p.name == pane) {
        console.annotated.record(path.clone());
    }
    if window.image.has_marks() {
        // Marks drawn on the image shown while the screenshot was written: they are kept, and the
        // file stays the console's until it closes. Asked before opening, which would take the
        // keyboard and replace a notice not yet read.
        let said = format!(
            "The screenshot was saved to {}; send or clear the marks on this image to open it.",
            path.display()
        );
        tell(window, pane, by, Said::Final(&said));
    } else {
        // The console is open (`gone` above) and nothing is marked: the image opens.
        let opened = origin::open_image_target(window, pane, path);
        debug_assert!(opened.is_ok(), "{opened:?}");
    }
    ctx.request_repaint();
}

/// A console that closed while its screenshot was written could not remove its folder, which held
/// the file: both go now.
fn forsake(path: &Path) {
    if let Err(error) = std::fs::remove_file(path) {
        eprintln!("the pasted screenshot {} remains ({error})", path.display());
        return;
    }
    if let Some(folder) = path.parent() {
        forsake_folder(folder, None);
    }
}

/// The folder of a console that closed while its screenshot was being written goes if nothing
/// else is in it; a write that failed meanwhile is said on standard error, where nobody else will.
fn forsake_folder(folder: &Path, failed: Option<&str>) {
    if let Some(why) = failed {
        eprintln!("the pasted screenshot for a closed console was not written: {why}");
    }
    if std::fs::remove_dir(folder).is_ok() {
        let after = if failed.is_some() {
            "its failed paste"
        } else {
            "its pasted screenshot was gone"
        };
        eprintln!(
            "the panel directory {} was removed after {after}",
            folder.display()
        );
    }
}

/// What a paste that opened nothing says: the hint alone, something a retry can mend followed by
/// the hint, or something it cannot, alone.
enum Said<'a> {
    Hint,
    Retry(&'a str),
    Final(&'a str),
}

/// Says what became of a paste. Over unsent marks it is said beside them, after any notice not yet
/// read; over an image a key's paste found shown or still loading, it replaces the notice. Both
/// bring the drawer's document tab into view without taking the keyboard, which may be a
/// console's by now. Anything else opens the viewer for that console, empty.
fn tell(window: &mut crate::Window, pane: usize, by: By, said: Said) {
    let (alone, text) = match said {
        Said::Hint => (None, HINT.to_owned()),
        Said::Retry(said) => (Some(said), format!("{said} {HINT}")),
        Said::Final(said) => (Some(said), said.to_owned()),
    };
    let alone = alone.unwrap_or(NO_IMAGE).to_owned();
    // Loading: shown with its notice once decoded. Failed: nothing to keep.
    let image = &window.image;
    let kept = image.shown() && empty(image).is_none() && image.message().is_none();
    if window.image.has_marks() {
        let notice = &mut window.image.notice;
        if !notice.as_deref().is_some_and(|n| n.ends_with(&alone)) {
            super::send::join(notice, alone);
        }
        return origin::reveal_image(window);
    }
    if by == By::Key && kept {
        window.image.say(alone);
        return origin::reveal_image(window);
    }
    if !origin::pin_empty_image(window, pane, text) {
        closed(&mut window.image);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC8: the viewer's own decode limit, at its edge: 8192 × 8192 RGBA is exactly 256 MiB.
    #[test]
    fn an_image_the_decode_would_refuse_is_refused_before_a_file_is_written() {
        assert!(!decodes_too_large(8192, 8192));
        assert!(decodes_too_large(8192, 8193));
        assert!(!decodes_too_large(0, u32::MAX));
        assert_eq!(
            too_large(DECODE_LIMIT),
            "The clipboard image is too large to open (limit: 256 MiB)."
        );
    }

    /// AC8: a PNG over the file limit is refused, one at it is kept.
    #[test]
    fn a_png_over_the_file_limit_is_refused() {
        let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([9, 9, 9, 255]));
        let size = encode(&image, usize::MAX).unwrap().len();
        assert_eq!(encode(&image, size).unwrap().len(), size);
        assert_eq!(encode(&image, size - 1), Err(too_large(size as u64 - 1)));
    }

    /// AC1: the first name nothing takes, a dangling link included, and nobody's file touched.
    #[cfg(unix)]
    #[test]
    fn the_screenshot_takes_the_first_free_name_and_touches_nothing_else() {
        let dir = panel_contract::PanelDir::create().unwrap();
        let folder = dir.path();
        std::os::unix::fs::symlink(folder.join("nowhere"), folder.join("pasted-1.png")).unwrap();
        std::fs::write(folder.join("pasted-2.png"), b"theirs").unwrap();
        let written = write_new(folder, b"png").unwrap();
        assert_eq!(written, folder.join("pasted-3.png"));
        assert_eq!(std::fs::read(&written).unwrap(), b"png");
        assert_eq!(
            std::fs::read(folder.join("pasted-2.png")).unwrap(),
            b"theirs"
        );
        assert!(!folder.join("nowhere").exists());
        for name in ["pasted-1.png", "pasted-2.png", "pasted-3.png"] {
            let _ = std::fs::remove_file(folder.join(name));
        }
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

    /// Which presses are the paste chord: Ctrl+V with or without Shift (Cmd on macOS), the Paste
    /// key, Shift+Insert on Windows, and egui's text paste; not Ctrl+Alt+V as a key, a plain V, a
    /// release, or a held chord's repeat.
    #[test]
    fn the_paste_chord_is_taken_and_nothing_else() {
        use egui::{Key, Modifiers};
        let shift_command = Modifiers::COMMAND | Modifiers::SHIFT;
        let taken = [
            key(Key::V, Modifiers::COMMAND),
            key(Key::V, shift_command),
            key(Key::Paste, Modifiers::NONE),
            egui::Event::Paste("text".into()),
        ];
        let repeat = egui::Event::Key {
            key: Key::V,
            physical_key: None,
            pressed: true,
            repeat: true,
            modifiers: Modifiers::COMMAND,
        };
        // Shift+Insert is the paste chord on Windows alone, where Win+Shift+S leaves only an image.
        let insert = key(Key::Insert, Modifiers::SHIFT);
        let ctrl_insert = key(Key::Insert, Modifiers::SHIFT | Modifiers::CTRL);
        let (windows_taken, windows_kept) = if cfg!(windows) {
            (vec![insert], vec![ctrl_insert])
        } else {
            (vec![], vec![insert, ctrl_insert])
        };
        for event in windows_taken {
            let mut input = egui::InputState::default();
            input.events = vec![event.clone()];
            assert!(take_key(&mut input), "{event:?} was not taken");
        }
        for event in windows_kept {
            let mut input = egui::InputState::default();
            input.events = vec![event.clone()];
            assert!(!take_key(&mut input), "{event:?} was taken");
        }
        let kept = [
            repeat,
            key(Key::V, Modifiers::COMMAND | Modifiers::ALT),
            key(Key::V, Modifiers::NONE),
            egui::Event::Key {
                key: Key::V,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            },
        ];
        for event in taken {
            let mut input = egui::InputState::default();
            input.events = vec![event.clone()];
            assert!(take_key(&mut input), "{event:?} was not taken");
            assert!(input.events.is_empty());
        }
        for event in kept {
            let mut input = egui::InputState::default();
            input.events = vec![event.clone()];
            assert!(!take_key(&mut input), "{event:?} was taken");
            assert_eq!(input.events, vec![event]);
        }
    }
}
