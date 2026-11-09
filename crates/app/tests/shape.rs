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

mod common;

use common::{list_against_disk, rust_sources};
use std::path::Path;

/// Every module of the crate, pulled in **at compile time** for the same reason its siblings give:
/// a directory walk turns a renamed or deleted file into a target that is silently skipped, and a
/// guard with no file to look at reads exactly like a clean one.
const MODULES: &[(&str, &str)] = &[
    ("src/chrome/menu.rs", include_str!("../src/chrome/menu.rs")),
    (
        "src/chrome/updates.rs",
        include_str!("../src/chrome/updates.rs"),
    ),
    (
        "src/chrome/updates/feed.rs",
        include_str!("../src/chrome/updates/feed.rs"),
    ),
    (
        "src/chrome/updates/service.rs",
        include_str!("../src/chrome/updates/service.rs"),
    ),
    (
        "src/viewer/origin.rs",
        include_str!("../src/viewer/origin.rs"),
    ),
    (
        "src/viewer/delivery.rs",
        include_str!("../src/viewer/delivery.rs"),
    ),
    ("src/viewer/diff.rs", include_str!("../src/viewer/diff.rs")),
    (
        "src/viewer/review.rs",
        include_str!("../src/viewer/review.rs"),
    ),
    (
        "src/viewer/review/message.rs",
        include_str!("../src/viewer/review/message.rs"),
    ),
    ("src/links/local.rs", include_str!("../src/links/local.rs")),
    (
        "src/viewer/interaction_tests.rs",
        include_str!("../src/viewer/interaction_tests.rs"),
    ),
    ("src/viewer.rs", include_str!("../src/viewer.rs")),
    ("src/viewer/load.rs", include_str!("../src/viewer/load.rs")),
    (
        "src/viewer/markdown.rs",
        include_str!("../src/viewer/markdown.rs"),
    ),
    (
        "src/viewer/tests.rs",
        include_str!("../src/viewer/tests.rs"),
    ),
    ("src/viewer/view.rs", include_str!("../src/viewer/view.rs")),
    (
        "src/console_input.rs",
        include_str!("../src/console_input.rs"),
    ),
    ("src/summary.rs", include_str!("../src/summary.rs")),
    (
        "src/summary/input.rs",
        include_str!("../src/summary/input.rs"),
    ),
    (
        "src/summary/model.rs",
        include_str!("../src/summary/model.rs"),
    ),
    (
        "src/summary/view.rs",
        include_str!("../src/summary/view.rs"),
    ),
    (
        "src/summary/probe.rs",
        include_str!("../src/summary/probe.rs"),
    ),
    (
        "src/panel/summary.rs",
        include_str!("../src/panel/summary.rs"),
    ),
    (
        "src/measure/overview.rs",
        include_str!("../src/measure/overview.rs"),
    ),
    (
        "src/tests/overview.rs",
        include_str!("../src/tests/overview.rs"),
    ),
    (
        "src/settings/editor/view/footer.rs",
        include_str!("../src/settings/editor/view/footer.rs"),
    ),
    (
        "src/settings/editor/view/typography.rs",
        include_str!("../src/settings/editor/view/typography.rs"),
    ),
    (
        "src/settings/editor/view/tests/usability.rs",
        include_str!("../src/settings/editor/view/tests/usability.rs"),
    ),
    (
        "src/settings/editor/view/tests/layout.rs",
        include_str!("../src/settings/editor/view/tests/layout.rs"),
    ),
    (
        "src/tab_bar/interaction.rs",
        include_str!("../src/tab_bar/interaction.rs"),
    ),
    (
        "src/tests/integrated_chrome.rs",
        include_str!("../src/tests/integrated_chrome.rs"),
    ),
    (
        "src/tests/pane_rename.rs",
        include_str!("../src/tests/pane_rename.rs"),
    ),
    (
        "src/chrome/branding.rs",
        include_str!("../src/chrome/branding.rs"),
    ),
    (
        "src/chrome/connection.rs",
        include_str!("../src/chrome/connection.rs"),
    ),
    ("src/chrome.rs", include_str!("../src/chrome.rs")),
    (
        "src/header/integrated.rs",
        include_str!("../src/header/integrated.rs"),
    ),
    ("src/pane_view.rs", include_str!("../src/pane_view.rs")),
    ("src/visibility.rs", include_str!("../src/visibility.rs")),
    (
        "src/settings/editor/probe.rs",
        include_str!("../src/settings/editor/probe.rs"),
    ),
    (
        "src/settings/editor/view/tests.rs",
        include_str!("../src/settings/editor/view/tests.rs"),
    ),
    (
        "src/settings/editor/view/colors.rs",
        include_str!("../src/settings/editor/view/colors.rs"),
    ),
    (
        "src/settings/editor/view/appearance.rs",
        include_str!("../src/settings/editor/view/appearance.rs"),
    ),
    (
        "src/tests/settings_chrome.rs",
        include_str!("../src/tests/settings_chrome.rs"),
    ),
    (
        "src/tab_bar/chrome.rs",
        include_str!("../src/tab_bar/chrome.rs"),
    ),
    ("src/panel/cache.rs", include_str!("../src/panel/cache.rs")),
    (
        "src/settings/editor/runtime.rs",
        include_str!("../src/settings/editor/runtime.rs"),
    ),
    (
        "src/fonts/selection.rs",
        include_str!("../src/fonts/selection.rs"),
    ),
    (
        "src/settings/editor/script.rs",
        include_str!("../src/settings/editor/script.rs"),
    ),
    (
        "src/tests/settings_editor.rs",
        include_str!("../src/tests/settings_editor.rs"),
    ),
    (
        "src/fonts/startup.rs",
        include_str!("../src/fonts/startup.rs"),
    ),
    (
        "src/settings/editor.rs",
        include_str!("../src/settings/editor.rs"),
    ),
    (
        "src/settings/editor/view.rs",
        include_str!("../src/settings/editor/view.rs"),
    ),
    (
        "src/settings/editor/timing.rs",
        include_str!("../src/settings/editor/timing.rs"),
    ),
    (
        "src/settings/document.rs",
        include_str!("../src/settings/document.rs"),
    ),
    (
        "src/settings/store.rs",
        include_str!("../src/settings/store.rs"),
    ),
    ("src/lib.rs", include_str!("../src/lib.rs")),
    (
        "src/painter/pane.rs",
        include_str!("../src/painter/pane.rs"),
    ),
    ("src/ligatures.rs", include_str!("../src/ligatures.rs")),
    (
        "src/ligature_tests.rs",
        include_str!("../src/ligature_tests.rs"),
    ),
    (
        "src/fonts/ligatures.rs",
        include_str!("../src/fonts/ligatures.rs"),
    ),
    (
        "src/painter/ligatures.rs",
        include_str!("../src/painter/ligatures.rs"),
    ),
    (
        "src/painter/ligatures/tests.rs",
        include_str!("../src/painter/ligatures/tests.rs"),
    ),
    (
        "src/pointer/header.rs",
        include_str!("../src/pointer/header.rs"),
    ),
    (
        "src/header/actions.rs",
        include_str!("../src/header/actions.rs"),
    ),
    (
        "src/header/controls.rs",
        include_str!("../src/header/controls.rs"),
    ),
    ("src/header/menu.rs", include_str!("../src/header/menu.rs")),
    (
        "src/header/splitting.rs",
        include_str!("../src/header/splitting.rs"),
    ),
    (
        "src/tests/header_controls.rs",
        include_str!("../src/tests/header_controls.rs"),
    ),
    (
        "src/tests/markdown.rs",
        include_str!("../src/tests/markdown.rs"),
    ),
    ("src/alarms.rs", include_str!("../src/alarms.rs")),
    ("src/closing.rs", include_str!("../src/closing.rs")),
    ("src/control.rs", include_str!("../src/control.rs")),
    (
        "src/control/retiring.rs",
        include_str!("../src/control/retiring.rs"),
    ),
    ("src/injected.rs", include_str!("../src/injected.rs")),
    (
        "src/injected/links.rs",
        include_str!("../src/injected/links.rs"),
    ),
    (
        "src/injected/search.rs",
        include_str!("../src/injected/search.rs"),
    ),
    ("src/links.rs", include_str!("../src/links.rs")),
    (
        "src/links/console.rs",
        include_str!("../src/links/console.rs"),
    ),
    ("src/panel.rs", include_str!("../src/panel.rs")),
    (
        "src/panel/document.rs",
        include_str!("../src/panel/document.rs"),
    ),
    (
        "src/panel/buttons.rs",
        include_str!("../src/panel/buttons.rs"),
    ),
    ("src/panel/links.rs", include_str!("../src/panel/links.rs")),
    ("src/panel/color.rs", include_str!("../src/panel/color.rs")),
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
    ("src/fonts/atlas.rs", include_str!("../src/fonts/atlas.rs")),
    ("src/header.rs", include_str!("../src/header.rs")),
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
    (
        "src/painter/emoji.rs",
        include_str!("../src/painter/emoji.rs"),
    ),
    ("src/pty_output.rs", include_str!("../src/pty_output.rs")),
    ("src/watcher.rs", include_str!("../src/watcher.rs")),
    ("src/theme.rs", include_str!("../src/theme.rs")),
    (
        "src/theme/catalog.rs",
        include_str!("../src/theme/catalog.rs"),
    ),
    ("src/mouse.rs", include_str!("../src/mouse.rs")),
    ("src/opening.rs", include_str!("../src/opening.rs")),
    ("src/pointer.rs", include_str!("../src/pointer.rs")),
    (
        "src/pointer/links.rs",
        include_str!("../src/pointer/links.rs"),
    ),
    ("src/report.rs", include_str!("../src/report.rs")),
    ("src/search.rs", include_str!("../src/search.rs")),
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
/// **`src/fonts.rs` reached exactly 600 and the next line it needed failed this guard**, which is
/// the answer it was written to give: the color glyph and its atlas went out to `src/fonts/atlas.rs`
/// rather than the ceiling going up. A ceiling that moves whenever something reaches it is not a
/// ceiling.
const MODULE_CEILING: usize = 600;

/// Production lines an `impl` block may hold — and a type's own blocks, summed across every module
/// up to its first `mod tests`, the cut every count of this file makes.
///
/// A type whose behaviour does not fit in 300 lines is more than one type, or its seams are in the
/// wrong place. Measured as this guard lands: the largest block outside `src/lib.rs` is
/// `impl Discoverer` at 269, so this ceiling asks nothing of the code that is already in shape. It
/// read 223 until somebody counted again: a number in a comment decays on its own, and the ones this
/// file states about the crate are re-measured by every delivery that touches it.
///
/// **For its first three weeks it held blocks only, and a type walked straight through it.** A block
/// is where the code sits; the sentence above is about a type. Moving methods into an `impl Window`
/// of a child module passed every block ceiling while the type kept growing — 1 124 production lines
/// in two blocks when the rule landed, 1 782 in fifteen blocks across twelve files when it was summed
/// again on 28 September 2026. So the same number is now held twice: per block, which still catches
/// a foreign trait's impl, and per type, which is what the rationale always said. D28 records the
/// amendment.
const IMPL_CEILING: usize = 300;

/// What is over a ceiling today, with the number it is at and the reason it is not fixed here.
///
/// **A ratchet, not an exemption**: the guard fails when one of these grows. Lowering a number is
/// the normal outcome of a delivery that takes a piece out, and this table is where that progress is
/// recorded.
///
/// **Three kinds of entry, told apart by their shape**: a module (`src/lib.rs`), one block of one
/// module (`src/x.rs::impl Foo`) and a type summed across the crate (`impl Foo`).
const OVER_CEILING_TODAY: &[(&str, usize, &str)] = &[
    (
        "src/lib.rs",
        647,
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
         Seventy-one out and thirty in against 1 000 leaves **959**. **And the phase that gives an \
         agent a pane of its own paid it with the piece its own subject names**: what a keyboard \
         chord asks of the window, and the dispatch that does it, went out to `opening.rs`, **33 \
         production lines**, against the **19** this delivery spends back here — the field a window \
         keeps its handles in, the walk that answers what the connections left for the tree, the \
         arguments a launch now takes, and the flag a console the person took over carries. \
         Thirty-three out and nineteen in against 959 leaves **945**. **And then five lines came \
         back with nothing going out, which is the direction this table exists to allow**: the \
         milestone that gives every pane a header made the three helpers that turn a pointer's \
         position into a cell take the console's rectangle instead of the pane's, and the shorter \
         name let rustfmt fold a six-line signature into one. Nothing was moved and nothing was \
         deleted — the formatter decides what this number counts, and here it decided in the \
         file's favour. Nine hundred and forty-five less five leaves **940**. **And the delivery \
         that actually cut the strip paid it the way D28 prescribes, with the piece its own subject \
         names**: attending the border between a console and its panel went out to \
         `panel/pane.rs`, **53 production lines**, which is the module that already answered how \
         much of its rectangle a pane's panel takes — and going `pub(crate)` on the way widened \
         nothing, because it lived in the crate's root module where every module could already \
         reach it. Against that, the strip spends **seven** lines back here: the `mod`, the two \
         fields a window keeps (whether it paints strips at all, and where the last press fell), \
         their two seeds, the one that reads the knob at startup, the one that resolves it, and \
         the one that reads it once per walk — **less the one the clippy ceiling gave back**, \
         because grouping the blink and the strip into one value took the painting's call from \
         eight arguments to seven and two lines to one. Fifty-three out and six net in against 940 \
         leaves **894**. **And the delivery that declared the strip to the bench paid it with the \
         piece its own subject names**: closing the frame — every console's spans folded, what the \
         window timed apart subtracted, and the rest recorded — went out to `report.rs`, **20 \
         production lines**, which is the module that already held the pass that stops a round \
         escaping that same sum, and whose one idea a review widened by a step to say so. Against \
         that, declaring the strips spends **four** lines back here: the field a pane's outcome \
         carries the cost out through, the one the window keeps the accounting in, and the two of \
         the call that hands each strip over. **What made it four rather than seven was the review**: \
         the first shape kept the frame's tally and the run's on two separate fields and summed and \
         subtracted inline, and a mutation round walked straight through it — deleting the \
         subtraction that keeps the strips out of the framework's figure (**D13**) left the whole \
         suite green, and so did turning the sum over the panes into a replacement. Both now live in \
         one type in `report.rs` with a test on them, and the window holds one field instead of two. \
         Twenty out and four in against 894 leaves **878** — the figures are the diff's own, taken \
         by matching the two files' production lines rather than by counting them by hand, which is \
         how the first telling of this line got both of them wrong. **And the off-frame panel read \
         paid for itself with the piece that already owns how a pane gets its panel**: the delivery \
         first spent two lines here — constructing the committed state before the channel, and \
         retaining a closed pane's read tally — then grouping that tally with the counters the \
         window already keeps gave one back. Moving the state and channel construction together to \
         `panel/pane.rs` then took four lines out. **878 + 2 - 1 - 4 leaves 875**. **The review that \
         made every pane share one window read accumulator paid for its wiring with the constructor \
         that already belongs to opening**: threading the accumulator through new panes first added \
         80 production lines here, then the 124-line pane-launching block moved to `opening.rs`. \
         **875 + 80 - 124 leaves 831**. **The retirement probe then paid for the context its hidden \
         panes need by shortening that sizing entry point**: passing the context added one \
         production line, and the shorter name and parameter folded its six-line signature into \
         one. **831 + 1 - 5 leaves 827**. **The delivery that makes links openable paid it by moving \
         out a piece that was ready to go**: the three helpers that turn a pointer's position \
         into a cell — which links do not use — went out to `mouse.rs`, the module that already answers what a pane does with \
         a mouse gesture, **48 production lines**, and reading how many gestures the bench \
         asks for went to `injected.rs`, the bench's own module, **6** more, against the **11** \
         links spend here — the `mod`, a pane's painted snapshot, the outcome field that carries \
         the bench's points, the window's tally, where an opened link goes and the gesture holder, \
         the bench's flag read once and handed to the painter, and the call that hands what a pane \
         found to the report and the injector, which rustfmt lays out on three. Everything else of \
         the feature went to modules of its own. **827 + 11 - 48 - 6 leaves 784**. **Then the \
         saving state followed the saving policy into `session.rs`**: the pending deadline and the \
         last document written became fields of `Persistence`, and only their two declarations \
         counted here — both constructors sit where this guard does not count. **784 - 2 leaves \
         782**. **Watcher rearming then followed pane closing into `closing.rs`: eight production \
         lines left the core and one new call remained, so 782 + 1 - 8 leaves 775.** **Then the piece \
         went out ahead of the delivery that needs the room**: collecting what a closing console's \
         frame left undeclared is closing's work, so it went to `closing.rs` as a free function — \
         out of this file and out of `impl Pane` at once — **10 production lines**, before the \
         scrollback's search spends any here. **775 - 10 leaves 765.** The search then \
         spent **eight**: its `mod`, the field a console keeps its search in, the one a pane's outcome \
         hands the search's cost through, the window's tally, the line that hands it over, and the \
         three that time the keyboard's pass on a frame somebody typed in — and paid **five** more \
         here, finding a console by name moving to `Tab` and walking every console becoming one \
         line. **765 + 8 - 5 leaves 768.** Pane controls move splitting out and add the header input hook; the measured module now has 725 production lines. Settings then moves startup font resolution to fonts/startup.rs and adds its global entry wiring, leaving 693 including the measurement injector hook. Runtime font handoff now belongs to editor/runtime.rs, leaving688. Shared strip partitioning leaves687. Integrated chrome moves hidden-view geometry and startup size parsing out, leaving685. Fresh tab creation now belongs to opening.rs and the measured module is666. Overview extracts console routing and leaves649. Optional ligatures extract startup font configuration, leaving647.",
    ),
    (
        "impl Window",
        1617,
        "the window's own behaviour. D28's order moved its methods out of `lib.rs` into \
         satellite blocks in twelve modules, which shrank the file and not the type. Recorded on 28 September \
         2026, the day the guard first summed a type's blocks. **The scrollback's search spent five \
         here** — three timing the keyboard's pass on a frame somebody typed in, which closed a \
         declared hole, the report's `search:` field and the tally's seed when a window opens; \
         handing each painting's cost over sits in `impl eframe::App`, which this count does not \
         read — **and paid five**: finding a console by name moved to `Tab`, and walking every \
         console became one line. **1782 + 5 - 5 leaves 1782.** Header operations now use free functions around pane identity; the measured Window implementation has 1764 lines. Applying configuration now belongs to Settings, paying for editor construction and reporting; the measured total is 1750. Integrated chrome keeps tab interaction and card rendering in free functions, leaving1705 including explicitly timed hidden-view geometry. Moving fresh tab creation to opening.rs pays for the new controls and leaves1695. Overview extracts console routing and pure deadline selection, leaving1680. The common initial-state constructor now lives in opening::initial_window as a free function, leaving1620 with the ligature preference and report hook. Local Markdown link routing and gesture cleanup now use free functions, leaving1617.",
    ),
    (
        "impl Pane",
        541,
        "a console with its panel, its keys and its mouse. On the day it was recorded it was six \
         blocks in six files: the keyboard and mouse passes (157 each), how a pane is opened (112) \
         and how it gets its panel (120), and 45 more in `lib.rs` and `control.rs`. **The search \
         paid for its own lines here before spending them**: collecting a closing console's \
         leftovers became a free function of `closing.rs`, 10 lines out, and the keyboard pass \
         spent ten — nine asking the search about each key, one seeding a new pane's search \
         record — with the rules themselves in `search.rs`, outside the type. **591 - 10 + 10 \
         leaves 591.**",
    ),
    (
        "impl PanelState",
        363,
        "what a pane holds of its panel. On the day it was recorded it was split over `panel.rs` \
         and its child modules for the document, links, driving and the pane, to pay the block \
         ceiling. Cache construction now belongs to the resolved-colour cache and leaves363.",
    ),
    (
        "impl InjectedDrags",
        332,
        "the bench's injected gestures. On the day it was recorded its link gestures sat in a \
         child module (45 lines) that had gone there to pay the block ceiling. **The search gesture \
         went to a child module as a free function**, so the type paid one line for its arm in the \
         dispatch, and a three-line method with a single caller went back to being the addition it \
         wrapped. **334 + 1 - 3 leaves 332.**",
    ),
    (
        "impl Node",
        304,
        "the layout tree's node. On the day it was recorded it was two blocks of `tree.rs`, of 265 \
         and 56 lines.",
    ),
];

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
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        let code = line.trim_start();
        // Match only the compiler's unambiguous test-only single-line statement case.
        // Multiline items and other conditions stay counted conservatively.
        if depth == 0
            && code.trim_end() == "#[cfg(test)]"
            && lines.peek().is_some_and(|next| {
                let next = next.trim();
                next.ends_with(';') && !next.contains(['{', '}']) && !next.starts_with("//")
            })
        {
            lines.next();
            continue;
        }
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

#[test]
fn only_an_unambiguous_test_statement_is_excluded_from_production_lines() {
    let source = "#[cfg(test)]\nprobe();\nproduction();\n#[cfg(unix)]\nunix_work();\n// #[cfg(test)]\nnot_a_probe();\n#[cfg(test)]\nmultiline(\n    argument,\n);\n#[cfg(test)]\nif true { work(); }\nafter();\n";
    assert_eq!(
        production_lines(source),
        [
            "production();",
            "#[cfg(unix)]",
            "unix_work();",
            "not_a_probe();",
            "#[cfg(test)]",
            "multiline(",
            "    argument,",
            ");",
            "#[cfg(test)]",
            "if true { work(); }",
            "after();",
        ]
    );
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

/// A block's header without its opening brace — the one spelling every lookup of this file uses.
fn head(header: &str) -> &str {
    header.trim_end_matches(" {").trim_end()
}

/// The name this guard calls an `impl` block, which is what a failure has to be searchable by.
fn name_of(module: &str, header: &str) -> String {
    format!("{module}::{}", head(header))
}

/// The last segment of a path, without its generic arguments: `crate::Window` and `Window<T>` are
/// both `Window`.
fn last_segment(path: &str) -> &str {
    let path = path.split(['<', ' ']).next().unwrap_or(path);
    path.rsplit("::").next().unwrap_or(path)
}

/// A top-level declaration without the visibility in front of it.
fn without_visibility(line: &str) -> &str {
    ["pub(crate) ", "pub(super) ", "pub "]
        .iter()
        .find_map(|visibility| line.strip_prefix(visibility))
        .unwrap_or(line)
}

/// The traits this crate declares itself, found the same way the blocks are: at column zero.
fn local_traits<'a>(modules: &[(&str, &'a str)]) -> std::collections::BTreeSet<&'a str> {
    let mut traits = std::collections::BTreeSet::new();
    for (_, source) in modules {
        for line in production_lines(source) {
            if let Some(rest) = without_visibility(line).strip_prefix("trait ") {
                traits.insert(last_segment(rest.split([':', '{']).next().unwrap_or(rest)));
            }
        }
    }
    traits
}

