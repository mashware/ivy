//! The title bar's allowance chip: what it says, built from the readings AI usage already holds
//! for this window — the Claude broker's snapshots and the Codex allowance observation (#283).
//!
//! The model takes plain values and `now`, so what a person reads is decided here and tested on
//! every platform, while the sources that fill it stay behind their own platform gates.

use chrono::{DateTime, TimeZone};
use eframe::egui;
use std::fmt::Display;

mod popover;
mod surface;
pub(crate) use popover::show as show_summary;
pub(crate) use surface::{Chip, paint};

/// Rebuilds the chip from this frame's readings. Kept out of measured runs, where its work would
/// land in the frame undeclared (D13).
pub(super) fn begin_pass(view: &mut super::View, ctx: &egui::Context) {
    let now = chrono::Utc::now().timestamp();
    #[cfg(test)]
    if let Some((claude, codex)) = view.chip.forced {
        view.chip.update(ctx, claude, codex, now);
        return;
    }
    #[cfg(target_os = "linux")]
    let claude = view.quota.chip_source();
    #[cfg(not(target_os = "linux"))]
    let claude = Source::Unsupported;
    let codex = view.allowances.chip_source(now);
    view.chip.update(ctx, claude, codex, now);
}

/// The next second at which the chip's text changes with no new reading: a reading turning stale
/// or a window reaching its reset.
pub(super) fn next_change(sources: &[Source], now: i64) -> Option<i64> {
    sources
        .iter()
        .filter_map(|source| match source {
            Source::Read(reading) => Some(reading),
            _ => None,
        })
        .flat_map(|reading| {
            [
                Some(reading.observed_at + STALE_AFTER + 1),
                reading.five_hour.and_then(|window| window.resets_at),
                reading.weekly.and_then(|window| window.resets_at),
            ]
        })
        .flatten()
        .filter(|at| *at > now)
        .min()
}

/// After this long a reading is shown as stale instead of as current: an old percentage read as
/// today's is the failure the chip exists to avoid (#283).
pub(super) const STALE_AFTER: i64 = 30 * 60;
/// From here a window is shown amber, and at `EXHAUSTED` red.
const AMBER_FROM: f64 = 80.0;
const EXHAUSTED: f64 = 100.0;
/// The two windows' lengths, for the pace the popover shows.
const FIVE_HOURS: i64 = 5 * 60 * 60;
const WEEK: i64 = 7 * 24 * 60 * 60;
/// A used share within this many points of the elapsed share is on pace.
const PACE_MARGIN: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Span {
    FiveHour,
    Weekly,
}

impl Span {
    fn short(self) -> &'static str {
        match self {
            Self::FiveHour => "5h",
            Self::Weekly => "W",
        }
    }

    fn long(self) -> &'static str {
        match self {
            Self::FiveHour => "5 hours",
            Self::Weekly => "Weekly",
        }
    }

    fn seconds(self) -> i64 {
        match self {
            Self::FiveHour => FIVE_HOURS,
            Self::Weekly => WEEK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Normal,
    Amber,
    Red,
    Stale,
}

/// One allowance window as its source reported it. `expired` is a window whose reset has passed:
/// what it holds now is unknown, so it is never shown as a percentage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Allowance {
    pub(super) used: Option<f64>,
    pub(super) resets_at: Option<i64>,
    pub(super) expired: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Reading {
    /// Seconds since the epoch.
    pub(super) observed_at: i64,
    pub(super) five_hour: Option<Allowance>,
    pub(super) weekly: Option<Allowance>,
}

impl Reading {
    fn window(&self, span: Span) -> Option<Allowance> {
        match span {
            Span::FiveHour => self.five_hour,
            Span::Weekly => self.weekly,
        }
    }
}

/// What one agent's allowances are in this window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Source {
    /// This platform cannot read them (#293, #294).
    Unsupported,
    /// The person has not enabled them.
    Off,
    /// Enabled, and nothing read yet.
    Waiting,
    Read(Reading),
}

impl Source {
    pub(super) fn enabled(&self) -> bool {
        matches!(self, Self::Waiting | Self::Read(_))
    }
}

