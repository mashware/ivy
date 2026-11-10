//! Overview-only input, projection and paint enqueue work, excluding ordinary pane refresh.
use super::Series;
use std::{io::Write, time::Duration};

#[derive(Default)]
pub(crate) struct Overview {
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
    navigations: usize,
    focus_changes: usize,
    deadlines: usize,
}

impl Overview {
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
    pub(crate) fn projected(&mut self, rows: usize) {
        self.projected += rows;
    }
    pub(crate) fn painted(&mut self, rows: usize) {
        self.painted += rows;
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
            "overview mode: {}",
            match self.mode {
                None => "historical",
                Some(false) => "closed",
                Some(true) => "open",
            }
        )?;
        writeln!(
            to,
            "overview application: frames={}, total={:?}, {} (input, projection and paint enqueue; excludes pane refresh and tessellation)",
            self.work.total, self.total, self.work
        )?;
        writeln!(
            to,
            "overview witnesses: open={}, projected={}, painted={}, navigation={}, focus={}, deadlines={}",
            self.open_frames,
            self.projected,
            self.painted,
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
        let mut timing = Overview::default();
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
}
