//! What the summary under the chip says for one agent: a header with when it was read, then each
//! window with its use, reset and pace — or, before there is a reading, why there is none.

use super::{Agent, Allowance, Named, STALE_AFTER, Source, Tone, clock, percent, tone_of};
use chrono::TimeZone;
use std::fmt::Display;

/// A used share within this many points of the elapsed share is on track.
const PACE_MARGIN: f64 = 5.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Panel {
    /// «Updated …», when there is a reading.
    pub(super) updated: Option<String>,
    /// Why what is shown may not be current: stale, or the last read failed.
    pub(super) note: Option<String>,
    pub(super) body: Body,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Body {
    /// No windows to show: what the state is and, where known, what happens next.
    Message {
        text: String,
        hint: Option<String>,
        tone: Tone,
    },
    Windows(Vec<Row>),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub(super) name: String,
    /// The share for the bar; none where the figure is unknown.
    pub(super) used: Option<f64>,
    pub(super) used_text: String,
    pub(super) reset: Option<String>,
    pub(super) pace: Option<String>,
    pub(super) tone: Tone,
}

fn message(text: &str, hint: Option<String>, tone: Tone) -> Panel {
    Panel {
        updated: None,
        note: None,
        body: Body::Message {
            text: text.into(),
            hint,
            tone,
        },
    }
}

pub(super) fn describe<Tz: TimeZone>(agent: Agent, source: &Source, now: i64, zone: &Tz) -> Panel
where
    Tz::Offset: Display,
{
    let reading = match source {
        Source::Unsupported => {
            return message(
                "Allowances are not available on this platform.",
                None,
                Tone::Stale,
            );
        }
        Source::Off => {
            return message(
                "Allowance reads are off.",
                Some("Turn them on in AI usage.".into()),
                Tone::Stale,
            );
        }
        Source::Waiting(hint) => {
            return message(
                "Waiting for the first reading.",
                Some(hint.clone()),
                Tone::Stale,
            );
        }
        Source::Failed(why) => {
            let text = format!("Could not read {} allowances.", agent.name());
            return message(&text, Some(why.clone()), Tone::Red);
        }
        Source::Read(reading) => reading,
    };
    let age = now - reading.observed_at;
    let stale = age > STALE_AFTER;
    let note = if stale {
        Some(format!("Stale · no new reading for {}", duration(age)))
    } else {
        reading.note.clone()
    };
    let body = if reading.windows.is_empty() {
        Body::Message {
            text: "No windows reported yet.".into(),
            hint: None,
            tone: Tone::Stale,
        }
    } else {
        Body::Windows(
            reading
                .windows
                .iter()
                .map(|named| row(named, stale, now, zone))
                .collect(),
        )
    };
    Panel {
        updated: Some(format!("Updated {}", ago(age))),
        note,
        body,
    }
}

fn row<Tz: TimeZone>(named: &Named, stale: bool, now: i64, zone: &Tz) -> Row
where
    Tz::Offset: Display,
{
    let Allowance {
        used,
        resets_at,
        expired,
    } = named.window;
    let unknown = |text: &str| Row {
        name: named.name.clone(),
        used: None,
        used_text: text.into(),
        reset: None,
        pace: None,
        tone: Tone::Stale,
    };
    if expired {
        return unknown("Reset, not read since");
    }
    let Some(used) = used else {
        return unknown("Usage not reported");
    };
    let reset = resets_at.and_then(|at| {
        let when = clock(at, now, zone)?;
        let at_time = if at - now < 24 * 60 * 60 { "at " } else { "" };
        Some(format!(
            "Resets {at_time}{when} (in {})",
            duration(at - now)
        ))
    });
    Row {
        name: named.name.clone(),
        used: Some(used),
        used_text: format!("{} used", percent(used)),
        reset,
        pace: resets_at.and_then(|at| pace(named.length, used, at, now)),
        tone: if stale { Tone::Stale } else { tone_of(used) },
    }
}

/// Whether the window is being used faster than its time runs — the used share against the share
/// of the window already gone — and, at that rate, whether it lasts to its reset.
fn pace(length: i64, used: f64, resets_at: i64, now: i64) -> Option<String> {
    if length <= 0 || used >= 100.0 {
        return None;
    }
    let left = (resets_at - now).clamp(0, length);
    let gone = length - left;
    let elapsed = gone as f64 * 100.0 / length as f64;
    let delta = used - elapsed;
    let mut text = if delta.abs() <= PACE_MARGIN {
        "Pace: on track".to_owned()
    } else if delta > 0.0 {
        format!("Pace: ahead (+{}%)", delta.round())
    } else {
        format!("Pace: behind (−{}%)", (-delta).round())
    };
    if gone > 0 && used > 0.0 {
        let to_full = ((100.0 - used) * gone as f64 / used) as i64;
        if to_full < left {
            text.push_str(&format!(" · may run out in ~{}", duration(to_full)));
        } else {
            text.push_str(" · lasts to reset");
        }
    }
    Some(text)
}

/// How long ago, as the header says it.
fn ago(seconds: i64) -> String {
    if seconds < 60 {
        "just now".into()
    } else {
        format!("{} ago", duration(seconds))
    }
}

/// A span in its two largest units: «3h 53m», «3d 20h», «42m».
pub(super) fn duration(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    let (days, hours, minutes) = (minutes / 1440, minutes / 60 % 24, minutes % 60);
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}