/// The windows each agent puts on the chip, in the chip's order. Codex's weekly window and any
/// other bucket appear only in the popover.
fn spans(agent: Agent) -> &'static [Span] {
    match agent {
        Agent::Claude => &[Span::FiveHour, Span::Weekly],
        Agent::Codex => &[Span::FiveHour],
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Item {
    pub(super) agent: Agent,
    pub(super) text: String,
    pub(super) tone: Tone,
}

/// The chip's items, in its fixed order: Claude's 5-hour window with its reset and its weekly
/// window, then Codex's 5-hour window. An agent that is not enabled adds nothing, so with nothing
/// enabled there is no chip.
pub(super) fn items<Tz: TimeZone>(claude: &Source, codex: &Source, now: i64, zone: &Tz) -> Vec<Item>
where
    Tz::Offset: Display,
{
    let mut out = Vec::new();
    for (agent, source) in [(Agent::Claude, claude), (Agent::Codex, codex)] {
        if !source.enabled() {
            continue;
        }
        for &span in spans(agent) {
            let (text, tone) = item(agent, span, source, now, zone);
            out.push(Item { agent, text, tone });
        }
    }
    out
}

fn item<Tz: TimeZone>(
    agent: Agent,
    span: Span,
    source: &Source,
    now: i64,
    zone: &Tz,
) -> (String, Tone)
where
    Tz::Offset: Display,
{
    let label = span.short();
    let stale = || (format!("{label} stale"), Tone::Stale);
    let Source::Read(reading) = source else {
        return stale();
    };
    if now - reading.observed_at > STALE_AFTER {
        return stale();
    }
    let Some(window) = reading.window(span).filter(|window| !window.expired) else {
        return stale();
    };
    let Some(used) = window.used else {
        return stale();
    };
    // Floored, so the figure never shows a threshold the tone has not reached: 79.6 reads 79 %.
    let shown = used.clamp(0.0, EXHAUSTED).floor();
    let reset = window.resets_at.and_then(|at| clock(at, now, zone));
    if used >= EXHAUSTED {
        return match reset {
            Some(reset) => (format!("{label} {shown}% · back {reset}"), Tone::Red),
            None => (format!("{label} {shown}%"), Tone::Red),
        };
    }
    let tone = if used >= AMBER_FROM {
        Tone::Amber
    } else {
        Tone::Normal
    };
    match reset {
        Some(reset) if agent == Agent::Claude && span == Span::FiveHour => {
            (format!("{label} {shown}% · {reset}"), tone)
        }
        _ => (format!("{label} {shown}%"), tone),
    }
}

/// A reset as a person reads it from the bar: the time when it falls within a day, and the weekday
/// with it when it does not.
fn clock<Tz: TimeZone>(at: i64, now: i64, zone: &Tz) -> Option<String>
where
    Tz::Offset: Display,
{
    let time = DateTime::from_timestamp(at, 0)?.with_timezone(zone);
    let format = if at - now < 24 * 60 * 60 {
        "%H:%M"
    } else {
        "%a %H:%M"
    };
    Some(time.format(format).to_string())
}

/// The widest text each enabled item can take, so the slot is sized by what the chip may say and
/// not by what it says now: a slot that followed the live value would move the tabs as it changed.
pub(super) fn templates(claude: &Source, codex: &Source) -> Vec<Item> {
    let mut out = Vec::new();
    for (agent, source) in [(Agent::Claude, claude), (Agent::Codex, codex)] {
        if !source.enabled() {
            continue;
        }
        for &span in spans(agent) {
            // A 5-hour window resets within the day, so its reset never carries a weekday.
            let day = if span == Span::Weekly { "Wed " } else { "" };
            let text = format!("{} 100% · back {day}88:88", span.short());
            out.push(Item {
                agent,
                text,
                tone: Tone::Red,
            });
        }
    }
    out
}

/// Space around the chip's content, a dot's diameter and the gap after it, and an item separator.
pub(super) const PAD: f32 = 8.0;
pub(super) const DOT: f32 = 6.0;
pub(super) const DOT_GAP: f32 = 5.0;
pub(super) const SEPARATOR: &str = " | ";

/// The chip's width for `items`, given how wide a text is. One function sizes the slot from the
/// templates and lays out the live chip, so the two cannot disagree on the arithmetic.
pub(super) fn width(items: &[Item], measure: &mut impl FnMut(&str) -> f32) -> f32 {
    if items.is_empty() {
        return 0.0;
    }
    let separator = measure(SEPARATOR);
    let mut total = 2.0 * PAD;
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            total += separator;
        }
        if index == 0 || items[index - 1].agent != item.agent {
            total += DOT + DOT_GAP;
        }
        total += measure(&item.text);
    }
    total
}

