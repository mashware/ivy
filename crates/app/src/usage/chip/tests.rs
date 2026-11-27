//! The chip's text and the summary's sections, decided from plain readings on every platform.

use super::summary::{Body, Row, describe};
use super::*;
use chrono::FixedOffset;

const NOW: i64 = 1_800_000_000;

fn zone() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap()
}

fn window(used: f64, resets_at: i64) -> Option<Allowance> {
    Some(Allowance {
        used: Some(used),
        resets_at: Some(resets_at),
        expired: false,
    })
}

/// A time later the same day: NOW is 08:00 UTC.
fn at(hours: i64, minutes: i64) -> i64 {
    let midnight = NOW - NOW % 86_400;
    midnight + hours * 3600 + minutes * 60
}

fn named(name: &str, length: i64, window: Option<Allowance>) -> Option<Named> {
    window.map(|window| Named {
        name: name.into(),
        length,
        extra: false,
        window,
    })
}

fn reading(observed_at: i64, five: Option<Allowance>, week: Option<Allowance>) -> Source {
    Source::Read(Reading {
        observed_at,
        windows: [
            named("5-hour session", FIVE_HOURS, five),
            named("Weekly", WEEK, week),
        ]
        .into_iter()
        .flatten()
        .collect(),
        last_failure: None,
    })
}

fn read(five: Option<Allowance>, week: Option<Allowance>) -> Source {
    reading(NOW - 60, five, week)
}

