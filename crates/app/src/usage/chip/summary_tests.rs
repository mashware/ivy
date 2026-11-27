//! The summary under the chip in real application passes: it opens and closes by the pointer
//! alone, it opens AI usage, and it is not modal — it neither dims the consoles nor takes their
//! input outside its own rectangle (#283, AC5 and AC8).

use super::Source;
use super::bar_tests::{area, codex, integrated, painted};
use crate::{Window, painter::CellMetrics};
use eframe::egui;

fn press(at: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

fn release(at: egui::Pos2) -> Vec<egui::Event> {
    vec![egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]
}

fn click(window: &mut Window, ctx: &egui::Context, area: egui::Rect, at: egui::Pos2) {
    painted(window, ctx, area, press(at));
    painted(window, ctx, area, release(at));
    painted(window, ctx, area, vec![]);
}

/// The consoles' fills and text, each at the opacity it was painted with — what a dimmed pass
/// changes; a text colour alone does not, because dimming lowers the opacity, not the colour. What
/// falls under the summary is left out: it is the summary's own paint, not the consoles'.
fn inks(
    shapes: &[egui::epaint::ClippedShape],
    consoles: egui::Rect,
    summary: egui::Rect,
) -> Vec<egui::Color32> {
    let mut inks: Vec<_> = shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(text)
                if consoles.contains(text.pos) && !summary.expand(2.0).contains(text.pos) =>
            {
                Some(
                    text.override_text_color
                        .unwrap_or(text.galley.job.sections[0].format.color)
                        .gamma_multiply(text.opacity_factor),
                )
            }
            egui::Shape::Rect(rect)
                if consoles.contains_rect(rect.rect) && !summary.intersects(rect.rect) =>
            {
                Some(rect.fill)
            }
            _ => None,
        })
        .collect();
    inks.sort_by_key(|c| c.to_array());
    inks.dedup();
    inks
}

fn open_summary(window: &mut Window, ctx: &egui::Context, area: egui::Rect) -> egui::Rect {
    let chip = crate::chrome::strip(window, area)
        .chip
        .expect("the chip is shown");
    click(window, ctx, area, window.usage.chip.hugged(chip).center());
    crate::usage::chip_summary(&window.usage).expect("a click on the chip opens the summary")
}

#[test]
fn the_summary_neither_dims_nor_takes_the_consoles_input_and_escape_is_not_its_key() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    window.usage.chip.forced = Some(codex());
    for _ in 0..2 {
        painted(&mut window, &ctx, area, vec![]);
    }
    let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let consoles = crate::tab_bar::geometry(&window, area, &metrics).consoles;
    let plain = painted(&mut window, &ctx, area, vec![]);
    let summary = open_summary(&mut window, &ctx, area);
    assert!(
        consoles.intersects(summary),
        "the summary floats over a console"
    );
    assert!(!crate::chrome::is_modal(&window, &ctx));
    assert!(!window.usage.blocks(&ctx));
    let open = painted(&mut window, &ctx, area, vec![]);
    let plain = inks(&plain, consoles, summary);
    assert!(!plain.is_empty(), "no console paint was found");
    assert_eq!(inks(&open, consoles, summary), plain, "the summary dimmed");

    // Escape belongs to the console: the summary does not close on it.
    let escape = egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    painted(&mut window, &ctx, area, vec![escape]);
    assert!(crate::usage::chip_summary(&window.usage).is_some());

    // A press on the summary stays there; the same press with it closed reaches the console.
    let left = egui::pos2(consoles.left() + 20.0, consoles.center().y);
    let inside = summary.center();
    click(&mut window, &ctx, area, left);
    let focused = window.active().focus;
    let summary = open_summary(&mut window, &ctx, area);
    assert!(summary.contains(inside));
    click(&mut window, &ctx, area, inside);
    assert!(crate::usage::chip_summary(&window.usage).is_some());
    assert_eq!(window.active().focus, focused, "the press went through");
    let chip = crate::chrome::strip(&window, area).chip.unwrap();
    let chip = window.usage.chip.hugged(chip).center();
    click(&mut window, &ctx, area, chip);
    assert!(
        crate::usage::chip_summary(&window.usage).is_none(),
        "a second click closes it"
    );
    click(&mut window, &ctx, area, inside);
    assert_ne!(
        window.active().focus,
        focused,
        "control: with the summary closed the same press reaches the console"
    );
}