/// The popover's lines for one agent.
pub(super) fn describe<Tz: TimeZone>(
    agent: Agent,
    source: &Source,
    now: i64,
    zone: &Tz,
) -> Vec<(String, Tone)>
where
    Tz::Offset: Display,
{
    let reading = match source {
        Source::Unsupported => {
            return vec![(
                "Allowances are not available on this platform.".into(),
                Tone::Stale,
            )];
        }
        Source::Off => {
            return vec![(
                "Allowance reads are off. Turn them on in AI usage.".into(),
                Tone::Stale,
            )];
        }
        Source::Waiting => return vec![("Waiting for the first reading.".into(), Tone::Stale)],
        Source::Read(reading) => reading,
    };
    let stale = now - reading.observed_at > STALE_AFTER;
    let mut out = Vec::new();
    let observed = clock(reading.observed_at, now, zone).unwrap_or_default();
    out.push(if stale {
        (format!("Stale · last reading {observed}"), Tone::Stale)
    } else {
        (format!("Read at {observed}"), Tone::Normal)
    });
    if agent == Agent::Claude {
        out.push((
            "From the launch's statusline · account unverified".into(),
            Tone::Stale,
        ));
    }
    for span in [Span::FiveHour, Span::Weekly] {
        out.push(window_line(span, reading.window(span), stale, now, zone));
    }
    out
}

fn window_line<Tz: TimeZone>(
    span: Span,
    window: Option<Allowance>,
    stale: bool,
    now: i64,
    zone: &Tz,
) -> (String, Tone)
where
    Tz::Offset: Display,
{
    let name = span.long();
    let Some(window) = window else {
        return (format!("{name} · not reported"), Tone::Stale);
    };
    if window.expired {
        return (format!("{name} · reset, not read since"), Tone::Stale);
    }
    let Some(used) = window.used else {
        return (format!("{name} · usage not reported"), Tone::Stale);
    };
    let shown = used.clamp(0.0, EXHAUSTED).floor();
    let mut text = format!("{name} · {shown}% used");
    if let Some(reset) = window.resets_at.and_then(|at| clock(at, now, zone)) {
        text.push_str(&format!(" · resets {reset}"));
    }
    if let Some(pace) = window.resets_at.map(|at| pace(span, used, at, now)) {
        text.push_str(&format!(" · {pace}"));
    }
    let tone = if stale {
        Tone::Stale
    } else if used >= EXHAUSTED {
        Tone::Red
    } else if used >= AMBER_FROM {
        Tone::Amber
    } else {
        Tone::Normal
    };
    (text, tone)
}

