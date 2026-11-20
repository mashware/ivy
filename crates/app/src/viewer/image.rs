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
    /// Whether the drag under way pans the image, decided when its button went down.
    panning: bool,
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
    /// scrolled with the wheel, the scroll bars, or a drag that pans it. `space` is whether Space
    /// is held and the drawer's, which is when Space + left drag pans.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, theme: crate::theme::Theme, space: bool) {
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
        let panning = &mut self.panning;
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
        let view = ui.available_rect_before_wrap();
        let mut scale =
            chosen.unwrap_or_else(|| fit(view.width(), size.x, ui.ctx().pixels_per_point()));
        let zoom = ui.input(|i| i.zoom_delta());
        if zoom != 1.0
            && let Some(at) = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|at| view.contains(*at))
        {
            (scale, *offset) = zoom_around(scale, *offset, at - view.min, zoom);
            *chosen = Some(scale);
        }
        let shown = size * scale;
        let grab = without_bars(view, shown, &ui.spacing().scroll);
        let (moved, hand) = ui.input(|i| pan(i, grab, space, panning));
        *offset = within(*offset - moved, *offset, shown, view.size());
        if let Some(hand) = hand {
            ui.ctx().set_cursor_icon(hand);
        }
        // egui scrolls only forward from the content's corner and never past its far edge, so a
        // zoom that leaves the image short of the view on an axis is held where the pointer put
        // it by blank space laid before or after it.
        let lead = (-*offset).max(egui::Vec2::ZERO);
        let trail = (*offset + view.size() - shown).max(egui::Vec2::ZERO);
        let output = egui::ScrollArea::both()
            .id_salt("drawer-image")
            .auto_shrink(false)
            .scroll_source(egui::scroll_area::ScrollSource {
                drag: egui::scroll_area::DragScroll::Never,
                ..Default::default()
            })
            .scroll_offset(offset.max(egui::Vec2::ZERO))
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(lead + shown + trail, egui::Sense::hover());
                ui.painter().image(
                    texture.id(),
                    egui::Rect::from_min_size(rect.min + lead, shown),
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            });
        *offset = output.state.offset - lead;
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

/// The scale and offset after zooming by `factor`, keeping the image point under `at`, a position
/// inside the view, where it was on screen. The offset is where the view's corner falls on the
/// image, in points, and is negative where the image starts inside the view.
fn zoom_around(scale: f32, offset: egui::Vec2, at: egui::Vec2, factor: f32) -> (f32, egui::Vec2) {
    let zoomed = (scale * factor).clamp(SMALLEST, LARGEST);
    let point = (offset + at) / scale;
    (zoomed, point * zoomed - at)
}

/// Where a pan drag may take `offset`: where the image of `shown` size leaves none of a `view`
/// it could fill empty, or fits in it whole; and as far out as `before`, where a zoom around the
/// pointer had left it, so a drag can bring it back but never push it further.
fn within(
    offset: egui::Vec2,
    before: egui::Vec2,
    shown: egui::Vec2,
    view: egui::Vec2,
) -> egui::Vec2 {
    let spare = shown - view;
    let low = spare.min(egui::Vec2::ZERO).min(before);
    let high = spare.max(egui::Vec2::ZERO).max(before);
    offset.clamp(low, high)
}

/// The view less the strips egui's scroll bars take, so a drag on a bar moves the bar alone.
fn without_bars(
    view: egui::Rect,
    shown: egui::Vec2,
    scroll: &egui::style::ScrollStyle,
) -> egui::Rect {
    let strip = scroll.bar_width + scroll.bar_outer_margin;
    let mut grab = view;
    if shown.y > view.height() {
        grab.max.x -= strip;
    }
    if shown.x > view.width() {
        grab.max.y -= strip;
    }
    grab
}

/// How far a pan drag moved the pointer this frame, and the hand the cursor shows. A plain left
/// drag never pans: marks are drawn with it. Space + left drag is Figma's convention, and the
/// middle button pans with no key for a mouse that has one. Whether a drag pans is decided when
/// its button goes down and kept until it is released, so letting Space go halfway through does
/// not turn the rest into a plain drag.
fn pan(
    input: &egui::InputState,
    grab: egui::Rect,
    space: bool,
    panning: &mut bool,
) -> (egui::Vec2, Option<egui::CursorIcon>) {
    let pointer = &input.pointer;
    if pointer.any_pressed() {
        let over = pointer.press_origin().is_some_and(|at| grab.contains(at));
        *panning = over
            && (pointer.button_pressed(egui::PointerButton::Middle)
                || (space && pointer.button_pressed(egui::PointerButton::Primary)));
        if *panning {
            // The press frame's delta is the travel from wherever the pointer was before it.
            return (egui::Vec2::ZERO, Some(egui::CursorIcon::Grabbing));
        }
    } else if *panning && (pointer.middle_down() || pointer.primary_down()) {
        return (pointer.delta(), Some(egui::CursorIcon::Grabbing));
    }
    if !pointer.any_down() {
        *panning = false;
    }
    let hovering = pointer.hover_pos().is_some_and(|at| grab.contains(at));
    let hand = (space && hovering).then_some(egui::CursorIcon::Grab);
    (egui::Vec2::ZERO, hand)
}

