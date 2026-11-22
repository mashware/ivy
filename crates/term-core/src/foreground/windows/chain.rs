//! Bounded, revalidated ancestry walk shared by the native reader and portable tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub pid: u32,
    pub birth: u64,
    pub sequence: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Inside,
    Outside,
    Unknown,
}

pub(super) fn walk<N>(
    first: N,
    root: Identity,
    anchor: Option<Identity>,
    mut identity: impl FnMut(&N) -> Option<Identity>,
    mut parent: impl FnMut(&N) -> Option<u32>,
    mut open: impl FnMut(u32) -> Option<N>,
) -> Outcome {
    let mut nodes: Vec<(N, Identity, Option<u32>)> = Vec::new();
    let mut current = first;
    let result = loop {
        if nodes.len() == 64 {
            return Outcome::Unknown;
        }
        let Some(id) = identity(&current) else {
            return Outcome::Unknown;
        };
        if id.pid == 0 {
            return Outcome::Unknown;
        }
        if nodes.iter().any(|(_, seen, _)| seen.pid == id.pid)
            || nodes
                .last()
                .is_some_and(|(_, child, _)| id.sequence >= child.sequence)
        {
            return Outcome::Unknown;
        }
        if id == root {
            nodes.push((current, id, None));
            break Outcome::Inside;
        }
        if anchor == Some(id) {
            nodes.push((current, id, None));
            break Outcome::Outside;
        }
        let Some(ppid) = parent(&current) else {
            return Outcome::Unknown;
        };
        if ppid == id.pid {
            return Outcome::Unknown;
        }
        nodes.push((current, id, Some(ppid)));
        if ppid == 0 {
            break Outcome::Outside;
        }
        let Some(next) = open(ppid) else {
            return Outcome::Unknown;
        };
        current = next;
    };
    for (node, id, previous_parent) in &nodes {
        if identity(node) != Some(*id) || previous_parent.is_some_and(|p| parent(node) != Some(p)) {
            return Outcome::Unknown;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn id(pid: u32, birth: u64) -> Identity {
        Identity {
            pid,
            birth,
            sequence: birth,
        }
    }
    fn run(edges: &[(u32, u64, u32)], root: Identity, anchor: Option<Identity>) -> Outcome {
        walk(
            edges[0].0,
            root,
            anchor,
            |pid| edges.iter().find(|e| e.0 == *pid).map(|e| id(e.0, e.1)),
            |pid| edges.iter().find(|e| e.0 == *pid).map(|e| e.2),
            |pid| edges.iter().any(|e| e.0 == pid).then_some(pid),
        )
    }
    #[test]
    fn windows_chain_bounds_cycles_and_missing_history() {
        assert_eq!(
            run(&[(3, 30, 2), (2, 20, 1), (1, 10, 0)], id(1, 10), None),
            Outcome::Inside
        );
        assert_eq!(
            run(
                &[(3, 30, 2), (2, 20, 1), (1, 10, 0)],
                id(9, 1),
                Some(id(1, 10))
            ),
            Outcome::Outside
        );
        assert_eq!(run(&[(3, 30, 2)], id(1, 10), None), Outcome::Unknown);
        assert_eq!(
            run(&[(3, 30, 2), (2, 20, 3)], id(1, 10), None),
            Outcome::Unknown
        );
        assert_eq!(
            run(&[(3, 30, 2), (2, 30, 0)], id(1, 10), None),
            Outcome::Unknown
        );
        let edges: Vec<_> = (1..=65).rev().map(|n| (n, n as u64, n - 1)).collect();
        assert_eq!(run(&edges, id(1, 1), None), Outcome::Unknown);
    }
    #[test]
    fn windows_anchor_is_an_outside_boundary_without_reading_its_history() {
        let edges = [(3, 30, 2), (2, 20, 999)];
        assert_eq!(run(&edges, id(9, 1), Some(id(2, 20))), Outcome::Outside);
        assert_eq!(run(&edges, id(9, 1), None), Outcome::Unknown);
    }

    #[test]
    fn later_reused_parent_is_unknown_despite_earlier_wall_clock_birth() {
        let records = [(3, 100, 20, 2), (2, 50, 30, 1), (1, 10, 10, 0)];
        let identity = |pid: &u32| {
            records.iter().find(|r| r.0 == *pid).map(|r| Identity {
                pid: r.0,
                birth: r.1,
                sequence: r.2,
            })
        };
        assert_eq!(
            walk(
                3,
                identity(&1).unwrap(),
                None,
                identity,
                |pid| records.iter().find(|r| r.0 == *pid).map(|r| r.3),
                Some
            ),
            Outcome::Unknown
        );
    }

    #[test]
    fn windows_chain_rechecks_identity_and_edges() {
        let reads = Cell::new(0);
        let outcome = walk(
            2,
            id(1, 1),
            None,
            |pid| {
                reads.set(reads.get() + 1);
                Some(id(
                    *pid,
                    if reads.get() > 2 && *pid == 2 {
                        9
                    } else {
                        *pid as u64
                    },
                ))
            },
            |_| Some(1),
            Some,
        );
        assert_eq!(outcome, Outcome::Unknown);
        let live_reads = Cell::new(0);
        let outcome = walk(
            2,
            id(1, 1),
            None,
            |pid| {
                live_reads.set(live_reads.get() + 1);
                if live_reads.get() > 2 && *pid == 2 {
                    None
                } else {
                    Some(id(*pid, *pid as u64))
                }
            },
            |_| Some(1),
            Some,
        );
        assert_eq!(outcome, Outcome::Unknown);
        let reads = Cell::new(0);
        let outcome = walk(
            2,
            id(1, 1),
            None,
            |pid| Some(id(*pid, *pid as u64)),
            |_| {
                reads.set(reads.get() + 1);
                Some(if reads.get() == 1 { 1 } else { 0 })
            },
            Some,
        );
        assert_eq!(outcome, Outcome::Unknown);
    }
}
