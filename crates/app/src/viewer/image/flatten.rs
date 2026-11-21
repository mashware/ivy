//! The copy Send writes: the original screenshot at its own resolution with the marks drawn into
//! it, as a PNG, on a worker so a large image does not stop the window.
use super::marks::{self, Shape, Style};
use crate::viewer::load::{Job, spawn};
use eframe::egui;
use skrifa::MetadataProvider;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tiny_skia as sk;

/// The face the markers' numbers are drawn with, from the faces Ivy ships.
const DIGITS: &str = "Geist SemiBold";

/// Flattens `marks` into a copy of `original` and writes it at `path`, answering with that path
/// once the file is written and closed.
pub(super) fn start(
    original: Arc<image::RgbaImage>,
    marks: Vec<Shape>,
    path: PathBuf,
    ctx: egui::Context,
) -> Result<Job<PathBuf>, String> {
    spawn(ctx, move || {
        let flat = flatten(&original, &marks)?;
        write_new(&path, &flat)?;
        Ok(path)
    })
}

/// `original` with `marks` drawn over it, antialiased, at the original's resolution.
pub(crate) fn flatten(
    original: &image::RgbaImage,
    marks: &[Shape],
) -> Result<image::RgbaImage, String> {
    let (width, height) = original.dimensions();
    let size = sk::IntSize::from_wh(width, height).ok_or("The image has no pixels.")?;
    // tiny-skia keeps its pixels premultiplied and `image` keeps them straight.
    let data = original
        .pixels()
        .flat_map(|p| {
            let c = sk::ColorU8::from_rgba(p[0], p[1], p[2], p[3]).premultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    let mut pixmap = sk::Pixmap::from_vec(data, size).ok_or("The image could not be copied.")?;
    let style = Style::of(egui::vec2(width as f32, height as f32));
    let mut ink = sk::Paint::default();
    let [r, g, b, a] = marks::INK;
    ink.set_color_rgba8(r, g, b, a);
    ink.anti_alias = true;
    let line = sk::Stroke {
        width: style.stroke,
        line_cap: sk::LineCap::Round,
        line_join: sk::LineJoin::Round,
        ..Default::default()
    };
    let font = skrifa::FontRef::new(face()).map_err(|e| format!("The digits' face: {e}"))?;
    let mut number = 0;
    for mark in marks {
        let stroke = |path: Option<sk::Path>, pixmap: &mut sk::Pixmap| {
            if let Some(path) = path {
                pixmap.stroke_path(&path, &ink, &line, sk::Transform::identity(), None);
            }
        };
        match *mark {
            Shape::Arrow { from, to } => {
                let [tip, left, right] = marks::head(from, to, &style);
                let base = left.lerp(right, 0.5);
                let mut shaft = sk::PathBuilder::new();
                shaft.move_to(from.x, from.y);
                shaft.line_to(base.x, base.y);
                stroke(shaft.finish(), &mut pixmap);
                fill(&mut pixmap, polygon(&[tip, left, right]), &ink);
            }
            Shape::Ellipse(rect) => {
                stroke(
                    sk_rect(rect).and_then(sk::PathBuilder::from_oval),
                    &mut pixmap,
                );
            }
            Shape::Rectangle(rect) => {
                stroke(sk_rect(rect).map(sk::PathBuilder::from_rect), &mut pixmap);
            }
            Shape::Marker { at, .. } => {
                number += 1;
                fill(
                    &mut pixmap,
                    sk::PathBuilder::from_circle(at.x, at.y, style.radius),
                    &ink,
                );
                let mut white = sk::Paint::default();
                white.set_color_rgba8(255, 255, 255, 255);
                white.anti_alias = true;
                fill(
                    &mut pixmap,
                    digits(&font, number, at, style.radius * 1.1),
                    &white,
                );
            }
        }
    }
    let data = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(width, height, data)
        .ok_or_else(|| "The copy could not be built.".into())
}

/// Writes `image` as a PNG at `path`, which must not exist yet; on Unix readable by its owner only,
/// elsewhere with the platform's default permissions. A file half written is removed: it was
/// created here, so nobody else's file goes with it.
pub(crate) fn write_new(path: &Path, image: &image::RgbaImage) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|e| format!("The copy could not be created at {}: {e}", path.display()))?;
    let mut writer = std::io::BufWriter::new(file);
    let written = image
        .write_with_encoder(image::codecs::png::PngEncoder::new(&mut writer))
        .map_err(|e| e.to_string())
        .and_then(|()| writer.flush().map_err(|e| e.to_string()));
    written.map_err(|why| {
        drop(writer);
        abandon(path, &why)
    })
}

/// Removes the copy a write gave up on at `path`, and answers what to say: why the write failed
/// and, when the half-written copy could not be removed, that it stays and why, as `deliver` says
/// of a copy it could not remove. One that is already gone is not worth a word.
fn abandon(path: &Path, why: &str) -> String {
    let left = match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            format!("; the half-written copy could not be removed: {error}")
        }
        _ => String::new(),
    };
    format!(
        "The copy could not be written at {}: {why}{left}",
        path.display()
    )
}

