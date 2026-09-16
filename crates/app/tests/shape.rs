//! **A module has a ceiling and an `impl` block has a ceiling, and this is what holds them** (D28).
//!
//! Its two siblings hold the language — [`prose`](../prose.rs) for the comments,
//! [`glossary`](../glossary.rs) for the identifiers — and both exist because the same lesson was
//! paid for twice: a rule that lives only in somebody's judgement grows for eighteen days and then
//! costs a milestone. «The code is in Spanish» was never written down and reached 43 860 lines.
//! «A module does not grow past a few hundred lines» was never written down either, and one file of
//! this crate reached **3 119 production lines** while the other eleven stayed between 97 and 542.
//! That count is the one the rule was written against; the crate has gained modules since, every one
//! of them a piece coming *out* of that file.
//!
//! # What it counts, and what it deliberately does not
//!
//! **Production lines only**: comments and the `#[cfg(test)] mod tests` block do not count. Rust
//! keeps unit tests inside the file by convention and this project writes comments that argue, so a
//! ceiling over the raw line count would punish the two habits it most wants to keep — and the
//! cheapest way to pass it would be to delete argument.
//!
//! **Nothing about function length**, and that was measured before being dropped: across the whole
//! workspace the median production function is 10 lines, the 90th percentile is 38, and **13 of 443**
//! are over 60. A ceiling there would flag thirteen pieces of existing, argued code and prevent no
//! future defect.
//!
//! # The one exception, and why it is a number rather than a name
//!
//! `src/lib.rs` is over the ceiling today and cannot come under it in one change: it is 20 820 lines
//! with 16 953 of tests reaching into private items. So it is declared here **with the number it has
//! now**, and the guard fails if that number **grows**. An exception with no number is an exemption,
//! and the next reader cannot tell «too big, being worked on» from «fine». D28 carries the order the
//! pieces come out in.
//!
//! **The tests are what makes the order slow, not the code.** A piece leaves as a module of its own
//! and `lib.rs` re-imports it by name, so those 16 953 lines reach the moved items exactly where
//! they reached them before and not one test is edited by a move. A guard list that names files is
//! the one thing a split does break, and there are **six** of them across this suite —
//! `MODULES` here, the file list and the corpus floor in `prose`, two lists in `glossary`, the
//! sentence walk inside `settings.rs`, and `MODULES` in `scripts`. The last of those went unpaid by
//! the delivery that wrote this line, in silence, because it was the one list with no count floor;
//! it has one now.

/// Every module of the crate, pulled in **at compile time** for the same reason its siblings give:
/// a directory walk turns a renamed or deleted file into a target that is silently skipped, and a
/// guard with no file to look at reads exactly like a clean one.
const MODULES: &[(&str, &str)] = &[
    ("src/lib.rs", include_str!("../src/lib.rs")),
    ("src/alarms.rs", include_str!("../src/alarms.rs")),
    ("src/closing.rs", include_str!("../src/closing.rs")),
    ("src/control.rs", include_str!("../src/control.rs")),
    ("src/injected.rs", include_str!("../src/injected.rs")),
    ("src/panel.rs", include_str!("../src/panel.rs")),
    (
        "src/panel/buttons.rs",
        include_str!("../src/panel/buttons.rs"),
    ),
    (
        "src/panel/driving.rs",
        include_str!("../src/panel/driving.rs"),
    ),
    ("src/panel/pane.rs", include_str!("../src/panel/pane.rs")),
    ("src/measure.rs", include_str!("../src/measure.rs")),
    (
        "src/measure/header.rs",
        include_str!("../src/measure/header.rs"),
    ),
    ("src/session.rs", include_str!("../src/session.rs")),
    ("src/fonts.rs", include_str!("../src/fonts.rs")),
    ("src/settings.rs", include_str!("../src/settings.rs")),
    ("src/keyboard.rs", include_str!("../src/keyboard.rs")),
    ("src/painter.rs", include_str!("../src/painter.rs")),
    (
        "src/painter/notices.rs",
        include_str!("../src/painter/notices.rs"),
    ),
    (
        "src/painter/blocks.rs",
        include_str!("../src/painter/blocks.rs"),
    ),
    ("src/pty_output.rs", include_str!("../src/pty_output.rs")),
    ("src/watcher.rs", include_str!("../src/watcher.rs")),
    ("src/theme.rs", include_str!("../src/theme.rs")),
    ("src/mouse.rs", include_str!("../src/mouse.rs")),
    ("src/opening.rs", include_str!("../src/opening.rs")),
    ("src/pointer.rs", include_str!("../src/pointer.rs")),
    ("src/report.rs", include_str!("../src/report.rs")),
    ("src/tab_bar.rs", include_str!("../src/tab_bar.rs")),
    ("src/tiling.rs", include_str!("../src/tiling.rs")),
    ("src/tree.rs", include_str!("../src/tree.rs")),
    ("src/main.rs", include_str!("../src/main.rs")),
];

