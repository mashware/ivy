//! The title bar's allowance chip: what it says, built from the readings AI usage already holds
//! for this window — the Claude broker's snapshots and the Codex allowance observation (#283).
//!
//! The model takes plain values and `now`, so what a person reads is decided here and tested on
//! every platform, while the sources that fill it stay behind their own platform gates.

use chrono::{DateTime, TimeZone};
use eframe::egui;
use std::fmt::Display;

mod popover;
mod summary;
mod surface;
pub(crate) use popover::show as show_summary;
pub(crate) use surface::{Chip, paint};

/// Rebuilds the chip from this frame's readings. Kept out of measured runs, where its work would
/// land in the frame undeclared (D13).
pub(super) fn begin_pass(view: &mut super::View, ctx: &egui::Context) {
    let now = chrono::Utc::now().timestamp();
    #[cfg(target_os = "linux")]
    let claude = view.quota.chip_source();
    #[cfg(not(target_os = "linux"))]
    let claude = Source::Unsupported;
    let codex = view.allowances.chip_source(now);
    #[cfg(test)]
    let (claude, codex) = view.chip.forced.clone().unwrap_or((claude, codex));
    view.chip.update(ctx, claude, codex, now);
    if view.chip.popover {
        // The open summary counts minutes («Updated 3m ago», «in 2h 14m»): a window at rest
        // repaints at the next minute so they do not freeze.
        let wait = 60 - now.rem_euclid(60) as u64;
        ctx.request_repaint_after(std::time::Duration::from_secs(wait));
    }
}

/// The next second at which a reading's look changes with no new read: it turns stale, or one of its
/// windows reaches its reset. Readings only; a source with none has nothing to time.
pub(super) fn next_change(sources: &[&Source], now: i64) -> Option<i64> {
    sources
        .iter()
        .filter_map(|source| match source {
            Source::Read(reading) => Some(reading),
            _ => None,
        })
        .flat_map(|reading| {
            std::iter::once(reading.observed_at + STALE_AFTER + 1).chain(
                reading
                    .windows
                    .iter()
                    .filter_map(|named| named.window.resets_at),
            )
        })
        .filter(|at| *at > now)
        .min()
}

/// After this long a reading is shown as stale instead of as current: an old percentage read as
/// today's is the failure the chip exists to avoid (#283).
pub(super) const STALE_AFTER: i64 = 30 * 60;
/// From here a window is shown amber, and at `EXHAUSTED` red.
const AMBER_FROM: f64 = 80.0;
const EXHAUSTED: f64 = 100.0;
/// The lengths of the 5-hour and the weekly window, which the chip and the popover name.
pub(super) const FIVE_HOURS: i64 = 5 * 60 * 60;
pub(super) const WEEK: i64 = 7 * 24 * 60 * 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    Normal,
    Amber,
    Red,
    /// A reading that is too old to show as current.
    Stale,
    /// No figure: nothing read yet, a window not reported, or one that reset since.
    Unavailable,
    /// The read itself failed: said apart from waiting and stale, on the bar too.
    Failed,
}

/// How a used share reads: amber from 80 %, red when exhausted.
pub(super) fn tone_of(used: f64) -> Tone {
    if used >= EXHAUSTED {
        Tone::Red
    } else if used >= AMBER_FROM {
        Tone::Amber
    } else {
        Tone::Normal
    }
}

/// A used share as a figure. Floored, so it never shows a threshold the tone has not reached:
/// 79.6 reads 79 %.
pub(super) fn percent(used: f64) -> String {
    // Adding zero turns a negative zero into zero, which would otherwise print «-0%».
    format!("{}%", used.clamp(0.0, EXHAUSTED).floor() + 0.0)
}

/// One allowance window as its source reported it. `expired` is a window whose reset has passed:
/// what it holds now is unknown, so it is never shown as a percentage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Allowance {
    pub(super) used: Option<f64>,
    pub(super) resets_at: Option<i64>,
    pub(super) expired: bool,
}

/// A window with the name the popover gives it and its length in seconds.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Named {
    pub(super) name: String,
    pub(super) length: i64,
    /// A window of a limit other than the main allowance: listed in the popover, never the
    /// chip's.
    pub(super) extra: bool,
    pub(super) window: Allowance,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Reading {
    /// Seconds since the epoch.
    pub(super) observed_at: i64,
    /// Every window the source reported: the main allowance's first, then any other limit's.
    pub(super) windows: Vec<Named>,
    /// Why the latest read did not replace this reading, when it did not.
    pub(super) last_failure: Option<String>,
}