fn face() -> &'static [u8] {
    crate::fonts::FACES
        .iter()
        .find(|(name, _)| *name == DIGITS)
        .map(|(_, bytes)| *bytes)
        .expect("Ivy ships the face its markers are numbered with")
}

fn sk_rect(rect: egui::Rect) -> Option<sk::Rect> {
    sk::Rect::from_ltrb(rect.left(), rect.top(), rect.right(), rect.bottom())
}

fn polygon(points: &[egui::Pos2]) -> Option<sk::Path> {
    let mut path = sk::PathBuilder::new();
    let (first, rest) = points.split_first()?;
    path.move_to(first.x, first.y);
    for point in rest {
        path.line_to(point.x, point.y);
    }
    path.close();
    path.finish()
}

fn fill(pixmap: &mut sk::Pixmap, path: Option<sk::Path>, paint: &sk::Paint) {
    if let Some(path) = path {
        pixmap.fill_path(
            &path,
            paint,
            sk::FillRule::Winding,
            sk::Transform::identity(),
            None,
        );
    }
}

/// The outline of `number` set in `font` at `size` pixels, centred on `center`.
fn digits(
    font: &skrifa::FontRef,
    number: usize,
    center: egui::Pos2,
    size: f32,
) -> Option<sk::Path> {
    let location = skrifa::instance::LocationRef::default();
    let px = skrifa::instance::Size::new(size);
    let metrics = font.glyph_metrics(px, location);
    let charmap = font.charmap();
    let outlines = font.outline_glyphs();
    let glyphs: Vec<_> = number
        .to_string()
        .chars()
        .filter_map(|c| charmap.map(c))
        .collect();
    let width: f32 = glyphs
        .iter()
        .filter_map(|&g| metrics.advance_width(g))
        .sum();
    let cap = font.metrics(px, location).cap_height.unwrap_or(size * 0.7);
    let mut pen = Pen {
        path: sk::PathBuilder::new(),
        origin: egui::pos2(center.x - width / 2.0, center.y + cap / 2.0),
    };
    for glyph in glyphs {
        if let Some(outline) = outlines.get(glyph) {
            let settings = skrifa::outline::DrawSettings::unhinted(px, location);
            outline.draw(settings, &mut pen).ok()?;
        }
        pen.origin.x += metrics.advance_width(glyph).unwrap_or(0.0);
    }
    pen.path.finish()
}

/// Turns a glyph's outline, whose y grows upwards from the baseline, into a path on the image,
/// whose y grows downwards.
struct Pen {
    path: sk::PathBuilder,
    origin: egui::Pos2,
}

impl Pen {
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        (self.origin.x + x, self.origin.y - y)
    }
}

