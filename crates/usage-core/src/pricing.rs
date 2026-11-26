use crate::accounting::Counts;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

const ATTODOLLARS_PER_DOLLAR: i128 = 1_000_000_000_000_000_000;
const ATTODOLLARS_PER_TOKEN_PER_MICRODOLLAR_PER_MTOK: i128 = 1_000_000;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Price(i128);

impl Price {
    pub fn attodollars(self) -> i128 {
        self.0
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let units = self.0 / ATTODOLLARS_PER_DOLLAR;
        let decimals = self.0 % ATTODOLLARS_PER_DOLLAR;
        if decimals == 0 {
            return write!(f, "{units}");
        }
        let fraction = format!("{decimals:018}");
        write!(f, "{units}.{}", fraction.trim_end_matches('0'))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Estimate {
    Unavailable,
    Partial(Price),
    Complete(Price),
}

#[derive(Debug, Default)]
pub(crate) struct EstimateTotal {
    known: Option<Price>,
    incomplete: bool,
    overflowed: bool,
}

impl EstimateTotal {
    pub(crate) fn add(&mut self, estimate: Estimate) {
        self.incomplete |= !matches!(estimate, Estimate::Complete(_));
        if let Estimate::Partial(price) | Estimate::Complete(price) = estimate {
            let total = self.known.map_or(0, Price::attodollars);
            if let Some(amount) = total.checked_add(price.attodollars()) {
                self.known = Some(Price(amount));
            } else {
                self.overflowed = true;
            }
        }
    }

    pub(crate) fn estimate(&self) -> Estimate {
        match (self.overflowed, self.known, self.incomplete) {
            (false, Some(price), false) => Estimate::Complete(price),
            (false, Some(price), true) => Estimate::Partial(price),
            _ => Estimate::Unavailable,
        }
    }
}

#[derive(Deserialize)]
struct Rates {
    input: u64,
    cache_read: u64,
    cache_write_5m: u64,
    cache_write_1h: u64,
    output: u64,
}

impl Rates {
    fn estimate(&self, counts: &Counts) -> Estimate {
        let writes = match (counts.cache_write, counts.cache_write_1h) {
            (Some(0), None | Some(0)) => (Some(0), Some(0)),
            (Some(total), Some(hour)) if hour <= total => (Some(total - hour), Some(hour)),
            _ => (None, None),
        };
        let mut amount = 0i128;
        let mut known = false;
        let mut incomplete = counts.flags != 0;
        for (count, rate) in [
            (counts.uncached_input, self.input),
            (counts.cache_read, self.cache_read),
            (writes.0, self.cache_write_5m),
            (writes.1, self.cache_write_1h),
            (counts.output, self.output),
        ] {
            if let Some(count) = count {
                known = true;
                let contribution =
                    i128::from(count)
                        .checked_mul(i128::from(rate))
                        .and_then(|value| {
                            value.checked_mul(ATTODOLLARS_PER_TOKEN_PER_MICRODOLLAR_PER_MTOK)
                        });
                let Some(sum) = contribution.and_then(|value| amount.checked_add(value)) else {
                    return Estimate::Unavailable;
                };
                amount = sum;
            } else {
                incomplete = true;
            }
        }

        match (known, incomplete) {
            (false, _) => Estimate::Unavailable,
            (true, true) => Estimate::Partial(Price(amount)),
            (true, false) => Estimate::Complete(Price(amount)),
        }
    }
}

#[derive(Deserialize)]
pub struct Catalog {
    pub version: String,
    pub source: String,
    models: BTreeMap<String, Option<Rates>>,
}

impl Catalog {
    pub fn bundled() -> &'static Self {
        static BUNDLED: OnceLock<Catalog> = OnceLock::new();
        BUNDLED.get_or_init(|| {
            serde_json::from_str(include_str!("claude_prices.json"))
                .expect("bundled Claude pricing catalog")
        })
    }

    pub(crate) fn known_model(&self, id: &str) -> bool {
        self.models.contains_key(id)
    }

    pub(crate) fn estimate(&self, model: Option<&str>, counts: &Counts) -> Estimate {
        model
            .and_then(|id| self.models.get(id))
            .and_then(Option::as_ref)
            .map_or(Estimate::Unavailable, |rates| rates.estimate(counts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn witness() -> Counts {
        Counts {
            uncached_input: Some(100),
            cache_read: Some(30),
            cache_write: Some(10),
            cache_write_1h: Some(0),
            output: Some(20),
            flags: 0,
        }
    }

    fn fixture_rates() -> Rates {
        Rates {
            input: 2_000_000,
            cache_read: 200_000,
            cache_write_5m: 2_500_000,
            cache_write_1h: 4_000_000,
            output: 10_000_000,
        }
    }

    #[test]
    fn fixture_prices_to_431_millionths_before_rounding() {
        let Estimate::Complete(price) = fixture_rates().estimate(&witness()) else {
            panic!("expected complete price");
        };
        assert_eq!(price.attodollars(), 431_000_000_000_000);
        assert_eq!(price.to_string(), "0.000431");
    }

    #[test]
    fn one_hour_writes_are_a_subset_with_a_distinct_rate() {
        let counts = Counts {
            cache_write_1h: Some(4),
            ..witness()
        };
        assert_eq!(counts.known_tokens(), Some(160));
        assert_eq!(
            fixture_rates().estimate(&counts),
            Estimate::Complete(Price(437_000_000_000_000)),
        );
    }

    #[test]
    fn unknown_duration_reports_only_the_known_subtotal() {
        let counts = Counts {
            cache_write_1h: None,
            ..witness()
        };
        assert_eq!(
            fixture_rates().estimate(&counts),
            Estimate::Partial(Price(406_000_000_000_000)),
        );
    }

    #[test]
    fn priced_zero_is_distinct_from_unavailable_and_aggregates_stay_partial() {
        let complete_zero = Counts {
            uncached_input: Some(0),
            cache_read: Some(0),
            cache_write: Some(0),
            cache_write_1h: None,
            output: Some(0),
            flags: 0,
        };
        let catalog = Catalog::bundled();
        let priced = catalog.estimate(Some("claude-sonnet-4-6"), &complete_zero);
        assert_eq!(priced, Estimate::Complete(Price(0)));
        assert_eq!(
            catalog.estimate(Some("unknown-model"), &complete_zero),
            Estimate::Unavailable
        );
        assert_eq!(
            catalog.estimate(Some("claude-haiku-5-5"), &witness()),
            Estimate::Unavailable
        );
        assert!(catalog.known_model("claude-haiku-5-5"));
        let mut sum = EstimateTotal::default();
        sum.add(priced);
        sum.add(Estimate::Unavailable);
        assert_eq!(sum.estimate(), Estimate::Partial(Price(0)));
    }
}