impl Reading {
    /// The first window of `length` seconds: the chip's 5-hour window is the one of 300 minutes,
    /// since Codex labels neither of its windows.
    pub(super) fn window(&self, length: i64) -> Option<Allowance> {
        self.windows
            .iter()
            .find(|named| !named.extra && named.length == length)
            .map(|named| named.window)
    }
}

/// What one agent's allowances are in this window.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Source {
    /// This platform cannot read them (#293, #294).
    Unsupported,
    /// The person has not enabled them.
    Off,
    /// Enabled and nothing read yet; the text says what happens next.
    Waiting(String),
    /// Enabled, and the last read failed with nothing earlier to show; the text says why.
    Failed(String),
    /// Enabled, and every launch that could report has ended; the text says how to get one.
    Ended(String),
    Read(Reading),
}

impl Source {
    pub(super) fn enabled(&self) -> bool {
        matches!(
            self,
            Self::Waiting(_) | Self::Failed(_) | Self::Ended(_) | Self::Read(_)
        )
    }

    pub(super) fn reading(&self) -> Option<&Reading> {
        match self {
            Self::Read(reading) => Some(reading),
            _ => None,
        }
    }
}

/// The windows each agent puts on the chip, in the chip's order, as label and length: Claude's
/// 5-hour and weekly windows; Codex's 5-hour window when its plan has one, otherwise its shortest
/// window — a plan with only a weekly window would leave a 5-hour item empty for ever. The
/// popover lists every window.
fn spans(agent: Agent, source: &Source) -> Vec<(String, i64)> {
    match agent {
        Agent::Claude => vec![("5h".into(), FIVE_HOURS), ("W".into(), WEEK)],
        Agent::Codex => {
            let shortest = source.reading().and_then(|reading| {
                if reading.window(FIVE_HOURS).is_some() {
                    return Some(FIVE_HOURS);
                }
                reading
                    .windows
                    .iter()
                    .filter(|named| !named.extra)
                    .map(|named| named.length)
                    .filter(|length| *length > 0)
                    .min()
            });
            let length = shortest.unwrap_or(FIVE_HOURS);
            vec![(length_label(length), length)]
        }
    }
}

/// The agents the chip renders, in its order. With two, the summary has a tab for each and opens
/// on the first; with one, it has no tabs.
pub(super) fn shown(claude: &Source, codex: &Source) -> Vec<Agent> {
    [(Agent::Claude, claude), (Agent::Codex, codex)]
        .into_iter()
        .filter(|(_, source)| source.enabled())
        .map(|(agent, _)| agent)
        .collect()
}

/// The share and tone of the window the chip shows first for `agent`, grey once the reading is
/// stale; none without a figure.
pub(super) fn headline(agent: Agent, source: &Source, now: i64) -> Option<(f64, Tone)> {
    let reading = source.reading()?;
    let (_, length) = spans(agent, source).into_iter().next()?;
    let used = reading
        .window(length)
        .filter(|window| !window.expired)?
        .used?;
    let tone = if now - reading.observed_at > STALE_AFTER {
        Tone::Stale
    } else {
        tone_of(used)
    };
    Some((used, tone))
}

/// A window's length as the chip labels it: «5h», «W», or its hours, days or minutes.
fn length_label(seconds: i64) -> String {
    let minutes = seconds / 60;
    match minutes {
        300 => "5h".into(),
        10_080 => "W".into(),
        _ if minutes % 1440 == 0 => format!("{}d", minutes / 1440),
        _ if minutes % 60 == 0 && minutes < 60_000 => format!("{}h", minutes / 60),
        _ if minutes < 1000 => format!("{minutes}m"),
        // Rounded to hours, or to days, so the label stays within the slot's four characters.
        _ if minutes < 59_970 => format!("{}h", (minutes + 30) / 60),
        _ => format!("{}d", (minutes + 720) / 1440),
    }
}

/// One item of the chip: a dim label, its value, and after a dim «·» the reset where it is shown.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Item {
    pub(super) agent: Agent,
    pub(super) label: String,
    pub(super) value: String,
    pub(super) suffix: Option<String>,
    pub(super) tone: Tone,
}

/// What an item's parts are painted as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    Label,
    Value,
    Dim,
}

impl Item {
    /// The item's text in the order it is painted, each piece with its part.
    pub(super) fn parts(&self) -> Vec<(&str, Part)> {
        let mut parts = vec![
            (self.label.as_str(), Part::Label),
            (" ", Part::Dim),
            (self.value.as_str(), Part::Value),
        ];
        if let Some(suffix) = &self.suffix {
            parts.push((" · ", Part::Dim));
            parts.push((suffix.as_str(), Part::Value));
        }
        parts
    }
}

