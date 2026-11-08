//! The inverted ratchet: **it fails for what is still in Spanish**, crate by crate.
//!
//! Every other guard in this repository protects behaviour. This one protects a decision — the
//! glossary written in `docs/02-decisions.md` — and it is the only thing standing between a
//! milestone that renames tens of thousands of lines and a tree that ends up half translated
//! without anybody noticing. The compiler cannot help: a Spanish identifier compiles perfectly.
//!
//! **It derives the Spanish vocabulary from the glossary instead of repeating it.** A test that
//! copied the list would be a second place to keep it up to date, and the first time the two
//! disagreed the guard would be the one that is wrong. This is the same mechanism that ties the
//! measurement script to the report it reads: derive, never duplicate.
//!
//! **And it is scoped per crate, on purpose.** A single repo-wide list that shrinks towards zero
//! would be edited by several deliveries at once — four of them translate prose in parallel — and
//! every one of them would land on the same lines. A list of *finished crates* only ever grows, one
//! entry per delivery, so two deliveries never touch it together.
//!
//! # What it does not see, said out loud
//!
//! - **A lowercase Spanish noun that the glossary never names.** The glossary decides *types*, so a
//!   local called `ruta` or `hueco` is invisible here. Precision was chosen over reach on purpose:
//!   this guard breaks the build, and a lexicon wide enough to catch every Spanish word would also
//!   catch English ones. What closes that gap is reading the diff, not widening the pattern.
//! - **Test function names.** They are prose — sentences, not identifiers — and they are translated
//!   in the prose deliveries, so they are skipped by the `#[test]` above them. Everything else
//!   inside a test module is checked, locals included.
//! - **Comments, rustdoc and string literals**, for the same reason: prose has its own delivery.

mod common;

use common::fold;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The decisions record, which carries the glossary in its annex.
const DECISIONS: &str = include_str!("../../../docs/02-decisions.md");

/// The crates whose identifiers are **all English**, under `crates/`. One entry per delivery.
const FINISHED_CRATES: &[&str] = &["panel-contract", "host-load", "term-core", "panel-control"];

/// Single **files** that carry **no name the glossary retired**, for a crate that is not finished yet.
///
/// **It said «identifiers are all English» for one delivery, and that was not true of a single one of
/// the five.** Their families are renamed and their glossary names are gone, which is what this list
/// is for; what they still hold is the lowercase Spanish local the glossary never had to decide — the
/// `resto` of a slice, the `ahora` of a clock, the `fase` of a header — declared out loud as this
/// file's blind spot at the top and taken by the delivery that finishes `app/src/lib.rs`. There are
/// **195** of them; the count, the method and why the first attempt at it said 190 are in **D27**.
///
/// The claim matters because of what this guard does when it is wrong: it breaks the build. A list
/// that overstates what it checked is read once, believed, and then relied on by the next delivery to
/// decide what is already done — which is exactly how the five ended up declared finished.
///
/// **This came in once, was deleted, and is back — and the round trip is the point.** It was built to
/// point the guard at `app/src/lib.rs`, and pointing it there proved that file could not be declared
/// clean either: it still holds hundreds of Spanish names. With nothing left to name, a branch of the
/// walk became reachable by nobody, so it went out — this project deletes a mechanism with no
/// scenario. It returns with the delivery that gives it one.
///
/// A crate whose every file ends up here is **not** thereby ready for [`FINISHED_CRATES`]: that list
/// is the stronger claim, and this one cannot add up to it.
const GLOSSARY_CLEAN_FILES: &[&str] = &[
    "app/src/painter/pane.rs",
    "app/src/ligatures.rs",
    "app/src/ligature_tests.rs",
    "app/src/fonts/ligatures.rs",
    "app/src/painter/ligatures.rs",
    "app/src/painter/ligatures/tests.rs",
    "app/src/console_input.rs",
    "app/src/summary.rs",
    "app/src/summary/input.rs",
    "app/src/summary/model.rs",
    "app/src/summary/view.rs",
    "app/src/summary/probe.rs",
    "app/src/panel/summary.rs",
    "app/src/measure/overview.rs",
    "app/src/tests/overview.rs",
    "app/src/mouse.rs",
    "app/src/opening.rs",
    "app/src/keyboard.rs",
    "app/src/painter.rs",
    "app/src/painter/emoji.rs",
    "app/src/painter/notices.rs",
    "app/src/fonts.rs",
    "app/src/fonts/atlas.rs",
    "app/src/header.rs",
    "app/src/pointer/header.rs",
    "app/src/header/actions.rs",
    "app/src/header/controls.rs",
    "app/src/header/menu.rs",
    "app/src/header/splitting.rs",
    "app/src/tests/header_controls.rs",
    "app/src/panel.rs",
    "app/src/panel/buttons.rs",
    "app/src/theme.rs",
    "app/src/theme/catalog.rs",
    "app/src/watcher.rs",
    "app/src/pty_output.rs",
    "app/src/lib.rs",
    "app/src/pointer.rs",
    "app/src/report.rs",
    "app/src/search.rs",
    "app/src/tab_bar.rs",
    "app/src/tiling.rs",
    "app/src/tree.rs",
    "app/src/chrome.rs",
    "app/src/header/integrated.rs",
    "app/src/pane_view.rs",
    "app/src/visibility.rs",
    "app/src/tests/pane_rename.rs",
    "app/src/tests/integrated_chrome.rs",
    "app/src/tab_bar/interaction.rs",
    "app/src/alarms.rs",
    "app/src/closing.rs",
    "app/src/control.rs",
    "app/src/control/retiring.rs",
    "app/src/panel/document.rs",
    "app/src/panel/driving.rs",
    "app/src/panel/pane.rs",
    "app/src/injected.rs",
    "app/src/injected/links.rs",
    "app/src/injected/search.rs",
    "app/src/links.rs",
    "app/src/links/console.rs",
    "app/src/panel/links.rs",
    "app/src/panel/color.rs",
    "app/src/pointer/links.rs",
    "app/src/measure.rs",
    "app/src/measure/header.rs",
    "app/src/session.rs",
    "app/src/settings.rs",
    "app/src/settings/editor/script.rs",
    "app/src/tests/settings_editor.rs",
    "app/src/fonts/startup.rs",
    "app/src/settings/editor/runtime.rs",
    "app/src/panel/cache.rs",
    "app/src/tab_bar/chrome.rs",
    "app/src/tests/settings_chrome.rs",
    "app/src/settings/editor/view/appearance.rs",
    "app/src/settings/editor/view/colors.rs",
    "app/src/settings/editor/view/tests.rs",
    "app/src/settings/editor/view/footer.rs",
    "app/src/settings/editor/view/tests/usability.rs",
    "app/src/settings/editor/view/typography.rs",
    "app/src/settings/editor/view/tests/layout.rs",
    "app/src/settings/editor/probe.rs",
    "app/src/fonts/selection.rs",
    "app/src/settings/editor.rs",
    "app/src/settings/editor/view.rs",
    "app/src/settings/editor/timing.rs",
    "app/src/settings/document.rs",
    "app/src/settings/store.rs",
    "app/examples/sweep.rs",
    "app/examples/leak.rs",
    "app/examples/fonts.rs",
    "app/examples/screen_demo.rs",
    "app/tests/waits.rs",
    "app/tests/decisions.rs",
    "app/tests/common/mod.rs",
    "app/src/tests/markdown.rs",
];

/// Files covered by the complete citations manifest but not yet declared glossary-clean.
///
/// Together with [`GLOSSARY_CLEAN_FILES`] this partitions [`CITATIONS_CHECKED`]. A file moves from
/// here to the clean list when its retired identifiers are removed; it never disappears from both.
const GLOSSARY_PENDING_FILES: &[&str] = &[
    "app/src/main.rs",
    "app/src/painter/blocks.rs",
    "app/tests/batches.rs",
    "app/tests/prose.rs",
    "app/tests/scripts.rs",
    "app/tests/shape.rs",
    "app/tests/vendor_patch.rs",
];