/// Production lines a module may hold.
///
/// **600 and not a rounder number, because it was the measured shape of this crate.** When the rule
/// landed, the eleven modules that are not `src/lib.rs` ran from 97 to 542, so 600 left the largest
/// of them room to grow by a tenth and stopped the next one becoming a second `lib.rs`. A ceiling
/// below 542 would have failed on work nobody had been asked to do; one far above it would never
/// have fired.
///
/// **Today the largest is `src/fonts.rs`, at exactly 600.** The margin the number was chosen for is
/// spent, and that is said here rather than fixed by raising it: a ceiling that moves whenever
/// something reaches it is not a ceiling. The next line `fonts.rs` gains fails this guard, and what
/// it will need is a piece taken out — the same answer `lib.rs` is getting.
const MODULE_CEILING: usize = 600;

/// Production lines an `impl` block may hold.
///
/// A type whose behaviour does not fit in 300 lines is more than one type, or its seams are in the
/// wrong place. Measured as this guard lands: the largest block outside `src/lib.rs` is
/// `impl Discoverer` at 269, so this ceiling asks nothing of the code that is already in shape. It
/// read 223 until somebody counted again: a number in a comment decays on its own, and the ones this
/// file states about the crate are re-measured by every delivery that touches it.
const IMPL_CEILING: usize = 300;

