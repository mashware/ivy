//! Paste: a screenshot the desktop's own tool left on the clipboard, written as a PNG into a
//! console's own folder and opened in the drawer pinned to that console (D117).
use crate::clipboard::Pasted;
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

/// Who asked for a paste: the header's button opens the viewer whatever the clipboard holds, a key
/// pressed over an image already shown only says what it found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    Button,
    Key,
}

/// A paste from the read to the opened image; one at a time.
#[derive(Default)]
pub(crate) struct Pasting {
    /// The paste chord, taken by the viewer while it painted; the input pass asks for it.
    pub(super) key: bool,
    asked: Option<Asked>,
    writing: Option<(usize, Asked, Job<PathBuf>)>,
}

impl Pasting {
    fn busy(&self) -> bool {
        self.asked.is_some() || self.writing.is_some()
    }

    pub(crate) fn pending(&self) -> bool {
        self.key || self.writing.is_some()
    }
}

/// Takes the paste chord out of `input`: as a key when the clipboard held no text — the vendored
/// egui-winit lets it through then — or as the text paste egui-winit made of it otherwise. Shift
/// is not looked at: egui-winit does not, so both chords reach here as the same paste.
pub(super) fn take_key(input: &mut egui::InputState) -> bool {
    let before = input.events.len();
    input.events.retain(|event| match event {
        egui::Event::Paste(_) => false,
        egui::Event::Key {
            key: egui::Key::V,
            pressed: true,
            modifiers,
            ..
        } => !(modifiers.command && !modifiers.alt),
        _ => true,
    });
    input.events.len() != before
}

/// A paste for the console `pane`. Refused, before the clipboard is read, when the image shown
/// keeps unsent marks or the console has no folder to keep the file in.
pub(crate) fn ask(window: &mut crate::Window, pane: usize, by: Asked) {
    if origin::keeps_marks(window) {
        return;
    }
    if window.image.paste.busy() {
        tell(window, pane, by, Some(BUSY));
        return;
    }
    let Some(console) = window.all_panes().find(|p| p.name == pane) else {
        return;
    };
    if console.panel.is_none() {
        tell(window, pane, by, Some(NO_FOLDER));
        return;
    }
    if !window.clipboard.request_image(pane, Instant::now()) {
        tell(window, pane, by, Some("The clipboard is busy; try again."));
        return;
    }
    window.image.paste.asked = Some(by);
}

/// What the clipboard answered for the console `pane`.
pub(crate) fn received(
    window: &mut crate::Window,
    pane: usize,
    pasted: Pasted,
    ctx: &egui::Context,
) {
    let by = window.image.paste.asked.take().unwrap_or(Asked::Button);
    match pasted {
        Pasted::Image(image) => {
            if let Err(why) = write(window, pane, by, image, ctx) {
                tell(window, pane, by, Some(&why));
            }
        }
        Pasted::Absent if by == Asked::Button => tell(window, pane, by, None),
        Pasted::Absent => tell(window, pane, by, Some(NO_IMAGE)),
        Pasted::Failed(why) => {
            let said = format!("The clipboard could not be read: {why}.");
            tell(window, pane, by, Some(&said));
        }
        Pasted::Expired => tell(window, pane, by, Some(EXPIRED)),
    }
}

/// Refuses an image the viewer would refuse anyway, before a file is left for it, then starts the
/// worker that encodes and writes it.
fn write(
    window: &mut crate::Window,
    pane: usize,
    by: Asked,
    image: image::RgbaImage,
    ctx: &egui::Context,
) -> Result<(), String> {
    let folder = window
        .all_panes()
        .find(|p| p.name == pane)
        .and_then(|p| p.panel.as_ref())
        .map(|dir| dir.path().to_owned())
        .ok_or(NO_FOLDER)?;
    if decodes_too_large(image.width(), image.height()) {
        return Err(too_large(DECODE_LIMIT));
    }
    let job = spawn(ctx.clone(), move || {
        let png = encode(&image, IMAGE_LIMIT)?;
        write_new(&folder, &png)
    })?;
    window.image.paste.writing = Some((pane, by, job));
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
                let _ = std::fs::remove_file(&path);
                Err(format!(
                    "The screenshot could not be written at {}: {e}",
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
        ask(window, pane, Asked::Key);
    }
    let Some((_, _, job)) = &window.image.paste.writing else {
        return;
    };
    let Some(written) = take(job) else {
        ctx.request_repaint_after(Duration::from_millis(50));
        return;
    };
    let (pane, by, _) = window.image.paste.writing.take().expect("a paste being written");
    let path = match written {
        Ok(path) => path,
        Err(why) => return tell(window, pane, by, Some(&why)),
    };
    let Some(console) = window.all_panes_mut().find(|p| p.name == pane) else {
        return forsake(&path);
    };
    // The console removes it when it closes, with Send's copies.
    console.annotated.record(path.clone());
    if let Err(why) = origin::open_image_target(window, pane, path) {
        // Marks drawn on the image shown while the screenshot was written: they are kept, and
        // the file stays the console's until it closes.
        window.image.say(why.to_string());
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
    if let Some(folder) = path.parent()
        && std::fs::remove_dir(folder).is_ok()
    {
        eprintln!(
            "the panel directory {} was removed once its pasted screenshot was gone",
            folder.display()
        );
    }
}

/// Says what became of a paste: over an image already shown, a key's paste only says it; anything
/// else opens the viewer for that console, empty, with the hint after what is said.
fn tell(window: &mut crate::Window, pane: usize, by: Asked, said: Option<&str>) {
    if by == Asked::Key
        && window.image.loaded_picture()
        && let Some(said) = said
    {
        window.image.say(said);
        return;
    }
    let text = match said {
        Some(said) => format!("{said} {HINT}"),
        None => HINT.to_owned(),
    };
    let _ = origin::pin_empty_image(window, pane, text);
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
        assert_eq!(std::fs::read(folder.join("pasted-2.png")).unwrap(), b"theirs");
        assert!(!folder.join("nowhere").exists());
        for name in ["pasted-1.png", "pasted-2.png", "pasted-3.png"] {
            let _ = std::fs::remove_file(folder.join(name));
        }
    }
}
