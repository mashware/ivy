#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(i64)]
pub enum Provider {
    #[default]
    Claude,
    Codex,
}

impl Provider {
    pub(crate) fn from_row(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Self> {
        match row.get::<_, i64>(index)? {
            0 => Ok(Self::Claude),
            1 => Ok(Self::Codex),
            _ => Err(rusqlite::Error::IntegralValueOutOfRange(
                index,
                row.get(index)?,
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Counts {
    pub uncached_input: Option<i64>,
    pub cache_read: Option<i64>,
    pub cache_write: Option<i64>,
    pub cache_write_1h: Option<i64>,
    pub output: Option<i64>,
    pub reasoning_output: Option<i64>,
    pub flags: i64,
}

impl Counts {
    pub(crate) const INPUT_INVALID: i64 = 1;
    pub(crate) const READ_INVALID: i64 = 1 << 1;
    pub(crate) const WRITE_INVALID: i64 = 1 << 2;
    pub(crate) const DURATION_INVALID: i64 = 1 << 3;
    pub(crate) const OUTPUT_INVALID: i64 = 1 << 4;

    pub(crate) fn ordered(self) -> [Option<i64>; 5] {
        [
            self.uncached_input,
            self.cache_read,
            self.cache_write,
            self.cache_write_1h,
            self.output,
        ]
    }

    pub(crate) fn is_complete(self) -> bool {
        self.flags == 0
            && self
                .token_categories()
                .into_iter()
                .all(|count| count.is_some())
    }

    pub(crate) fn known_tokens(self) -> Option<i128> {
        self.token_categories()
            .into_iter()
            .flatten()
            .map(i128::from)
            .reduce(|total, count| total + count)
    }

    fn token_categories(self) -> [Option<i64>; 4] {
        [
            self.uncached_input,
            self.cache_read,
            self.cache_write,
            self.output,
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Observation {
    pub provider: Provider,
    pub event_key: [u8; 32],
    pub source_key: [u8; 32],
    pub evidence_key: [u8; 32],
    pub at_ms: Option<i64>,
    pub model: Option<String>,
    pub model_key: Option<[u8; 32]>,
    pub counts: Counts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i64)]
pub(crate) enum Resolution {
    Resolved = 0,
    Conflicted = 1,
}

impl TryFrom<i64> for Resolution {
    type Error = ();

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Resolved),
            1 => Ok(Self::Conflicted),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Canonical {
    pub provider: Provider,
    pub at_ms: Option<i64>,
    pub model: Option<String>,
    pub counts: Counts,
    pub resolution: Resolution,
}

#[derive(Default)]
pub(crate) struct Reducer {
    provider: Provider,
    counts: Option<Counts>,
    flags: i64,
    model: Option<String>,
    model_key: Option<[u8; 32]>,
    at_ms: Option<i64>,
    conflicted: bool,
}

impl Reducer {
    pub(crate) fn push_sorted(&mut self, observation: Observation) {
        if self.counts.is_some() && self.provider != observation.provider {
            self.conflicted = true;
            return;
        }
        self.provider = observation.provider;
        if self.provider == Provider::Codex {
            if let Some(counts) = &mut self.counts {
                let next = observation.counts;
                for (known, new) in [
                    (&mut counts.uncached_input, next.uncached_input),
                    (&mut counts.cache_read, next.cache_read),
                    (&mut counts.cache_write, next.cache_write),
                    (&mut counts.cache_write_1h, next.cache_write_1h),
                    (&mut counts.output, next.output),
                    (&mut counts.reasoning_output, next.reasoning_output),
                ] {
                    match (*known, new) {
                        (Some(a), Some(b)) => self.conflicted |= a != b,
                        (None, Some(b)) => *known = Some(b),
                        _ => {}
                    }
                }
                self.conflicted |= counts
                    .reasoning_output
                    .zip(counts.output)
                    .is_some_and(|(subset, total)| subset > total);
            } else {
                self.counts = Some(observation.counts);
            }
            self.conflicted |= self
                .at_ms
                .zip(observation.at_ms)
                .is_some_and(|(a, b)| a != b);
        } else {
            let current = observation.counts.ordered();
            if let Some(previous) = self.counts {
                let previous = previous.ordered();
                debug_assert!(previous <= current);
                // In lexicographic order a component-wise chain exists exactly when all
                // adjacent tuples are ordered component-wise. A later common maximum
                // cannot hide an earlier pair of contradictory snapshots.
                self.conflicted |= previous
                    .into_iter()
                    .zip(current)
                    .any(|(old, new)| old > new);
            }
            self.counts = Some(observation.counts);
        }
        self.flags |= observation.counts.flags;

        if let Some(model_key) = observation.model_key {
            self.conflicted |= self.model_key.is_some_and(|old| old != model_key);
            self.model_key = Some(model_key);
        }
        if let Some(model) = observation.model {
            self.model = Some(model);
        }
        if let Some(at_ms) = observation.at_ms {
            self.at_ms = Some(self.at_ms.map_or(at_ms, |earliest| earliest.min(at_ms)));
        }
    }

    pub(crate) fn finish(self) -> Option<Canonical> {
        let mut counts = self.counts?;
        let resolution = if self.conflicted {
            counts = Counts::default();
            Resolution::Conflicted
        } else {
            Resolution::Resolved
        };
        counts.flags |= self.flags;
        Some(Canonical {
            provider: self.provider,
            at_ms: self.at_ms,
            model: if self.conflicted { None } else { self.model },
            counts,
            resolution,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(input: i64, output: i64, at_ms: i64) -> Observation {
        Observation {
            provider: Provider::Claude,
            event_key: [1; 32],
            source_key: [2; 32],
            evidence_key: [3; 32],
            at_ms: Some(at_ms),
            model: Some("claude-sonnet-4-6".to_owned()),
            model_key: Some([4; 32]),
            counts: Counts {
                uncached_input: Some(input),
                cache_read: Some(30),
                cache_write: Some(10),
                cache_write_1h: Some(0),
                output: Some(output),
                ..Counts::default()
            },
        }
    }

    fn reduce(mut input: Vec<Observation>) -> Canonical {
        input.sort_by_key(|item| item.counts.ordered());
        let mut reducer = Reducer::default();
        for item in input {
            reducer.push_sorted(item);
        }
        reducer.finish().unwrap()
    }

    #[test]
    fn replay_and_later_streaming_timestamp_count_one_event() {
        let final_observation = observation(100, 20, 1_000);
        let original = observation(100, 5, 999);
        let canonical = reduce(vec![final_observation.clone(), original, final_observation]);
        assert_eq!(canonical.at_ms, Some(999));
        assert_eq!(canonical.counts.known_tokens(), Some(160));
        assert_eq!(canonical.resolution, Resolution::Resolved);
        assert!(canonical.counts.is_complete());
    }

    #[test]
    fn common_maximum_cannot_hide_a_conflict_in_any_order() {
        let input = [
            observation(100, 20, 100),
            observation(110, 10, 101),
            observation(110, 20, 102),
        ];
        for [i, j, k] in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let canonical = reduce(vec![input[i].clone(), input[j].clone(), input[k].clone()]);
            assert_eq!(canonical.resolution, Resolution::Conflicted);
            assert_eq!(canonical.counts.known_tokens(), None);
            assert_eq!(canonical.at_ms, Some(100));
        }
    }

    #[test]
    fn different_unknown_model_digests_conflict() {
        let mut a = observation(100, 20, 100);
        let mut b = a.clone();
        a.model = None;
        b.model = None;
        b.model_key = Some([5; 32]);
        assert_eq!(reduce(vec![a, b]).resolution, Resolution::Conflicted);
    }
}
