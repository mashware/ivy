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
    ("LARGE", "documents"),
    ("LINKS", "links"),
    ("SEARCH", "search"),
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

/// **Batch E's own direction has to name every count, and its leak guard has to exist.**
///
/// The pair of the loop above is satisfied by any condition on `SEARCH = 1` that invalidates — and
/// batch E has two: the counts, and the keystrokes that must not reach a program. Deleting the
/// first left every test green, because the second still matched. So each count is held by name, and
/// the leak guard on its own.
#[test]
fn batch_e_demands_presses_found_and_marked_and_no_keystroke() {
    let lines: Vec<&str> = MEASURE.lines().collect();
    let direction = lines
        .windows(2)
        .find(|pair| {
            pair[0].contains("if [ \"${SEARCH:-}\" = 1 ]; then") && pair[1].contains("$SEARCH_")
        })
        .map(|pair| pair[1])
        .expect("batch E's own direction is not in the script");
    // The exact condition and not each count on its own: with `&&` in place of `||` every count
    // is still named, and a run that found nothing is published.
    assert!(
        direction.contains(
            "[ \"$SEARCH_PRESSES\" = 0 ] || [ \"$SEARCH_FOUND\" = 0 ] || [ \"$SEARCH_MARKED\" = 0 ]"
        ),
        "batch E can publish a run in which one of its counts is zero: «{direction}»"
    );
    // And the other direction reads the paintings with a search OPEN: a search on a query that
    // matches nothing marks no cell and still paints inside the console's span.
    assert!(
        invalidates_on(
            "[ \"${SEARCH:-}\" != 1 ] && { [ \"$SEARCH_PRESSES\" != 0 ] || [ \"$SEARCH_OPEN\" != 0 ]; }"
        ),
        "a batch that asks for no search can open one that marks nothing and keep its good name"
    );
    assert!(
        invalidates_on(
            "if grep -q \"^key in the frame → handed over to be written\" \"$REPORT\"; then"
        ),
        "nothing invalidates a batch E run whose query or presses reached a program"
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
        (" presses, ", " presses, "),
        (" found .*$", " found ·"),
        (
            " paintings with a search open, ",
            " paintings with a search open, ",
        ),
        (" with matches marked .*$", " with matches marked ·"),
        (
            " paintings with a search open, .*$",
            " paintings with a search open, ",
        ),
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
    run_the_guards_as(name, &[], report, status)
}

/// [`run_the_guards`], with the knobs a batch's row sets set the way the batch table would have set
/// them — and **nothing else from this process's environment**, so a knob whoever runs the suite
/// happens to have exported cannot change which guard a fixture reaches.
fn run_the_guards_as(name: &str, knobs: &[&str], report: &str, status: u8) -> (String, bool) {
    let guards = &MEASURE[MEASURE
        .find("\ninvalidate() {")
        .expect("the script no longer defines invalidate()")..];
    // **The floor is defined above the function that invalidates a run**, where the seeding reads it too, so the slice
    // would reach the large pair's guards with it unset and die under `set -u` before they speak.
    let floor = MEASURE
        .lines()
        .find(|line| line.starts_with("LARGE_FLOOR="))
        .expect("the script declares no floor for the large document");
    // **Only a knob the table can set**, so a typo stands for no batch rather than for a different one.
    for knob in knobs {
        assert!(
            DECLARED.iter().any(|(declared, _)| declared == knob),
            "«{knob}» is not a knob any batch declares"
        );
    }
    let set: String = knobs.iter().map(|knob| format!("{knob}=1\n")).collect();
    let dir = std::env::temp_dir().join(format!("batches-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("report.txt");
    std::fs::write(&path, report).unwrap();

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("set -eu\nSTATUS={status}\n{floor}\n{set}{guards}"))
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("REPORT", &path)
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

/// The assignments of one batch's row in the table, without the comment that follows them.
fn row_of(batch: char) -> Vec<&'static str> {
    let row = MEASURE
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&format!("{batch})       TABS=")))
        .unwrap_or_else(|| panic!("batch {batch} is not in the table"));
    let (_, assignments) = row.split_once(')').expect("a case arm");
    let (assignments, _) = assignments.split_once(";;").expect("an arm that closes");
    assignments
        .split(';')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .collect()
}