/// Files whose **citations** are checked, though their identifiers are not yet.
///
/// **Every file of the crate, and that is what closes the class.** The first version listed only the
/// file the delivery was renaming, which left the other direction unguarded: nobody checked what a
/// *sibling* cites **towards** the renamed file. Four citations shipped broken that way — two in
/// `session.rs` sitting two and three lines from citations the same delivery did update, one in
/// `measure.rs`, and one in an example whose comment exists precisely to say «copied from
/// `app::PUMP_BUDGET`; if it changes there, this stops measuring the same thing». It changed there.
///
/// The price is thirty entries in [`NOT_OURS`], because a sibling's comments cite font tables, shell
/// commands and Unicode property names. They are listed once and they stay true.
///
/// **This is a weaker claim than [`FINISHED_CRATES`] and it is a separate list so it cannot be read as the
/// same one.** `app/src/lib.rs` has had its window family renamed and its own locals with it, but it
/// still holds hundreds of Spanish names the glossary never had to decide — the `desde` of a range,
/// the `cual` of an index — and a later delivery takes them. Calling it finished would be a claim the
/// guard cannot back.
///
/// What *can* be claimed today is the one thing renaming breaks and nothing else notices: **a comment
/// that still cites a name the code no longer has**. The compiler does not read comments, the suite
/// does not either, and a reader who greps for the name a comment gives them finds nothing. This
/// delivery renamed a hundred and thirty names in that file; without this, a hundred and fifty-eight
/// comments would have been left pointing at names that no longer exist.
///
/// The second half of each entry is the crate whose names the file may cite: a comment in `lib.rs` is
/// entitled to name something its sibling modules declare.
///
/// **Every file meant every file except two, and neither exception was written down.** The binary was
/// simply missing and is now here. This file is **not**, and that is the answer rather than an
/// oversight: putting it on the list reports **sixty-five** citations as this delivery leaves it — and
/// the number grows with the file, because what it reports is the vocabulary and the shapes this file
/// discusses: retired names (`banderas`, `RatonReclamado`), illustrative ones (`CamelCase`,
/// `like_this`, `Type`) and plain words in backticks. **One of the sixty-five was real.** A guard whose
/// subject is the names that are gone cannot be judged by «cites a name the code has».
///
/// **The one that was real got fixed by reading all sixty-five, and it is worth knowing that happened:**
/// the doc of [`no_comment_cites_a_name_the_code_no_longer_has`] named three live identifiers and one
/// of them, `prioridad`, had left the tree entirely — so the sentence explaining the guard was itself
/// one of the citations it describes. One in sixty-five is not a guard, but it is a good sweep.
///
/// What *can* be enforced here is a **file** name, and [`no_retired_file_name_is_still_cited`] reads
/// this file for exactly that — it found two.
const CITATIONS_CHECKED: &[(&str, &str)] = &[
    ("app/src/painter/pane.rs", "app"),
    ("app/src/ligatures.rs", "app"),
    ("app/src/ligature_tests.rs", "app"),
    ("app/src/fonts/ligatures.rs", "app"),
    ("app/src/painter/ligatures.rs", "app"),
    ("app/src/painter/ligatures/tests.rs", "app"),
    ("app/src/tab_bar/interaction.rs", "app"),
    ("app/src/tests/integrated_chrome.rs", "app"),
    ("app/src/tests/pane_rename.rs", "app"),
    ("app/src/console_input.rs", "app"),
    ("app/src/summary.rs", "app"),
    ("app/src/summary/input.rs", "app"),
    ("app/src/summary/model.rs", "app"),
    ("app/src/summary/view.rs", "app"),
    ("app/src/summary/probe.rs", "app"),
    ("app/src/panel/summary.rs", "app"),
    ("app/src/measure/overview.rs", "app"),
    ("app/src/tests/overview.rs", "app"),
    ("app/src/chrome.rs", "app"),
    ("app/src/header/integrated.rs", "app"),
    ("app/src/pane_view.rs", "app"),
    ("app/src/visibility.rs", "app"),
    ("app/src/lib.rs", "app"),
    ("app/src/alarms.rs", "app"),
    ("app/src/closing.rs", "app"),
    ("app/src/control.rs", "app"),
    ("app/src/control/retiring.rs", "app"),
    ("app/src/panel/document.rs", "app"),
    ("app/src/panel/driving.rs", "app"),
    ("app/src/panel/pane.rs", "app"),
    ("app/src/injected.rs", "app"),
    ("app/src/injected/links.rs", "app"),
    ("app/src/injected/search.rs", "app"),
    ("app/src/links.rs", "app"),
    ("app/src/links/console.rs", "app"),
    ("app/src/panel/links.rs", "app"),
    ("app/src/panel/color.rs", "app"),
    ("app/src/pointer/links.rs", "app"),
    ("app/src/main.rs", "app"),
    ("app/src/settings.rs", "app"),
    ("app/src/settings/editor/script.rs", "app"),
    ("app/src/tests/settings_editor.rs", "app"),
    ("app/src/fonts/startup.rs", "app"),
    ("app/src/settings/editor/runtime.rs", "app"),
    ("app/src/panel/cache.rs", "app"),
    ("app/src/tab_bar/chrome.rs", "app"),
    ("app/src/tests/settings_chrome.rs", "app"),
    ("app/src/settings/editor/view/appearance.rs", "app"),
    ("app/src/settings/editor/view/colors.rs", "app"),
    ("app/src/settings/editor/view/tests.rs", "app"),
    ("app/src/settings/editor/view/footer.rs", "app"),
    ("app/src/settings/editor/view/tests/usability.rs", "app"),
    ("app/src/settings/editor/view/typography.rs", "app"),
    ("app/src/settings/editor/view/tests/layout.rs", "app"),
    ("app/src/settings/editor/probe.rs", "app"),
    ("app/src/fonts/selection.rs", "app"),
    ("app/src/settings/editor.rs", "app"),
    ("app/src/settings/editor/view.rs", "app"),
    ("app/src/settings/editor/timing.rs", "app"),
    ("app/src/settings/document.rs", "app"),
    ("app/src/settings/store.rs", "app"),
    ("app/src/painter/blocks.rs", "app"),
    ("app/src/painter/emoji.rs", "app"),
    ("app/src/fonts.rs", "app"),
    ("app/src/fonts/atlas.rs", "app"),
    ("app/src/header.rs", "app"),
    ("app/src/pointer/header.rs", "app"),
    ("app/src/header/actions.rs", "app"),
    ("app/src/header/controls.rs", "app"),
    ("app/src/header/menu.rs", "app"),
    ("app/src/header/splitting.rs", "app"),
    ("app/src/tests/header_controls.rs", "app"),
    ("app/src/measure.rs", "app"),
    ("app/src/measure/header.rs", "app"),
    ("app/src/panel.rs", "app"),
    ("app/src/panel/buttons.rs", "app"),
    ("app/src/painter.rs", "app"),
    ("app/src/painter/notices.rs", "app"),
    ("app/src/pointer.rs", "app"),
    ("app/src/report.rs", "app"),
    ("app/src/search.rs", "app"),
    ("app/src/tab_bar.rs", "app"),
    ("app/src/pty_output.rs", "app"),
    ("app/src/mouse.rs", "app"),
    ("app/src/opening.rs", "app"),
    ("app/src/session.rs", "app"),
    ("app/src/keyboard.rs", "app"),
    ("app/src/theme.rs", "app"),
    ("app/src/theme/catalog.rs", "app"),
    ("app/src/tiling.rs", "app"),
    ("app/src/tree.rs", "app"),
    ("app/src/watcher.rs", "app"),
    ("app/examples/fonts.rs", "app"),
    ("app/examples/leak.rs", "app"),
    ("app/examples/screen_demo.rs", "app"),
    ("app/examples/sweep.rs", "app"),
    ("app/tests/prose.rs", "app"),
    ("app/tests/shape.rs", "app"),
    ("app/tests/scripts.rs", "app"),
    ("app/tests/batches.rs", "app"),
    ("app/tests/vendor_patch.rs", "app"),
    ("app/tests/waits.rs", "app"),
    ("app/tests/decisions.rs", "app"),
    ("app/tests/common/mod.rs", "app"),
    ("app/src/tests/markdown.rs", "app"),
];

/// How much each shape of the annex has to yield for this test to mean anything.
///
/// **Without a floor this test passes by being empty**, which is the failure mode a guard is most
/// likely to end in: someone reshapes a table in the decisions record, the extractor stops matching,
/// every crate looks clean, and nothing says otherwise. **One floor per shape and not one over the
/// sum**: the tables carry eight names no arrow carries, and a total of 112 losing 8 still clears any
/// floor worth setting. The real counts are comfortably above these; they are here to notice a
/// collapse, not to pin the glossary's size.
const ARROWS_AT_LEAST: usize = 100;
const TABLES_AT_LEAST: usize = 9;

/// How many `` `old.rs`→`new.rs` `` arrows the annex has to still yield, for the same reason as the
/// two above: an extractor that stops matching turns [`no_retired_file_name_is_still_cited`] into a
/// test that passes by looking at nothing. Seventeen is what the annex carries; the floor is here to
/// notice a collapse, not to pin the list.
const FILE_ARROWS_AT_LEAST: usize = 17;

/// Spanish function words, which give away a compound name the glossary never had to name.
///
/// The glossary decides **types**, so on its own it cannot see `base_de`, `mitad_del_motor` or
/// `lo_quiere` — and in the crate that is still to come those are most of what there is. A function
/// word is the safe half of a Spanish lexicon: it is short, it is closed, and it is checked as a
/// **whole segment**, so `serde` is not `de` and `config` is not `con`.
///
/// **`sin` is deliberately absent**: it is also the English trigonometric function, and a guard that
/// breaks the build has to be wrong less often than the code it judges.
const FUNCTION_WORDS: &[&str] = &[
    "de", "del", "la", "el", "los", "las", "lo", "que", "con", "por", "para", "una", "uno", "al",
    "mas", "ya",
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two levels above this crate")
}

/// The Spanish side of the glossary, the English side, and how many came from each shape.
///
/// The English side matters because de-pluralising an identifier can land it on a Spanish entry by
/// accident — `Series` becomes `serie`, and `Serie` is precisely the name it replaced. A segment that
/// *is* an English glossary name is never a leftover, so it is exempt before anything else is asked.
///
/// **The counts are returned per shape and not as one total**, which is the lesson the previous
/// delivery paid for: a single floor over the sum hides the loss of a whole shape. Measured — losing
/// both tables drops the total from 112 to 104 and a floor of 100 never notices.
struct Glossary {
    spanish: HashSet<String>,
    english: HashSet<String>,
    from_arrows: usize,
    from_tables: usize,
}

// **Folding is what closes a whole family of leftovers here**: the glossary writes `Pestaña` and
// `TamañoPedido`, and a Spanish identifier left in the code would be written `pestanas` or
// `tamano_pedido`, because identifiers in this repository avoid the tilde on purpose. Without
// `fold` the two never meet. It lives in `common` because `prose` needs the same one, and two copies
// are two chances to disagree.

fn glossary() -> Glossary {
    let annex = DECISIONS
        .split_once("### The glossary, and the two borders that order it")
        .expect("the decisions record no longer carries the glossary annex")
        .1;

    let mut spanish = HashSet::new();
    let mut english = HashSet::new();
    let mut from_arrows = 0;
    let mut from_tables = 0;

    // Shape 1, and the bulk of it: `Spanish`→`English`, sometimes with a parenthesis in between and
    // sometimes with the English name in bold.
    for line in annex.lines() {
        let mut rest = line;
        while let Some((before, after)) = rest.split_once('→') {
            if let (Some(a), Some(b)) = (last_backticked(before), first_backticked(after)) {
                spanish.insert(fold(a));
                english.insert(fold(b));
                from_arrows += 1;
            }
            rest = after;
        }
    }

    // Shape 2: the two tables whose first column is Spanish **by their own heading**. Keying off the
    // heading is what makes this safe to widen: the `Stored*` table also has a name in its first
    // column, but that name is already English, and reading it as Spanish would break the build for
    // every crate that adopted it.
    for heading in [
        "| Type | First name, and why it falls",
        "| Today | What it is",
    ] {
        // **The heading row is looked up WHOLE**, not with `split_once` over the text: splitting on
        // the prefix leaves half a row — from «| Ends up |» onwards — and the column index taken from
        // that half points at the first column, which is the Spanish one. The first version of this
        // put the Spanish names into the **English** set, exempting them: worse than the defect it
        // came to fix. Its own `reparto` test case caught it.
        let mut rows = annex
            .lines()
            .skip_while(|line| !line.starts_with(heading))
            .peekable();
        let head: Vec<&str> = rows
            .peek()
            .unwrap_or_else(|| {
                panic!(
                    "the glossary annex no longer has a table headed «{heading}». If it was \
                     translated or reshaped, this anchor has to move with it — skipping the table \
                     instead would leave the guard reporting every crate clean with eight names \
                     missing from its vocabulary"
                )
            })
            .split('|')
            .collect();
        // Which column holds the adopted name is read off the heading rather than counted by hand: in
        // one of these two tables the first backticked token after column 1 is the **discarded** name,
        // and in the other the last one is a Spanish name quoted inside the prose.
        let kept = head
            .iter()
            .position(|c| c.trim() == "Ends up")
            .unwrap_or_else(|| {
                panic!("the table headed «{heading}» no longer has an «Ends up» column")
            });

        for line in rows.skip(1) {
            if !line.starts_with('|') {
                break;
            }
            let cells: Vec<&str> = line.split('|').collect();
            if let (Some(a), Some(b)) = (
                first_backticked(cells.get(1).unwrap_or(&"")),
                cells.get(kept).and_then(|c| first_backticked(c)),
            ) {
                spanish.insert(fold(a));
                english.insert(fold(b));
                from_tables += 1;
            }
        }
    }

    Glossary {
        spanish,
        english,
        from_arrows,
        from_tables,
    }
}