/// The chip's text as one line, items joined the way the bar paints them.
fn line(items: &[Item]) -> String {
    items
        .iter()
        .map(|item| {
            item.parts()
                .iter()
                .map(|(text, _)| *text)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// AC2: the owner's example. The label is the dim part and the figure the plain one; Claude's
/// reset follows its 5-hour figure after a dim «·».
#[test]
fn a_claude_reading_reads_five_hours_with_its_reset_then_the_week() {
    let claude = read(window(38.0, at(18, 26)), window(6.0, NOW + 3 * 86_400));
    let items = items(&claude, &Source::Off, NOW, &zone());
    assert_eq!(line(&items), "5h 38% · 18:26 | W 6%");
    assert!(items.iter().all(|item| item.agent == Agent::Claude));
    assert!(items.iter().all(|item| item.tone == Tone::Normal));
    assert_eq!(
        items[0].parts(),
        vec![
            ("5h", Part::Label),
            (" ", Part::Dim),
            ("38%", Part::Value),
            (" · ", Part::Dim),
            ("18:26", Part::Value),
        ]
    );
}

/// AC3: amber from 80 % used, red at 100 % with when the window comes back; 79.9 is still
/// normal and shown as 79, so the figure never claims a threshold the colour has not.
#[test]
fn codex_turns_amber_at_eighty_and_red_with_its_return_when_exhausted() {
    let tone_and_text = |used: f64| {
        let codex = read(window(used, at(18, 26)), None);
        let items = items(&Source::Off, &codex, NOW, &zone());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].agent, Agent::Codex);
        (items[0].tone, line(&items))
    };
    assert_eq!(tone_and_text(79.9), (Tone::Normal, "5h 79% · 18:26".into()));
    assert_eq!(tone_and_text(80.0), (Tone::Amber, "5h 80% · 18:26".into()));
    assert_eq!(tone_and_text(99.9), (Tone::Amber, "5h 99% · 18:26".into()));
    assert_eq!(
        tone_and_text(100.0),
        (Tone::Red, "5h 100% · back 18:26".into())
    );
}

/// AC4: no reading, a failed one, an old one, a window reset since, or a window with no figure
/// keep their label and grey a dash — never a percentage, and above all never «0%».
#[test]
fn a_missing_or_old_reading_keeps_its_label_with_a_grey_dash_and_never_zero() {
    let old = reading(
        NOW - STALE_AFTER - 1,
        window(0.0, at(18, 0)),
        window(0.0, NOW + 86_400 * 2),
    );
    let expired = read(
        Some(Allowance {
            used: Some(0.0),
            resets_at: Some(NOW - 1),
            expired: true,
        }),
        Some(Allowance {
            used: None,
            resets_at: None,
            expired: false,
        }),
    );
    for (source, tone) in [
        (Source::Waiting("hint".into()), Tone::Unavailable),
        (old, Tone::Stale),
        (expired, Tone::Unavailable),
    ] {
        let items = items(&source, &Source::Off, NOW, &zone());
        assert_eq!(line(&items), "5h — | W —", "{source:?}");
        assert!(items.iter().all(|item| item.tone == tone), "{items:?}");
    }
    let edge = reading(
        NOW - STALE_AFTER,
        window(0.0, at(18, 0)),
        window(0.0, NOW + 86_400 * 2),
    );
    assert_eq!(
        line(&items(&edge, &Source::Off, NOW, &zone())),
        "5h 0% · 18:00 | W 0%"
    );
}

/// AC1 at the model: with nothing enabled there is nothing to paint and no slot to reserve.
#[test]
fn nothing_enabled_gives_no_items_and_no_width() {
    for (claude, codex) in [
        (Source::Off, Source::Off),
        (Source::Unsupported, Source::Unsupported),
        (Source::Unsupported, Source::Off),
    ] {
        assert!(items(&claude, &codex, NOW, &zone()).is_empty());
        assert!(templates(&claude, &codex).is_empty());
        assert_eq!(width(&templates(&claude, &codex), &mut |_| 10.0), 0.0);
    }
    let waiting = Source::Waiting(String::new());
    assert_eq!(templates(&waiting, &waiting).len(), 3);
}

/// The slot is sized by the widest thing each item can say, so a live chip never outgrows it
/// with any figure, tone or reset a reading can carry.
#[test]
fn every_live_chip_fits_the_slot_its_templates_reserve() {
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(Default::default(), |ui| {
        let mut measure = |text: &str| {
            ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(
                        text.to_owned(),
                        egui::FontId::monospace(12.0),
                        egui::Color32::WHITE,
                    )
                    .size()
                    .x
            })
        };
        let waiting = Source::Waiting(String::new());
        let slot = width(&templates(&waiting, &waiting), &mut measure);
        // A 5-hour window resets within five hours; a week's within seven days.
        let five_hours = [NOW + 60, NOW + FIVE_HOURS / 2, NOW + FIVE_HOURS];
        let week = [NOW + 60, at(23, 58), NOW + 3 * 86_400, NOW + 6 * 86_400];
        for used in [0.0, 8.0, 38.0, 88.0, 99.9, 100.0, 140.0] {
            for short in five_hours {
                for long in week {
                    let source = read(window(used, short), window(used, long));
                    let live = items(&source, &source, NOW, &zone());
                    let live = width(&live, &mut measure);
                    assert!(live <= slot, "{used} {short} {long}: {live} > {slot}");
                }
            }
        }
    });
    output.textures_delta.clear();
}

fn message(body: &Body) -> (&str, Option<&str>) {
    match body {
        Body::Message { text, hint, .. } => (text, hint.as_deref()),
        Body::Windows(rows) => panic!("windows where a message was expected: {rows:?}"),
    }
}

/// AC7, and every state without a reading: the summary keeps its structure and says which state
/// it is in — a failed read says it failed, never «waiting».
#[test]
fn the_summary_names_each_state_without_a_reading() {
    for agent in [Agent::Claude, Agent::Codex] {
        let panel = describe(agent, &Source::Unsupported, NOW, &zone());
        assert_eq!(
            message(&panel.body).0,
            "Allowances are not available on this platform."
        );
    }
    let off = describe(Agent::Codex, &Source::Off, NOW, &zone());
    assert_eq!(message(&off.body).0, "Allowance reads are off.");
    // Claude has no reads to turn on; its figures come from a launch made with reporting.
    let off = describe(Agent::Claude, &Source::Off, NOW, &zone());
    assert_eq!(
        message(&off.body),
        (
            "Claude's allowance comes from a reporting launch.",
            Some("Start Claude Code with reporting from AI usage.")
        )
    );
    let waiting = Source::Waiting("Codex reads on · reading now…".into());
    let waiting = describe(Agent::Codex, &waiting, NOW, &zone());
    assert_eq!(
        message(&waiting.body),
        (
            "Waiting for the first reading.",
            Some("Codex reads on · reading now…")
        )
    );
    let failed = Source::Failed("Codex returned no verified reading.".into());
    let failed = describe(Agent::Codex, &failed, NOW, &zone());
    assert_eq!(
        message(&failed.body),
        (
            "Could not read Codex allowances.",
            Some("Codex returned no verified reading.")
        )
    );
    assert!(matches!(failed.body, Body::Message { tone: Tone::Failed, .. }));
    // A launch that ended is not a launch still to come.
    let ended = Source::Ended("Start Claude Code with reporting from AI usage.".into());
    let ended = describe(Agent::Claude, &ended, NOW, &zone());
    assert_eq!(
        message(&ended.body).0,
        "The launch that reported it has ended."
    );
}