/// **`X` feeds no written-out size of its own**, so nothing the report says can prove the control is
/// as large as the batch it is compared with: only reads and patches feed that number. A row of the
/// table losing `LARGE=1` would leave `X` publishing seven lines, `Y − X` would be the large document
/// rather than the patch, and every guard would stay quiet. So the table is what is held: the two of
/// them carry the knob, nothing else does, and `Y` is `X` plus the one verb.
#[test]
fn the_large_pair_differs_in_the_patch_and_in_nothing_else() {
    let control = row_of('X');
    let patching = row_of('Y');
    assert!(
        control.contains(&"LARGE=1") && patching.contains(&"LARGE=1"),
        "the large pair does not both publish the large document: X {control:?}, Y {patching:?}"
    );
    let mut control_plus_the_verb = control.clone();
    control_plus_the_verb.push("PATCHES=1");
    control_plus_the_verb.sort_unstable();
    let mut patching_sorted = patching.clone();
    patching_sorted.sort_unstable();
    assert_eq!(
        control_plus_the_verb, patching_sorted,
        "Y is not X plus the patch, so Y − X is not what a patch costs"
    );
    let carriers: Vec<&str> = MEASURE
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.strip_prefix(char::is_alphabetic)
                .is_some_and(|rest| rest.starts_with(")       TABS="))
        })
        .filter(|line| {
            line.split("#")
                .next()
                .is_some_and(|code| code.contains("LARGE=1"))
        })
        .collect();
    assert_eq!(
        carriers.len(),
        2,
        "the large document is published by batches other than X and Y: {carriers:?}"
    );
}

/// **Every declared knob is cleared before the batch table sets it**, because the seeding and the
/// guards read them from the script's own shell: one exported by whoever measures reaches every batch
/// and is judged by the guards as the batch's own load — a `PATCHES=1` runs X as Y under X's name.
#[test]
fn no_knob_comes_from_the_callers_environment() {
    let table = MEASURE
        .find("case \"$BATCH\" in")
        .expect("the batch table is not in the script");
    let cleared: Vec<&str> = MEASURE[..table]
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(str::split_whitespace)
        .filter_map(|word| word.strip_suffix('='))
        .collect();
    for (knob, _) in DECLARED {
        assert!(
            cleared.contains(knob),
            "«{knob}» is not cleared before the batch table: exported by whoever measures, it \
             reaches every batch and the guards judge it as the batch's own load"
        );
    }
}

/// **The floor is a literal in shell and a fraction of a constant in Rust**, and it is the second that
/// means something: «near the ceiling» is 7/8 of `MAX_BYTES`. A ceiling that moved would leave the
/// script calling a medium panel large, or refusing every run.
#[test]
fn the_large_floor_is_seven_eighths_of_the_contracts_ceiling() {
    let floor: u64 = MEASURE
        .lines()
        .find_map(|line| line.strip_prefix("LARGE_FLOOR="))
        .expect("the script declares no floor for the large document")
        .trim()
        .parse()
        .expect("the floor is a number");
    assert_eq!(floor, panel_contract::MAX_BYTES / 8 * 7);
}

