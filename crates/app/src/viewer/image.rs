//! The drawer's image: a PNG or JPEG a console printed, or Files opened, one per console and shown
//! in the drawer's image tab while that console has the focus (D72). It is decoded once per open
//! and never refreshed, so a program that overwrites the same screenshot name does not change what
//! is on screen.
use super::Source;
use super::load::{Job, Picture};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

mod flatten;
mod marks;
pub(crate) mod send;
mod tools;
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

/// What a console holds of images: its own, and the copies its Sends wrote into its folder.
#[derive(Default)]
pub(crate) struct Console {
    pub(super) slot: Slot,
    pub(super) annotated: send::Written,
}

/// One console's image, with its marks and what was said about them.
#[derive(Default)]
pub(crate) struct Slot {
    document: Option<Opened>,
    /// A decode whose image was replaced or closed before it finished. A read stuck on an
    /// unresponsive filesystem cannot be cancelled, so its handle is kept and no other decode
    /// starts in this slot until it ends: repeated opens cannot pile up readers, and a stuck read
    /// holds up only its own console.
    abandoned: Option<(PathBuf, Job<Picture>)>,
    /// Why the last thing asked of the marks was refused, or what became of them.
    notice: Option<String>,
    /// What a cancelled Send's worker left to say when it ended, until a drawer lays this
    /// console's image out: the image may be closed by then, and the next one opened clears the
    /// notice it would have gone to.
    unshown: Option<String>,
}

/// What the drawer keeps across consoles: one pointer, one Send at a time, one count of openings.
#[derive(Default)]
pub(crate) struct Viewer {
    /// The button whose drag pans the image, latched when it went down.
    panning: Option<egui::PointerButton>,
    /// The frame an image was last painted in, and which opening it was: a pan or a stroke begun
    /// on one console's image must not carry over to another's.
    painted: Option<(u64, u64)>,
    send: send::Sending,
    /// How many images were opened, which names each opening: see [`send::Request::document`].
    opened: u64,
}

struct Opened {
    id: u64,
    origin: Source,
    path: PathBuf,
    load: Load,
    /// Screen points per pixel of the original; `None` fits the drawer's width.
    scale: Option<f32>,
    offset: egui::Vec2,
    /// The view's size and the scale the last frame it was painted, so a resize puts the image
    /// back inside and a fitted image keeps the part being read.
    view: Option<(egui::Vec2, f32)>,
    marks: marks::Marks,
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
    /// Replaces the image `slot` held with `path`, read from `origin`'s console.
    pub(super) fn open(&mut self, slot: &mut Slot, origin: Source, path: PathBuf) {
        self.panning = None;
        self.opened += 1;
        slot.open(self.opened, origin, path);
    }

    /// Whether a Send is between its click and its Enter.
    #[cfg(test)]
    pub(super) fn sending(&self) -> bool {
        self.send.busy()
    }

    /// Whether Send has work for the input pass: a Send on its way, or a cancelled one's worker.
    pub(super) fn send_pending(&self) -> bool {
        self.send.pending()
    }
}

