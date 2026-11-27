//! What AI usage declares in the frame report (D13): its measured modes and one named series per
//! kind of work, so none of it is published as the framework's cost.

use super::{View, request_comparison};
use crate::Window;
use eframe::egui;
use std::{
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};
use usage_core::Service;

#[derive(Clone, Copy)]
pub(super) enum Measurement {
    Open,
    Closed,
    Allowances,
}

pub(crate) fn configure_measurement(window: &mut Window, ctx: &egui::Context, measuring: bool) {
    window.usage.measuring = measuring;
    if !measuring {
        return;
    }
    let mode = match std::env::var("MEASURE_USAGE").ok().as_deref() {
        Some("open") => Measurement::Open,
        Some("closed") => Measurement::Closed,
        Some("allowances") => Measurement::Allowances,
        _ => return,
    };
    let Some(root) = std::env::var_os("MEASURE_USAGE_ROOT").map(PathBuf::from) else {
        return;
    };
    let Some(cache) = std::env::var_os("MEASURE_USAGE_CACHE").map(PathBuf::from) else {
        return;
    };
    let ctx = ctx.clone();
    let service = Service::new(cache, move || ctx.request_repaint());
    service.enable(vec![root]);
    window.usage.service = Some(service);
    window.usage.measured = Some(mode);
    window.usage.open = matches!(mode, Measurement::Open | Measurement::Allowances);
    if matches!(mode, Measurement::Allowances) {
        window.usage.allowances.prepare_measurement();
    }
    request_comparison(&mut window.usage);
}

/// The report's names for the series, by index.
const SERIES: [&str; 2] = ["Usage snapshot", "Usage paint"];

#[derive(Default)]
pub(super) struct Timing {
    pub(super) pending: [Option<Duration>; 2],
    series: [crate::measure::Series; 2],
    totals: [Duration; 2],
    frames: [usize; 2],
}

impl Timing {
    pub(super) fn record(&mut self, index: usize, cost: Duration) -> Duration {
        *self.pending[index].get_or_insert_default() += cost;
        cost
    }
}

impl View {
    pub(crate) fn close_frame(&mut self) {
        let timing = &mut self.timing;
        for (index, pending) in timing.pending.iter_mut().enumerate() {
            if let Some(cost) = pending.take() {
                timing.series[index].record(cost);
                timing.totals[index] += cost;
                timing.frames[index] += 1;
            }
        }
    }

    pub(crate) fn publish(&self, to: &mut dyn Write) -> io::Result<()> {
        let mode = match self.measured {
            Some(Measurement::Open) => "open",
            Some(Measurement::Closed) => "closed",
            Some(Measurement::Allowances) => "allowances",
            None => "none",
        };
        let tokens = self.snapshot.as_ref().map_or(0, |s| s.totals.tokens);
        writeln!(to, "usage measurement: {mode}, tokens={tokens}")?;
        writeln!(
            to,
            "allowance fixture windows: {}",
            self.allowances.measured_windows()
        )?;
        for (index, name) in SERIES.into_iter().enumerate() {
            let frames = self.timing.frames[index];
            let total = self.timing.totals[index];
            writeln!(to, "{name}: frames={frames}, total={total:?}")?;
            writeln!(to, "{name} series: {}", self.timing.series[index])?;
        }
        Ok(())
    }
}
