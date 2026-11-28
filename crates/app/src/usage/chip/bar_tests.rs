//! The chip in a real application pass: where the title bar puts it, and that it is not there when
//! nothing is enabled or the bar is compact (#283).

use super::{Allowance, Named, Reading, Source};
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
        windows: vec![Named {
            name: "5-hour session".into(),
            length: super::FIVE_HOURS,
            extra: false,
            window: Allowance {
                used: Some(42.0),
                resets_at: Some(now + 3600),
                expired: false,
            },
        }],
        last_failure: None,
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
    // The box hugs the text against the slot's right edge, leaving the rest of the slot empty.
    let hugged = window.usage.chip.hugged(chip);
    assert_eq!(hugged.right(), chip.right());
    assert!(hugged.width() < chip.width(), "{hugged:?} in {chip:?}");
    let box_drawn = shapes
        .iter()
        .any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.rect == hugged));
    assert!(box_drawn, "no box hugging the chip's text");
    let at = |wanted: &str| {
        shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == wanted => Some(text.pos),
            _ => None,
        })
    };
    let (label, value) = (at("5h"), at("42%"));
    assert!(
        label.zip(value).is_some_and(|(label, value)| {
            hugged.contains(label) && hugged.contains(value) && label.x < value.x
        }),
        "{:?}",
        texts(&shapes)
    );
    let compact = area(400.0);
    let shapes = painted(&mut window, &ctx, compact, vec![]);
    assert!(crate::chrome::strip(&window, compact).compact());
    assert!(!texts(&shapes).iter().any(|text| text == "42%"));
}

/// AC9 (D13): the chip's work is declared under a series of its own — what it reads in
/// `usage::begin_pass` and what it paints inside the title bar — and the chrome's own figure no
/// longer holds the painting, so nothing is counted twice or published as the framework's.
#[test]
fn the_chip_declares_its_work_under_its_own_series_and_not_as_the_chromes() {
    use crate::usage::report::CHIP;
    let (mut window, ctx) = integrated();
    window.usage.chip.forced = Some(codex());
    let wide = area(1400.0);
    for _ in 0..2 {
        painted(&mut window, &ctx, wide, vec![]);
    }
    window.usage.timing = Default::default();
    let metrics =
        crate::painter::CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    window.chrome.take_cost();
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(wide),
            ..Default::default()
        },
        |ui| {
            crate::usage::begin_pass(&mut window, ui);
            let read = window.usage.timing.pending[CHIP].expect("the chip's reading is declared");
            let set_aside = window.timed_separately;
            let started = std::time::Instant::now();
            crate::chrome::before_keys(&mut window, ui, &metrics);
            let wall = started.elapsed();
            let painted = window.usage.timing.pending[CHIP].unwrap() - read;
            assert!(
                painted > std::time::Duration::ZERO,
                "the chip's paint is declared"
            );
            assert_eq!(window.timed_separately - set_aside, painted);
            let chrome = window.chrome.take_cost();
            assert!(
                chrome + painted <= wall,
                "{chrome:?} + {painted:?} > {wall:?}"
            );
        },
    );
    output.textures_delta.clear();
    window.usage.close_frame();
    let mut report = Vec::new();
    window.usage.publish(&mut report).unwrap();
    let report = String::from_utf8(report).unwrap();
    assert!(report.contains("Usage chip: frames=1,"), "{report}");
}

/// A measured run reads nothing for the chip and paints no chip, so its series stays at zero —
/// the number the measurement script requires of every batch.
#[test]
fn a_measured_run_does_no_chip_work() {
    let (mut window, ctx) = integrated();
    window.usage.measuring = true;
    window.usage.chip.forced = Some(codex());
    let wide = area(1400.0);
    for _ in 0..3 {
        painted(&mut window, &ctx, wide, vec![]);
    }
    assert_eq!(
        window.usage.timing.pending[crate::usage::report::CHIP],
        None
    );
    assert_eq!(crate::chrome::strip(&window, wide).chip, None);
}

#[test]
fn the_painted_figures_and_claude_dot_use_their_declared_inks() {
    let (mut window, ctx) = integrated();
    let area = area(1600.0);
    let now = chrono::Utc::now().timestamp();
    let roles = crate::ui::style::roles(&ctx, &window.theme);
    let read = |used, observed_at| {
        Source::Read(Reading {
            observed_at,
            windows: [super::FIVE_HOURS, super::WEEK]
                .into_iter()
                .map(|length| Named {
                    name: "window".into(),
                    length,
                    extra: false,
                    window: Allowance {
                        used: Some(used),
                        resets_at: Some(now + 7200),
                        expired: false,
                    },
                })
                .collect(),
            last_failure: None,
        })
    };
    for (source, wanted, color) in [
        (read(85.0, now), "85%", roles.wait),
        (read(100.0, now), "100%", roles.block),
        (read(10.0, now - super::STALE_AFTER - 1), "—", roles.tx3),
        (Source::Waiting("reading".into()), "—", roles.tx3),
        (Source::Failed("failed".into()), "!", roles.block),
    ] {
        window.usage.chip.forced = Some((source.clone(), source));
        for _ in 0..2 {
            painted(&mut window, &ctx, area, vec![]);
        }
        let shapes = painted(&mut window, &ctx, area, vec![]);
        let slot = crate::chrome::strip(&window, area).chip.unwrap();
        let chip = window.usage.chip.hugged(slot);
        let figures: Vec<_> = shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if chip.contains(text.pos) && text.galley.text() == wanted =>
                {
                    Some(
                        text.override_text_color
                            .unwrap_or(text.galley.job.sections[0].format.color)
                            .gamma_multiply(text.opacity_factor),
                    )
                }
                _ => None,
            })
            .collect();
        assert_eq!(figures, vec![color; 3], "painted {wanted}");
        assert!(
            shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Circle(circle)
                    if chip.contains(circle.center)
                        && circle.fill == egui::Color32::from_rgb(0xd9, 0x77, 0x57)
            )),
            "Claude's dot was not painted in clay"
        );
    }
}

fn repaint_after(ctx: &egui::Context, mut draw: impl FnMut(&egui::Context)) -> std::time::Duration {
    let mut delay = std::time::Duration::ZERO;
    for _ in 0..4 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| draw(ui.ctx()));
        output.textures_delta.clear();
        delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    }
    delay
}

#[test]
fn the_chip_schedules_the_frame_that_marks_a_reading_stale() {
    let ctx = egui::Context::default();
    let mut chip = super::surface::Chip::default();
    let now = 1_800_000_000;
    let source = Source::Read(Reading {
        observed_at: now - super::STALE_AFTER + 120,
        windows: Vec::new(),
        last_failure: None,
    });
    let delay = repaint_after(&ctx, |ctx| {
        chip.update(ctx, Source::Off, source.clone(), now)
    });
    assert!(delay > std::time::Duration::from_secs(120), "{delay:?}");
    assert!(delay <= std::time::Duration::from_secs(121), "{delay:?}");
}

#[test]
fn an_open_summary_schedules_its_next_minute_without_a_new_reading() {
    let (mut window, ctx) = integrated();
    window.usage.chip.forced = Some(codex());
    let closed = repaint_after(&ctx, |ctx| super::begin_pass(&mut window.usage, ctx));
    assert!(closed > std::time::Duration::from_secs(60), "{closed:?}");
    window.usage.chip.popover = true;
    let open = repaint_after(&ctx, |ctx| super::begin_pass(&mut window.usage, ctx));
    assert!(open <= std::time::Duration::from_secs(60), "{open:?}");
}