impl skrifa::outline::OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.at(x, y);
        self.path.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.at(x, y);
        self.path.line_to(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (cx0, cy0) = self.at(cx0, cy0);
        let (x, y) = self.at(x, y);
        self.path.quad_to(cx0, cy0, x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (cx0, cy0) = self.at(cx0, cy0);
        let (cx1, cy1) = self.at(cx1, cy1);
        let (x, y) = self.at(x, y);
        self.path.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }

    fn close(&mut self) {
        self.path.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grey(width: u32, height: u32) -> image::RgbaImage {
        image::RgbaImage::from_pixel(width, height, image::Rgba([90, 90, 90, 255]))
    }

    /// The copy has the original's size, the marks' ink where they were drawn, a white number on
    /// a marker's disc, and the original's pixels everywhere else.
    #[test]
    fn the_copy_keeps_the_size_and_draws_the_marks_where_they_are() {
        let mut original = grey(900, 600);
        original.put_pixel(850, 550, image::Rgba([90, 90, 90, 128]));
        let rect = egui::Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(300.0, 200.0));
        let marks = [
            Shape::Rectangle(rect),
            Shape::Marker {
                serial: 1,
                at: egui::pos2(600.0, 400.0),
                note: "here".into(),
            },
        ];
        let copy = flatten(&original, &marks).unwrap();
        assert_eq!(copy.dimensions(), (900, 600));
        let [r, g, b, _] = marks::INK;
        assert_eq!(
            copy.get_pixel(200, 100).0,
            [r, g, b, 255],
            "the rectangle's top edge"
        );
        assert_eq!(
            copy.get_pixel(200, 150).0,
            [90, 90, 90, 255],
            "inside the rectangle"
        );
        assert_eq!(
            copy.get_pixel(800, 50).0,
            [90, 90, 90, 255],
            "away from every mark"
        );
        // A translucent pixel keeps its colour through tiny-skia's premultiplied pixels.
        let [r1, g1, b1, a1] = copy.get_pixel(850, 550).0;
        assert!([r1, g1, b1].iter().all(|c| c.abs_diff(90) <= 1) && a1 == 128);
        let style = Style::of(egui::vec2(900.0, 600.0));
        let disc = (600 - style.radius as u32 + 2..600 + style.radius as u32 - 2)
            .map(|x| copy.get_pixel(x, 400).0)
            .collect::<Vec<_>>();
        assert!(disc.contains(&[r, g, b, 255]), "no disc");
        assert!(disc.contains(&[255, 255, 255, 255]), "no digit on the disc");
    }

    fn rect(min: (f32, f32), max: (f32, f32)) -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(min.0, min.1), egui::pos2(max.0, max.1))
    }

    /// An arrow is inked along its shaft and at its head, an ellipse at the middle of its sides
    /// but not at its box's corners, where a rectangle is, and nothing behind an arrow's tail.
    #[test]
    fn each_shape_is_drawn_as_itself() {
        let marks = [
            Shape::Arrow {
                from: egui::pos2(500.0, 100.0),
                to: egui::pos2(800.0, 100.0),
            },
            Shape::Ellipse(rect((100.0, 300.0), (300.0, 500.0))),
            Shape::Rectangle(rect((500.0, 300.0), (700.0, 500.0))),
        ];
        let copy = flatten(&grey(900, 600), &marks).unwrap();
        let [r, g, b, _] = marks::INK;
        let ink = [r, g, b, 255];
        let grey = [90, 90, 90, 255];
        for (x, y, expected, what) in [
            (650, 100, ink, "the arrow's shaft"),
            (790, 100, ink, "the arrow's head"),
            (480, 100, grey, "behind the arrow's tail"),
            (100, 400, ink, "the ellipse's left side"),
            (200, 300, ink, "the ellipse's top"),
            (100, 300, grey, "the ellipse's box corner"),
            (500, 300, ink, "the rectangle's corner"),
            (600, 400, grey, "inside the rectangle"),
        ] {
            assert_eq!(copy.get_pixel(x, y).0, expected, "{what}");
        }
    }

    /// A marker's number is its place among the markers: the same marker reads differently when
    /// another marker comes before it.
    #[test]
    fn a_markers_number_is_its_place_among_the_markers() {
        let marker = |x: f32| Shape::Marker {
            serial: 1,
            at: egui::pos2(x, 300.0),
            note: String::new(),
        };
        let first = flatten(&grey(900, 600), &[marker(600.0)]).unwrap();
        let second = flatten(&grey(900, 600), &[marker(200.0), marker(600.0)]).unwrap();
        let disc = |copy: &image::RgbaImage| {
            let radius = Style::of(egui::vec2(900.0, 600.0)).radius as u32;
            (600 - radius..600 + radius)
                .flat_map(|x| (300 - radius..300 + radius).map(move |y| (x, y)))
                .map(|(x, y)| copy.get_pixel(x, y).0)
                .collect::<Vec<_>>()
        };
        assert_ne!(
            disc(&first),
            disc(&second),
            "both discs carry the same number"
        );
        // A shape that is not a marker, drawn clear of the disc, takes no number.
        let boxed = [
            Shape::Rectangle(rect((50.0, 50.0), (150.0, 150.0))),
            marker(600.0),
        ];
        let after_a_box = flatten(&grey(900, 600), &boxed).unwrap();
        assert_eq!(
            disc(&first),
            disc(&after_a_box),
            "the rectangle took a number"
        );
    }

    /// A write given up on removes its half-written copy and says only why it failed; one whose
    /// copy cannot be removed says that too, with why.
    #[test]
    fn a_failed_write_says_when_its_half_written_copy_stays() {
        let folder = std::env::temp_dir().join(format!("ivy-abandon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir(&folder).unwrap();
        let half = folder.join("annotated-1.png");
        std::fs::write(&half, b"half").unwrap();
        let said = abandon(&half, "disk full");
        assert_eq!(
            said,
            format!(
                "The copy could not be written at {}: disk full",
                half.display()
            )
        );
        assert!(!half.exists(), "the half-written copy stayed");
        // A directory where the copy was: `remove_file` refuses it.
        let stuck = folder.join("annotated-2.png");
        std::fs::create_dir(&stuck).unwrap();
        let said = abandon(&stuck, "disk full");
        assert!(
            said.starts_with(&format!(
                "The copy could not be written at {}: disk full; the half-written copy could not \
                 be removed: ",
                stuck.display()
            )),
            "{said}"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A copy is never written over an existing file, and what is written decodes to the copy.
    #[test]
    fn a_copy_is_written_new_and_never_over_a_file() {
        let folder = std::env::temp_dir().join(format!("ivy-flatten-{}", std::process::id()));
        let _ = std::fs::create_dir(&folder);
        let path = folder.join("annotated-1.png");
        let _ = std::fs::remove_file(&path);
        let copy = grey(30, 20);
        write_new(&path, &copy).unwrap();
        assert_eq!(image::open(&path).unwrap().to_rgba8(), copy);
        let again = write_new(&path, &grey(10, 10));
        assert!(again.is_err());
        assert_eq!(image::open(&path).unwrap().to_rgba8(), copy);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&folder);
    }
}