/// The type an `impl` header charges its lines to, or `None` for the impl of a foreign trait.
///
/// **A foreign trait's impl is left out of the type's sum on purpose**: `Display`, `Drop` or
/// `eframe::App` is a contract somebody else wrote, and its size is set by that contract rather than
/// by the type's own seams. The per-block ceiling still holds it. **A trait this crate declares is
/// not left out**, because moving a type's methods into `impl OurTrait for Window` would otherwise be
/// the same walk around the ceiling that splitting it into blocks was.
///
/// **The key is the last segment of the type's path**, so `impl crate::Window` and `impl Window` are
/// one type. It reads a one-line header with no generics of its own, and
/// [`every_impl_header_is_one_the_type_walk_can_read`] is what fails when a header stops being one.
fn type_of<'a>(header: &'a str, local: &std::collections::BTreeSet<&str>) -> Option<&'a str> {
    let rest = head(header).strip_prefix("impl ")?;
    match rest.split_once(" for ") {
        Some((tr, ty)) => local.contains(last_segment(tr)).then(|| last_segment(ty)),
        None => Some(last_segment(rest)),
    }
}

/// Every type's `impl` blocks, summed across the given modules.
///
/// **Two distinct types sharing a name in different modules are summed together** — there is no
/// compiler here to tell them apart. Unratcheted, that only makes the ceiling fire sooner. Ratcheted,
/// one of the two could grow while the other shrank by as much, so
/// [`no_entry_of_the_ratchet_is_stale`] refuses a type entry whose name is defined twice.
fn type_sizes<'a>(modules: &[(&str, &'a str)]) -> std::collections::BTreeMap<&'a str, usize> {
    let local = local_traits(modules);
    let mut sizes = std::collections::BTreeMap::new();
    for (_, source) in modules {
        let lines = production_lines(source);
        for (header, size, _) in impl_blocks(&lines) {
            if let Some(ty) = type_of(header, &local) {
                *sizes.entry(ty).or_insert(0) += size;
            }
        }
    }
    sizes
}