/// **The large pair's own directions**: the check before publishing that is the only proof of `X`'s size, and
/// the report guard that proves `Y`'s patch applied at that size as the pane wrote it out. The pair
/// loop above only proves that *some* condition on `LARGE=1` invalidates.
#[test]
fn the_large_pair_proves_its_size_before_and_after_the_run() {
    let lines: Vec<&str> = MEASURE.lines().collect();
    let before = lines
        .iter()
        .position(|line| line.contains("[ \"$compact\" -lt \"$LARGE_FLOOR\" ]"))
        .expect("nothing checks the large document's size before it is published");
    // The bound the check reads is the one `the_large_document_lands_between_the_floor_and_the_ceiling`
    // proves sits under the compact size; any other arithmetic here is a bound nobody proved.
    assert!(
        MEASURE.contains("compact=$((bytes - 3 * lines))"),
        "the check before publishing no longer reads the bound the suite proves"
    );
    assert!(
        lines[before + 1..]
            .iter()
            .take_while(|line| line.trim() != "fi")
            .any(|line| line.trim() == "return 1"),
        "a large document under the floor is noticed and published anyway"
    );
    assert!(
        invalidates_on("[ \"$LARGEST_SERIALIZED\" -lt \"$LARGE_FLOOR\" ]"),
        "nothing invalidates a large patching run whose patch was written out under the floor"
    );
    assert!(
        invalidates_on("[ \"$LARGEST_SERIALIZED\" -ge \"$LARGE_FLOOR\" ]"),
        "nothing invalidates a batch that is not large and wrote out a large document"
    );
}

/// The report lines every guard above the documents block reads, all of them satisfied for a batch
/// that publishes over the channel and declares nothing else, so the guard that speaks is the
/// documents block's and not an earlier one.
fn a_channel_report_with(documents: &str) -> String {
    format!(
        "buttons:      0 drawn whole, 0 pressed\n\
         links: 0 sources found · 0 plain · 0 OSC 8 · 0 panel activations\n\
         control:      120 applied, 0 refused, 0 released\n\
         {documents}\n"
    )
}

/// **The large pair's size guards, run and not read.** The test above holds their text; this one
/// holds what they do at the edge, which is where a guard's conjunction or its comparison can be
/// wrong while its text still matches: `Y` one byte under the floor is thrown away and at the floor
/// is not thrown away by its size guard, `X` — large and not patching — is not either, `U` at the
/// floor is thrown away as a leak, and a documents line that stopped carrying the size is thrown away
/// instead of being read as a zero, for the large pair and for the batches that feed the size alike.
///
/// **A case that must pass the documents block proves it by the guard AFTER it speaking**: the
/// fixture has no strip line, so the next guard invalidates for that, and its sentence is the proof
/// the size guards ran and stayed quiet rather than the script dying before them.
#[test]
fn the_large_pair_is_judged_on_the_size_the_pane_wrote_out() {
    let floor: u64 = panel_contract::MAX_BYTES / 8 * 7;
    let documents = |size: u64| {
        format!(
            "documents:    0 read back, 0 refused, 60 patched, 0 patches refused, {size} bytes the \
             largest written out"
        )
    };
    let large_patching = ["CONTROL", "LARGE", "PATCHES"];

    let (said, invalid) = run_the_guards_as(
        "y-under",
        &large_patching,
        &a_channel_report_with(&documents(floor - 1)),
        0,
    );
    assert!(
        invalid && said.contains("under the floor"),
        "Y wrote its patch out one byte under the floor and kept its good name: «{said}»"
    );

    let (said, _) = run_the_guards_as(
        "y-at",
        &large_patching,
        &a_channel_report_with(&documents(floor)),
        0,
    );
    let past_the_documents = |said: &str| said.contains("whether a strip was painted");
    assert!(
        past_the_documents(&said),
        "Y at the floor did not get past the documents block: «{said}»"
    );

    let (said, _) = run_the_guards_as(
        "x",
        &["CONTROL", "LARGE"],
        &a_channel_report_with(
            "documents:    0 read back, 0 refused, 0 patched, 0 patches refused, 0 bytes the \
             largest written out",
        ),
        0,
    );
    assert!(
        past_the_documents(&said),
        "X, which publishes the large document and patches nothing, was stopped: «{said}»"
    );

    let (said, invalid) = run_the_guards_as(
        "u-at",
        &["CONTROL", "PATCHES"],
        &a_channel_report_with(&documents(floor)),
        0,
    );
    assert!(
        invalid && said.contains("leaked"),
        "U wrote out a document at the floor and kept its good name: «{said}»"
    );

    let (said, invalid) = run_the_guards_as(
        "y-no-size",
        &large_patching,
        &a_channel_report_with(
            "documents:    0 read back, 0 refused, 60 patched, 0 patches refused, and nothing else",
        ),
        0,
    );
    assert!(
        invalid && said.contains("written-out size"),
        "a documents line with no size was read as a zero instead of being thrown away: «{said}»"
    );

    let (said, invalid) = run_the_guards_as(
        "u-no-size",
        &["CONTROL", "PATCHES"],
        &a_channel_report_with(
            "documents:    0 read back, 0 refused, 60 patched, 0 patches refused, and nothing else",
        ),
        0,
    );
    assert!(
        invalid && said.contains("written-out size"),
        "U's documents line with no size was read as a zero, which silences its leak guard: «{said}»"
    );
}

