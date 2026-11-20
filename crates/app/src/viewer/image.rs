//! The drawer's image: a PNG or JPEG a console printed, or Files opened, shown in the document tab
//! when it is the last document opened (D72). It is decoded once per open and never refreshed, so
//! a program that overwrites the same screenshot name does not change what is on screen.
use super::Source;
use super::load::{Job, Picture};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// How far the zoom goes, in screen points per pixel of the original.
const SMALLEST: f32 = 0.01;
const LARGEST: f32 = 32.0;

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
    document: Option<Opened>,
    /// A decode whose image was replaced or closed before it finished. A read stuck on an
    /// unresponsive filesystem cannot be cancelled, so its handle is kept and no other decode
    /// starts until it ends: repeated opens cannot pile up readers.
    abandoned: Option<(PathBuf, Job<Picture>)>,
}

struct Opened {
    origin: Source,
    path: PathBuf,
    load: Load,
    /// Screen points per pixel of the original; `None` fits the drawer's width.
    scale: Option<f32>,
    offset: egui::Vec2,
}

enum Load {
    /// Waiting for a frame to say how large a texture the GPU takes.
    Pending,
    Decoding(Job<Picture>),
    Ready {
        original: Arc<image::RgbaImage>,
        gpu: Gpu,
    },
    Failed(String),
}

/// The display copy until the first frame that shows it hands it to the GPU, then the texture.
enum Gpu {
    Waiting(egui::ColorImage),
    Uploaded(egui::TextureHandle),
}

impl Viewer {
    /// Replaces whatever image was shown with `path`, read from `origin`'s console. A relative
    /// path is one whose console's directory is unknown: it is refused and says so.
    pub(super) fn open(&mut self, origin: Source, path: PathBuf) {
        self.clear();
        let load = if path.is_relative() {
            Load::Failed("This console has not published its working directory. Open an absolute image path or enable OSC 7 in the shell.".into())
        } else {
            Load::Pending
        };
        self.document = Some(Opened {
            origin,
            path,
            load,
            scale: None,
            offset: egui::Vec2::ZERO,
        });
    }

    /// Forgets the image, keeping an unfinished decode's handle.
    pub(super) fn clear(&mut self) {
        if let Some(Opened {
            path,
            load: Load::Decoding(job),
            ..
        }) = self.document.take()
        {
            self.abandoned = Some((path, job));
        }
    }

    /// Whether the document tab paints this image.
    pub(super) fn shown(&self) -> bool {
        self.document.is_some()
    }

    pub(super) fn origin(&self) -> Option<&Source> {
        Some(&self.document.as_ref()?.origin)
    }

    pub(super) fn path(&self) -> Option<&Path> {
        Some(&self.document.as_ref()?.path)
    }

    pub(super) fn file_name(&self) -> Option<String> {
        self.path()?
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    /// Why the image is not shown, when it cannot be.
    pub(crate) fn message(&self) -> Option<&str> {
        match &self.document.as_ref()?.load {
            Load::Failed(why) => Some(why),
            _ => None,
        }
    }

    /// The decoded image at full resolution, once it has arrived.
    pub(super) fn original(&self) -> Option<&Arc<image::RgbaImage>> {
        match &self.document.as_ref()?.load {
            Load::Ready { original, .. } => Some(original),
            _ => None,
        }
    }

    /// The original's size in pixels.
    pub(super) fn size(&self) -> Option<egui::Vec2> {
        let original = self.original()?;
        Some(egui::vec2(
            original.width() as f32,
            original.height() as f32,
        ))
    }

    /// Starts the one decode this open gets, unless another is still running, and takes its result
    /// when it arrives.
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        if let Some((_, job)) = &self.abandoned
            && job.poll().is_some()
        {
            self.abandoned = None;
        }
        let Some(document) = self.document.as_mut() else {
            return;
        };
        if matches!(document.load, Load::Pending) {
            document.load = match &self.abandoned {
                Some((busy, _)) => Load::Failed(format!("Still reading {}…", busy.display())),
                None => {
                    let side = ctx.input(|i| i.max_texture_side);
                    match Job::image(document.path.clone(), side, ctx.clone()) {
                        Ok(job) => Load::Decoding(job),
                        Err(why) => Load::Failed(why),
                    }
                }
            };
        }
        if let Load::Decoding(job) = &document.load {
            match job.poll() {
                Some(Ok(picture)) => {
                    let (original, display) = picture.into_parts();
                    document.load = Load::Ready {
                        original,
                        gpu: Gpu::Waiting(display),
                    };
                }
                Some(Err(why)) => document.load = Load::Failed(why),
                None => ctx.request_repaint_after(Duration::from_millis(100)),
            }
        }
    }