impl Slot {
    /// Replaces the image with `path`, opened as `id`. A relative path is one whose console's
    /// directory is unknown: it is refused and says so.
    fn open(&mut self, id: u64, origin: Source, path: PathBuf) {
        self.clear();
        self.notice = None;
        let load = if path.is_relative() {
            Load::Failed("This console has not published its working directory. Open an absolute image path or enable OSC 7 in the shell.".into())
        } else {
            Load::Pending
        };
        self.document = Some(Opened {
            id,
            origin,
            path,
            load,
            scale: None,
            offset: egui::Vec2::ZERO,
            view: None,
            marks: marks::Marks::default(),
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

    /// Whether there is an image to paint.
    #[cfg(test)]
    pub(super) fn shown(&self) -> bool {
        self.document.is_some()
    }

    /// Whether the image carries marks or notes that were not sent, which closing it would lose.
    pub(super) fn has_marks(&self) -> bool {
        self.document.as_ref().is_some_and(|d| !d.marks.is_empty())
    }

    pub(super) fn say(&mut self, notice: impl Into<String>) {
        self.notice = Some(notice.into());
    }

    /// What the copy is drawn from: the original at full resolution and the marks as they are.
    fn to_flatten(&self) -> Option<(Arc<image::RgbaImage>, Vec<marks::Shape>)> {
        let marks = self.document.as_ref()?.marks.marks().to_vec();
        Some((Arc::clone(self.original()?), marks))
    }

    /// Which opening of an image is shown.
    fn document(&self) -> Option<u64> {
        Some(self.document.as_ref()?.id)
    }

    /// The marks reached their console: they are forgotten, and the drawer says where they went.
    fn sent(&mut self) {
        if let Some(document) = self.document.as_mut() {
            document.marks.clear();
            self.notice = Some(format!("Sent to {}.", document.origin.label));
        }
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

    #[cfg(test)]
    pub(crate) fn loaded(&self) -> bool {
        self.original().is_some()
    }

    /// Makes the image's decode one that has not finished, for as long as the returned sender
    /// lives.
    #[cfg(test)]
    pub(crate) fn stall(&mut self) -> std::sync::mpsc::Sender<Result<Picture, String>> {
        let (job, sender) = super::load::stalled();
        self.document.as_mut().unwrap().load = Load::Decoding(job);
        sender
    }
}

impl Viewer {
    /// Paints `slot`'s image over the drawer's body, fitted to its width until the person zooms,
    /// and moved by the wheel or a drag that pans it; there are no scroll bars. A plain left drag
    /// draws with the chosen tool. `space` is whether Space is held and the drawer's, which is when
    /// Space + left drag pans.
    pub(super) fn show(
        &mut self,
        slot: &mut Slot,
        ui: &mut egui::Ui,
        theme: crate::theme::Theme,
        space: bool,
    ) {
        let frame = ui.ctx().cumulative_frame_nr();
        // A drag whose release came while this image was not painted must not resume on return.
        let opening = slot.document();
        if self
            .painted
            .is_none_or(|(last, painted)| frame > last + 1 || Some(painted) != opening)
        {
            self.panning = None;
            if let Some(document) = slot.document.as_mut() {
                document.marks.cancel();
            }
        }
        let Some(size) = slot.size() else {
            let message = slot.message();
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
        // Taken up only by a frame that lays the column out, where the notice is painted.
        if let Some(said) = slot.unshown.take() {
            send::join(&mut slot.notice, said);
        }
        let panning = &mut self.panning;
        let notice = slot.notice.as_deref();
        let sending = &mut self.send;
        let Some(Opened {
            id,
            origin,
            load: Load::Ready { gpu, .. },
            scale: chosen,
            offset,
            view: seen,
            marks,
            ..
        }) = slot.document.as_mut()
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
        // The tools and notes are laid out before the view is taken: they share the image's layer,
        // so a click on them inside the view would read as a stroke on the image.
        let something_to_send = !marks.is_empty();
        let body = ui.available_rect_before_wrap();
        let refused = theme.panel_color(Some(panel_contract::Mark::Block), None);
        let send =
            something_to_send.then(|| (origin.label.as_str(), sending.busy_for(*id), sending.parked()));
        let fixed = tools::Fixed::new(ui.ctx(), ui.style(), body, notice, refused, send);
        let layout = tools::split(body, tools::rows(marks), fixed.height(), something_to_send);
        let view = layout.view;
        let roles = crate::ui::style::roles(ui.ctx(), &theme);
        // While a Send is on its way the marks stay as the click fixed them: the copy is drawn from
        // them a frame later, and a delivered Send clears them.
        tools::toolbar(ui, layout.bar, marks, sending.busy_for(*id), &roles);
        let panel = tools::Panel {
            fixed,
            roles,
            refused,
            document: *id,
            origin,
            sending,
        };
        sending.answer(tools::notes(ui, &layout, marks, panel), &mut slot.notice);
        // Undo, Clear or a note's first or last letter changed whether there is something to
        // send after the split was made: the next frame lays the button and its line out again.
        if marks.is_empty() == something_to_send {
            ui.ctx().request_repaint();
        }
        // Read again after the notes: a click on Send in this frame freezes what follows it.
        let frozen = sending.busy_for(*id);
        // Ctrl+Z reaches here only while the drawer holds the keys: a console that holds them
        // has taken it by now. Any text field with the keyboard keeps it, a note's own undo
        // among them.
        if !frozen && !ui.ctx().text_edit_focused() && ui.input_mut(marks::take_undo) {
            marks.undo();
        }
        let room = view.size();
        if !(room.x > 0.0 && room.y > 0.0 && room.is_finite()) {
            return;
        }
        // Only a frame that paints the image may keep a pan latched.
        self.painted = Some((frame, *id));
        // A menu or popup open above the image takes the pointer, and with it the wheel and a press.
        // egui's own hover test is not used: it reaches a few points into a popup's border. The
        // view tested is this frame's, but `layer_id_at` knows every area, the drawer's own among
        // them, only from the previous pass: on the first frame after the drawer is maximized,
        // resized or shown again, a gesture where it just grew can still be dropped. Where no area
        // is under the pointer `layer_id_at` answers none: that is the background.
        let on_image = |at: egui::Pos2| {
            let top = ui
                .ctx()
                .layer_id_at(at)
                .unwrap_or_else(egui::LayerId::background);
            view.contains(at) && top == ui.layer_id()
        };
        let over = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|&at| on_image(at));
        let over_image = over.is_some();
        // Judged outside `pan`'s read of the input: `layer_id_at` takes the lock that read holds.
        use egui::PointerButton::{Middle, Primary};
        let presses = ui.input(|i| [Middle, Primary].map(|b| viewport::pressed_at(i, b)));
        let pressed = presses.map(|at| at.is_some_and(on_image));
        let ppp = ui.ctx().pixels_per_point();
        let mut scale = chosen.unwrap_or_else(|| viewport::fit(room.x, size.x, ppp));
        if *seen != Some((room, scale)) {
            if chosen.is_none()
                && let Some((_, then)) = *seen
            {
                *offset *= scale / then;
            }
            *offset = viewport::clamp(*offset, None, viewport::shown(size, scale, room, ppp), room);
        }
        let zoom = ui.input(|i| i.zoom_delta());
        if zoom != 1.0
            && let Some(at) = over
        {
            let zoomed;
            (scale, zoomed) = viewport::zoom_around(scale, *offset, at - view.min, zoom, size);
            *offset = viewport::clamp(
                zoomed,
                Some(zoomed),
                viewport::shown(size, scale, room, ppp),
                room,
            );
            *chosen = Some(scale);
        }
        let (moved, hand) = ui.input(|i| viewport::pan(i, over_image, pressed, space, panning));
        let shown = viewport::shown(size, scale, room, ppp);
        *offset = viewport::clamp(*offset - moved, Some(*offset), shown, room);
        if over.is_some() {
            // Only what moved the image is taken; the rest stays for whoever reads the wheel later.
            *offset = ui.input_mut(|i| {
                let (to, taken) = viewport::wheel(i.smooth_scroll_delta, *offset, shown, room);
                i.smooth_scroll_delta -= taken;
                to
            });
        }
        *seen = Some((room, scale));
        if let Some(hand) = hand {
            ui.ctx().set_cursor_icon(hand);
        }
        let whole = egui::Rect::from_min_size(view.min - *offset, shown);
        let visible = whole.intersect(view);
        if !visible.is_positive() {
            return;
        }
        // The body's own rounded shape, textured with the image and clipped to where the image is,
        // takes the body's bottom corners wherever the image reaches them. The image's own rect
        // with those corners would not: epaint caps a radius at half a rect's shortest side.
        let uv = egui::Rect::from_min_max(
            ((view.min - whole.min) / shown).to_pos2(),
            ((view.max - whole.min) / shown).to_pos2(),
        );
        let corners = layout.corners(super::view::BODY_CORNER);
        ui.painter().with_clip_rect(visible).add(
            egui::epaint::RectShape::filled(view, corners, egui::Color32::WHITE)
                .with_texture(texture.id(), uv),
        );
        let placement = marks::Placement::new(whole, size);
        if frozen {
            marks.cancel();
        } else {
            let stroke = presses[1].filter(|_| pressed[1] && panning.is_none());
            ui.input(|i| marks.gesture(i, stroke, placement));
        }
        marks.paint(&ui.painter().with_clip_rect(view), placement);
    }
}

/// The slot of the console called `pane`, in whichever tab it is.
pub(super) fn slot_of(tabs: &mut [crate::tree::Tab], pane: usize) -> Option<&mut Slot> {
    tabs.iter_mut()
        .find_map(|tab| tab.pane_called(pane))
        .map(|pane| &mut pane.image.slot)
}

/// The slot the image tab shows: the focused console's, in the tab on show.
pub(super) fn focused(window: &crate::Window) -> Option<&Slot> {
    let tab = window.active();
    let pane = tab.panes().into_iter().find(|pane| pane.name == tab.focus)?;
    Some(&pane.image.slot)
}

/// The same, to change it, borrowing only the tabs so the drawer's own state stays at hand.
pub(super) fn focused_mut(tabs: &mut [crate::tree::Tab], active: usize) -> Option<&mut Slot> {
    let tab = tabs.get_mut(active)?;
    let focus = tab.focus;
    Some(&mut tab.pane_called(focus)?.image.slot)
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

    /// The image the image tab shows: the focused console's.
    fn slot(window: &crate::Window) -> &Slot {
        focused(window).expect("the tab on show has a focused console")
    }

    fn slot_mut(window: &mut crate::Window) -> &mut Slot {
        focused_mut(&mut window.tabs, window.active).expect("the tab on show has a focused console")
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
        let mut viewer = Slot::default();
        viewer.open(1, source(), path.clone());
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
        pass_with(window, ctx, events, |_| {})
    }

    /// One real application pass with `after` drawing on top of it, as a later layer does.
    fn pass_with(
        window: &mut crate::Window,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        after: impl FnMut(&egui::Context),
    ) -> egui::FullOutput {
        pass_on(window, ctx, area(), events, after)
    }

    /// The same, on a screen of `screen`.
    fn pass_on(
        window: &mut crate::Window,
        ctx: &egui::Context,
        screen: egui::Rect,
        events: Vec<egui::Event>,
        mut after: impl FnMut(&egui::Context),
    ) -> egui::FullOutput {
        let mut frame = eframe::Frame::_new_kittest();
        eframe::App::logic(window, ctx, &mut frame);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                max_texture_side: Some(MAX_SIDE),
                events,
                ..Default::default()
            },
            |ui| {
                eframe::App::ui(window, ui, &mut frame);
                after(ui.ctx());
            },
        );
        output.textures_delta.clear();
        output
    }

    /// The image the drawer painted: the one rect filled with a texture, at any depth of nested
    /// shapes, its clip, and the whole image's place, worked back from the part in view and its
    /// texture coordinates. More than one textured rect fails, rather than picking one.
    fn image_shape(
        shapes: &[egui::epaint::ClippedShape],
    ) -> Option<(egui::Rect, &egui::epaint::RectShape, egui::Rect)> {
        fn walk<'a>(
            shape: &'a egui::Shape,
            clip: egui::Rect,
            found: &mut Vec<(&'a egui::epaint::RectShape, egui::Rect)>,
        ) {
            match shape {
                egui::Shape::Rect(rect) if rect.brush.is_some() => found.push((rect, clip)),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, clip, found);
                    }
                }
                _ => {}
            }
        }
        let mut found = Vec::new();
        for clipped in shapes {
            walk(&clipped.shape, clipped.clip_rect, &mut found);
        }
        assert!(found.len() <= 1, "{} textured rects painted", found.len());
        let (rect, clip) = found.pop()?;
        let uv = rect.brush.as_ref()?.uv;
        let size = rect.rect.size() / uv.size();
        let min = rect.rect.min - uv.min.to_vec2() * size;
        // Worked back through a division, two passes over the same image can differ in their last
        // bits; a 1/1024 point grid hides that and nothing a test measures.
        let grid = |v: egui::Vec2| (v * 1024.0).round() / 1024.0;
        let whole = egui::Rect::from_min_size(grid(min.to_vec2()).to_pos2(), grid(size));
        Some((whole, rect, clip))
    }

    /// Where the drawer painted the whole image, also the parts outside the view.
    fn image_rect(shapes: &[egui::epaint::ClippedShape]) -> Option<egui::Rect> {
        Some(image_shape(shapes)?.0)
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
    fn texture_size(viewer: &Slot) -> Option<[usize; 2]> {
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
        window_with_image_in(path, ["sleep 30", "sleep 30"])
    }

    /// The same, with the two consoles running `programs`.
    fn window_with_image_in(
        path: &Path,
        programs: [&str; 2],
    ) -> (crate::Window, egui::Context, Duration) {
        window_with_image_in_tabs(path, &[programs])
    }

    /// The same, with one tab per pair of `programs`; the image is pinned to the first tab's
    /// second console.
    fn window_with_image_in_tabs(
        path: &Path,
        programs: &[[&str; 2]],
    ) -> (crate::Window, egui::Context, Duration) {
        let mut window = crate::tests::window_of_tabs(programs.iter().copied());
        // With the bundled fonts, as the application has them: the tools are icons of the set.
        let ctx = crate::ui::icons::installed_for_tests(1.0);
        painted(&mut window, &ctx, vec![]);
        let name = window.active().panes()[1].name;
        crate::viewer::open_diff_for_pane(&mut window, name);
        crate::viewer::open_image_target(&mut window, name, path.to_owned()).unwrap();
        let started = Instant::now();
        // A guard against a decode that never ends, not a speed claim: an unoptimised 2400×1600
        // decode took 1.45 s alone and up to 3.9 s beside this module's other decodes, so a 5 s
        // bound failed 19 tests whenever the machine was busy, before and after the rebase alike.
        while image_rect(&painted(&mut window, &ctx, vec![])).is_none() {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "{:?}",
                slot(&window).message()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let [width, height] = texture_size(slot(&window)).unwrap();
        let (wide, high) = slot(&window).original().unwrap().dimensions();
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
        let first = Arc::clone(slot(&window).original().unwrap());
        write_png(&path, 64, 32);
        let dock = window.sidebar.last_dock.unwrap();
        for (action, tool) in [
            (StripAction::Changes, Tool::Git),
            (StripAction::Image, Tool::Image),
        ] {
            let place = crate::viewer::strip_place(&window, dock, action).unwrap();
            click(&mut window, &ctx, place);
            assert_eq!(active(&window), Some(tool));
        }
        crate::sidebar::close(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        assert_eq!(active(&window), None);
        crate::sidebar::toggle(&mut window, Tool::Image, &ctx);
        let until = Instant::now() + (took * 5).max(Duration::from_millis(300));
        let mut shapes = Vec::new();
        while Instant::now() < until {
            shapes = painted(&mut window, &ctx, vec![]);
            std::thread::sleep(Duration::from_millis(5));
        }
        let now = slot(&window).original().unwrap();
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
        write_png(&path, 300, 60);
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

    /// A window with the 300x60 fixture fitted, which is smaller than the drawer on both axes,
    /// and the pointer resting at `at(image)`, with Ctrl held.
    fn small_window(
        folder: &Folder,
        at: impl Fn(egui::Rect) -> egui::Pos2,
    ) -> (crate::Window, egui::Context, egui::Rect, egui::Pos2) {
        let path = folder.0.join("small.png");
        write_png(&path, 300, 60);
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
            egui::pos2(image.center().x, image.bottom() + 100.0)
        });
        let view = image_view(&window, &ctx);
        assert!(view.contains(at), "{at:?} outside {view:?}");
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

    /// A drawer's image machinery and one console's slot, painted with no window around them.
    #[derive(Default)]
    struct Alone {
        image: Viewer,
        slot: Slot,
    }

    impl Alone {
        fn show(&mut self, ui: &mut egui::Ui, theme: crate::theme::Theme, space: bool) {
            self.image.show(&mut self.slot, ui, theme, space);
        }
    }

    impl std::ops::Deref for Alone {
        type Target = Slot;
        fn deref(&self) -> &Slot {
            &self.slot
        }
    }

    impl std::ops::DerefMut for Alone {
        fn deref_mut(&mut self) -> &mut Slot {
            &mut self.slot
        }
    }

    /// An image holding `original`, decoded and not yet shown.
    fn ready(original: image::RgbaImage) -> Alone {
        let mut alone = Alone::default();
        alone.image.open(&mut alone.slot, source(), PathBuf::from("/shot.png"));
        alone.slot.document.as_mut().unwrap().load = Load::Ready {
            original: Arc::new(original),
            gpu: Gpu::Waiting(egui::ColorImage::filled([4, 2], egui::Color32::RED)),
        };
        alone
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
        let mut viewer = Slot::default();
        viewer.open(1, source(), PathBuf::from("shot.png"));
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
        assert!(!slot(&window).loaded());
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
        let finish = slot_mut(&mut window).stall();
        crate::viewer::open_image_target(&mut window, name, second.clone()).unwrap();
        let still = format!("Still reading {}…", first.display());
        wait_for_text(&mut window, &ctx, &still);
        let dock = window.sidebar.last_dock.unwrap();
        let close = crate::viewer::strip_place(&window, dock, StripAction::CloseImage).unwrap();
        click(&mut window, &ctx, close);
        assert!(!slot(&window).shown());
        crate::viewer::open_image_target(&mut window, name, second.clone()).unwrap();
        wait_for_text(&mut window, &ctx, &still);
        assert!(slot(&window).abandoned.is_some());
        drop(finish);
        crate::viewer::open_image_target(&mut window, name, second).unwrap();
        let started = Instant::now();
        while !slot(&window).loaded() {
            painted(&mut window, &ctx, vec![]);
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "{:?}",
                slot(&window).message()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(slot(&window).original().unwrap().dimensions(), (48, 16));
        assert!(slot(&window).abandoned.is_none());
    }

    /// The image has a tab of its own: closing the document tab beside it leaves the image painted,
    /// and closing the image's tab forgets it and hands the drawer to the next tab, here Changes.
    #[test]
    fn closing_the_document_tab_keeps_the_image_and_closing_its_own_forgets_it() {
        use crate::sidebar::{Tool, active};
        use crate::viewer::StripAction;
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 64, 32);
        let guide = folder.0.join("guide.md");
        std::fs::write(&guide, "# Guide\n").unwrap();
        let (mut window, ctx, _) = window_with_image(&path);
        let name = slot(&window).origin().unwrap().pane;
        assert!(crate::viewer::open_document_target(
            &mut window,
            name,
            guide,
            None
        ));
        painted(&mut window, &ctx, vec![]);
        let place = |window: &crate::Window, action| {
            crate::viewer::strip_place(window, window.sidebar.last_dock.unwrap(), action)
        };
        let at = place(&window, StripAction::Image).unwrap();
        click(&mut window, &ctx, at);
        assert_eq!(active(&window), Some(Tool::Image));
        assert!(image_rect(&painted(&mut window, &ctx, vec![])).is_some());
        let at = place(&window, StripAction::CloseDocument).unwrap();
        click(&mut window, &ctx, at);
        assert_eq!(active(&window), Some(Tool::Image));
        assert!(place(&window, StripAction::Document).is_none());
        assert!(
            image_rect(&painted(&mut window, &ctx, vec![])).is_some(),
            "closing the document tab took the image off the drawer"
        );
        let at = place(&window, StripAction::CloseImage).unwrap();
        click(&mut window, &ctx, at);
        assert_eq!(active(&window), Some(Tool::Git));
        assert_eq!(crate::viewer::shown_image(&window), None);
        assert!(place(&window, StripAction::Image).is_none());
        assert!(image_rect(&painted(&mut window, &ctx, vec![])).is_none());
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

    /// Whether two places of the image are the same but for the last bits of the division that
    /// works them out: the view's own size changes when the first mark brings Send's rows.
    fn same_place(a: egui::Rect, b: egui::Rect) -> bool {
        (a.min - b.min).length() < 0.01 && (a.max - b.max).length() < 0.01
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
        assert!(
            same_place(plain, before),
            "a plain left drag panned the image"
        );
        let cursors = drag(&mut window, &ctx, egui::PointerButton::Middle, by);
        assert_eq!(cursors, [egui::CursorIcon::Grabbing; 3]);
        let middle = settle(&mut window, &ctx);
        assert!(
            (middle.min - (before.min + by)).length() < 0.5,
            "{before:?} -> {middle:?}"
        );
        assert!((middle.size() - before.size()).length() < 0.01);
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
        let after = settle(&mut window, &ctx);
        assert!(same_place(after, before), "a console's Space panned");
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

    /// There are no scroll bars: Space + left drag at the image's right edge, where a bar would
    /// sit, pans the image the whole way, with the hand showing.
    #[test]
    fn a_space_drag_at_the_drawer_edge_pans_the_image() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let before = settle(&mut window, &ctx);
        let view = image_view(&window, &ctx);
        let edge = egui::pos2(view.right() - 3.0, view.center().y);
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

    /// The clip the image was painted under.
    fn image_clip(shapes: &[egui::epaint::ClippedShape]) -> Option<egui::Rect> {
        Some(image_shape(shapes)?.2)
    }

    /// The top of the drawer's body, below its strip.
    fn body_top(window: &crate::Window, dock: egui::Rect) -> f32 {
        let tabs = crate::viewer::drawer::tabs(window);
        crate::viewer::drawer::Strip::framed(dock, &tabs)
            .rect
            .bottom()
    }

    /// The drawer's body, below its strip.
    fn body(window: &crate::Window) -> egui::Rect {
        let dock = window.sidebar.last_dock.unwrap();
        let inner = dock.shrink(1.0);
        egui::Rect::from_min_max(egui::pos2(inner.left(), body_top(window, dock)), inner.max)
    }

    /// How the drawer's body was split around the image on the last pass.
    fn layout(window: &crate::Window, ctx: &egui::Context) -> tools::Layout {
        let marks = &slot(&window).document.as_ref().unwrap().marks;
        let something_to_send = !marks.is_empty();
        let label = slot(&window).origin().unwrap().label.as_str();
        let sending = &window.image.send;
        let send = something_to_send.then(|| (label, sending.busy(), sending.parked()));
        let notice = slot(&window).notice.as_deref();
        let style = ctx.global_style();
        let refused = egui::Color32::RED;
        let fixed = tools::Fixed::new(ctx, &style, body(window), notice, refused, send);
        tools::split(
            body(window),
            tools::rows(marks),
            fixed.height(),
            something_to_send,
        )
    }

    /// Where the image is shown: the drawer's body without the tools, the notes and the line Send
    /// types.
    fn image_view(window: &crate::Window, ctx: &egui::Context) -> egui::Rect {
        layout(window, ctx).view
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
        let body = body_top(&window, dock);
        assert!(
            clip.top() >= body,
            "{clip:?} reaches the strip above {body}"
        );
    }

    /// Beside a wide drawer the notes take a column of their own: the image keeps the same height
    /// however many markers there are, and the notes are painted to its right.
    #[test]
    fn the_notes_beside_the_image_take_none_of_its_height() {
        let folder = Folder::new();
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let (mut window, ctx, _) = window_with_image(&path);
        marks_mut(&mut window).tool = marks::Tool::Marker;
        let mut first = None;
        for n in 0..5 {
            let at = image_view(&window, &ctx).center() + egui::vec2(n as f32 * 30.0 - 60.0, 0.0);
            press(&mut window, &ctx, at);
            let shapes = painted(&mut window, &ctx, vec![]);
            let clip = image_clip(&shapes).unwrap();
            assert_eq!(marks_of(&window).numbered().count(), n + 1);
            let first = *first.get_or_insert(clip);
            assert_eq!(
                (clip.top(), clip.bottom()),
                (first.top(), first.bottom()),
                "with {} markers",
                n + 1
            );
            let (_, label) = text_rect(&shapes, |t| t == "General note").unwrap();
            assert!(label.left() >= clip.right(), "{label:?} beside {clip:?}");
        }
    }

    /// In a window too narrow for the column beside the image, the notes are painted under it, and
    /// the image, no longer reaching the body's foot, takes none of its rounded corners there.
    #[test]
    fn a_narrow_window_puts_the_notes_under_the_image() {
        let folder = Folder::new();
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let (mut window, ctx, _) = window_with_image(&path);
        let narrow = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(tools::COLUMN + tools::NARROWEST - 40.0, 700.0),
        );
        marks_mut(&mut window).tool = marks::Tool::Marker;
        let mut shapes = Vec::new();
        for _ in 0..3 {
            shapes = pass_on(&mut window, &ctx, narrow, vec![], |_| {}).shapes;
        }
        let dock = window.sidebar.last_dock.unwrap();
        assert!(dock.width() < tools::COLUMN + tools::NARROWEST, "{dock:?}");
        let (_, rect, clip) = image_shape(&shapes).unwrap();
        let at = clip.center();
        for pressed in [true, false] {
            let events = vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            pass_on(&mut window, &ctx, narrow, events, |_| {});
        }
        assert_eq!(marks_of(&window).numbered().count(), 1, "{rect:?}");
        let shapes = pass_on(&mut window, &ctx, narrow, vec![], |_| {}).shapes;
        let (_, rect, clip) = image_shape(&shapes).unwrap();
        assert_eq!((rect.corner_radius.sw, rect.corner_radius.se), (0, 0));
        let (_, label) = text_rect(&shapes, |t| t == "General note").unwrap();
        assert!(label.top() >= clip.bottom(), "{label:?} under {clip:?}");
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
        for _ in 0..8 {
            drag(
                &mut window,
                &ctx,
                egui::PointerButton::Middle,
                egui::vec2(150.0, 150.0),
            );
        }
        let large = image_view(&window, &ctx);
        let cornered = settle(&mut window, &ctx);
        assert!(
            (cornered.right_bottom() - large.right_bottom()).length() < 2.0,
            "{cornered:?} in {large:?}"
        );
        crate::sidebar::toggle_maximize(&mut window);
        let back = settle(&mut window, &ctx);
        let small = image_view(&window, &ctx);
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

    /// Opens `path` in the console `name` as Ctrl+click does, and paints until its image is.
    fn open_in(
        window: &mut crate::Window,
        ctx: &egui::Context,
        name: usize,
        path: &Path,
    ) -> egui::Rect {
        crate::viewer::open_image_target(window, name, path.to_owned()).unwrap();
        let started = Instant::now();
        loop {
            if let Some(rect) = image_rect(&painted(window, ctx, vec![])) {
                return rect;
            }
            // A hang guard, not a speed claim.
            assert!(started.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// The image tab shows the focused console's image: focusing the other console shows its own,
    /// and focusing back shows the first as it was left, at its zoom and with its notes.
    #[test]
    fn the_image_tab_follows_the_focus_and_each_console_keeps_its_own() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let names: Vec<_> = window.active().panes().iter().map(|p| p.name).collect();
        let zoomed = settle(&mut window, &ctx);
        let small = folder.0.join("small.png");
        write_png(&small, 120, 60);
        let first = open_in(&mut window, &ctx, names[0], &small);
        assert_eq!(window.active().focus, names[0]);
        assert_eq!(first.size(), egui::vec2(120.0, 60.0));
        marks_mut(&mut window).general = "on the first console".into();
        let noted = |shapes: &[egui::epaint::ClippedShape]| {
            texts(shapes, area())
                .iter()
                .any(|t| t.contains("on the first console"))
        };
        assert!(noted(&painted(&mut window, &ctx, vec![])));
        let focus = |window: &mut crate::Window, name: usize| {
            let active = window.active;
            window.tabs[active].focus = name;
        };
        focus(&mut window, names[1]);
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(same_place(image_rect(&shapes).unwrap(), zoomed));
        assert!(!noted(&shapes), "the first console's note shows on the second's image");
        focus(&mut window, names[0]);
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(noted(&shapes), "the first console's note was lost");
        assert_eq!(image_rect(&shapes).unwrap().size(), first.size());
        focus(&mut window, names[1]);
        assert!(
            same_place(settle(&mut window, &ctx), zoomed),
            "the second console's zoom was lost"
        );
    }

    /// A pan latched on one console's image does not carry over to another console's when the
    /// focus moves with the button still down.
    #[test]
    fn a_pan_begun_on_one_consoles_image_does_not_move_another_consoles() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let names: Vec<_> = window.active().panes().iter().map(|p| p.name).collect();
        let second = settle(&mut window, &ctx);
        let other = folder.0.join("other.png");
        write_png(&other, 2400, 1600);
        open_in(&mut window, &ctx, names[0], &other);
        let at = start_middle_drag(&mut window, &ctx);
        let active = window.active;
        window.tabs[active].focus = names[1];
        assert!(ctx.input(|i| i.pointer.middle_down()));
        travel(&mut window, &ctx, at, egui::vec2(-30.0, -45.0));
        assert!(
            same_place(settle(&mut window, &ctx), second),
            "the first console's pan moved the second console's image"
        );
    }

    /// Closing a console takes its image and its unsent marks with it, with no question; the
    /// console left keeps its own image, painted once it has the focus. Closing a tab does the
    /// same for the consoles in it.
    #[test]
    fn closing_a_console_or_its_tab_takes_its_image_and_marks_with_no_question() {
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 300, 100);
        let (mut window, ctx, _) =
            window_with_image_in_tabs(&path, &[["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let kept = window.active().panes()[1].name;
        let kept_rect = image_rect(&painted(&mut window, &ctx, vec![])).unwrap();
        let small = folder.0.join("small.png");
        write_png(&small, 120, 60);
        let noted = |shapes: &[egui::epaint::ClippedShape]| {
            texts(shapes, area()).iter().any(|t| t.contains("unsent"))
        };
        let elsewhere = window.tabs[1].panes()[0].name;
        open_in(&mut window, &ctx, elsewhere, &small);
        assert_eq!(window.active, 1);
        marks_mut(&mut window).general = "unsent".into();
        window.close_the_active_tab(&ctx);
        assert_eq!(window.tabs.len(), 1, "closing the tab asked something");
        let beside = window.active().panes()[0].name;
        open_in(&mut window, &ctx, beside, &small);
        marks_mut(&mut window).general = "unsent".into();
        assert!(noted(&painted(&mut window, &ctx, vec![])));
        window.close_a_pane_called(beside, &ctx).unwrap();
        assert!(window.all_panes().all(|pane| pane.name != beside));
        assert_eq!(window.active().focus, kept);
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(same_place(image_rect(&shapes).unwrap(), kept_rect));
        assert!(!noted(&shapes), "a closed console's marks are still painted");
        assert_eq!(slot(&window).notice, None);
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
            // A hang guard, not a speed claim: this 2400×1600 debug decode takes seconds under
            // the full suite's load, like the one `window_with_image` waits for.
            assert!(started.elapsed() < Duration::from_secs(30));
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
        // The image's own console: a press on another would focus it, and the image tab with it.
        let console = window.active().panes()[1]
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
        crate::viewer::show_image(&mut window, true);
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
        crate::sidebar::toggle(&mut window, crate::sidebar::Tool::Image, &ctx);
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
        let after = settle(&mut window, &ctx);
        assert!(same_place(after, before), "a console's Space panned");
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

    fn plain_wheel(at: egui::Pos2, delta: egui::Vec2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta,
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    /// A popup open over the zoomed image, in the drawer's own order as the chrome's menus are and
    /// above it as Files' search is: the wheel over it is left to the popup and the image stays
    /// where it is. Over the image itself the wheel moves it and is taken, so nothing later in the
    /// frame scrolls with the same turn.
    #[test]
    fn the_wheel_over_a_popup_on_the_image_is_the_popups() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let at = window.sidebar.last_dock.unwrap().center();
        let place = egui::Rect::from_center_size(at, egui::vec2(220.0, 140.0));
        let turn = |window: &mut crate::Window, after: &mut dyn FnMut(&egui::Context)| {
            let mut before = None;
            for _ in 0..3 {
                let output = pass_with(
                    window,
                    &ctx,
                    vec![egui::Event::PointerMoved(at)],
                    &mut *after,
                );
                before = image_rect(&output.shapes);
            }
            let mut events = plain_wheel(at, egui::vec2(0.0, -3.0));
            let mut now = None;
            for _ in 0..30 {
                let output = pass_with(window, &ctx, std::mem::take(&mut events), &mut *after);
                now = image_rect(&output.shapes);
            }
            (before.unwrap(), now.unwrap())
        };
        for order in [egui::Order::Foreground, egui::Order::Tooltip] {
            // What the popup was left of the wheel, after the drawer painted below it.
            let mut received = egui::Vec2::ZERO;
            let mut popup = |ctx: &egui::Context| {
                egui::Area::new(egui::Id::new(("test-popup", order)))
                    .order(order)
                    .fixed_pos(place.min)
                    .show(ctx, |ui| {
                        ui.allocate_exact_size(place.size(), egui::Sense::hover());
                        received += ui.input(|i| i.smooth_scroll_delta);
                    });
            };
            let (before, after) = turn(&mut window, &mut popup);
            assert_eq!(
                after, before,
                "{order:?}: the wheel over the popup moved the image"
            );
            assert!(received.y < 0.0, "{order:?}: the popup was left no wheel");
        }
        let mut left = egui::Vec2::ZERO;
        let (before, after) = turn(&mut window, &mut |ctx: &egui::Context| {
            left += ctx.input(|i| i.smooth_scroll_delta);
        });
        assert!(after.top() < before.top(), "{before:?} -> {after:?}");
        assert_eq!(
            left,
            egui::Vec2::ZERO,
            "the image's wheel was left for later"
        );
    }

    /// A wheel a few points inside a popup's border, over the image, is the popup's: egui's hover
    /// test reaches that far into a lower layer, and the image must not take the turn from there.
    #[test]
    fn the_wheel_just_inside_a_popup_border_is_the_popups() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let centre = window.sidebar.last_dock.unwrap().center();
        let place = egui::Rect::from_center_size(centre, egui::vec2(220.0, 140.0));
        let received = std::cell::Cell::new(egui::Vec2::ZERO);
        let mut popup = |ctx: &egui::Context| {
            egui::Area::new(egui::Id::new("test-popup-border"))
                .order(egui::Order::Foreground)
                .fixed_pos(place.min)
                .show(ctx, |ui| {
                    ui.allocate_exact_size(place.size(), egui::Sense::hover());
                    received.set(received.get() + ui.input(|i| i.smooth_scroll_delta));
                });
        };
        for at in [
            place.left_top() + egui::vec2(3.0, 3.0),
            place.left_center() + egui::vec2(2.0, 0.0),
            place.center_bottom() - egui::vec2(0.0, 4.0),
        ] {
            let mut before = None;
            for _ in 0..3 {
                let output = pass_with(
                    &mut window,
                    &ctx,
                    vec![egui::Event::PointerMoved(at)],
                    &mut popup,
                );
                before = image_rect(&output.shapes);
            }
            received.set(egui::Vec2::ZERO);
            let mut events = plain_wheel(at, egui::vec2(0.0, -3.0));
            let mut now = None;
            for _ in 0..30 {
                let output = pass_with(&mut window, &ctx, std::mem::take(&mut events), &mut popup);
                now = image_rect(&output.shapes);
            }
            assert_eq!(
                now, before,
                "{at:?}: the wheel inside the popup moved the image"
            );
            assert!(
                received.get().y < 0.0,
                "{at:?}: the popup was left no wheel"
            );
        }
    }

    /// A zoom on the very first frame the image is painted is not dropped: whether the pointer is
    /// on the image is judged against that frame's view, not one remembered from the frame before.
    #[test]
    fn a_zoom_on_the_first_frame_the_image_shows_is_not_dropped() {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let run = |viewer: &mut Alone, events| {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| viewer.show(ui, crate::theme::Theme::default(), false),
            );
            let rect = image_rect(&output.shapes);
            output.drop_without_applying_deltas();
            rect
        };
        let mut viewer = Alone::default();
        viewer.image.open(&mut viewer.slot, source(), PathBuf::from("/shot.png"));
        let at = egui::pos2(150.0, 50.0);
        // Loading, then decoded: no pass before this one painted the image.
        assert_eq!(run(&mut viewer, vec![egui::Event::PointerMoved(at)]), None);
        viewer = ready(image::RgbaImage::new(600, 120));
        // Fitted, 600 points wide: an 800-point view does not enlarge it.
        let first = run(&mut viewer, vec![egui::Event::Zoom(2.0)]).unwrap();
        assert!(first.width() > 900.0, "{first:?}");
    }

    /// A fitted screenshot shorter than the drawer, which a zoom around its centre carried across
    /// the drawer's top, comes back with the wheel; the wheel the other way never pushes it further
    /// out, and that turn, which moved nothing, is left for whoever reads the wheel later.
    #[test]
    fn the_wheel_brings_back_a_smaller_image_a_zoom_carried_across_an_edge() {
        let folder = Folder::new();
        let (mut window, ctx, _, at) = small_window(&folder, |image| image.center());
        painted(
            &mut window,
            &ctx,
            vec![ctrl_wheel(
                egui::MouseWheelUnit::Line,
                1.0,
                egui::TouchPhase::Move,
            )],
        );
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
        let zoomed = settle(&mut window, &ctx);
        let view = image_view(&window, &ctx);
        assert!(
            zoomed.top() < view.top() && zoomed.height() < view.height(),
            "{zoomed:?} in {view:?}"
        );
        let mut left = egui::Vec2::ZERO;
        let mut events = plain_wheel(at, egui::vec2(0.0, -1.0));
        for _ in 0..30 {
            pass_with(&mut window, &ctx, std::mem::take(&mut events), |ctx| {
                left += ctx.input(|i| i.smooth_scroll_delta);
            });
        }
        assert_eq!(settle(&mut window, &ctx), zoomed, "pushed further out");
        assert!(left.y < 0.0, "the cancelled turn was taken: {left:?}");
        painted(&mut window, &ctx, plain_wheel(at, egui::vec2(0.0, 1.0)));
        let back = settle(&mut window, &ctx);
        assert!(back.top() > zoomed.top(), "{zoomed:?} -> {back:?}");
        assert_eq!(back.size(), zoomed.size());
    }

    /// A wheel turned on both axes over an image taller than the drawer and narrower than it moves
    /// it down and takes only that axis: the sideways part is left for whoever reads the wheel
    /// later in the frame.
    #[test]
    fn the_image_takes_only_the_wheel_axes_it_applies() {
        let folder = Folder::new();
        let path = folder.0.join("tall.png");
        write_png(&path, 600, 4800);
        let (mut window, ctx, _) = window_with_image(&path);
        let before = settle(&mut window, &ctx);
        let dock = window.sidebar.last_dock.unwrap();
        assert!(
            before.width() < dock.width() && before.height() > dock.height(),
            "{before:?} in {dock:?}"
        );
        let at = egui::pos2(before.center().x, dock.center().y);
        let mut left = egui::Vec2::ZERO;
        let mut events = plain_wheel(at, egui::vec2(3.0, -3.0));
        let mut after = None;
        for _ in 0..30 {
            let output = pass_with(&mut window, &ctx, std::mem::take(&mut events), |ctx| {
                left += ctx.input(|i| i.smooth_scroll_delta);
            });
            after = image_rect(&output.shapes);
        }
        let after = after.unwrap();
        assert!(after.top() < before.top(), "{before:?} -> {after:?}");
        assert_eq!(after.left(), before.left());
        assert_eq!(left.y, 0.0, "the applied axis was left for later");
        assert!(left.x > 0.0, "the axis not applied was taken: {left:?}");
    }

    /// The wheel does not slide an image smaller than the drawer on either axis.
    #[test]
    fn the_wheel_leaves_an_image_smaller_than_the_drawer_where_it_is() {
        let folder = Folder::new();
        let (mut window, ctx, before, at) = small_window(&folder, |image| image.center());
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
        for delta in [egui::vec2(0.0, 3.0), egui::vec2(3.0, 0.0)] {
            painted(&mut window, &ctx, plain_wheel(at, delta));
            assert_eq!(settle(&mut window, &ctx), before, "{delta:?}");
        }
    }

    /// A pan whose button is still down does not resume after frames in which the view had no
    /// room for the image, as in a window so narrow that the drawer's frame leaves no width inside.
    #[test]
    fn a_pan_is_dropped_through_frames_with_no_room_for_the_image() {
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(2000, 2000));
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 400.0));
        let none = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(0.0, 400.0));
        let run = |viewer: &mut Alone, events, view: egui::Rect| {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(view));
                    viewer.show(&mut ui, crate::theme::Theme::default(), false);
                },
            );
            let rect = image_rect(&output.shapes);
            output.drop_without_applying_deltas();
            rect
        };
        let from = screen.center();
        let before = run(&mut viewer, vec![egui::Event::PointerMoved(from)], screen).unwrap();
        let press = egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Middle,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        };
        run(&mut viewer, vec![press], screen);
        assert_eq!(viewer.image.panning, Some(egui::PointerButton::Middle));
        let up = from - egui::vec2(0.0, 20.0);
        assert_eq!(
            run(&mut viewer, vec![egui::Event::PointerMoved(up)], none),
            None
        );
        let further = up - egui::vec2(0.0, 40.0);
        let after = run(
            &mut viewer,
            vec![egui::Event::PointerMoved(further)],
            screen,
        );
        assert_eq!(after, Some(before), "the pan resumed");
    }

    /// The 500×400 screen the viewer is laid out alone in.
    const SCREEN: egui::Rect = egui::Rect {
        min: egui::Pos2::ZERO,
        max: egui::pos2(500.0, 400.0),
    };

    /// One pass of `viewer` alone, laid out in `view`, under a foreground popup at `popup` when
    /// there is one; says where it painted the whole image.
    fn alone(
        ctx: &egui::Context,
        viewer: &mut Alone,
        events: Vec<egui::Event>,
        view: egui::Rect,
        popup: Option<egui::Rect>,
    ) -> Option<egui::Rect> {
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(SCREEN),
                events,
                ..Default::default()
            },
            |ui| {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
                viewer.show(&mut child, crate::theme::Theme::default(), false);
                if let Some(place) = popup {
                    egui::Area::new(egui::Id::new("test-popup-press"))
                        .order(egui::Order::Foreground)
                        .fixed_pos(place.min)
                        .show(ui.ctx(), |ui| {
                            ui.allocate_exact_size(place.size(), egui::Sense::hover());
                        });
                }
            },
        );
        let rect = image_rect(&output.shapes);
        output.drop_without_applying_deltas();
        rect
    }

    fn button(button: egui::PointerButton, pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Moves the pointer up from `from` in three passes of 20 points, with whatever buttons are
    /// down, and says where the last one painted the image.
    fn climb(
        ctx: &egui::Context,
        viewer: &mut Alone,
        from: egui::Pos2,
        view: egui::Rect,
        popup: Option<egui::Rect>,
    ) -> Option<egui::Rect> {
        let mut after = None;
        for step in 1..=3 {
            let at = from - egui::vec2(0.0, 20.0 * step as f32);
            after = alone(
                ctx,
                viewer,
                vec![egui::Event::PointerMoved(at)],
                view,
                popup,
            );
        }
        after
    }

    /// A middle press beside the image and a move onto it, delivered in the same frame, start no
    /// pan: the drag that follows leaves the image, taller than the view, where it is.
    #[test]
    fn a_press_beside_the_image_starts_no_pan_when_the_move_on_comes_with_it() {
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(2000, 4000));
        let view = egui::Rect::from_min_max(egui::pos2(100.0, 0.0), SCREEN.max);
        let beside = egui::pos2(50.0, 200.0);
        let moved = |at| vec![egui::Event::PointerMoved(at)];
        let before = alone(&ctx, &mut viewer, moved(beside), view, None).unwrap();
        assert!(before.height() > view.height(), "{before:?} in {view:?}");
        let on = view.center();
        let events = vec![
            button(egui::PointerButton::Middle, beside, true),
            egui::Event::PointerMoved(on),
        ];
        alone(&ctx, &mut viewer, events, view, None);
        assert_eq!(viewer.image.panning, None);
        let after = climb(&ctx, &mut viewer, on, view, None);
        assert_eq!(after, Some(before), "the press beside the image panned it");
    }

    /// A middle press on a popup open over the image is the popup's, also when the pointer leaves
    /// the popup for the image in the same frame: no pan starts and the drag leaves the image.
    #[test]
    fn a_press_on_a_popup_over_the_image_starts_no_pan_when_the_pointer_leaves_it_at_once() {
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(2000, 4000));
        let place = egui::Rect::from_center_size(SCREEN.center(), egui::vec2(160.0, 100.0));
        let popup = Some(place);
        let off = egui::pos2(place.center().x, place.bottom() + 60.0);
        let moved = |at| vec![egui::Event::PointerMoved(at)];
        alone(&ctx, &mut viewer, moved(place.center()), SCREEN, popup);
        let before = alone(&ctx, &mut viewer, moved(place.center()), SCREEN, popup).unwrap();
        assert!(before.height() > SCREEN.height(), "{before:?}");
        let events = vec![
            button(egui::PointerButton::Middle, place.center(), true),
            egui::Event::PointerMoved(off),
        ];
        alone(&ctx, &mut viewer, events, SCREEN, popup);
        assert_eq!(viewer.image.panning, None);
        let after = climb(&ctx, &mut viewer, off, SCREEN, popup);
        assert_eq!(
            after,
            Some(before),
            "the press on the popup panned the image"
        );
    }

    /// A middle press on the image and a move off it, delivered in the same frame, start the pan:
    /// the press is judged where it landed, and the drag that follows moves the image.
    #[test]
    fn a_press_on_the_image_pans_though_the_pointer_leaves_it_in_the_same_frame() {
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(2000, 4000));
        let view = egui::Rect::from_min_max(egui::pos2(100.0, 0.0), SCREEN.max);
        let on = view.center();
        let beside = egui::pos2(50.0, on.y);
        let before = alone(&ctx, &mut viewer, vec![], view, None).unwrap();
        let events = vec![
            button(egui::PointerButton::Middle, on, true),
            egui::Event::PointerMoved(beside),
        ];
        alone(&ctx, &mut viewer, events, view, None);
        assert_eq!(viewer.image.panning, Some(egui::PointerButton::Middle));
        let after = climb(&ctx, &mut viewer, beside, view, None).unwrap();
        assert_eq!(after.top(), before.top() - 60.0, "{before:?} -> {after:?}");
    }

    /// A middle press on the image in the same frame another button is released starts the pan,
    /// though egui forgets where a press began on any release.
    #[test]
    fn a_middle_press_pans_though_another_button_is_released_in_the_same_frame() {
        use egui::PointerButton::{Middle, Primary};
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(2000, 4000));
        let on = SCREEN.center();
        let before = alone(&ctx, &mut viewer, vec![], SCREEN, None).unwrap();
        let primary = vec![egui::Event::PointerMoved(on), button(Primary, on, true)];
        alone(&ctx, &mut viewer, primary, SCREEN, None);
        assert_eq!(viewer.image.panning, None);
        let events = vec![button(Middle, on, true), button(Primary, on, false)];
        alone(&ctx, &mut viewer, events, SCREEN, None);
        assert_eq!(viewer.image.panning, Some(Middle));
        let after = climb(&ctx, &mut viewer, on, SCREEN, None).unwrap();
        assert_eq!(after.top(), before.top() - 60.0, "{before:?} -> {after:?}");
    }

    /// A fitted image scrolled down keeps the part at the top of the view there when the drawer
    /// resizes and the fit changes its scale.
    #[test]
    fn a_fitted_image_keeps_the_part_being_read_when_the_drawer_resizes() {
        let folder = Folder::new();
        let path = folder.0.join("tall.png");
        write_png(&path, 1200, 4800);
        let (mut window, ctx, _) = window_with_image(&path);
        let at = window.sidebar.last_dock.unwrap().center();
        for _ in 0..10 {
            painted(&mut window, &ctx, plain_wheel(at, egui::vec2(0.0, -6.0)));
        }
        let read = |window: &crate::Window, image: egui::Rect| {
            let top = body_top(window, window.sidebar.last_dock.unwrap());
            (top - image.top()) / image.height()
        };
        let before = settle(&mut window, &ctx);
        let was = read(&window, before);
        assert!(was > 0.0, "{before:?}");
        crate::sidebar::toggle_maximize(&mut window);
        let after = settle(&mut window, &ctx);
        assert!(after.width() > before.width(), "{before:?} -> {after:?}");
        assert!(
            (read(&window, after) - was).abs() * after.height() < 1.0,
            "{was} -> {}",
            read(&window, after)
        );
    }

    /// The image takes the body's rounded bottom-left corner, and not the bottom-right one the
    /// notes' column holds: it is painted as the body's own shape, clipped to the body when it
    /// fills it and to the image itself when it is smaller.
    #[test]
    fn the_image_takes_the_body_rounded_corners() {
        let folder = Folder::new();
        let (mut window, ctx) = zoomed_window(&folder);
        let shapes = painted(&mut window, &ctx, vec![]);
        let (_, rect, clip) = image_shape(&shapes).unwrap();
        let round = super::super::view::BODY_CORNER;
        assert!(round > 0);
        assert_eq!((rect.corner_radius.sw, rect.corner_radius.se), (round, 0));
        assert_eq!((rect.corner_radius.nw, rect.corner_radius.ne), (0, 0));
        assert_eq!(clip, rect.rect, "a zoomed image is clipped to the body");
        let (mut window, ctx, _, _) = small_window(&folder, |image| image.center());
        let shapes = painted(&mut window, &ctx, vec![]);
        let (whole, _, clip) = image_shape(&shapes).unwrap();
        assert!(
            whole.expand(0.01).contains_rect(clip) && clip.expand(0.01).contains_rect(whole),
            "{clip:?} is not the image {whole:?}"
        );
    }

    fn marks_of(window: &crate::Window) -> &marks::Marks {
        &slot(&window).document.as_ref().unwrap().marks
    }

    fn marks_mut(window: &mut crate::Window) -> &mut marks::Marks {
        &mut slot_mut(window).document.as_mut().unwrap().marks
    }

    fn ink() -> egui::Color32 {
        let [r, g, b, a] = marks::INK;
        egui::Color32::from_rgba_unmultiplied(r, g, b, a)
    }

    /// The segments painted in the marks' ink: the arrows' shafts.
    fn shafts(shapes: &[egui::epaint::ClippedShape]) -> Vec<[egui::Pos2; 2]> {
        shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::LineSegment { points, stroke } if stroke.color == ink() => {
                    Some(*points)
                }
                _ => None,
            })
            .collect()
    }

    /// The pixel of an image of `size` under `at`, with the whole image painted at `image`.
    fn pixel_under(at: egui::Pos2, image: egui::Rect, size: egui::Vec2) -> egui::Pos2 {
        ((at - image.min) / image.size() * size).to_pos2()
    }

    /// A plain left drag draws an arrow from the pixel pressed to the pixel let go; a drag of less
    /// than four screen points, which on this fitted screenshot is more than four of its pixels,
    /// leaves nothing; and a zoom moves the arrow with the image, never across it.
    #[test]
    fn a_plain_drag_draws_an_arrow_that_stays_on_its_pixels_through_a_zoom() {
        let folder = Folder::new();
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let size = egui::vec2(2400.0, 1600.0);
        let (mut window, ctx, _) = window_with_image(&path);
        let image = settle(&mut window, &ctx);
        assert!(image.width() / size.x < 0.6, "{image:?} is not scaled down");
        let from = image_view(&window, &ctx).center();
        use egui::PointerButton::Primary;
        drag_at(
            &mut window,
            &ctx,
            Primary,
            from,
            egui::vec2(3.0, 0.0),
            false,
        );
        assert!(marks_of(&window).marks().is_empty(), "a slip left a mark");
        drag_at(
            &mut window,
            &ctx,
            Primary,
            from,
            egui::vec2(60.0, 30.0),
            false,
        );
        let [mark] = marks_of(&window).marks() else {
            panic!("{} marks", marks_of(&window).marks().len());
        };
        let marks::Shape::Arrow {
            from: tail,
            to: tip,
        } = *mark
        else {
            panic!("{mark:?}");
        };
        assert!((tail - pixel_under(from, image, size)).length() < 1.0);
        let end = from + egui::vec2(60.0, 30.0);
        assert!((tip - pixel_under(end, image, size)).length() < 1.0);
        let [shaft] = shafts(&painted(&mut window, &ctx, vec![]))[..] else {
            panic!("not one shaft painted");
        };
        assert!((shaft[0] - from).length() < 0.5, "{shaft:?}");
        let at = from + egui::vec2(-150.0, 40.0);
        painted(
            &mut window,
            &ctx,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::ModifiersChanged(ctrl()),
                ctrl_wheel(egui::MouseWheelUnit::Line, 2.0, egui::TouchPhase::Move),
            ],
        );
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
        let zoomed = settle(&mut window, &ctx);
        assert!(
            zoomed.width() > image.width() * 1.2,
            "{image:?} -> {zoomed:?}"
        );
        let [shaft] = shafts(&painted(&mut window, &ctx, vec![]))[..] else {
            panic!("not one shaft painted after the zoom");
        };
        assert!(
            (pixel_under(shaft[0], zoomed, size) - tail).length() < 1.0,
            "the arrow moved across the image: {shaft:?} on {zoomed:?}"
        );
    }

    /// Clicks with the marker tool place markers numbered in order; Ctrl+Z, and not Ctrl+Shift+Z,
    /// takes back the last while the drawer holds the keys, but not while a note is being written,
    /// where the keys are the note's.
    #[test]
    fn markers_are_numbered_in_order_and_ctrl_z_takes_back_the_last() {
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _) = window_with_image(&path);
        marks_mut(&mut window).tool = marks::Tool::Marker;
        let image = settle(&mut window, &ctx).intersect(image_view(&window, &ctx));
        for x in [0.2, 0.5, 0.8] {
            click(&mut window, &ctx, image.lerp_inside(egui::vec2(x, 0.3)));
        }
        let shapes = painted(&mut window, &ctx, vec![]);
        let mut numbers = texts(&shapes, image_view(&window, &ctx));
        numbers.retain(|text| ["1", "2", "3"].contains(&text.as_str()));
        assert_eq!(numbers, ["1", "2", "3"]);
        let undo = egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: ctrl(),
        };
        // Ctrl+Shift+Z is redo elsewhere; with no redo here it leaves the markers alone.
        let shifted = egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                shift: true,
                ..ctrl()
            },
        };
        painted(&mut window, &ctx, vec![shifted]);
        assert_eq!(marks_of(&window).notes().len(), 3, "Ctrl+Shift+Z undid");
        painted(&mut window, &ctx, vec![undo.clone()]);
        assert_eq!(marks_of(&window).notes().len(), 2);
        // The first note's field, past its marker's disc in the column beside the image.
        let column = layout(&window, &ctx).column;
        let field = egui::pos2(column.left() + 100.0, column.top() + 6.0 + 11.0);
        // A pointer arrives before it presses: a press in the frame the pointer arrives finds
        // the drawer's widgets where the previous frame's hit test left them.
        painted(&mut window, &ctx, vec![egui::Event::PointerMoved(field)]);
        click(&mut window, &ctx, field);
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::Text("button clipped".into())],
        );
        assert_eq!(marks_of(&window).notes(), ["button clipped", ""]);
        // The note's own Ctrl+Z takes back the typing; the markers stay.
        painted(&mut window, &ctx, vec![undo.clone()]);
        assert_eq!(marks_of(&window).notes().len(), 2);
        // A console that took the keys gets its Ctrl+Z; the markers stay.
        click(&mut window, &ctx, egui::pos2(300.0, 100.0));
        assert!(!window.viewer.keyboard);
        painted(&mut window, &ctx, vec![undo]);
        assert_eq!(marks_of(&window).notes().len(), 2);
    }

    /// Deleting marker 1 leaves marker 2's field its own: the keyboard stays in it, and a Ctrl+Z
    /// there goes back through marker 2's typing, never to the text marker 1's note had. Each note
    /// is held still for over a second of frames, which is what makes egui's undo keep it.
    #[test]
    fn a_deleted_markers_note_never_comes_back_in_the_next_ones_field() {
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _) = window_with_image(&path);
        marks_mut(&mut window).tool = marks::Tool::Marker;
        let image = settle(&mut window, &ctx).intersect(image_view(&window, &ctx));
        for x in [0.3, 0.7] {
            click(&mut window, &ctx, image.lerp_inside(egui::vec2(x, 0.3)));
        }
        let column = layout(&window, &ctx).column;
        let first = egui::pos2(column.left() + 100.0, column.top() + 6.0 + 11.0);
        let write = |window: &mut crate::Window, at: egui::Pos2, text: &str| {
            press(window, &ctx, at);
            painted(window, &ctx, vec![egui::Event::Text(text.into())]);
            for _ in 0..70 {
                painted(window, &ctx, vec![]);
            }
        };
        write(&mut window, first, "first");
        let second = place_of(&mut window, &ctx, "What this marker shows");
        write(&mut window, second, "second");
        assert_eq!(marks_of(&window).notes(), ["first", "second"]);
        let focused = ctx.memory(|m| m.focused());
        let (_, index, _) = marks_of(&window).numbered().next().unwrap();
        marks_mut(&mut window).delete(index);
        painted(&mut window, &ctx, vec![]);
        assert_eq!(
            ctx.memory(|m| m.focused()),
            focused,
            "the keyboard left the note"
        );
        press(&mut window, &ctx, first);
        let undo = egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: ctrl(),
        };
        painted(&mut window, &ctx, vec![undo]);
        assert_eq!(
            marks_of(&window).notes(),
            [""],
            "undo went through another note"
        );
    }

    /// Marker 1 of the next image, opened over the first or after it was closed, gets a field of
    /// its own: it starts empty, and a Ctrl+Z there never brings back what marker 1's note held on
    /// the first image, though that note was held still for over a second of frames, which is what
    /// makes egui's undo keep it by the field's id.
    #[test]
    fn a_note_never_carries_into_the_same_marker_of_the_next_image() {
        use crate::viewer::StripAction;
        let folder = Folder::new();
        let (first, second) = (folder.0.join("a.png"), folder.0.join("b.png"));
        write_png(&first, 640, 400);
        write_png(&second, 640, 400);
        let undo = egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: ctrl(),
        };
        for closed in [false, true] {
            let (mut window, ctx, _) = window_with_image(&first);
            let name = slot(&window).origin().unwrap().pane;
            let mark = |window: &mut crate::Window| {
                marks_mut(window).tool = marks::Tool::Marker;
                let image = settle(window, &ctx).intersect(image_view(window, &ctx));
                click(window, &ctx, image.lerp_inside(egui::vec2(0.5, 0.3)));
                // The hint is painted only in an empty field.
                place_of(window, &ctx, "What this marker shows")
            };
            let field = mark(&mut window);
            press(&mut window, &ctx, field);
            painted(&mut window, &ctx, vec![egui::Event::Text("first".into())]);
            for _ in 0..70 {
                painted(&mut window, &ctx, vec![]);
            }
            assert_eq!(marks_of(&window).notes(), ["first"]);
            marks_mut(&mut window).clear();
            if closed {
                crate::viewer::apply_strip_action(&mut window, StripAction::CloseImage, &ctx);
                assert!(!slot(&window).shown(), "the image did not close");
            }
            crate::viewer::open_image_target(&mut window, name, second.clone()).unwrap();
            let started = Instant::now();
            while image_rect(&painted(&mut window, &ctx, vec![])).is_none() {
                assert!(started.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(5));
            }
            let field = mark(&mut window);
            assert_eq!(marks_of(&window).notes(), [""], "closed: {closed}");
            press(&mut window, &ctx, field);
            for event in [egui::Event::Text("x".into()), undo.clone()] {
                painted(&mut window, &ctx, vec![event]);
                assert_ne!(
                    marks_of(&window).notes(),
                    ["first"],
                    "the first image's note came back, closed: {closed}"
                );
            }
            // The Ctrl+Z reached the field: it took back the typing.
            assert_eq!(marks_of(&window).notes(), [""], "closed: {closed}");
        }
    }

    /// The general note written on an image with no marks keeps the keyboard letter after
    /// letter: its first letter makes something to send, and the button that appears with it, laid
    /// out ahead of the field, does not take the field's focus; nor does its last letter erased.
    #[test]
    fn the_general_note_keeps_the_keyboard_as_the_send_button_comes_and_goes() {
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _) = window_with_image(&path);
        assert!(marks_of(&window).is_empty());
        let field = place_of(&mut window, &ctx, "A note on the whole image");
        press(&mut window, &ctx, field);
        let focused = ctx.memory(|m| m.focused());
        assert!(focused.is_some(), "the press did not focus the field");
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let typed = [
            (egui::Event::Text("a".into()), "a"),
            (egui::Event::Text("b".into()), "ab"),
            (key(egui::Key::Backspace), "a"),
            (key(egui::Key::Backspace), ""),
            (egui::Event::Text("c".into()), "c"),
        ];
        for (event, general) in typed {
            painted(&mut window, &ctx, vec![event]);
            painted(&mut window, &ctx, vec![]);
            assert_eq!(marks_of(&window).general, general);
            assert_eq!(ctx.memory(|m| m.focused()), focused, "after {general:?}");
        }
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(
            text_rect(&shapes, |t| t.starts_with("Send to ")).is_some(),
            "a general note is something to send"
        );
    }

    /// The text painted that `wanted` accepts, where, and the clip it was painted under.
    fn clipped_text(
        shapes: &[egui::epaint::ClippedShape],
        wanted: impl Fn(&str) -> bool,
    ) -> Option<(egui::Rect, egui::Rect)> {
        shapes.iter().find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) if wanted(text.galley.text()) => Some((
                text.galley.rect.translate(text.pos.to_vec2()),
                clipped.clip_rect,
            )),
            _ => None,
        })
    }

    /// On a drawer both narrow and short, a refusal that wraps over several lines is painted
    /// whole: inside the drawer, inside the clip it is painted under, and clear of the Send button
    /// under it, which is painted whole too.
    #[test]
    fn a_refusal_is_painted_whole_on_a_narrow_short_drawer() {
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(400, 300));
        viewer.document.as_mut().unwrap().marks.general = "the header overlaps".into();
        let notice = "This image has marks that were not sent: send them or clear them before \
                      opening another image or document.";
        viewer.notice = Some(notice.into());
        let view = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(300.0, 230.0));
        let mut shapes = Vec::new();
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(SCREEN),
                    ..Default::default()
                },
                |ui| {
                    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(view));
                    viewer.show(&mut ui, crate::theme::Theme::default(), false);
                },
            );
            output.textures_delta.clear();
            shapes = output.shapes;
        }
        let (said, clip) = clipped_text(&shapes, |t| t == notice).expect("the notice is painted");
        assert!(said.height() > 30.0, "{said:?} is not wrapped");
        assert!(
            clip.expand(0.5).contains_rect(said),
            "{said:?} clipped by {clip:?}"
        );
        assert!(view.contains_rect(said), "{said:?} outside {view:?}");
        let (button, clip) =
            clipped_text(&shapes, |t| t.starts_with("Send to ")).expect("the button is painted");
        assert!(
            clip.expand(0.5).contains_rect(button),
            "{button:?} clipped by {clip:?}"
        );
        assert!(button.top() >= said.bottom(), "{button:?} over {said:?}");
        assert!(view.contains_rect(button), "{button:?} outside {view:?}");
    }

    /// Three passes of `viewer` alone, laid out in `view`, with `events` in the last; what it
    /// painted then.
    fn shown_in(
        ctx: &egui::Context,
        viewer: &mut Alone,
        view: egui::Rect,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut shapes = Vec::new();
        for pass in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(SCREEN.union(view)),
                    events: if pass == 2 { events.clone() } else { vec![] },
                    ..Default::default()
                },
                |ui| {
                    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(view));
                    viewer.show(&mut ui, crate::theme::Theme::default(), false);
                },
            );
            output.textures_delta.clear();
            shapes = output.shapes;
        }
        shapes
    }

    /// Each text painted that `wanted` accepts: its galley, and the part of it its clip shows.
    fn visible_texts(
        shapes: &[egui::epaint::ClippedShape],
        wanted: impl Fn(&str) -> bool,
    ) -> Vec<(Arc<egui::Galley>, egui::Rect)> {
        shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if wanted(text.galley.text()) => {
                    let rect = text.galley.rect.translate(text.pos.to_vec2());
                    Some((Arc::clone(&text.galley), rect.intersect(clipped.clip_rect)))
                }
                _ => None,
            })
            .collect()
    }

    /// On a drawer too short for the toolbar, a wrapped refusal and the Send button, nothing of
    /// the column or the line under it shows outside the drawer or over the toolbar, and the
    /// refusal always shows one whole line: cut to it, with the whole of it on hover, above a
    /// button that keeps its full height while both fit; shorter, the line takes the column's head
    /// and the button is cut under it rather than the refusal being cut away (#197).
    #[test]
    fn a_drawer_too_short_cuts_the_refusal_and_never_paints_over_the_toolbar() {
        let notice = "This image has marks that were not sent: send them or clear them before \
                      opening another image or document.";
        let footer = |t: &str| {
            t == notice || t.starts_with("Send to ") || t == "Nothing can be sent from here."
        };
        for height in [118.0, 84.0, 64.0] {
            let ctx = egui::Context::default();
            let mut viewer = ready(image::RgbaImage::new(400, 300));
            viewer.document.as_mut().unwrap().marks.general = "the header overlaps".into();
            viewer.notice = Some(notice.into());
            let view = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(300.0, height));
            let below_bar = egui::Rect::from_min_max(
                egui::pos2(view.left(), view.top() + tools::TOOLBAR),
                view.max,
            );
            let shapes = shown_in(&ctx, &mut viewer, view, vec![]);
            for (galley, shown) in &visible_texts(&shapes, footer) {
                assert!(
                    !shown.is_positive() || below_bar.contains_rect(*shown),
                    "{:?} shows at {shown:?}, outside {below_bar:?}",
                    galley.text()
                );
            }
            let said = visible_texts(&shapes, |t| t == notice);
            let [(said, shown)] = &said[..] else {
                panic!("the refusal is not painted once at {height}");
            };
            assert!(
                said.elided && said.rows.len() == 1,
                "{} rows",
                said.rows.len()
            );
            assert!(
                shown.height() >= said.rect.height() - 0.5,
                "only {shown:?} of the refusal's line shows at {height}"
            );
            // Moved once, then held still: egui shows a tooltip only to a pointer at rest.
            let mut hover = vec![egui::Event::PointerMoved(shown.center())];
            let whole = (0..60).any(|_| {
                let shapes = shown_in(&ctx, &mut viewer, view, std::mem::take(&mut hover));
                visible_texts(&shapes, |t| t == notice)
                    .iter()
                    .any(|(galley, _)| !galley.elided)
            });
            assert!(
                whole,
                "hovering the cut refusal does not show it whole at {height}"
            );
            // Cut away entirely under the line, the button is not painted at all.
            let Some((button, _)) = clipped_text(&shapes, |t| t.starts_with("Send to ")) else {
                assert!(height < 100.0, "the button is not painted at {height}");
                continue;
            };
            let frame = shapes.iter().find_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect) if rect.rect.contains_rect(button) => {
                    Some((rect.rect, rect.rect.intersect(clipped.clip_rect)))
                }
                _ => None,
            });
            let (frame, frame_shown) = frame.expect("the button has a frame");
            assert!(frame.height() >= 32.0, "{frame:?}");
            assert!(
                !frame_shown.is_positive() || below_bar.contains_rect(frame_shown),
                "{frame_shown:?}"
            );
            assert!(
                !shown.intersect(frame_shown).is_positive(),
                "{shown:?} over the button {frame_shown:?} at {height}"
            );
            if height > 100.0 {
                assert_eq!(frame_shown, frame, "the button is cut where both fit");
            }
        }
    }

    /// While a Send is on its way the line under the notes greys out with them, and a refusal cut
    /// to one line still shows whole on hover: the notes' column is disabled under it, not over it.
    #[test]
    fn a_send_on_its_way_greys_the_line_and_keeps_the_cut_refusal_readable() {
        let notice = "This image has marks that were not sent: send them or clear them before \
                      opening another image or document.";
        let asked = |height: f32| {
            let mut viewer = ready(image::RgbaImage::new(400, 300));
            viewer.document.as_mut().unwrap().marks.general = "the header overlaps".into();
            viewer.notice = Some(notice.into());
            let request = send::Request {
                document: viewer.document().unwrap(),
                pane: 0,
                path: PathBuf::from("/a/annotated-1.png"),
                line: "x".into(),
            };
            // Nothing polls a viewer alone, so the Send stays asked.
            viewer.image.send.ask(request);
            let view = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(300.0, height));
            (viewer, view)
        };
        let ctx = egui::Context::default();
        let (mut viewer, view) = asked(230.0);
        let shapes = shown_in(&ctx, &mut viewer, view, vec![]);
        let faded = shapes.iter().find_map(|clipped| match &clipped.shape {
            // A disabled painter fades the glyphs' own vertex colours, not the shape's factor.
            egui::Shape::Text(text) if text.galley.text() == "Nothing can be sent from here." => {
                Some(text.galley.rows[0].visuals.mesh.vertices[0].color.a())
            }
            _ => None,
        });
        let alpha = faded.expect("the line under the notes is painted");
        assert!(alpha < 255, "the line is not greyed out: alpha {alpha}");

        let ctx = egui::Context::default();
        let (mut viewer, view) = asked(118.0);
        let shapes = shown_in(&ctx, &mut viewer, view, vec![]);
        let said = visible_texts(&shapes, |t| t == notice);
        let [(galley, shown)] = &said[..] else {
            panic!("the refusal is not painted once");
        };
        assert!(galley.elided, "the refusal is not cut");
        let mut hover = vec![egui::Event::PointerMoved(shown.center())];
        let whole = (0..60).any(|_| {
            let shapes = shown_in(&ctx, &mut viewer, view, std::mem::take(&mut hover));
            visible_texts(&shapes, |t| t == notice)
                .iter()
                .any(|(galley, _)| !galley.elided)
        });
        assert!(
            whole,
            "hovering the cut refusal during a Send does not show it whole"
        );
    }

    /// A Cancel clicked before the copy's worker started takes the Send back with no worker to
    /// park: Send is ready again at once, and the notes can be edited.
    #[test]
    fn cancel_before_the_copy_started_parks_nothing() {
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(400, 300));
        viewer.document.as_mut().unwrap().marks.general = "the header overlaps".into();
        viewer.image.send.ask(send::Request {
            document: viewer.document().unwrap(),
            pane: 0,
            path: PathBuf::from("/a/annotated-1.png"),
            line: "x".into(),
        });
        let view = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(700.0, 400.0));
        let shapes = shown_in(&ctx, &mut viewer, view, vec![]);
        let (_, cancel) = text_rect(&shapes, |t| t == "Cancel").expect("Cancel is offered");
        let at = cancel.center();
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let events = vec![egui::Event::PointerMoved(at), button(true), button(false)];
        shown_in(&ctx, &mut viewer, view, events);
        assert!(!viewer.image.sending(), "the Send was not taken back");
        assert!(
            !viewer.image.send.parked(),
            "a worker that never started was parked"
        );
        assert_eq!(
            viewer.notice.as_deref(),
            Some("Send cancelled; nothing was sent.")
        );
        assert_eq!(
            viewer.document.as_ref().unwrap().marks.general,
            "the header overlaps"
        );
    }

    /// What a cancelled Send's worker left to say joins the notice the drawer already shows rather
    /// than taking its place: a refusal nobody has read yet is painted beside it, and the held
    /// word is taken up once.
    #[test]
    fn a_parked_workers_word_joins_the_notice_instead_of_replacing_it() {
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(400, 300));
        let refusal = "The line was typed, but Enter was not pressed: the console closed.";
        let held = "The cancelled copy's worker reported: disk full.";
        viewer.notice = Some(refusal.into());
        viewer.unshown = Some(held.into());
        let view = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(700.0, 400.0));
        let shapes = shown_in(&ctx, &mut viewer, view, vec![]);
        let said = texts(&shapes, view).join("\n");
        assert!(said.contains(refusal), "the refusal was lost: {said}");
        assert!(said.contains(held), "the worker's word was lost: {said}");
        assert!(viewer.unshown.is_none());
    }

    /// Between the heights where the Send button shows whole under the notice's line and those
    /// where it is cut away entirely, it never shows as a strip too thin to read or aim at: under
    /// `SLIVER` it is not laid out, and the notice says on hover why there is no Send.
    #[test]
    fn a_drawer_too_short_for_the_button_drops_it_and_says_why_on_hover() {
        let notice = "This image has marks that were not sent: send them or clear them before \
                      opening another image or document.";
        let mut dropped = None;
        let mut shown = 0;
        for height in (60..=100).map(|h| h as f32) {
            let ctx = egui::Context::default();
            let mut viewer = ready(image::RgbaImage::new(400, 300));
            viewer.document.as_mut().unwrap().marks.general = "the header overlaps".into();
            viewer.notice = Some(notice.into());
            let view = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(300.0, height));
            let shapes = shown_in(&ctx, &mut viewer, view, vec![]);
            let Some((button, _)) = clipped_text(&shapes, |t| t.starts_with("Send to ")) else {
                dropped.get_or_insert((height, ctx, viewer, view));
                continue;
            };
            let frame = shapes.iter().find_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect) if rect.rect.contains_rect(button) => {
                    Some(rect.rect.intersect(clipped.clip_rect))
                }
                _ => None,
            });
            let frame = frame.expect("the button has a frame");
            assert!(
                frame.height() >= tools::SLIVER - 0.5,
                "only {} of the button shows at {height}",
                frame.height()
            );
            shown += 1;
        }
        assert!(shown > 0, "the button never shows");
        let (height, ctx, mut viewer, view) = dropped.expect("the button is never dropped");
        let shapes = shown_in(&ctx, &mut viewer, view, vec![]);
        let [(_, line)] = &visible_texts(&shapes, |t| t == notice)[..] else {
            panic!("the refusal is not painted once at {height}");
        };
        let mut hover = vec![egui::Event::PointerMoved(line.center())];
        let said = (0..60).any(|_| {
            let shapes = shown_in(&ctx, &mut viewer, view, std::mem::take(&mut hover));
            text_rect(&shapes, |t| t == tools::ENLARGE).is_some()
        });
        assert!(said, "hovering the refusal does not say why at {height}");
    }

    /// Beside the image, a refusal is laid out at the column's width and painted inside the
    /// column and its clip, which itself stays inside the column, wrapped rather than cut: the
    /// drawer has the height for it.
    #[test]
    fn beside_the_image_a_refusal_wraps_at_the_columns_width() {
        let ctx = egui::Context::default();
        let mut viewer = ready(image::RgbaImage::new(400, 300));
        viewer.document.as_mut().unwrap().marks.general = "the header overlaps".into();
        let notice = "This image has marks that were not sent: send them or clear them before \
                      opening another image or document.";
        viewer.notice = Some(notice.into());
        let view = egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(tools::COLUMN + tools::NARROWEST + 100.0, 380.0),
        );
        let shapes = shown_in(&ctx, &mut viewer, view, vec![]);
        let (said, clip) = clipped_text(&shapes, |t| t == notice).expect("the notice is painted");
        let column = egui::Rect::from_min_max(
            egui::pos2(view.right() - tools::COLUMN, view.top() + tools::TOOLBAR),
            view.max,
        );
        assert!(column.contains_rect(said), "{said:?} outside {column:?}");
        assert!(
            clip.expand(0.5).contains_rect(said),
            "{said:?} clipped by {clip:?}"
        );
        assert!(
            column.expand(0.5).contains_rect(clip),
            "{clip:?} reaches outside {column:?}"
        );
        assert!(said.height() > 30.0, "{said:?} is not wrapped");
    }

    /// Unsent marks keep the image: opening another image over it, or closing its tab, is refused
    /// with a message, the image and its marks stay, and once the marks are cleared the same open
    /// goes through. A Markdown file opens beside it, refused by nothing.
    #[test]
    fn unsent_marks_keep_the_image_until_they_are_cleared() {
        use crate::viewer::StripAction;
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 64, 32);
        let (mut window, ctx, _) = window_with_image(&path);
        let name = slot(&window).origin().unwrap().pane;
        marks_mut(&mut window).general = "the header overlaps".into();
        let other = folder.0.join("other.png");
        write_png(&other, 64, 32);
        let kept = |window: &crate::Window| {
            assert_eq!(slot(&window).path(), Some(path.as_path()));
            assert_eq!(marks_of(window).general, "the header overlaps");
            assert!(crate::viewer::origin::showing_image(window));
        };
        assert!(crate::viewer::open_document_target(
            &mut window,
            name,
            folder.0.join("guide.md"),
            None,
        ));
        assert_eq!(slot(&window).path(), Some(path.as_path()));
        assert_eq!(marks_of(&window).general, "the header overlaps");
        assert!(slot(&window).notice.is_none());
        crate::viewer::show_image(&mut window, true);
        let said = crate::viewer::NotOpened::Marks.to_string();
        assert!(said.contains("another image"), "{said}");
        assert_eq!(
            crate::viewer::open_image_target(&mut window, name, other.clone()),
            Err(crate::viewer::NotOpened::Marks)
        );
        kept(&window);
        crate::viewer::apply_strip_action(&mut window, StripAction::CloseImage, &ctx);
        kept(&window);
        let shapes = painted(&mut window, &ctx, vec![]);
        let notice = slot(&window).notice.clone().unwrap();
        assert!(
            texts(&shapes, area()).contains(&notice),
            "the refusal is not painted"
        );
        marks_mut(&mut window).clear();
        crate::viewer::open_image_target(&mut window, name, other.clone()).unwrap();
        assert_eq!(slot(&window).path(), Some(other.as_path()));
        assert!(slot(&window).notice.is_none());
    }

    /// The text painted that `wanted` accepts, and where.
    fn text_rect(
        shapes: &[egui::epaint::ClippedShape],
        wanted: impl Fn(&str) -> bool,
    ) -> Option<(String, egui::Rect)> {
        fn walk(
            shape: &egui::Shape,
            wanted: &dyn Fn(&str) -> bool,
            found: &mut Option<(String, egui::Rect)>,
        ) {
            match shape {
                egui::Shape::Text(text) if wanted(text.galley.text()) => {
                    let rect = text.galley.rect.translate(text.pos.to_vec2());
                    *found = Some((text.galley.text().to_owned(), rect));
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        walk(shape, wanted, found);
                    }
                }
                _ => {}
            }
        }
        let mut found = None;
        for clipped in shapes {
            walk(&clipped.shape, &wanted, &mut found);
        }
        found
    }

    /// A window whose image is pinned to a console that reads raw and records what it reads, with
    /// a directory of its own, and a marker with a note on the image.
    #[cfg(unix)]
    fn window_to_send(
        path: &Path,
        received: &term_core::received::Received,
    ) -> (crate::Window, egui::Context, std::path::PathBuf) {
        window_to_send_beside(path, received, "sleep 30")
    }

    /// The same, with `beside` running in the image's console's neighbour.
    #[cfg(unix)]
    fn window_to_send_beside(
        path: &Path,
        received: &term_core::received::Received,
        beside: &str,
    ) -> (crate::Window, egui::Context, std::path::PathBuf) {
        let script = received.script("stty raw -echo");
        let (mut window, ctx, _) = window_with_image_in(path, [beside, &script]);
        received.wait_until_ready(Duration::from_secs(5));
        let name = slot(&window).origin().unwrap().pane;
        let dir = crate::PanelDir::create().unwrap();
        let folder = dir.path().to_owned();
        origin_pane(&mut window).panel = Some(dir);
        marks_mut(&mut window).tool = marks::Tool::Marker;
        let at = image_view(&window, &ctx).center();
        painted(&mut window, &ctx, vec![egui::Event::PointerMoved(at)]);
        click(&mut window, &ctx, at);
        assert_eq!(slot(&window).origin().unwrap().pane, name);
        *marks_mut(&mut window).note_mut(0).unwrap() = "button clipped".into();
        (window, ctx, folder)
    }

    fn origin_pane(window: &mut crate::Window) -> &mut crate::Pane {
        let name = slot(&window).origin().unwrap().pane;
        window.all_panes_mut().find(|p| p.name == name).unwrap()
    }

    /// Paints, then clicks the button that names the console, as a person's pointer reaches it.
    /// Answers the line painted under it on that frame.
    fn click_send(window: &mut crate::Window, ctx: &egui::Context) -> String {
        let shapes = painted(window, ctx, vec![]);
        let label = slot(&window).origin().unwrap().label.clone();
        let (_, button) = text_rect(&shapes, |t| t == format!("Send to {label} and press Enter"))
            .expect("the button names the console");
        let line = text_rect(&shapes, |t| t.starts_with("Annotated screenshot: "))
            .map(|(line, _)| line)
            .unwrap_or_default();
        painted(
            window,
            ctx,
            vec![egui::Event::PointerMoved(button.center())],
        );
        click(window, ctx, button.center());
        line
    }

    /// Passes frames until the Send in flight has done what it can before an Enter.
    fn until_written(window: &mut crate::Window, ctx: &egui::Context) {
        let started = Instant::now();
        while window.image.send.before_the_line() {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "{:?}",
                slot(&window).notice
            );
            painted(window, ctx, vec![]);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Send writes a copy with the marks at the original's full resolution, larger than the GPU's
    /// texture, into the console's own folder, leaves the original as it was, types into that
    /// console exactly the line painted under the button, composed for the paste mode it has when
    /// it is sent, though the drawer closed in between, and presses Enter on a later frame in a
    /// write of its own.
    #[cfg(unix)]
    #[test]
    fn send_writes_the_copy_then_the_shown_line_and_presses_enter_on_a_later_frame() {
        let received = term_core::received::Received::new("annotated-send");
        let folder = Folder::new();
        let path = folder.0.join("wide.png");
        write_png(&path, 2400, 1600);
        let before = std::fs::read(&path).unwrap();
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        marks_mut(&mut window).general = "the header overlaps".into();
        let copy = dir.join("annotated-1.png");
        let shown = click_send(&mut window, &ctx);
        assert_eq!(
            shown,
            format!(
                "Annotated screenshot: {} · 1: button clipped · Note: the header overlaps",
                copy.display()
            )
        );
        crate::sidebar::close(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        assert!(window.image.send.writing(), "{:?}", slot(&window).notice);
        // The console turns paste markers on while the copy is written: the paste is composed
        // for the mode it has when it is sent.
        origin_pane(&mut window).screen.advance(b"\x1b[?2004h");
        until_written(&mut window, &ctx);
        let label = &slot(&window).origin().unwrap().label;
        assert_eq!(slot(&window).notice, Some(format!("Sent to {label}.")));
        let paste = format!("\x1b[200~{shown}\x1b[201~");
        assert_eq!(
            received.wait_for(paste.len(), Duration::from_secs(5)),
            paste.as_bytes()
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            received.so_far(),
            paste.as_bytes(),
            "Enter rode with the paste"
        );
        painted(&mut window, &ctx, vec![]);
        let submitted = [paste.as_bytes(), b"\r"].concat();
        assert_eq!(
            received.wait_for(submitted.len(), Duration::from_secs(5)),
            submitted
        );
        let written = ::image::open(&copy).unwrap().to_rgba8();
        assert_eq!(written.dimensions(), (2400, 1600));
        assert_ne!(
            written,
            ::image::open(&path).unwrap().to_rgba8(),
            "no mark in the copy"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the original changed"
        );
        assert!(marks_of(&window).is_empty());
        let _ = std::fs::remove_file(&copy);
    }

    /// The slot of the console called `name`, wherever the focus is.
    fn slot_called(window: &mut crate::Window, name: usize) -> &mut Slot {
        slot_of(&mut window.tabs, name).expect("the console is open")
    }

    /// A Send follows its image, not the focus: with another console's image shown while the copy
    /// is written, the line is typed into the Send's own console and its marks clear there, while
    /// the other console's image keeps its notes, says nothing and offers no Cancel for it.
    #[cfg(unix)]
    #[test]
    fn a_send_reaches_its_own_console_while_another_consoles_image_is_shown() {
        let received = term_core::received::Received::new("annotated-follows");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        let origin = slot(&window)
            .origin()
            .map(|o| (o.pane, o.label.clone()))
            .unwrap();
        let line = click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (copy, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        let beside = window.active().panes()[0].name;
        let other = folder.0.join("other.png");
        write_png(&other, 64, 32);
        open_in(&mut window, &ctx, beside, &other);
        marks_mut(&mut window).general = "on the other console".into();
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(
            text_rect(&shapes, |t| t == "Cancel").is_none(),
            "the other console's image offers to take this Send back"
        );
        flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
        sender.send(Ok(copy.clone())).unwrap();
        for _ in 0..3 {
            painted(&mut window, &ctx, vec![]);
        }
        let submitted = [line.as_bytes(), b"\r"].concat();
        assert_eq!(
            received.wait_for(submitted.len(), Duration::from_secs(5)),
            submitted
        );
        let own = slot_called(&mut window, origin.0);
        assert_eq!(own.notice, Some(format!("Sent to {}.", origin.1)));
        assert!(own.document.as_ref().unwrap().marks.is_empty());
        let shown = slot_called(&mut window, beside);
        assert_eq!(shown.notice, None);
        assert_eq!(
            shown.document.as_ref().unwrap().marks.general,
            "on the other console"
        );
        let _ = std::fs::remove_file(&copy);
    }

    /// A cancelled Send whose worker ends after the focus moved to another console's image says
    /// what it has to say under its own console's image, never under the one shown.
    #[cfg(unix)]
    #[test]
    fn a_cancelled_sends_last_word_waits_for_its_own_consoles_image() {
        let received = term_core::received::Received::new("annotated-cancel-elsewhere");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        let own = slot(&window).origin().unwrap().pane;
        click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (_, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        let cancel = place_of(&mut window, &ctx, "Cancel");
        press(&mut window, &ctx, cancel);
        let beside = window.active().panes()[0].name;
        let other = folder.0.join("other.png");
        write_png(&other, 64, 32);
        open_in(&mut window, &ctx, beside, &other);
        let reported = "the half-written copy could not be removed: busy";
        sender.send(Err(reported.into())).unwrap();
        let said = |shapes: &[egui::epaint::ClippedShape]| {
            texts(shapes, area()).iter().any(|t| t.contains(reported))
        };
        for _ in 0..3 {
            assert!(
                !said(&painted(&mut window, &ctx, vec![])),
                "said under another console's image"
            );
        }
        assert!(!window.image.send.parked());
        let active = window.active;
        window.tabs[active].focus = own;
        assert!(
            said(&painted(&mut window, &ctx, vec![])),
            "lost on the way to its own console"
        );
    }

    /// Every refusal Send can know before writing anything says why, keeps the marks, writes no
    /// copy and types nothing: a line longer than the console keeps, a paste waiting for an
    /// answer, a writer that gave up and a console with no folder; a console gone takes its
    /// image with it and says nothing under another's.
    #[cfg(unix)]
    #[test]
    fn a_refused_send_says_why_and_keeps_the_marks() {
        let received = term_core::received::Received::new("annotated-refused");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        std::fs::write(dir.join("keep"), b"").unwrap();
        let refused = |window: &mut crate::Window, ctx: &egui::Context, why: &str| {
            until_written(window, ctx);
            let notice = slot(&window).notice.clone().unwrap_or_default();
            assert!(
                // Refused before the copy was written, not written and then removed.
                notice.contains(why) && notice.ends_with("; nothing was sent."),
                "{notice}"
            );
            assert_eq!(marks_of(window).notes(), ["button clipped"]);
            assert!(received.so_far().is_empty());
            let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
            assert_eq!(entries.len(), 1, "a copy was written");
        };
        marks_mut(&mut window).general = "x".repeat(5000);
        click_send(&mut window, &ctx);
        refused(&mut window, &ctx, "longer than the origin console takes");
        marks_mut(&mut window).general.clear();
        let mut held = None;
        crate::keyboard::paste_or_hold(&"y".repeat(5000), false, &mut Vec::new(), &mut held);
        origin_pane(&mut window).held = held;
        click_send(&mut window, &ctx);
        refused(&mut window, &ctx, "paste waiting for an answer");
        origin_pane(&mut window).held = None;
        let live = std::mem::replace(
            &mut origin_pane(&mut window).pty,
            crate::launch::Process::deaf(),
        );
        click_send(&mut window, &ctx);
        refused(&mut window, &ctx, "cannot receive input");
        origin_pane(&mut window).pty = live;
        let dir_kept = origin_pane(&mut window).panel.take();
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(text_rect(&shapes, |t| t.contains("no folder of its own")).is_some());
        click_send(&mut window, &ctx);
        assert!(
            !window.image.send.asked(),
            "a Send with nowhere to write was asked"
        );
        assert_eq!(marks_of(&window).notes(), ["button clipped"]);
        // The console closes between the click and the frame that would write the copy.
        origin_pane(&mut window).panel = dir_kept;
        click_send(&mut window, &ctx);
        let name = slot(&window).origin().unwrap().pane;
        window.close_a_pane_called(name, &ctx).unwrap();
        until_written(&mut window, &ctx);
        // Its slot, marks and all, went with it: the refusal is for standard error, never for the
        // image of the console left.
        assert!(!window.image.sending());
        assert_eq!(slot(&window).notice, None);
        assert!(!slot(&window).shown());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The console closes while its copy is being written: the copy and the directory it left go
    /// with its image and marks, and nothing is typed anywhere, then or later.
    #[cfg(unix)]
    #[test]
    fn a_console_closed_while_its_copy_is_written_gets_nothing_and_keeps_no_copy() {
        let received = term_core::received::Received::new("annotated-closed");
        let other = term_core::received::Received::new("annotated-closed-other");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let beside = other.script("stty raw -echo");
        let (mut window, ctx, dir) = window_to_send_beside(&path, &received, &beside);
        other.wait_until_ready(Duration::from_secs(5));
        let line = click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (request, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        let copy = dir.join("annotated-1.png");
        assert_eq!(request, copy);
        flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
        let name = slot(&window).origin().unwrap().pane;
        window.close_a_pane_called(name, &ctx).unwrap();
        sender.send(Ok(copy.clone())).unwrap();
        for _ in 0..3 {
            painted(&mut window, &ctx, vec![]);
        }
        assert!(
            !copy.exists() && !dir.exists(),
            "the copy or its folder stayed"
        );
        // Its image and marks went with it, and what is left to say is not said under the image
        // of the console left.
        assert_eq!(slot(&window).notice, None);
        assert!(!line.is_empty());
        // Its own console is gone; the one left, which now has the keys, got nothing either.
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            other.so_far().is_empty(),
            "the line went to another console"
        );
        assert!(!window.image.sending());
    }

    /// A frame egui runs twice, as it does when a pass asks to be discarded, still keeps the
    /// Enter for a later frame: two writes microseconds apart would reach the console together.
    #[cfg(unix)]
    #[test]
    fn the_enter_waits_for_a_later_frame_when_a_frame_runs_twice() {
        let received = term_core::received::Received::new("annotated-twice");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        let line = click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (copy, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
        sender.send(Ok(copy.clone())).unwrap();
        let mut passes = 0;
        pass_with(&mut window, &ctx, vec![], |ctx| {
            passes += 1;
            if passes == 1 {
                ctx.request_discard("a layout that changed");
            }
        });
        assert_eq!(passes, 2, "the frame did not run twice");
        assert_eq!(
            received.wait_for(line.len(), Duration::from_secs(5)),
            line.as_bytes()
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            received.so_far(),
            line.as_bytes(),
            "Enter came in the same frame"
        );
        painted(&mut window, &ctx, vec![]);
        let submitted = [line.as_bytes(), b"\r"].concat();
        assert_eq!(
            received.wait_for(submitted.len(), Duration::from_secs(5)),
            submitted
        );
        let _ = std::fs::remove_file(&copy);
    }

    /// Moves the pointer to `at` in one pass and clicks there in the next two, as a person's
    /// pointer reaches a widget before pressing it.
    fn press(window: &mut crate::Window, ctx: &egui::Context, at: egui::Pos2) {
        painted(window, ctx, vec![egui::Event::PointerMoved(at)]);
        click(window, ctx, at);
    }

    /// Where the drawer painted the text `wanted` this pass.
    fn place_of(window: &mut crate::Window, ctx: &egui::Context, wanted: &str) -> egui::Pos2 {
        let shapes = painted(window, ctx, vec![]);
        text_rect(&shapes, |t| t == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} is not painted"))
            .1
            .center()
    }

    /// Where the drawer last painted the icon `glyph` this pass, inside the part of the drawer
    /// `part` picks: the strip above the drawer paints a cross of its own.
    fn icon_place(
        window: &mut crate::Window,
        ctx: &egui::Context,
        glyph: &str,
        part: fn(&tools::Layout) -> egui::Rect,
    ) -> egui::Pos2 {
        let shapes = painted(window, ctx, vec![]);
        let within = part(&layout(window, ctx));
        shapes
            .iter()
            .filter_map(|s| crate::ui::icons::painted_icon(&s.shape))
            .rfind(|(painted, _, rect)| painted == glyph && within.contains_rect(*rect))
            .unwrap_or_else(|| panic!("{glyph:?} is not painted"))
            .2
            .center()
    }

    /// Where the last marker's delete is.
    fn delete_place(window: &mut crate::Window, ctx: &egui::Context) -> egui::Pos2 {
        icon_place(window, ctx, egui_phosphor::regular::X, |l| l.column)
    }

    /// Where Undo is.
    fn undo_place(window: &mut crate::Window, ctx: &egui::Context) -> egui::Pos2 {
        icon_place(
            window,
            ctx,
            egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
            |l| l.bar,
        )
    }

    /// What the marks are, as far as a gesture can change them.
    fn state(window: &crate::Window) -> (usize, Vec<String>, String) {
        let marks = marks_of(window);
        let notes = marks.notes().into_iter().map(str::to_owned).collect();
        (marks.marks().len(), notes, marks.general.clone())
    }

    /// While a Send is on its way nothing edits the marks it is drawing: a click on the image, a
    /// marker's Delete, Undo, Clear, Ctrl+Z and typing into a note all do nothing, though each of
    /// them, tried first with no Send in flight, did what it says. The copy then carries the marks
    /// as the click fixed them, and the drawer says it was sent.
    #[cfg(unix)]
    #[test]
    fn a_send_in_flight_freezes_the_marks_and_their_notes() {
        let received = term_core::received::Received::new("annotated-frozen");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        let spot = image_view(&window, &ctx).center() + egui::vec2(-80.0, 30.0);
        let undo = egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: ctrl(),
        };
        let one = (1, vec!["button clipped".to_owned()], String::new());
        let two = |general: &str| {
            let notes = vec!["button clipped".to_owned(), String::new()];
            (2, notes, general.to_owned())
        };
        // With no Send in flight, every gesture does what it says.
        let clear = place_of(&mut window, &ctx, "Clear");
        press(&mut window, &ctx, clear);
        assert_eq!(state(&window), (0, vec![], String::new()), "Clear");
        press(&mut window, &ctx, spot);
        *marks_mut(&mut window).note_mut(0).unwrap() = "button clipped".into();
        assert_eq!(state(&window), one, "a click on the image");
        press(&mut window, &ctx, spot);
        assert_eq!(state(&window), two(""), "a second click on the image");
        // The last delete painted is the second marker's.
        let delete = delete_place(&mut window, &ctx);
        press(&mut window, &ctx, delete);
        assert_eq!(state(&window), one, "a marker's Delete");
        press(&mut window, &ctx, spot);
        let button = undo_place(&mut window, &ctx);
        press(&mut window, &ctx, button);
        assert_eq!(state(&window), one, "Undo");
        press(&mut window, &ctx, spot);
        painted(&mut window, &ctx, vec![undo.clone()]);
        assert_eq!(state(&window), one, "Ctrl+Z");
        let general = place_of(&mut window, &ctx, "A note on the whole image");
        press(&mut window, &ctx, general);
        painted(&mut window, &ctx, vec![egui::Event::Text("x".into())]);
        let fixed = (1, vec!["button clipped".to_owned()], "x".to_owned());
        assert_eq!(state(&window), fixed, "typing into the general note");

        let line = click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (copy, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(
            text_rect(&shapes, |t| t == "Sending…").is_some(),
            "the button says nothing"
        );
        press(&mut window, &ctx, spot);
        assert_eq!(state(&window), fixed, "a click on the image");
        let delete = delete_place(&mut window, &ctx);
        press(&mut window, &ctx, delete);
        assert_eq!(state(&window), fixed, "a marker's Delete");
        let undo_button = undo_place(&mut window, &ctx);
        press(&mut window, &ctx, undo_button);
        assert_eq!(state(&window), fixed, "Undo");
        let clear = place_of(&mut window, &ctx, "Clear");
        press(&mut window, &ctx, clear);
        assert_eq!(state(&window), fixed, "Clear");
        painted(&mut window, &ctx, vec![undo]);
        assert_eq!(state(&window), fixed, "Ctrl+Z");
        for field in ["x", "button clipped"] {
            let at = place_of(&mut window, &ctx, field);
            press(&mut window, &ctx, at);
            painted(&mut window, &ctx, vec![egui::Event::Text("!".into())]);
            assert_eq!(state(&window), fixed, "typing into {field:?}");
        }

        flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
        sender.send(Ok(copy.clone())).unwrap();
        painted(&mut window, &ctx, vec![]);
        let label = &slot(&window).origin().unwrap().label;
        assert_eq!(slot(&window).notice, Some(format!("Sent to {label}.")));
        assert!(line.ends_with("· 1: button clipped · Note: x"), "{line}");
        assert_eq!(
            received.wait_for(line.len(), Duration::from_secs(5)),
            line.as_bytes()
        );
        let _ = std::fs::remove_file(&copy);
    }

    /// While the copy is written, a Cancel beside «Sending…» takes the Send back: the marks and
    /// notes stay and can be edited again, and nothing is typed. While the cancelled worker is
    /// still writing, Send is greyed out and says why, whatever notice comes, and a Send asked
    /// anyway is refused with it; once that worker ends its copy is removed, the drawer says Send
    /// is ready, and the next Send goes through.
    #[cfg(unix)]
    #[test]
    fn cancel_takes_a_send_back_while_its_copy_is_written() {
        let received = term_core::received::Received::new("annotated-cancel");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (copy, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        let cancel = place_of(&mut window, &ctx, "Cancel");
        press(&mut window, &ctx, cancel);
        assert!(!window.image.sending(), "the Send was not taken back");
        assert!(window.image.send.parked(), "the worker was not kept");
        assert_eq!(marks_of(&window).notes(), ["button clipped"]);
        let general = place_of(&mut window, &ctx, "A note on the whole image");
        press(&mut window, &ctx, general);
        painted(&mut window, &ctx, vec![egui::Event::Text("x".into())]);
        assert_eq!(marks_of(&window).general, "x", "the notes stayed frozen");
        let parked = "Send cancelled; nothing was sent. Still finishing the previous copy.";
        assert_eq!(slot(&window).notice.as_deref(), Some(parked));

        let finishing = "Still finishing the previous copy…";
        let shapes = painted(&mut window, &ctx, vec![]);
        let (_, button) = text_rect(&shapes, |t| t == finishing).expect("Send says why it waits");
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::PointerMoved(button.center())],
        );
        click(&mut window, &ctx, button.center());
        assert!(!window.image.sending(), "a greyed-out Send was taken up");
        assert_eq!(slot(&window).notice.as_deref(), Some(parked));
        // Another notice takes the cancel's place; the button still says why it waits.
        let other = folder.0.join("other.png");
        let name = slot(&window).origin().unwrap().pane;
        assert!(crate::viewer::open_image_target(&mut window, name, other).is_err());
        assert_ne!(slot(&window).notice.as_deref(), Some(parked));
        let shapes = painted(&mut window, &ctx, vec![]);
        assert!(
            text_rect(&shapes, |t| t == finishing).is_some(),
            "the reason went"
        );
        window.image.send.ask(send::Request {
            document: slot(&window).document().unwrap(),
            pane: slot(&window).origin().unwrap().pane,
            path: copy.clone(),
            line: "x".into(),
        });
        painted(&mut window, &ctx, vec![]);
        let notice = slot(&window).notice.clone().unwrap_or_default();
        assert!(
            notice.starts_with("Still finishing the previous copy"),
            "{notice}"
        );
        assert!(!window.image.sending());

        flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
        sender.send(Ok(copy.clone())).unwrap();
        painted(&mut window, &ctx, vec![]);
        assert!(!window.image.send.parked());
        assert!(!copy.exists(), "the cancelled copy stayed");
        let notice = slot(&window).notice.clone().unwrap_or_default();
        assert!(
            notice.ends_with("Send is ready.") && !notice.contains("Still finishing"),
            "{notice}"
        );
        nothing_typed_before(&mut window, &received);

        let line = click_send(&mut window, &ctx);
        until_written(&mut window, &ctx);
        assert_eq!(
            received.wait_for(1 + line.len(), Duration::from_secs(5)),
            [&SENTINEL[..], line.as_bytes()].concat()
        );
        let _ = std::fs::remove_file(&copy);
    }

    /// A cancelled Send's worker that ends cleanly says Send is ready only where it is: not once
    /// the marks were cleared meanwhile. Its copy is gone either way, and the drawer says that
    /// much; a console that closed meanwhile took its image along, and nothing is said under the
    /// image of the console left.
    #[cfg(unix)]
    #[test]
    fn a_cancelled_send_says_send_is_ready_only_with_something_to_send() {
        for case in ["cleared", "closed"] {
            let received = term_core::received::Received::new("annotated-cancel-ready");
            let folder = Folder::new();
            let path = folder.0.join("shot.png");
            write_png(&path, 640, 400);
            let (mut window, ctx, _dir) = window_to_send(&path, &received);
            click_send(&mut window, &ctx);
            painted(&mut window, &ctx, vec![]);
            let (copy, sender) = window
                .image
                .send
                .stall()
                .expect("the copy is being written");
            let cancel = place_of(&mut window, &ctx, "Cancel");
            press(&mut window, &ctx, cancel);
            assert!(window.image.send.parked(), "the worker was not kept");
            flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
            if case == "cleared" {
                marks_mut(&mut window).clear();
            } else {
                let name = slot(&window).origin().unwrap().pane;
                window.close_a_pane_called(name, &ctx).unwrap();
            }
            sender.send(Ok(copy.clone())).unwrap();
            painted(&mut window, &ctx, vec![]);
            assert!(!window.image.send.parked());
            assert!(!copy.exists(), "the cancelled copy stayed");
            let said = (case == "cleared").then_some("Send cancelled; nothing was sent. Its copy is gone.");
            assert_eq!(slot(&window).notice.as_deref(), said, "{case}");
        }
    }

    /// A cancelled Send whose worker then fails says what the worker reported, which is where a
    /// half-written copy it could not remove is named. Said while the image is closed, it waits
    /// for the next image the drawer lays out instead of being cleared by that opening, and is
    /// shown there once.
    #[cfg(unix)]
    #[test]
    fn a_cancelled_send_whose_worker_fails_says_what_it_reported() {
        use crate::viewer::StripAction;
        let received = term_core::received::Received::new("annotated-cancel-failed");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        let name = slot(&window).origin().unwrap().pane;
        click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (_, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        let cancel = place_of(&mut window, &ctx, "Cancel");
        press(&mut window, &ctx, cancel);
        marks_mut(&mut window).clear();
        crate::viewer::apply_strip_action(&mut window, StripAction::CloseImage, &ctx);
        assert!(!slot(&window).shown(), "the image did not close");
        let reported = "the half-written copy could not be removed: busy";
        sender.send(Err(reported.into())).unwrap();
        painted(&mut window, &ctx, vec![]);
        assert!(!window.image.send.parked());
        let said = |shapes: &[egui::epaint::ClippedShape]| {
            texts(shapes, area()).iter().any(|t| t.contains(reported))
        };
        let reopen = |window: &mut crate::Window| {
            crate::viewer::open_image_target(window, name, path.clone()).unwrap();
            let started = Instant::now();
            loop {
                let shapes = painted(window, &ctx, vec![]);
                if image_rect(&shapes).is_some() {
                    break shapes;
                }
                assert!(started.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        assert!(said(&reopen(&mut window)), "the worker's failure was lost");
        assert!(!said(&reopen(&mut window)), "the failure is said again");
        nothing_typed_before(&mut window, &received);
    }

    /// What `nothing_typed_before` writes.
    #[cfg(unix)]
    const SENTINEL: &[u8; 1] = b"#";

    /// Writes a byte of its own to the image's console and waits for it: the writes reach the
    /// program in order, so a byte arriving first means nothing was typed before it, without
    /// guessing how long a typed line would take to arrive.
    #[cfg(unix)]
    fn nothing_typed_before(window: &mut crate::Window, received: &term_core::received::Received) {
        origin_pane(window).pty.write(SENTINEL).unwrap();
        assert_eq!(
            received.wait_for(SENTINEL.len(), Duration::from_secs(5)),
            SENTINEL,
            "something was typed before"
        );
    }

    /// A cancelled Send whose worker fails after its console closed has no image left to say it
    /// under: it goes to standard error, and the console left hears nothing of it.
    #[cfg(unix)]
    #[test]
    fn a_cancelled_send_whose_worker_fails_after_its_console_closed_says_so() {
        let received = term_core::received::Received::new("annotated-cancel-closed");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (_, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        let cancel = place_of(&mut window, &ctx, "Cancel");
        press(&mut window, &ctx, cancel);
        let name = slot(&window).origin().unwrap().pane;
        window.close_a_pane_called(name, &ctx).unwrap();
        sender.send(Err("disk full".into())).unwrap();
        for _ in 0..3 {
            painted(&mut window, &ctx, vec![]);
        }
        assert!(!window.image.send.parked());
        assert_eq!(slot(&window).notice, None);
    }

    /// Clicks Send, takes the copy's write over once it `started`, and replaces the image, as the
    /// test helpers can though the drawer would not let a person: the marks cleared and another
    /// image opened from the same console, given a note of its own once decoded.
    #[cfg(unix)]
    fn replace_while_sending(
        window: &mut crate::Window,
        ctx: &egui::Context,
        other: &Path,
        started: bool,
    ) -> Option<(PathBuf, std::sync::mpsc::Sender<Result<PathBuf, String>>)> {
        let name = slot(&window).origin().unwrap().pane;
        click_send(window, ctx);
        if started {
            painted(window, ctx, vec![]);
        }
        let writing = window.image.send.stall();
        marks_mut(window).clear();
        crate::viewer::open_image_target(window, name, other.to_owned()).unwrap();
        // Decoded without a pass, so that the asked Send is still waiting for its frame.
        let begun = Instant::now();
        while !slot(&window).loaded() {
            slot_mut(window).poll(ctx);
            assert!(begun.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(5));
        }
        marks_mut(window).general = "the new image's note".into();
        writing
    }

    /// A Send whose image was replaced by another from the same console, while its copy was
    /// written or before it started, types nothing, keeps the new image's marks, says it was not
    /// sent, and leaves no copy behind.
    #[cfg(unix)]
    #[test]
    fn a_send_whose_image_was_replaced_neither_clears_the_new_marks_nor_says_sent() {
        let received = term_core::received::Received::new("annotated-replaced");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let other = folder.0.join("other.png");
        write_png(&other, 320, 200);
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        let replaced = |window: &crate::Window| {
            let notice = slot(&window).notice.clone().unwrap_or_default();
            assert!(notice.contains("no longer the one shown"), "{notice}");
            assert_eq!(marks_of(window).general, "the new image's note");
            assert!(received.so_far().is_empty());
            assert!(!window.image.sending());
        };
        // While its copy is written.
        let (copy, sender) = replace_while_sending(&mut window, &ctx, &other, true)
            .expect("the copy is being written");
        flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
        sender.send(Ok(copy.clone())).unwrap();
        painted(&mut window, &ctx, vec![]);
        replaced(&window);
        assert!(!copy.exists(), "the copy stayed");
        // Before it started: asked, and the image replaced before the next frame.
        assert!(replace_while_sending(&mut window, &ctx, &path, false).is_none());
        painted(&mut window, &ctx, vec![]);
        replaced(&window);
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "a copy was written"
        );
    }

    /// A name already taken in the console's folder, by a file or by a link to nothing, is
    /// skipped: the line shows the first free name, a gap before a taken one included, and Send
    /// writes there, leaving the others be.
    #[cfg(unix)]
    #[test]
    fn send_takes_the_first_name_free_in_the_folder() {
        let received = term_core::received::Received::new("annotated-free");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        let dangling = dir.join("annotated-2.png");
        std::os::unix::fs::symlink(dir.join("nowhere"), &dangling).unwrap();
        // A gap before a taken name is the first free one, not the name after the taken one.
        let shapes = painted(&mut window, &ctx, vec![]);
        let (line, _) = text_rect(&shapes, |t| t.starts_with("Annotated screenshot: "))
            .expect("the line is painted");
        let first = dir.join("annotated-1.png");
        assert!(line.contains(&first.display().to_string()), "{line}");
        let foreign = first;
        std::fs::write(&foreign, b"not ours").unwrap();
        let copy = dir.join("annotated-3.png");
        let shown = click_send(&mut window, &ctx);
        assert!(shown.contains(&copy.display().to_string()), "{shown}");
        until_written(&mut window, &ctx);
        let label = &slot(&window).origin().unwrap().label;
        assert_eq!(slot(&window).notice, Some(format!("Sent to {label}.")));
        assert!(copy.exists());
        assert_eq!(std::fs::read(&foreign).unwrap(), b"not ours");
        assert!(std::fs::symlink_metadata(&dangling).is_ok());
        let _ = std::fs::remove_file(&copy);
        let _ = std::fs::remove_file(&foreign);
        let _ = std::fs::remove_file(&dangling);
    }

    /// A refused delivery whose copy cannot be removed says so, with the copy's path and why.
    #[cfg(unix)]
    #[test]
    fn a_copy_that_cannot_be_removed_is_named_with_the_error() {
        let received = term_core::received::Received::new("annotated-stuck");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (copy, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        // A directory where the copy should be: `remove_file` refuses it.
        std::fs::create_dir(&copy).unwrap();
        let mut held = None;
        crate::keyboard::paste_or_hold(&"y".repeat(5000), false, &mut Vec::new(), &mut held);
        origin_pane(&mut window).held = held;
        sender.send(Ok(copy.clone())).unwrap();
        painted(&mut window, &ctx, vec![]);
        let notice = slot(&window).notice.clone().unwrap_or_default();
        assert!(
            notice.contains(&format!(
                "the copy at {} could not be removed: ",
                copy.display()
            )),
            "{notice}"
        );
        assert!(received.so_far().is_empty());
        let _ = std::fs::remove_dir(&copy);
    }

    /// Two Sends to one console write `annotated-1.png` and `annotated-2.png`, and closing the
    /// console takes both, and then its folder.
    #[cfg(unix)]
    #[test]
    fn a_console_that_closes_takes_every_copy_sent_to_it_and_its_folder() {
        let received = term_core::received::Received::new("annotated-twice-sent");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        let mut typed = Vec::new();
        for n in 1..=2 {
            if n == 2 {
                let at = image_view(&window, &ctx).center();
                press(&mut window, &ctx, at);
                *marks_mut(&mut window).note_mut(0).unwrap() = "again".into();
            }
            let line = click_send(&mut window, &ctx);
            let copy = dir.join(format!("annotated-{n}.png"));
            assert!(line.contains(&copy.display().to_string()), "{line}");
            until_written(&mut window, &ctx);
            painted(&mut window, &ctx, vec![]);
            typed.extend_from_slice(line.as_bytes());
            typed.push(b'\r');
            assert_eq!(
                received.wait_for(typed.len(), Duration::from_secs(5)),
                typed
            );
            assert!(copy.exists());
        }
        let name = slot(&window).origin().unwrap().pane;
        window.close_a_pane_called(name, &ctx).unwrap();
        assert!(!dir.exists(), "the copies or the folder stayed");
    }

    /// The console closes while its copy is written and the write then fails: the directory the
    /// console could not remove, because the half-written copy was in it, goes once the copy is
    /// gone, and what the worker met goes to standard error, not under the image of the console
    /// left.
    #[cfg(unix)]
    #[test]
    fn a_write_that_fails_after_its_console_closed_says_so_and_leaves_no_folder() {
        let received = term_core::received::Received::new("annotated-failed-closed");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (copy, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        std::fs::write(&copy, b"half").unwrap();
        let name = slot(&window).origin().unwrap().pane;
        window.close_a_pane_called(name, &ctx).unwrap();
        assert!(
            dir.exists(),
            "the premise: the half-written copy keeps the folder"
        );
        // What `flatten::write_new` does with a copy it could not finish.
        std::fs::remove_file(&copy).unwrap();
        sender
            .send(Err(format!(
                "The copy could not be written at {}: disk full",
                copy.display()
            )))
            .unwrap();
        painted(&mut window, &ctx, vec![]);
        assert_eq!(slot(&window).notice, None);
        assert!(!dir.exists(), "the folder stayed");
        assert!(!window.image.sending());
    }

    /// A file that takes the shown name between the click and the frame that starts the copy
    /// refuses the Send: writing under another name would type a line other than the one shown.
    #[cfg(unix)]
    #[test]
    fn a_name_taken_after_the_line_was_shown_refuses_the_send() {
        let received = term_core::received::Received::new("annotated-taken");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        let shown = click_send(&mut window, &ctx);
        let taken = dir.join("annotated-1.png");
        assert!(shown.contains(&taken.display().to_string()), "{shown}");
        std::fs::write(&taken, b"theirs").unwrap();
        until_written(&mut window, &ctx);
        assert_eq!(
            slot(&window).notice.as_deref(),
            Some("The copy's name changed since the line was shown; nothing was sent.")
        );
        assert_eq!(std::fs::read(&taken).unwrap(), b"theirs");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "a copy was written"
        );
        assert_eq!(marks_of(&window).notes(), ["button clipped"]);
        assert!(received.so_far().is_empty());
        let _ = std::fs::remove_file(&taken);
    }

    /// While the copy is written, and so takes the name the line showed, the line painted stays
    /// the one the Send types, and a second Send asked meanwhile replaces nothing: the first one's
    /// line is the one the console receives.
    #[cfg(unix)]
    #[test]
    fn while_the_copy_is_written_the_line_shown_is_the_line_it_types() {
        let received = term_core::received::Received::new("annotated-in-flight");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        let shown = click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let (copy, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        assert_eq!(copy, dir.join("annotated-1.png"));
        // The worker's copy, on disk while it is still being written.
        flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
        let shapes = painted(&mut window, &ctx, vec![]);
        let (painting, _) = text_rect(&shapes, |t| t.starts_with("Annotated screenshot: "))
            .expect("the line is painted while the copy is written");
        assert_eq!(painting, shown);
        let document = slot(&window).document().unwrap();
        let pane = slot(&window).origin().unwrap().pane;
        window.image.send.ask(send::Request {
            document,
            pane,
            path: dir.join("annotated-9.png"),
            line: "a second Send".into(),
        });
        sender.send(Ok(copy.clone())).unwrap();
        painted(&mut window, &ctx, vec![]);
        let label = &slot(&window).origin().unwrap().label;
        assert_eq!(slot(&window).notice, Some(format!("Sent to {label}.")));
        assert_eq!(
            received.wait_for(shown.len(), Duration::from_secs(5)),
            shown.as_bytes()
        );
        let _ = std::fs::remove_file(&copy);
    }

    /// Ctrl+Z in the same pass as the click on Send takes nothing back: the Send it asked draws
    /// the marks as they were at the click. The same Ctrl+Z with no Send undoes the last mark.
    #[cfg(unix)]
    #[test]
    fn ctrl_z_in_the_pass_that_clicks_send_takes_nothing_back() {
        let received = term_core::received::Received::new("annotated-same-pass");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        let undo = egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: ctrl(),
        };
        let spot = image_view(&window, &ctx).center() + egui::vec2(-80.0, 30.0);
        press(&mut window, &ctx, spot);
        assert_eq!(marks_of(&window).marks().len(), 2);
        painted(&mut window, &ctx, vec![undo.clone()]);
        assert_eq!(marks_of(&window).marks().len(), 1, "Ctrl+Z with no Send");
        press(&mut window, &ctx, spot);
        let label = slot(&window).origin().unwrap().label.clone();
        let shapes = painted(&mut window, &ctx, vec![]);
        let (_, button) = text_rect(&shapes, |t| t == format!("Send to {label} and press Enter"))
            .expect("the button names the console");
        let at = button.center();
        let pointer = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        painted(&mut window, &ctx, vec![egui::Event::PointerMoved(at)]);
        painted(
            &mut window,
            &ctx,
            vec![egui::Event::PointerMoved(at), pointer(true)],
        );
        painted(&mut window, &ctx, vec![pointer(false), undo]);
        assert!(window.image.sending(), "the click asked no Send");
        assert_eq!(
            marks_of(&window).marks().len(),
            2,
            "Ctrl+Z took a mark back"
        );
        until_written(&mut window, &ctx);
        let label = &slot(&window).origin().unwrap().label;
        assert_eq!(slot(&window).notice, Some(format!("Sent to {label}.")));
    }

    /// An earlier name freed between the click and the frame that starts the copy refuses the
    /// Send too (D113): the first free name is no longer the one the line showed, though the
    /// shown one is still free.
    #[cfg(unix)]
    #[test]
    fn an_earlier_name_freed_after_the_line_was_shown_refuses_the_send() {
        let received = term_core::received::Received::new("annotated-freed");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, dir) = window_to_send(&path, &received);
        let earlier = dir.join("annotated-1.png");
        std::fs::write(&earlier, b"theirs").unwrap();
        let shown = click_send(&mut window, &ctx);
        let named = dir.join("annotated-2.png");
        assert!(shown.contains(&named.display().to_string()), "{shown}");
        std::fs::remove_file(&earlier).unwrap();
        until_written(&mut window, &ctx);
        assert_eq!(
            slot(&window).notice.as_deref(),
            Some("The copy's name changed since the line was shown; nothing was sent.")
        );
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "a copy was written"
        );
        assert_eq!(marks_of(&window).notes(), ["button clipped"]);
        std::thread::sleep(Duration::from_millis(200));
        assert!(received.so_far().is_empty());
    }

    /// Nothing reaches the console while its copy is still being written, through several frames;
    /// the line goes only once the worker has closed the file.
    #[cfg(unix)]
    #[test]
    fn nothing_is_typed_until_the_copy_is_closed() {
        let received = term_core::received::Received::new("annotated-not-yet");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let (mut window, ctx, _dir) = window_to_send(&path, &received);
        let line = click_send(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            received.so_far().is_empty(),
            "typed before the copy was closed"
        );
        let (copy, sender) = window
            .image
            .send
            .stall()
            .expect("the copy is being written");
        for _ in 0..3 {
            painted(&mut window, &ctx, vec![]);
        }
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            received.so_far().is_empty(),
            "typed before the copy was closed"
        );
        assert!(window.image.send.writing());
        flatten::write_new(&copy, &::image::RgbaImage::new(4, 4)).unwrap();
        sender.send(Ok(copy.clone())).unwrap();
        painted(&mut window, &ctx, vec![]);
        painted(&mut window, &ctx, vec![]);
        let submitted = [line.as_bytes(), b"\r"].concat();
        assert_eq!(
            received.wait_for(submitted.len(), Duration::from_secs(5)),
            submitted
        );
        let _ = std::fs::remove_file(&copy);
    }

    /// The person switches to another tab while the copy is written: the line and its Enter still
    /// reach the image's own console, and the other tab's consoles get nothing.
    #[cfg(unix)]
    #[test]
    fn a_send_reaches_its_console_while_another_tab_is_shown() {
        let received = term_core::received::Received::new("annotated-other-tab");
        let other = term_core::received::Received::new("annotated-other-tab-beside");
        let folder = Folder::new();
        let path = folder.0.join("shot.png");
        write_png(&path, 640, 400);
        let script = received.script("stty raw -echo");
        let beside = other.script("stty raw -echo");
        let (mut window, ctx, _) =
            window_with_image_in_tabs(&path, &[["sleep 30", &script], [&beside, "sleep 30"]]);
        received.wait_until_ready(Duration::from_secs(5));
        other.wait_until_ready(Duration::from_secs(5));
        let dir = crate::PanelDir::create().unwrap();
        let copy_dir = dir.path().to_owned();
        origin_pane(&mut window).panel = Some(dir);
        marks_mut(&mut window).tool = marks::Tool::Marker;
        let at = image_view(&window, &ctx).center();
        press(&mut window, &ctx, at);
        *marks_mut(&mut window).note_mut(0).unwrap() = "button clipped".into();
        let line = click_send(&mut window, &ctx);
        assert!(line.contains("annotated-1.png"), "{line}");
        window.active = 1;
        until_written(&mut window, &ctx);
        painted(&mut window, &ctx, vec![]);
        let submitted = [line.as_bytes(), b"\r"].concat();
        assert_eq!(
            received.wait_for(submitted.len(), Duration::from_secs(5)),
            submitted
        );
        let label = &slot(&window).origin().unwrap().label;
        assert_eq!(slot(&window).notice, Some(format!("Sent to {label}.")));
        assert!(other.so_far().is_empty(), "the line went to another tab");
        let _ = std::fs::remove_file(copy_dir.join("annotated-1.png"));
    }
}