/// The chip's items, in its fixed order: Claude's 5-hour window with its reset and its weekly
/// window, then Codex's one item. An agent that is not enabled adds nothing, so with nothing
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
        let spans = spans(agent, source);
        let single = spans.len() == 1;
        for (label, length) in spans {
            out.push(item(agent, label, length, single, source, now, zone));
        }
    }
    out
}

fn item<Tz: TimeZone>(
    agent: Agent,
    label: String,
    length: i64,
    single: bool,
    source: &Source,
    now: i64,
    zone: &Tz,
) -> Item
where
    Tz::Offset: Display,
{
    // An item without a figure keeps its label and says why in its value and ink: a grey dash for
    // nothing to show or an old reading, a red «!» for a read that failed. The words are the
    // popover's.
    let without = |value: &str, tone| Item {
        agent,
        label: label.clone(),
        value: value.into(),
        suffix: None,
        tone,
    };
    let reading = match source {
        Source::Read(reading) => reading,
        Source::Failed(_) => return without("!", Tone::Failed),
        _ => return without("—", Tone::Unavailable),
    };
    if now - reading.observed_at > STALE_AFTER {
        return without("—", Tone::Stale);
    }
    let Some(window) = reading.window(length).filter(|window| !window.expired) else {
        return without("—", Tone::Unavailable);
    };
    let Some(used) = window.used else {
        return without("—", Tone::Unavailable);
    };
    let tone = tone_of(used);
    let reset = window.resets_at.and_then(|at| clock(at, now, zone));
    let suffix = match (tone, reset) {
        (Tone::Red, Some(reset)) => Some(format!("back {reset}")),
        // The reset follows Claude's 5-hour figure, and the figure of an agent's only item.
        (_, Some(reset)) if single || (agent == Agent::Claude && length == FIVE_HOURS) => {
            Some(reset)
        }
        _ => None,
    };
    Item {
        agent,
        label,
        value: percent(used),
        suffix,
        tone,
    }
}

/// A reset as a person reads it from the bar: the time when it falls on today's local date, and
/// the weekday with it when it does not — a reset tomorrow at 09:00 is «Tue 09:00», not «09:00».
pub(super) fn clock<Tz: TimeZone>(at: i64, now: i64, zone: &Tz) -> Option<String>
where
    Tz::Offset: Display,
{
    let time = DateTime::from_timestamp(at, 0)?.with_timezone(zone);
    let today = DateTime::from_timestamp(now, 0)?.with_timezone(zone);
    let format = if time.date_naive() == today.date_naive() {
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
        let fixed = agent == Agent::Claude;
        for (label, _) in spans(agent, &Source::Waiting(String::new())) {
            // Any reset can fall on another local date, even a 5-hour one past midnight, so each
            // carries a weekday; Codex's one item can be any of its windows, and its label is at
            // most four characters (`length_label`).
            let label = if fixed { label } else { "888d".to_owned() };
            out.push(Item {
                agent,
                label,
                value: "100%".into(),
                suffix: Some("back Wed 88:88".to_owned()),
                tone: Tone::Red,
            });
        }
    }
    out
}

/// Space around the chip's content, a dot's diameter and the gap after it, the gap either side of
/// an item separator, and the separator.
pub(super) const PAD: f32 = 10.0;
pub(super) const DOT: f32 = 8.0;
pub(super) const DOT_GAP: f32 = 6.0;
pub(super) const ITEM_GAP: f32 = 8.0;
pub(super) const SEPARATOR: &str = "|";

/// The chip's width for `items`, given how wide a text is. One function sizes the slot from the
/// templates and the box around the live chip, so the two cannot disagree on the arithmetic.
pub(super) fn width(items: &[Item], measure: &mut impl FnMut(&str) -> f32) -> f32 {
    if items.is_empty() {
        return 0.0;
    }
    let separator = measure(SEPARATOR) + 2.0 * ITEM_GAP;
    let mut total = 2.0 * PAD;
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            total += separator;
        }
        if index == 0 || items[index - 1].agent != item.agent {
            total += DOT + DOT_GAP;
        }
        total += item
            .parts()
            .iter()
            .map(|(text, _)| measure(text))
            .sum::<f32>();
    }
    total
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod bar_tests;

#[cfg(test)]
mod summary_tests;