    /// Paints the image over the drawer's body: fitted to its width until the person zooms, and
    /// scrolled with the wheel or the scroll bars.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, theme: crate::theme::Theme) {
        let Some(size) = self.size() else {
            let message = self.message();
            egui::Frame::new()
                .inner_margin(14)
                .show(ui, |ui| match message {
                    Some(why) => {
                        ui.colored_label(
                            theme.panel_color(Some(panel_contract::Mark::Block), None),
                            why,
                        );
                    }
                    None => {
                        ui.label("Loading…");
                    }
                });
            return;
        };
        let Some(Opened {
            load: Load::Ready { gpu, .. },
            scale: chosen,
            offset,
            ..
        }) = self.document.as_mut()
        else {
            return;
        };
        if let Gpu::Waiting(display) = gpu {
            let display = std::mem::take(display);
            *gpu = Gpu::Uploaded(ui.ctx().load_texture(
                "drawer-image",
                display,
                egui::TextureOptions::LINEAR,
            ));
        }
        let Gpu::Uploaded(texture) = gpu else {
            return;
        };
        let body = ui.max_rect();
        let mut scale =
            chosen.unwrap_or_else(|| fit(body.width(), size.x, ui.ctx().pixels_per_point()));
        let zoom = ui.input(|i| i.zoom_delta());
        if zoom != 1.0
            && let Some(at) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|at| body.contains(*at))
        {
            (scale, *offset) = zoom_around(scale, *offset, at - body.min, zoom);
            *chosen = Some(scale);
        }
        let output = egui::ScrollArea::both()
            .id_salt("drawer-image")
            .auto_shrink(false)
            .scroll_offset(*offset)
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(size * scale, egui::Sense::hover());
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            });
        *offset = output.state.offset;
    }

    #[cfg(test)]
    pub(crate) fn loaded(&self) -> bool {
        self.original().is_some()
    }

    /// Makes the shown image's decode one that has not finished, for as long as the returned
    /// sender lives.
    #[cfg(test)]
    pub(crate) fn stall(&mut self) -> std::sync::mpsc::Sender<Result<Picture, String>> {
        let (job, sender) = Job::stalled();
        self.document.as_mut().unwrap().load = Load::Decoding(job);
        sender
    }
}

/// The scale and scroll offset after zooming by `factor`, keeping the image point under `at`, a
/// position inside the view, where it was on screen.
fn zoom_around(scale: f32, offset: egui::Vec2, at: egui::Vec2, factor: f32) -> (f32, egui::Vec2) {
    let zoomed = (scale * factor).clamp(SMALLEST, LARGEST);
    let point = (offset + at) / scale;
    (zoomed, (point * zoomed - at).max(egui::Vec2::ZERO))
}