/// Whether the Space held now is the drawer's: it is when it went down while the drawer `holds`
/// the keys, and stays as it was, `held`, until it is let go.
pub(super) fn space_is_ours(input: &egui::InputState, holds: bool, held: bool) -> bool {
    let pressed = input.events.iter().any(|event| {
        matches!(
            event,
            egui::Event::Key {
                key: egui::Key::Space,
                pressed: true,
                repeat: false,
                ..
            }
        )
    });
    input.key_down(egui::Key::Space) && if pressed { holds } else { held }
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
        pass(window, ctx, events).shapes
    }

    /// One real application pass, returning everything it produced.
    fn pass(
        window: &mut crate::Window,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
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
        output
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

    /// The Ctrl the desktop reports before the wheel turns, as egui-winit sends it.
    fn ctrl() -> egui::Modifiers {
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        }
    }

    /// One wheel event as egui-winit builds it from a Wayland axis event while Ctrl is held: a
    /// mouse notch is a line with no start or end, a touchpad's two fingers are points with both.
    fn ctrl_wheel(unit: egui::MouseWheelUnit, dy: f32, phase: egui::TouchPhase) -> egui::Event {
        egui::Event::MouseWheel {
            unit,
            delta: egui::vec2(0.0, dy),
            phase,
            modifiers: ctrl(),
        }
    }

    /// From the fitted view, an image smaller than the drawer on both axes zooms in and back out
    /// around the pointer on its centre, with a mouse wheel and with a touchpad, as the desktop
    /// delivers them; the zoom stops at its bounds.
    #[test]
    fn zooming_keeps_the_point_under_the_pointer_and_stops_at_its_bounds() {
        use egui::{MouseWheelUnit::*, TouchPhase::*};
        let folder = Folder::new();
        let path = folder.0.join("small.png");
        write_png(&path, 600, 120);
        let (mut window, ctx, _) = window_with_image(&path);
        let before = settle(&mut window, &ctx);
        let body = window.sidebar.last_dock.unwrap();
        assert!(body.width() > before.width() && body.height() > before.height() * 1.5);
        let at = before.center();
        let under = |rect: egui::Rect| (at - rect.min) / rect.size();
        painted(&mut window, &ctx, vec![egui::Event::PointerMoved(at)]);
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::ModifiersChanged(ctrl())],
        );
        let notches = vec![ctrl_wheel(Line, 1.0, Move)];
        let mut gestures = vec![(notches.clone(), true), (notches.clone(), true)];
        let fingers = std::iter::once(ctrl_wheel(Point, 6.0, Start))
            .chain((0..6).map(|_| ctrl_wheel(Point, 6.0, Move)))
            .chain(std::iter::once(ctrl_wheel(Point, 0.0, End)));
        gestures.push((fingers.collect(), true));
        gestures.push((vec![ctrl_wheel(Line, -1.0, Move)], false));
        let mut last = before;
        for (events, bigger) in gestures {
            for event in events {
                painted(&mut window, &ctx, vec![event]);
            }
            let now = settle(&mut window, &ctx);
            assert_eq!(now.width() > last.width(), bigger, "{last:?} -> {now:?}");
            // Within the point the content's corner is rounded to.
            assert!(
                ((under(now) - under(before)) * now.size()).length() < 1.0,
                "the point under the pointer moved: {before:?} -> {now:?}"
            );
            last = now;
        }
        let (offset, at) = (egui::vec2(100.0, 40.0), egui::vec2(200.0, 150.0));
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
                    viewer.show(ui, crate::theme::Theme::default(), false);
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

    fn space(pressed: bool) -> egui::Event {
        egui::Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Paints passes until the image stops moving, and says where it ended.
    fn settle(window: &mut crate::Window, ctx: &egui::Context) -> egui::Rect {
        let mut shapes = painted(window, ctx, vec![]);
        for _ in 0..30 {
            shapes = painted(window, ctx, vec![]);
        }
        image_rect(&shapes).unwrap()
    }

    /// A window with the wide fixture open and zoomed in around the drawer's centre, so it can be
    /// moved every way; the drawer holds the keys, as after opening it.
    fn zoomed_window(folder: &Folder) -> (crate::Window, egui::Context) {
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let (mut window, ctx, _) = window_with_image(&path);
        let at = window.sidebar.last_dock.unwrap().center();
        let chord = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        for _ in 0..4 {
            painted(
                &mut window,
                &ctx,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::ModifiersChanged(chord),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Line,
                        delta: egui::vec2(0.0, 3.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: chord,
                    },
                ],
            );
        }
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
        settle(&mut window, &ctx);
        (window, ctx)
    }

    /// Drags `button` from the drawer's centre by `by` in three steps, returning the cursor each
    /// moving pass asked for.
    fn drag(
        window: &mut crate::Window,
        ctx: &egui::Context,
        button: egui::PointerButton,
        by: egui::Vec2,
    ) -> Vec<egui::CursorIcon> {
        let from = window.sidebar.last_dock.unwrap().center();
        drag_at(window, ctx, button, from, by, false)
    }

    /// Drags `button` from `from` by `by` in three steps, letting Space go after the first when
    /// `let_go` is set, and returns the cursor each moving pass asked for.
    fn drag_at(
        window: &mut crate::Window,
        ctx: &egui::Context,
        button: egui::PointerButton,
        from: egui::Pos2,
        by: egui::Vec2,
        let_go: bool,
    ) -> Vec<egui::CursorIcon> {
        let press = |pos, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        // Arriving and pressing in one pass, as a quick hand does: the travel there is no pan.
        pass(
            window,
            ctx,
            vec![egui::Event::PointerMoved(from), press(from, true)],
        );
        let mut cursors = Vec::new();
        for step in 1..=3 {
            let at = from + by * step as f32 / 3.0;
            let mut events = vec![egui::Event::PointerMoved(at)];
            if let_go && step == 2 {
                events.insert(0, space(false));
            }
            let output = pass(window, ctx, events);
            cursors.push(output.platform_output.cursor_icon);
        }
        pass(window, ctx, vec![press(from + by, false)]);
        cursors
    }

    /// With a mouse, the zoomed image moves with the middle button and with Space + left drag, the
    /// hand showing while it does; a plain left drag leaves it where it is, and no gesture scrolls
    /// a console.
    #[test]
    fn middle_drag_and_space_drag_pan_the_image_and_a_plain_drag_does_not() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
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
        let before = settle(&mut window, &ctx);
        let by = egui::vec2(-30.0, -45.0);
        drag(&mut window, &ctx, egui::PointerButton::Primary, by);
        let plain = settle(&mut window, &ctx);
        assert_eq!(plain, before, "a plain left drag panned the image");
        let cursors = drag(&mut window, &ctx, egui::PointerButton::Middle, by);
        assert_eq!(cursors, [egui::CursorIcon::Grabbing; 3]);
        let middle = settle(&mut window, &ctx);
        assert!(
            (middle.min - (before.min + by)).length() < 0.5,
            "{before:?} -> {middle:?}"
        );
        assert_eq!(middle.size(), before.size());
        let hover = pass(&mut window, &ctx, vec![space(true)]);
        assert_eq!(hover.platform_output.cursor_icon, egui::CursorIcon::Grab);
        let cursors = drag(&mut window, &ctx, egui::PointerButton::Primary, by);
        assert_eq!(cursors, [egui::CursorIcon::Grabbing; 3]);
        pass(&mut window, &ctx, vec![space(false)]);
        let spaced = settle(&mut window, &ctx);
        assert!(
            (spaced.min - (middle.min + by)).length() < 0.5,
            "{middle:?} -> {spaced:?}"
        );
        assert_eq!(scrolls(&window), [0, 0], "a pan scrolled a console");
    }

    /// With a console holding the keys, Space is that program's: the drawer does not claim it and
    /// hovering the image with it held shows no hand.
    #[test]
    fn a_space_typed_into_a_console_reaches_it_and_does_not_pan() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        crate::viewer::blur(&mut window);
        let at = window.sidebar.last_dock.unwrap().center();
        pass(&mut window, &ctx, vec![egui::Event::PointerMoved(at)]);
        let metrics =
            crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
        let mut owned = true;
        let mut left = Vec::new();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area()),
                events: vec![space(true)],
                ..Default::default()
            },
            |ui| {
                owned = crate::viewer::before_keys(&mut window, ui, &metrics);
                left = ui.input(|i| i.events.clone());
            },
        );
        output.drop_without_applying_deltas();
        assert!(!owned);
        assert!(left.contains(&space(true)), "{left:?}");
        let held = pass(&mut window, &ctx, vec![]);
        assert!(ctx.input(|i| i.key_down(egui::Key::Space)));
        assert_ne!(held.platform_output.cursor_icon, egui::CursorIcon::Grab);
        // The click hands the drawer the keys; the Space already down stays the console's.
        let before = settle(&mut window, &ctx);
        let cursors = drag(
            &mut window,
            &ctx,
            egui::PointerButton::Primary,
            egui::vec2(-30.0, -45.0),
        );
        assert!(ctx.input(|i| i.key_down(egui::Key::Space)));
        assert!(
            !cursors.contains(&egui::CursorIcon::Grabbing),
            "{cursors:?}"
        );
        assert_eq!(
            settle(&mut window, &ctx),
            before,
            "a console's Space panned"
        );
    }

    /// Once egui has seen a touch its own drag-to-scroll would move the image as well: a pan
    /// still travels exactly as far as the pointer, and a plain left drag leaves the image.
    #[test]
    fn after_a_touch_a_pan_moves_the_image_once_and_a_plain_drag_not_at_all() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let touch = |phase| egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos: egui::pos2(2.0, 2.0),
            force: None,
        };
        pass(&mut window, &ctx, vec![touch(egui::TouchPhase::Start)]);
        pass(&mut window, &ctx, vec![touch(egui::TouchPhase::End)]);
        assert!(ctx.input(|i| i.has_touch_screen()));
        let before = settle(&mut window, &ctx);
        let by = egui::vec2(-30.0, -45.0);
        drag(&mut window, &ctx, egui::PointerButton::Primary, by);
        assert_eq!(
            settle(&mut window, &ctx),
            before,
            "a plain left drag scrolled"
        );
        drag(&mut window, &ctx, egui::PointerButton::Middle, by);
        let after = settle(&mut window, &ctx);
        assert!(
            (after.min - (before.min + by)).length() < 0.5,
            "{before:?} -> {after:?}"
        );
    }

    /// A drag that started as a pan stays one when Space is let go before the button.
    #[test]
    fn letting_space_go_halfway_through_a_pan_keeps_panning() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let before = settle(&mut window, &ctx);
        pass(&mut window, &ctx, vec![space(true)]);
        let from = window.sidebar.last_dock.unwrap().center();
        let by = egui::vec2(-30.0, -45.0);
        let cursors = drag_at(
            &mut window,
            &ctx,
            egui::PointerButton::Primary,
            from,
            by,
            true,
        );
        assert_eq!(cursors, [egui::CursorIcon::Grabbing; 3]);
        let after = settle(&mut window, &ctx);
        assert!(
            (after.min - (before.min + by)).length() < 0.5,
            "{before:?} -> {after:?}"
        );
    }

    /// Space + left drag on a scroll bar is the bar's: no hand over it and no pan.
    #[test]
    fn a_space_drag_on_a_scroll_bar_leaves_the_pan_to_the_bar() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let before = settle(&mut window, &ctx);
        let dock = window.sidebar.last_dock.unwrap();
        let bar = egui::pos2(dock.right() - 3.0, dock.center().y);
        pass(&mut window, &ctx, vec![space(true)]);
        let hover = pass(&mut window, &ctx, vec![egui::Event::PointerMoved(bar)]);
        assert_ne!(hover.platform_output.cursor_icon, egui::CursorIcon::Grab);
        let by = egui::vec2(0.0, 30.0);
        let cursors = drag_at(
            &mut window,
            &ctx,
            egui::PointerButton::Primary,
            bar,
            by,
            false,
        );
        assert!(
            !cursors.contains(&egui::CursorIcon::Grabbing),
            "{cursors:?}"
        );
        let after = settle(&mut window, &ctx);
        assert_eq!(after.left(), before.left());
        assert!(after.top() < before.top(), "{before:?} -> {after:?}");
    }

    /// A maximized drawer reads every key even when nobody clicked it, so Space is its own there
    /// and Space + left drag pans.
    #[test]
    fn space_pans_a_maximized_drawer_that_was_not_clicked() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        crate::sidebar::toggle_maximize(&mut window);
        crate::viewer::blur(&mut window);
        let before = settle(&mut window, &ctx);
        pass(&mut window, &ctx, vec![space(true)]);
        let by = egui::vec2(-30.0, -45.0);
        let cursors = drag(&mut window, &ctx, egui::PointerButton::Primary, by);
        assert_eq!(cursors, [egui::CursorIcon::Grabbing; 3]);
        let after = settle(&mut window, &ctx);
        assert!(
            (after.min - (before.min + by)).length() < 0.5,
            "{before:?} -> {after:?}"
        );
    }

    /// Shift + wheel moves the zoomed image sideways and not down, which egui does on its own.
    #[test]
    fn shift_wheel_pans_the_image_sideways() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let before = settle(&mut window, &ctx);
        let at = window.sidebar.last_dock.unwrap().center();
        painted(
            &mut window,
            &ctx,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::ModifiersChanged(egui::Modifiers::SHIFT),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, -3.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::SHIFT,
                },
            ],
        );
        let after = settle(&mut window, &ctx);
        assert!(after.left() < before.left(), "{before:?} -> {after:?}");
        assert_eq!(after.top(), before.top());
    }
}