/// **The large document the script builds, weighed by what the producer will send.** The check
/// before publishing is a lower bound — the bytes minus three per newline — and only running the
/// script's own functions can say whether the document it builds lands between the floor and the
/// ceiling once compacted, and whether the bound it checks really sits under that compact size.
#[test]
fn the_large_document_lands_between_the_floor_and_the_ceiling() {
    let from = |start: &str, end: &str| {
        let at = MEASURE
            .find(start)
            .unwrap_or_else(|| panic!("no «{start}» in the script"));
        let len = MEASURE[at..]
            .find(end)
            .unwrap_or_else(|| panic!("«{start}» never reaches «{end}»"));
        &MEASURE[at..at + len]
    };
    let bodies = from("\ndocument_body() {", "\npublish() {");
    let document_of = from("\ndocument_of() {", "\n}\n");
    let build = |large: &str| {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!(
                "set -eu\nBATCH=Y\nMIB=44\nLARGE={large}\n{bodies}\n{document_of}\n}}\ndocument_of 1"
            ))
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .output()
            .expect("sh could not be run");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("the document is text")
    };
    let floor = (panel_contract::MAX_BYTES / 8 * 7) as usize;

    let large = build("1");
    let compact = serde_json::to_string(
        &serde_json::from_str::<serde_json::Value>(&large).expect("the large document is JSON"),
    )
    .unwrap()
    .len();
    let bound = large.len() - 3 * large.matches('\n').count();
    assert!(
        (floor..=panel_contract::MAX_BYTES as usize).contains(&compact),
        "the large document compacts to {compact} bytes, outside [{floor}, ceiling]"
    );
    assert!(
        bound <= compact && bound >= floor,
        "the check before publishing reads {bound} bytes for a document that compacts to {compact}: \
         it is not a lower bound that clears the floor"
    );

    let ordinary = build("");
    assert!(
        ordinary.len() < floor / 100,
        "a batch that is not large builds a {} byte document",
        ordinary.len()
    );
}

/// **The batch whose panels carry emoji is counted as a batch that rasterizes**, in both directions.
///
/// Batch `Z` dumps ASCII in its consoles and publishes emoji in its panels. The colour guard used to
/// ask only whether the batch was `Q`, so every `Z` run would invalidate itself for rasterizing, and
/// a `Z` whose panels never reached the screen would be read as «the panel's colour path is free».
#[test]
fn the_batch_with_emoji_in_its_panels_is_held_to_the_colour_guard() {
    assert!(
        MEASURE
            .lines()
            .any(|line| line.trim_start().starts_with("Z)") && line.contains("PANEL_EMOJI=1")),
        "no batch declares emoji in its panels"
    );
    // **The whole block, because the knob's test also appears where the panels are seeded**: a
    // search for the test alone stays green with it deleted from the guard, and two assertions over
    // the `if` and its body separately stay green with the body moved under another `if`.
    assert!(
        MEASURE.contains(
            "if [ \"${EMOJI:-}\" = 1 ] || [ \"${PANEL_EMOJI:-}\" = 1 ]; then\n    COLOR_GLYPHS=1\nfi"
        ),
        "the colour guard does not count the panel knob as a batch that must rasterize"
    );
    assert!(
        invalidates_on("[ \"$COLOR_GLYPHS\" = 1 ] && [ \"$RASTERIZED\" = 0 ]"),
        "a batch painting emoji that rasterized none is not invalidated"
    );
    assert!(
        invalidates_on("[ \"$COLOR_GLYPHS\" != 1 ] && [ \"$RASTERIZED\" != 0 ]"),
        "a batch painting no emoji that rasterized some is not invalidated"
    );
}