/// How many times the given modules define a `struct` or `enum` with this name.
fn definitions_of(modules: &[(&str, &str)], name: &str) -> usize {
    modules
        .iter()
        .flat_map(|(_, source)| production_lines(source))
        .filter(|line| {
            ["struct ", "enum "].iter().any(|kind| {
                without_visibility(line)
                    .strip_prefix(kind)
                    .is_some_and(|rest| {
                        last_segment(rest.split([';', '(', '{']).next().unwrap_or(rest)) == name
                    })
            })
        })
        .count()
}

/// The size of the one block a ratchet entry names, or why it names none or several.
///
/// **One entry, one block.** A name matching two blocks would hand the entry's number to both as
/// their ceiling, so the smaller one could grow to it with nothing firing.
fn the_one_block(source: &str, header: &str) -> Result<usize, String> {
    let lines = production_lines(source);
    let named: Vec<usize> = impl_blocks(&lines)
        .into_iter()
        .filter(|(h, ..)| head(h) == header)
        .map(|(_, size, _)| size)
        .collect();
    match named.as_slice() {
        [size] => Ok(*size),
        [] => Err("names an impl block that no longer exists".to_string()),
        several => Err(format!(
            "names {} impl blocks of that module, so its number would be the ceiling of all of \
             them: merge them or record the type instead",
            several.len()
        )),
    }
}