/// Whether the window is being used faster than its time runs: the used share against the share
/// of the window already gone.
fn pace(span: Span, used: f64, resets_at: i64, now: i64) -> &'static str {
    let length = span.seconds();
    let left = (resets_at - now).clamp(0, length);
    let elapsed = (length - left) as f64 * 100.0 / length as f64;
    if used > elapsed + PACE_MARGIN {
        "ahead of pace"
    } else if used < elapsed - PACE_MARGIN {
        "under pace"
    } else {
        "on pace"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    const NOW: i64 = 1_800_000_000;

    /// The chip's text as one line, items joined the way the bar paints them.
    fn line(items: &[Item]) -> String {
        items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>()
            .join(SEPARATOR)
    }

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

    fn at(hours: i64, minutes: i64) -> i64 {
        // NOW is 08:00 UTC; a time later the same day.
        let midnight = NOW - NOW % 86_400;
        midnight + hours * 3600 + minutes * 60
    }

    fn read(five_hour: Option<Allowance>, weekly: Option<Allowance>) -> Source {
        Source::Read(Reading {
            observed_at: NOW - 60,
            five_hour,
            weekly,
        })
    }

    /// AC2: the owner's example, Claude's dot first and its reset after the 5-hour figure.
    #[test]
    fn a_claude_reading_reads_five_hours_with_its_reset_then_the_week() {
        let claude = read(window(38.0, at(18, 26)), window(6.0, NOW + 3 * 86_400));
        let items = items(&claude, &Source::Off, NOW, &zone());
        assert_eq!(line(&items), "5h 38% · 18:26 | W 6%");
        assert!(items.iter().all(|item| item.agent == Agent::Claude));
        assert!(items.iter().all(|item| item.tone == Tone::Normal));
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
            (items[0].tone, items[0].text.clone())
        };
        assert_eq!(tone_and_text(79.9), (Tone::Normal, "5h 79%".into()));
        assert_eq!(tone_and_text(80.0), (Tone::Amber, "5h 80%".into()));
        assert_eq!(tone_and_text(99.9), (Tone::Amber, "5h 99%".into()));
        assert_eq!(
            tone_and_text(100.0),
            (Tone::Red, "5h 100% · back 18:26".into())
        );
    }

    /// AC4: no reading, an old one, a window reset since, or a window with no figure are all
    /// greyed as stale — never a percentage, and above all never «0%».
    #[test]
    fn a_missing_or_old_reading_is_stale_and_never_zero() {
        let old = Source::Read(Reading {
            observed_at: NOW - STALE_AFTER - 1,
            five_hour: window(0.0, at(18, 0)),
            weekly: window(0.0, NOW + 86_400 * 2),
        });
        let fresh_edge = Source::Read(Reading {
            observed_at: NOW - STALE_AFTER,
            five_hour: window(0.0, at(18, 0)),
            weekly: window(0.0, NOW + 86_400 * 2),
        });
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
        for source in [Source::Waiting, old, expired] {
            let items = items(&source, &Source::Off, NOW, &zone());
            assert_eq!(line(&items), "5h stale | W stale", "{source:?}");
            assert!(items.iter().all(|item| item.tone == Tone::Stale));
        }
        let items = items(&fresh_edge, &Source::Off, NOW, &zone());
        assert_eq!(line(&items), "5h 0% · 18:00 | W 0%");
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
        let both = templates(&Source::Waiting, &Source::Waiting);
        assert_eq!(both.len(), 3, "Claude's two windows and Codex's 5-hour one");
    }

    /// AC7: where allowances cannot be read the popover says so, instead of a reading or a hint
    /// to enable them.
    #[test]
    fn an_unsupported_platform_says_allowances_are_not_available() {
        for agent in [Agent::Claude, Agent::Codex] {
            let lines = describe(agent, &Source::Unsupported, NOW, &zone());
            assert_eq!(
                lines,
                vec![(
                    "Allowances are not available on this platform.".to_owned(),
                    Tone::Stale
                )]
            );
        }
        let off = describe(Agent::Codex, &Source::Off, NOW, &zone());
        assert_ne!(off[0].0, "Allowances are not available on this platform.");
    }

    #[test]
    fn the_popover_shows_both_windows_with_reset_and_pace() {
        // Half of the five hours gone and 80 % used: ahead of pace, amber.
        let claude = read(
            window(80.0, NOW + FIVE_HOURS / 2),
            window(10.0, NOW + WEEK - WEEK / 10),
        );
        let lines = describe(Agent::Claude, &claude, NOW, &zone());
        let texts: Vec<_> = lines.iter().map(|(text, _)| text.as_str()).collect();
        assert_eq!(texts[0], "Read at 07:59");
        assert!(texts[1].contains("account unverified"));
        assert_eq!(
            lines[2],
            (
                "5 hours · 80% used · resets 10:30 · ahead of pace".into(),
                Tone::Amber
            )
        );
        // A tenth of the week gone and a tenth used; the reset is days away, so it names the day.
        assert_eq!(texts[3], "Weekly · 10% used · resets Thu 15:12 · on pace");
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
                            egui::FontId::proportional(12.0),
                            egui::Color32::WHITE,
                        )
                        .size()
                        .x
                })
            };
            let slot = width(&templates(&Source::Waiting, &Source::Waiting), &mut measure);
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
}

#[cfg(test)]
mod bar_tests;

#[cfg(test)]
mod summary_tests;
