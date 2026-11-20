//! The drawer's image: a PNG or JPEG a console printed, or Files opened, shown in the document tab
//! when it is the last document opened (D72). It is decoded once per open and never refreshed, so
//! a program that overwrites the same screenshot name does not change what is on screen.
use super::Source;
use super::load::{Job, Picture};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How far the zoom goes, in screen points per pixel of the original.
const SMALLEST: f32 = 0.01;
const LARGEST: f32 = 32.0;

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
    pub(super) path: Option<PathBuf>,
    started: bool,
    job: Option<Job<Picture>>,
    picture: Option<Picture>,
    texture: Option<egui::TextureHandle>,
    /// Screen points per pixel of the original; `None` fits the drawer's width.
    scale: Option<f32>,
    offset: egui::Vec2,
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

    #[cfg(test)]
    pub(super) fn loaded(&self) -> bool {
        self.picture.is_some()
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

    /// Paints the image over the drawer's body: fitted to its width until the person zooms, and
    /// scrolled with the wheel or the scroll bars.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, theme: crate::theme::Theme) {
        if !self.message.is_empty() {
            egui::Frame::new().inner_margin(14).show(ui, |ui| {
                ui.colored_label(
                    theme.panel_color(Some(panel_contract::Mark::Block), None),
                    &self.message,
                );
            });
            return;
        }
        let Some(picture) = self.picture.as_mut() else {
            egui::Frame::new().inner_margin(14).show(ui, |ui| {
                ui.label("Loading…");
            });
            return;
        };
        // The GPU copy is uploaded on the first frame that shows the image and kept from then on.
        let texture = self.texture.get_or_insert_with(|| {
            ui.ctx().load_texture(
                "drawer-image",
                std::mem::take(&mut picture.display),
                egui::TextureOptions::LINEAR,
            )
        });
        let body = ui.max_rect();
        let size = egui::vec2(
            picture.original.width() as f32,
            picture.original.height() as f32,
        );
        let mut scale = self.scale.unwrap_or_else(|| fit(body.width(), size.x));
        let zoom = ui.input(|i| i.zoom_delta());
        if zoom != 1.0
            && let Some(at) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|at| body.contains(*at))
        {
            (scale, self.offset) = zoom_around(scale, self.offset, at - body.min, zoom);
            self.scale = Some(scale);
        }
        let output = egui::ScrollArea::both()
            .id_salt("drawer-image")
            .auto_shrink(false)
            .scroll_offset(self.offset)
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(size * scale, egui::Sense::hover());
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            });
        self.offset = output.state.offset;
    }
}

/// The scale and scroll offset after zooming by `factor`, keeping the image point under `at`, a
/// position inside the view, where it was on screen.
fn zoom_around(scale: f32, offset: egui::Vec2, at: egui::Vec2, factor: f32) -> (f32, egui::Vec2) {
    let zoomed = (scale * factor).clamp(SMALLEST, LARGEST);
    let point = (offset + at) / scale;
    (zoomed, (point * zoomed - at).max(egui::Vec2::ZERO))
}