/// What a ratchet entry measures today, by the kind its shape names — or why it cannot say.
fn measured(what: &str, modules: &[(&str, &str)]) -> Result<usize, String> {
    let module = |name: &str| {
        modules
            .iter()
            .find(|(module, _)| *module == name)
            .map(|(_, source)| *source)
            .ok_or_else(|| "names a module this guard does not walk".to_string())
    };
    if let Some(ty) = what.strip_prefix("impl ") {
        if definitions_of(modules, ty) != 1 {
            return Err(
                "sums every type of that name, so one could grow while another shrank \
                        and the number would not move: rename one of them first"
                    .to_string(),
            );
        }
        type_sizes(modules)
            .get(ty)
            .copied()
            .ok_or_else(|| "names a type with no impl block any more".to_string())
    } else if let Some((name, header)) = what.split_once("::") {
        the_one_block(module(name)?, header)
    } else {
        Ok(production_lines(module(what)?).len())
    }
}

/// The ceiling a ratchet entry would be under if it were not in the table: each kind its own, so a
/// module that fell under 600 is not «still too big» because it is over 300.
fn ceiling_of(what: &str) -> usize {
    if what.contains("impl ") {
        IMPL_CEILING
    } else {
        MODULE_CEILING
    }
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
    // one entry to two — so a floor over the sum survives every delivery of D28's order.
    //
    // **What catches a split that forgets its new files is the tree, not the count.** The count was
    // that guard once and it went slack twice — nineteen against a list of twenty-one, then
    // thirty-seven against forty-one — because a number raised by hand binds only the delivery that
    // remembers to raise it. So the list is checked against `src/` in both directions, and the
    // count stays as a floor against the walk itself collapsing.
    let declared: Vec<&str> = MODULES.iter().map(|(name, _)| *name).collect();
    let on_disk = rust_sources(Path::new(env!("CARGO_MANIFEST_DIR")), &["src"]);
    if let Err(failure) = list_against_disk("MODULES", &declared, &on_disk) {
        panic!("{failure}. A module with no row has no ceiling");
    }
    let total: usize = MODULES
        .iter()
        .map(|(_, src)| production_lines(src).len())
        .sum();
    assert!(
        MODULES.len() >= 37 && total >= 6_000,
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
    // and reads exactly like a crate in good shape. There are 121 of them today; the floor sits
    // below that on purpose, because merging two blocks of a type is a change the ratchet wants.
    assert!(
        seen >= 100,
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
fn no_type_is_over_its_ceiling() {
    let sizes = type_sizes(MODULES);
    // A floor over what the walk found, for the reason every guard here carries one: a `type_of`
    // that stopped matching would find no types and read exactly like a crate in shape. Sixty-eight
    // types are charged lines today; the floor leaves room for a delivery that deletes one.
    assert!(
        sizes.len() >= 55 && sizes.get("Window").copied().unwrap_or(0) > IMPL_CEILING,
        "the walk found {} types with impl blocks charged to them: it stopped recognising them",
        sizes.len()
    );
    let over: Vec<String> = sizes
        .iter()
        .filter_map(|(ty, size)| {
            let ceiling = ceiling_for(&format!("impl {ty}")).unwrap_or(IMPL_CEILING);
            (*size > ceiling).then(|| {
                format!("  impl {ty}: {size} production lines, over its ceiling of {ceiling}")
            })
        })
        .collect();
    assert!(
        over.is_empty(),
        "a type grew past what D28 allows it. A type's impl blocks — its own and those of the \
         traits this crate declares — summed across every module, hold {IMPL_CEILING} production lines — moving methods into another block or \
         another file does not pay it. The ones already over it are declared in \
         `OVER_CEILING_TODAY` **with the number they are at**, and that number may only go \
         down:\n{}",
        over.join("\n")
    );
}

#[test]
fn a_type_spread_across_modules_is_summed_as_one() {
    // Two blocks of one type in two files, one of them through a path, and a foreign trait's impl
    // beside them. The sizes are picked so each block is under the ceiling and the sum is over it,
    // which is the shape that walked through the guard when it only counted blocks.
    let body = "    fn f() {}\n".repeat(160);
    let first = format!("impl Thing {{\n{body}}}\n");
    let second = format!("impl crate::Thing {{\n{body}}}\nimpl Drop for Thing {{\n{body}}}\n");
    let sizes = type_sizes(&[("a.rs", &first), ("b.rs", &second)]);
    assert_eq!(sizes.get("Thing"), Some(&324));
    assert_eq!(
        sizes.len(),
        1,
        "a foreign trait's impl is charged to nobody: {sizes:?}"
    );
    assert!(sizes["Thing"] > IMPL_CEILING);
}

#[test]
fn a_trait_this_crate_declares_is_charged_to_the_type() {
    // The other way around the per-type ceiling: move the methods into the impl of a trait declared
    // for the purpose. Its lines stay the type's; a foreign trait's beside it stay nobody's.
    let body = "    fn f() {}\n".repeat(160);
    let own = format!("impl Thing {{\n{body}}}\n");
    let moved = format!(
        "pub(crate) trait Pointer {{\n    fn f();\n}}\nimpl Pointer for crate::Thing {{\n{body}}}\n\
         impl std::fmt::Display for Thing {{\n{body}}}\n"
    );
    let sizes = type_sizes(&[("a.rs", &own), ("b.rs", &moved)]);
    assert_eq!(sizes.get("Thing"), Some(&324), "{sizes:?}");
    assert_eq!(sizes.len(), 1, "{sizes:?}");
}

#[test]
fn every_impl_header_is_one_the_type_walk_can_read() {
    // `type_of` reads the first line of a block at column zero, with no generics of its own. A
    // header rustfmt wrapped would lose its ` for `, one with `impl<` would need its generics
    // parsed, `impl dyn` names no type, and an `impl` indented inside a module or a function is
    // never found at all. None of them exists today; the first one to appear fails here, where the
    // walk can be taught it, instead of charging its lines to the wrong type or to none.
    let unreadable = unreadable_headers(MODULES);
    assert!(
        unreadable.is_empty(),
        "impl headers the type walk cannot read:\n{}",
        unreadable.join("\n")
    );
    let odd = "impl<T> Thing<T> {\n}\nimpl VeryLongTrait\n    for Thing\n{\n}\nmod inner {\n    \
               impl super::Thing {\n    }\n}\nimpl dyn Shape {\n}\nimpl Fine {\n}\n";
    assert_eq!(unreadable_headers(&[("a.rs", odd)]).len(), 4);
}

/// Every production line that opens an `impl` the type walk would misread or miss.
fn unreadable_headers(modules: &[(&str, &str)]) -> Vec<String> {
    let mut unreadable = Vec::new();
    for (module, source) in modules {
        for line in production_lines(source) {
            let code = line.trim_start();
            let opens = code.starts_with("impl ") || code.starts_with("impl<");
            let nested = opens && code.len() < line.len();
            let odd =
                !line.ends_with(" {") || line.starts_with("impl<") || line.starts_with("impl dyn ");
            if opens && (nested || odd) {
                unreadable.push(format!("  {module}: {line}"));
            }
        }
    }
    unreadable
}

#[test]
fn a_ratchet_entry_is_measured_by_its_kind_and_refused_when_ambiguous() {
    let body = "    fn f() {}\n".repeat(3);
    let one =
        format!("struct Thing;\nstruct Strips;\nimpl Thing {{\n{body}}}\nimpl Strips {{\n}}\n");
    let two = format!("struct Strips;\nimpl Strips {{\n{body}}}\n");
    let modules = [("src/a.rs", one.as_str()), ("src/b.rs", two.as_str())];
    assert_eq!(measured("impl Thing", &modules), Ok(5));
    assert!(
        measured("impl Strips", &modules).is_err(),
        "two types share the name"
    );
    assert_eq!(measured("src/a.rs::impl Thing", &modules), Ok(5));
    assert_eq!(measured("src/b.rs", &modules), Ok(6));
    assert!(measured("src/c.rs", &modules).is_err());
    assert_eq!(ceiling_of("src/lib.rs"), MODULE_CEILING);
    assert_eq!(ceiling_of("impl Window"), IMPL_CEILING);
    assert_eq!(ceiling_of("src/lib.rs::impl Window"), IMPL_CEILING);
}

#[test]
fn a_ratchet_entry_that_names_two_blocks_is_refused() {
    // Nine names of this crate match more than one block of their module; an entry keyed by one of
    // them used to measure whichever block came first and lend its number to the other.
    let source =
        "impl Thing {\n    fn a() {}\n}\nimpl Thing {\n    fn b() {}\n}\nimpl Other {\n}\n";
    assert!(the_one_block(source, "impl Thing").is_err());
    assert_eq!(the_one_block(source, "impl Other"), Ok(2));
    assert!(the_one_block(source, "impl Gone").is_err());
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
        let real = measured(what, MODULES).unwrap_or_else(|problem| panic!("`{what}` {problem}"));
        assert!(
            real <= *at,
            "`{what}` is at {real} production lines and the ratchet records {at}: it grew. The \
             reason recorded for it is «{why}»"
        );
        let ceiling = ceiling_of(what);
        assert!(
            real > ceiling,
            "`{what}` is at {real} lines, under its ceiling of {ceiling}: it does not need an entry \
             any more, and leaving one reads as «still too big»"
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