/// What is over a ceiling today, with the number it is at and the reason it is not fixed here.
///
/// **A ratchet, not an exemption**: the guard fails when one of these grows. Lowering a number is
/// the normal outcome of a delivery that takes a piece out, and this table is where that progress is
/// recorded.
const OVER_CEILING_TODAY: &[(&str, usize, &str)] = &[(
    "src/lib.rs",
    959,
    "the window's core, and the file nobody ever split. D28 has the order its pieces come out \
         in; it cannot come under the ceiling in one change because 16 953 lines of tests reach \
         into its private items. **It came in at 3 119; the bench's injected gestures were the \
         first piece out and reading the settings file the second, which left for `settings.rs` \
         with the delivery that added the font by name**. **And the number carried three lines of \
         slack nobody had noticed**: it read 2 804 while the file was at 2 801, so a delivery could \
         grow the file and pass. The one that held a paste spent two of them, said so, and closed \
         the gap by recording what the file actually is — a ratchet with slack is a ratchet that \
         ratchets late. **Then the order moved it for real**, in five deliveries out of 2 803: the \
         tiling and the tree 692, the mouse 430, publishing the report 96, the tab bar 100 and \
         saving 51 into `session.rs`. **And then this entry fired on an ordinary delivery for the \
         second time**: the one that tells the focused console's cursor apart needed ten lines \
         here, so painting a pane left for `painter.rs` — 62 lines, and the piece that delivery \
         was already editing, which is exactly the way through D28 records. Sixty-two out and ten \
         back in leaves 1 382. **And then the piece went out BEFORE the delivery that needs the \
         room, which is the first time that happened here**: the blinking cursor needs about nine \
         lines in this file, so the deadlines the window sets itself left for `alarms.rs` on their \
         own — 16 lines out, one `mod` line back in — leaving **1 367, against the 3 119 this file \
         came in at**. Taking the piece out first is what lets the ratchet say whether the delivery that \
         follows paid for its own lines — **and it says the blink did not**: it spent 14 lines here, \
         not nine, so a second piece went out in the same delivery. Putting the settings the file \
         asks for onto the window left for `settings.rs`, 19 lines, the piece that delivery was \
         already editing. Nineteen out and fourteen in against 1 367 leaves **1 362**, which is what \
         the guard measured until the phase that gives the panel buttons. (The first telling said \
         fifteen in and did not reconcile; a comment audit did the subtraction.) **Then two pieces \
         went out ahead of the deliveries that need the room, and nothing came in with them**: what \
         a pane does with the keys of one pass left for `keyboard.rs` and what it does with the \
         wheel and a mouse gesture for `mouse.rs`, **238 production lines against the 1 362 above**, \
         leaving 1 124. The subtraction is stated between the two numbers this guard checks and not \
         broken down per piece: a per-piece tally is arithmetic nothing runs, and the last time one \
         was written here it did not reconcile either. That second move is also what retired the \
         `impl Pane` entry that used to sit below: at 204 it no longer exceeds its ceiling, and an \
         entry that stops exceeding one is deleted rather than kept as a trophy — which is exactly \
         what `no_entry_of_the_ratchet_is_stale` fails for. **And then the milestone that gives the \
         panel buttons paid it again**, because the previous delivery had left `impl Window` at its \
         recorded number with no room at all and four lines were owed here: how a window comes into \
         being — with the tabs it is asked for, or with the ones it left behind — went out to \
         `opening.rs`, **117 production lines**, and `impl Window` lost 112 of them. That is also what \
         retired the entry that used to sit below for that block: at **289** it no longer exceeds \
         its ceiling. **Fifteen lines came straight back in the same delivery** — the fields the \
         buttons' measurement needs on `Window` and on a pane's outcome, and the walk that hands the \
         bench's injector the button the painting drew — so the file went 1 124 → 1 007 → **1 022**, \
         which is what the guard measured until the delivery that gives the panel a channel of its \
         own. **That one paid it the way D28 prescribes, and with the piece the delivery's own \
         subject names**: how a pane gets its panel — its directory, the identity it hands its \
         program, what arrives for it and how much width it takes — went out to `panel/pane.rs`, \
         **46 production lines**, against the **24** the channel spends here: a `mod`, a `use`, the \
         field a pane keeps its end of the channel in, the block that opens it, the one that applies \
         what arrived, and the four that keep what the channel cost off the panel's stopwatch on \
         the consoles nobody paints. Forty-six out and twenty-four in against 1 022 leaves \
         **1 000**. The two \
         numbers are said rather than netted off: what the order is worth is only readable if a \
         delivery's own cost is written beside the piece it took out. **And the phase that gives the \
         panel a channel of its own paid it again**: the aggregate of the channel's counters had to \
         survive a pane that closes — a pane or a tab closed mid-run used to take its \
         `applied`/`refused`/`released` out of the numbers the report folds — so everything that \
         happens when a tab or the focused console goes left for `closing.rs`, **71 production \
         lines**, against the **30** the counters' field, their init and the banking cost back here. \
         Seventy-one out and thirty in against 1 000 leaves **959**.",
)];

