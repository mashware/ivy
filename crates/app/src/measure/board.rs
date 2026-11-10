//! Board-only input, projection and paint enqueue work, excluding ordinary pane refresh.
use super::Series;
use std::{io::Write, time::Duration};

#[derive(Default)]
pub(crate) struct Board {
    mode: Option<bool>,
    cost: Duration,
    frame: Duration,
    touched: bool,
    open: bool,
    work: Series,
    total: Duration,
    open_frames: usize,
    projected: usize,
    painted: usize,
    repositories: usize,
    ungrouped: usize,
    navigations: usize,
    focus_changes: usize,
    deadlines: usize,
}

impl Board {
    pub(crate) fn set_mode(&mut self, mode: &str) {
        self.mode = match mode {
            "closed" => Some(false),
            "open" => Some(true),
            _ => None,
        };
    }
    pub(crate) fn record_work(&mut self, cost: Duration) {
        self.cost += cost;
        self.frame += cost;
        self.touched = true;
    }
    /// Drain promptly into the window's subtraction, independently of closing the frame sample.
    pub(crate) fn take_cost(&mut self) -> Duration {
        std::mem::take(&mut self.cost)
    }
    pub(crate) fn saw_open(&mut self) {
        self.open = true;
    }
    pub(crate) fn projected(&mut self, cards: usize) {
        self.projected += cards;
    }
    pub(crate) fn painted(&mut self, cards: usize) {
        self.painted += cards;
    }
    /// One projection's repository sections — «Other consoles» never counts. `repositories` keeps
    /// the most any projection grouped: a maximum, not a sum, because summed over frames a board
    /// that grouped one repository would still reach any floor the script sets. `ungrouped` counts
    /// the projections with no repository section — a board opened before its projects resolved,
    /// or one with none — so the frames averaged into the span without a repository are declared.
    pub(crate) fn grouped(&mut self, repositories: usize) {
        self.repositories = self.repositories.max(repositories);
        self.ungrouped += usize::from(repositories == 0);
    }
    /// Call after resolving and accepting a live identity, with the observed focus transition.
    pub(crate) fn navigation(&mut self, focus_changed: bool) {
        self.navigations += 1;
        self.focus_changes += usize::from(focus_changed);
    }
    pub(crate) fn deadline_expired(&mut self) {
        self.deadlines += 1;
    }
    /// Repeated passes contribute all their work, but only one sample and open frame.
    pub(crate) fn close_frame(&mut self) {
        if std::mem::take(&mut self.touched) {
            let cost = std::mem::take(&mut self.frame);
            self.total += cost;
            self.work.record(cost);
        }
        self.open_frames += usize::from(std::mem::take(&mut self.open));
    }
    pub(crate) fn publish(&self, to: &mut dyn Write) -> std::io::Result<()> {
        writeln!(
            to,
            "board mode: {}",
            match self.mode {
                None => "historical",
                Some(false) => "closed",
                Some(true) => "open",
            }
        )?;
        writeln!(
            to,
            "board application: frames={}, total={:?}, {} (input, projection and paint enqueue; excludes pane refresh and tessellation)",
            self.work.total, self.total, self.work
        )?;
        writeln!(
            to,
            "board witnesses: open={}, projected={}, painted={}, repositories={}, ungrouped={}, navigation={}, focus={}, deadlines={}",
            self.open_frames,
            self.projected,
            self.painted,
            self.repositories,
            self.ungrouped,
            self.navigations,
            self.focus_changes,
            self.deadlines
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_passes_are_subtracted_once_and_recorded_as_one_frame() {
        let mut timing = Board::default();
        timing.record_work(Duration::from_micros(7));
        timing.saw_open();
        assert_eq!(timing.take_cost(), Duration::from_micros(7));
        assert_eq!(timing.take_cost(), Duration::ZERO);
        timing.record_work(Duration::from_micros(11));
        timing.saw_open();
        assert_eq!(timing.take_cost(), Duration::from_micros(11));
        timing.close_frame();
        timing.close_frame();
        let mut report = Vec::new();
        timing.publish(&mut report).unwrap();
        let report = String::from_utf8(report).unwrap();
        assert!(report.contains("frames=1, total=18µs"), "{report}");
        assert!(report.contains("open=1,"), "{report}");
        timing.record_work(Duration::ZERO);
        timing.close_frame();
        assert_eq!(
            timing.work.total, 2,
            "sub-clock work is still one actual frame"
        );
    }

    #[test]
    fn repositories_keep_the_most_one_projection_grouped_and_empty_ones_are_declared() {
        let mut timing = Board::default();
        for repositories in [0, 1, 1, 0] {
            timing.grouped(repositories);
        }
        let mut report = Vec::new();
        timing.publish(&mut report).unwrap();
        let report = String::from_utf8(report).unwrap();
        assert!(
            report.contains("repositories=1, ungrouped=2,"),
            "two one-repository frames must not add up to two repositories: {report}"
        );
    }
}
