//! **A batch that declares a load must be invalidated in both directions, and this is what holds
//! it** — the rule **D13** states as «a mislabelled number is worse than no number».
//!
//! The measurement script already reads two of its report's lines in both directions: a batch that
//! asks for buttons and drew none, and a batch that asks for none and drew some; the same pair for
//! the control channel. Both pairs existed only in the script, which nothing in this suite read, so
//! a delivery that deleted half of one would leave a batch publishing «this costs nothing» with
//! every test green.
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
const DECLARED: &[(&str, &str)] = &[("ACTIONS", "buttons"), ("CONTROL", "control")];

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
