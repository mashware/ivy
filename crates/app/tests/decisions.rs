//! The decisions record keeps its decisions in the order of their numbers.
//!
//! **Order is not tidiness here, it is what a paragraph belongs to.** The record has no markup for
//! where a decision ends: everything under a `## D<n>` heading, annexes included, reads as that
//! decision's content until the next one. So a decision written in the wrong place does not only
//! look out of order — whatever follows it up to the next heading is read as *its* reasoning, and the
//! annexes of the decision it interrupted are cut in two. It has happened twice: one decision sat in
//! the middle of another's annexes, and two neighbours swapped places. Both read fine from the table
//! of contents a reader builds in their head, which is why nobody saw them.
//!
//! **A gap and a repeat are the same fault seen from the numbers.** The code cites decisions by number
//! (D28, D32), so a number given twice makes every citation of it ambiguous, and a missing one is a
//! decision that was deleted without saying so — this project edits a decision, it does not remove it.

/// The record, read at compile time like the sibling guards read it.
const DECISIONS: &str = include_str!("../../../docs/02-decisiones.md");

/// Fewer headings than this means the walk stopped recognising them, not that the record shrank: a
/// record with none is in perfect order, and this guard would pass on it. It only catches a walk
/// that went blind to every heading; one heading written another way is [`misshapen`]'s job.
const HEADINGS_AT_LEAST: usize = 60;

/// Every line outside a fenced block, numbered from 1. What sits in a fence is an example of
/// Markdown, not part of the record.
fn outside_fences(md: &str) -> Vec<(usize, &str)> {
    let mut fence: Option<&str> = None;
    let mut lines = Vec::new();
    for (index, line) in md.lines().enumerate() {
        let trimmed = line.trim_start();
        for marker in ["```", "~~~"] {
            if trimmed.starts_with(marker) {
                fence = match fence {
                    None => Some(marker),
                    Some(open) if open == marker => None,
                    other => other,
                };
            }
        }
        if fence.is_none() {
            lines.push((index + 1, line));
        }
    }
    lines
}

/// The number of a line written exactly `## D<n> <title>`, or none.
fn heading_number(line: &str) -> Option<u32> {
    let rest = line.strip_prefix("## D")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || !rest[digits.len()..].starts_with(' ') {
        return None;
    }
    Some(digits.parse().expect("a run of digits is a number"))
}

/// The number of every `## D<n> — …` heading, in the order the document writes them, with its line.
fn decision_headings(md: &str) -> Vec<(usize, u32)> {
    outside_fences(md)
        .into_iter()
        .filter_map(|(line, text)| heading_number(text).map(|n| (line, n)))
        .collect()
}

/// Every heading that names a decision but is not written the way [`decision_headings`] reads it —
/// `## D61: …`, a non-breaking space, a third `#`, an indent Markdown still renders as a heading,
/// a space inside `D 61`. **Such a heading is invisible to the order check**, and when it is the
/// newest one the gap check cannot see it either, because nothing after it is missing.
fn misshapen(md: &str) -> Vec<String> {
    outside_fences(md)
        .into_iter()
        .filter(|&(_, text)| {
            let indented = text.trim_start_matches(char::is_whitespace);
            let unhashed = indented.trim_start_matches('#');
            let named = unhashed.trim_start_matches(char::is_whitespace);
            unhashed.len() < indented.len()
                && named.strip_prefix('D').is_some_and(|n| {
                    n.trim_start_matches(char::is_whitespace)
                        .starts_with(|c: char| c.is_ascii_digit())
                })
                && heading_number(text).is_none()
        })
        .map(|(line, text)| {
            format!("line {line} names a decision but is not `## D<n> — …`: {text:?}")
        })
        .collect()
}