/// Execute the shipped validator: deleting a direction changes the observed exit status.
#[test]
fn graphical_settings_reports_require_the_requested_effects() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let dir = std::env::temp_dir().join(format!(
        "settings-report-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let report = dir.join("report");
    let function = MEASURE
        .split("# Graphical settings report validation (also exercised with synthetic reports).\n")
        .nth(1)
        .unwrap()
        .split("# End graphical settings report validation.")
        .next()
        .unwrap();
    let shell = format!("set -eu\n{function}\ninvalidate() {{ exit 7; }}\ncheck_settings_report\n");
    let accepts = |mode: &str,
                   visible: bool,
                   entry: usize,
                   editor: usize,
                   fonts: usize,
                   families: usize| {
        let regime = if mode.is_empty() { "historical" } else { mode };
        fs::write(&report,format!("settings UI: visible={visible}, measurement=true\nsettings mode: {regime}\nsettings entry: frames={entry}, total=1ms\nsettings editor: frames={editor}, total=1ms\nsettings font enumeration: frames={fonts}, total=1ms\nsettings installed families: {families}\nsettings font requests: {}\n", if fonts>0 {2} else {0})).unwrap();
        Command::new("sh")
            .arg("-c")
            .arg(&shell)
            .env("REPORT", &report)
            .env("SETTINGS", mode)
            .status()
            .unwrap()
            .success()
    };
    assert!(accepts("", false, 0, 0, 0, 0));
    assert!(!accepts("", true, 1, 0, 0, 0));
    assert!(!accepts("", false, 0, 1, 0, 0));
    assert!(accepts("closed", true, 10, 0, 0, 0));
    assert!(!accepts("closed", true, 0, 0, 0, 0));
    assert!(!accepts("closed", true, 10, 1, 0, 0));
    assert!(accepts("open", true, 10, 9, 0, 0));
    assert!(!accepts("open", true, 10, 0, 0, 0));
    assert!(!accepts("open", true, 10, 9, 1, 10));
    assert!(accepts("fonts", true, 10, 9, 1, 20));
    assert!(!accepts("fonts", true, 10, 9, 0, 20));
    assert!(!accepts("fonts", true, 10, 9, 1, 0));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn settings_variants_are_not_averaged_across_modes() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let dir = std::env::temp_dir().join(format!(
        "settings-compare-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let first = dir.join("first");
    let second = dir.join("second");
    let header = |mode: &str| {
        format!(
            "window: 1280×800 points\ntabs: 1\n  pane 0: 50×41 (left)\nsettings UI: visible=true, measurement=true\nsettings mode: {mode}\n"
        )
    };
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/measure-frame.sh");
    let comparable = || {
        Command::new("sh")
            .arg(&script)
            .arg("compare")
            .arg(&first)
            .arg(&second)
            .env_remove("MEASURE_AS_CONSOLE")
            .output()
            .unwrap()
            .status
            .success()
    };
    fs::write(&first, header("closed")).unwrap();
    fs::write(&second, header("closed")).unwrap();
    assert!(comparable());
    fs::write(&second, header("open")).unwrap();
    assert!(!comparable());
    fs::remove_dir_all(dir).unwrap();
}