/// The production lines of a module: no comments, and nothing from `mod tests` onwards.
///
/// **It stops at `mod tests` and that boundary is the rule's, not laziness** — the same cut its
/// sibling in `settings.rs` makes, and for the same kind of reason. Rust puts unit tests in the file
/// the code lives in; counting them would make this guard measure a convention of the language
/// instead of the shape of the code.
///
/// **Block comments are walked by depth, because Rust's nest.** Leaving at the first `*/` would let
/// the prose after it count as code, which is a ceiling failing on argument.
fn production_lines(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    for line in source.lines() {
        let code = line.trim_start();
        if code.starts_with("mod tests") || code.starts_with("pub mod tests") {
            break;
        }
        let opens = line.matches("/*").count();
        let closes = line.matches("*/").count();
        if depth > 0 || opens > 0 {
            depth = depth + opens - closes.min(depth + opens);
            continue;
        }
        if code.is_empty() || code.starts_with("//") {
            continue;
        }
        out.push(line);
    }
    out
}

/// Every `impl` block of a module, as `(what it is called, how many production lines)`.
///
/// **It leans on rustfmt and says so**: a top-level `impl` starts at column zero and its closing
/// brace is a `}` alone at column zero. That is true of every file of this crate because
/// `cargo fmt --check` is a gate, and [`every_impl_block_was_closed`] is what turns the assumption
/// into something that fails out loud instead of quietly counting to the end of the file.
fn impl_blocks<'a>(lines: &[&'a str]) -> Vec<(&'a str, usize, bool)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with("impl ") || lines[i].starts_with("impl<") {
            let mut j = i + 1;
            while j < lines.len() && lines[j] != "}" {
                j += 1;
            }
            let closed = j < lines.len();
            out.push((lines[i], j.min(lines.len() - 1) - i + 1, closed));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// The name this guard calls an `impl` block, which is what a failure has to be searchable by.
fn name_of(module: &str, header: &str) -> String {
    let head = header.trim_end_matches(" {").trim_end();
    format!("{module}::{head}")
}

fn ceiling_for(name: &str) -> Option<usize> {
    OVER_CEILING_TODAY
        .iter()
        .find(|(what, ..)| *what == name)
        .map(|(_, at, _)| *at)
}

#[test]
fn no_module_is_over_its_ceiling() {
    // **A floor first, because this whole file passes by looking at nothing otherwise.** It is how
    // the guards of this suite die: somebody reshapes the walk, it finds no lines, every module
    // looks tiny and nothing says so. Measured today: **twenty-five modules**, the thinnest being
    // `src/main.rs` with three.
    //
    // **Splitting a module does not raise the total and that is the point** — it moves lines from
    // one entry to two — so a floor over the sum survives every delivery of D28's order. What has to
    // be raised with each split is the module *count*, and a split that forgets to add its new files
    // to `MODULES` fails right here instead of quietly exempting them.
    //
    // **And that last sentence was false for two deliveries**, because the count sat at nineteen
    // while the list held twenty-one: two modules could have been dropped with the suite green, which
    // is the same slack this file's own ratchet prose warns about. It is tight now, and it stays
    // tight only if the number is raised in the commit that adds the entry.
    let total: usize = MODULES
        .iter()
        .map(|(_, src)| production_lines(src).len())
        .sum();
    assert!(
        MODULES.len() >= 29 && total >= 6_000,
        "the walk found {total} production lines over {} modules: it stopped matching, and this \
         guard is measuring nothing",
        MODULES.len()
    );

    let mut over = Vec::new();
    for (name, source) in MODULES {
        let lines = production_lines(source).len();
        let ceiling = ceiling_for(name).unwrap_or(MODULE_CEILING);
        if lines > ceiling {
            over.push(format!(
                "  {name}: {lines} production lines, over its ceiling of {ceiling}"
            ));
        }
    }
    assert!(
        over.is_empty(),
        "a module grew past what D28 allows it. A module holds {MODULE_CEILING} production lines; \
         the ones already over it are declared in `OVER_CEILING_TODAY` **with the number they are \
         at**, and that number may only go down:\n{}",
        over.join("\n")
    );
}

#[test]
fn no_impl_block_is_over_its_ceiling() {
    let mut over = Vec::new();
    let mut seen = 0usize;
    for (module, source) in MODULES {
        let lines = production_lines(source);
        for (header, size, _) in impl_blocks(&lines) {
            seen += 1;
            let name = name_of(module, header);
            let ceiling = ceiling_for(&name).unwrap_or(IMPL_CEILING);
            if size > ceiling {
                over.push(format!(
                    "  {name}: {size} production lines, over its ceiling of {ceiling}"
                ));
            }
        }
    }
    // The same floor, for the same reason: a scanner that stops recognising `impl` reports nothing
    // and reads exactly like a crate in good shape. There are fifty-seven of them today.
    assert!(
        seen >= 30,
        "the scanner found {seen} impl blocks in the whole crate: it stopped recognising them"
    );
    assert!(
        over.is_empty(),
        "an impl block grew past what D28 allows it. A block holds {IMPL_CEILING} production \
         lines; the ones already over it are declared in `OVER_CEILING_TODAY` **with the number \
         they are at**, and that number may only go down:\n{}",
        over.join("\n")
    );
}

#[test]
fn every_impl_block_was_closed() {
    // **The scanner's own assumption, asserted instead of trusted.** It finds a block's end by
    // looking for a `}` alone at column zero, which is true because `cargo fmt --check` is a gate.
    // If that ever stops holding, the block runs to the end of the file and this guard reports a
    // number nobody can act on — so the assumption fails out loud here rather than quietly there.
    for (module, source) in MODULES {
        let lines = production_lines(source);
        for (header, _, closed) in impl_blocks(&lines) {
            assert!(
                closed,
                "{}: the scanner never found the `}}` that closes it at column zero, so every \
                 count from here on is wrong",
                name_of(module, header)
            );
        }
    }
}

#[test]
fn no_entry_of_the_ratchet_is_stale() {
    // **What keeps the table from rotting into a blanket exemption**, the lesson its sibling
    // `no_island_is_stale` records for the declared Spanish: an entry naming something that is no
    // longer over the ceiling excuses nothing and is read by the next reader as «still too big».
    //
    // **And it is what turns the ratchet into progress rather than paperwork**: a delivery that
    // takes a piece out has to come here and lower the number, because leaving it high is a failure.
    for (what, at, why) in OVER_CEILING_TODAY {
        let real = if let Some((module, header)) = what.split_once("::") {
            let source = MODULES
                .iter()
                .find(|(name, _)| name == &module)
                .map(|(_, src)| *src)
                .unwrap_or_else(|| panic!("`{what}` names a module this guard does not walk"));
            let lines = production_lines(source);
            impl_blocks(&lines)
                .into_iter()
                .find(|(h, ..)| h.trim_end_matches(" {").trim_end() == header)
                .map(|(_, size, _)| size)
                .unwrap_or_else(|| panic!("`{what}` names an impl block that no longer exists"))
        } else {
            let source = MODULES
                .iter()
                .find(|(name, _)| name == what)
                .map(|(_, src)| *src)
                .unwrap_or_else(|| panic!("`{what}` names a module this guard does not walk"));
            production_lines(source).len()
        };

        assert!(
            real <= *at,
            "`{what}` is at {real} production lines and the ratchet records {at}: it grew. The \
             reason recorded for it is «{why}»"
        );
        assert!(
            real > MODULE_CEILING.min(IMPL_CEILING) || real > *at,
            "`{what}` is at {real} lines, under every ceiling: it does not need an entry any more, \
             and leaving one reads as «still too big»"
        );
        // **Zero tolerance, and it used to be two hundred.** A ratchet that allows slack is a
        // ratchet that ratchets late: at 200 — two thirds of `IMPL_CEILING` — a delivery could take
        // 199 lines out, forget to lower the number, and pass in silence, which is the precise
        // failure the `src/lib.rs` entry's own prose says was closed when its three lines of slack
        // were found. It was closed for that one number and left open for every future one. The
        // guard fired on this train not because of the slack at all: walking every commit of the
        // order, the largest recorded-minus-real gaps were 134 and 111 — both comfortably under the
        // old 200 — and what actually fired, once, was the **growth** assertion above (recorded
        // 1 462 against a real 1 485). The tolerance was never what caught anything; it was what
        // let a stale number sit for weeks.
        //
        // **What zero costs is one edit per delivery, which is the point**: the number is the
        // measurement or it is not a measurement.
        assert_eq!(
            *at, real,
            "`{what}` is at {real} and the ratchet records {at}: a delivery moved the code and did \
             not re-measure. The number here is the measurement, not an approximation of it"
        );
    }
}

/// A pane declares its pseudoterminal **before** the bridge that holds its receiver, and Rust drops
/// fields in declaration order.
///
/// **This is not tidiness and the crate below cannot enforce it.** `term-core` tells a console being
/// closed from a listener that vanished on its own by whether the stop flag is up, and it raises that
/// flag in `Pty::drop` — but the receiving end belongs to whoever hosts the console. A `Pane` that
/// declared its bridge first would let the receiver go while the flag was still down and record
/// **every ordinary close** as «nobody was listening», which is the sentence that mechanism exists
/// to keep from being said falsely. The crate says so where it matters and asks whoever first reads
/// the distinction to pin it; that reader is the developer line, and this is the pin.
///
/// **It is a property of the file's text because there is nothing left to ask at runtime.** Once both
/// halves are dropped there is no cell anybody holds and no console to answer for either of them.
/// The effect of the reversal *is* covered by a test —
/// `a_bridge_whose_thread_lets_go_of_the_receiver_records_nobody_listening` — which is what makes
/// this one a guard over the order rather than over the mechanism.
#[test]
fn the_pseudoterminal_is_declared_before_the_bridge_that_holds_its_receiver() {
    let source = MODULES
        .iter()
        .find(|(name, _)| *name == "src/lib.rs")
        .map(|(_, source)| *source)
        .expect("`src/lib.rs` is not in the module list");
    let fields = source
        .split_once("struct Pane {")
        .expect("`struct Pane` is not declared in `src/lib.rs` under that name")
        .1;
    let pty = fields
        .find("\n    pty: Pty,")
        .expect("`Pane` no longer declares a field `pty: Pty`");
    let bridge = fields
        .find("\n    pty_output: PtyOutput,")
        .expect("`Pane` no longer declares a field `pty_output: PtyOutput`");

    assert!(
        pty < bridge,
        "`Pane` declares its bridge before its pseudoterminal, so the bridge is dropped first and \
         its receiver goes away while the stop flag is still down: every ordinary close would be \
         recorded as «nobody was listening». Declare `pty` first"
    );
}

/// **A gate that is private on purpose keeps a counted set of callers, and this counts them.**
///
/// `keyboard::paste` strips the control characters out of anything that did not come from a key, and
/// its own docblock says why it is not `pub`: *«an ungated door sitting beside the gated one, equally
/// reachable and better named, is one autocomplete away from being used»*. That argument rested on
/// distance — the code that could have bypassed it lived in `lib.rs`, a module away. **D28's order
/// removed the distance**: the walk that consumes a pass's keys now lives in `keyboard.rs` itself,
/// and the two functions that assemble a mouse report live beside what sends one.
///
/// So the protection stops being geography and becomes a number. **Three doors reach `paste`** —
/// composing a paste as it arrives, composing a held one when it is finally answered against the
/// mode that is current then (the whole of **D31**), and composing what a panel's button sends. The
/// mouse's two report forms have one door each. A call site beyond the count is the defect, whether
/// or not it is wrong on the day it is written.
///
/// **The third one is what this guard was written for, one delivery before it existed.** The
/// milestone that gives the panel buttons needed a way to write text another process declared, the
/// obvious move was a second filter beside the first, and this number is what turned that into a
/// decision somebody had to make out loud instead of a function somebody added.
///
/// # Two blind spots this guard had, both found by mutating it
///
/// **It read [`production_lines`], which stops at the first `mod tests`** — and `keyboard.rs` keeps
/// about 160 lines of production code *below* its own test module. A third caller appended there
/// left this guard green. So it reads [`outside_the_test_modules`] instead, which removes each test
/// block by counting braces rather than truncating at the first one: a call in a unit test is still
/// not a caller, and a call after one still is.
///
/// **And it looked its module up by exact name**, so a child module calling `super::paste` was never
/// read — the shape this very delivery created with `measure/header.rs`. It now folds in every entry
/// of [`MODULES`] that lives under the gate's own module.
#[test]
fn a_gate_that_is_private_on_purpose_still_has_its_counted_callers() {
    for (module, gate, doors, through) in [
        (
            "src/keyboard.rs",
            "paste",
            3,
            "`paste_or_hold`, a held paste being answered, and what a panel's button sends",
        ),
        ("src/mouse.rs", "extended_form", 1, "`encode`"),
        ("src/mouse.rs", "legacy_form", 1, "`encode`"),
    ] {
        let family = module.trim_end_matches(".rs");
        let sources: Vec<&str> = MODULES
            .iter()
            .filter(|(name, _)| *name == module || name.starts_with(&format!("{family}/")))
            .map(|(_, source)| *source)
            .collect();
        assert!(
            !sources.is_empty(),
            "`{module}` is not in the module list, so this guard reads nothing"
        );

        let callers: usize = sources
            .iter()
            .flat_map(|source| outside_the_test_modules(source))
            .filter(|line| !line.trim_start().starts_with("fn "))
            .map(|line| calls_of(line, gate))
            .sum();

        assert_eq!(
            callers, doors,
            "`{module}` and the modules under it call `{gate}` {callers} times and the doors to it \
             are {through}: a call site beyond those is a way past the gate, which is the thing \
             that docblock says it is private to prevent"
        );
    }
}

/// The lines of a module with its **test blocks removed**, rather than everything after the first
/// one.
///
/// [`production_lines`] truncates at `mod tests`, which is right for a line count — what follows is
/// mostly tests, and counting them would make that ceiling measure a convention of the language.
/// It is wrong for a guard that asks *whether a call exists*: `keyboard.rs` keeps four `impl` blocks
/// and about 160 lines of production code below its test module, and a truncating walk cannot see a
/// single one of them. **Measured by mutation**: a third caller of the gate, appended past that cut,
/// left the guard green.
///
/// Blocks are removed by counting braces from the `mod tests` line, so production code that follows
/// one is read. Nested braces inside the block are what the count is for; a brace inside a string
/// literal would fool it, and no test module in this crate opens with one.
fn outside_the_test_modules(source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut lines = source.lines();
    while let Some(line) = lines.next() {
        let code = line.trim_start();
        if !(code.starts_with("mod tests") || code.starts_with("pub mod tests")) {
            out.push(line);
            continue;
        }
        let mut depth = line.matches('{').count() - line.matches('}').count();
        for inner in lines.by_ref() {
            depth += inner.matches('{').count();
            depth -= inner.matches('}').count().min(depth);
            if depth == 0 {
                break;
            }
        }
    }
    out
}

/// How many times a line calls that function **by its whole name**.
///
/// The whole-name part is the guard's own bug, found by running it: `paste` as a substring also
/// matches `bracketed_paste()`, which reads a mode off the screen and goes nowhere near the gate.
fn calls_of(line: &str, name: &str) -> usize {
    let wanted = format!("{name}(");
    let mut found = 0;
    let mut rest = line;
    while let Some(at) = rest.find(&wanted) {
        let before = rest[..at].chars().next_back();
        if !before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            found += 1;
        }
        rest = &rest[at + wanted.len()..];
    }
    found
}
