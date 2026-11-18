//! Headless epaint atlas comparison. See docs/console-glyph-hinting.md for interpretation.
use eframe::egui::{self, epaint::text::TextOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use skrifa::{
    MetadataProvider as _,
    instance::{LocationRef, Size},
    outline::{DrawSettings, HintingInstance, SmoothMode, Target, pen::NullPen},
};
use std::{error::Error, path::Path, sync::Arc};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const GLYPHS: &str = "Iil1mw0O+-={}";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Sample {
    chr: char,
    glyph_id: u32,
    position: [f32; 2],
    advance: f32,
    offset: [f32; 2],
    uv: [[u16; 2]; 2],
    origin: [i32; 2],
    size: [usize; 2],
    alpha: Vec<u8>,
}

fn difference(a: &Sample, b: &Sample) -> (usize, u64) {
    let left = a.origin[0].min(b.origin[0]);
    let top = a.origin[1].min(b.origin[1]);
    let right = (a.origin[0] + a.size[0] as i32).max(b.origin[0] + b.size[0] as i32);
    let bottom = (a.origin[1] + a.size[1] as i32).max(b.origin[1] + b.size[1] as i32);
    let at = |s: &Sample, x: i32, y: i32| -> u8 {
        let x = x - s.origin[0];
        let y = y - s.origin[1];
        if x < 0 || y < 0 || x >= s.size[0] as i32 || y >= s.size[1] as i32 {
            0
        } else {
            s.alpha[y as usize * s.size[0] + x as usize]
        }
    };
    let mut count = 0;
    let mut total = 0;
    for y in top..bottom {
        for x in left..right {
            let delta = at(a, x, y).abs_diff(at(b, x, y));
            count += usize::from(delta != 0);
            total += u64::from(delta);
        }
    }
    (count, total)
}

fn samples(
    bytes: &[u8],
    hinting: Option<bool>,
    size: f32,
    scale: f32,
    text: &str,
    run: bool,
) -> Result<Vec<Sample>> {
    eprintln!(
        "HINT_SAMPLE hinting={hinting:?} points={size} scale={scale} text={text:?} no_bins={run}"
    );
    let face = skrifa::FontRef::from_index(bytes, 0)?;
    for chr in text.chars() {
        if face
            .charmap()
            .map(chr)
            .is_none_or(|id| id == skrifa::GlyphId::NOTDEF)
        {
            return Err(format!("missing glyph: {chr:?}").into());
        }
    }
    let mut data = egui::FontData::from_owned(bytes.to_vec());
    data.tweak.hinting = hinting;
    // Ivy registers a second copy without bins for its optional shaped ASCII path.
    if run {
        data.tweak.subpixel_binning = Some(false);
    }
    let mut definitions = egui::FontDefinitions::empty();
    definitions
        .font_data
        .insert("exact-face".into(), Arc::new(data));
    definitions
        .families
        .insert(egui::FontFamily::Monospace, vec!["exact-face".into()]);
    let mut fonts = egui::epaint::Fonts::new(TextOptions::default(), definitions);
    let font = egui::FontId::monospace(size);
    let galley =
        fonts
            .with_pixels_per_point(scale)
            .layout_no_wrap(text.into(), font, egui::Color32::WHITE);
    let image = fonts.image();
    let mut result = Vec::new();
    for row in &galley.rows {
        for glyph in &row.glyphs {
            let uv = glyph.uv_rect;
            let width = usize::from(uv.max[0] - uv.min[0]);
            let height = usize::from(uv.max[1] - uv.min[1]);
            let mut alpha = Vec::with_capacity(width * height);
            for y in uv.min[1]..uv.max[1] {
                for x in uv.min[0]..uv.max[0] {
                    alpha.push(image[(usize::from(x), usize::from(y))].a());
                }
            }
            if !alpha.iter().any(|&a| a != 0) {
                return Err(format!("empty glyph allocation: {:?}", glyph.chr).into());
            }
            result.push(Sample {
                chr: glyph.chr,
                glyph_id: face
                    .charmap()
                    .map(glyph.chr)
                    .ok_or("missing mapped glyph")?
                    .to_u32(),
                position: [glyph.pos.x, glyph.pos.y],
                advance: glyph.advance_width,
                offset: [uv.offset.x, uv.offset.y],
                uv: [uv.min, uv.max],
                // Baseline at y=0; x retains the run position. No independent tight-crop alignment.
                origin: [
                    ((glyph.pos.x + uv.offset.x) * scale).round() as i32,
                    (uv.offset.y * scale).round() as i32,
                ],
                size: [width, height],
                alpha,
            });
        }
    }
    if result.len() != text.chars().count() {
        return Err("unexpected shaping/substitution in ASCII sample".into());
    }
    Ok(result)
}

fn console_samples(
    bytes: &[u8],
    hinting: Option<bool>,
    size: f32,
    scale: f32,
    ligatures: bool,
) -> Result<Vec<Sample>> {
    let first = samples(bytes, hinting, size, scale, "M", ligatures)?;
    let width = first[0].advance;
    let mut result = if ligatures {
        samples(bytes, hinting, size, scale, GLYPHS, true)?
    } else {
        let mut cells = Vec::new();
        for chr in GLYPHS.chars() {
            cells.extend(samples(
                bytes,
                hinting,
                size,
                scale,
                &chr.to_string(),
                false,
            )?);
        }
        cells
    };
    // Mirrors the fixed column placement in painter/ligatures.rs::on_grid and ordinary cells.
    for (column, glyph) in result.iter_mut().enumerate() {
        let x = (column as f32 * width * scale).round() / scale;
        glyph.position[0] = x;
        glyph.origin[0] = ((x + glyph.offset[0]) * scale).round() as i32;
    }
    Ok(result)
}

fn target() -> Target {
    Target::Smooth {
        mode: SmoothMode::Normal,
        symmetric_rendering: true,
        preserve_linear_metrics: true,
    }
}

fn replay(bytes: &[u8], ppem: f32, chr: char) -> Result<Value> {
    let font = skrifa::FontRef::from_index(bytes, 0)?;
    let outlines = font.outline_glyphs();
    let id = font.charmap().map(chr).ok_or("missing replay glyph")?;
    let outline = outlines.get(id).ok_or("missing outline")?;
    let mut instance = HintingInstance::new(
        &outlines,
        Size::unscaled(),
        LocationRef::default(),
        Target::default(),
    )?;
    let location = font.axes().location(std::iter::empty::<(&str, f32)>());
    instance.reconfigure(&outlines, Size::new(ppem), &location, target())?;
    outline.draw(DrawSettings::hinted(&instance, false), &mut NullPen)?;
    outline.draw(
        DrawSettings::unhinted(Size::new(ppem), &location),
        &mut NullPen,
    )?;
    Ok(
        json!({"creation":"Ok", "reconfigure":"Ok", "enabled":instance.is_enabled(), "hinted_draw":"Ok", "unhinted_draw":"Ok", "has_hinting":outline.has_hinting(),"prefer_interpreter":outlines.prefer_interpreter()}),
    )
}

fn hash(path: &Path) -> Result<String> {
    let output = std::process::Command::new("sha256sum").arg(path).output()?;
    if !output.status.success() {
        return Err("sha256sum failed".into());
    }
    Ok(String::from_utf8(output.stdout)?
        .split_whitespace()
        .next()
        .ok_or("missing SHA-256")?
        .into())
}

fn measure(path: &Path) -> Result<Value> {
    eprintln!("HINT_FONT {}", path.display());
    let bytes = std::fs::read(path)?;
    let font = skrifa::FontRef::from_index(&bytes, 0)?;
    let axes: Vec<_> = font.axes().iter().map(|a| json!({"tag":a.tag().to_string(), "min":a.min_value(), "default":a.default_value(), "max":a.max_value()})).collect();
    let mut cases = Vec::new();
    for size in [14.0, 14.7] {
        for scale in [1.0, 2.0] {
            let mut comparisons = Vec::new();
            for (label, text, run) in GLYPHS
                .chars()
                .map(|c| ("cell", c.to_string(), false))
                .chain([
                    ("console-row", GLYPHS.into(), false),
                    ("ordinary-run", GLYPHS.into(), false),
                    ("ligature-registration", GLYPHS.into(), true),
                ])
            {
                let take = |hinting| {
                    if label == "console-row" || label == "ligature-registration" {
                        console_samples(&bytes, hinting, size, scale, run)
                    } else {
                        samples(&bytes, hinting, size, scale, &text, run)
                    }
                };
                let default = take(None)?;
                let on = take(Some(true))?;
                let off = take(Some(false))?;
                if default != on || on != take(Some(true))? {
                    return Err("default/on or same-mode determinism check failed".into());
                }
                for (a, b) in on.iter().zip(&off) {
                    let (pixels, delta) = difference(a, b);
                    comparisons.push(json!({"path":label,"text":text,"chr":a.chr,"on":a,"off":b,"different_pixels":pixels,"absolute_alpha_difference":delta,"replay":replay(&bytes,size*scale,a.chr)?}));
                }
            }
            cases.push(json!({"points":size,"pixels_per_point":scale,"ppem":size*scale,"comparisons":comparisons}));
        }
    }
    Ok(
        json!({"file":path.file_name().ok_or("missing file name")?.to_string_lossy(),"sha256":hash(path)?,"face_index":0,"family":font.localized_strings(skrifa::string::StringId::FAMILY_NAME).english_or_first().map(|s|s.to_string()),"version":font.localized_strings(skrifa::string::StringId::VERSION_STRING).english_or_first().map(|s|s.to_string()),"axes":axes,"cases":cases}),
    )
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "usage: hinting <UbuntuSansMono-variable.ttf> <output.json> (requires sha256sum)"
                .into(),
        );
    }
    let bundled =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/fonts/GeistMono-Regular.ttf");
    let result = json!({"epaint":"0.36.1","skrifa":"0.44.0","target":"Smooth(light=false,symmetric_rendering=true,preserve_linear_metrics=true)","location":"axis defaults","color_transfer":"TextOptions::default","bin":"cell starts at x=0; ordinary-run uses default bins; ligature-registration disables bins","fonts":[measure(&bundled)?, measure(Path::new(&args[0]))?]});
    std::fs::write(&args[1], serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Sample {
        Sample {
            chr: 'I',
            glyph_id: 1,
            position: [0.0, 0.0],
            advance: 1.0,
            offset: [0.0, 0.0],
            uv: [[0, 0], [1, 1]],
            origin: [0, -1],
            size: [1, 1],
            alpha: vec![100],
        }
    }
    #[test]
    fn comparison_preserves_alpha_size_and_bearings() {
        let a = sample();
        assert_eq!(difference(&a, &a), (0, 0));
        let mut b = a.clone();
        b.alpha[0] = 90;
        assert_eq!(difference(&a, &b), (1, 10));
        b = a.clone();
        b.origin[0] += 1;
        assert_eq!(difference(&a, &b), (2, 200));
        b = a.clone();
        b.size[0] = 2;
        b.alpha.push(50);
        assert_eq!(difference(&a, &b), (1, 50));
    }
    #[test]
    fn missing_face_or_character_never_becomes_equality() {
        assert!(samples(b"invalid", None, 14.0, 1.0, "I", false).is_err());
        let bytes = include_bytes!("../../../resources/fonts/GeistMono-Regular.ttf");
        assert!(samples(bytes, None, 14.0, 1.0, "\u{10ffff}", false).is_err());
    }
}
