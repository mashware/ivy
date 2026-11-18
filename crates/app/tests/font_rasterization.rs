//! Pixel witnesses for the terminal's two hinting paths, independent of system fonts or a GPU.
use eframe::egui::{self, epaint::text::TextOptions};
use std::sync::Arc;

const GLYPHS: &str = "Iil1mw0O+-={}";
const SIZE: f32 = 11.0 * (4.0 / 3.0);
const UBUNTU: &[u8] = include_bytes!("fixtures/hinting/UbuntuSansMono.ttf");
const GEIST: &[u8] = include_bytes!("../../../resources/fonts/GeistMono-Regular.ttf");

#[derive(Debug, PartialEq)]
struct Mask {
    size: [u16; 2],
    offset: [f32; 2],
    alpha: Vec<u8>,
}

fn masks(bytes: &'static [u8], hinting: bool, scale: f32) -> Vec<Mask> {
    let mut data = egui::FontData::from_static(bytes);
    data.tweak.hinting = Some(hinting);
    let mut definitions = egui::FontDefinitions::empty();
    definitions
        .font_data
        .insert("fixture".into(), Arc::new(data));
    definitions
        .families
        .insert(egui::FontFamily::Monospace, vec!["fixture".into()]);
    let options = TextOptions {
        color_transfer_function: egui::epaint::FontColorTransferFunction::Off,
        ..Default::default()
    };
    let mut fonts = egui::epaint::Fonts::new(options, definitions);
    // The ordinary console asks for each character at a cell origin, rather than shaping a run.
    GLYPHS
        .chars()
        .map(|chr| {
            let galley = fonts.with_pixels_per_point(scale).layout_no_wrap(
                chr.to_string(),
                egui::FontId::monospace(SIZE),
                egui::Color32::WHITE,
            );
            let glyph = &galley.rows[0].glyphs[0];
            assert_eq!(glyph.chr, chr);
            let uv = glyph.uv_rect;
            let image = fonts.image();
            let mut alpha = Vec::new();
            for y in uv.min[1]..uv.max[1] {
                for x in uv.min[0]..uv.max[0] {
                    alpha.push(image[(usize::from(x), usize::from(y))].a());
                }
            }
            assert!(alpha.iter().any(|&a| a != 0), "missing bitmap for {chr:?}");
            Mask {
                size: [uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]],
                offset: [uv.offset.x, uv.offset.y],
                alpha,
            }
        })
        .collect()
}

/// Stable FNV-1a over the mask's dimensions, placement and every coverage byte.
fn fingerprint(masks: &[Mask]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for mask in masks {
        for byte in mask
            .size
            .iter()
            .flat_map(|n| n.to_le_bytes())
            .chain(mask.offset.iter().flat_map(|n| n.to_bits().to_le_bytes()))
            .chain(mask.alpha.iter().copied())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[test]
fn instructionless_ubuntu_gets_real_hinted_masks_at_both_desktop_scales() {
    let changed = [1.0, 2.0].map(|scale| {
        let hinted = masks(UBUNTU, true, scale);
        let unhinted = masks(UBUNTU, false, scale);
        let count = hinted.iter().zip(&unhinted).filter(|(a, b)| a != b).count();
        eprintln!(
            "Ubuntu scale={scale}: changed={count}/{} hinted={:016x} unhinted={:016x}",
            hinted.len(),
            fingerprint(&hinted),
            fingerprint(&unhinted)
        );
        count
    });
    assert!(
        changed.iter().all(|&count| count > 0),
        "hinting must change actual coverage or glyph placement at each scale: {changed:?}"
    );
}

#[test]
fn native_hinted_geist_preserves_its_original_coverage_and_placement() {
    let actual = [1.0, 2.0].map(|scale| {
        let result = masks(GEIST, true, scale);
        let hash = fingerprint(&result);
        eprintln!("Geist scale={scale}: hash={hash:016x}");
        hash
    });
    // Captured from unmodified epaint 0.36.1 with explicit raw coverage transfer.
    assert_eq!(actual, [0x94eb485e3e91af08, 0x653d2574509c45e3]);
}
