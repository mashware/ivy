use super::{Bucket, Cost, Day, Granularity, Model, Provider, Request, Snapshot, Totals};
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
        self.totals.reasoning_output_known |= counts.reasoning_output.is_some();
        self.totals.reasoning_output += counts.reasoning_output.unwrap_or_default() as u128;
        self.totals.output += counts.output.unwrap_or_default() as u128;
        self.estimates.add(match item.provider {
            Provider::Claude => catalog.estimate(item.model.as_deref(), counts),
            Provider::Codex => Estimate::Unavailable,
        });
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
    provider: Provider,
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
                provider: self.provider,
                label: label.unwrap_or_else(|| "Unknown model".into()),
                totals: totals.finish(),
            });
        }
    }

    fn finish(mut self) -> Vec<Model> {
        self.flush();
        if self.has_remainder {
            self.visible.push(Model {
                provider: self.provider,
                label: "Other models".into(),
                totals: self.remainder.finish(),
            });
        }
        self.visible
    }
}

pub(super) struct Summary {
    pub(super) snapshot: Snapshot,
    pub(super) source_counts: [u64; 2],
    pub(super) has_events: bool,
    pub(super) partial: bool,
}

pub(super) fn read(
    ledger: &mut Ledger,
    request: &Request,
    cancel: Arc<AtomicBool>,
) -> crate::ledger::Result<Summary> {
    let range = request.range;
    let catalog = Catalog::bundled();
    let mut days: BTreeMap<NaiveDate, Accumulator> = BTreeMap::new();
    let mut claude_models = Models::default();
    let mut codex_models = Models {
        provider: Provider::Codex,
        ..Models::default()
    };
    let mut buckets = BTreeMap::<(chrono::NaiveDate, u32, i32), (String, Accumulator)>::new();
    let matches_provider =
        |item: &Contribution| request.provider.is_none_or(|p| p == item.provider);
    let mut overall = Accumulator::default();
    let mut has_events = false;
    let coverage = ledger.visit(
        range.start_ms,
        range.end_ms,
        cancel,
        |item| {
            if !matches_provider(&item)
                || request
                    .model
                    .as_ref()
                    .is_some_and(|m| item.model.as_deref().unwrap_or("Unknown model") != m)
            {
                return Ok(());
            }
            has_events = true;
            if let Some(at) = DateTime::<Utc>::from_timestamp_millis(item.at_ms) {
                let local = at.with_timezone(&Local).fixed_offset();
                if request.granularity == Granularity::Day {
                    days.entry(bucket_key(local, Granularity::Day).0)
                        .or_default()
                        .add(&item, catalog);
                } else {
                    let key = bucket_key(local, request.granularity);
                    buckets
                        .entry(key)
                        .or_insert_with(|| {
                            let label = match request.granularity {
                                Granularity::Hour => local.format("%Y-%m-%d %H:00 %:z").to_string(),
                                Granularity::Month => local.format("%Y-%m").to_string(),
                                Granularity::Day => unreachable!("days use the calendar trend"),
                            };
                            (label, Accumulator::default())
                        })
                        .1
                        .add(&item, catalog);
                }
            }
            overall.add(&item, catalog);
            Ok(())
        },
        |item| {
            if matches_provider(&item)
                && request
                    .model
                    .as_ref()
                    .is_none_or(|model| item.model.as_deref().unwrap_or("Unknown model") == model)
            {
                match item.provider {
                    Provider::Claude => claude_models.add(item, catalog),
                    Provider::Codex => codex_models.add(item, catalog),
                }
            }
            Ok(())
        },
    )?;
    let totals = overall.finish();
    let (source_flags, undated) =
        request
            .provider
            .map_or((coverage.source_flags, coverage.undated), |provider| {
                let scoped = coverage.for_provider(provider);
                (scoped.source_flags, scoped.undated)
            });
    let partial = totals.partial || source_flags != 0 || undated;
    let summary = Snapshot {
        range: Some(range),
        provider: request.provider,
        model_filter: request.model.clone(),
        granularity: request.granularity,
        buckets: buckets
            .into_values()
            .map(|(label, value)| Bucket {
                label,
                totals: value.finish(),
            })
            .collect(),
        codex_last_success_at_ms: coverage.meta.codex_last_success_at_ms,
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
        models: claude_models
            .finish()
            .into_iter()
            .chain(codex_models.finish())
            .collect(),
        ..Snapshot::default()
    };
    Ok(Summary {
        snapshot: summary,
        source_counts: [
            coverage.for_provider(Provider::Claude).source_count,
            coverage.for_provider(Provider::Codex).source_count,
        ],
        has_events,
        partial,
    })
}

fn bucket_key(
    local: DateTime<chrono::FixedOffset>,
    granularity: Granularity,
) -> (NaiveDate, u32, i32) {
    use chrono::{Datelike, Timelike};
    let date = local.date_naive();
    match granularity {
        Granularity::Hour => (date, local.hour(), -local.offset().local_minus_utc()),
        Granularity::Day => (date, 0, 0),
        Granularity::Month => (date.with_day(1).expect("valid month"), 0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_periods_keep_repeated_hours_and_month_boundaries_distinct() {
        let time = |text| DateTime::parse_from_rfc3339(text).unwrap();
        let early = time("2026-11-01T01:30:00-04:00");
        let late = time("2026-11-01T01:30:00-05:00");
        assert!(bucket_key(early, Granularity::Hour) < bucket_key(late, Granularity::Hour));
        assert_eq!(
            bucket_key(early, Granularity::Day),
            bucket_key(late, Granularity::Day)
        );
        let october = time("2026-10-31T23:59:00-04:00");
        assert!(bucket_key(october, Granularity::Month) < bucket_key(early, Granularity::Month));
    }
}
