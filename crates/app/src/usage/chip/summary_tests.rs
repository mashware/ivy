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
    let button = egui::pos2(summary.left() + 40.0, summary.bottom() - 40.0);
    painted(&mut window, &ctx, area, press(button));
    painted(&mut window, &ctx, area, release(button));
    assert!(window.usage.blocks(&ctx), "the AI usage view did not open");
    assert!(crate::usage::chip_summary(&window.usage).is_none());
}

/// Both agents with readings: Claude 38 % of its five hours, Codex 85 % of a weekly-only plan.
fn both() -> (Source, Source) {
    use super::{Allowance, FIVE_HOURS, Named, Reading, WEEK};
    let now = chrono::Utc::now().timestamp();
    let named = |name: &str, length, used| Named {
        name: name.into(),
        length,
        extra: false,
        window: Allowance {
            used: Some(used),
            resets_at: Some(now + 3600),
            expired: false,
        },
    };
    let claude = Reading {
        observed_at: now,
        windows: vec![
            named("5-hour session", FIVE_HOURS, 38.0),
            named("Weekly", WEEK, 6.0),
        ],
        last_failure: None,
    };
    let codex = Reading {
        observed_at: now,
        windows: vec![named("Weekly", WEEK, 85.0)],
        last_failure: None,
    };
    (Source::Read(claude), Source::Read(codex))
}

/// The tab row's bottom, where the header starts when there are tabs.
fn below_tabs(summary: egui::Rect) -> f32 {
    summary.top() + 10.0 + 44.0
}

/// The header's agent name: the first agent name painted below .
fn header(shapes: &[egui::epaint::ClippedShape], top: f32) -> Option<String> {
    shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text)
            if text.pos.y > top && ["Claude Code", "Codex"].contains(&text.galley.text()) =>
        {
            Some(text.galley.text().to_owned())
        }
        _ => None,
    })
}

fn ready(window: &mut Window, ctx: &egui::Context, area: egui::Rect, sources: (Source, Source)) {
    window.usage.chip.forced = Some(sources);
    for _ in 0..2 {
        painted(window, ctx, area, vec![]);
    }
}

/// The summary as the design lays it out: both agents' tabs, a header with the agent and when it
/// was read, each window with its figure and reset, and «Open AI usage» as a menu row — painted,
/// in that order from the top.
#[test]
fn the_summary_paints_tabs_header_windows_and_footer_in_order() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    ready(&mut window, &ctx, area, both());
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
        "38% used",
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
}

/// With both agents on the chip the summary opens on the first, Claude; a tab picked is kept
/// while it stays open and forgotten once it closes.
#[test]
fn with_both_agents_it_opens_on_claude_and_forgets_a_picked_tab_once_closed() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    ready(&mut window, &ctx, area, both());
    let summary = open_summary(&mut window, &ctx, area);
    let top = below_tabs(summary);
    let shown = |window: &mut Window| header(&painted(window, &ctx, area, vec![]), top);
    assert_eq!(shown(&mut window).as_deref(), Some("Claude Code"));
    let codex_tab = egui::pos2(summary.right() - 60.0, summary.top() + 30.0);
    click(&mut window, &ctx, area, codex_tab);
    assert_eq!(shown(&mut window).as_deref(), Some("Codex"));
    let chip = crate::chrome::strip(&window, area).chip.unwrap();
    let chip = window.usage.chip.hugged(chip).center();
    click(&mut window, &ctx, area, chip);
    assert!(crate::usage::chip_summary(&window.usage).is_none());
    open_summary(&mut window, &ctx, area);
    assert_eq!(shown(&mut window).as_deref(), Some("Claude Code"));
}

/// Each tab's thin bar is the share of the window the chip shows for that agent, in the chip's
/// tone: Claude's 5 hours at 38 % in its own colour, Codex's week at 85 % in amber.
#[test]
fn each_tabs_bar_is_the_chips_window_in_the_chips_tone() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    ready(&mut window, &ctx, area, both());
    let summary = open_summary(&mut window, &ctx, area);
    let shapes = painted(&mut window, &ctx, area, vec![]);
    let roles = crate::ui::style::roles(&ctx, &window.theme);
    let claude = super::surface::dot(super::Agent::Claude, &window.theme);
    let bars: Vec<_> = shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if (rect.rect.height() - 3.0).abs() < 0.01
                    && rect.rect.top() < below_tabs(summary)
                    && summary.contains_rect(rect.rect) =>
            {
                Some((rect.rect, rect.fill))
            }
            _ => None,
        })
        .collect();
    let fill = |color: egui::Color32| {
        bars.iter()
            .find(|(_, fill)| *fill == color)
            .unwrap_or_else(|| panic!("no bar in {color:?}: {bars:?}"))
            .0
    };
    let track = |bar: egui::Rect| {
        bars.iter()
            .find(|(rect, fill)| *fill == roles.line && rect.left() == bar.left())
            .unwrap()
            .0
    };
    let (claude_bar, codex_bar) = (fill(claude), fill(roles.wait));
    assert!(claude_bar.center().x < codex_bar.center().x);
    let share = |bar: egui::Rect| bar.width() / track(bar).width();
    assert!(
        (share(claude_bar) - 0.38).abs() < 0.01,
        "{}",
        share(claude_bar)
    );
    assert!(
        (share(codex_bar) - 0.85).abs() < 0.01,
        "{}",
        share(codex_bar)
    );
}

