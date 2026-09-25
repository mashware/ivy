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
    ("READS", "documents"),
    ("PATCHES", "documents"),
    ("LINKS", "links"),
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

#[test]
fn a_read_batch_requires_a_nonzero_largest_serialized_document() {
    assert!(
        MEASURE.contains(
            r#"LARGEST_SERIALIZED=$(printf '%s' "$DOCUMENTS_LINE" | sed -n 's/.*, \([0-9]*\) bytes the largest written out$/\1/p')"#
        ),
        "the measurement script no longer extracts the largest serialized document from the report"
    );
    assert!(
        invalidates_on("[ \"${READS:-}\" = 1 ] && [ \"$LARGEST_SERIALIZED\" = 0 ]"),
        "batch V can publish a read count without proving that any document bytes were serialized"
    );
}

/// **The asymmetric guard this file protects, because the symmetric ones protect each other.**
///
/// Every pair above is symmetric — declared and measured none, declared none and measured some — and
/// the symmetry is what makes a deleted half visible to the loop. The patching batch has a third
/// that fits no pair: **none of its patches may be refused**. It is the only load of this bench a
/// batch can promise, because its publisher holds the pane it patches and every op names a key it
/// published itself, and it is worth asserting because a refused patch skips the write-out and the
/// install — while leaving on screen exactly the panel a successful one would have. A run that
/// refused every patch publishes the cost of stopping half way through one under the name of
/// patching, and looks perfect. (It does pay the clone and the index: the ops are only known to
/// resolve by resolving them, which is what all-or-nothing costs.)
///
/// **It is not the script's only asymmetric guard**, and saying otherwise was the first version of
/// this comment: `ACTIONS=1` with nothing pressed is another, and nothing in this file protects it
/// either. That one is not added here because it is not this delivery's, and a guard added without
/// its own reasoning is the kind nobody dares delete later.
#[test]
fn a_refused_patch_cannot_be_published_as_the_cost_of_patching() {
    assert!(
        invalidates_on("[ \"$PATCHES_REFUSED\" != 0 ]"),
        "nothing invalidates a patching batch whose patches were all refused: the run would publish \
         the cost of the refusal under the name of the work"
    );
}

