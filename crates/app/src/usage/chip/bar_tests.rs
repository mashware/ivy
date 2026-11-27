//! The chip in a real application pass: where the title bar puts it, and that it is not there when
//! nothing is enabled or the bar is compact (#283).

use super::{Allowance, Reading, Source};
use crate::{Window, tests::window_of_tabs};
use eframe::egui;

/// One real application pass, returning what it painted.
pub(super) fn painted(
    window: &mut Window,
    ctx: &egui::Context,
    area: egui::Rect,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut frame = eframe::Frame::_new_kittest();
    eframe::App::logic(window, ctx, &mut frame);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(area),
            events,
            ..Default::default()
        },
        |ui| eframe::App::ui(window, ui, &mut frame),
    );
    output.textures_delta.clear();
    output.shapes
}

pub(super) fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}

pub(super) fn integrated() -> (Window, egui::Context) {
    let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
    window.chrome.enable();
    window.headers = window.headers.integrated(true);
    (window, egui::Context::default())
}

pub(super) fn area(width: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 900.0))
}

/// A Codex reading taken now, 42 % of its five hours used.
pub(super) fn codex() -> (Source, Source) {
    let now = chrono::Utc::now().timestamp();
    let reading = Reading {
        observed_at: now,
        five_hour: Some(Allowance {
            used: Some(42.0),
            resets_at: Some(now + 3600),
            expired: false,
        }),
        weekly: None,
    };
    (Source::Off, Source::Read(reading))
}

/// AC1: with nothing enabled there is no slot, no chip and no chip text, and the tabs keep the
/// band they had.
#[test]
fn with_nothing_enabled_the_title_bar_has_no_chip() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    for _ in 0..2 {
        painted(&mut window, &ctx, area, vec![]);
    }
    let shapes = painted(&mut window, &ctx, area, vec![]);
    assert_eq!(window.usage.chip.slot(), 0.0);
    let strip = crate::chrome::strip(&window, area);
    assert_eq!(strip.chip, None);
    assert!(!texts(&shapes).iter().any(|text| text.starts_with("5h")));
}

/// The chip shows in the wide bar, in the slot left of the Board, and the tabs give up exactly its
/// width; in a compact bar there is no chip at all.
#[test]
fn a_reading_paints_the_chip_in_the_wide_bar_and_never_in_the_compact_one() {
    let (mut window, ctx) = integrated();
    let wide = area(1400.0);
    let before = crate::chrome::strip(&window, wide);
    window.usage.chip.forced = Some(codex());
    for _ in 0..2 {
        painted(&mut window, &ctx, wide, vec![]);
    }
    let shapes = painted(&mut window, &ctx, wide, vec![]);
    assert!(window.usage.chip.slot() > 0.0);
    let strip = crate::chrome::strip(&window, wide);
    let chip = strip.chip.expect("a wide bar has room for the chip");
    assert!(chip.right() < crate::chrome::strip::slots(strip.row).board.left());
    assert!(strip.tabs.right() < before.tabs.right());
    let painted_at = shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == "5h 42%" => Some(text.pos),
        _ => None,
    });
    assert!(
        painted_at.is_some_and(|pos| chip.contains(pos)),
        "{:?}",
        texts(&shapes)
    );
    let compact = area(400.0);
    let shapes = painted(&mut window, &ctx, compact, vec![]);
    assert!(crate::chrome::strip(&window, compact).compact());
    assert!(!texts(&shapes).iter().any(|text| text == "5h 42%"));
}
