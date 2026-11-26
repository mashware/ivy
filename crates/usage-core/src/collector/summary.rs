use super::{Cost, DateRange, Day, Model, Snapshot, Totals};
use crate::accounting::Resolution;
use crate::ledger::{Contribution, Ledger};
use crate::pricing::{Catalog, Estimate, EstimateTotal};
use chrono::{DateTime, Local, NaiveDate, Utc};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Default)]
struct Accumulator {
    totals: Totals,
    estimates: EstimateTotal,
}

impl Accumulator {
    fn add(&mut self, item: &Contribution, catalog: &Catalog) {
        let counts = &item.counts;
        self.totals.partial |= !counts.is_complete() || item.resolution == Resolution::Conflicted;
        if let Some(tokens) = counts.known_tokens() {
            self.totals.tokens += tokens as u128;
            self.totals.tokens_known = true;
        }
        self.totals.uncached_input_known |= counts.uncached_input.is_some();
        self.totals.cache_read_known |= counts.cache_read.is_some();
        self.totals.cache_write_known |= counts.cache_write.is_some();
        self.totals.output_known |= counts.output.is_some();
        self.totals.uncached_input += counts.uncached_input.unwrap_or_default() as u128;
        self.totals.cache_read += counts.cache_read.unwrap_or_default() as u128;
        self.totals.cache_write += counts.cache_write.unwrap_or_default() as u128;
        self.totals.output += counts.output.unwrap_or_default() as u128;
        self.estimates
            .add(catalog.estimate(item.model.as_deref(), counts));
    }

    fn finish(mut self) -> Totals {
        self.totals.estimate = match self.estimates.estimate() {
            Estimate::Unavailable => None,
            Estimate::Partial(price) => Some(Cost {
                usd: price.attodollars() as f64 / 1e18,
                partial: true,
            }),
            Estimate::Complete(price) => Some(Cost {
                usd: price.attodollars() as f64 / 1e18,
                partial: false,
            }),
        };
        self.totals
    }
}

#[derive(Default)]
struct Models {
    current: Option<(Option<String>, Accumulator)>,
    visible: Vec<Model>,
    remainder: Accumulator,
    has_remainder: bool,
}

impl Models {
    fn add(&mut self, item: Contribution, catalog: &Catalog) {
        if self
            .current
            .as_ref()
            .is_some_and(|(label, _)| *label != item.model)
        {
            self.flush();
        }
        if self.visible.len() >= 50 {
            self.has_remainder = true;
            self.remainder.add(&item, catalog);
        } else {
            self.current
                .get_or_insert_with(|| (item.model.clone(), Accumulator::default()))
                .1
                .add(&item, catalog);
        }
    }

    fn flush(&mut self) {
        if let Some((label, totals)) = self.current.take() {
            self.visible.push(Model {
                label: label.unwrap_or_else(|| "Unknown model".into()),
                totals: totals.finish(),
            });
        }
    }

    fn finish(mut self) -> Vec<Model> {
        self.flush();
        if self.has_remainder {
            self.visible.push(Model {
                label: "Other models".into(),
                totals: self.remainder.finish(),
            });
        }
        self.visible
    }
}

pub(super) struct Summary {
    pub(super) snapshot: Snapshot,
    pub(super) source_count: u64,
    pub(super) has_events: bool,
    pub(super) partial: bool,
}

pub(super) fn read(
    ledger: &mut Ledger,
    range: DateRange,
    cancel: Arc<AtomicBool>,
) -> crate::ledger::Result<Summary> {
    let catalog = Catalog::bundled();
    let mut days: BTreeMap<NaiveDate, Accumulator> = BTreeMap::new();
    let mut models = Models::default();
    let mut overall = Accumulator::default();
    let mut has_events = false;
    let coverage = ledger.visit(
        range.start_ms,
        range.end_ms,
        cancel,
        |item| {
            has_events = true;
            let date = DateTime::<Utc>::from_timestamp_millis(item.at_ms)
                .map(|date| date.with_timezone(&Local).date_naive());
            if let Some(date) = date {
                days.entry(date).or_default().add(&item, catalog);
            }
            overall.add(&item, catalog);
            Ok(())
        },
        |item| {
            models.add(item, catalog);
            Ok(())
        },
    )?;
    let totals = overall.finish();
    let partial = totals.partial || coverage.source_flags != 0 || coverage.undated;
    let summary = Snapshot {
        range: Some(range),
        last_success_at_ms: coverage.meta.last_success_at_ms,
        pricing_version: catalog.version.to_string(),
        pricing_source: catalog.source.to_string(),
        has_history: coverage.meta.published_epoch > 0,
        totals,
        daily: days
            .into_iter()
            .map(|(date, total)| Day {
                date,
                totals: total.finish(),
            })
            .collect(),
        models: models.finish(),
        ..Snapshot::default()
    };
    Ok(Summary {
        snapshot: summary,
        source_count: coverage.source_count,
        has_events,
        partial,
    })
}
