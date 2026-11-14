//! Explicit Board states and locally observed waiting durations.
use crate::agent_signals::View;
use panel_contract::{Attention, Panel};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Column {
    Wait,
    Block,
    Working,
    Done,
    Idle,
}
pub(super) const COLUMNS: [Column; 5] = [
    Column::Wait,
    Column::Block,
    Column::Working,
    Column::Done,
    Column::Idle,
];
impl Column {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Wait => "Waiting for you",
            Self::Block => "Stuck",
            Self::Working => "Working",
            Self::Done => "Finished",
            Self::Idle => "Idle",
        }
    }
    pub(super) fn symbol(self) -> &'static str {
        match self {
            Self::Wait => "?",
            Self::Block => "⊘",
            Self::Working => "▸",
            Self::Done => "∎",
            Self::Idle => "·",
        }
    }
    pub(super) fn rank(self) -> usize {
        self as usize
    }
}

pub(super) fn classify(panel: Option<&Panel>, signal: Option<View>, now: SystemTime) -> Column {
    let attention = panel.and_then(|p| p.attention);
    let busy = signal.is_some_and(|s| s.busy);
    if attention == Some(Attention::Wait) && !busy {
        Column::Wait
    } else if attention == Some(Attention::Block) {
        Column::Block
    } else if attention == Some(Attention::Done) || signal.is_some_and(|s| s.done) {
        Column::Done
    } else if signal.is_some_and(|s| s.waiting) {
        Column::Wait
    } else if panel.is_some_and(|p| p.is_stale_at(now)) {
        Column::Idle
    } else if busy {
        Column::Working
    } else {
        Column::Idle
    }
}

const WARM: Duration = Duration::from_secs(300);
const HOT: Duration = Duration::from_secs(900);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Heat {
    Normal,
    Warm,
    Hot,
}
pub(super) fn heat(age: Duration) -> Heat {
    if age >= HOT {
        Heat::Hot
    } else if age >= WARM {
        Heat::Warm
    } else {
        Heat::Normal
    }
}
#[derive(Default)]
pub(super) struct Observations {
    panes: HashMap<usize, (Column, Option<Instant>)>,
    epochs: HashMap<usize,u64>,
}
impl Observations {
    pub(super) fn observe_epoch(&mut self,pane:usize,column:Column,epoch:u64,now:Instant) {
        if self.epochs.get(&pane).is_some_and(|old|*old!=epoch){self.reset(pane);}
        self.epochs.insert(pane,epoch);
        self.observe(pane,column,now);
    }

    pub(super) fn observe(&mut self, pane: usize, column: Column, now: Instant) {
        let entry = self.panes.entry(pane).or_insert((column, None));
        if column == Column::Wait {
            if entry.1.is_none() {
                entry.1 = Some(now);
            }
        } else {
            entry.1 = None;
        }
        entry.0 = column;
    }
    pub(super) fn retain(&mut self, live: &[usize]) {
        self.panes.retain(|pane, _| live.contains(pane));
        self.epochs.retain(|pane,_|live.contains(pane));
    }
    pub(super) fn since(&self, pane: usize) -> Option<Instant> {
        self.panes.get(&pane).and_then(|entry| entry.1)
    }
    pub(super) fn reset(&mut self, pane: usize) {
        self.panes.remove(&pane);
        self.epochs.remove(&pane);
    }
    pub(super) fn next_threshold(&self, now: Instant) -> Option<Duration> {
        self.panes
            .values()
            .filter_map(|(_, since)| {
                let age = now.saturating_duration_since((*since)?);
                if age < WARM {
                    Some(WARM - age)
                } else if age < HOT {
                    Some(HOT - age)
                } else {
                    None
                }
            })
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_signals::Client;
    fn signal(busy: bool, waiting: bool, done: bool) -> View {
        View {
            client: Client::Codex,
            busy,
            waiting,
            done,
            backed: true,
        }
    }
    fn panel(attention: Option<Attention>, stale: bool, now: SystemTime) -> Panel {
        let mut panel =
            panel_contract::parse_document(r#"{"lines":["body"],"phase":"working"}"#).unwrap();
        panel.attention = attention;
        if stale {
            panel.written_at = Some(now - panel.stale_after - Duration::from_secs(1));
        }
        panel
    }
    #[test]
    fn new_shell_epoch_resets_even_an_unchanged_wait() {
        let mut state=Observations::default();let now=Instant::now();
        state.observe_epoch(1,Column::Wait,0,now);
        state.observe_epoch(1,Column::Wait,0,now+Duration::from_secs(10));
        assert_eq!(state.since(1),Some(now));
        state.observe_epoch(1,Column::Wait,1,now+Duration::from_secs(20));
        assert_eq!(state.since(1),Some(now+Duration::from_secs(20)));
    }
    #[test]
    fn explicit_precedence_and_missing_panel() {
        let now = SystemTime::now();
        let cases = [
            (
                Some(Attention::Wait),
                signal(false, false, true),
                false,
                Column::Wait,
            ),
            (
                Some(Attention::Wait),
                signal(true, false, false),
                false,
                Column::Working,
            ),
            (
                Some(Attention::Block),
                signal(true, true, true),
                false,
                Column::Block,
            ),
            (
                Some(Attention::Done),
                signal(false, true, false),
                false,
                Column::Done,
            ),
            (None, signal(false, true, false), true, Column::Wait),
            (None, signal(true, false, false), true, Column::Idle),
            (None, signal(true, false, false), false, Column::Working),
            (None, signal(false, false, false), false, Column::Idle),
        ];
        for (attention, view, stale, expected) in cases {
            assert_eq!(
                classify(Some(&panel(attention, stale, now)), Some(view), now),
                expected
            );
        }
        assert_eq!(
            classify(None, Some(signal(false, false, true)), now),
            Column::Done
        );
        assert_eq!(classify(None, None, now), Column::Idle);
    }
    #[test]
    fn waits_keep_start_until_leave_reset_or_removal() {
        let now = Instant::now();
        let mut observations = Observations::default();
        observations.observe(1, Column::Wait, now);
        observations.observe(1, Column::Wait, now + Duration::from_secs(3));
        assert_eq!(observations.since(1), Some(now));
        observations.observe(1, Column::Working, now);
        assert_eq!(observations.since(1), None);
        observations.observe(1, Column::Wait, now + Duration::from_secs(4));
        assert_eq!(observations.since(1), Some(now + Duration::from_secs(4)));
        observations.reset(1);
        assert_eq!(observations.since(1), None);
        observations.observe(1, Column::Wait, now);
        observations.observe(2, Column::Wait, now);
        observations.retain(&[2]);
        assert_eq!(observations.since(1), None);
        assert_eq!(observations.since(2), Some(now));
    }
    #[test]
    fn exact_heat_and_deadline_boundaries() {
        let now = Instant::now();
        let mut observations = Observations::default();
        observations.observe(1, Column::Wait, now);
        for (seconds, expected, deadline) in [
            (299, Heat::Normal, Some(1)),
            (300, Heat::Warm, Some(600)),
            (899, Heat::Warm, Some(1)),
            (900, Heat::Hot, None),
        ] {
            assert_eq!(heat(Duration::from_secs(seconds)), expected);
            assert_eq!(
                observations.next_threshold(now + Duration::from_secs(seconds)),
                deadline.map(Duration::from_secs)
            );
        }
    }
}