fn first_backticked(text: &str) -> Option<&str> {
    let (_, after) = text.split_once('`')?;
    let (inside, _) = after.split_once('`')?;
    (!inside.is_empty() && !inside.contains(' ')).then_some(inside)
}

fn last_backticked(text: &str) -> Option<&str> {
    let end = text.rfind('`')?;
    let start = text[..end].rfind('`')?;
    let inside = &text[start + 1..end];
    (!inside.is_empty() && !inside.contains(' ')).then_some(inside)
}

/// The identifiers of a Rust file, with prose and literals removed and test names skipped.
///
/// Written by hand rather than pulled in as a dependency: the whole job is «which characters are
/// code», and a parser would be a new package in the tree for the sake of four states.
fn identifiers(src: &str) -> Vec<(usize, String)> {
    #[derive(PartialEq)]
    enum Where {
        Code,
        LineComment,
        BlockComment,
        Text,
    }

    let mut found = Vec::new();
    let bytes: Vec<char> = src.chars().collect();
    let mut place = Where::Code;
    // **Rust block comments nest**, and leaving at the first `*/` bites both ways: the prose after it
    // reads as code — a false positive breaks the build — and a `#[test]` living in there switches off
    // the next function's name. Both measured.
    let mut comment_depth = 0usize;
    let mut line = 1usize;
    let mut i = 0usize;
    let mut word = String::new();
    let mut word_line = 1usize;
    // Set by the attribute `#[test]`, cleared by the next `fn`.
    //
    // **It used to skip that function's name, and it does not any more.** Test names were the one
    // prose this guard was told to walk past, because they were Spanish while production was being
    // renamed and judging them would have broken the build on work that had not been done yet.
    // The delivery that translated the 366 of `lib.rs` is what retires the exemption: removing it
    // leaves every file of the crate green, so the skip now protects nothing and hides the class of
    // name this milestone is most likely to see come back.
    //
    // **It still has to match the attribute and not the bare word.** The first version raised this
    // flag on any `test`, and `#[cfg(test)]` opens every test module in this repository — so the
    // first function of each one had its name skipped, silently. A mutation found it.
    let mut inside_a_test_attribute = false;
    let mut previous_word = String::new();

    while i < bytes.len() {
        let c = bytes[i];
        if c == '\n' {
            line += 1;
        }
        match place {
            Where::LineComment => {
                if c == '\n' {
                    place = Where::Code;
                }
                i += 1;
                continue;
            }
            Where::BlockComment => {
                if c == '/' && bytes.get(i + 1) == Some(&'*') {
                    comment_depth += 1;
                    i += 2;
                    continue;
                }
                if c == '*' && bytes.get(i + 1) == Some(&'/') {
                    comment_depth -= 1;
                    if comment_depth == 0 {
                        place = Where::Code;
                    }
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            Where::Text => {
                if c == '\\' {
                    // **The line is counted by hand here.** Skipping two characters also skips the
                    // increment at the top of the loop, and a `\\` + newline continuation is exactly
                    // that: the number drifted by one line per earlier continuation in the file, and
                    // this repository breaks almost every assertion message with a `\\`.
                    if bytes.get(i + 1) == Some(&'\n') {
                        line += 1;
                    }
                    i += 2;
                    continue;
                }
                if c == '"' {
                    place = Where::Code;
                }
                i += 1;
                continue;
            }
            Where::Code => {}
        }

        if c == '/' && bytes.get(i + 1) == Some(&'/') {
            place = Where::LineComment;
            i += 2;
            continue;
        }
        if c == '/' && bytes.get(i + 1) == Some(&'*') {
            place = Where::BlockComment;
            comment_depth = 1;
            i += 2;
            continue;
        }
        // A raw string, and it has to be its own case: this repository writes its JSON fixtures as
        // `r#"…"#`, and JSON is full of quotes. Treating one as an ordinary string ends it at the
        // first quote **inside** the JSON and hands the rest of the fixture over as if it were code.
        if c == 'r' && matches!(bytes.get(i + 1), Some('#') | Some('"')) {
            let mut hashes = 0usize;
            while bytes.get(i + 1 + hashes) == Some(&'#') {
                hashes += 1;
            }
            if bytes.get(i + 1 + hashes) == Some(&'"') {
                let mut j = i + 2 + hashes;
                while j < bytes.len() {
                    if bytes[j] == '"' && (0..hashes).all(|k| bytes.get(j + 1 + k) == Some(&'#')) {
                        break;
                    }
                    if bytes[j] == '\n' {
                        line += 1;
                    }
                    j += 1;
                }
                i = j + 1 + hashes;
                continue;
            }
        }
        if c == '"' {
            place = Where::Text;
            i += 1;
            continue;
        }
        // A character literal, told apart from a lifetime by what closes it. Without this the CJK
        // load and the wide-character assertions read as identifiers, and the not-even-ASCII half of
        // the guard fires on the test data it was never meant to judge.
        if c == '\'' {
            let escaped = bytes.get(i + 1) == Some(&'\\');
            let closes = if escaped { None } else { bytes.get(i + 2) };
            if escaped || closes == Some(&'\'') {
                let mut j = i + 1;
                while j < bytes.len() {
                    if bytes[j] == '\\' {
                        j += 2;
                        continue;
                    }
                    if bytes[j] == '\'' {
                        break;
                    }
                    j += 1;
                }
                i = j + 1;
                continue;
            }
        }

        if c == '#' {
            let ahead: String = bytes[i..bytes.len().min(i + 16)].iter().collect();
            if ahead.starts_with("#[test]") || ahead.starts_with("#[should_panic") {
                inside_a_test_attribute = true;
            }
            i += 1;
            continue;
        }

        if c.is_alphanumeric() || c == '_' {
            if word.is_empty() {
                word_line = line;
            }
            word.push(c);
            i += 1;
            continue;
        }

        if !word.is_empty() {
            let done = std::mem::take(&mut word);
            if previous_word == "fn" && inside_a_test_attribute {
                inside_a_test_attribute = false;
            }
            found.push((word_line, done.clone()));
            previous_word = done;
        }
        i += 1;
    }
    if !word.is_empty() {
        found.push((word_line, word));
    }
    found
}

/// `mouse_reporting` → `mouse`, `reporting`; `CombiningSpan` → `combining`, `span`;
/// `DEFAULT_WIDTH` → `default`, `width`.
///
/// **A piece with no lowercase letter in it is one word, and getting that wrong made this guard blind
/// to every constant in the repository.** Splitting before *each* capital left a screaming-snake-case
/// name as a row of single letters — a name of fourteen letters became fourteen of them — and every
/// one was then dropped by the two-character floor in [`why_it_is_a_leftover`], so such a name could
/// carry any Spanish word at all and no rule was ever asked about it. It had been that way since the
/// delivery that declared the first three crates finished, which is what makes it worth this much
/// prose: the hole was not in a rule, it was upstream of all of them. Measured when it was closed:
/// three constants in a module already declared finished, and **nothing** in the three finished crates
/// — those really were clean.
///
/// **The test for a lowercase letter is the whole rule, and a case-change boundary was tried first and
/// thrown away.** Reading `PTYOutput` as `pty` + `output` instead of `p`, `t`, `y`, `output` is nicer
/// to look at and changes no verdict: the letters it saves are dropped by the same floor, and the word
/// after them is found either way. No name in the tree tells the two rules apart, and this file does
/// not keep a mechanism with no scenario.
fn segments(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for piece in name.split('_') {
        let shouting = !piece.chars().any(|c| c.is_lowercase());
        let mut current = String::new();
        for c in piece.chars() {
            if c.is_uppercase() && !current.is_empty() && !shouting {
                out.push(std::mem::take(&mut current));
            }
            current.extend(c.to_lowercase());
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    out
}

/// The `.rs` files under a path, which may be a directory or a single file.
///
/// **It panics if it cannot read it.**
///
/// Returning an empty list was a silence of the expensive kind: a misspelled name in the list of
/// finished crates left this guard with no file to look at, and a crate with no files reads exactly
/// like a clean one. Measured: turning one hyphen into an underscore in a crate name left all four
/// tests green.
/// Everything this milestone has declared clear of the glossary, whole crates and single files alike.
///
/// **The two lists are walked by one function so they cannot drift apart.** Adding a guard that reads
/// only one of them is the shape of hole this file keeps finding: a list that looks watched and is not.
/// The label comes back with the files so a failure can name what was declared, not just the path.
fn guarded_targets(root: &Path) -> Vec<(String, Vec<PathBuf>)> {
    // The citations list already proves that it names every file in the crate. Partitioning that
    // complete list makes a clean file impossible to drop silently: it must remain clean or move to
    // the explicit pending set. A length floor only notices some removals and becomes stale as the
    // list grows.
    let clean: HashSet<&str> = GLOSSARY_CLEAN_FILES.iter().copied().collect();
    let pending: HashSet<&str> = GLOSSARY_PENDING_FILES.iter().copied().collect();
    let citations: HashSet<&str> = CITATIONS_CHECKED.iter().map(|(path, _)| *path).collect();
    assert_eq!(
        clean.len(),
        GLOSSARY_CLEAN_FILES.len(),
        "the glossary-clean list contains a duplicate path"
    );
    assert_eq!(
        pending.len(),
        GLOSSARY_PENDING_FILES.len(),
        "the glossary-pending list contains a duplicate path"
    );
    assert!(
        clean.is_disjoint(&pending),
        "a file cannot be both glossary-clean and pending"
    );
    let classified: HashSet<&str> = clean.union(&pending).copied().collect();
    assert_eq!(
        classified, citations,
        "every citation-checked file must be classified as glossary-clean or pending"
    );

    // The remaining floors protect the two independent claims from collapsing to an empty or tiny
    // list before their own completeness checks run.
    assert!(
        FINISHED_CRATES.len() >= 4 && CITATIONS_CHECKED.len() >= 33,
        "the remaining guarded lists have {} crates and {} citation entries: they only ever grow, \
         so this is one of them being emptied and every check downstream going quiet",
        FINISHED_CRATES.len(),
        CITATIONS_CHECKED.len()
    );
    let mut out = Vec::new();
    for crate_name in FINISHED_CRATES {
        out.push((
            (*crate_name).to_string(),
            rust_files(&root.join("crates").join(crate_name)),
        ));
    }
    for path in GLOSSARY_CLEAN_FILES {
        let file = root.join("crates").join(path);
        out.push((
            (*path).to_string(),
            if file.is_file() {
                vec![file]
            } else {
                Vec::new()
            },
        ));
    }
    out
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).unwrap_or_else(|failure| {
        panic!(
            "cannot read {}: {failure}. If that is a name from the finished-crates list, it is \
             misspelled — and without this panic the guard would call clean a crate it never read",
            dir.display()
        )
    });
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// The extractor is still finding the glossary — **counted per shape, not in total**.
///
/// A floor over the sum is the disguise this project has on file: a number with which the defect and
/// the fix give the same answer. Measured — knocking both tables out of shape drops the total from 112
/// to 104, and a floor of 100 waves it through while eight names quietly leave the vocabulary.
#[test]
fn the_glossary_is_still_where_this_test_reads_it_from() {
    let glossary = glossary();
    assert!(
        glossary.from_arrows >= ARROWS_AT_LEAST,
        "the arrow shape `Spanish`→`English` yielded {} pairs and there are at least \
         {ARROWS_AT_LEAST}: the annex writes them some other way now, and this guard is running on a \
         vocabulary it no longer has",
        glossary.from_arrows
    );
    assert!(
        glossary.from_tables >= TABLES_AT_LEAST,
        "the two tables with a «Ends up» column yielded {} rows and there are at least \
         {TABLES_AT_LEAST}: their eight names are the ones no arrow in the annex carries, so losing \
         them is invisible in any total",
        glossary.from_tables
    );
}

/// Why this one identifier is a leftover, or `None` if it is not one.
///
/// Split out from the walk so the rules can be asked directly, without a crate to point it at: some
/// of the answers below are only ever reached by names that do not exist in the tree yet, and a rule
/// with no oracle is the same thing as no rule.
fn why_it_is_a_leftover(name: &str, glossary: &Glossary) -> Option<String> {
    // An identifier with a letter outside ASCII cannot be English. It is the cheapest half of this
    // guard and it needs no glossary at all.
    if name.chars().any(|c| c.is_alphabetic() && !c.is_ascii()) {
        return Some(format!("`{name}` is not even ASCII"));
    }
    let pieces = segments(name);

    // **The compound rule, and it was the biggest hole in the first version.** The glossary stores
    // `RatonReclamado` as one token, `ratonreclamado`; the identifier splits into «raton» + «reclamado»
    // and neither piece is in the set, so the two never met. Eighteen of the twenty compound names
    // survived **by luck**, because they happen to carry a particle the function words catch
    // (`DeLaBarra`, `RestoDelFotograma`, `EnElAtlas`…); the two without one — `RatonReclamado` and
    // `EnPuntos` — walked straight through a crate declared finished. Measured.
    //
    // Gluing the pieces back together is what fixes it, and it is done this way rather than by adding
    // each piece to the Spanish set: `CosteDelColor` would have put `color` in there, and `color` is
    // an English identifier this repository uses everywhere.
    // No English exemption here, unlike the one below: no name the glossary adopts has a glued form
    // that collides with a Spanish one, and removing the exemption broke nothing — so it was a
    // mechanism with no scenario. If one ever shows up, this test will say so loudly.
    let joined = fold(&pieces.join(""));
    if glossary.spanish.contains(&joined) {
        return Some(format!("`{name}` is a compound the glossary retired"));
    }

    // A single-piece name cannot be a compound, and there a lone function word is almost always
    // English — the `la` of a matrix, the `el` of an index. The check only applies where it means
    // something: inside a compound.
    let compound = pieces.len() > 1;
    for piece in &pieces {
        let segment = fold(piece);
        if glossary.english.contains(&segment) {
            continue;
        }
        if compound && FUNCTION_WORDS.contains(&segment.as_str()) {
            return Some(format!("`{name}` is built with the Spanish «{piece}»"));
        }
        if segment.len() <= 2 {
            continue;
        }
        // **Both plural endings, and the trailing digits.** Trying `es` and giving up left `banderas`
        // uncaught; trying `s` alone left `divisiones` uncaught; and not stripping digits left `celda2`
        // uncaught while `celda_2` fell — Spanish uses both endings and this repository numbers its
        // fixtures, so all of them are asked. The order used to matter and that was a bug: with `es`
        // first, `Series` became `seri` and never landed on `Serie`, which made the English exemption
        // above look load-bearing when in fact nothing reached it.
        let bare = segment.trim_end_matches(|c: char| c.is_ascii_digit());
        let forms = [
            Some(segment.as_str()),
            segment.strip_suffix('s'),
            segment.strip_suffix("es"),
            (bare != segment.as_str()).then_some(bare),
        ];
        if forms
            .iter()
            .flatten()
            .any(|form| glossary.spanish.contains(*form))
        {
            return Some(format!("`{name}` still carries «{piece}»"));
        }
    }
    None
}

/// The three rules, asked directly, with the cases the tree cannot supply.
///
/// **The English exemption exists for names that have not arrived yet**, and without this test it
/// would be code with nothing judging it: removing it leaves every crate green today. It is needed
/// the moment the measurement module is translated — de-pluralising `Series` gives `serie`, and
/// `Serie` is exactly the name it replaced.
/// The names [`the_three_rules_answer_what_they_promise`] uses as **positive** cases.
///
/// They live here rather than only inside the test so the guard above them can walk them. **That guard
/// used to point at the exemption list**, which is gone — it had nothing left to exempt — and the list
/// was very nearly deleted with it. It is not, and the reason is the whole point: **the property it
/// defended did not disappear, it moved.**
///
/// What it defends is that every positive case still exercises **its own rule** instead of leaving
/// through a shortcut. The exemption list was one shortcut; the other is still there and is derived,
/// not written — [`why_it_is_a_leftover`] gives up on a piece the moment the glossary's **English**
/// side claims it. That side is extracted from the decisions record, which the deliveries still to come
/// edit. The day an arrow adopts `celda`, `reparto` or `maqueta` as English, the case carrying that
/// piece stops testing its rule and the suite stays green — which is the disguise this file has on
/// record three times over (`color_del_reclamo`, `pestanas`, `gesto`).
const POSITIVE_CASES: &[&str] = &[
    "Celda",
    "celda_de_arriba",
    "CeldaDeArriba",
    "banderas",
    "base_de",
    "lo_quiere",
    "que_pestaña",
    "RatonReclamado",
    "en_puntos",
    "tamano_pedido",
    "pestanas",
    "celda2",
    "ANCHO_DE_FABRICA",
    "CONSOLA_MINIMA",
    "CONSOLA2_MINIMA",
    "USO_ALTO",
    "CELDA_ALTA",
    "reparto",
    "maqueta",
    "Resultado",
    "color_del_borde",
];

#[test]
fn the_three_rules_answer_what_they_promise() {
    let glossary = glossary();
    let verdict = |name: &str| why_it_is_a_leftover(name, &glossary);
    // **This goes first on purpose.** Every name this test uses as a POSITIVE case has to stay outside
    // the shortcut that is still live: the English side of the glossary, which comes from the annex and
    // which the prose deliveries still edit. The day an arrow adopts one of these names as its English,
    // its case stops checking its rule and leaves through the shortcut. It goes before the cases so the
    // message names **the cause** and not the symptom, which would be thirty lines further down.
    // **And a floor, because without it this loop passes by being empty.** Measured: emptying the list
    // leaves the case green without checking a single subject, which is the exact way the guards of this
    // file die. There are twenty-one subjects and the list only ever grows.
    assert!(
        POSITIVE_CASES.len() >= 21,
        "the positive-subjects list has {} entries: someone emptied or trimmed it, and this case \
         stopped looking at anything",
        POSITIVE_CASES.len()
    );
    for case in POSITIVE_CASES {
        // **Every subject has to stay a genuine leftover**, which is what makes it a positive case. An
        // entry that stopped being one would sit here defending no rule at all.
        assert!(
            verdict(case).is_some(),
            "«{case}» is in the positive-cases list and no rule catches it any more"
        );
        let silenced = segments(case)
            .iter()
            .any(|piece| glossary.english.contains(&fold(piece)));
        assert!(
            !silenced,
            "«{case}» is a positive case of this test and the glossary now claims one of its pieces as \
             English: the case leaves through that shortcut and asserts nothing. Pick another subject \
             for the rule, or the rule stops being tested"
        );
    }

    // Retired vocabulary, as a whole name and as a piece of a compound.
    assert!(verdict("Celda").is_some(), "a retired type name");
    assert!(
        verdict("celda_de_arriba").is_some(),
        "a retired name inside a compound"
    );
    // **The same name in CamelCase, which is a different code path**: snake_case is split by the
    // underscore and needs no case rule at all, so without this the whole case-splitting half of
    // [`segments`] could stop working and every assertion here would stay green.
    assert!(
        verdict("CeldaDeArriba").is_some(),
        "the same compound written in CamelCase"
    );
    assert!(
        verdict("banderas").is_some(),
        "a retired name in the plural"
    );
    // Spanish grammar, which the glossary never had to name.
    assert!(verdict("base_de").is_some(), "a compound built with «de»");
    assert!(verdict("lo_quiere").is_some(), "a compound built with «lo»");
    // Anything outside ASCII.
    assert!(
        verdict("que_pestaña").is_some(),
        "a name with a letter outside ASCII"
    );
    // **The compound names, which the glossary stores glued together.** Two of the twenty carry no
    // Spanish particle at all, so the function words above cannot see them either: these two are the
    // whole reason the pieces get glued back before asking.
    assert!(
        verdict("RatonReclamado").is_some(),
        "a compound with no Spanish particle in it"
    );
    assert!(
        verdict("en_puntos").is_some(),
        "the other one, in snake_case"
    );
    // The tilde the code drops on purpose, and the digit a fixture adds.
    assert!(
        verdict("tamano_pedido").is_some(),
        "a compound the glossary writes with a tilde and the code writes without one"
    );
    assert!(verdict("pestanas").is_some(), "the same, in the plural");
    assert!(
        verdict("celda2").is_some(),
        "a retired name with a digit stuck on the end"
    );
    // **A run of capitals is one word, and until this was fixed the guard could not see a single
    // constant.** Splitting before every capital left these names as rows of single letters, and the
    // two-character floor then dropped every one of them: no rule was ever asked. Both halves are
    // asserted because they fail for different reasons — the first needs the function words to see
    // «de», the second needs the retired vocabulary to see «consola» — and a segmentation that broke
    // only one of them would still look half right. Revert [`segments`] and both of these go green
    // while three constants of a finished module walk through.
    assert!(
        verdict("ANCHO_DE_FABRICA").is_some(),
        "a screaming-snake-case name built with the Spanish «de»"
    );
    assert!(
        verdict("CONSOLA_MINIMA").is_some(),
        "a screaming-snake-case name carrying a retired noun"
    );
    // **And a numbered constant, because «no lowercase letter» is not «every letter uppercase».** Written
    // with `chars().all(char::is_uppercase)` the digit breaks the condition, the piece is split back into
    // single letters, and the defect this closes is reopened for every constant with a number in it.
    // Measured: that version leaves the eight tests green without this case.
    assert!(
        verdict("CONSOLA2_MINIMA").is_some(),
        "a numbered constant: «no lowercase letter» is not «every letter uppercase»"
    );
    // **The two-character floor, which nothing held up until this delivery.** Measured: raising it to 3 or 5
    // —or removing it— left the eight tests green, and at 5 the guard stops seeing `celda`, `hueco`,
    // `serie`, `nodo`, `fila` and `mitad` inside any compound. Two cases because each one kills a
    // different value: a three-letter name and a five-letter one.
    assert!(
        verdict("USO_ALTO").is_some(),
        "a three-letter retired name, which a floor of three would skip"
    );
    assert!(
        verdict("CELDA_ALTA").is_some(),
        "a five-letter retired name, which a floor of five would skip"
    );
    // The three names that live only in the table with a «Ends up» column, which no arrow carries.
    assert!(
        verdict("reparto").is_some(),
        "a name only the Layout table names"
    );
    assert!(verdict("maqueta").is_some(), "another one");
    assert!(
        verdict("Resultado").is_some(),
        "a name only the calque table names"
    );

    // And what must NOT be reported.
    assert_eq!(
        verdict("Series"),
        None,
        "«Series» is the English name the glossary chose, and de-pluralising it lands on «Serie», \
         the very name it replaced: without the English exemption this guard would refuse the \
         rename it is supposed to be enforcing"
    );
    assert_eq!(
        verdict("Timings"),
        None,
        "another English glossary name in the plural"
    );
    assert_eq!(
        verdict("el"),
        None,
        "a lone function word is not a compound: `el` on its own is an English identifier far more \
         often than a Spanish article"
    );
    assert_eq!(
        verdict("panel_vacio"),
        None,
        "the declared blind spot, kept honest by asserting it"
    );
    // The three names the calque table adopts. They used to pass by luck: the extractor was taking the
    // **discarded** name out of that table, so `Outcome`, `PtyOutput` and `MouseReport` were never in
    // the English set at all and only survived because no segment of theirs collided.
    for adopted in [
        "Outcome",
        "PtyOutput",
        "MouseReport",
        "Pickup",
        "Tiling",
        "CombiningSpan",
    ] {
        assert_eq!(
            verdict(adopted),
            None,
            "«{adopted}» is the name the glossary adopts, and it has to be exempt by the English \
             side and not by chance"
        );
    }
    // **Gluing the pieces must not have widened the Spanish side piece by piece.** Adding each piece
    // of every compound would have put `color` in there, out of `CosteDelColor` — and `color` is an
    // English identifier this repository uses everywhere.
    // The other side of the rule: an English constant stays English. Without these, «a piece with no
    // lowercase letter is one word» could be satisfied by never splitting a constant at all.
    for shouting in [
        "DEFAULT_WIDTH",
        "MAX_CHARACTERS",
        "ATLAS_SIDE",
        "SIZE_TO_ASK_WITH",
    ] {
        assert_eq!(
            verdict(shouting),
            None,
            "«{shouting}» is English, and segmenting a run of capitals must not invent a Spanish \
             piece out of it"
        );
    }
    for english_word in ["color", "colors", "cost", "atlas", "points", "size"] {
        assert_eq!(
            verdict(english_word),
            None,
            "«{english_word}» is a piece of a Spanish compound in the glossary and an English word \
             on its own: gluing has to be by the whole name, never piece by piece"
        );
    }
    // And the same name **as a Spanish compound** is still caught, so the rule above is not a hole.
    //
    // **The subject used to be `color_del_reclamo` and had to be changed**, which is worth keeping: a
    // delivery put that very name on the exemption list this file used to have, and the case went on
    // passing while asserting nothing. The list is gone, so nothing can silence a case that way today
    // — but `color_del_borde` is the subject because of it.
    assert!(
        verdict("color_del_borde").is_some(),
        "a Spanish compound whose first piece happens to be an English word"
    );
}

#[test]
fn no_spanish_identifier_survives_in_a_finished_crate() {
    let glossary = glossary();
    let root = repo();
    let mut leftovers = Vec::new();

    for (what, files) in guarded_targets(&root) {
        assert!(
            !files.is_empty(),
            "«{what}» is declared finished and has not a single `.rs` file to look at: either it is \
             gone, or its name on the list is misspelled. Either way this guard would be calling clean \
             something it never read"
        );
        for file in files {
            let shown = file
                .strip_prefix(&root)
                .unwrap_or(&file)
                .display()
                .to_string();
            let src = std::fs::read_to_string(&file).expect("a file the walk just listed");

            for (line, name) in identifiers(&src) {
                if let Some(why) = why_it_is_a_leftover(&name, &glossary) {
                    leftovers.push(format!("  {shown}:{line}: {why}"));
                }
            }
        }
    }

    assert!(
        leftovers.is_empty(),
        "{} identifier(s) in code this milestone has already declared clear of the glossary are \
         still using a name it retired. Either the rename was left half done, or the glossary changed \
         its mind and nobody told the code:\n{}",
        leftovers.len(),
        leftovers.join("\n")
    );
}

#[test]
fn no_file_of_a_finished_crate_is_still_named_in_spanish() {
    let spanish = glossary().spanish;
    let root = repo();
    let mut leftovers = Vec::new();

    for (what, files) in guarded_targets(&root) {
        assert!(!files.is_empty(), "«{what}» has no files to look at");
        for file in files {
            let name = file
                .file_name()
                .expect("a path the walk built from a directory entry")
                .to_string_lossy()
                .to_string();
            if spanish.contains(&fold(&name)) {
                let shown = file.strip_prefix(&root).unwrap_or(&file).display();
                leftovers.push(format!("  {shown}"));
            }
        }
    }

    assert!(
        leftovers.is_empty(),
        "{} file(s) this guard reads still carry the Spanish name the glossary retired, and the \
         glossary lists the name they should have:\n{}",
        leftovers.len(),
        leftovers.join("\n")
    );
}

/// Names a comment may cite that are **not** identifiers of the crate and never will be.
///
/// This one list is copied rather than derived, and there is no source to derive it from: a program
/// the prose names (`vim`), a constant of the system (`OPOST`), an item of a dependency
/// (`LineWriter`) and a method of `std` (`recv`) are all outside every crate in this tree. Keeping
/// it closed is what makes the guard below safe to break the build with — and the failure message
/// says so, because the fix for a genuine foreign name is to add it here, not to widen the rule.
const NOT_OURS: &[&str] = &[
    // **Standard-library waits, not names of this tree.** `waits.rs` cites the ones it judges and the
    // ones it deliberately does not.
    "wait_while",
    "wait_timeout_while",
    "recv_deadline",
    "accept",
    // **terminfo capability names and the escape they describe.** The keyboard cites the entries of
    // the `TERM` it advertises to say which sequence each key sends; they are names in
    // `xterm-256color`'s terminfo entry, not identifiers of this tree.
    "smkx",
    "kich1",
    "kdch1",
    "kpp",
    "knp",
    "khome",
    "kend",
    "kcuu1",
    "kcub1",
    "SS3",
    // The curses call that sends `smkx`: what makes a program ask for application cursor keys.
    "keypad",
    // **The three verbs of the control channel's wire format.** They are names on a wire and not
    // identifiers of the app crate: the channel's crate is what declares them, and the app cites
    // them where it explains what it does with each one.
    "split_open",
    "split_read",
    "split_close",
    // **And the read verb, which is a name on the wire and nowhere else.** Its two siblings are
    // functions of the channel's crate —`panel_set` and `panel_patch` both have one, because each
    // has an envelope to read— and this one takes no arguments at all, so there is nothing for a
    // function to read and no identifier for the guard to find.
    "panel_get",
    // **A field of the panel document, cited where the channel explains what it does with it.** The
    // contract's crate declares it; the channel's crate carries the bytes and never names a type for
    // them, which is the whole point of borrowing them raw.
    "updated_at",
    // A cargo feature of `serde_json`, named where this crate explains what NOT having it on costs:
    // without it a map is sorted, which silently reordered a document's keys on the way back.
    "preserve_order",
    // **The kernel's own names for what a command line may cost.** `execve` is where a request's
    // `program` and `args` end up, and `E2BIG` is what it answers when the argument list is past what
    // the system takes — cited where the wire bounds them, and `ARG_MAX` where that bound is sized.
    // `MAX_ARG_STRLEN` is the ceiling on ONE argument, cited where the bench publisher explains why
    // its document arrives on standard input.
    "execve",
    "E2BIG",
    "ARG_MAX",
    "MAX_ARG_STRLEN",
    // The batch letter the publisher example is written for. A measurement batch is named in
    // `measure-frame.sh`, never in Rust.
    "S",
    // Traits of the standard library and of `serde`, cited where this crate leans on what they
    // produce rather than deciding it again.
    "Display",
    "Serialize",
    // The operating system's answer when a process is out of descriptors, cited where the acceptor
    // says why it does not treat every accept failure as fatal.
    "EMFILE",
    // **Unicode's own names for what a button's text may not carry.** `Cc` is the category of the
    // control characters the kernel's line discipline acts on, `Cf` the format ones that are painted
    // as nothing or paint the rest backwards, and `DEL` the one control that is not in the C0 range.
    // They are cited where the contract explains why a button carrying one is dropped rather than
    // repaired.
    "Cc",
    "Cf",
    "DEL",
    // Other programs, and what the operating system gives them.
    "bash",
    // The one that writes for ever in two-byte lines, which is why the leak bench uses it.
    "yes",
    "cat",
    "clear",
    "ECHO",
    "ECHOCTL",
    "emacs",
    "exec",
    "exit",
    "fork",
    "htop",
    // The two tools `vendor_patch.rs` names when arguing for the check it chose over the other.
    "diff",
    "rustfmt",
    // Kitty's setting for what a console paints when it is not the one being typed into, and the
    // value it ships as its default. They are cited where this crate follows them instead of
    // deciding the same thing again.
    "cursor_shape_unfocused",
    "hollow",
    // egui's own method for putting a shape on the queue, cited where a test explains that it does
    // **not** drop a shape whose clip is empty — the shape arrives complete and the tessellator is
    // what skips it, which is how a mark can be asserted in full and drawn nowhere.
    "add",
    // Alacritty's own type for the cursor a frame is asked to draw, and its terminal mode for
    // whether the cursor is shown. Both are cited where this crate explains that the blink bit is
    // independent of them: the shape comes out `Hidden` under `CSI ?25l` and the style is not
    // touched, which is why a repaint deadline armed from the program's request would wake the
    // window to paint nothing.
    "RenderableCursor",
    "SHOW_CURSOR",
    // Alacritty's method for the shape door and the standard-library method it reaches the default
    // through, both cited where this crate pins that picking a shape by OSC inherits the blink this
    // terminal defaults to.
    "set_cursor_shape",
    "get_or_insert",
    // Alacritty's own name for entering and leaving the alternate screen, cited where this crate
    // pins that the trip does not hand the cursor's style back: what it swaps is the grid, the
    // keyboard modes and the selection, never the style the blink lives in.
    "swap_alt",
    // The two survivors this crate's closing path is written against: a program that puts itself in
    // a session of its own, and the one that is *named* for outliving the shell that launched it.
    "setsid",
    "nohup",
    // What hanging a program up is written against: the system call that reaps it, the Windows call
    // `portable-pty` kills with, and the shell builtin a test uses to make a program deaf to it.
    "waitpid",
    "TerminateProcess",
    "trap",
    // The programs D56 measured against it: `sudo` accepts the hang-up, `pkexec` makes it refused.
    "sudo",
    // The kernel constants and tools this tree names but does not own. `MAX_CANON` is here to be
    // *contradicted*: it is 255, and the 4 096 that bounds a canonical line is `N_TTY_BUF_SIZE`,
    // a compile-time constant of the `n_tty` driver. A comment that could not name the wrong one
    // could not correct it.
    "ICANON",
    "ICRNL",
    "ISIG",
    "MAX_CANON",
    "conhost",
    "getconf",
    "ioctl",
    "less",
    "ls",
    "man",
    "nano",
    "nvim",
    "readline",
    "N_TTY_BUF_SIZE",
    "n_tty",
    "O_APPEND",
    "OPOST",
    "printf",
    "SIGKILL",
    "sh",
    "strace",
    "stty",
    "termios",
    "tmux",
    "vim",
    "xterm",
    "zle",
    // Environment variables — ours and other people's. A variable is not an identifier of this tree,
    // and the ones the measurement script exports leave with the script, in a later delivery.
    "MEASURE_MIB",
    "MEASURE_BATCH",
    // And the five of the measurement gate, which this file cites when explaining which ones the script
    // sends to itself and which it exports for the application. Their **constants** —`TABS_VARIABLE` and
    // the rest— are identifiers of this crate and are checked on their own; what goes here is the name
    // that travels in the environment.
    "MEASURE_TRIES",
    "MEASURE_SIZE",
    "MEASURE_TABS",
    "MEASURE_PANES",
    "MEASURE_DRAGS",
    "PANEL_DIR",
    "PANEL_ID",
    "PWD",
    "SHELL",
    "SHLVL",
    "WINDOW_SIZE",
    "TERM",
    // Items of the crates this one depends on.
    "click",
    "click_and_drag",
    "CLICK_AND_DRAG",
    "content_rect",
    "distance_sq_to_pos",
    "dragged",
    "FOCUSABLE",
    "inner_size",
    "InputState",
    "interact_radius",
    "LineWriter",
    // The key `fontique` looks a family name up by, which is what makes asking for one by name
    // work however it is capitalized. Cited where that is explained.
    "NameKey",
    "NaN",
    "NonZeroUsize",
    "on_hover_cursor",
    "or_else",
    "portable_pty",
    // `winit` and `egui_winit`'s own vocabulary, cited in `vendor_patch.rs` to say why no test in
    // this tree can construct a `KeyEvent` or call `on_window_event` from outside those crates.
    "winit",
    "egui_winit",
    "KeyEvent",
    "on_window_event",
    // The writer `portable-pty` hands out for the master, cited because *its* destructor is what
    // used to rescue this crate's closing path by accident.
    "UnixMasterWriter",
    "prepare_raw_input",
    "recv",
    "serde",
    "set_var",
    "vars_os",
    "with",
    // Strings, not names: what a key is called and what it is worth in the settings file, what the
    // panel contract admits for its `attention` field, what the measurement variable accepts, and what
    // a report line is labelled. Every one of them is a string, so no set of identifiers can hold it —
    // and every one is translated with the file that defines it, not with this one.
    //
    // **`version` is here as the settings key and not as the constant**, which the guard tells apart by
    // itself: it compares with the case kept, and the constant is `VERSION`.
    "block",
    "Ctrl",
    "esc",
    "Fin",
    "Inicio",
    "down",
    "none",
    "pgdn",
    "pgup",
    // **Still live, checked by removing it**: one comment in `app/src/lib.rs` still cites the old
    // Spanish name of the report line this delivery renamed to `restored:`, and the guard reads that
    // backtick as a citation. It goes when that comment is translated. A reviewer declared it dead by
    // reasoning; the guard brought it back in red.
    "restauradas",
    "stale_after_minutes",
    "version",
    "wait",
    // Single letters standing for a quantity in prose.
    "f",
    "w",
    // ── What the sibling files cite, now that their citations are checked too ──
    // The standard library and the language.
    "fsync",
    "read",
    // The system's own error number for «the other end of this pseudoterminal is gone», cited by the
    // value that reports how a reading ended and by the test that pins why that value carries a
    // number at all: the standard library has no name for this one and flattens it into a bucket
    // whose whole printed form is the word «uncategorized». Same case as `OPOST` above — a constant
    // of the operating system, which this tree will never define.
    "EIO",
    // A method of `f32`. It is cited by the rounding that decides the grid, as the alternative that
    // was discarded — so it is named here precisely because the code does **not** call it, and its
    // siblings `floor` and `round` need no entry only because the code does.
    "ceil",
    // Other programs, and the shell.
    "echo",
    // From coreutils, which is why the two guards that run the script only run on Linux.
    "timeout",
    "grep",
    "sed",
    // From the shell and from procps, which is what the script uses to know whether a window is open:
    // the guard that runs it stops **before** that line, and its comment says why.
    "pgrep",
    "unset",
    "VTE",
    // Items of the crates this one depends on.
    "break_on_newline",
    // The names egui gives the four arrows, cited by the comment that explains why this project no
    // longer needs an alias for them.
    "Down",
    "Up",
    "Fonts",
    "from_name",
    "has_glyph",
    "has_glyphs",
    "NEAREST",
    "Single",
    // Font tables and font families, which belong to whoever made the font.
    "cmap",
    "COLR",
    "Hack",
    "Noto",
    // `std`, which is not a crate of this tree either.
    "sync_channel",
    "each_ref",
    // The VT engine this project hosts rather than writes, named where a comment has to say that a
    // fact about character widths belongs to it and to the table it reads, not to this repository.
    "alacritty_terminal",
    // A branch of epaint's tessellator and the marker its author left in it, cited where this crate
    // has to say which of the library's paths a shape must stay out of. Both belong to the library.
    "thin_line",
    "TODO",
    // Two of the tessellator's options, cited where this crate has to say what the library already
    // rounds by itself — which is what decides how much of the grid's alignment is ours to do.
    "round_text_to_pixels",
    "round_rects_to_pixels",
    // Unicode property names and script codes, which belong to Unicode.
    "Zsym",
    "Emoji_Component",
    "EmojiModifierBase",
    "EmojiOtherAndEmojiComponent",
    "Emoji_Presentation",
    // Another environment variable.
    "HOSTNAME",
    // More values that are not names: keys of the settings file, values of the panel contract, and
    // what a key is called when somebody types it.
    "Alt",
    "f4",
    "pageup",
    "pending",
    "prior",
    "waiting",
    // **A name that never existed, and the argument needs it.** The measurement bench's naming rule
    // is stated by counter-example — «a type called `Configuracion` inside the measurement module
    // would read as *the configuration* instead of as *what it cost to read it*» — so the citation is
    // of something deliberately absent. Nothing else in this list is of that kind.
    "Configuracion",
    // Keys of the domain-memory store's own frontmatter, cited by the guard that walks it. The
    // store's format belongs to the MCP server that writes it, not to this crate.
    "content_hashes",
    "file_paths",
    "symbols",
    "tags",
    // The language tag of a fenced block, which belongs to whoever renders the Markdown.
    "mermaid",
];

/// The `.rs` file names this milestone retired, derived from the arrows in the annex.
///
/// **Derived and not copied**, like the vocabulary above: the annex writes the eighteen renames as
/// `` `old.rs`→`new.rs` `` and this reads them back, so a list edited there cannot fall out of step
/// with the guard that enforces it.
fn retired_file_stems() -> Vec<String> {
    let mut out = Vec::new();
    for piece in DECISIONS.split("`→`") {
        let Some(before) = piece.rsplit('`').next() else {
            continue;
        };
        if let Some(stem) = before.strip_suffix(".rs")
            && !stem.is_empty()
            && stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            out.push(stem.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Whether a backticked citation names this retired file: `` `old` ``, `` `old.rs` ``,
/// `` `examples/old.rs` `` or `` `old::a_function` ``.
///
/// The shapes are spelled with a stem nobody retired on purpose — written with a real one, this doc
/// comment would be reported by the very test it explains.
fn cites_the_file(piece: &str, stem: &str) -> bool {
    let body = piece.trim();
    if let Some(path) = body.strip_suffix(".rs")
        && path.rsplit('/').next() == Some(stem)
    {
        return true;
    }
    let first = body
        .split("::")
        .next()
        .unwrap_or(body)
        .trim_end_matches("()");
    first == stem
}

/// Every identifier a pair of backticks cites — **all the segments of a path, not just the first**.
///
/// Shapes: `` `name` ``, `` `name()` ``, `` `name: Type` `` (a field with its type, which is how this
/// repository writes a measured layout) and `` `Type::member` `` with any number of segments.
///
/// **The qualified form used to be skipped, and the reason written here for skipping it was false.**
/// It said `` `Panel::written_at` `` is «a rustdoc link that `cargo doc` already judges». It is not:
/// rustdoc resolves links inside **brackets** — `[`Panel::written_at`]` — and a bare pair of
/// backticks is code formatting, nothing more. Worse, most of these live in `//` comments, which
/// rustdoc does not read at all. Fourteen citations naming methods and types this milestone had
/// already renamed were sitting behind that sentence with the suite green, two of them left by the
/// delivery that wrote the sentence. Measured, and the lesson is the one this project keeps
/// relearning: a plausible written justification is the cheapest place for a hole to hide.
///
/// So every segment is asked about. A bracketed rustdoc link passes anyway — its segments all exist,
/// or `cargo doc-check` was already failing — and the overlap costs nothing.
fn cited_names(inside: &str) -> Vec<&str> {
    // **A call with arguments in it, and the four it found are why it is here.** Until this line the
    // reader only stripped an empty `()`, so `` `Tema::con(...)` `` came back as nothing at all and the
    // guard never asked about `Tema` — a type that had left the tree entirely. Measured over the whole
    // citations list: four reports, three of them real, all three naming things a delivery renamed two
    // PRs ago. The wider version of the same idea — split a citation on spaces and ask about every word
    // — was measured too and thrown away: **309 reports and not one real**, because `cargo doc`,
    // `panel.json` and `Mayúsculas` are all words in backticks that are not citations of anything.
    let inside = match inside.find('(') {
        Some(at) if inside.ends_with(')') && !inside[..at].is_empty() => &inside[..at],
        _ => inside,
    };
    // A single colon separates a field from its type; a double one separates path segments.
    let path = match inside.split_once(':') {
        Some((_, rest)) if rest.starts_with(':') => inside,
        Some((before, _)) => before.trim_end(),
        None => inside,
    };
    let segments: Vec<&str> = path.split("::").collect();
    let an_identifier = |name: &str| {
        let mut chars = name.chars();
        chars.next().is_some_and(|first| {
            (first.is_ascii_alphabetic() || first == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    };
    // **All or nothing.** `panel.json` splits into one segment that is not an identifier, and so does
    // `cargo test`; reporting the good half of something that is not a citation at all would be a
    // false alarm, and a guard that breaks the build has to be wrong less often than the code.
    if segments.iter().all(|s| an_identifier(s)) {
        segments
    } else {
        Vec::new()
    }
}

/// Every identifier a crate's code actually defines or uses, comments and literals excluded.
fn code_names(files: &[PathBuf]) -> HashSet<String> {
    let mut names = HashSet::new();
    for file in files {
        let src = std::fs::read_to_string(file).expect("a file the walk just listed");
        names.extend(identifiers(&src).into_iter().map(|(_, name)| name));
        // The scanner drops the name of the function under a `#[test]`, which is exactly right when
        // it is hunting Spanish and exactly wrong here: a comment is allowed to cite a test by name.
        for line in src.lines() {
            if let Some((_, rest)) = line.split_once("fn ") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
            }
        }
    }
    names
}

/// A comment that cites an identifier by a name the code no longer has.
///
/// **This is the half of a rename that nothing else in the tree can see.** The compiler checks the
/// code, `cargo doc` checks the `[`link`]` form, and this delivery's own rule says a reference in a
/// comment is not prose and gets edited with the code — but a reference written `` `like_this` ``
/// has no oracle at all, so the rule was kept by hand. Twenty-one of them were not.
///
/// It is scoped per crate and not repo-wide on purpose: `desde` and `resto` are live identifiers **in
/// `app`** and in no other crate, so a union of every crate's names would call a citation of either of
/// them, made anywhere, fine — and report nothing.
///
/// **This sentence has now been one of the citations it describes twice.** It named three names; one,
/// `prioridad`, had left the tree entirely and the delivery before this one caught it, and the third,
/// `parar`, left with the module that owned it — caught by the review that proposed that module's map,
/// because nothing here reads this file for identifier citations and the reason is written above. Two
/// is a pattern: **the doc of a guard ages exactly like the code it guards**, and it has no guard.
#[test]
fn no_comment_cites_a_name_the_code_no_longer_has() {
    let root = repo();
    let mut stale = Vec::new();

    // Two kinds of entry, and the difference is **which files supply the names**: a finished crate
    // answers for itself, while a single file answers with its whole crate behind it, because a
    // comment there is entitled to cite what a sibling module declares.
    let mut jobs: Vec<(Vec<PathBuf>, Vec<PathBuf>)> = Vec::new();
    for crate_name in FINISHED_CRATES {
        let files = rust_files(&root.join("crates").join(crate_name));
        assert!(
            !files.is_empty(),
            "«{crate_name}» is declared finished and has no file to look at: its name on the list \
             is misspelled, and without this the guard would report a crate it never read as clean"
        );
        jobs.push((files.clone(), files));
    }
    // **«Every file of the crate» is checked against the tree, not trusted.** Measured: removing one
    // entry from the list —including the one this delivery adds— left the eight tests green, because a
    // file that leaves the list simply stops being looked at and nothing misses what it does not report.
    // The only exception is this file, and its reason is written above.
    let listed: HashSet<&str> = CITATIONS_CHECKED.iter().map(|(path, _)| *path).collect();
    for file in rust_files(&root.join("crates").join("app")) {
        let relative = file
            .strip_prefix(root.join("crates"))
            .expect("a path the walk built under `crates`")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "app/tests/glossary.rs" {
            continue;
        }
        assert!(
            listed.contains(relative.as_str()),
            "`{relative}` is a file of `app` and is not on the citations list. Every file of the crate \
             means every file: one that leaves the list stops being read, and nothing misses what it \
             no longer reports"
        );
    }

    for (path, crate_name) in CITATIONS_CHECKED {
        let file = root.join("crates").join(path);
        assert!(
            file.is_file(),
            "«{path}» is on the citations list and is not a file: the name is misspelled, and \
             without this the guard would report a file it never read as clean"
        );
        jobs.push((
            vec![file],
            rust_files(&root.join("crates").join(crate_name)),
        ));
    }

    for (to_read, for_names) in jobs {
        let files = to_read;
        let names = code_names(&for_names);
        for file in &files {
            let shown = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .display()
                .to_string();
            let src = std::fs::read_to_string(file).expect("a file the walk just listed");
            for (number, line) in src.lines().enumerate() {
                if !line.trim_start().starts_with("//") {
                    continue;
                }
                for piece in line.split('`').skip(1).step_by(2) {
                    for cited in cited_names(piece) {
                        if names.contains(cited) || NOT_OURS.contains(&cited) {
                            continue;
                        }
                        stale.push(format!("  {shown}:{}: `{cited}`", number + 1));
                    }
                }
            }
        }
    }

    assert!(
        stale.is_empty(),
        "{} comment(s) cite an identifier the crate around them does not have. A rename \
         does not break a comment, it makes it lie — and a reader who greps for the name it gives \
         finds nothing. Either the citation has to move to the new name, or the name belongs to \
         something outside this tree and goes in `NOT_OURS`:\n{}",
        stale.len(),
        stale.join("\n")
    );
}

/// A citation of a file this milestone renamed, anywhere on the code side of the tree.
///
/// **This is the hole the per-crate citation guard cannot close, and the reason is worth writing
/// down.** That guard asks whether the cited name exists **anywhere in the crate**, and every single
/// retired module name still does — as a *local variable*. «pantalla» is a `Screen` in two examples,
/// «fuentes» is a list of faces in three modules, «sesion» is a header in the measurement module,
/// «vigilante» is a watcher in the module now called `watcher.rs`. So a comment saying «the rule lives
/// in fuentes» passes: the guard finds the local and calls the citation good. Four such citations
/// were sitting in the tree with the suite green, and no widening of the other guard would have found
/// them — the name it looks for is genuinely there.
///
/// What makes this one possible is that the **old** names are known: the annex writes every file
/// rename as an arrow, so the question stops being «does this name exist?» and becomes «is this a name
/// we retired?», which has an exact answer.
///
/// **It reads the code side and not the prose side**, and that boundary is a decision already taken:
/// the annex counted 599 prose citations that were dead **before** this milestone and gave their
/// repair to the prose and document deliveries, so `docs/`, the two `README`s and the stored knowledge
/// are theirs. What is left here is code, its manifests and the measurement script — where a stale
/// citation has no delivery of its own and would simply stay.
#[test]
fn no_retired_file_name_is_still_cited() {
    let root = repo();
    let stems = retired_file_stems();
    assert!(
        stems.len() >= FILE_ARROWS_AT_LEAST,
        "the annex yielded {} file renames and it has to yield at least {FILE_ARROWS_AT_LEAST}: \
         without them this test looks at every file and asks about nothing",
        stems.len()
    );
    // **Counting is not identifying, and that is the disguise this project has on its record.** Measured:
    // returning the seventeen stems with a suffix stuck on —seventeen names that do not exist— keeps the
    // floor above happy and this test passes without looking at anything. So four of the ones the annex
    // carries are asked for by name.
    for known in ["pantalla", "fuentes", "sesion", "vigilante"] {
        assert!(
            stems.iter().any(|stem| stem == known),
            "the annex has to yield «{known}» and it did not: the extractor is finding the right \
             *number* of names and not the right names"
        );
    }

    let mut targets = vec![root.join("Cargo.toml")];
    for dir in ["crates", "scripts"] {
        let mut pending = vec![root.join(dir)];
        while let Some(here) = pending.pop() {
            for entry in std::fs::read_dir(&here)
                .unwrap_or_else(|failure| panic!("cannot read {}: {failure}", here.display()))
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else if path
                    .extension()
                    .is_some_and(|e| e == "rs" || e == "toml" || e == "sh")
                {
                    targets.push(path);
                }
            }
        }
    }
    assert!(
        targets.len() > 20,
        "the walk found {} files to read, which cannot be the code side of this repository",
        targets.len()
    );
    // **One of each kind, and this is not decoration.** Measured: drop `scripts` from the walk above
    // and every assertion below still passes, because the citations that were stale there are fixed —
    // so the only thing left proving the walk reaches a shell script is this.
    //
    // **And the manifests are asked for one by one, because the root one was masking the others.** With
    // a single «is there a `.toml`?» the manifest of the root satisfied it on its own, so dropping the
    // whole `toml` extension from the walk stayed green — and two of the stale citations this test
    // found lived in a crate's manifest, not in the root's.
    let named = |what: &str| {
        targets
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == what))
    };
    assert!(
        named("Cargo.toml"),
        "the walk did not reach a manifest at all"
    );
    assert!(
        targets.iter().any(|path| {
            path.file_name().is_some_and(|name| name == "Cargo.toml")
                && path.parent().is_some_and(|dir| {
                    dir.parent()
                        .is_some_and(|up| up.file_name().is_some_and(|name| name == "crates"))
                })
        }),
        "the walk reached no manifest of a crate: the one at the root was answering for all of them"
    );
    for kind in ["rs", "sh"] {
        assert!(
            targets
                .iter()
                .any(|path| path.extension().is_some_and(|e| e == kind)),
            "the walk claims to read `.{kind}` files and found none: a whole kind of file would be \
             going unread while this test stayed green"
        );
    }
    targets.sort();

    let mut stale = Vec::new();
    for file in &targets {
        let shown = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .display()
            .to_string();
        let src = std::fs::read_to_string(file).expect("a file the walk just listed");
        for (number, line) in src.lines().enumerate() {
            for piece in line.split('`').skip(1).step_by(2) {
                for stem in &stems {
                    if cites_the_file(piece, stem) {
                        stale.push(format!(
                            "  {shown}:{}: `{piece}` → `{stem}` is gone",
                            number + 1
                        ));
                    }
                }
            }
        }
    }

    assert!(
        stale.is_empty(),
        "{} citation(s) on the code side of the tree still name a file this milestone renamed. The \
         file is not missing — it answers to another name now, and a reader who greps for the one \
         given here finds nothing. Cite the new name; and if the word inside the backticks was never \
         a file at all, it was never a citation either, so write it as «a word» and not as code:\n{}",
        stale.len(),
        stale.join("\n")
    );
}

/// The reader of a file citation, over its four shapes.
///
/// **Without this the reader had no oracle at all.** Measured: `fn cites_the_file(..) -> bool { false }`
/// left the eight tests green, and so did breaking each of the four shapes on its own. The reason is
/// worth writing down, because it is the shape of hole a guard is most likely to end in: the only test
/// that calls it *looks for* citations, and the tree is clean — so **breaking the finder makes the suite
/// greener, not redder**. A finder needs its own oracle; the absence of what it finds cannot be one.
#[test]
fn the_reader_of_file_citations_knows_the_four_shapes() {
    for shape in [
        "pantalla",
        "pantalla.rs",
        "examples/pantalla.rs",
        "pantalla::draw",
        "pantalla()",
    ] {
        assert!(
            cites_the_file(shape, "pantalla"),
            "«{shape}» is one of the four shapes this reader promises to know"
        );
    }
    assert!(
        !cites_the_file("screen", "pantalla"),
        "the name it was renamed to is not a citation of the name it had"
    );
    assert!(
        !cites_the_file("pantallazo", "pantalla"),
        "a longer name that merely starts the same is not this file"
    );
    assert!(
        !cites_the_file("draw::pantalla", "pantalla"),
        "only the FIRST segment of a path is the module: a member that happens to share the name is \
         not a citation of the file"
    );
}

/// The reader of citations, over the shapes that decide whether one is even asked about.
///
/// **Half of these are exemptions, and an exemption with no test is what lets a guard go quiet.**
/// The double colon is the one that matters: this repository writes hundreds of
/// `[`Type::method`]` links and every one of them would be reported as a stale citation.
#[test]
fn the_reader_of_citations_asks_about_the_right_things() {
    assert_eq!(cited_names("cuantos"), vec!["cuantos"], "a bare name");
    assert_eq!(cited_names("lo_quiere()"), vec!["lo_quiere"], "a call");
    assert_eq!(
        cited_names("desde: u32"),
        vec!["desde"],
        "a field with its type, which is how this repository writes a measured layout"
    );
    // **The qualified form, which used to come back empty behind a false reason.** Both halves are
    // asked about, because both can die: `Ventana::pintar_division` was two dead names at once.
    assert_eq!(
        cited_names("Panel::written_at"),
        vec!["Panel", "written_at"],
        "a qualified path is every one of its segments: a bare pair of backticks is not a rustdoc \
         link, so nothing else in the tree judges it"
    );
    // **A call with its arguments written in, which used to come back empty.** Both shapes: the one
    // with a real argument and the one with an ellipsis, which is how this repository writes «and the
    // rest». Without this the guard skips the citation entirely instead of asking about it.
    assert_eq!(
        cited_names("Theme::from_colors(...)"),
        vec!["Theme", "from_colors"],
        "a call with its arguments elided is still a citation of the thing being called"
    );
    assert_eq!(
        cited_names("scroll_by(ToBottom)"),
        vec!["scroll_by"],
        "and one with a real argument: the argument is a name of its own and is not asked about here"
    );
    assert_eq!(
        cited_names("panel.json"),
        Vec::<&str>::new(),
        "a file name is not a citation, and stays that way"
    );
    assert_eq!(
        cited_names("session::Nodo::Rama"),
        vec!["session", "Nodo", "Rama"],
        "however many segments it has"
    );
    assert!(
        cited_names("panel.json").is_empty(),
        "a file name is not an identifier"
    );
    assert!(cited_names("cargo test").is_empty(), "nor is a command");
    assert!(
        cited_names("").is_empty(),
        "nor is an empty pair of backticks"
    );
    assert!(cited_names("--mib").is_empty(), "nor is a flag");
    // **All or nothing, and this is the case that forced it.** Splitting on `::` first would have
    // turned a version number into two «identifiers» and reported the half that looks like one.
    assert!(
        cited_names("egui::0.29").is_empty(),
        "a path with one segment that is not an identifier is not a citation at all"
    );
}

/// The scanner, over the battery of shapes Rust allows and this repository writes.
///
/// **It exists because half of its branches are not touched by any file in today's tree**, and a
/// branch with no oracle is a branch nobody knows works: nested-comment depth was exercised by
/// nothing — removing it left the other four tests green — and its defect still bites both ways, with
/// a false positive that breaks the build and a false negative that switches a name off.
#[test]
fn the_scanner_tells_code_from_prose() {
    let source = r####"
// la celda de arriba
/// la celda de la documentación
/* la celda del bloque */
/* uno /* dos */ la celda de dentro */
/* uno /* dos */ #[test] */
fn celda_escondida() {}
fn f<'a>(_x: &'a str) -> &'a str {
    let _ = "la celda de la cadena";
    let _ = r#"{"celda": 1}"#;
    let _ = b"la celda en bytes";
    let _ = 'ñ';
    let _ = "// celda";
    let _ = "/* celda */";
    let _ = "una \" celda";
    let partida = "una linea \
                   y su continuacion";
    partida
}
#[test]
fn nombre_de_prueba_en_espanol() {}
#[test]
#[ignore]
fn otro_nombre_de_prueba() {}
fn ayudante_espanol() {}
"####;

    let scanned = identifiers(source);
    let found: Vec<&str> = scanned.iter().map(|(_, n)| n.as_str()).collect();

    // What **is** code and has to come out.
    for expected in ["celda_escondida", "f", "partida", "ayudante_espanol"] {
        assert!(
            found.contains(&expected),
            "`{expected}` is code and the scanner did not see it: {found:?}"
        );
    }
    // What is prose or a literal and must **not** come out. `celda` appears only inside comments and
    // strings, so seeing it means one of the six shapes has thrown the scanner off.
    assert!(
        !found.contains(&"celda"),
        "the scanner read prose or a literal as code: {found:?}"
    );
    // **And the test names, which this guard used to walk past and now judges.** They were exempt
    // while they were still Spanish; with them translated the exemption only hid the easiest way for
    // Spanish to come back into the crate. The `#[ignore]` between the attribute and the `fn` is
    // here on purpose: it is what tells «cleared by the next `fn`» from «cleared by the next word».
    for judged in ["nombre_de_prueba_en_espanol", "otro_nombre_de_prueba"] {
        assert!(
            found.contains(&judged),
            "`{judged}` is a test name and is no longer skipped, so the scanner has to report it: \
             {found:?}"
        );
    }

    // And the line it reports, which is what whoever fixes it reads. `ayudante_espanol` sits after a
    // string continuation, which is exactly what used to make the number drift.
    let line_of = |name: &str| {
        scanned
            .iter()
            .find(|(_, n)| n == name)
            .map(|(l, _)| *l)
            .unwrap_or_else(|| panic!("`{name}` is missing"))
    };
    let real = |name: &str| {
        source
            .lines()
            .position(|l| l.contains(&format!("fn {name}")))
            .expect("the line exists")
            + 1
    };
    for name in ["celda_escondida", "ayudante_espanol"] {
        assert_eq!(
            line_of(name),
            real(name),
            "the line reported for `{name}` is not its own"
        );
    }
}