/// On the bar a failed read is not a grey dash: it is a red «!», told apart from waiting, a
/// launch that ended, an old reading and a window with no figure, which all grey a dash.
#[test]
fn a_failed_read_is_a_red_mark_and_the_rest_grey_a_dash() {
    let tones = |source: Source| {
        let items = items(&source, &Source::Off, NOW, &zone());
        (items[0].value.clone(), items[0].tone)
    };
    assert_eq!(tones(Source::Failed("why".into())), ("!".into(), Tone::Failed));
    assert_eq!(tones(Source::Waiting("hint".into())), ("—".into(), Tone::Unavailable));
    assert_eq!(tones(Source::Ended("hint".into())), ("—".into(), Tone::Unavailable));
    let old = reading(NOW - STALE_AFTER - 1, window(10.0, at(18, 0)), None);
    assert_eq!(tones(old), ("—".into(), Tone::Stale));
}

/// The chip's own clock: an idle window is woken when a reading turns stale — one second past
/// `STALE_AFTER` — or when one of its windows resets first, whichever is sooner.
#[test]
fn the_next_change_is_the_stale_flip_or_an_earlier_reset() {
    let fresh = reading(NOW, window(10.0, NOW + 3 * 3600), None);
    assert_eq!(next_change(&[&fresh], NOW), Some(NOW + STALE_AFTER + 1));
    let soon = reading(NOW, window(10.0, NOW + 600), None);
    assert_eq!(next_change(&[&soon, &fresh], NOW), Some(NOW + 600));
    assert_eq!(next_change(&[&Source::Waiting(String::new())], NOW), None);
}

/// Every window length labels in at most four characters, which is what the Codex template
/// reserves.
#[test]
fn every_window_label_fits_four_characters() {
    for minutes in 1..=(366 * 1440) {
        let label = length_label(minutes * 60);
        assert!(label.chars().count() <= 4, "{minutes} min: {label}");
    }
}

/// Each window with its figure, its reset with how long until it, and its pace with whether it
/// lasts to the reset.
#[test]
fn the_summary_shows_each_window_with_use_reset_and_pace() {
    // Half of the five hours gone and 80 % used: ahead, and out before the reset.
    let claude = read(
        window(80.0, NOW + FIVE_HOURS / 2),
        window(10.0, NOW + WEEK - WEEK / 10),
    );
    let panel = describe(Agent::Claude, &claude, NOW, &zone());
    assert_eq!(panel.updated.as_deref(), Some("Updated 1m ago"));
    assert_eq!((panel.stale, panel.failure), (None, None));
    let Body::Windows(rows) = panel.body else {
        panic!("no windows");
    };
    assert_eq!(
        rows[0],
        Row {
            name: "5-hour session".into(),
            used: Some(80.0),
            used_text: "80% used".into(),
            reset: Some("Resets at 10:30 (in 2h 30m)".into()),
            pace: Some("Pace: ahead (+30%) · may run out in ~37m".into()),
            tone: Tone::Amber,
        }
    );
    assert_eq!(
        rows[1].reset.as_deref(),
        Some("Resets Thu 15:12 (in 6d 7h)")
    );
    assert_eq!(
        rows[1].pace.as_deref(),
        Some("Pace: on track · lasts to reset")
    );
    assert_eq!(rows[1].tone, Tone::Normal);
}