/// Without a reporting launch Claude is not on the chip, so it gets no tab: the summary is
/// Codex's alone, and a discreet line says where Claude's figures come from.
#[test]
fn without_a_reporting_launch_there_is_no_claude_tab() {
    let (mut window, ctx) = integrated();
    let area = area(1400.0);
    ready(&mut window, &ctx, area, codex());
    let summary = open_summary(&mut window, &ctx, area);
    let shapes = painted(&mut window, &ctx, area, vec![]);
    let texts = super::bar_tests::texts(&shapes);
    assert!(!texts.iter().any(|text| text == "Claude Code"), "{texts:?}");
    assert_eq!(header(&shapes, summary.top()).as_deref(), Some("Codex"));
    assert!(
        texts
            .iter()
            .any(|text| text == "Claude: start it with reporting from AI usage"),
        "{texts:?}"
    );

    // No reading yet: the header stays and the windows section says why.
    let waiting = Source::Waiting("Codex reads on · reading now…".into());
    window.usage.chip.forced = Some((Source::Off, waiting));
    painted(&mut window, &ctx, area, vec![]);
    let texts = super::bar_tests::texts(&painted(&mut window, &ctx, area, vec![]));
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

/// The owner's window through its own sources rather than forced readings: a weekly-only Codex
/// plan, a failed read, no Claude reporting. The summary shows Codex alone every time it opens,
/// whichever way it was closed. Linux x86_64 only: elsewhere Codex reads are unsupported and
/// there is no chip to click.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_owners_window_shows_codex_alone_on_every_open() {
    for failed in [false, true] {
        for way in ["chip", "console", "board", "open AI usage"] {
            let (mut window, ctx) = integrated();
            let area = area(1400.0);
            if failed {
                window.usage.allowances.fail_for_test();
            } else {
                window.usage.allowances.weekly_for_test(85.0);
            }
            for _ in 0..2 {
                painted(&mut window, &ctx, area, vec![]);
            }
            for round in 0..2 {
                let summary = open_summary(&mut window, &ctx, area);
                let shapes = painted(&mut window, &ctx, area, vec![]);
                let texts = super::bar_tests::texts(&shapes);
                assert_eq!(
                    header(&shapes, summary.top()).as_deref(),
                    Some("Codex"),
                    "{way} {round}"
                );
                assert!(!texts.iter().any(|text| text == "Claude Code"), "{texts:?}");
                let wanted = if failed {
                    "Could not read Codex allowances."
                } else {
                    "85% used"
                };
                assert!(texts.iter().any(|text| text == wanted), "{texts:?}");
                match way {
                    "chip" => {
                        let chip = crate::chrome::strip(&window, area).chip.unwrap();
                        let chip = window.usage.chip.hugged(chip).center();
                        click(&mut window, &ctx, area, chip);
                    }
                    "console" => click(&mut window, &ctx, area, egui::pos2(100.0, 500.0)),
                    "board" => {
                        let row = crate::chrome::strip(&window, area).row;
                        let board = crate::chrome::strip::slots(row).board.center();
                        click(&mut window, &ctx, area, board);
                        click(&mut window, &ctx, area, board);
                    }
                    _ => {
                        let open = egui::pos2(summary.left() + 40.0, summary.bottom() - 40.0);
                        click(&mut window, &ctx, area, open);
                        let escape = egui::Event::Key {
                            key: egui::Key::Escape,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        };
                        painted(&mut window, &ctx, area, vec![escape]);
                        painted(&mut window, &ctx, area, vec![]);
                    }
                }
                assert!(crate::usage::chip_summary(&window.usage).is_none(), "{way}");
                assert!(!window.usage.blocks(&ctx), "{way}");
            }
        }
    }
}