/// A press outside closes the summary and still reaches what it landed on; «Open AI usage» opens
/// the modal view.
#[test]
fn a_press_outside_closes_the_summary_and_open_ai_usage_opens_the_view() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    window.usage.chip.forced = Some(codex());
    for _ in 0..2 {
        painted(&mut window, &ctx, area, vec![]);
    }
    let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(window.font_size));
    let consoles = crate::tab_bar::geometry(&window, area, &metrics).consoles;
    let left = egui::pos2(consoles.left() + 20.0, consoles.bottom() - 20.0);
    let right = egui::pos2(consoles.right() - 20.0, consoles.bottom() - 20.0);
    click(&mut window, &ctx, area, left);
    let focused = window.active().focus;
    open_summary(&mut window, &ctx, area);
    click(&mut window, &ctx, area, right);
    assert!(crate::usage::chip_summary(&window.usage).is_none());
    assert_ne!(
        window.active().focus,
        focused,
        "the press that closed the summary did not reach the console"
    );

    let summary = open_summary(&mut window, &ctx, area);
    assert!(!window.usage.blocks(&ctx));
    // «Open AI usage» is the summary's bottom-left button.
    let button = egui::pos2(summary.left() + 40.0, summary.bottom() - 26.0);
    painted(&mut window, &ctx, area, press(button));
    painted(&mut window, &ctx, area, release(button));
    assert!(window.usage.blocks(&ctx), "the AI usage view did not open");
    assert!(crate::usage::chip_summary(&window.usage).is_none());
}

/// The summary as the design lays it out: both agents' tabs, a header with the agent and when it
/// was read, each window with its figure and reset, and «Open AI usage» as a menu row — painted,
/// in that order from the top. Before a reading it keeps the header and says why there is none.
#[test]
fn the_summary_paints_tabs_header_windows_and_footer_in_order() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    window.usage.chip.forced = Some(codex());
    for _ in 0..2 {
        painted(&mut window, &ctx, area, vec![]);
    }
    let summary = open_summary(&mut window, &ctx, area);
    let shapes = painted(&mut window, &ctx, area, vec![]);
    let at = |wanted: &str| {
        shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == wanted => Some(text.pos),
            _ => None,
        })
    };
    let order = [
        "Claude Code",
        "Updated just now",
        "5-hour session",
        "42% used",
        "Open AI usage",
    ];
    let positions: Vec<_> = order
        .iter()
        .map(|text| at(text).unwrap_or_else(|| panic!("{text} was not painted")))
        .collect();
    assert!(positions.iter().all(|pos| summary.contains(*pos)));
    assert!(
        positions.windows(2).all(|pair| pair[0].y <= pair[1].y),
        "{positions:?}"
    );
    assert!(at("Codex").is_some_and(|tab| (tab.y - positions[0].y).abs() < 1.0));
    let reset = shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().starts_with("Resets at "))
    });
    assert!(reset, "no reset line");

    // No reading yet: the header stays and the windows section says why.
    let waiting = Source::Waiting("Codex reads on · reading now…".into());
    window.usage.chip.forced = Some((Source::Off, waiting));
    painted(&mut window, &ctx, area, vec![]);
    let shapes = painted(&mut window, &ctx, area, vec![]);
    let texts = super::bar_tests::texts(&shapes);
    for wanted in [
        "Codex",
        "Waiting for the first reading.",
        "Codex reads on · reading now…",
        "Open AI usage",
    ] {
        assert!(
            texts.iter().any(|text| text == wanted),
            "{wanted}: {texts:?}"
        );
    }
}

/// The summary opens on the chip's first agent — Codex when the chip shows only Codex — keeps a
/// tab picked while it stays open, and forgets it once closed. Claude's tab, with no reporting
/// launch, says where its figures come from rather than offering a toggle it does not have.
#[test]
fn the_summary_opens_on_the_chips_first_agent_and_forgets_a_picked_tab_once_closed() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    window.usage.chip.forced = Some(codex());
    for _ in 0..2 {
        painted(&mut window, &ctx, area, vec![]);
    }
    // The header's name is the text below the tab row that names an agent.
    let header = |window: &mut Window, summary: egui::Rect| {
        let shapes = painted(window, &ctx, area, vec![]);
        let below_tabs = summary.top() + 10.0 + 44.0;
        shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.pos.y > below_tabs
                    && ["Claude Code", "Codex"].contains(&text.galley.text()) =>
            {
                Some(text.galley.text().to_owned())
            }
            _ => None,
        })
    };
    let summary = open_summary(&mut window, &ctx, area);
    assert_eq!(header(&mut window, summary).as_deref(), Some("Codex"));

    let claude_tab = egui::pos2(summary.left() + 60.0, summary.top() + 30.0);
    click(&mut window, &ctx, area, claude_tab);
    assert_eq!(header(&mut window, summary).as_deref(), Some("Claude Code"));
    let texts = super::bar_tests::texts(&painted(&mut window, &ctx, area, vec![]));
    assert!(
        texts
            .iter()
            .any(|text| text == "Claude's allowance comes from a reporting launch."),
        "{texts:?}"
    );
    assert!(!texts.iter().any(|text| text == "Allowance reads are off."));

    let chip = crate::chrome::strip(&window, area).chip.unwrap();
    let chip = window.usage.chip.hugged(chip).center();
    click(&mut window, &ctx, area, chip);
    assert!(crate::usage::chip_summary(&window.usage).is_none());
    let summary = open_summary(&mut window, &ctx, area);
    assert_eq!(header(&mut window, summary).as_deref(), Some("Codex"));
}