/// The scale that fits an image `width` pixels wide into `room` points, never enlarging it.
fn fit(room: f32, width: f32) -> f32 {
    (room / width.max(1.0)).min(1.0)
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

    fn area() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0))
    }

    /// One real application pass, returning what it painted.
    fn painted(
        window: &mut crate::Window,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut frame = eframe::Frame::_new_kittest();
        eframe::App::logic(window, ctx, &mut frame);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area()),
                events,
                ..Default::default()
            },
            |ui| eframe::App::ui(window, ui, &mut frame),
        );
        output.textures_delta.clear();
        output.shapes
    }

    /// Where the drawer painted the image: the one mesh drawn with a texture of its own.
    fn image_rect(shapes: &[egui::epaint::ClippedShape]) -> Option<egui::Rect> {
        fn find(shape: &egui::Shape) -> Option<egui::Rect> {
            match shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                    Some(mesh.calc_bounds())
                }
                egui::Shape::Vec(shapes) => shapes.iter().find_map(find),
                _ => None,
            }
        }
        shapes.iter().find_map(|clipped| find(&clipped.shape))
    }

    /// The fills and text colours painted inside `consoles`, as a dimmed pass would change them.
    fn inks(shapes: &[egui::epaint::ClippedShape], consoles: egui::Rect) -> Vec<egui::Color32> {
        let mut inks: Vec<_> = shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(text) if consoles.contains(text.pos) => Some(
                    text.override_text_color
                        .unwrap_or(text.galley.job.sections[0].format.color)
                        .gamma_multiply(text.opacity_factor),
                ),
                egui::Shape::Rect(rect) if consoles.contains_rect(rect.rect) => Some(rect.fill),
                _ => None,
            })
            .collect();
        inks.sort_by_key(|c| c.to_array());
        inks.dedup();
        inks
    }

    /// A window with an image open in the drawer, pinned to its second console, once decoded.
    fn window_with_image(path: &Path) -> (crate::Window, egui::Context, Duration) {
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        painted(&mut window, &ctx, vec![]);
        let name = window.active().panes()[1].name;
        crate::viewer::open_diff_for_pane(&mut window, name);
        assert!(crate::viewer::open_image_target(
            &mut window,
            name,
            path.to_owned()
        ));
        let started = Instant::now();
        while image_rect(&painted(&mut window, &ctx, vec![])).is_none() {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{}",
                window.image.message
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        (window, ctx, started.elapsed())
    }

    fn click(window: &mut crate::Window, ctx: &egui::Context, pos: egui::Pos2) {
        for pressed in [true, false] {
            painted(
                window,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }

    /// Switching to the review and back, or closing and reopening the drawer, shows the image that
    /// was decoded when it was opened, though the file changed on disk in between.
    #[test]
    fn the_image_survives_switching_tabs_and_reopening_the_drawer() {
        use crate::sidebar::{Tool, active};
        use crate::viewer::StripAction;
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, took) = window_with_image(&path);
        let first = std::sync::Arc::clone(&window.image.picture.as_ref().unwrap().original);
        write_png(&path, 64, 32);
        let dock = window.sidebar.last_dock.unwrap();
        for (action, tool) in [
            (StripAction::Changes, Tool::Git),
            (StripAction::Document, Tool::Document),
        ] {
            let place = crate::viewer::strip_place(&window, dock, action).unwrap();
            click(&mut window, &ctx, place);
            assert_eq!(active(&window), Some(tool));
        }
        crate::sidebar::close(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        assert_eq!(active(&window), None);
        crate::sidebar::toggle(&mut window, Tool::Document, &ctx);
        let until = Instant::now() + (took * 5).max(Duration::from_millis(300));
        let mut shapes = Vec::new();
        while Instant::now() < until {
            shapes = painted(&mut window, &ctx, vec![]);
            std::thread::sleep(Duration::from_millis(5));
        }
        let now = &window.image.picture.as_ref().unwrap().original;
        assert!(std::sync::Arc::ptr_eq(&first, now));
        assert_eq!(now.dimensions(), (640, 400));
        assert!(
            image_rect(&shapes).is_some(),
            "the reopened drawer paints no image"
        );
    }

    /// Ctrl+wheel over the image zooms it around the pointer; the consoles beside the drawer keep
    /// their colours, and the same gesture over a console leaves the image alone.
    #[test]
    fn ctrl_wheel_zooms_the_image_without_dimming_the_consoles() {
        let folder = Folder::new();
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let (mut window, ctx, _) = window_with_image(&path);
        let shapes = painted(&mut window, &ctx, vec![]);
        let metrics =
            crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
        let consoles = crate::tab_bar::geometry(&window, area(), &metrics).consoles;
        let plain = inks(&shapes, consoles);
        assert!(!plain.is_empty());
        let before = image_rect(&shapes).unwrap();
        let body = window.sidebar.last_dock.unwrap();
        let at = egui::pos2(body.center().x, body.bottom() - 60.0);
        let chord = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let wheel = |at: egui::Pos2| {
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::ModifiersChanged(chord),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, 3.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: chord,
                },
            ]
        };
        let mut shapes = painted(&mut window, &ctx, wheel(at));
        for _ in 0..30 {
            shapes = painted(&mut window, &ctx, vec![]);
        }
        let after = image_rect(&shapes).unwrap();
        assert!(
            after.width() > before.width() * 1.2,
            "{before:?} -> {after:?}"
        );
        let under = |rect: egui::Rect| (at - rect.min) / rect.width();
        assert!(
            (under(before) - under(after)).length() < 0.01,
            "the point under the pointer moved: {before:?} -> {after:?}"
        );
        assert_eq!(
            inks(&shapes, consoles),
            plain,
            "the zoom dimmed the consoles"
        );
        painted(&mut window, &ctx, wheel(consoles.center()));
        for _ in 0..30 {
            shapes = painted(&mut window, &ctx, vec![]);
        }
        assert_eq!(image_rect(&shapes).unwrap().width(), after.width());
    }

    /// With the keys elsewhere, a Ctrl+wheel over the drawer is still the drawer's: egui reports
    /// it as a zoom with no scroll, which the drawer's own scroll test would not see.
    #[test]
    fn the_drawer_claims_a_ctrl_wheel_over_it_without_the_keyboard() {
        let folder = Folder::new();
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let (mut window, ctx, _) = window_with_image(&path);
        crate::viewer::blur(&mut window);
        let at = window.sidebar.last_dock.unwrap().center();
        let chord = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let metrics =
            crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
        let mut owned = false;
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area()),
                events: vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::ModifiersChanged(chord),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: egui::vec2(0.0, 3.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: chord,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                assert_eq!(ui.input(|i| i.smooth_scroll_delta), egui::Vec2::ZERO);
                owned = crate::viewer::before_keys(&mut window, ui, &metrics);
            },
        );
        output.drop_without_applying_deltas();
        assert!(owned);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer_and_stops_at_its_bounds() {
        let (offset, at) = (egui::vec2(100.0, 40.0), egui::vec2(200.0, 150.0));
        let (scale, moved) = zoom_around(0.5, offset, at, 1.5);
        assert_eq!(scale, 0.75);
        assert_eq!((moved + at) / scale, (offset + at) / 0.5);
        assert_eq!(zoom_around(0.5, offset, at, 1000.0).0, LARGEST);
        assert_eq!(zoom_around(0.5, offset, at, 0.0001).0, SMALLEST);
    }

    /// An image wider than the drawer fits its width; a narrower one is shown at one point per
    /// pixel rather than enlarged.
    #[test]
    fn an_image_is_fitted_to_the_drawer_width_and_never_enlarged() {
        let body = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(500.0, 400.0));
        for (width, height, shown) in [(2000, 1000, 500.0), (300, 100, 300.0)] {
            let ctx = egui::Context::default();
            let mut viewer = Viewer {
                picture: Some(Picture {
                    original: std::sync::Arc::new(image::RgbaImage::new(width, height)),
                    display: egui::ColorImage::filled([4, 2], egui::Color32::RED),
                }),
                ..Viewer::default()
            };
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(body),
                    ..Default::default()
                },
                |ui| viewer.show(ui, crate::theme::Theme::default()),
            );
            let rect = image_rect(&output.shapes).unwrap();
            output.drop_without_applying_deltas();
            assert_eq!(rect.width(), shown, "{width}x{height}");
            assert_eq!(rect.height(), shown * height as f32 / width as f32);
        }
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
