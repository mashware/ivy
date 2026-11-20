//! The drawer's image: a PNG or JPEG a console printed, or Files opened, shown in the document tab
//! when it is the last document opened (D72). It is decoded once per open and never refreshed, so
//! a program that overwrites the same screenshot name does not change what is on screen.
use super::Source;
use super::load::{Job, Picture};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Which document the drawer's document tab paints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Document {
    #[default]
    Markdown,
    Image,
}

/// Whether a path names a file this viewer opens, by its extension in any letter case.
pub(crate) fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["png", "jpg", "jpeg"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

#[derive(Default)]
pub(crate) struct Viewer {
    pub(super) document: Document,
    pub(super) origin: Option<Source>,
    path: Option<PathBuf>,
    started: bool,
    job: Option<Job<Picture>>,
    picture: Option<Picture>,
    pub(super) message: String,
}

impl Viewer {
    /// Replaces whatever image was shown with `path`, read from `origin`'s console.
    pub(super) fn open(&mut self, origin: Source, path: PathBuf) {
        let relative = path.is_relative();
        *self = Self {
            document: Document::Image,
            origin: Some(origin),
            path: Some(path),
            ..Self::default()
        };
        if relative {
            self.started = true;
            self.message = "This console has not published its working directory. Open an absolute image path or enable OSC 7 in the shell.".into();
        }
    }

    /// Whether the document tab paints this image.
    pub(super) fn shown(&self) -> bool {
        self.document == Document::Image
    }

    pub(super) fn file_name(&self) -> Option<String> {
        self.path
            .as_ref()?
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    /// Starts the one decode this open gets, and takes its result when it arrives.
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        if !self.started
            && let Some(path) = &self.path
        {
            self.started = true;
            match Job::image(path.clone(), ctx.input(|i| i.max_texture_side), ctx.clone()) {
                Ok(job) => self.job = Some(job),
                Err(why) => self.message = why,
            }
        }
        if let Some(result) = self.job.as_ref().and_then(|job| job.poll()) {
            self.job = None;
            match result {
                Ok(picture) => self.picture = Some(picture),
                Err(why) => self.message = why,
            }
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    pub(crate) struct Folder(pub(crate) PathBuf);
    impl Folder {
        pub(crate) fn new() -> Self {
            let mut nonce = [0; 8];
            getrandom::fill(&mut nonce).unwrap();
            let path =
                std::env::temp_dir().join(format!("ivy-image-{:x}", u64::from_ne_bytes(nonce)));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn write_png(path: &Path, width: u32, height: u32) {
        image::RgbaImage::from_pixel(width, height, image::Rgba([200, 40, 40, 255]))
            .save_with_format(path, image::ImageFormat::Png)
            .unwrap();
    }

    fn source() -> Source {
        Source {
            pane: 1,
            label: "console".into(),
            cwd: None,
        }
    }

    #[test]
    fn image_paths_are_recognised_by_extension_in_any_case() {
        for path in [
            "/a/shot.png",
            "/a/shot.PNG",
            "/a/p.jpg",
            "/a/p.JpEg",
            "shot.png",
        ] {
            assert!(is_image(Path::new(path)), "{path}");
        }
        for path in ["/a/shot.gif", "/a/notes.md", "/a/png", "/a/shot.png.txt"] {
            assert!(!is_image(Path::new(path)), "{path}");
        }
    }

    /// An agent overwrites the same screenshot name; the image open on screen is the one decoded
    /// when it was opened, however long the viewer keeps polling.
    #[test]
    fn an_open_image_is_decoded_once_and_not_read_again() {
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 40, 20);
        let ctx = egui::Context::default();
        let mut viewer = Viewer::default();
        viewer.open(source(), path.clone());
        let started = Instant::now();
        while viewer.picture.is_none() {
            viewer.poll(&ctx);
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{}",
                viewer.message
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let first = std::sync::Arc::clone(&viewer.picture.as_ref().unwrap().original);
        let took = started.elapsed();
        write_png(&path, 80, 10);
        let until = Instant::now() + (took * 5).max(Duration::from_millis(300));
        while Instant::now() < until {
            viewer.poll(&ctx);
            std::thread::sleep(Duration::from_millis(5));
        }
        let now = &viewer.picture.as_ref().unwrap().original;
        assert!(std::sync::Arc::ptr_eq(&first, now));
        assert_eq!(now.dimensions(), (40, 20));
    }

    #[test]
    fn a_relative_path_says_why_and_reads_nothing() {
        let ctx = egui::Context::default();
        let mut viewer = Viewer::default();
        viewer.open(source(), PathBuf::from("shot.png"));
        viewer.poll(&ctx);
        assert!(viewer.job.is_none() && viewer.picture.is_none());
        assert!(viewer.message.contains("working directory"));
        assert!(viewer.shown());
    }
}