/// **Batch W's own direction has to name every source, and the per-gesture demand has to exist.**
///
/// The pair above only proves that *some* condition on `LINKS = 1` invalidates. A guard that read
/// the plain count alone would satisfy it and publish a run in which OSC 8 or the panel never
/// opened a thing under the name of all three — measured: dropping two of the four checks left every
/// other test in this file green. And one activation per source is not the work either: a gesture
/// counts once its release is sent, landed or not, so the sum has to reach the gestures completed.
#[test]
fn batch_w_demands_every_source_and_an_opened_link_per_gesture() {
    // The knob is read twice — by the seeding and by the guard — so the guard is the occurrence
    // whose next line reads the report's counts.
    let lines: Vec<&str> = MEASURE.lines().collect();
    let direction = lines
        .windows(2)
        .find(|pair| {
            pair[0].contains("if [ \"${LINKS:-}\" = 1 ]; then") && pair[1].contains("$LINK_")
        })
        .map(|pair| pair[1])
        .expect("batch W's own direction is not in the script");
    for count in ["$LINK_FOUND", "$LINK_PLAIN", "$LINK_OSC", "$LINK_PANEL"] {
        assert!(
            direction.contains(&format!("[ \"{count}\" = 0 ]")),
            "batch W can publish a run in which {count} is zero: «{direction}»"
        );
    }
    assert!(
        invalidates_on(
            "[ \"$GESTURE\" = link ] && [ \"$((LINK_PLAIN + LINK_OSC + LINK_PANEL))\" -lt \"$INJECTED\" ]"
        ),
        "nothing invalidates a batch W run whose gestures mostly opened nothing"
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

/// **The line's name is not what the script reads — the words inside it are.**
///
/// The guard above holds the «documents:» heading and nothing under it, and the numbers are pulled
/// out by `sed` patterns anchored on the prose beside them. Rewording «patched» to «patches applied»
/// leaves that heading standing, every assertion above green, and each `sed` matching nothing — and
/// a capture that matches nothing used to become a zero, which reads as «no patches» and silences
/// both directions of the pair at once. The script now invalidates an empty capture, and this is the
/// other half: the anchors it reads by have to be words the report actually writes.
#[test]
fn every_word_the_scripts_patterns_anchor_on_is_one_the_report_writes() {
    const HEADER: &str = include_str!("../src/measure/header.rs");

    // The literal each `sed` in the script leans on, and the substring of the report line that has to
    // carry it. Kept as a list rather than derived from the script, for the reason `MODULES` in
    // `shape.rs` gives: a pattern extractor turns a rewritten `sed` into a target silently skipped.
    const ANCHORS: &[(&str, &str)] = &[
        ("read back", "read back"),
        ("patched,", "patched,"),
        ("patches refused,", "patches refused,"),
        (" sources found ", " sources found "),
        (" plain ", " plain "),
        (" OSC 8 ", " OSC 8 "),
        (" panel activations$", " panel activations\""),
        (" drawn.*$", " drawn whole,"),
        (" pressed.*$", " pressed\""),
        (" applied.*$", " applied,"),
    ];

    for (in_the_script, in_the_report) in ANCHORS {
        assert!(
            MEASURE.contains(in_the_script),
            "no pattern in the script reads «{in_the_script}» any more: this guard is stale"
        );
        assert!(
            HEADER.contains(in_the_report),
            "the script pulls a number out by «{in_the_script}» and the report no longer writes \
             «{in_the_report}»: every run reads that count as zero"
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

/// Runs the script's guards — from the function that invalidates a run to the end of the file — over
/// `report`, as they run after a launch that exited with `status`. Returns what they printed on standard error and whether
/// the report ended up renamed `.INVALID`.
///
/// **It runs the script and does not read it**, and that is the point: the defect this holds is a
/// line that reads correctly and ends the run under `set -eu`. It was verified wrongly once by
/// running the bare `grep` instead of the script, which is what the settings guard's comment records.
/// The launch is replaced by the exit status it would have left; nothing above that function is
/// needed by what follows it.
fn run_the_guards(name: &str, report: &str, status: u8) -> (String, bool) {
    let guards = &MEASURE[MEASURE
        .find("\ninvalidate() {")
        .expect("the script no longer defines invalidate()")..];
    let dir = std::env::temp_dir().join(format!("batches-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("report.txt");
    std::fs::write(&path, report).unwrap();

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("set -eu\nSTATUS={status}\n{guards}"))
        .env("REPORT", &path)
        .env_remove("ACTIONS")
        .env_remove("CONTROL")
        .env_remove("LINKS")
        .env_remove("CJK")
        .env_remove("EMOJI")
        .env_remove("READS")
        .env_remove("PATCHES")
        .env_remove("HEADER")
        .output()
        .expect("sh could not be run");
    let invalid = dir.join("report.txt.INVALID").exists();
    let _ = std::fs::remove_dir_all(&dir);
    (
        String::from_utf8_lossy(&output.stderr).into_owned(),
        invalid,
    )
}

/// **A run whose application crashed is invalidated for the crash, whatever its report lost.**
///
/// The application writes the report on the way out, so a panic leaves it truncated — here, empty.
/// Two things used to stand between that report and the status check: a guard whose number came out
/// of an assignment ending in `grep` ended the script in silence, report unrenamed, and a guard
/// above the check that did invalidate named a missing line instead of the crash.
#[test]
fn a_crashed_run_is_invalidated_for_the_crash_whatever_its_report_lost() {
    let (said, invalid) = run_the_guards("crashed", "", 101);

    assert!(
        invalid,
        "a run that exited with status 101 kept its report's good name; it said: «{said}»"
    );
    assert!(
        said.contains("status 101"),
        "a run that exited with status 101 was invalidated for something other than the crash: «{said}»"
    );
}

/// **A report missing the line a guard reads is invalidated by that guard**, rather than ending the
/// script on the assignment that reads it — or, once that assignment survives, reading the missing
/// numbers as zeros. The status is zero, so the crash check cannot be what speaks: this is the half
/// the test above cannot see once that check comes first. Each fixture carries every line above the
/// guard it aims at, so the guard that speaks is the one it names and not a later one.
#[test]
fn a_report_missing_the_line_a_guard_reads_is_invalidated_by_that_guard() {
    let links = "links: 0 sources found · 0 plain · 0 OSC 8 · 0 panel activations\n";
    let buttons = "buttons:      0 drawn whole, 0 pressed\n";
    for (lost, report) in [
        ("buttons", links.to_owned()),
        ("control", format!("{buttons}{links}")),
    ] {
        let (said, invalid) = run_the_guards(lost, &report, 0);
        assert!(
            invalid && said.contains(&format!("{lost} line could not be read")),
            "a report with no «{lost}:» line was not invalidated for it; it said: «{said}»"
        );
    }
}