/// The scale that fits an image `width` pixels wide into `room` points, never enlarging it past
/// one physical pixel per pixel: a screenshot shown larger than it was taken only blurs.
fn fit(room: f32, width: f32, pixels_per_point: f32) -> f32 {
    (room / width.max(1.0)).min(1.0 / pixels_per_point)
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
        while !viewer.loaded() {
            viewer.poll(&ctx);
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{:?}",
                viewer.message()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let first = Arc::clone(viewer.original().unwrap());
        let took = started.elapsed();
        write_png(&path, 80, 10);
        let until = Instant::now() + (took * 5).max(Duration::from_millis(300));
        while Instant::now() < until {
            viewer.poll(&ctx);
            std::thread::sleep(Duration::from_millis(5));
        }
        let now = viewer.original().unwrap();
        assert!(Arc::ptr_eq(&first, now));
        assert_eq!(now.dimensions(), (40, 20));
    }

    /// The largest texture the test GPU takes, smaller than the wide fixtures' 2400 pixels.
    const MAX_SIDE: usize = 2048;

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
                max_texture_side: Some(MAX_SIDE),
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

    /// The text painted inside `within`.
    fn texts(shapes: &[egui::epaint::ClippedShape], within: egui::Rect) -> Vec<String> {
        fn walk(shape: &egui::Shape, within: egui::Rect, found: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(text) if within.contains(text.pos) => {
                    found.push(text.galley.text().to_owned());
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, within, found);
                    }
                }
                _ => {}
            }
        }
        let mut found = Vec::new();
        for clipped in shapes {
            walk(&clipped.shape, within, &mut found);
        }
        found
    }

    /// The size of the texture the GPU was handed, once the image has been shown.
    fn texture_size(viewer: &Viewer) -> Option<[usize; 2]> {
        match &viewer.document.as_ref()?.load {
            Load::Ready {
                gpu: Gpu::Uploaded(texture),
                ..
            } => Some(texture.size()),
            _ => None,
        }
    }

    /// A window with an image open in the drawer, pinned to its second console, once decoded.
    fn window_with_image(path: &Path) -> (crate::Window, egui::Context, Duration) {
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        painted(&mut window, &ctx, vec![]);
        let name = window.active().panes()[1].name;
        crate::viewer::open_diff_for_pane(&mut window, name);
        crate::viewer::open_image_target(&mut window, name, path.to_owned()).unwrap();
        let started = Instant::now();
        while image_rect(&painted(&mut window, &ctx, vec![])).is_none() {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{:?}",
                window.image.message()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let [width, height] = texture_size(&window.image).unwrap();
        let (wide, high) = window.image.original().unwrap().dimensions();
        if wide.max(high) as usize > MAX_SIDE {
            assert_eq!(width.max(height), MAX_SIDE, "{width}x{height}");
        } else {
            assert_eq!([width, height], [wide as usize, high as usize]);
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
        let first = Arc::clone(window.image.original().unwrap());
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
        let now = window.image.original().unwrap();
        assert!(Arc::ptr_eq(&first, now));
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
        for pane in window.active_mut().panes_mut() {
            pane.screen.advance(b"\x1b[31mred\x1b[0m plain text\r\n");
        }
        // With the keys elsewhere, the drawer owns only the frames of the gesture itself, so the
        // consoles' plain colours are the ones painted while nothing claims the pointer.
        crate::viewer::blur(&mut window);
        let shapes = painted(&mut window, &ctx, vec![]);
        let metrics =
            crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
        let consoles = crate::tab_bar::geometry(&window, area(), &metrics).consoles;
        assert!(
            texts(&shapes, consoles).concat().contains("redplaintext"),
            "the consoles painted no text for a dim to change"
        );
        let plain = inks(&shapes, consoles);
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
        assert_eq!(
            inks(&shapes, consoles),
            plain,
            "the zoom dimmed the consoles"
        );
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

    /// A viewer holding `original`, decoded and not yet shown.
    fn ready(original: image::RgbaImage) -> Viewer {
        let mut viewer = Viewer::default();
        viewer.open(source(), PathBuf::from("/shot.png"));
        viewer.document.as_mut().unwrap().load = Load::Ready {
            original: Arc::new(original),
            gpu: Gpu::Waiting(egui::ColorImage::filled([4, 2], egui::Color32::RED)),
        };
        viewer
    }

    /// An image wider than the drawer fits its width; a narrower one is shown at one physical
    /// pixel per pixel rather than enlarged, which on a display of two pixels per point is half a
    /// point.
    #[test]
    fn an_image_is_fitted_to_the_drawer_width_and_never_enlarged() {
        let body = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(500.0, 400.0));
        for (width, height, pixels_per_point, shown) in [
            (2000, 1000, 1.0, 500.0),
            (300, 100, 1.0, 300.0),
            (300, 100, 2.0, 150.0),
            (2000, 1000, 2.0, 500.0),
        ] {
            let ctx = egui::Context::default();
            let mut viewer = ready(image::RgbaImage::new(width, height));
            let display = egui::ViewportInfo {
                native_pixels_per_point: Some(pixels_per_point),
                ..Default::default()
            };
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(body),
                    viewports: [(egui::ViewportId::ROOT, display)].into_iter().collect(),
                    ..Default::default()
                },
                |ui| {
                    assert_eq!(ui.ctx().pixels_per_point(), pixels_per_point);
                    viewer.show(ui, crate::theme::Theme::default());
                },
            );
            let rect = image_rect(&output.shapes).unwrap();
            output.drop_without_applying_deltas();
            assert_eq!(
                rect.width(),
                shown,
                "{width}x{height} at {pixels_per_point}"
            );
            assert_eq!(rect.height(), shown * height as f32 / width as f32);
        }
    }

    #[test]
    fn a_relative_path_says_why_and_reads_nothing() {
        let ctx = egui::Context::default();
        let mut viewer = Viewer::default();
        viewer.open(source(), PathBuf::from("shot.png"));
        viewer.poll(&ctx);
        assert!(!viewer.loaded());
        assert!(matches!(
            viewer.document.as_ref().unwrap().load,
            Load::Failed(_)
        ));
        assert!(viewer.message().unwrap().contains("working directory"));
        assert!(viewer.shown());
    }

    /// Polls until the drawer paints `wanted`, returning everything it painted as text.
    fn wait_for_text(window: &mut crate::Window, ctx: &egui::Context, wanted: &str) -> String {
        let started = Instant::now();
        loop {
            let shapes = painted(window, ctx, vec![]);
            let dock = window.sidebar.last_dock.unwrap();
            let text = texts(&shapes, dock).join("\n");
            if text.contains(wanted) {
                return text;
            }
            assert!(started.elapsed() < Duration::from_secs(5), "{text}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A file named like a PNG that is not one says so in the drawer instead of loading forever.
    #[test]
    fn a_file_that_does_not_decode_says_why_in_the_drawer() {
        let folder = Folder::new();
        let path = folder.0.join("anim.png");
        std::fs::write(&path, b"GIF89a\x01\x00\x01\x00\x00\x00\x00;").unwrap();
        let mut window = crate::tests::window_of_tabs([["sleep 30", "sleep 30"]]);
        let ctx = egui::Context::default();
        painted(&mut window, &ctx, vec![]);
        let name = window.active().panes()[1].name;
        crate::viewer::open_image_target(&mut window, name, path).unwrap();
        wait_for_text(&mut window, &ctx, "This file is not a PNG or JPEG image");
        assert!(!window.image.loaded());
    }

    /// While a decode has not finished, a second open starts no reader of its own: it says which
    /// file is still being read, also when the tab that started that read was closed. Once the read
    /// ends, opening again decodes.
    #[test]
    fn a_second_open_waits_for_the_decode_still_running() {
        use crate::viewer::StripAction;
        let folder = Folder::new();
        let (first, second) = (folder.0.join("first.png"), folder.0.join("second.png"));
        write_png(&first, 32, 16);
        write_png(&second, 48, 16);
        let (mut window, ctx, _) = window_with_image(&first);
        let name = window.active().panes()[1].name;
        let finish = window.image.stall();
        crate::viewer::open_image_target(&mut window, name, second.clone()).unwrap();
        let still = format!("Still reading {}…", first.display());
        wait_for_text(&mut window, &ctx, &still);
        let dock = window.sidebar.last_dock.unwrap();
        let close = crate::viewer::strip_place(&window, dock, StripAction::CloseDocument).unwrap();
        click(&mut window, &ctx, close);
        assert!(!window.image.shown());
        crate::viewer::open_image_target(&mut window, name, second.clone()).unwrap();
        wait_for_text(&mut window, &ctx, &still);
        assert!(window.image.abandoned.is_some());
        drop(finish);
        crate::viewer::open_image_target(&mut window, name, second).unwrap();
        let started = Instant::now();
        while !window.image.loaded() {
            painted(&mut window, &ctx, vec![]);
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{:?}",
                window.image.message()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(window.image.original().unwrap().dimensions(), (48, 16));
        assert!(window.image.abandoned.is_none());
    }

    /// Closing the document tab forgets the image, and the next document tab is Markdown's again.
    #[test]
    fn closing_the_document_tab_forgets_the_image() {
        use crate::viewer::StripAction;
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 64, 32);
        let (mut window, ctx, _) = window_with_image(&path);
        let dock = window.sidebar.last_dock.unwrap();
        let close = crate::viewer::strip_place(&window, dock, StripAction::CloseDocument).unwrap();
        click(&mut window, &ctx, close);
        assert!(!window.image.shown());
        assert_eq!(crate::viewer::shown_image(&window), None);
        crate::sidebar::toggle(&mut window, crate::sidebar::Tool::Document, &ctx);
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(image_rect(&shapes).is_none());
    }

    /// The wheel over the image pans it and Ctrl+wheel zooms it, and neither reaches a console,
    /// which the same wheel scrolls back through its history when the pointer is over it.
    #[test]
    fn gestures_over_the_image_move_it_and_never_scroll_a_console() {
        let folder = Folder::new();
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let (mut window, ctx, _) = window_with_image(&path);
        let history: String = (0..400).map(|n| format!("line {n}\r\n")).collect();
        for pane in window.active_mut().panes_mut() {
            pane.screen.advance(history.as_bytes());
        }
        let scrolls = |window: &crate::Window| -> Vec<usize> {
            window
                .active()
                .panes()
                .iter()
                .map(|pane| pane.screen.view().scroll())
                .collect()
        };
        // As when Files takes the keys: the drawer then keeps only what happens over it.
        crate::viewer::blur(&mut window);
        let before = image_rect(&painted(&mut window, &ctx, vec![])).unwrap();
        assert_eq!(scrolls(&window), [0, 0]);
        let body = window.sidebar.last_dock.unwrap();
        let at = egui::pos2(body.center().x, body.bottom() - 60.0);
        let wheel = |at: egui::Pos2, dy: f32, modifiers: egui::Modifiers| {
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, dy),
                    phase: egui::TouchPhase::Move,
                    modifiers,
                },
            ]
        };
        let settle = |window: &mut crate::Window, events| {
            let mut shapes = painted(window, &ctx, events);
            for _ in 0..30 {
                shapes = painted(window, &ctx, vec![]);
            }
            image_rect(&shapes).unwrap()
        };
        let panned = settle(&mut window, wheel(at, -3.0, egui::Modifiers::NONE));
        assert!(panned.top() < before.top(), "{before:?} -> {panned:?}");
        assert_eq!(panned.width(), before.width());
        let chord = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let zoomed = settle(&mut window, wheel(at, 3.0, chord));
        assert!(
            zoomed.width() > panned.width() * 1.2,
            "{panned:?} -> {zoomed:?}"
        );
        assert_eq!(
            scrolls(&window),
            [0, 0],
            "a gesture over the image scrolled a console"
        );
        let console = window.active().panes()[0]
            .painted_console
            .as_ref()
            .unwrap()
            .area
            .center();
        settle(&mut window, wheel(console, 3.0, egui::Modifiers::NONE));
        assert!(
            scrolls(&window)[0] > 0,
            "the wheel over a console did not scroll it"
        );
    }
}
