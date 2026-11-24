//! Reader projection: declarations decide placement; filtering never decides milestone state.
use super::{Columns, progress};
use crate::board::{filter, model::Entry, observation::Column};
use panel_contract::{
    Mark,
    plan::{Milestone, Plan},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::board) enum Class {
    Claim(Column),
    Active,
    Done,
    Ready,
    Later,
}
impl Class {
    pub(super) fn active(self) -> bool {
        matches!(self, Self::Claim(_) | Self::Active)
    }
}

pub(in crate::board) fn claim(entries: impl IntoIterator<Item = Column>) -> Option<Column> {
    entries
        .into_iter()
        .filter(|c| filter::claims(*c))
        .min_by_key(|c| match c {
            Column::Wait => 0,
            Column::Block => 1,
            _ => 2,
        })
}

pub(super) struct Projection {
    pub(super) classes: Vec<Class>,
    pub(super) missing: Vec<Vec<String>>,
    pub(super) diagnostics: Vec<String>,
}

impl Projection {
    pub(super) fn new(
        plan: &Plan,
        placed: &Columns<'_>,
        column: &impl Fn(&Entry) -> Column,
    ) -> Self {
        let complete: Vec<_> = plan
            .milestones
            .iter()
            .map(|m| {
                let (a, b) = progress(m);
                a == b
            })
            .collect();
        let cycles = cycle_dependents(plan);
        let mut diagnostics = Vec::new();
        let mut missing = Vec::new();
        let classes = plan
            .milestones
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let entries: Vec<_> = placed.milestones[i]
                    .loose
                    .iter()
                    .copied()
                    .chain(placed.milestones[i].items.iter().flatten().copied())
                    .collect();
                let waits: Vec<_> = m
                    .depends_on
                    .iter()
                    .filter_map(|id| {
                        let target = plan
                            .milestones
                            .iter()
                            .position(|m| m.id.as_ref() == Some(id));
                        let cycle = target.is_some_and(|target| cycles[target]);
                        if target.is_none() || cycle {
                            diagnostics.push(format!(
                                "unmet roadmap dependency {id}: {}",
                                if cycle { "cycle" } else { "unknown id" }
                            ));
                        }
                        if target.is_none_or(|j| !complete[j]) || cycle {
                            Some(target.map_or_else(
                                || id.clone(),
                                |j| {
                                    let (a, b) = progress(&plan.milestones[j]);
                                    format!("{id} ({a}/{b})")
                                },
                            ))
                        } else {
                            None
                        }
                    })
                    .collect();
                let ready = waits.is_empty();
                missing.push(waits);
                if complete[i] {
                    Class::Done
                } else if !entries.is_empty()
                    || m.items.iter().any(|i| i.mark == Some(Mark::Current))
                {
                    claim(entries.into_iter().map(column)).map_or(Class::Active, Class::Claim)
                } else if ready {
                    Class::Ready
                } else {
                    Class::Later
                }
            })
            .collect();
        diagnostics.sort();
        diagnostics.dedup();
        Self {
            classes,
            missing,
            diagnostics,
        }
    }
}

// Peel acyclic leaves once; remaining nodes are in a cycle or depend on one.
fn cycle_dependents(plan: &Plan) -> Vec<bool> {
    let mut ids = std::collections::HashMap::new();
    for (i, m) in plan.milestones.iter().enumerate() {
        if let Some(id) = &m.id {
            ids.entry(id).or_insert(i);
        }
    }
    let mut counts = vec![0; plan.milestones.len()];
    let mut parents = vec![Vec::new(); counts.len()];
    for (i, m) in plan.milestones.iter().enumerate() {
        for id in &m.depends_on {
            if let Some(&j) = ids.get(id) {
                counts[i] += 1;
                parents[j].push(i);
            }
        }
    }
    let mut leaves: Vec<_> = counts
        .iter()
        .enumerate()
        .filter(|(_, n)| **n == 0)
        .map(|(i, _)| i)
        .collect();
    while let Some(i) = leaves.pop() {
        for &p in &parents[i] {
            counts[p] -= 1;
            if counts[p] == 0 {
                leaves.push(p);
            }
        }
    }
    counts.into_iter().map(|n| n > 0).collect()
}

pub(super) fn visible(m: &Milestone, attached: &[bool], full: bool, collapsed: bool) -> Vec<usize> {
    visible_items(&m.items, attached, full, collapsed)
}

pub(super) fn visible_items(
    items: &[panel_contract::plan::Item],
    attached: &[bool],
    full: bool,
    collapsed: bool,
) -> Vec<usize> {
    if full {
        return (0..items.len()).collect();
    }
    if collapsed {
        return items
            .iter()
            .position(|i| !matches!(i.mark, Some(Mark::Done)))
            .into_iter()
            .collect();
    }
    let relevant = |i: usize| {
        attached.get(i).copied().unwrap_or(false)
            || matches!(
                items[i].mark,
                Some(Mark::Current | Mark::Wait | Mark::Block)
            )
    };
    let last = (0..items.len()).rev().find(|&i| relevant(i));
    let mut pending = 0;
    (0..items.len())
        .filter(|&i| {
            if relevant(i) {
                return true;
            }
            if last.is_none_or(|last| i > last) && !matches!(items[i].mark, Some(Mark::Done)) {
                pending += 1;
                return pending <= 2;
            }
            false
        })
        .collect()
}

/// Fixed collapsed columns; expanded columns share the surplus in proportion to their weight.
pub(super) fn widths(available: f32, columns: &[(bool, bool)], separators: usize) -> Vec<f32> {
    let gaps = columns.len().saturating_sub(1) as f32 * 20.0 + separators as f32 * 24.0;
    let minimum: f32 = columns
        .iter()
        .map(|(collapsed, _)| if *collapsed { 190.0 } else { 240.0 })
        .sum();
    let surplus = (available - gaps - minimum).max(0.0);
    let weights: f32 = columns
        .iter()
        .map(|(collapsed, card)| {
            if *collapsed {
                0.0
            } else if *card {
                1.2
            } else {
                1.0
            }
        })
        .sum();
    columns
        .iter()
        .map(|(collapsed, card)| {
            if *collapsed {
                190.0
            } else {
                240.0 + surplus * (if *card { 1.2 } else { 1.0 }) / weights.max(1.0)
            }
        })
        .collect()
}
