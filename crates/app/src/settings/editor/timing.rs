//! Named application work, summed across passes and recorded once per frame.
use std::{io::Write, time::Duration};
#[derive(Default)]
pub(super) struct Timing {
    pending: [Option<Duration>; 5],
    frames: [usize; 5],
    totals: [Duration; 5],
    pub work: Duration,
    series: [crate::measure::Series; 5],
}
impl Timing {
    pub fn record_work(&mut self, index: usize, cost: Duration) {
        *self.pending[index].get_or_insert_default() += cost;
        self.work += cost;
    }
    pub fn close_frame(&mut self) {
        for (index, pending) in self.pending.iter_mut().enumerate() {
            if let Some(cost) = pending.take() {
                self.series[index].record(cost);
                self.frames[index] += 1;
                self.totals[index] += cost;
            }
        }
    }
    pub fn publish(&self, to: &mut dyn Write) -> std::io::Result<()> {
        for (index, name) in [
            "entry",
            "editor",
            "config I/O and validation",
            "font enumeration",
            "input ownership",
        ]
        .iter()
        .enumerate()
        {
            let frames = self.frames[index];
            let total = self.totals[index];
            writeln!(
                to,
                "settings {name}: frames={frames}, total={total:?}, mean={:?}",
                total.checked_div(frames as u32)
            )?;
            writeln!(to, "settings {name} series: {}", self.series[index])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passes_sum_into_one_frame_and_cost_is_discounted_once() {
        let mut timing = Timing::default();
        timing.record_work(0, Duration::from_micros(7));
        timing.record_work(0, Duration::from_micros(13));
        assert_eq!(std::mem::take(&mut timing.work), Duration::from_micros(20));
        assert_eq!(std::mem::take(&mut timing.work), Duration::ZERO);
        timing.close_frame();
        timing.close_frame();
        assert_eq!(timing.frames[0], 1);
        assert_eq!(timing.totals[0], Duration::from_micros(20));
        let mut output = Vec::new();
        timing.publish(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("settings entry: frames=1, total=20µs"));
        assert!(output.contains("settings font enumeration: frames=0"));
    }
}