/// An old reading still shows its windows, greyed, and says how old it is; when its refresh failed
/// too, it says both instead of the age hiding the failure.
#[test]
fn a_stale_reading_is_greyed_and_says_so() {
    let mut old = reading(NOW - 31 * 60, window(40.0, at(10, 0)), None);
    if let Source::Read(reading) = &mut old {
        reading.last_failure = Some("Last read failed: Codex closed.".into());
    }
    let panel = describe(Agent::Codex, &old, NOW, &zone());
    assert_eq!(
        panel.stale.as_deref(),
        Some("Stale · no new reading for 31m")
    );
    assert_eq!(
        panel.failure.as_deref(),
        Some("Last read failed: Codex closed.")
    );
    let Body::Windows(rows) = panel.body else {
        panic!("no windows");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tone, Tone::Stale);
}

/// A Codex plan with only a weekly window shows that window, labelled «W» with its reset, instead
/// of a 5-hour item that would stay «—» for ever; with both, the 5-hour window comes first. Any
/// other shortest window is labelled by its length.
#[test]
fn codex_shows_its_five_hour_window_or_else_its_shortest_one() {
    let weekly_only = reading(NOW - 60, None, window(42.0, NOW + 3 * 86_400));
    let items = items(&Source::Off, &weekly_only, NOW, &zone());
    assert_eq!(line(&items), "W 42% · Mon 08:00");
    assert_eq!(items[0].tone, Tone::Normal);

    let both = read(window(10.0, at(18, 26)), window(42.0, NOW + 3 * 86_400));
    assert_eq!(
        line(&super::items(&Source::Off, &both, NOW, &zone())),
        "5h 10% · 18:26"
    );

    let other = Source::Read(Reading {
        observed_at: NOW - 60,
        windows: [12 * 3600, 30 * 86_400, 90 * 60]
            .into_iter()
            .map(|length| Named {
                name: "window".into(),
                length,
                extra: false,
                window: window(5.0, NOW + 60).unwrap(),
            })
            .collect(),
        last_failure: None,
    });
    assert_eq!(
        line(&super::items(&Source::Off, &other, NOW, &zone())),
        "90m 5% · 08:01"
    );
    assert_eq!(length_label(12 * 3600), "12h");
    assert_eq!(length_label(30 * 86_400), "30d");
}

/// A reset on another local date names its day, even when it is hours away: at 23:00, a reset at
/// 01:00 is tomorrow's.
#[test]
fn a_reset_past_midnight_names_its_day() {
    let midnight = NOW - NOW % 86_400;
    let late = midnight + 23 * 3600;
    assert_eq!(clock(late + 1800, late, &zone()).as_deref(), Some("23:30"));
    assert_eq!(clock(late + 2 * 3600, late, &zone()).as_deref(), Some("Sat 01:00"));
}

/// A share that comes as negative zero still reads «0%».
#[test]
fn negative_zero_reads_zero() {
    assert_eq!(percent(-0.0), "0%");
    assert_eq!(percent(0.4), "0%");
}

/// Pace is classified on the figure it prints: five points either way is on track, six is not —
/// behind as well as ahead.
#[test]
fn pace_is_judged_on_the_figure_it_shows() {
    // Half of the five hours gone: a share of 50 % is on track.
    let pace = |used: f64| {
        let source = read(window(used, NOW + FIVE_HOURS / 2), None);
        let Body::Windows(rows) = describe(Agent::Codex, &source, NOW, &zone()).body else {
            panic!("no windows");
        };
        rows[0].pace.clone().unwrap()
    };
    assert!(pace(55.4).starts_with("Pace: on track"), "{}", pace(55.4));
    assert!(pace(44.6).starts_with("Pace: on track"), "{}", pace(44.6));
    assert!(pace(55.6).starts_with("Pace: ahead (+6%)"), "{}", pace(55.6));
    assert!(pace(44.4).starts_with("Pace: behind (−6%)"), "{}", pace(44.4));
    assert!(pace(20.0).starts_with("Pace: behind (−30%)"), "{}", pace(20.0));
}
