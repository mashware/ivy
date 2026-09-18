//! **A batch that declares a load must be invalidated in both directions, and this is what holds
//! it** — the rule **D13** states as «a mislabelled number is worse than no number».
//!
//! The measurement script reads a report line in both directions for each knob a batch can declare:
//! a batch that asks for buttons and drew none, and a batch that asks for none and drew some; the
//! same pair for the control channel, and the same pair for the strip over each console. Those pairs
//! existed only in the script, which nothing in this suite read, so a delivery that deleted half of
//! one would leave a batch publishing «this costs nothing» with every test green.
//!
//! **The strip's pair is the one that guards the OTHER runs, not its own.** A band over every console
//! takes a row off each of them, so a batch it leaked into publishes a grid that compares with none
//! of the eighteen already measured — and the report is otherwise complete and every other guard
//! quiet.
//!
//! # What it checks, and why it is not «the script contains this sentence»
//!
//! A guard that matched wording rots the first time somebody rephrases a message, and then it gets
//! deleted. What is checked is the **shape**: for every knob a batch can declare, the script has an
//! «if» on that knob being set **and** one on it not being set, and each of them invalidates the run.
//! Rewriting the messages leaves it alone; deleting a direction does not.

/// The script this guard reads, pulled in at compile time for the reason its siblings give: a walk
/// of the directory turns a renamed file into a target that is silently skipped, and a guard with
/// nothing to look at reads exactly like a clean one.
const MEASURE: &str = include_str!("../../../scripts/measure-frame.sh");

/// The knobs a batch declares, each with what the report line it is read against is called.
///
/// **A knob goes in here the moment a batch can set it**, which is the whole maintenance rule: the
/// list is short because every entry is a load that would otherwise be publishable as free.
const DECLARED: &[(&str, &str)] = &[
    ("ACTIONS", "buttons"),
    ("CONTROL", "control"),
    ("HEADER", "header"),
];

/// Whether the script has a conditional on this test that ends up invalidating the run.
///
/// It walks from the condition to the line that closes its block, which is what every one of
/// these blocks looks like — the script is checked by `sh -n` and formatted by hand in one style.
///
/// **Every occurrence is tried, not the first**, and that is not caution: a knob is read twice in
/// this script — once by the seeding, to decide what a batch publishes, and once by the
/// invalidation. Looking only at the first would test the seeding and report the guard missing,
/// which is exactly how the first version of this failed.
fn invalidates_on(condition: &str) -> bool {
    let lines: Vec<&str> = MEASURE.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(condition))
        .any(|(at, _)| {
            lines[at + 1..]
                .iter()
                .take_while(|line| line.trim() != "fi")
                .any(|line| line.contains("invalidate"))
        })
}

