//! The drawer's image: a PNG or JPEG a console printed, or Files opened, shown in the document tab
//! when it is the last document opened (D72). It is decoded once per open and never refreshed, so
//! a program that overwrites the same screenshot name does not change what is on screen.
use super::Source;
use super::load::{Job, Picture};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

mod viewport;

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
    /// The button whose drag pans the image, latched when it went down.
    panning: Option<egui::PointerButton>,
    /// The frame the image was last painted in.
    painted: Option<u64>,
}

struct Opened {
    origin: Source,
    path: PathBuf,
    load: Load,
    /// Screen points per pixel of the original; `None` fits the drawer's width.
    scale: Option<f32>,
    offset: egui::Vec2,
    /// The view's size the last frame it was painted, so a resize puts the image back inside.
    view: Option<egui::Vec2>,
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
            view: None,
        });
    }

    /// Forgets the image, keeping an unfinished decode's handle.
    pub(super) fn clear(&mut self) {
        self.panning = None;
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

    /// Paints the image over the drawer's body, fitted to its width until the person zooms, and
    /// moved by the wheel or a drag that pans it; there are no scroll bars. `space` is whether
    /// Space is held and the drawer's, which is when Space + left drag pans.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, theme: crate::theme::Theme, space: bool) {
        let frame = ui.ctx().cumulative_frame_nr();
        // A drag whose release came while the image was not painted must not resume on return.
        if self.painted.is_none_or(|last| frame > last + 1) {
            self.panning = None;
        }
        self.painted = Some(frame);
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
            view: seen,
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
        let room = view.size();
        if !(room.x > 0.0 && room.y > 0.0 && room.is_finite()) {
            return;
        }
        ui.allocate_rect(view, egui::Sense::hover());
        let mut scale =
            chosen.unwrap_or_else(|| viewport::fit(room.x, size.x, ui.ctx().pixels_per_point()));
        if *seen != Some(room) {
            *offset = viewport::clamp(*offset, None, size * scale, room);
            *seen = Some(room);
        }
        let over = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|at| view.contains(*at));
        let zoom = ui.input(|i| i.zoom_delta());
        if zoom != 1.0
            && let Some(at) = over
        {
            let zoomed;
            (scale, zoomed) = viewport::zoom_around(scale, *offset, at - view.min, zoom, size);
            *offset = viewport::clamp(zoomed, Some(zoomed), size * scale, room);
            *chosen = Some(scale);
        }
        let (mut moved, hand) = ui.input(|i| viewport::pan(i, view, space, panning));
        if over.is_some() {
            moved += ui.input(|i| i.smooth_scroll_delta);
        }
        *offset = viewport::clamp(*offset - moved, Some(*offset), size * scale, room);
        if let Some(hand) = hand {
            ui.ctx().set_cursor_icon(hand);
        }
        ui.painter().image(
            texture.id(),
            egui::Rect::from_min_size(view.min - *offset, size * scale),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
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
    /// delivers them, though zooming in carries its top across the drawer's.
    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
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
    }

    /// A window with the 600x120 fixture fitted, which is smaller than the drawer on both axes,
    /// and the pointer resting at `at(image)`, with Ctrl held.
    fn small_window(
        folder: &Folder,
        at: impl Fn(egui::Rect) -> egui::Pos2,
    ) -> (crate::Window, egui::Context, egui::Rect, egui::Pos2) {
        let path = folder.0.join("small.png");
        write_png(&path, 600, 120);
        let (mut window, ctx, _) = window_with_image(&path);
        let before = settle(&mut window, &ctx);
        let at = at(before);
        painted(&mut window, &ctx, vec![egui::Event::PointerMoved(at)]);
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::ModifiersChanged(ctrl())],
        );
        (window, ctx, before, at)
    }

    /// Ctrl+wheel over the blank below a fitted screenshot keeps the image's nearest point, on its
    /// bottom edge, where it was, so the image grows from there and stays in sight.
    #[test]
    fn zooming_over_the_blank_beside_the_image_keeps_its_nearest_point() {
        let folder = Folder::new();
        let (mut window, ctx, before, at) = small_window(&folder, |image| {
            egui::pos2(image.center().x, image.bottom() + 150.0)
        });
        let view = window.sidebar.last_dock.unwrap();
        assert!(view.contains(at));
        // Three notches keep it narrower than the drawer, where a zoom never clamps it.
        for _ in 0..3 {
            painted(
                &mut window,
                &ctx,
                vec![ctrl_wheel(
                    egui::MouseWheelUnit::Line,
                    1.0,
                    egui::TouchPhase::Move,
                )],
            );
        }
        let after = settle(&mut window, &ctx);
        assert!(
            after.width() > before.width() * 1.5 && after.width() < view.width(),
            "{before:?} -> {after:?}"
        );
        assert!(
            (after.bottom() - before.bottom()).abs() < 1.0,
            "{before:?} -> {after:?}"
        );
        let x = |rect: egui::Rect| (at.x - rect.left()) / rect.width();
        assert!(
            (x(after) - x(before)).abs() * after.width() < 1.0,
            "{before:?} -> {after:?}"
        );
    }

    /// A drag brings an image a zoom left part out of the drawer back in, and never pushes it
    /// further out; an image larger than the drawer stops at its edges.
    #[test]
    fn a_drag_never_pushes_the_image_further_out_than_it_is() {
        use egui::PointerButton::Middle;
        let folder = Folder::new();
        let (mut window, ctx, fitted, at) = small_window(&folder, |image| image.center());
        for _ in 0..2 {
            painted(
                &mut window,
                &ctx,
                vec![ctrl_wheel(
                    egui::MouseWheelUnit::Line,
                    1.0,
                    egui::TouchPhase::Move,
                )],
            );
        }
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
        let zoomed = settle(&mut window, &ctx);
        let view = window.sidebar.last_dock.unwrap();
        assert!(zoomed.left() < view.left(), "{zoomed:?}");
        let out = egui::vec2(-40.0, 0.0);
        drag_at(&mut window, &ctx, Middle, at, out, false);
        assert_eq!(settle(&mut window, &ctx), zoomed, "pushed further out");
        drag_at(&mut window, &ctx, Middle, at, -out, false);
        let back = settle(&mut window, &ctx);
        // Within the point the content's corner is rounded to on each side.
        assert!(
            (back.min - (zoomed.min - out)).length() < 1.5,
            "{zoomed:?} -> {back:?}"
        );

        let (mut window, ctx) = zoomed_window(&folder);
        // Short strokes, so the pointer stays inside the window egui is told about.
        let stroke = egui::vec2(150.0, 100.0);
        for _ in 0..60 {
            drag(&mut window, &ctx, Middle, stroke);
        }
        let edge = settle(&mut window, &ctx);
        // A fitted image sits at the drawer's corner, the edge a drag stops at.
        assert_eq!(edge.left_top(), fitted.left_top(), "{edge:?}");
        drag(&mut window, &ctx, Middle, stroke);
        assert_eq!(settle(&mut window, &ctx), edge, "dragged past its edge");
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
        assert_eq!(
            settle(&mut window, &ctx),
            before,
            "a console's Space panned"
        );
        assert!(
            !cursors.contains(&egui::CursorIcon::Grabbing),
            "{cursors:?}"
        );
    }

    /// Once egui has seen a touch it turns on drag-to-scroll for its scroll areas: a pan still
    /// travels exactly as far as the pointer, and a plain left drag, which is what a one-finger
    /// touch drag arrives as, leaves the image.
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
        drag(&mut window, &ctx, egui::PointerButton::Middle, by);
        let after = settle(&mut window, &ctx);
        assert!(
            (after.min - (before.min + by)).length() < 0.5,
            "{before:?} -> {after:?}"
        );
        drag(&mut window, &ctx, egui::PointerButton::Primary, by);
        assert_eq!(
            settle(&mut window, &ctx),
            after,
            "a plain left drag scrolled"
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

    /// There are no scroll bars: Space + left drag at the drawer's right edge, where a bar would
    /// sit, pans the image the whole way, with the hand showing.
    #[test]
    fn a_space_drag_at_the_drawer_edge_pans_the_image() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let before = settle(&mut window, &ctx);
        let dock = window.sidebar.last_dock.unwrap();
        let edge = egui::pos2(dock.right() - 3.0, dock.center().y);
        pass(&mut window, &ctx, vec![space(true)]);
        let hover = pass(&mut window, &ctx, vec![egui::Event::PointerMoved(edge)]);
        assert_eq!(hover.platform_output.cursor_icon, egui::CursorIcon::Grab);
        let by = egui::vec2(0.0, 30.0);
        let cursors = drag_at(
            &mut window,
            &ctx,
            egui::PointerButton::Primary,
            edge,
            by,
            false,
        );
        assert_eq!(cursors, [egui::CursorIcon::Grabbing; 3]);
        let after = settle(&mut window, &ctx);
        assert!(
            (after.min - (before.min + by)).length() < 0.5,
            "{before:?} -> {after:?}"
        );
    }

    /// The clip the image was painted under: the one mesh drawn with a texture of its own.
    fn image_clip(shapes: &[egui::epaint::ClippedShape]) -> Option<egui::Rect> {
        shapes.iter().find_map(|clipped| match &clipped.shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                Some(clipped.clip_rect)
            }
            _ => None,
        })
    }

    /// A zoomed image far larger than the drawer is painted inside the drawer's body only, never
    /// over its strip.
    #[test]
    fn a_zoomed_image_is_clipped_to_the_drawer() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let shapes = painted(&mut window, &ctx, vec![]);
        let dock = window.sidebar.last_dock.unwrap();
        assert!(image_rect(&shapes).unwrap().contains_rect(dock));
        let clip = image_clip(&shapes).unwrap();
        assert!(dock.contains_rect(clip), "{clip:?} outside {dock:?}");
        let strip = crate::viewer::strip_place(&window, dock, crate::viewer::StripAction::Document)
            .unwrap();
        assert!(
            clip.top() > strip.y,
            "{clip:?} covers the strip at {strip:?}"
        );
    }

    /// A small image moved into the far corner of a maximized drawer is put back whole inside the
    /// drawer when it shrinks again.
    #[test]
    fn a_smaller_drawer_takes_the_image_back_inside() {
        let folder = Folder::new();
        let (mut window, ctx, _, _) = small_window(&folder, |image| image.center());
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
        crate::sidebar::toggle_maximize(&mut window);
        settle(&mut window, &ctx);
        for _ in 0..4 {
            drag(
                &mut window,
                &ctx,
                egui::PointerButton::Middle,
                egui::vec2(150.0, 150.0),
            );
        }
        let large = window.sidebar.last_dock.unwrap();
        let cornered = settle(&mut window, &ctx);
        assert!(
            (cornered.right_bottom() - large.right_bottom()).length() < 2.0,
            "{cornered:?} in {large:?}"
        );
        crate::sidebar::toggle_maximize(&mut window);
        let back = settle(&mut window, &ctx);
        let small = window.sidebar.last_dock.unwrap();
        assert!(small.width() < large.width() || small.height() < large.height());
        assert!(
            small.expand(0.5).contains_rect(back),
            "{back:?} outside {small:?}"
        );
    }

    /// The middle button pressed on the drawer's centre, and the pointer moved a little with it.
    fn start_middle_drag(window: &mut crate::Window, ctx: &egui::Context) -> egui::Pos2 {
        let from = window.sidebar.last_dock.unwrap().center();
        let press = egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Middle,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        };
        pass(window, ctx, vec![egui::Event::PointerMoved(from), press]);
        let at = from + egui::vec2(0.0, -10.0);
        pass(window, ctx, vec![egui::Event::PointerMoved(at)]);
        at
    }

    /// Moves the pointer, with whatever buttons are down, by `by` in three passes.
    fn travel(window: &mut crate::Window, ctx: &egui::Context, from: egui::Pos2, by: egui::Vec2) {
        for step in 1..=3 {
            let at = from + by * step as f32 / 3.0;
            pass(window, ctx, vec![egui::Event::PointerMoved(at)]);
        }
    }

    /// An image opened while the middle button of a pan is still down is not dragged by it.
    #[test]
    fn a_new_image_is_not_moved_by_a_pan_begun_on_the_old_one() {
        let folder = Folder::new();
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let (mut window, ctx, _) = window_with_image(&path);
        let at = start_middle_drag(&mut window, &ctx);
        let name = window.active().panes()[1].name;
        crate::viewer::open_image_target(&mut window, name, path).unwrap();
        let started = Instant::now();
        let fresh = loop {
            if let Some(rect) = image_rect(&painted(&mut window, &ctx, vec![])) {
                break rect;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(ctx.input(|i| i.pointer.middle_down()));
        travel(&mut window, &ctx, at, egui::vec2(0.0, -45.0));
        assert_eq!(settle(&mut window, &ctx), fresh, "the old pan moved it");
    }

    /// A pan whose button was let go while the drawer showed Changes does not resume when the
    /// image comes back with that button down again.
    #[test]
    fn a_pan_released_while_the_image_was_hidden_does_not_resume() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let at = start_middle_drag(&mut window, &ctx);
        crate::viewer::activate(&mut window, false);
        let console = window.active().panes()[0]
            .painted_console
            .as_ref()
            .unwrap()
            .area
            .center();
        for pressed in [false, true] {
            pass(
                &mut window,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos: console,
                    button: egui::PointerButton::Middle,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        crate::viewer::activate(&mut window, true);
        let before = settle(&mut window, &ctx);
        assert!(ctx.input(|i| i.pointer.middle_down()));
        travel(&mut window, &ctx, at, egui::vec2(-30.0, -45.0));
        assert_eq!(settle(&mut window, &ctx), before, "the hidden pan resumed");
    }

    /// A stray left click in the middle of a middle-button pan neither stops it nor takes it over.
    #[test]
    fn a_stray_press_during_a_pan_does_not_stop_it() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let before = settle(&mut window, &ctx);
        let from = start_middle_drag(&mut window, &ctx);
        let by = egui::vec2(-30.0, -45.0);
        let middle = from + by / 2.0;
        pass(&mut window, &ctx, vec![egui::Event::PointerMoved(middle)]);
        for pressed in [true, false] {
            pass(
                &mut window,
                &ctx,
                vec![egui::Event::PointerButton {
                    pos: middle,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        travel(&mut window, &ctx, middle, by / 2.0);
        let after = settle(&mut window, &ctx);
        let travelled = by + egui::vec2(0.0, -10.0);
        assert!(
            (after.min - (before.min + travelled)).length() < 0.5,
            "{before:?} -> {after:?}"
        );
    }

    /// A Space the drawer owned, then the drawer closed with Escape, and Space let go and pressed
    /// again in a console: when the image comes back with that Space still down, it is the
    /// console's, so no hand shows and a left drag does not pan.
    #[test]
    fn a_space_pressed_while_the_drawer_was_closed_does_not_pan_when_it_returns() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let at = window.sidebar.last_dock.unwrap().center();
        pass(&mut window, &ctx, vec![egui::Event::PointerMoved(at)]);
        let owned = pass(&mut window, &ctx, vec![space(true)]);
        assert_eq!(owned.platform_output.cursor_icon, egui::CursorIcon::Grab);
        let escape = |pressed| egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        pass(&mut window, &ctx, vec![escape(true)]);
        pass(&mut window, &ctx, vec![escape(false)]);
        assert_eq!(crate::sidebar::active(&window), None);
        pass(&mut window, &ctx, vec![space(false)]);
        pass(&mut window, &ctx, vec![space(true)]);
        crate::sidebar::toggle(&mut window, crate::sidebar::Tool::Markdown, &ctx);
        let before = settle(&mut window, &ctx);
        assert!(ctx.input(|i| i.key_down(egui::Key::Space)));
        let hover = pass(&mut window, &ctx, vec![egui::Event::PointerMoved(at)]);
        assert_ne!(hover.platform_output.cursor_icon, egui::CursorIcon::Grab);
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