/// Every way the sequence of headings breaks the record, one sentence each, or none.
///
/// The misplaced decisions are the ones outside a longest increasing run: that names the decision
/// that moved, not the five neighbours whose adjacency its move broke.
fn faults(headings: &[(usize, u32)]) -> Vec<String> {
    let numbers: Vec<u32> = headings.iter().map(|&(_, n)| n).collect();
    let mut faults = Vec::new();

    // Longest strictly increasing subsequence, quadratic: the record holds tens of headings.
    let mut length = vec![1usize; numbers.len()];
    let mut before = vec![None; numbers.len()];
    for i in 0..numbers.len() {
        for j in 0..i {
            if numbers[j] < numbers[i] && length[j] + 1 > length[i] {
                length[i] = length[j] + 1;
                before[i] = Some(j);
            }
        }
    }
    let mut kept = vec![false; numbers.len()];
    let mut cursor = (0..numbers.len()).max_by_key(|&i| (length[i], std::cmp::Reverse(i)));
    while let Some(i) = cursor {
        kept[i] = true;
        cursor = before[i];
    }
    for (i, &(line, n)) in headings.iter().enumerate() {
        if kept[i] {
            continue;
        }
        let name =
            |k: Option<usize>| k.map_or("the edge".to_string(), |k| format!("D{}", numbers[k]));
        let previous = i.checked_sub(1);
        let next = (i + 1 < numbers.len()).then_some(i + 1);
        faults.push(format!(
            "D{n} (line {line}) sits between {} and {}",
            name(previous),
            name(next)
        ));
    }

    let mut sorted = numbers.clone();
    sorted.sort_unstable();
    for pair in sorted.windows(2) {
        if pair[0] == pair[1] {
            faults.push(format!("D{} is written twice", pair[0]));
        }
    }
    let max = sorted.last().copied().unwrap_or(0);
    for missing in (1..=max).filter(|n| sorted.binary_search(n).is_err()) {
        faults.push(format!("D{missing} is missing, and D{max} exists"));
    }
    faults
}

#[test]
fn every_decision_comes_after_the_one_before_it() {
    let headings = decision_headings(DECISIONS);
    assert!(
        headings.len() >= HEADINGS_AT_LEAST,
        "the walk found {} `## D<n> — ` headings and there are at least {HEADINGS_AT_LEAST}: the \
         record writes them some other way now, and an empty sequence is always in order",
        headings.len()
    );
    let mut faults = faults(&headings);
    faults.extend(misshapen(DECISIONS));
    assert!(
        faults.is_empty(),
        "docs/02-decisiones.md is out of order — whatever follows a misplaced heading reads as that \
         decision's content:\n  {}",
        faults.join("\n  ")
    );
}

/// The oracle names the decision that moved, and not only on the record it happens to pass on.
#[test]
fn the_oracle_names_the_decision_that_moved() {
    let md = "## D1 — a\n## D2 — b\n## D5 — moved\n## D3 — c\n## D4 — d\n\
              ```\n## D9 — an example, not a decision\n```\n## D6 — e\n## D6 — again\n## D8 — f\n\
              ~~~\n## D9 — nor is this one\n```\n## D9 — a backtick line does not close a tilde fence\n~~~\n\
              ## Dx — no number, no decision\n";
    let found = faults(&decision_headings(md));
    assert_eq!(
        found,
        [
            "D5 (line 3) sits between D2 and D3",
            "D6 (line 10) sits between D6 and D8",
            "D6 is written twice",
            "D7 is missing, and D8 exists",
        ]
    );
    assert!(misshapen(md).is_empty(), "{:?}", misshapen(md));
}

/// A decision written in another form is named, including the newest, which no gap would betray.
#[test]
fn a_heading_in_another_form_is_named() {
    let md = "## D1 — a\n## D2: colon\n## D3\u{a0}— no-break space\n### D4 — one level down\n\
              ## D5x — letters after the number\n## Dx — no number\n## Decisions, not a decision\n\
              \x20## D7 — indented\n## D 8 — a space in the name\nD9 — prose, not a heading\n\
              ```\n## D6: inside a fence\n```\n";
    assert_eq!(
        misshapen(md),
        [
            "line 2 names a decision but is not `## D<n> — …`: \"## D2: colon\"",
            "line 3 names a decision but is not `## D<n> — …`: \"## D3\\u{a0}— no-break space\"",
            "line 4 names a decision but is not `## D<n> — …`: \"### D4 — one level down\"",
            "line 5 names a decision but is not `## D<n> — …`: \"## D5x — letters after the number\"",
            "line 8 names a decision but is not `## D<n> — …`: \" ## D7 — indented\"",
            "line 9 names a decision but is not `## D<n> — …`: \"## D 8 — a space in the name\"",
        ]
    );
}