#[test]
fn every_declared_load_is_read_in_both_directions() {
    let mut missing = Vec::new();

    for (knob, _) in DECLARED {
        let set = format!("[ \"${{{knob}:-}}\" = 1 ]");
        let unset = format!("[ \"${{{knob}:-}}\" != 1 ]");
        if !invalidates_on(&set) {
            missing.push(format!(
                "{knob}: nothing invalidates a batch that declares it and measured none of it"
            ));
        }
        if !invalidates_on(&unset) {
            missing.push(format!(
                "{knob}: nothing invalidates a batch that declares none of it and measured some"
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "a batch could publish a load as costing nothing:\n  {}",
        missing.join("\n  ")
    );
}

/// **A knob the application reads by PRESENCE may never travel with an empty value**, and this is the
/// only one of them.
///
/// The knob at `header::HEADER_VARIABLE` is asked for with `env::var_os(..).is_some()`, so giving it
/// an empty value turns the strips **on**. Its five neighbours in the launch block go out as
/// `MEASURE_X="${X:-}"`, which is right for a knob read as `= 1` and catastrophic for this one: every
/// batch would paint a strip, every console would be a row per pane shorter, and the whole published
/// series would stop being comparable with runs that have none — with no guard firing, because the
/// reports would all agree with each other. The first version of the launch block was written that
/// way and it was caught by reading, not by a test. This is the test.
#[test]
fn the_strips_knob_is_exported_by_presence_and_never_with_an_empty_value() {
    // **The comments are skipped, and that is not a detail**: the launch block explains this very
    // trap in prose, quoting the shape it must not have, so a search over the whole file finds it
    // there and fails on the explanation instead of on the code.
    let code = || {
        MEASURE
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
    };
    assert!(
        code().all(|line| !line.contains("MEASURE_HEADER=\"")),
        "the strips' knob is exported with a value that may be empty, and an empty one switches it \
         ON: every batch would paint a strip and the published series would stop being comparable"
    );
    assert!(
        code().any(|line| line.contains("export MEASURE_HEADER=1")),
        "no batch can ask for the strips: the knob is never exported set"
    );
    assert!(
        code().any(|line| line.contains("unset MEASURE_HEADER")),
        "a batch that does not ask for the strips inherits whatever the person measuring had \
         exported by hand, and opens with a row less per console than the batch it is compared with"
    );
}

/// Every batch a report is taken from has to **seed a panel**, or its witness never appears and the
/// run is invalidated for a reason that has nothing to do with what it measures.
///
/// **It is the first thing that went wrong with batch `T`.** It was added to the table of batches
/// and to the launch block and not to the seeding `case`, so every one of its runs died on «nobody
/// got to publish a panel» — a batch that cannot produce one valid report, with the suite green and
/// the script's own guard doing its job for the wrong reason. `A` is the one batch that seeds
/// nothing on purpose (no panel at all) and `B` seeds a quiet one.
#[test]
fn every_batch_that_measures_a_panel_seeds_one() {
    let seeded: Vec<&str> = MEASURE
        .lines()
        .filter(|line| line.contains("seed talkative") || line.contains("seed quiet"))
        .collect();
    let declared: Vec<char> = MEASURE
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix(char::is_alphabetic)
                .and_then(|rest| {
                    rest.starts_with(")       TABS=")
                        .then(|| line.trim().chars().next())
                        .flatten()
                })
        })
        .collect();

    assert!(
        declared.len() >= 14,
        "the batch table is not being read: {} letters found",
        declared.len()
    );
    for batch in declared {
        assert!(
            seeded.iter().any(|line| line.contains(batch)),
            "batch {batch} declares a layout and seeds no panel: every one of its runs is \
             invalidated for «nobody got to publish a panel», whatever it was measuring"
        );
    }
}

/// The other half: the line each knob is read against has to **exist in the report**, and it is the
/// application that writes it. Without this, deleting the line from the report would leave the
/// script's two directions reading an empty string — both of them satisfied, and the guard above
/// green over a report that says nothing.
#[test]
fn every_line_the_script_reads_is_one_the_application_writes() {
    const HEADER: &str = include_str!("../src/measure/header.rs");

    for (knob, line) in DECLARED {
        assert!(
            MEASURE.contains(&format!("grep -E \"^{line}:\"")),
            "{knob}: the script does not read a «{line}:» line out of the report"
        );
        assert!(
            HEADER.contains(&format!("\"{line}:")),
            "{knob}: the report does not write a «{line}:» line for the script to read"
        );
    }
}

/// **A knob no batch ever sets makes both of its directions vacuous**, and neither guard above can
/// see it: the pair of «if»s is still in the script, the report still writes its line, and the load
/// they protect is simply never asked for.
///
/// It is the shape batch `T` takes the moment its row of the table loses `HEADER=1`. The run goes
/// out under the name of the batch that measures what a strip costs, paints none, invalidates
/// nothing — the direction that fires is the one for a batch that did **not** ask, and it is
/// satisfied — and publishes a second control for `C` as the measurement of a load nobody
/// performed. That is **D13**'s «a mislabelled number is worse than no number», with every test in
/// this file green. The same hole is open for every other knob, so it is checked over the list.
#[test]
fn every_declared_knob_is_asked_for_by_some_batch() {
    // The rows of the batch table, which is where a batch declares its load: a letter, the case
    // arm's parenthesis and the layout. Written out here rather than shared with the seeding test
    // next door, because each of them is reading the table for a different reason.
    let rows = || {
        MEASURE.lines().map(str::trim).filter(|line| {
            line.strip_prefix(char::is_alphabetic)
                .is_some_and(|rest| rest.starts_with(")       TABS="))
        })
    };
    assert!(
        rows().count() >= 14,
        "the batch table is not being read: {} rows found, so the loop below would report every \
         knob missing",
        rows().count()
    );

    for (knob, line) in DECLARED {
        assert!(
            rows().any(|row| row.contains(&format!("{knob}=1"))),
            "no batch asks for «{knob}»: its two guards can never fire, and whichever batch was \
             meant to measure that load publishes the «{line}:» line of a run that did nothing"
        );
    }
}
