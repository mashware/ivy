#![deny(rustdoc::broken_intra_doc_links)]
// **A broken rustdoc link is invisible to the compiler, and half the repository is rustdoc.** When
// this was put in, the tree had **three** of them pointing at names that earlier renames had left
// behind: nobody noticed because `cargo doc` warns and does not fail. With the `deny` it fails, and
// the milestone that renames 46,458 lines is going to be grateful for it.
//
// **The command that makes it effective is `cargo doc-check`**, which is the `.cargo/config.toml`
// alias for `doc --workspace --no-deps --document-private-items`. And it has a name for a reason:
// without that flag the `deny` barely touches anything —rustdoc does not resolve the links of
// private items, and this crate is almost all private—, so its only gate was **remembering a flag
// written in a comment inside the very file it protects**. The two links in `app` that were still
// broken only came out with it. Test modules are never documented, so their rustdoc stays out
// whatever happens.
//
// It fires on `cargo doc`, **not** on `cargo test` — this repository has no continuous integration,
// so it still has to be invoked. What changes is that now it breaks instead of warning.
//! Window, panes and painting.
//!
//! The only crate tied to the interface framework. If the framework changes, this one is rewritten
//! and the other two stay exactly as they are.
//!
//! It is a library as well as a binary on purpose: the sweep bench that measures repainting has to
//! call the real painter, and if it duplicated it its number would stop being the product's.
//!
//! This is where the pieces come together: the bridge brings what the program writes, the grid
//! interprets it, the painter draws it and the keyboard goes back to the pseudoterminal. What is
//! still missing are the stopwatches.
//!
//! **The frame is split into two halves that do not do the same thing.** `logic` always runs, with
//! the window hidden too, and it is where the effects that cannot be repeated live: pumping,
//! interpreting and resizing. `ui` runs only when there is a real interface pass, and it is where
//! the keyboard is read and the painting happens. How it is shared out is not aesthetic: with the
//! window hidden, the input `logic` sees is the one of the last shown pass, so reading the keyboard
//! there would resend the last key pressed thousands of times per second during a dump.

#[cfg(test)]
#[macro_export]
macro_rules! windows_unix_omission {
    ($name:ident, $reason:literal) => {
        #[cfg(windows)]
        #[test]
        #[ignore = $reason]
        fn $name() {}
    };
}

mod agent_setup;
mod agent_signals;
mod ai_sessions;
mod alarms;
mod chrome;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod client_attestation;

#[cfg(all(test, windows))]
mod client_attestation {
    mod programs {
        mod tests {
            macro_rules! windows_unix_omission {
                ($name:ident) => {
                    #[test]
                    #[ignore = "unix-only: exercises Unix native client discovery and process identity"]
                    fn $name() {}
                };
            }
            windows_unix_omission!(desktop_path_discovers_native_claude_in_its_user_installation);
            windows_unix_omission!(native_claude_version_symlink_is_discovered_with_colon_in_home);
            windows_unix_omission!(explicit_path_keeps_precedence_over_native_claude_installation);
            windows_unix_omission!(
                unsupported_path_client_does_not_fall_back_to_another_installation
            );
            windows_unix_omission!(native_claude_fallback_refuses_scripts_missing_or_relative_home);
            windows_unix_omission!(
                native_fallback_does_not_discover_other_clients_or_nonexecutables
            );
            windows_unix_omission!(trusted_path_discards_empty_and_relative_entries);
        }
        mod child {
            mod tests {
                macro_rules! windows_unix_omission {
                    ($name:ident) => {
                        #[test]
                        #[ignore = "unix-only: exercises Unix resolver-child lifetime, process groups and pipe EOF"]
                        fn $name() {}
                    };
                }
                windows_unix_omission!(volta_stdout_and_success_are_collected_under_one_deadline);
                windows_unix_omission!(
                    volta_timeout_does_not_wait_after_kill_and_the_child_is_reaped
                );
                windows_unix_omission!(
                    volta_descendant_holding_stdout_cannot_extend_the_exchange_deadline
                );
                windows_unix_omission!(
                    resolver_cleanup_exhaustion_rejects_before_spawning_another_child
                );
            }
        }
    }
}
mod clipboard;
mod closing;
mod console_input;
pub(crate) mod control;
mod pane_view;
mod resume;
use panel::pane::{identity_of, panel_or_none};
mod board;
pub mod fonts;
mod git_process;
mod header;
mod injected;
pub mod keyboard;
mod launch;
mod ligatures;
pub use launch::enable as enable_scope_launcher;
mod links;
pub mod measure;
pub mod mouse;
mod opener;
mod opening;
pub mod painter;
pub mod panel;
mod pointer;
mod project;
mod project_view;
pub mod pty_output;
mod report;
mod restoration;
mod search;
pub mod session;
pub mod settings;
mod shell_integration;
use shell_integration::the_users_shell;
mod explorer;
mod sidebar;
mod startup;
mod tab_bar;
pub mod theme;
mod tiling;
mod tree;
mod ui;
mod viewer;
mod visibility;
pub mod watcher;
#[cfg(windows)]
mod windows_fs;

// **Brought back by name, and the names are the point.** The four modules named below came out of
// this file under **D28** and hold items it still uses. **They are not the whole order**, and the
// sentence below used to imply they were: `report`, `session` and `injected` also received pieces of
// this file and appear here in no `use` line, because what they took reaches back into the window
// rather than the other way round. Re-importing what the four hold is what keeps every call site, and
// the 16 952 lines of tests that reach into them, exactly as they were. A move that had to edit its own behavioural
// tests would not be a move, and across the whole order not one of them was touched.
//
// **By name and not with a glob**, which was the first shape of this and is the wrong one: a glob
// is **shadowed in silence** by a later item of this file that happens to share a name, so the
// day somebody writes a second `Divider` here the compiler says nothing and half the file starts
// meaning the other one. Named, that same day is a duplicate definition and the build stops. It
// also gives the reader the one thing the glob takes away — the list of what this file still needs
// back. For the list of what left, the order is in **D28**, not here.
#[cfg(test)]
use fonts::choose_the_font;
#[cfg(test)]
use opening::window_size;
use opening::{WindowRequest, how_many_tabs, opening_layout_of, size_at_startup};
use pointer::{DragIntent, WheelInput, WheelOutcome};
#[cfg(test)]
use tab_bar::set_the_tab_bar_aside;
use tiling::{DIVIDER_THICKNESS, Orientation};
#[cfg(test)]
use tiling::{Side, can_split};
use tree::Tab;

use eframe::egui;
use injected::{InjectedDrags, gesture_at_startup};
use measure::{RestOfFrame, Timings};
use painter::CellMetrics;
use panel_contract::PanelDir;
use pty_output::PtyOutput;
use std::time::{Instant, SystemTime};
#[cfg(test)]
use term_core::Pty;
use term_core::{Cell, Screen, Scroll, Size};
use theme::Theme;
use watcher::{Flag, Watcher};

/// The name the window presents itself to the desktop with.
const APP_NAME: &str = "Ivy";

/// The font size, in points.
const DEFAULT_FONT_SIZE: f32 = 14.0;

/// How much the grid swallows at most in one frame.
///
/// **It is counted in bytes and not in time, and that is deliberate.** With a deadline, the cost of
/// interpreting would by definition be worth whatever was set and the number would give back what
/// one put in oneself; with a fixed quantity of bytes the cost of interpreting is constant by
/// construction, everything else in the frame is the painter, and if swallowing this starts costing
/// milliseconds **that is the signal** the milestone is looking for.
///
/// The number comes from a measurement, not from an intuition: the grid absorbs 64.5 MiB/s, so this
/// is some 2 ms. A dump of 10 MiB takes about 80 frames — a little over a second— and does not lose
/// a single byte on the way.
const PUMP_BUDGET: usize = 128 * 1024;

/// How far the vector of what has been typed is allowed to grow before giving the memory back.
///
/// Typing it never gets close —it is a few bytes per frame—, so this only acts after a paste. The
/// value is not critical: it is enough for it to be roomy for normal use and small next to a real
/// clipboard.
const KEYSTROKE_SLACK: usize = 8 * 1024;

/// The size the grid starts with, before there is a window to measure.
///
/// It lives as long as the first interface pass takes: as soon as there is a real rectangle, the
/// grid starts measuring itself from it.
const INITIAL_GRID: Size = Size { rows: 24, cols: 80 };

/// What is grabbed of the panel's border, in points.
///
/// **It is used in two ways and it is best not to confuse them when touching the number:** with the
/// panel in view, on each side of the cut —a band of twice this, astride it—; **folded**, the whole
/// of it inwards from the right edge, which is also twice but **all of it inside the console**.
/// That second one is the band which, according to the comment of the gesture itself, prevents
/// grabbing the console there to take it somewhere else.
///
/// **Wider than the line of a tree divider, and on purpose.** A divider **is painted**, so it is
/// visible where to aim; the panel's border paints nothing —see [`Pane::drag_its_panel_divider`]—
/// and with two points you would have to sweep the console to find it. And egui's grace margin does
/// not help: its `interact_radius` widens the set of nearby candidates, but the drag is resolved by
/// exact containment of the rectangle.
///
/// Widening **discounts nothing** from the tiling, because this rectangle is not painted: it is
/// only where the mouse counts.
const PANEL_GRIP: f32 = 4.0;

/// What the window takes away from having painted a pane.
///
/// **It has a name and is not a loose pair**, for the same reason as [`crate::tiling::Tiling`] and
/// [`crate::panel::PanelTiling`]: two values that are returned together and mean different things
/// get swapped in the call without the compiler saying anything, and here one of the two feeds the
/// report of a measurement batch.
struct PaneOutcome {
    /// Where its panel's border ended up, or nothing if it had no panel in view. The gesture a
    /// batch injects into itself needs it: its hook runs **before** the interface pass, so it
    /// cannot compute it without duplicating the tiling.
    panel_divider: Option<f32>,
    /// Whether the gesture of the panel's border got to change its width **on this frame**.
    ///
    /// **It is what separates «a gesture was injected» from «the gesture was good for something»**,
    /// and without that second number a batch where none of them took would be published as «the
    /// gesture costs nothing». It is the same discipline as a console's drop, which counts the
    /// reorderings the tree accepts and not the drops that are sent.
    width_changed: bool,
    /// Where this pane's panel drew its first whole button, if it drew one. **For the bench's
    /// injector**, which presses there on the next frame — see `InjectedDrags::a_button_at`.
    first_button: Option<egui::Rect>,
    /// How many buttons this pane's panel drew **whole** on this frame.
    ///
    /// **Reported rather than counted by the caller**, because only the painting knows which of them
    /// survived the clip — and a button the clip cut in half is not drawn as far as a press is
    /// concerned. The report's line is read by the script in both directions (**D13**).
    buttons_painted: usize,
    /// Where the bench's link gesture can click, only when the measurement batch asks for it.
    links: links::ForTheBench,
    /// What it cost to rasterize color glyphs **while this pane was being painted**.
    ///
    /// **It travels through here and not through an accumulator of the window because it has to be
    /// subtracted from THIS console's span.** The discoverer belongs to the window, so its counter
    /// is shared; reading it when the frame closes would say how much was spent but not who to take
    /// it from.
    color: fonts::ColorCost,
    /// Whether this pane painted a cursor **that is in a blink**, either half of it.
    ///
    /// **It comes back from the painting because only the painting knows.** Three guards leave
    /// `paint_cursor` before any ink is chosen — no cursor in the view, a shape of `Hidden`, no cell
    /// where it stands — and a console that trips one of them wakes nobody however loudly its
    /// program asks. See [`alarms::BlinkState::painted`].
    blinking: bool,
    /// What painting the strip over this console cost **on this frame**, output or none, and
    /// nothing at all when no strip was cut.
    ///
    /// **It comes back through here instead of travelling inside the round's spans, and that is the
    /// whole difference.** A round is only closed on a frame where this console produced output, so
    /// a strip — which is painted on every frame the pane is painted — would have been sampled over
    /// the dumping frames alone and a quiet console would have published none at all. The window
    /// sums it over its panes and records it once per frame, next to where the tab bar's is
    /// recorded: see [`measure::Strips`]. **Not on the bar's terms**, which is the one difference
    /// that matters here — the bar's accumulator is a duration gated on not being zero, so it drops
    /// a frame it painted faster than the clock can see.
    ///
    /// **An `Option` and not a zero, because the report's count is read against the gate.** A strip
    /// drawn in less than the clock's resolution is still a frame that had one, and a zero cannot
    /// say that apart from a frame that had none — which is precisely the pair the measurement
    /// script throws a run away for.
    strip_cost: Option<std::time::Duration>,
    /// What searching this console cost on this painting. See [`search::Cost`].
    search: search::Cost,
}

/// The environment variable that says how many tabs to open with.
///
/// **It exists in order to be able to measure with nobody in front of it, and that is why it
/// carries the prefix of the measurement script** — `MEASURE_MIB` and `MEASURE_BATCH` already live
/// in `scripts/measure-frame.sh`. The shortcuts that create tabs come in through framework events,
/// so a script has no way of opening the second one; without this gate, the two-tab batch of D3's
/// annex cannot be launched.
///
/// It is written without an eñe on purpose: it is what a shell script has to be able to type
/// without encoding surprises, and its two sisters are already ASCII.
const TABS_VARIABLE: &str = "MEASURE_TABS";

/// The environment variable that says what size to open the window with, in points:
/// `<width>x<height>`.
///
/// **It exists because without it the measurement is not reproducible, and that is checked**:
/// without asking for a size the desktop decides, and three runs of the same batch on the same
/// machine on the same day gave three different grids —61×57, 40×66 and 36×47—. Every measurement
/// in the project hangs off the grid: two batches with different grids cannot be averaged or
/// compared, and nothing in the report gave it away.
///
/// **It is only asked for when measuring, and not always**, because changing how the window opens
/// for whoever uses it is not asked for by any phase: without the variable, the desktop goes on
/// deciding, which is the behavior of always.
///
/// It is written without an eñe for the same reason as its three sisters: it is what a shell script
/// can type without encoding surprises.
const SIZE_VARIABLE: &str = "MEASURE_SIZE";

/// The environment variable that says what layout of consoles to open each tab with:
/// `<columns>x<rows>`.
///
/// **It exists for the same reason as [`TABS_VARIABLE`]**: splitting comes in through framework
/// events, so a script has no way of pressing the chord, and without this gate the four-pane batch
/// —the one the tree milestone has to measure— cannot be launched with nobody in front of it.
///
/// **It opens the layout instead of splitting four times**, which is what the two columns of always
/// already do: the tab is born with its tree and nobody consults the floor of [`tiling::can_split`] at
/// startup. Whoever asks for a layout that does not fit in their window will see consoles one
/// column wide, just as happens today with a tiny window.
const PANES_VARIABLE: &str = "MEASURE_PANES";

/// The environment variable that says **how many drag gestures** to inject during the batch.
///
/// **It exists because a drag does not happen on its own, and without it the declaration of the
/// gesture's cost is worth zero.** The criterion this milestone has to close says that *when there
/// have been drags during the batch* their cost must be declared and outside «the rest of the
/// frame». The batches run **with nobody in front of them** —the consoles launch their load on
/// their own and the window closes on its own—, so without this gate the report would publish a
/// zero and nobody would be able to tell it apart from «it costs nothing»: exactly the disguise of
/// «declaring zero» this project already has on file.
///
/// **The events come in where those of the real mouse come in** ([`eframe::App::raw_input_hook`],
/// which runs just before egui processes the batch of events), and not by calling the gesture's
/// effect by hand. It is the only thing that measures the real path: the `interact` per console,
/// the target's veil and the tiling that is redone on dropping. Calling the tree's operation
/// directly would measure a function, and the annex compares **frames**.
///
/// It is written without an eñe for the same reason as its three sisters: it is what a shell script
/// can type without encoding surprises.
pub(crate) const DRAGS_VARIABLE: &str = "MEASURE_DRAGS";

/// Which gesture the batch asks for, and `console` when nothing is said.
///
/// **By default the console one**, so that the batches that already existed go on measuring exactly
/// what they measured: an old batch that suddenly dragged panels would publish its numbers under
/// the same letter.
pub(crate) const GESTURE_VARIABLE: &str = "MEASURE_GESTURE";

/// The five variables that say this session was asked for by a measurement batch.
///
/// **A named list, and not five loose reads, because two vetoes hang off it**: that the batch does
/// not step on the layout of whoever measures, and that it does not read their settings. Written by
/// hand in the place where they are read, **adding a sixth and forgetting to put it there was
/// caught by nobody** —not the suite, not the script, not a review, because the new variable would
/// live in another file—. It was caught by a review that mutated the whole expression to `false`
/// with the suite green. Whether any of the measurement gate's variables is set.
///
/// **It receives whoever looks at the environment instead of looking at it, and that is the only
/// thing that makes it checkable.** Setting variables from a test is undefined behavior in a
/// process with threads, and this suite runs in parallel launching processes; without this cut,
/// **the whole expression could be swapped for `false`** — turning off the two vetoes that hang off
/// it— with the suite green. It was caught by a review.
fn any_measurement_variable(is_set: impl Fn(&str) -> bool) -> bool {
    MEASUREMENT_VARIABLES.iter().any(|which| is_set(which))
}

const MEASUREMENT_VARIABLES: [&str; 6] = [
    TABS_VARIABLE,
    PANES_VARIABLE,
    SIZE_VARIABLE,
    DRAGS_VARIABLE,
    GESTURE_VARIABLE,
    "MEASURE_BOARD",
];

/// How many cells the panel's border is pushed **on every drag frame** of the injected gesture.
///
/// **Per frame and not per gesture, and that was found out by probing.** The point the gesture
/// takes the pointer to is computed from where the border is **now**, so with two drag frames a
/// whole gesture moves it twice as far: the width goes from 36 to 28 and from 28 to 20. It is a
/// realistic drag —a mouse that moves while the button is pressed does exactly that— but the sum
/// has to be done with the number of frames in front of you, or the annex publishes a travel that
/// is not the one that happened.
///
/// **In cells and not in points, because what the gesture has to cross are cell boundaries:** that
/// is where the panel's width really changes and where its layout is redone. Measured in points or
/// in a fraction of the width, the same batch in another window or with another font size would
/// cross a different number of them and its numbers could not be compared.
///
/// Eight: enough to cross several on each gesture, and few enough that the panel does not reach its
/// minimum or its ceiling with the factory width — reaching an extreme would turn half the gestures
/// into a no-op and the batch would measure two regimes. The chord a console is grabbed with to
/// take it somewhere else, since **D20**.
///
/// **It lives here and not repeated in each place** because three paths that have to say the same
/// thing share it: the one that tiles the real gesture, the gesture the measurement batch injects
/// into itself, and the tests. With the constant loose, changing the chord in one of them leaves
/// the others measuring or testing the gesture that no longer is.
pub(crate) const MOVE_CHORD: egui::Modifiers = egui::Modifiers {
    alt: false,
    ctrl: true,
    shift: true,
    mac_cmd: false,
    command: false,
};

pub(crate) const CELLS_THE_PANEL_GESTURE_CROSSES: f32 = 8.0;

/// How many cells the tree's divider is pushed **on every drag frame** of the injected gesture.
///
/// **Per frame and not per gesture**, and in cells and not in a fraction, for the two reasons that
/// already justify its sibling the panel one: the point is computed from where the border is
/// **now**, and what has to be crossed are cell edges.
///
/// **Eight is the same number and not the same sum, and here the margin is narrow.** The floor is
/// not the panel's minimum width but [`panel::MINIMUM_CONSOLE`]: with the batch's window and two
/// panes with a panel, a whole gesture moves the border sixteen cells and the console that narrows
/// goes from 39 columns down to **23** — three above its twenty. **With ten it already touches the
/// floor**, measured, and then half the gestures would be no-ops against it and the batch would
/// average two regimes under the same label, with the report declaring effects all the same: no
/// guard of the script would catch it.
///
/// That is why it is not raised without coming back here, and the same goes for the window size,
/// the font size and the panel's factory width. Tied down by
/// `the_divider_gestures_round_trip_leaves_no_console_at_its_floor`.
pub(crate) const CELLS_THE_DIVIDER_GESTURE_CROSSES: f32 = 8.0;

/// How many cells the stretch the injected gesture marks spans, on its long pull.
///
/// **Twenty, and the number does NOT come from marking more costing more** — that is what this
/// comment said and the measurement disproved it. While a selection is alive, composing the picture
/// asks **for every cell of the grid** whether it is highlighted, be there two marked or two
/// thousand: the cost grows with the grid, not with the stretch (**D22**). What twenty buys is that
/// the stretch is **that of a real gesture** —a path, a command— and that it fits with plenty to
/// spare in the narrowest console a batch opens, which is 47 columns.
pub(crate) const CELLS_THE_GESTURE_MARKS: f32 = 20.0;

/// How many frames the drag span of an injected gesture lasts, between pressing and releasing.
///
/// **Two and not one, and the reason is NOT that with one it would not be a drag** — that is what
/// this comment said before the review and it was false. With one, the only movement jumps straight
/// to the target's edge: it is hundreds of points from where it was pressed, well above the
/// threshold at which egui tells a click from a drag, so the drag starts and the veil is painted
/// all the same.
///
/// **What the second frame adds is the state in flight WITHOUT a valid target**: the pointer over
/// the very console being dragged, where there is no veil to paint. Measuring a drag is those two
/// states, and with only one the batch would never go over the first.
pub(crate) const FRAMES_DRAGGING: usize = 2;

/// On how many frames the size is asked for again before letting it go. See [`RequestedSize`].
///
/// **It counts frames with the wrong size, not answered requests**, and that is where the number
/// comes from. A resize on Wayland needs a configuration round trip from the compositor, that is
/// **at least one frame**, and at startup the frames go one after another: with a cap of four, a
/// compositor that takes five to answer —a slower machine than the one that measured this, a busier
/// session— would exhaust the attempts **before the first answer arrives** and the window would not
/// ask for anything again, throwing away every batch on that machine. Thirty-two frames are a blink
/// next to the hundreds a batch has, and they still bound the fight.
const FRAMES_INSISTING_ON_SIZE: u8 = 32;

/// How far the window's size may deviate from the one asked for to pass it as good, in points.
///
/// **Half a point, and not zero.** A compositor can deliver a size with a fraction —scaling
/// rounding—, and with exact equality the window would spend the four attempts fighting over a
/// difference that does not move a single pixel, let alone a cell of the grid, which is what the
/// measurement compares.
const SIZE_TOLERANCE: f32 = 0.5;

/// How often the window looks again when it has been left with nobody to warn it.
///
/// Half a second is what it takes to notice that a panel is running late without the window being
/// awake the whole time. **It only runs in degraded mode**: while the watcher works, the window
/// sleeps until somebody publishes.
const POLL_WITHOUT_A_WATCHER: std::time::Duration = std::time::Duration::from_millis(500);

/// What the frame needs to know after pumping every console.
struct Pump {
    /// The budget cut one of them off and it has output left to swallow, so another frame is
    /// needed.
    some_left_a_queue: bool,
    /// They have all died and none has a queue: **only then** is the window closed.
    all_finished: bool,
}

/// How many consoles each tab opens with, and in what shape.
///
/// **It is a layout and not a number of panes.** Four consoles in a row and four in a square are
/// the same quantity and **two different measurements**: in a row each one keeps a quarter of the
/// width and the panel does not fit —which is literally the open question of
/// [#33](https://github.com/mashware/ivy-history-private/issues/33)—, and in a square each one has half
/// the width and the panel goes in. A single number would force inventing which of the two is
/// measured. **Both axes are strictly positive, and the type guarantees it** — that is why the
/// constructor returns `Option` instead of correcting a zero. Zero columns or zero rows is not a
/// small layout: it is a tab with no console at all, and out of that comes a branch with no
/// children whose sum of weights is zero. It is the same criterion as [`crate::tiling::Weight`], and for the same
/// reason.
struct OpeningLayout {
    columns: usize,
    rows: usize,
}

impl OpeningLayout {
    /// The one the window opens with if nobody asks for another: the two columns of always.
    const DEFAULT: Self = Self {
        columns: 2,
        rows: 1,
    };

    /// **It also rejects what cannot be counted**, and it is not zeal: both axes come in through
    /// the environment, and `4294967296x4294967296` passes both greater than zero while its product
    /// **wraps round to zero** in release. Out of that comes a tab with no console at all, which is
    /// just what this type promises cannot exist: the application would crash on startup
    /// contradicting its own contract —«a value that is not understood leaves the layout of
    /// always»—. Rejecting the impossible product **is not inventing a ceiling** on panes, which is
    /// a decision already taken the other way round: it is refusing to represent what does not fit
    /// in a number.
    fn new(columns: usize, rows: usize) -> Option<Self> {
        let panes = columns.checked_mul(rows)?;

        (panes > 0).then_some(Self { columns, rows })
    }

    /// **It cannot overflow**, and [`OpeningLayout::new`] guarantees it: it is the only place that
    /// builds a layout with values from outside, and it already checked that the product fits. The
    /// other one, [`OpeningLayout::DEFAULT`], is a literal of two by one.
    fn how_many_panes(&self) -> usize {
        self.columns * self.rows
    }
}

/// What the window opens with: what the measurement asks for, or what was saved, or the factory
/// one.
enum OpenWith {
    /// So many tabs of so many consoles, which is what the application has done since F1.
    Factory { tabs: usize, layout: OpeningLayout },
    /// The layout that was left saved the last time.
    Restored(session::Validated),
}

/// The decision, with the given values: **what a measurement batch asks for beats what was saved**.
///
/// **The precedence is written down because without it a run measures a layout nobody asked it
/// for.** The batches ask for tabs and panes by environment variable, and the report declares what
/// was asked for so that the script invalidates the run if it does not add up; with the file
/// winning, a two-console batch would open the nine the user left placed and the witness would see
/// nothing odd.
///
/// **It is enough for the variable to BE there, even if it is not understood.** A batch that writes
/// it wrong has asked for something, and what the run itself does with an unreadable value is
/// already decided —it keeps the one of always— and its script catches it by comparing with what
/// the report declares. Looking at whether it is also understood would be a second rule that
/// protects from nothing.
fn open_with(
    tabs: Option<&str>,
    panes: Option<&str>,
    saved: Option<session::Validated>,
) -> OpenWith {
    let factory = OpenWith::Factory {
        tabs: how_many_tabs(tabs),
        layout: opening_layout_of(panes),
    };

    if tabs.is_some() || panes.is_some() {
        return factory;
    }

    match saved {
        Some(validated) => OpenWith::Restored(validated),
        None => factory,
    }
}

/// Opens the window and does not come back until it is closed.
///
/// **The environment is read here, and nowhere else.** [`Window::with_tabs`] receives the number
/// already resolved so that no test depends on what the session that launches it has exported; the
/// price is that this line, like [`eframe::App::on_exit`]'s, is covered by no test: changing the
/// `1` of [`open_with`] for a fixed value would leave the whole suite green and the measurement
/// without its gate. Whoever measures finds out as soon as they open the window and see a single
/// tab.
pub fn run() -> eframe::Result {
    startup::run()
}

/// A console and everything needed to hold it up.
///
/// Every field in here was a field of [`Window`] when there was only one console. Grouping them is
/// what allows the whole set to be instantiated more than once without writing every operation
/// twice.
struct Pane {
    signals: agent_signals::Signals,
    /// What it is called in the report, and **it does not change even if others are closed**.
    ///
    /// **The index in the layout is no good for this**, for the same reason it was no good for the
    /// tabs: as soon as a pane can be closed, the ones beside it move up a place and the report
    /// would put another console's name on a table of numbers — with the spans of the one that
    /// produced them, and with nothing in the text that lets anyone notice. It is assigned when the
    /// pane is opened and never reused.
    ///
    /// **It goes in here and not in a parallel list of the tab's.** Two vectors that have to be
    /// kept aligned are an invariant somebody will break; inside the type there is nothing to
    /// align.
    name: usize,
    /// Explicit display label; never used as process input or identity.
    label: Option<String>,
    project: Option<project_view::Badge>,
    /// The foreground program and its folder's name, sampled with the directory: what a header
    /// says when no agent titles the console and nobody named it.
    program: Option<(String, String)>,
    restoration: restoration::PaneState,
    /// The pseudoterminal and its queue go together on purpose. The bridge's thread lives as long
    /// as the pseudoterminal lives, not as long as the bridge does: separating them would leave a
    /// thread and a queue with no owner, growing.
    pty: launch::Process,
    pty_output: PtyOutput,
    /// Startup assets stay alive until the console's shell no longer needs them.
    _shell_integration: Option<shell_integration::Prepared>,
    /// The directory where this console's program publishes its state, and the identifier it is
    /// known by. It is deleted when the console is dropped, because the type deletes it when it
    /// dies.
    ///
    /// **It is optional because it can genuinely be missing**, not out of caution: a read-only
    /// working directory in a container, a full disk, or the environment variable pointing at
    /// somewhere that cannot be written. When it is missing, the program inside sees neither of the
    /// two variables, the ones it would have inherited are taken away from it, and it behaves as it
    /// did before this milestone existed.
    ///
    /// It is read **while alive too** since F1.6: it is where this console's status panel comes
    /// from.
    panel: Option<PanelDir>,
    /// This console's end of the control channel, when it has one.
    ///
    /// **Optional for the same reason the directory above is**: a pane whose endpoint could not be
    /// bound is a complete pane that publishes by file, exactly as every pane did before this
    /// milestone. It is dropped with the pane, which is what asks its threads to end.
    control: Option<control::PaneControl>,
    /// What is known about this console's panel: its last good content and when it was read.
    panel_state: panel::PanelState,
    /// Whether this console's `panel.json` has to be read again. The watcher raises it and the
    /// frame picks it up.
    ///
    /// **It belongs to the pane and not to the watcher** because it is what makes a notice wake one
    /// console and not both: the watcher is one for all of them and the only thing that tells them
    /// apart is which flag its notice reaches.
    to_reread: Flag,
    screen: Screen,
    /// The terminal snapshot and rectangle that produced the pixels currently on screen.
    ///
    /// Link resolution reads this instead of [`Self::screen`]: output can be interpreted between a
    /// paint and a click, and then the live grid is already newer than what the person aimed at.
    /// Selection and the program's mouse reports still read the live grid.
    painted_console: Option<links::PaintedConsole>,
    /// What is picked up from the bridge in one frame, reused between frames so as not to allocate
    /// memory in every one. It never grows beyond the budget: it is asked for with a cap and
    /// interpreted whole.
    received: Vec<u8>,
    /// The bytes the keys of this pass send to the program. Reused for the same reason.
    keystrokes: Vec<u8>,
    /// A paste that was **not** sent, waiting for the person in front of the screen to say whether
    /// it should be.
    ///
    /// It survives between frames because the question does: the bytes are held, not written, and
    /// nothing reaches the program until somebody answers. Holding rather than warning is the whole
    /// point — once the line closes, the shell has already run the truncated command.
    held: Option<keyboard::HeldPaste>,
    /// The search open on this console, and what searching it cost. See [`search::OnThisConsole`].
    search: search::OnThisConsole,
    /// The size the last UI pass measured.
    ///
    /// It is measured where there is a rectangle —`ui`— and applied where the effects live
    /// —`logic`—, so it always arrives one frame late. It does not show: between two frames of a
    /// drag there is no time to see the old grid.
    measured: Size,
    /// This console's stopwatches, and not the window's: that way its spans keep comparing against
    /// D3's tables whether there is one pane or four.
    timings: Timings,
    /// What the first half of the frame already knows about the round trip —since when what was
    /// picked up had been waiting, when it was picked up and what interpreting it cost—, waiting
    /// for the UI pass to add what queueing the painting costs.
    pending_measurement: Option<(Option<Instant>, Instant, std::time::Duration)>,
    /// What it has cost to re-read this console's `panel.json` **without getting to paint it**,
    /// waiting for its round trip to close at the end of the UI pass.
    ///
    /// A console of a background tab re-reads its panel just like the one on show —how much width
    /// it has left depends on whether there is a panel—, but it does not go through the painting,
    /// which is where that span is declared. Without this field, reading a file from disk would sit
    /// inside the frame with nobody timing it and the subtraction would hand it to the UI
    /// framework.
    ///
    /// **It lives for an instant: it is written when measuring and emptied when the round trip
    /// closes, within the same pass.** It does not need to accumulate across passes, and it is
    /// worth knowing why: eframe calls `logic` **and** `ui` inside the same closure that egui
    /// repeats (`epi_integration.rs:288-303`), so a repeated pass pumps again and leaves a round
    /// trip pending again. There is never a read waiting for a round trip that does not come.
    panel_reread_unpainted: std::time::Duration,
    /// The same, for what applying the control channel's deliveries cost on a console nobody
    /// painted. It travels beside the one above and for the same reason — see [`measure::Spans`],
    /// where the two are separate fields.
    control_unpainted: std::time::Duration,
    /// What it has cost to reflow this console's text in this frame, waiting for the window to pick
    /// it up.
    ///
    /// **It lives here and not in the window because the work is the console's** —its width
    /// triggers it— but **the window publishes it**, which is who has the report. It accumulates
    /// instead of being replaced for the same reason as the round trip's spans: a repeated pass
    /// comes through here again and the work happened both times.
    reflowed: std::time::Duration,
    /// What the wheel has asked for and **does not yet add up to a whole movement**.
    ///
    /// **It belongs to the pane and not to the window**, because the wheel goes to the console
    /// under the pointer: kept in the window, moving the mouse over another console would hand it
    /// the remainder accumulated in the first one.
    pending_wheel: WheelInput,
    /// How many cells of width **this** console's panel takes.
    ///
    /// **In cells and not in points, and that is why it is an integer.** What the panel has to keep
    /// when the desktop's zoom changes is how many characters fit, not how many points it takes
    /// (D10). And the integer pays for itself: the panel's layout is already saved **by whole
    /// cells**, so dragging does not redo it at every point but on crossing a cell.
    ///
    /// **It belongs to the pane and not to the window.** Four panes are four producers publishing
    /// things of different sizes, and the point of being able to narrow one is to leave wide the
    /// one that says something and narrow the one that does not. An application-wide width would
    /// also have to yield to the minimum of the narrowest pane, or lie in all the others.
    panel_width: usize,
    /// Why an agent's access to this console ended, once the person took it over.
    ///
    /// **It lives on the pane because that is what decides it**: any user input that reaches this
    /// console —a keystroke, a paste, a mouse report— is a write into *its* pseudoterminal, and the
    /// pane is what those write sites already hold. While it is `None` the handles that opened it
    /// work; from the first byte on they are refused, with this sentence as the reason (**AC15**).
    taken_over: Option<String>,
    /// Whether this console's panel is folded.
    ///
    /// **It goes apart from the width and not inside it, and that is the decision.** A «zero width»
    /// that also meant folded would lose the working width, and unfolding would have to invent a
    /// value — when what the user expects is to get **theirs** back. With two fields, folding does
    /// not touch the width: it stops being asked for, and on unfolding it is still there.
    folded: bool,
}

/// Who gets the turn of the wheel over a console.
///
/// The order is fixed by **D23** and is decided whole before moving anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WheelOwner {
    /// Going up and down through what went off the top. The program does not notice.
    Scrollback,
    /// The mouse report, with its own two numbers.
    Program,
    /// The arrows, which is what makes `less` scroll without asking for the mouse.
    ArrowKeys,
}

/// The root window: the tabs, and what belongs to the program and not to any layout.
struct Window {
    /// Who gives notice that a producer has published, over the paths of the open consoles.
    /// Declared before `tabs` as a structural guard beside the explicit whole-window stop: pane
    /// retirement must never be reported as lost panel tracking.
    watcher: Watcher,
    /// The open layouts. **Never empty**: the window is always showing one.
    ///
    /// It opens with one, or with however many the measurement gate's variable says, and from then
    /// on they are created and closed by whoever uses the window.
    tabs: Vec<Tab>,
    /// Which one is on show and which one the keyboard goes to. **Always smaller than the number of
    /// tabs.**
    active: usize,
    /// How the modifier keys stood at the end of the previous pass.
    ///
    /// **It lives here and not in the pane because they belong to the keyboard, not to a console.**
    /// They have to be saved because a chord can begin in one frame and be used in the next: if
    /// Ctrl+Shift are held down and the V is released a frame later, the notice that they were
    /// pressed arrived in the previous batch and is no longer in this one. And saving them in the
    /// window is what makes switching console with the chord held down not lose it.
    modifiers: egui::Modifiers,
    /// Who looks in the machine for the fonts that are needed, **when they are needed**.
    ///
    /// **One per window and not one per console**, because the faces are registered with the text
    /// engine, which belongs to the window: two discoverers would look for the same thing twice and
    /// the second would erase for the first whatever it had registered.
    discoverer: fonts::Discoverer,
    /// What it costs to look for those fonts, so that the report declares it and it does not end up
    /// published as a cost of the framework.
    discovery: measure::Discovery,
    /// What it costs to rasterize the color glyphs, so that it does not end up inside the
    /// painting's span.
    color: measure::Rasterizing,
    /// What searching the consoles' scrollback cost this run. See [`measure::Searching`].
    searching: measure::Searching,
    theme: Theme,
    /// Which chord does what.
    ///
    /// **It goes next to the theme because it comes from the same place**: it is what whoever uses
    /// the terminal configured.
    keymap: keyboard::Keymap,
    /// Whether the copy shortcut is switched on.
    ///
    /// **It is a switch and not a chord, because its chord is decided by the desktop**:
    /// `Ctrl+Shift+C` does not arrive as a key, it arrives already turned into a request to copy.
    /// The same as pasting.
    copy_enabled: bool,
    ligatures: bool,
    ligature_measurement: Option<ligatures::Measurement>,
    /// Everything a blinking cursor needs the window to remember between frames.
    ///
    /// **One field for three things**, because this file cannot grow (**D28**) and because the three
    /// are one subject. See [`alarms::BlinkState`].
    blink: alarms::BlinkState,
    /// The font size the cell is measured with, in points.
    ///
    /// **It goes stuck to the theme because the two come from the same place**: they are what
    /// whoever uses the terminal configured, and the same function writes them. And **it is not a
    /// constant since it became configurable**: changing it changes the grid —how many rows and
    /// columns fit— and therefore what interpreting and painting cost. It is not an aesthetic
    /// setting: it is the parameter the whole measurement history depends on.
    font_size: f32,
    /// The cost of the pass nobody timed; see [`RestOfFrame`] for why it belongs to the window.
    rest_of_frame: RestOfFrame,
    /// When the frame in progress began, which is where the outbound stopwatch starts.
    frame_started: Instant,
    /// The last frame whose cost was already recorded, so as not to count it twice when `logic`
    /// runs again on the same frame number: egui repeating the pass runs the whole closure again,
    /// and eframe's logic-only ticks while the window is hidden do not advance the number.
    last_closed_frame: u64,
    /// What this frame has spent **outside the panes that are still alive**, and which therefore
    /// none of them is going to declare.
    ///
    /// There are two things, and both inflated «the rest of the frame» —that is, the cost
    /// attributed to the UI framework, which is the figure D3 decides with—:
    ///
    /// 1. **Opening a tab is two `fork`/`exec`s**, and they happen inside the UI pass because that
    ///    is where the chord is attended. Nobody was timing them.
    /// 2. **Closing a tab takes its panes, and with them their [`measure::Timings`]** — including
    ///    the round trip they already had recorded for this same frame. That work happened, so if
    ///    it leaves without being declared, the rest keeps it.
    ///
    /// It is emptied when the frame is closed. **It carries no series of its own in the report**:
    /// what is needed is that it be **subtracted**, and one more series for an event that happens
    /// when the user presses a shortcut would say very little and would have to be explained in
    /// full.
    timed_separately: std::time::Duration,
    /// What it has cost to paint the bar **in the frame in progress**, still unrecorded.
    ///
    /// **It exists because egui repeats the pass** after a discard, and both passes really
    /// paint. Recording in each one, the counter would say «frames» and would be counting passes,
    /// and the mean the report publishes would come out diluted under a label that promises
    /// something else. The repeated work **is** added up —it happened twice—, but the frame is
    /// still one.
    tab_bar_this_frame: std::time::Duration,
    /// What it has cost to paint the tab bar, so that the report publishes it under its name.
    ///
    /// **It goes apart from the drawer above even though it is also added to it.** The drawer is
    /// what is subtracted from the frame —in there it cannot be told who put what—, and this is
    /// what the report **names**: without naming it, the only way to know what the bar costs would
    /// be to subtract two different batches.
    tab_bar_cost: measure::TabBar,
    /// What the strips have cost — the frame in progress, and the run the report publishes.
    ///
    /// **One field and not two, and the type is what holds the rule**: what a strip costs goes into
    /// this tally **and** out of the frame's rest in the same statement, because either one
    /// forgotten publishes a lie — see [`report::Strips`], where the two lies are named. The tab
    /// bar's pair keeps them apart on this struct and states the rule in a comment; this one states
    /// it where a test can reach it.
    strips: report::Strips,
    /// What the panel's buttons drew and what was pressed this run. **For the report, which has to
    /// declare a painted path it did not have before** (D13).
    buttons: measure::Buttons,
    /// Link sources the bench found and accepted activations, published for the measurement gate.
    links: measure::Links,
    /// Where an accepted link goes: the desktop's opener, or a recorder under test.
    desktop: links::Desktop,
    /// The channel counters of panes **that have already closed**, so a run that closes one does not
    /// lose what it did.
    ///
    /// **It lives on the window for the same reason [`Self::buttons`] does.** `report` used to fold
    /// the counters out of the live panes alone, so a pane or a tab closed during a run took its
    /// `applied`/`refused`/`released` with it — and `measure-frame.sh` can then invalidate a run
    /// that was correct, which is the exact **D13** failure the `Control` docblock itself invokes.
    /// Live panes are still added on top when the report is built.
    control: control::WindowActivity,
    /// What it has cost to reflow the consoles' text when their width changed.
    ///
    /// **It lives in the window and not in each pane even though the work is the console's**,
    /// because what has to be readable is how many times it was reflowed **in the session** and how
    /// much each time cost: a mean per console spread over its thousands of still frames would say
    /// it costs nothing.
    reflow: measure::Reflow,
    launch_cost: launch::Cost,
    /// Where the layout goes, and what this session already knows about that file.
    ///
    /// **It is the same one that loaded on starting, and that is a rule and not a coincidence**:
    /// the veto of a newer version lives here, so a second instance would lose it and would wipe
    /// out the layout of a newer binary. The type's doc says so, and it was the named debt left by
    /// the PR that learned to restore.
    persistence: session::Persistence,
    /// Where the consoles came from, when they came from the file: how many they were and what it
    /// cost to open them.
    ///
    /// **Only the branch that restores fills it in**, so `None` means «this window was opened by a
    /// request, not by a file» — which is precisely what the report needs to tell apart and the
    /// console count does not.
    restoring: Option<measure::Restoring>,
    /// What it cost to read the settings of whoever uses the terminal, and whether they were read
    /// at all.
    reading_settings: measure::ReadingSettings,
    /// What choosing the font cost, and whether one was asked for at all.
    chosen_font: measure::ChoosingTheFont,
    /// Whether this session paints a strip over each console.
    ///
    /// **Resolved once, where the settings the same gate vetoes are resolved**, and carried from
    /// there: the geometry, the painting and the pointer all read this one value, so there is no
    /// regime where a row is reserved and not drawn. See [`header::Headers`].
    headers: header::Headers,
    header_input: header::actions::Input,
    settings_editor: settings::editor::Editor,
    chrome: chrome::Chrome,
    pane_view: pane_view::State,
    clipboard: clipboard::Service,
    board: board::State,
    sidebar: sidebar::Sidebar,
    viewer_other: viewer::State,
    viewer: viewer::State,
    explorer: explorer::Explorer,
    projects: project_view::State,
    restoration: restoration::State,
    /// The previous frame's mirror. **The deadline is armed against it**, so the deadline is
    /// rearmed **when something changes** and not when something is unwritten: without this, a
    /// non-writable directory would leave the difference in place and **the window would never go
    /// to sleep**.
    last_seen: Option<session::StoredDocument>,
    /// What it has cost to compare the mirror in this frame, **adding up its passes**.
    ///
    /// egui repeats the UI pass, so recording per pass would make the mean describe «per pass»
    /// instead of «per frame». It is accumulated here and recorded **once on closing**.
    mirror_this_frame: std::time::Duration,
    /// The two spans that saving costs, with their two rows in the report.
    saving: measure::Saving,
    /// The drag gestures the measurement batch performs on itself, if it asked for any.
    ///
    /// **Nothing when nobody asks for them**, which is always except in one batch: that way the
    /// path does not get to run for whoever uses the application. See [`DRAGS_VARIABLE`].
    injected_drags: Option<InjectedDrags>,
    /// What the raw-input hook spent injecting the gesture, waiting to be declared.
    ///
    /// **It exists because of the ORDER and not for convenience.** That hook runs just before the
    /// frame closes the previous one, so adding it straight into [`Window::timed_separately`] would
    /// let it be taken by the emptying of the frame **that is being closed** — one whose CPU cost
    /// does not include it, because the work happened after it had ended. The rest of that one
    /// would come out short and this one's inflated. It is parked here and declared to the frame
    /// that begins, which is the one that paid for it.
    spent_injecting: std::time::Duration,
    /// With which chord the primary button was pressed the last time.
    ///
    /// **It exists because the press and the drag do not fall in the same frame**, and that cost
    /// something to discover: egui gives a drag as begun when the pointer has moved **from** where
    /// it was pressed, that is, at the earliest in the following frame. Looking at the queue in the
    /// frame of the `drag_started()` finds no press —it has already gone by— and the gesture would
    /// always be resolved as if nobody had touched a single key.
    ///
    /// It is noted down on seeing the press and read on the drag beginning. **It is not cleared in
    /// between**: between the two frames there is nothing that invalidates it, and clearing it in
    /// the first one that brings no press would erase it right before it is used.
    chord_of_the_last_press: egui::Modifiers,
    /// Where that same press fell.
    ///
    /// **Its own field beside the chord, and written in the same statement as it**: the two answer
    /// «what did the user mean by this press», one from the keyboard and the other from the
    /// geometry, and the strip needs the second — a drag that begins a frame later has no way back
    /// to where the button went down. `None` until the first press: a window nobody has touched has
    /// no point to compare, and a corner invented for it would put every gesture on the strip of
    /// the top-left pane.
    where_the_last_press_fell: Option<egui::Pos2>,
    /// The last frame in which presses and releases were shared out, and **nothing** until the
    /// first one.
    ///
    /// See the reason where it is used: button events cannot be consumed, so an egui that replayed
    /// them on a repeated pass would send the same click twice. egui 0.36 does not — it is defence,
    /// and `egui_hands_a_frame_s_events_to_its_first_pass_only` trips if that changes.
    last_frame_with_buttons: Option<u64>,
    /// The pane whose link owns the current primary-button gesture, until its release.
    ///
    /// Keeping this beyond the press frame prevents egui from turning a Ctrl+click with a small
    /// movement into selection, pane movement or a program mouse report on the following frame.
    link_press_in_progress: Option<pointer::links::LinkGesture>,
    /// The last cell the program was told about while dragging, and of which console.
    ///
    /// **It serves not to repeat the same report.** A slow drag spends dozens of frames over the
    /// same cell, and telling it in all of them says nothing new to the program: sixty messages a
    /// second reach it saying what it already knew.
    last_cell_reported: Option<(usize, u16, u16)>,
    /// Whether the last press **of the primary button** fell on a console whose program had asked
    /// for the mouse.
    ///
    /// **It is saved with the chord and for the same reason**: the press and the drag do not fall
    /// in the same frame, so when egui gives the gesture as begun there is no longer any way to
    /// know which console the button went down on. Without this, dragging inside `htop` would start
    /// marking text as soon as the pointer moved, on top of what the program is painting.
    ///
    /// **It is only about the primary one because only the primary one drags.** What to do with the
    /// release of the others is said by [`Self::presses_for_the_program`], which keeps the count
    /// button by button.
    the_press_went_to_the_program: bool,
    /// Which buttons went down on which console bound for the program, and are still unreleased.
    ///
    /// **A gesture is one and its two ends go to the same place, or the program is left waiting.**
    /// Without this list, the release was resolved on its own and three ways of lying to the
    /// program inside came out: a release that falls **on another console** was taken by that one
    /// —and the one that received the press was left believing the button held down for ever, with
    /// `htop` stretching its column and `vim` extending its selection—; a release **outside every
    /// grid** was lost; and a button that is not the primary one received its press and never its
    /// release, because the only latch there was was the primary one's.
    ///
    /// **It is a list and not a single slot** because two buttons can be down at once: dragging
    /// with the left one and pressing the right one is a gesture that exists.
    presses_for_the_program: Vec<(usize, egui::PointerButton)>,
    /// What the drag in progress means, and **nothing while there is none**. See [`DragIntent`].
    ///
    /// **State of our own, and here it is needed.** What egui holds between frames is *which*
    /// control is being dragged, not *what it meant* to whoever grabbed it: that is only in the
    /// chord of the press event, which goes through the queue once and does not come back.
    drag_in_progress: Option<DragIntent>,
    /// What the last frame was painted with: each pane's rectangle and **where its panel's border
    /// ended up**, or nothing if that pane had no panel in sight.
    ///
    /// **It exists for the injected gesture and nothing else**, and that is why it keeps what has
    /// already been calculated instead of tiling again: the raw-input hook runs **before** the UI
    /// pass, so at that moment this frame's tiling does not yet exist. Tiling on its own account
    /// would be a second copy of the calculation that decides where each console is, and the two
    /// could disagree precisely in the batch that measures.
    ///
    /// **A list of pairs and not two lists**, for the same reason a pane's name lives inside it:
    /// two vectors that have to be kept aligned are an invariant somebody will break.
    last_painted: Vec<(egui::Rect, Option<f32>)>,
    /// The name the next tab to be opened will carry. See [`Tab::name`].
    ///
    /// It lives in the window and is not deduced from the list **because the list falls short as
    /// soon as something is closed**: with two open and the first one closed, `tabs.len()` is worth
    /// 1 and it would hand out again a name that has already been used.
    next_name: usize,
    /// The name the next pane to be opened will carry. See [`Pane::name`].
    ///
    /// **A single one for the whole window, not one per tab.** The report publishes the panes of
    /// every tab one after another, so two independent counters would give two «pane 0» in the same
    /// text — the name would again identify nobody, which is precisely what it comes to fix.
    next_pane_name: usize,
    /// Whether any tab has been closed **by hand**, and therefore the window no longer closes by
    /// itself.
    ///
    /// **It exists because of a case that only turns up when closing is possible**: a tab whose
    /// programs have already finished —and whose last content is being read, which is what the
    /// closing rule exists to preserve— alongside another with a live agent. On closing the agent's
    /// one by hand nothing alive is left, and the age-old rule would close the window **taking away
    /// what was being read**. Nobody asked for that: what was asked for was to close a tab.
    ///
    /// It is not lowered again. Whoever has closed a tab by hand has said that they govern the
    /// tabs, and they still have the last one with its chord and the window's button.
    some_closed_by_hand: bool,
    /// How big the window was in points the last time it was known, so that the report publishes
    /// it.
    ///
    /// **Without this, a measurement annex cannot be reproduced.** The grid in cells the report
    /// already published comes from the window's size **and** from the font's metrics, so reading
    /// it does not say which window it was taken with; and as the size was decided by the desktop,
    /// the annexes prior to this were taken with windows whose size nobody noted down. This figure
    /// is the one that is asked for again with [`SIZE_VARIABLE`].
    ///
    /// **It is saved instead of being asked for on publishing** because the report is written in
    /// `on_exit`, which does not receive egui's context: when it has to be said, there is no longer
    /// anybody to ask.
    window_size: egui::Vec2,
    /// The size that was asked for on starting, while it still has to be insisted on. See
    /// [`RequestedSize`].
    requested_size: Option<RequestedSize>,
    /// The consoles the control channel opened, by the opaque handle each connection was given.
    ///
    /// **It belongs to the window and not to a pane**, because opening one splits a tree and only
    /// the window has one; and **the handle is scoped by the connection that asked**
    /// ([`control::OpenedByAgents`]), so a second agent holding every identifier in its environment
    /// still cannot name a pane it did not open (**AC14**).
    opened_by_agents: control::OpenedByAgents,
}

/// A window size asked for from outside, and how many times there is still to insist.
///
/// **The two go together because neither exists without the other**: without the size there is
/// nothing to insist on, and without the count the insisting cannot end.
///
/// **Insisting is necessary, and it is measured.** Asking for the size while the window is being
/// built works for two frames: on the third the compositor sends its own configuration —on GNOME
/// over Wayland, 1126×661 points where 800×600 had been asked for— and the window goes to that
/// size. Asking again **afterwards** is respected, and then it stays.
///
/// **And the insisting ends**, with a small cap. A compositor that will not give way —a tiled
/// window, a desktop with a policy of its own— would turn this into a fight every frame: each
/// attempt asks for another frame, and the measurement this gate exists to make possible would be
/// measuring itself fighting. With the cap, whoever measures finds out anyway, because the report
/// publishes the size the window **had**.
struct RequestedSize {
    how_much: egui::Vec2,
    /// In how many more frames it will be asked for again. Why that number, in
    /// [`FRAMES_INSISTING_ON_SIZE`].
    attempts: u8,
}

impl Window {
    /// The tab that is seen and the one the keyboard goes to.
    ///
    /// They save writing the two fields in every place; **they add no guarantee at all** — they
    /// index the same and panic the same as doing it by hand.
    ///
    /// The mutable one is no good where another field of the window has to be borrowed as well —the
    /// theme, when painting—, because it borrows the whole window; there the list is indexed
    /// directly. The read-only one does not have that problem.
    fn active(&self) -> &Tab {
        &self.tabs[self.active]
    }

    /// The console of the active tab that is called that, if it is still there.
    pub(crate) fn pane_called(&mut self, name: usize) -> Option<&mut Pane> {
        self.active_mut().pane_called(name)
    }

    fn active_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// Every open console, seen or not.
    ///
    /// **The distinction between this and [`Window::active`] is the whole milestone's**, and it is
    /// not a matter of style: what a program *does* runs in every tab, and what is *seen* happens
    /// in only one. A walk put on the wrong side gives no error and shows nothing on screen — with
    /// a single tab the two do exactly the same—, so the only defence is that every place says
    /// which of the two things it is.
    fn all_panes(&self) -> impl Iterator<Item = &Pane> {
        self.tabs.iter().flat_map(|tab| tab.panes().into_iter())
    }

    fn all_panes_mut(&mut self) -> impl Iterator<Item = &mut Pane> {
        self.tabs.iter_mut().flat_map(Tab::panes_mut)
    }

    /// Arms the watcher over the directories of the consoles that have one.
    ///
    /// A console with no directory is not watched because there is nothing to watch: nobody can
    /// publish a panel for it. Its flag stays as it is and its read finds nowhere to look.
    fn watch<'a>(ctx: &egui::Context, panes: impl IntoIterator<Item = &'a Pane>) -> Watcher {
        opening::watch_panes(ctx, panes)
    }

    /// Asks for the window size again while the window does not have it and attempts remain.
    ///
    /// **Asking for it once, while the window is being built, is not enough**, and why is in
    /// [`RequestedSize`]. Here is the rest of the rule: while the size is the one asked for **no
    /// attempt is spent** —the first two frames already have it, and spending them there would
    /// leave the window with nothing to answer with when the compositor sends its own—, and when
    /// they run out it is said out loud and it stops looking.
    ///
    fn insist_on_the_size(&mut self, ctx: &egui::Context) {
        opening::insist_on_size(self, ctx);
    }

    /// Pumps **every** console of **every** tab and resizes them, and sums up what the frame has to
    /// decide with that.
    ///
    /// **The ones in the tabs that are not seen are pumped just the same, with the same budget**,
    /// and this is the milestone's promise, not a performance choice. Cutting it down would mean no
    /// longer emptying their pseudoterminal at the rate it produces, and the margin the kernel
    /// gives away before blocking the program is 20 KiB —six times less than the budget itself—, so
    /// the background tab's program **would stop when writing**. «It keeps running even if you are
    /// not looking at it» and «it consumes less because you are not looking at it» are incompatible
    /// by construction.
    ///
    /// **What does cost something, and is better not hidden:** the sharing out is done by the frame
    /// itself. Each console swallows its own and the frame gets longer, so with several dumping at
    /// once **all** of them go slower, the visible one included. «They keep running» does not mean
    /// «at the same speed as if you were looking at them».
    ///
    /// It lives apart from the framework's loop so that the two decisions that come out of here
    /// —asking for another frame and closing the window— can be checked without opening one. It is
    /// the only way to prove that **a dead console does not close the window**, which is what this
    /// milestone promises and what a failure here would take with it without any test noticing.
    fn pump_them_all(&mut self) -> Pump {
        pty_output::pump_all(self)
    }

    /// Opens a tab with its two consoles and leaves it in front.
    ///
    /// **If any console cannot be opened, the tab is not created**: it is said out loud on the
    /// error output and the window carries on exactly as it was. It is the same criterion as the
    /// panel directory —either both or neither—, and for the same reason: half a tab is discovered
    /// on reaching it, which is too late.
    fn create_a_tab(&mut self, ctx: &egui::Context) {
        opening::create_a_tab(self, ctx);
    }

    /// Puts the tab that has just been opened in front, or leaves the window as it was if it could
    /// not be opened.
    ///
    /// **It receives the attempt already made instead of making it, and it is for the same reason
    /// as [`panel_or_none`]:** opening a console only fails when the system gives no more
    /// pseudoterminals or descriptors, and that cannot be provoked from a test without touching the
    /// whole process. Splitting it here is what makes it possible to assert what the case promises
    /// — that a failure leaves no half tab, does not move the one that was being seen and rearms
    /// nothing.
    fn bring_to_front(&mut self, opened: term_core::Result<Tab>, ctx: &egui::Context) {
        let tab = match opened {
            Ok(tab) => tab,
            Err(failure) => {
                eprintln!("the tab could not be opened ({failure})");
                return;
            }
        };

        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.rearm_the_watcher(ctx);
    }
}

impl eframe::App for Window {
    /// Injects the drag gesture of a measurement batch, if the batch asked for any.
    ///
    /// **Here and not by calling the gesture's effect by hand**: eframe calls this right before
    /// egui processes the batch of input events (`eframe-0.36/src/native/epi_integration.rs:362`,
    /// inside `prepare_raw_input`), so the events dropped here travel **the same path** as those of
    /// the real mouse — each console's `interact`, the drag egui holds between frames, the target's
    /// veil and the tiling that is redone on release. Calling the tree's operation directly would
    /// measure a function, and the annex compares frames.
    ///
    /// **With no drags asked for it does absolutely nothing**, it does not even look at the clock:
    /// it is the path of whoever uses the application.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        settings::editor::inject(self, raw_input);
        board::probe::inject(self, raw_input, ctx);
        sidebar::probe::inject(self, raw_input, ctx);
        let Some(injected_drags) = self.injected_drags.as_mut() else {
            return;
        };

        let started = Instant::now();
        // **The frame and not the pass.** eframe calls this hook before egui's passes, so a
        // repeated pass never reaches it, but also on every logic-only tick while the window is
        // hidden, and those do not advance the frame number (eframe 0.36.1,
        // `native/epi_integration.rs:323-362`); keying the gesture on it holds it to one step.
        let events = injected_drags.events(&self.last_painted, ctx.cumulative_frame_nr());
        raw_input.events.extend(events);

        // **And frames are NOT asked for, even with gestures left. The first version tried that and
        // it falsified the whole batch.** Asking for a repaint on every frame sets the window
        // spinning flat out, and then the batch with drags stops being «the comparison batch plus
        // the gesture» and becomes another regime: measured on 18 August 2026, two runs gave
        // **2,219 and 7,008 frames** against the **413** of batch C it is compared with, and the
        // mean cost of the rest of the frame **went down** —163 µs and 103 µs against 280 µs—
        // simply because it was diluted across thousands of frames with no console work inside. Nor
        // did two runs of the same batch resemble each other.
        //
        // The gesture advances in the frames the batch already produces on its own: during a dump
        // there are plenty. If the window went to sleep in the middle of a gesture, all that
        // happens is that that gesture takes longer — the drag is held by egui, not by this
        // function.
        self.spent_injecting += started.elapsed();
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // **The way-out stopwatch starts here and not in the interface pass**, because `logic` runs
        // earlier within the same frame and what it does —swallowing and putting into the grid—
        // comes to cost milliseconds during a dump. The key was already waiting from before: if the
        // stopwatch started after that work, the number would come out short exactly at the moment
        // when it shows.
        self.frame_started = Instant::now();

        // What the previous frame cost, whole. It arrives one frame late because that is how long
        // it takes to be known: by the time it is tessellated and drawn, the pass is already over.
        // It is closed **once per frame and not per pass**: egui can repeat the pass, running this
        // whole closure again, and then the second close would pair this frame's spans with the
        // previous one's total — an invented sample, and the real total lost.
        //
        // **And the counter has to be the frame one, not the pass one.** This was written with
        // `cumulative_pass_nr` and that way it filtered nothing, because that one **goes up on
        // every pass**: on a repeated frame it gave two different values and the second close got
        // in anyway. Read in its source (`egui-0.36/src/context.rs:1748-1791`):
        // `cumulative_frame_nr` is incremented **once at the end of each `run_ui`**,
        // `cumulative_pass_nr` **once per completed pass**, and the documentation says the first is
        // always less than or equal to the second. Both are incremented on finishing, so within the
        // two passes of one same frame the frame counter is worth the same — and that is what makes
        // it a guard. It is tied down by `egui_counts_each_frame_once_and_every_pass_separately`.
        //
        // **These four lines are covered by no window test, and it is worth knowing**:
        // `eframe::Frame::_new_kittest()` hands over `cpu_usage: None` —checked—, so along this
        // path `close_frame` never gets called from a test. What is tested is egui's contract that
        // the guard depends on, and the close itself comes in through [`Window::close_the_frame`],
        // which the tests call directly. It is the same border declared by [`Window::on_exit`] and
        // by [`run`]'s environment reads.
        let frame_nr = ctx.cumulative_frame_nr();
        if frame_nr != self.last_closed_frame
            && let Some(seconds) = frame.info().cpu_usage
        {
            self.last_closed_frame = frame_nr;
            self.close_the_frame(std::time::Duration::from_secs_f32(seconds));
        }

        // **After closing, not before**: see [`Window::save_if_due`].
        self.save_if_due();

        // **With nobody to warn, it is looked at again on every frame**, which is how it was done
        // before there was a watcher. It is not caution: when tracking is lost —the user deletes
        // from their own console the directory that was being watched, or the watcher hands over a
        // failure— nobody raises any flag, and a panel frozen in silence is worse than one that was
        // reread too often.
        //
        // **And the frame has to be asked for, or the fallback never gets to run**: with the window
        // at rest and the console quiet —an agent waiting for an answer, which is exactly when its
        // panel says the most valuable thing— there is no round anywhere to pick up the flag. Half
        // a second of latency, and only in the degraded mode.
        //
        // **All of them, including the ones not on screen**, unlike the alarm: raising a flag is an
        // atomic write and what it buys is that on coming back to a tab its panel is up to date.
        // Why the alarm is scoped, in [`Window::set_the_alarm`].
        if self.watcher.is_blind() {
            for pane in self.all_panes() {
                pane.to_reread.raise();
            }
            ctx.request_repaint_after(POLL_WITHOUT_A_WATCHER);
        }

        // **How big the window is gets noted on every frame**, because the report is published in
        // `on_exit` and there is no longer a context there to ask it. See [`Window::window_size`].
        // It is one read of a rectangle and one write of two numbers: it is not timed because the
        // pair of `Instant::now()` would cost more than it does, just like the read of the area the
        // interface pass already does.
        // **`viewport_rect` and not `content_rect`**, which is the one egui clips in case the
        // system covers an edge: what is asked for with `inner_size` is the whole area, so
        // publishing the clip would give a number that cannot be asked for again. On a desktop the
        // two are equal; on a phone with a notch, not.
        self.window_size = ctx.viewport_rect().size();
        // The heartbeat goes here **and also at the end of the interface pass**, and both times are
        // needed: see [`Window::set_the_alarm`].
        self.set_the_alarm(ctx);

        let Pump {
            some_left_a_queue,
            all_finished,
        } = self.pump_them_all();

        project_view::refresh(self, ctx);
        restoration::refresh(self);

        self.insist_on_the_size(ctx);

        if some_left_a_queue {
            // The budget cut off the pumping. Without asking for another frame, what is left would
            // be waiting for the program to write again —or for the user to move the mouse— and a
            // dump would be seen in pieces, stopping. **It is enough that one asks for it**: the
            // frame belongs to the window, so the repaint one asks for serves the other too.
            ctx.request_repaint();
        }

        // **The window closes only when all of them have died.** With one console working and the
        // other finished, closing would take down the work of the one still alive — and the one
        // that died is left with its last content on screen, which is what one wants to read right
        // after a program ends.
        // **Unless some tab has been closed by hand**: see [`Window::some_closed_by_hand`].
        if all_finished && !self.some_closed_by_hand {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        chrome::begin_pass(self, ui, frame);
        clipboard::deliver(self, ui.ctx());
        let metrics = CellMetrics::measure(ui.ctx(), egui::FontId::monospace(self.font_size));

        // The board claims its input before the console dispatcher. Its named work is discounted
        // from frame accounting; outbound latency still includes this earlier application work.
        let chrome_owned = chrome::before_keys(self, ui, &metrics);
        self.timed_separately += self.chrome.take_cost();
        let sidebar_owned = sidebar::before_keys(self, ui, &metrics, chrome_owned);
        let mut board_owned = sidebar_owned || board::before_keys(self, ui, &metrics, chrome_owned);
        let pane_owned = !board_owned && pane_view::before_keys(self, ui, &metrics);
        if pane_owned {
            if !self.pane_view.is_renaming() {
                ui.style_mut().visuals.disabled_alpha = 1.0;
            }
            ui.disable();
        }
        board_owned |= board::blocked(self, ui.ctx());
        let (viewer_owned, explorer_pointer, requests) =
            sidebar::tool_keys(self, ui, &metrics, pane_owned || board_owned, chrome_owned);
        let area = ui.max_rect();

        // **The order is applied before tiling**, so that what gets painted in this same frame is
        // already the new tab. Applying it at the end would leave a frame showing the previous one,
        // which with the window at rest is the last one painted.
        for request in requests {
            self.apply(request, ui.ctx(), area, &metrics);
        }

        // **What the open connections asked of the tree, answered here** because this is where a
        // split's geometry exists. It goes after the keys —so a chord and a request in the same
        // frame cannot fight over the tree— and before the tab bar, whose height depends on how many
        // tabs there are and which a close can change.
        self.answer_the_windows_requests(ui.ctx(), area, &metrics);

        pane_view::reconcile(self);
        let overlay_owns_pointer =
            viewer_owned || explorer_pointer || board_owned || pane_owned || chrome_owned;
        let header_owns_pointer =
            overlay_owns_pointer || header::menu::pointer_blocked(self, ui.ctx());

        // **Everything that changes tab goes before tiling**, and this is what a defect cost: the
        // tiling comes out of the active tab's fraction, so attending to the click afterwards left
        // a frame painting **the new consoles with the old divider**. And it did not just look bad:
        // out of those rectangles comes `measured`, so the two pseudoterminals of the just-opened
        // tab were given a wrong grid and corrected it on the following frame — a double
        // readjustment, visible in any full-screen program.
        let (tab_bar, area) = tab_bar::partition(
            self,
            ui,
            &metrics,
            header_owns_pointer,
            // Only what is modal dims the consoles: the drawer and a title-bar press are not, and
            // dimming on them made the window blink on every button and shade beside the drawer.
            pane_owned || chrome::is_modal(self, ui.ctx()),
        );

        // **Walking the tree is timed, and it is not zeal: it is the instrument's rule.** The
        // frame's cost is split by subtraction —what no console declares is published as the
        // interface framework's cost, which is the figure D3 judges it with—, and this walk is no
        // pane's work: it is the layout's. With two consoles it was indexing an array and there was
        // nothing to declare; with the tree it is two walks and their allocations.
        //
        // It goes to `timed_separately`, which already exists for exactly this —the window's work
        // that is no console's— and which is already subtracted. A new span in the report for a few
        // microseconds would have to be explained in full and would say very little.
        // **The missing fonts are searched for here, BEFORE painting and not during.** Registering
        // a face remakes the text engine's whole glyph atlas —milliseconds, not microseconds— and
        // doing it in the middle of a pass would leave that same pass's measurements half done. The
        // price, declared: what the painting notes down is resolved on the **following** frame, so
        // a character that has just arrived comes out as a box for **one frame** —16 ms— before it
        // is seen properly.
        //
        // It goes to `timed_separately` **and** to its own span of the report: without the
        // subtraction, these milliseconds would be published as the framework's cost, which is the
        // figure **D3** judges it with.
        settings::editor::runtime::resolve_fonts(self, ui.ctx());

        let travel = Instant::now();
        let tiling_of = |window: &Self| pane_view::view_tiling(window, area, &metrics);
        let mut tiling = tiling_of(self);
        self.timed_separately += travel.elapsed();

        // The drag is attended to **before painting and the tiling is redone in the same pass**:
        // painting with the earlier tiling would leave the divider one frame behind the mouse,
        // which is exactly what shows in a drag.
        //
        // **All the dividers, each with its own identity.** With only one it made no difference to
        // attend to just the first; with several, the ones left out would be lines that are seen
        // and cannot be grabbed.
        //
        // It is walked over a copy of the list —the dividers are `Copy`— because attending to one
        // can change the tiling, and **it is enough that one moves**: two dividers cannot be
        // dragged in the same frame, so as soon as one responds the tiling is redone and looking
        // stops.
        //
        // **A known measurement gap, and declared here instead of covered up:** of this loop what
        // is timed is *redoing the tiling*, but not the calls to `interact` that attend to each
        // divider, so those are published inside «the rest of the frame» — that is, as the
        // framework's cost. It predates the panel having a divider of its own and does not grow
        // with it: the panel's is attended to in `paint_pane`, **inside** the span that is already
        // declared. It is left declared and unfixed for the same reason as the window-size blind
        // spot: a gap that is stated does not lie, and half-covering it does. What it would cost to
        // close it is splitting this function the way `handle_the_mouse` is split, so that the
        // timed region is the whole call and shrinking it stops being moving one line.
        let dividers = tiling.dividers.clone();
        for (which, divider) in dividers.into_iter().enumerate() {
            if self.active_mut().drag_the_divider(ui, which, divider) {
                let redoing = Instant::now();
                tiling = tiling_of(self);
                self.timed_separately += redoing.elapsed();
                // **And this is where the divider's gesture declares that it was good for
                // something.** It is the same number the panel's counts with `width_changed` and
                // the console's with the reorderings the tree accepts: sending the events does not
                // mean anything moves —if the handle is not grabbed, `drag_the_divider` returns
                // nothing on any frame—, and without separating them a run where none of them took
                // would publish «the gesture costs nothing».
                if let Some(injected_drags) = self.injected_drags.as_mut() {
                    injected_drags.one_more_with_effect();
                }
                break;
            }
        }

        // The dividers are painted before the consoles: if the glyph at the edge of one stuck out,
        // the clip already cuts it, and that way the line never ends up on top of a letter.
        //
        // **And they are painted before taking the parts** so as to be able to take them away
        // without copying them: the tiling is redone on every frame, so a clone here is a memory
        // allocation per frame that on top of that nobody would declare in the measurement.
        // **The mouse is attended to before painting anything, and the tiling is redone if it
        // dropped.** Dropping a console onto another **reorders the tree**, so the rectangles above
        // go on to describe the previous layout: painting with them would leave a frame with the
        // dividers and the consoles in the old place, and —worse— `paint_pane` would note down for
        // each console the old grid, which is the one the pumping announces to its program on the
        // following frame. A double readjustment, visible in any full-screen program, which is the
        // same defect this file already documents for dragging dividers.
        //
        // Redoing is timed like the dividers' one: it is the layout's work and what is not declared
        // is published as the interface framework's cost.
        //
        // **Attending to the mouse is declared on its own**: the stopwatch goes inside, and its
        // comment says why.
        let of_the_mouse = self.handle_the_mouse(ui, &tiling.parts, &metrics);
        if of_the_mouse.the_tree_changed {
            pane_view::reconcile(self);
            let redoing = Instant::now();
            tiling = tiling_of(self);
            self.timed_separately += redoing.elapsed();
        }

        for divider in &tiling.dividers {
            ui.painter()
                .rect_filled(divider.rect, 0.0, pane_view::separator_color(self));
        }

        let rects = tiling.parts;
        if self.chrome.enabled() {
            let visible = rects.iter().filter(|rect| rect.is_positive()).count();
            self.chrome.record_view(visible, rects.len() - visible);
        }
        // **The target that is painted indexes the earlier tiling**, and that is fine: there is
        // only a target while the drag is still in flight, and on that frame nobody has dropped, so
        // the tree did not change.
        //
        // **Closing a console by its button is the other way that pass can change the tree, and it
        // cannot coexist with a target.** A target needs a pane being dragged by the primary button,
        // which means that button is already down; the close needs a fresh **press** of the same
        // button, which cannot arrive without a release in between. Said here because the index
        // below is direct: if some later gesture gives the pass a second way to shorten this list,
        // this is the line that stops being true.
        let drag_target = of_the_mouse.target;

        // It is indexed by hand instead of going through `active_mut`, which would lend the whole
        // window: the theme is needed at the same time and it is another field.
        // **The list is prepared only when there is an injected gesture that is going to read it.**
        // With no batch dragging nothing is allocated: it is the path of whoever uses the
        // application, and an allocation per frame that nobody declares would end up published as
        // the framework's cost.
        // **One single question and not two.** It was resolved twice —once to size the list and
        // once per pane— and they were two places where the condition could drift apart: sized for
        // a batch whose entries never get in, or the other way round.
        let there_is_a_gesture = self.injected_drags.is_some();
        let measuring_links = self.measuring_links();
        let mut painted = if there_is_a_gesture {
            Vec::with_capacity(rects.len())
        } else {
            Vec::new()
        };
        let mut with_effect = 0usize;
        // **Two questions and not one, because a console only writes if the window is being written
        // into.** egui's flag is the desktop's answer —`false` the moment somebody alt-tabs away—
        // and the name is the window's own. Both are read once, before the walk borrows the panes.
        // **The name and not the position**: the position is only right while the walk order holds,
        // and the focus was made a name precisely so that reordering the tree cannot move it. See
        // **D36**.
        let the_window_has_the_keyboard = ui.ctx().input(|input| input.focused);
        let with_the_focus = self.tabs[self.active].focus;
        // **Read once, before the walk**, like the two questions above it: a clock read per pane
        // would put two consoles of the same frame in different halves of the same blink.
        let blink = self.blink.now();
        self.blink.new_frame();
        // Read once before the walk, like the two questions above it: it is the session's decision
        // and not a pane's, and every pane of the frame has to be tiled by the same one.
        let headers = self.headers;
        let integrated = self.chrome.enabled();
        let maximized = self.pane_view.zoomed(self.active().name);
        for (pane, rect) in self.tabs[self.active]
            .panes_mut()
            .into_iter()
            .zip(rects.iter().copied())
        {
            if !rect.is_positive() {
                pane.painted_console = None;
                continue;
            }
            let keys = painter::Keys::of(the_window_has_the_keyboard, pane.name, with_the_focus);
            let pane_outcome = painter::paint_pane(
                ui,
                pane,
                rect,
                &self.theme,
                &metrics,
                &mut self.discoverer,
                // **How many consoles this tab has decides whether the strip carries its button**,
                // and the list being walked is the answer: one rectangle per console of the active
                // tab. See [`header::Button::in_a_tab_of`].
                painter::HowToPaint::of(blink.on(keys), headers, &rects)
                    .ligatures(self.ligatures)
                    .integrated(integrated)
                    .maximized(maximized)
                    .measuring_links(measuring_links),
            );
            // **What the painting really drew, not what the program asked for**: it is what the
            // repaint deadline is armed from. See [`alarms::BlinkState::painted`].
            self.blink.saw(pane_outcome.blinking);
            // **It is noted per pane and not once per frame**, because it belongs to the pane it
            // was subtracted from: two consoles that rasterize in the same frame each pay their
            // own.
            if pane_outcome.color.rasterized + pane_outcome.color.failed > 0 {
                self.timed_separately += pane_outcome.color.spent;
                self.color.record(
                    pane_outcome.color.spent,
                    pane_outcome.color.rasterized,
                    pane_outcome.color.failed,
                );
            }
            // **Noting and subtracting are the same decision, so they are taken together** — the
            // rule the tab bar's own accumulation states: without subtracting, the strip is
            // published as the framework's cost (**D13**); without noting, the report cannot name
            // it and what it costs would have to be got by subtracting two batches.
            //
            // **Summed over the panes here and recorded once when the frame closes**, because the
            // strip is painted per pane and per pass while the count the report publishes says
            // frames: see [`measure::Strips`].
            self.strips
                .a_strip_was_painted(&mut self.timed_separately, pane_outcome.strip_cost);
            (pane_outcome.search).hand_over(&mut self.searching, &mut self.timed_separately);
            // **Summed over every pane and every frame, not per frame.** What the report answers is
            // «did this run draw any button at all», which is what makes a batch's zero mean the
            // seeding failed rather than that one frame happened to be short.
            self.buttons.painted += pane_outcome.buttons_painted;
            // **Whichever pane drew last, and that is said rather than promised.** An earlier
            // version of this claimed «the first pane's, so the second stays a control» — and the
            // loop overwrites, so the last one wins and batch R presses the **right** panel, because
            // its seeding gives every pane buttons. Which one is pressed does not change what is
            // measured: the walk and the composing are the same work wherever they happen, and the
            // panel's own gesture batch is what keeps a control inside a run.
            if let (Some(injected_drags), Some(first)) =
                (self.injected_drags.as_mut(), pane_outcome.first_button)
            {
                injected_drags.a_button_at = Some(first.center());
            }
            pane_outcome
                .links
                .hand_over(&mut self.links, &mut self.injected_drags);
            if there_is_a_gesture {
                painted.push((rect, pane_outcome.panel_divider));
                if pane_outcome.width_changed {
                    with_effect += 1;
                }
            }
        }

        // The highlight goes at the end so that it stays on top of the content: a border half
        // covered by a glyph of the edge says nothing. `StrokeKind::Inside` keeps it inside the
        // rectangle, so it does not invade the neighboring console or the divider.
        // **The focus is a name, so where it fell has to be looked up**: indexing the list of
        // rectangles with it would point at another console —or fall off the list— as soon as the
        // names stop matching the positions, which is the whole reason the focus is a name.
        pane_view::paint_borders(self, ui, &rects);

        // And the target's veil goes after the highlight, for the same reason: it is the last thing
        // decided and it has to be seen over everything else. It is timed separately because it
        // belongs to the layout, not to the console it falls on.
        // It is indexed directly, like the focus highlight just above and for the same reason: the
        // number comes out of walking this same list in this same frame.
        if let Some((target, side)) = drag_target {
            self.paint_the_drag(ui, rects[target], side);
        }

        // **The rectangles that have just been painted with, for the gesture a batch injects into
        // itself.** They are saved here and not before the painting because they are the definitive
        // ones: if somebody dropped in this same frame, the tiling was redone and the ones above no
        // longer hold.
        //
        // **They are MOVED, not copied, and the review asked for that.** The first version copied
        // the list with `extend_from_slice` and did not time the copy for being small — which is
        // exactly the leak the project's hard rule describes: what is not declared is published as
        // the framework's cost, the figure D3 judges egui with, and on top of that in the only
        // batch that travels this path. By moving the vector there is nothing to declare, because
        // no work is left to do.
        if let Some(injected_drags) = self.injected_drags.as_mut() {
            // **Here and not at startup**: it is the first place where the font's metric exists.
            // See [`InjectedDrags::cell`].
            injected_drags.already_knows_the_cell(metrics.width());
            self.last_painted = painted;
            // The sum of this frame's panes, already counted: one addition, not one per pane.
            injected_drags.with_effect += with_effect;
        }

        // The ones not painted have nowhere else to find out their size: this is the only half of
        // the frame that knows the area they would be given.
        self.size_hidden(ui.ctx(), area, &metrics);

        // **The bar is painted here, last, and that is an invariant of the milestone and not a
        // preference.** Its card says what each tab's panes claim, and this frame's panels are not
        // reread until this line: those of the tab on screen, when painting them just above; those
        // of the rest, in `size_hidden`. Painting it earlier, the card would go
        // **one frame behind** — and in a reactive window that is not «one frame», it is forever:
        // the frame the watcher asked for would be spent painting the old card and nobody would ask
        // for another.
        //
        // **Moving it down here does not change what is seen**: the bar and the consoles occupy
        // disjoint rectangles —`set_the_tab_bar_aside` cuts the window in two— and each paints with
        // its own clip, so which one goes on top decides nothing. Its click is still attended to
        // much earlier, with the rest of what can change tab.
        //
        // **And it is declared whole —accumulating and painting—, not just the walk.** Declaring
        // half would leave the new symbol in the framework's residue, which is the figure D3 judges
        // egui with.
        if let Some(tab_bar) = tab_bar {
            // **Noting and subtracting are the same decision, so they are taken on the same line.**
            // With two separate statements one can be done and the other forgotten, and both ways
            // of forgetting it are expensive: without subtracting, the bar is published as the
            // framework's cost; without noting, the report does not name it and two batches have to
            // be subtracted to know what it is worth.
            //
            // **It is accumulated per pass and noted on closing the frame**, like the layout's
            // mirror: egui repeats the pass and paints the bar again, so counting here would publish a
            // mean **per pass** under a label that says «per frame». The repeated work is indeed
            // added up —it happened twice— but the frame is still one.
            let tab_bar_outcome = self.paint_the_tab_bar(ui, tab_bar, &metrics, SystemTime::now());
            self.tab_bar_this_frame += tab_bar_outcome.cost;
            self.timed_separately += tab_bar_outcome.cost;
            self.tab_bar_cost.saw_marked(tab_bar_outcome.marked);
        }

        // It has already been painted, so whatever is still pending belongs to a console that was
        // not painted.
        chrome::after_panels(self, ui, area);
        header::menu::paint(self, ui.ctx(), &metrics);
        self.close_the_unpainted_rounds();

        // **The mirror is compared here, at the end of the interface pass, and not in the logic
        // half.** The mutations that change the layout happen **in this half** —splitting, closing,
        // moving a console, dragging a divider, dragging or folding a panel—, and seeing them one
        // frame late is not seeing them: in a reactive window the following frame may never arrive.
        //
        // **Changing tab is not on that list, and it is worth saying**: the mirror saves neither
        // the focus nor which tab is on screen, so that gesture arms no deadline. An earlier
        // comment included it and was false.
        //
        // **And it is idempotent under the repeated pass**, which is what allows there to be no
        // flags in those nine places: egui repeats `logic` **and** `ui` in the same round, and
        // comparing the same state twice gives the same result. A mechanism that demanded
        // remembering to warn along nine paths — we already know how that ends up here.
        self.check_whether_the_layout_moved();

        // **And now yes, with the panels already reread.** See [`Window::set_the_alarm`]: the logic
        // half asks for it with the state from before reading, so the frame in which a panel
        // appears for the first time would go to sleep without having scheduled anything.
        self.set_the_alarm(ui.ctx());
    }

    /// The numbers are published on shutdown, and **here and not on seeing the program die**, for
    /// two reasons that were discovered by looking at the output: asking to close does not close on
    /// the spot, so that path printed the report once for every frame that still arrived; and
    /// closing the window by hand —which is how one leaves half the time— does not go through
    /// there, so the session was lost whole without publishing anything.
    /// **These two lines are covered by no test, and it is worth knowing**: capturing the process's
    /// own standard output cannot be done from a test without machinery that does not pay off, so
    /// the tests come in through [`Window::publish_the_report`]. Emptying this would leave the
    /// whole suite green with the report mute — the same gap that already cost this project once.
    /// What covers it today is not a test, it is the use: whoever measures finds the batch's file
    /// empty as soon as they open it. That is why there can be **nothing** here other than the
    /// call.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // **What is pending is written before leaving.** Closing the window by hand is not dying,
        // and it is the only ending in which the half second the delay accepts losing **can** be
        // not lost. Here there is no frame left to measure, so there is no cost to declare.
        restoration::capture_on_exit(self);
        self.check_whether_the_layout_moved();
        self.save_now();

        self.publish_the_report(&mut std::io::stdout().lock())
            .expect("writing the report to standard output");
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // The background is painted here and not with a panel inside `ui` so that it also covers
        // the first frame and the gaps of a resize, where no panel has run yet: without this
        // eframe's default gray shows through. And it is the same background the painter takes for
        // granted: the cells that carry it are not drawn.
        self.theme.background().to_normalized_gamma_f32()
    }
}

// ── The layout, out and back ─────────────────────────────────────────────────────────────────────
//
// **The two conversions live here and not in `session`, and the border is on purpose.** The fields
// of a pane are private to this module, and materializing is **forking a process per leaf**:
// keeping it in a single place is what makes the other module testable in full without opening a
// pseudoterminal.
//
// **And the program is received, not read from the environment here.** It is the rule the startup
// already follows five times —the pure function receives the value, the one-line wrapper reads the
// environment—, and here it has two more reasons: reading `SHELL` once per leaf would read it a
// hundred and twenty-eight times in the worst case, and a test that depended on it **would not test
// this code, it would test the configuration of whoever runs it**. That last part is already
// written in `Pane::launching`, which exists exactly for this.
//
// **And this block no longer sits behind the test flag, which is what the startup has just
// changed.** It carried it from the day it was written, with the written reason that nobody called
// it in production and that this crate's idiom for that is the flag and not an `allow`; the one
// that calls it now is `Window::opening`, so the flag goes and the compiler covers this path again
// in the release build.
//
// **And both halves are now used in production**: materializing is called by the startup, and
// describing is called by the trigger that saves. No conversion is left without a consumer, which
// is what was needed for this not to be a public surface with no reason.

impl Window {
    /// How this window's layout is described, so that it can be found again.
    pub(crate) fn describe(&self) -> session::StoredDocument {
        session::StoredDocument {
            v: session::VERSION,
            tabs: self.tabs.iter().map(Tab::describe).collect(),
        }
    }

    /// The tabs an already-validated document describes, with its consoles open.
    ///
    /// **It returns the tabs and not a window**, so that whoever opens the window remains the only
    /// one that assembles the rest of its state — the stopwatches, the watcher and the counters
    /// come out of the built list, not out of what the file said.
    #[cfg(test)]
    pub(crate) fn materialize(
        ctx: &egui::Context,
        validated: &session::Validated,
        program: &str,
    ) -> term_core::Result<Vec<Tab>> {
        Self::materialize_counting_reads(ctx, validated, program, control::ReadActivity::default())
    }

    fn materialize_counting_reads(
        ctx: &egui::Context,
        validated: &session::Validated,
        program: &str,
        reads: control::ReadActivity,
    ) -> term_core::Result<Vec<Tab>> {
        let mut next_pane = 0;
        validated
            .tabs()
            .iter()
            .enumerate()
            .map(|(which, tab)| Tab::materialize(ctx, tab, program, &mut next_pane, which, &reads))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    // Only declaration witnesses activate this thread-local clock. Real operations still run;
    // completed launches, watcher rearms and screen reflows advance units inside their stopwatch.
    // Logical units test complete accounting, not performance; release binaries contain none.
    // Reader/watcher threads cannot advance this clock, and other tests retain their real clock.
    // The guard restores its previous state on panic as well as success.
    // Exact accumulator deltas catch both omitted declarations and narrowed span boundaries.
    // Independent chord witnesses cover dispatch; these tests isolate its cost declaration.
    std::thread_local! {
        static OPERATION_TIME: std::cell::Cell<Option<std::time::Duration>> = const { std::cell::Cell::new(None) };
    }

    pub(crate) enum OperationWork {
        Launch,
        Rearm,
        ScreenReflow,
    }

    pub(crate) fn completed_operation(work: OperationWork) {
        OPERATION_TIME.with(|time| {
            if let Some(now) = time.get() {
                let units = match work {
                    OperationWork::Launch => 1,
                    OperationWork::Rearm => 10,
                    OperationWork::ScreenReflow => 10,
                };
                time.set(Some(now + std::time::Duration::from_secs(units)));
            }
        });
    }

    struct OperationClockGuard(Option<std::time::Duration>);

    impl OperationClockGuard {
        fn enter() -> Self {
            Self(OPERATION_TIME.with(|time| time.replace(Some(std::time::Duration::ZERO))))
        }
    }

    impl Drop for OperationClockGuard {
        fn drop(&mut self) {
            OPERATION_TIME.with(|time| time.set(self.0));
        }
    }

    pub(crate) struct OperationClock {
        real: std::time::Instant,
        logical: Option<std::time::Duration>,
    }

    impl OperationClock {
        pub(crate) fn start(real: std::time::Instant) -> Self {
            Self {
                real,
                logical: OPERATION_TIME.with(|time| time.get()),
            }
        }

        pub(crate) fn elapsed(&self) -> std::time::Duration {
            match self.logical {
                Some(start) => OPERATION_TIME
                    .with(|time| time.get().expect("the scoped clock is still active") - start),
                None => self.real.elapsed(),
            }
        }
    }
    mod agent_signals;
    #[path = "board.rs"]
    mod board;
    mod board_parity;
    mod context_clipboard;
    mod header_controls;
    mod integrated_chrome;
    pub(crate) mod markdown;
    mod pane_rename;
    mod settings_chrome;
    mod settings_editor;
    mod title_typography;

    // **The tests reach further into the two modules than the code above does**, and this is
    // where that is said. They build tilings and weights directly to assert the arithmetic;
    // the window itself only ever asks for the result.
    use super::*;
    use crate::injected::{InjectedGesture, gesture_of, how_many_drags};
    use crate::session::SAVE_DELAY;
    use crate::tab_bar::{TAB_BAR_MARGIN, tab_rect};
    use crate::tiling::{Divider, INITIAL_FRACTION, Weight, side_of, tile, weights_of};
    use crate::tree::Child;
    use crate::tree::Node;
    use std::time::{Duration, Instant};
    use term_core::Half;

    /// Leaves exactly these events in the window's queue, in this order.
    ///
    /// **It empties whatever was there before, and that is half of the helper.** The notices that
    /// the modifiers changed are deliberately left in the queue when they are processed, so without
    /// emptying it a second call would run against its own **plus those of the previous span** —
    /// and the test would be asserting about a sequence that is not the one it wrote. A real frame
    /// rebuilds the whole list; this does the same.
    fn enqueue(ctx: &egui::Context, events: Vec<egui::Event>) {
        ctx.input_mut(|input| {
            input.events.clear();
            input.events.extend(events);
        });
    }

    /// The window's chord: `Ctrl+Shift`.
    ///
    /// **It is the very same constant production uses**, and that is not a saving of lines: with a
    /// copy, changing the chord would leave the tests green proving the gesture that no longer
    /// exists.
    fn with_shift() -> egui::Modifiers {
        MOVE_CHORD
    }
#[cfg(unix)]
#[test]
    fn a_console_holding_a_paste_paints_the_question_over_it() {
        // **The user-visible half, and without this test there is none.** Deleting the line that
        // paints the notice leaves the whole suite green and turns the product into something worse
        // than the bug it fixes: you paste, nothing reaches the program, nothing appears, and the
        // chord that would answer is eaten by a question nobody can see.
        //
        // **It counts what the painter enqueues**, the way this crate's other painting tests do.
        // Asserting that the field is set would pass with the painting deleted, which is the exact
        // shape this project calls a test that covers nothing.
        let Some(boundary) = keyboard::the_longest_line_a_reader_keeps() else {
            return;
        };
        let mut pane = pane_with("sleep 30", 0);
        let theme = theme::Theme::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 400.0));

        let quiet = shapes_painting(&mut pane, &theme, rect);
        keyboard::paste_or_hold(
            &"x".repeat(boundary + 1),
            false,
            &mut Vec::new(),
            &mut pane.held,
        );
        assert!(
            pane.held.is_some(),
            "the paste was not held, so nothing is being asked"
        );
        let asking = shapes_painting(&mut pane, &theme, rect);

        assert!(
            asking > quiet,
            "a console holding a paste painted no more than one holding none ({asking} against \
             {quiet}): the question is invisible and its chord is eaten by nothing on screen"
        );
    }

    /// How many shapes the painter enqueues for that console, painted onto a layer of its own.
    ///
    /// **It lends the console mutably because painting a question records where its notice landed**,
    /// which is what a click is judged against. A helper that could not do that would be measuring
    /// a painter that does not exist.
    fn shapes_painting(pane: &mut Pane, theme: &theme::Theme, rect: egui::Rect) -> usize {
        let ctx = egui::Context::default();
        // An empty pass first: the text engine does not exist until there has been one.
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut discoverer = fonts::Discoverer::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("the console"),
            ));
            let metrics =
                painter::CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE));
            painter::paint_a_console(
                &painter,
                pane,
                theme,
                &metrics,
                rect,
                &mut discoverer,
                painter::CursorInk::Filled,
            );
        });
        let painted = output.shapes.len();
        output.drop_without_applying_deltas();
        painted
    }

    /// Every thing a notice can say, so an assertion can check the box fits **all** of them.
    ///
    /// **It lives here and not beside the painter, and that is not tidiness.** The shape guard counts
    /// a file's production lines and stops only at a module literally called `tests`, so two
    /// `#[cfg(test)]` helpers left next to the painter's own items were charged against
    /// `painter.rs`'s ceiling — and wrapping them in a module of another name made it worse, by four
    /// lines. A reviewer found it by counting the file with the guard's own rules; here they cost
    /// nothing, because this module is the one the walk stops at.
    ///
    /// **Written out rather than read off the painter's own list**, so the assertions over the box do
    /// not shrink with that list. The third one is held to never being the tallest, so dropping it
    /// from the painter's list changes no box and nothing here goes red — which is the point of that
    /// rule, not a gap in this one.
    fn every_way_to_answer() -> [&'static str; 3] {
        [
            painter::notices::ANSWERED_BY_HAND,
            painter::notices::ANSWERED_BY_CLICK,
            painter::notices::ANSWERED_ONCE_BACK,
        ]
    }

    /// How tall the whole sentence lays out over the console, at the width the box is measured with.
    ///
    /// **It is not the promise any more and it is still the measurement**, which is why deleting it
    /// with the promise was premature: what a roomy console must show is a box the size of its
    /// question, and the only way to say «the size of its question» is to lay the question out.
    fn the_sentence_laid_out(
        pane: &Pane,
        theme: &theme::Theme,
        metrics: &CellMetrics,
        rect: egui::Rect,
        how: &str,
    ) -> egui::Vec2 {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut size = egui::Vec2::ZERO;
        let margin = metrics.width();
        let body = pane
            .held
            .as_ref()
            .expect("that console has a question to measure")
            .sentence();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            size = ui
                .painter()
                .layout(
                    format!("{how}\n\n{body}"),
                    metrics.font(),
                    theme.foreground(),
                    (rect.width() - 4.0 * margin).max(metrics.width()),
                )
                .size()
                + egui::vec2(2.0 * margin, 2.0 * margin);
        })
        .drop_without_applying_deltas();
        size
    }

    /// How tall **only** the line that says how to get out lays out, at the width the box is measured
    /// with.
    ///
    /// **It is the half of the sentence that may never be clipped**, and since the box gained a
    /// ceiling that is a different question from whether the whole sentence fits: on a narrow
    /// console it does not, and the promise kept instead is this one. It is the first thing in the
    /// galley, so it is also the part a clip reaches last.
    fn the_answer_laid_out(
        theme: &theme::Theme,
        metrics: &CellMetrics,
        rect: egui::Rect,
        how: &str,
    ) -> f32 {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut tall = 0.0;
        let margin = metrics.width();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            tall = ui
                .painter()
                .layout(
                    how.to_owned(),
                    metrics.font(),
                    theme.foreground(),
                    (rect.width() - 4.0 * margin).max(metrics.width()),
                )
                .size()
                .y;
        })
        .drop_without_applying_deltas();
        tall
    }

    /// Where that console's notice goes, asked of **the same function the painter and the mouse pass
    /// both call** — so where a button fits under it, this is the shortened question that ships, and
    /// a press at its centre can never be one that lands on the button instead.
    fn the_notice_box(
        pane: &Pane,
        theme: &theme::Theme,
        metrics: &CellMetrics,
        rect: egui::Rect,
    ) -> egui::Rect {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            found = painter::notices::the_boxes(ui.painter(), pane, theme, metrics, rect).question;
        })
        .drop_without_applying_deltas();
        found.expect("that console has a question with a box worth pressing")
    }

    /// Holds a paste on that console, and says so if the platform has no boundary to exceed.
    ///
    /// **The `false` return is not a pass.** Every caller turns it into an early `return`, which on
    /// a platform without a boundary makes the test a silent no-op — said here because **F2.7** is
    /// the next milestone and that is where it will happen.
    fn hold_a_paste_on(pane: &mut Pane) -> bool {
        let Some(boundary) = keyboard::the_longest_line_a_reader_keeps() else {
            return false;
        };
        keyboard::paste_or_hold(
            &"x".repeat(boundary + 1),
            false,
            &mut Vec::new(),
            &mut pane.held,
        );
        pane.held.is_some()
    }

    /// Runs the mouse pass twice over that input: egui resolves an interaction against what was
    /// registered on the previous pass, so a gesture delivered on the first finds nobody to take it.
    /// The first pass carries no events.
    fn deliver_to_the_mouse(
        window: &mut Window,
        ctx: &egui::Context,
        rects: &[egui::Rect],
        metrics: &CellMetrics,
        events: Vec<egui::Event>,
    ) {
        ctx.run_ui(egui::RawInput::default(), |ui| {
            window.handle_the_mouse(ui, rects, metrics);
        })
        .drop_without_applying_deltas();
        ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                window.handle_the_mouse(ui, rects, metrics);
            },
        )
        .drop_without_applying_deltas();
    }

    /// **The user-visible half.** A console paints its own question and the keys are read for one
    /// console only, so the notice has to name what would work **there** — asserted against the text
    /// that was really painted, both ways round.
    ///
    /// **The two readings are taken with the focus really moved**, not set by hand, and **read off
    /// the window's own frame**, since what a console is told is what the window hands its painter —
    /// a console's painter on its own cannot know it. The click and the painting are separate frames
    /// here: that the notice follows the click **in the click's own frame** is held by the cursor's
    /// test of it, `a_click_moves_the_filled_cursor_in_the_same_frame_that_moves_the_focus`, since
    /// both marks are painted from the one value.
#[cfg(unix)]
#[test]
    fn the_notice_names_the_keys_only_where_they_would_arrive() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with("sleep 30", "sleep 30");
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        let area = area_of(800.0 + DIVIDER_THICKNESS);
        let painted = egui::Context::default();
        // The text engine does not exist until there has been a pass.
        text_of_the_two_consoles(&mut window, &painted, area, true);

        // The focus is put on the right-hand console **by the gesture**.
        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            click_at(rects[1].center()),
        );
        let out_of_reach = text_of_the_two_consoles(&mut window, &painted, area, true).remove(0);

        // And back to the left one, which is where the question stands.
        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            click_at(rects[0].right_bottom() - egui::vec2(metrics.width(), metrics.height())),
        );
        let within_reach = text_of_the_two_consoles(&mut window, &painted, area, true).remove(0);

        assert!(
            !out_of_reach.contains("Escape") && !out_of_reach.contains("Ctrl+Enter"),
            "a console the keyboard does not reach painted a key anyway: {out_of_reach:?}"
        );
        assert!(
            out_of_reach.contains("click"),
            "a console the keyboard does not reach said nothing that works there: {out_of_reach:?}"
        );
        assert!(
            within_reach.contains("Escape") && within_reach.contains("Ctrl+Enter"),
            "the console the keyboard does reach stopped naming the keys that answer: \
             {within_reach:?}"
        );
    }

    /// **A press inside the console but outside the notice leaves the question standing**, and
    /// without this the box is load-bearing in no assertion at all: a review measured that hard-wiring
    /// the hit test to «yes» left every other test of this fix green.
#[cfg(unix)]
#[test]
    fn a_press_next_to_the_notice_does_not_answer_it() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with("sleep 30", "sleep 30");
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        let theme = window.theme;
        let notice = the_notice_box(window.active().panes()[0], &theme, &metrics, rects[0]);
        // The far corner of the same console: inside the pane, outside the box. Asserted, so that a
        // box that grew to fill the console turns this test red instead of making it vacuous.
        let beside_it = rects[0].right_bottom() - egui::vec2(metrics.width(), metrics.height());
        assert!(
            !notice.contains(beside_it) && rects[0].contains(beside_it),
            "the point this test presses is not beside the notice: box {notice:?}, point \
             {beside_it:?}"
        );

        deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(beside_it));

        assert!(
            window.active().panes()[0].held.is_some(),
            "a press that missed the notice answered the question anyway"
        );
    }

    /// **The other gap of the report: there was nothing to click.** A click on the notice of a
    /// console that does not have the focus discards its question — the only way a hand on the mouse
    /// could get rid of a question it did not ask for.
#[cfg(unix)]
#[test]
    fn a_click_on_its_notice_discards_the_question_of_an_unfocused_console() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with("sleep 30", "sleep 30");
        focus_on(window.active_mut(), 1);
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        let theme = window.theme;
        let notice = the_notice_box(window.active().panes()[0], &theme, &metrics, rects[0]);

        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            click_at(notice.center()),
        );

        assert!(
            window.active().panes()[0].held.is_none(),
            "the notice says a click discards it, and the click did not"
        );
    }

    /// Where the three things over that console go — the question, the button that sends it and the
    /// silence — asked of **the one function the painter and the mouse pass both call**.
    fn the_notices_on(
        pane: &Pane,
        theme: &theme::Theme,
        metrics: &CellMetrics,
        rect: egui::Rect,
    ) -> (Option<egui::Rect>, Option<egui::Rect>, Option<egui::Rect>) {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = (None, None, None);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let boxes = painter::notices::the_boxes(ui.painter(), pane, theme, metrics, rect);
            found = (boxes.question, boxes.send, boxes.silence);
        })
        .drop_without_applying_deltas();
        found
    }

    /// The question as it is laid out **with no button in mind**, which is what a console gets when
    /// the button has no room.
    fn the_question_alone(
        pane: &Pane,
        theme: &theme::Theme,
        metrics: &CellMetrics,
        rect: egui::Rect,
    ) -> Option<egui::Rect> {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            found = painter::notices::the_question_box_with_no_button(
                ui.painter(),
                pane,
                theme,
                metrics,
                rect,
            );
        })
        .drop_without_applying_deltas();
        found
    }
#[cfg(unix)]
#[test]
    fn the_send_button_sends_the_paste_composed_for_the_mode_at_the_click() {
        for turned_on_before_the_click in [true, false] {
            let ctx = egui::Context::default();
            let rects = two_wide_panes();
            let metrics = with_metrics(|metrics| metrics.clone());
            let received = term_core::received::Received::new("send-held");
            let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
            focus_on(window.active_mut(), 1);
            if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
                return;
            }
            let held_bytes =
                keyboard::the_longest_line_a_reader_keeps().expect("a paste was held") + 1;
            received.wait_until_ready(Duration::from_secs(5));
            let headers = window.headers;
            let pane = &mut window.active_mut().panes_mut()[0];
            pane.taken_over = None;
            if turned_on_before_the_click {
                pane.screen.advance(b"\x1b[?2004h");
            }
            assert_eq!(
                pane.screen.bracketed_paste(),
                turned_on_before_the_click,
                "the premise failed: the program's paste mode is not the one this run needs"
            );
            // Something to scroll back over, written into the grid the painter reads.
            pane.screen.advance("line\r\n".repeat(200).as_bytes());
            pane.screen.scroll_by(Scroll::Lines(20));
            assert!(
                pane.screen.view().scroll() > 0,
                "the premise failed: the view never left the bottom"
            );
            let console = pane.console_within(rects[0], &metrics, SystemTime::now(), headers);
            let theme = window.theme;
            let (_, send, _) =
                the_notices_on(window.active().panes()[0], &theme, &metrics, console);
            let send = send.expect("a console this size has room for the button");

            deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(send.center()));

            let mut expected = Vec::new();
            if turned_on_before_the_click {
                expected.extend_from_slice(b"\x1b[200~");
            }
            expected.extend(std::iter::repeat_n(b'x', held_bytes));
            if turned_on_before_the_click {
                expected.extend_from_slice(b"\x1b[201~");
            }
            let got = received.wait_for(expected.len(), Duration::from_secs(5));

            assert!(
                got == expected,
                "with bracketed paste {} at the click the program received {} bytes starting {:?} \
                 and ending {:?}; the paste composed for that mode is {} bytes",
                if turned_on_before_the_click {
                    "on"
                } else {
                    "off"
                },
                got.len(),
                String::from_utf8_lossy(&got[..got.len().min(8)]),
                String::from_utf8_lossy(&got[got.len().saturating_sub(8)..]),
                expected.len()
            );
            let pane = &window.active().panes()[0];
            assert!(pane.held.is_none(), "the paste was sent and is still held");
            assert!(
                pane.taken_over.is_some(),
                "the person's own paste did not hand the console back"
            );
            assert_eq!(
                pane.screen.view().scroll(),
                0,
                "the paste went out and the view stayed scrolled back, so it landed off screen"
            );
        }
    }
#[cfg(unix)]
#[test]
    fn a_click_on_the_question_discards_the_paste_and_the_program_receives_none_of_it() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let received = term_core::received::Received::new("discard-held");
        let mut window = window_with(&received.script("stty raw -echo"), "sleep 30");
        focus_on(window.active_mut(), 1);
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        let boundary = keyboard::the_longest_line_a_reader_keeps().expect("a paste was held");
        received.wait_until_ready(Duration::from_secs(5));
        let headers = window.headers;
        let theme = window.theme;
        let console = window.active().panes()[0].console_within(
            rects[0],
            &metrics,
            SystemTime::now(),
            headers,
        );
        let (question, send, _) =
            the_notices_on(window.active().panes()[0], &theme, &metrics, console);
        let question = question.expect("a held paste puts its question over the console");
        let send = send.expect("a console this size has room for the button");
        assert!(
            !send.contains(question.center()),
            "the premise failed: the centre of the question lies on the send button"
        );

        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            click_at(question.center()),
        );

        assert!(
            window.active().panes()[0].held.is_none(),
            "the question says a click discards it, and the click did not"
        );

        let pane = &mut window.active_mut().panes_mut()[0];
        keyboard::paste_or_hold(
            &"y".repeat(boundary + 1),
            false,
            &mut Vec::new(),
            &mut pane.held,
        );
        assert!(
            pane.held.is_some(),
            "the premise failed: the second paste was not held"
        );
        let console = pane.console_within(rects[0], &metrics, SystemTime::now(), headers);
        let (_, send, _) = the_notices_on(window.active().panes()[0], &theme, &metrics, console);
        let send = send.expect("a console this size has room for the button");

        deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(send.center()));

        let expected = vec![b'y'; boundary + 1];
        let got = received.wait_for(expected.len(), Duration::from_secs(5));

        let xs = got.iter().filter(|&&byte| byte == b'x').count();
        assert!(
            got == expected,
            "the program received {} bytes, {xs} of them from the discarded paste; the second \
             paste alone is {} bytes",
            got.len(),
            expected.len()
        );
    }

    /// **A press on the button carrying a chord sends nothing**, for the reason a chorded press on
    /// the question discards nothing: `Ctrl+Shift` moves the console, and Shift on its own is the
    /// escape hatch that takes the selection back.
#[cfg(unix)]
#[test]
    fn a_chorded_press_on_the_send_button_does_not_send_the_paste() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());

        for chord in [MOVE_CHORD, egui::Modifiers::SHIFT] {
            let mut window = window_with("sleep 30", "sleep 30");
            if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
                return;
            }
            let headers = window.headers;
            let theme = window.theme;
            let console = window.active_mut().panes_mut()[0].console_within(
                rects[0],
                &metrics,
                SystemTime::now(),
                headers,
            );
            let (_, send, _) =
                the_notices_on(window.active().panes()[0], &theme, &metrics, console);
            let at = send
                .expect("a console this size has room for the button")
                .center();

            deliver_to_the_mouse(
                &mut window,
                &ctx,
                &rects,
                &metrics,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: chord,
                    },
                ],
            );

            assert!(
                window.active().panes()[0].held.is_some(),
                "a press with {chord:?} on the send button answered the question"
            );
        }
    }

    /// **Where the button goes, over every height a console can have at the narrowest width.**
    ///
    /// Four things, and the sweep is what makes each of them mean something: the button is under the
    /// question and inside the top half wherever it is drawn; a question at its ceiling is shortened
    /// to make room for it rather than losing it; and where it has no room the question is exactly
    /// what it was before the button existed. **Both kinds of console are asserted to occur**, so a
    /// button that never appears, or always does, cannot pass by leaving one branch unwalked.
#[cfg(unix)]
#[test]
    fn the_send_button_sits_under_the_question_inside_the_top_half_or_not_at_all() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        let mut with_a_button = 0;
        let mut shortened = 0;
        let mut without_one = 0;

        for rows in 1..=60 {
            let console = egui::Rect::from_min_size(
                egui::pos2(41.0, metrics.height() * 1.5),
                egui::vec2(metrics.width() * 20.0, metrics.height() * rows as f32),
            );
            let alone = the_question_alone(&pane, &theme, &metrics, console);
            let (question, send, _) = the_notices_on(&pane, &theme, &metrics, console);
            // Half written out rather than read from the painter's constant, with the slack this
            // file uses for a bound re-derived from a sum.
            let the_middle = console.top() + console.height() * 0.5;

            match send {
                Some(send) => {
                    with_a_button += 1;
                    let question = question.expect("a button with no question over it");
                    // **A margin under the question, exactly**, and not merely somewhere below it: a
                    // reservation of the wrong size still leaves the button under the question and
                    // inside the half, while dropping it from consoles where it fits.
                    assert!(
                        (send.top() - (question.bottom() + metrics.width())).abs() <= 0.5
                            && send.bottom() <= the_middle + 1.0,
                        "at {rows} rows the button is {send:?}, the question {question:?} and the \
                         middle at {the_middle:?}: it is not a margin under the question inside the \
                         top half"
                    );
                    let alone = alone.expect("a question with a button but none on its own");
                    if question.height() + 1.0 < alone.height() {
                        shortened += 1;
                        // A shortened question gave up exactly the button's room, so the pair ends at
                        // the middle and not short of it.
                        assert!(
                            (send.bottom() - the_middle).abs() <= 1.0,
                            "at {rows} rows the question was shortened and the button ends at {:?}, \
                             not at the middle {the_middle:?}: it gave up more room than the button \
                             takes",
                            send.bottom()
                        );
                    }
                }
                None => {
                    without_one += 1;
                    assert_eq!(
                        question, alone,
                        "at {rows} rows there is no button, and the question is not the one it \
                         was before the button existed"
                    );
                }
            }
        }

        assert!(
            with_a_button > 0 && without_one > 0 && shortened > 0,
            "the sweep walked {with_a_button} consoles with a button, {shortened} of them with a \
             shortened question, and {without_one} without one: a branch went unwalked"
        );
        // And a console that shows the question without the button exists — the console the
        // decision was about, which must not lose the notice to make room.
        let question_but_no_button = (1..=60).any(|rows| {
            let console = egui::Rect::from_min_size(
                egui::pos2(41.0, metrics.height() * 1.5),
                egui::vec2(metrics.width() * 20.0, metrics.height() * rows as f32),
            );
            let (question, send, _) = the_notices_on(&pane, &theme, &metrics, console);
            question.is_some() && send.is_none()
        });
        assert!(
            question_but_no_button,
            "no height shows the question without the button, so the console too short for the \
             button was never measured"
        );
        // **And a console too narrow for the button** — tall enough for everything, but the button,
        // placed a margin in, would run past its right edge. The question stays; the button does not.
        let narrow = egui::Rect::from_min_size(
            egui::pos2(41.0, metrics.height() * 1.5),
            egui::vec2(metrics.width() * 16.0, metrics.height() * 60.0),
        );
        let (question, send, _) = the_notices_on(&pane, &theme, &metrics, narrow);
        assert!(
            question.is_some() && send.is_none(),
            "a console sixteen cells wide got question {question:?} and button {send:?}: the button \
             is drawn past the console's edge, or the question was lost with it"
        );
    }

    /// **The console saying it went quiet moves nothing above it.** The sentence goes under the
    /// button's place, so the question and the button are where they are with it standing and after
    /// it is dismissed — which is what keeps a second press on a sentence just dismissed from landing
    /// on a button that was not under it a moment before.
#[cfg(unix)]
#[test]
    fn the_console_saying_it_went_quiet_moves_neither_the_question_nor_its_button() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        // A program that writes, because a listener that is gone is only noticed on a delivery.
        let mut pane = pane_with("yes", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        make_it_go_quiet(&mut pane);
        let tall = egui::Rect::from_min_size(
            egui::pos2(41.0, metrics.height() * 1.5),
            egui::vec2(metrics.width() * 80.0, metrics.height() * 40.0),
        );

        let (question, send, silence) = the_notices_on(&pane, &theme, &metrics, tall);
        let silence =
            silence.expect("the premise failed: the console says nothing about going quiet");
        let send = send.expect("a console this size has room for the button");
        assert!(
            silence.top() >= send.bottom(),
            "the sentence is at {silence:?} and the button at {send:?}: it is not under the button"
        );

        pane.pty_output.dismiss_the_silence(pane.pty.ending());
        let (question_after, send_after, silence_after) =
            the_notices_on(&pane, &theme, &metrics, tall);
        assert!(
            silence_after.is_none(),
            "the premise failed: the dismissal did nothing"
        );
        assert_eq!(
            (question, Some(send)),
            (question_after, send_after),
            "dismissing the sentence moved the question or the button"
        );
    }

    /// **The box fits whichever sentence is shown**, and without that the two passes can judge
    /// different rectangles on the frame the focus moves — the mouse pass hit-testing one box while
    /// the painter draws another.
    ///
    /// **Measuring by the longer STRING was the first attempt and it was wrong**: the two answer
    /// sentences are exactly the same length in characters, and what sets the height is **wrapping**.
    ///
    /// **And the widths are measured, not picked.** Walking 20 to 60 cells, the two only ever wrap
    /// differently in five of them, and they do it in **both directions**: at 20 cells the clicking
    /// sentence is the taller one (384.9 against 368.9) and at 26 the keyboard's is (288.9 against
    /// 272.9). Both are asserted, because measuring against either sentence alone passes at the
    /// width where that one happens to be the taller — the first version of this test used 34
    /// cells, where they wrap the same, and stayed green under the exact mutation it was written
    /// for. Twenty cells is also this application's floor for a console, so it is not a contrived
    /// width.
    ///
    /// **What this asserts changed when the box got a ceiling, and the product promise changed with
    /// it.** It used to demand that the whole sentence fit; on a console of twenty cells and forty
    /// rows, half the height is 312 px and the sentence lays out 400, so the two promises — «never
    /// more than half a console» and «the sentence is whole» — cannot both hold, and the ceiling was
    /// chosen. What is promised instead, and asserted here, is that **the answer** always fits: the
    /// line saying how to get out is the first thing in the galley, so what a clip takes is the tail
    /// that counts bytes. The equality of the two boxes across the focus moving is unchanged and is
    /// still the reason this test exists.
#[cfg(unix)]
#[test]
    fn the_notice_box_does_not_shrink_when_the_keyboard_leaves() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        for cells in [20.0, 26.0] {
            let narrow = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(metrics.width() * cells, metrics.height() * 40.0),
            );
            let box_of_it = the_notice_box(&pane, &theme, &metrics, narrow);

            // **Asserted against BOTH sentences laid out, and not against the other box.** Comparing
            // the two boxes proves nothing: measuring them both with one sentence makes them equal
            // too, which is the mutation the comparison stayed green under.
            // **Half, written out here rather than read from the painter's constant.** A test that
            // asked the same constant the code asks would pass under a mutation of it; this one
            // pins the promise a person was given.
            assert!(
                box_of_it.height() <= narrow.height() * 0.5,
                "at {cells} cells the box is {:?} tall over a console of {:?}: it takes more than \
                 half, so the bottom half a person marks and copies in is no longer theirs",
                box_of_it.height(),
                narrow.height()
            );
            for how in every_way_to_answer() {
                let answer = the_answer_laid_out(&theme, &metrics, narrow, how);
                assert!(
                    answer <= box_of_it.height() - 2.0 * metrics.width(),
                    "at {cells} cells the box leaves {:?} for text and the line of «{how}» that says \
                     how to get out lays out {answer:?}: the answer itself would be clipped, which \
                     is worse than no question at all",
                    box_of_it.height() - 2.0 * metrics.width()
                );
            }
        }
    }

    /// **The border of the notice could be deleted outright and every test stayed green**, which is
    /// the whole of why this exists.
    ///
    /// The painting *was* being walked — a helper of these tests went through
    /// `painter::paint_a_console`, which paints the question — but every test that walked it read
    /// `Shape::Text` and nothing else, while the ones that reason about the box's size enter through
    /// `painter::notices::the_question_box_with_no_button`, which measures it and hands the rectangle back without drawing
    /// anything. So the border had no assertion of any kind on it, and it fails the way the cursor's
    /// outline did: `Stroke::new(0.0, …)` is queued, carries the right colour and covers the right
    /// rectangle, and cannot be seen. A console holding a paste would show the question floating
    /// over whatever the program had written underneath, with nothing separating the two.
#[cfg(unix)]
#[test]
    fn the_box_of_a_held_paste_is_stroked_over_the_console_holding_it() {
        let mut window = window_with("sleep 30", "sleep 30");
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        // **The console holding the paste is asked for the keyboard on purpose, not left to a
        // default.** It is what puts both marks in the same frame in the same colour, and that is
        // the only scenario in which either lookup's square / rounded discriminator does any work.
        // Depending on where the focus happens to start would make the whole point of this test an
        // accident of another function.
        focus_on(window.active_mut(), 0);
        let area = area_of(800.0 + DIVIDER_THICKNESS);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(area),
            ..Default::default()
        };
        let mut frame = eframe::Frame::_new_kittest();
        let mut output = ctx.run_ui(input, |ui| eframe::App::ui(&mut window, ui, &mut frame));
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();

        let outline = THE_FOCUS_COLOUR;
        let (stroked_at, box_of_it) = the_held_pastes_box(&shapes, outline);

        // **The console holding the paste is also the one with the keyboard, so both marks are in
        // this frame in the same colour** — and that is the only place either lookup's square /
        // rounded discriminator is exercised at all. Everywhere else only the highlight exists, so
        // the check that there is exactly one of them holds by there being nothing to confuse it
        // with.
        assert_ne!(
            box_of_it,
            the_focus_highlight(&shapes, outline),
            "the notice's box and the border around the focused console came out as the same \
             rectangle, so one of the two lookups is reading the other's mark"
        );

        // **The box is filled as well as stroked, and nothing asserted the fill.** Deleting it
        // leaves the question painted straight over the program's own text, unreadable, with every
        // assertion here still passing: a border around transparency is not a box.
        let filled_at = shapes
            .iter()
            .enumerate()
            .find(|(_, clipped)| match &clipped.shape {
                egui::Shape::Rect(shape) => {
                    shape.fill == THE_PANEL_BACKGROUND && shape.rect == box_of_it
                }
                _ => false,
            })
            .map(|(at, _)| at);
        let Some(filled_at) = filled_at else {
            panic!(
                "the notice's box is stroked but not filled, so the question is painted over \
                 whatever the program had written underneath"
            );
        };
        // **And the fill goes first.** Painted after the border it covers it entirely — same
        // rectangle, same corner, and a line of one point inside it — so the box loses its edge with
        // both shapes still on the queue, both the right colour and both the right size. Nothing
        // else in this crate asserts the order two shapes are queued in.
        assert!(
            filled_at < stroked_at,
            "the notice's box is filled after it is stroked, so the fill covers its own border"
        );

        // **Where it fell is asserted against the tiling, not against `the_question_box_with_no_button`.** Asking
        // that function what to expect would put the same call on both sides of the comparison —
        // the painter reaches the rectangle through it — so the assertion would hold however the
        // measuring changed.
        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let tiling = tile(
            area,
            Orientation::Columns,
            &weights_of(INITIAL_FRACTION),
            metrics.width(),
        );
        assert!(
            tiling.parts[0].contains_rect(box_of_it),
            "the box is at {box_of_it:?} and the console holding the paste is {:?}: the question is \
             drawn outside the console it belongs to",
            tiling.parts[0]
        );
        assert!(
            !tiling.parts[1].intersects(box_of_it),
            "the box reaches into the console that is holding nothing, which is a question asked of \
             the wrong program"
        );

        // **And the button under it is a box too**, for the same three reasons: stroked one point
        // wide, filled, and filled first. Found by its label, which is what a person reads.
        let label = where_the_send_label_is(&shapes).expect("the send button's label was painted");
        let button: Vec<_> = shapes
            .iter()
            .enumerate()
            .filter_map(|(at, clipped)| match &clipped.shape {
                egui::Shape::Rect(shape) if shape.rect.contains(label) => Some((at, shape)),
                _ => None,
            })
            .collect();
        // **Rounded, and it has to say so**: the square highlight around the focused console is
        // stroked in the same colour and also contains the label, so without the corner the lookup
        // lands on it the moment the button's own border is gone.
        let stroked = button
            .iter()
            .find(|(_, shape)| {
                shape.stroke.color == outline
                    && shape.stroke.width == 1.0
                    && shape.corner_radius != egui::CornerRadius::ZERO
            })
            .map(|(at, _)| *at)
            .expect("the send button is not stroked one point wide in the focus colour");
        let filled = button
            .iter()
            .find(|(_, shape)| shape.fill == THE_PANEL_BACKGROUND && shape.rect != box_of_it)
            .map(|(at, _)| *at)
            .expect(
                "the send button is not filled, so its label is painted over the program's text",
            );
        assert!(
            filled < stroked,
            "the send button is filled after it is stroked, so the fill covers its own border"
        );
        assert!(
            tiling.parts[0].contains(label) && label.y > box_of_it.bottom(),
            "the send button's label is at {label:?}, not under the question {box_of_it:?} inside \
             the console holding the paste"
        );
    }

    /// **A console too small has no notice to press.** The box is clamped to the console, and a
    /// three-row pane flattens it to something whose text is entirely clipped — a rectangle that
    /// still answers `contains` on its edge, so a press there would discard a paste while showing
    /// nothing that could be read as a question.
#[cfg(unix)]
#[test]
    fn a_console_too_small_for_the_notice_has_no_box_to_press() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        let sliver = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(metrics.width() * 20.0, metrics.height() * 1.2),
        );

        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            found = painter::notices::the_question_box_with_no_button(
                ui.painter(),
                &pane,
                &theme,
                &metrics,
                sliver,
            );
        })
        .drop_without_applying_deltas();

        assert!(
            found.is_none(),
            "a console of one row offered a box to press: {found:?}"
        );
    }

    /// **The shape where «a line fits» was not enough, which is the one a review named.**
    ///
    /// At this application's narrowest console the line saying how to get out wraps to about three,
    /// while a console of five to seven rows has room under the ceiling for **one**. So a floor
    /// asking for a line let a box through showing a third of an instruction — legible enough to
    /// look deliberate and useless for getting out. The floor asks for the whole answer instead, and
    /// this is the console that tells the two rules apart: under the old one it has a box, under this
    /// one it has none.
    ///
    /// **The silence is the price and it is recorded, not hidden**: such a console holds a paste with
    /// nothing on screen saying so. Choosing that over an unreadable rectangle was the user's call.
crate::windows_unix_omission!(
    a_console_with_room_for_a_line_but_not_the_answer_has_no_box,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn a_console_with_room_for_a_line_but_not_the_answer_has_no_box() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();

        // Walked rather than picked: the band is where the ceiling leaves at least one row and less
        // than the answer needs, and a single height would be a number chosen to pass.
        let mut seen_a_console_in_the_band = false;
        for rows in [5.0, 6.0, 7.0] {
            let narrow = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(metrics.width() * 20.0, metrics.height() * rows),
            );
            let mut found = None;
            let mut answer = 0.0;
            ctx.run_ui(egui::RawInput::default(), |ui| {
                found = painter::notices::the_question_box_with_no_button(
                    ui.painter(),
                    &pane,
                    &theme,
                    &metrics,
                    narrow,
                );
                answer = ui
                    .painter()
                    .layout(
                        painter::notices::ANSWERED_BY_HAND.to_owned(),
                        metrics.font(),
                        theme.foreground(),
                        (narrow.width() - 4.0 * metrics.width()).max(metrics.width()),
                    )
                    .size()
                    .y;
            })
            .drop_without_applying_deltas();

            let room = narrow.height() * 0.5 - 3.0 * metrics.width();
            if room >= metrics.height() && room < answer {
                seen_a_console_in_the_band = true;
                assert!(
                    found.is_none(),
                    "a console of {rows} rows leaves {room:?} for text and the way out needs \
                     {answer:?}, and it still offered {found:?}: a third of an instruction reads as \
                     deliberate and does not get anybody out"
                );
            }
        }
        assert!(
            seen_a_console_in_the_band,
            "no console in this walk had room for a line and not for the answer, so this test \
             asserted nothing: the band moved and the heights have to be measured again"
        );
    }

    /// **The wording for a window that lost the desktop's focus never lays out taller than the other
    /// two**, with the built-in font, at every width from 4 to 120 cells.
    ///
    /// **The way out on its own is enough to ask about**: the explanation under it is the same for
    /// all three and starts on a line of its own, so the whole question differs by exactly this.
    ///
    /// The box is measured against every wording, so a taller third one raises the floor of every
    /// console — in a focused window too, where it is never shown — and a console that used to hold
    /// the question holds its paste in silence instead. The first wording written for it did that.
#[cfg(unix)]
#[test]
    fn the_wording_for_an_absent_window_costs_no_console_its_notice() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        let [by_hand, by_click, once_back] = every_way_to_answer();
        for cells in 4..=120 {
            let area = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(metrics.width() * cells as f32, metrics.height() * 24.0),
            );
            let answer = |how| the_answer_laid_out(&theme, &metrics, area, how);
            assert!(
                answer(once_back) <= answer(by_hand).max(answer(by_click)),
                "at {cells} cells «{once_back}» lays out taller than both other ways out, so it \
                 raises the floor of every console"
            );
        }
    }

    /// **The way out that fits has to be the one that will be shown, and which one that is depends on
    /// where the keyboard is** — so a floor measured against either sentence on its own promises
    /// nothing.
    ///
    /// The two are the same length in characters and **wrap apart**: measured on this host, at 21
    /// cells the unfocused console's line takes a row more than the focused one's, and at 19 cells it
    /// is the other way round. So a floor that asked only about «Escape or a click…» would hand a
    /// box to a console with room for that and not for «A click discards it…», and the console it
    /// clips is the one nobody is looking at — the case the whole sentence exists for, since a paste
    /// held in silence is the failure this feature was built against.
    ///
    /// **Asked of a walk and not of a shape**, with its own guard against asserting nothing: the
    /// widths where the two wrap apart are a property of a font and a string, so a single pair of
    /// numbers here would rot into a test that passes because it stopped discriminating.
crate::windows_unix_omission!(
    no_console_gets_a_box_that_clips_the_way_out_it_will_show,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn no_console_gets_a_box_that_clips_the_way_out_it_will_show() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        let margin = metrics.width();
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();

        // A shape worth the walk is one where a box exists, the two lines wrap to different heights,
        // and the room is tight enough that forgetting one of them would have changed the answer.
        let mut saw_a_box_where_the_two_wrap_apart = false;
        for cells in 16..=32 {
            for rows in 8..=16 {
                let area = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(
                        metrics.width() * cells as f32,
                        metrics.height() * rows as f32,
                    ),
                );
                let mut found = None;
                let mut answers = [0.0_f32; 3];
                ctx.run_ui(egui::RawInput::default(), |ui| {
                    found = painter::notices::the_question_box_with_no_button(
                        ui.painter(),
                        &pane,
                        &theme,
                        &metrics,
                        area,
                    );
                    let wrap = (area.width() - 4.0 * margin).max(metrics.width());
                    for (at, how) in every_way_to_answer().into_iter().enumerate() {
                        answers[at] = ui
                            .painter()
                            .layout(how.to_owned(), metrics.font(), theme.foreground(), wrap)
                            .size()
                            .y;
                    }
                })
                .drop_without_applying_deltas();

                let Some(box_of_it) = found else {
                    continue;
                };
                let room = box_of_it.height() - 2.0 * margin;
                // **The guard is about the two that wrap apart, and only them**: the third is held
                // to never being the tallest by a test of its own, so letting it satisfy this guard
                // would make the walk look discriminating while it checked nothing about the two.
                let taller = answers[0].max(answers[1]);
                if answers[0] != answers[1] && room < taller + metrics.height() {
                    saw_a_box_where_the_two_wrap_apart = true;
                }
                for (at, how) in every_way_to_answer().into_iter().enumerate() {
                    assert!(
                        room + 1.0 >= answers[at],
                        "a console of {cells} cells by {rows} rows got a box leaving {room:?} for \
                         text, and «{how}» lays out {:?} there: the console that shows that line \
                         has its own way out clipped, so the floor was measured against something \
                         other than the room this box leaves for EVERY sentence — another \
                         sentence, or the box instead of its text area",
                        answers[at]
                    );
                }
            }
        }
        assert!(
            saw_a_box_where_the_two_wrap_apart,
            "no console in this walk got a box while the two ways out wrapped to different heights \
             with the room tight, so this test asserted nothing about which of them the floor \
             asks: the widths where they wrap apart have to be measured again"
        );
    }

    /// **The sentence is clipped to its own box, and nothing said so until a review did.**
    ///
    /// The ceiling shortens the box; nothing shortens the text. The only clip in force is the
    /// console's, so every line past the ceiling was queued **outside** the filled rectangle — over
    /// the grid, with no fill behind it, in the half the ceiling exists to keep clear, and outside
    /// the rectangle a press is judged by. The whole argument for choosing the ceiling over the
    /// sentence rested on «a clip takes the tail, never the answer», and that clip did not exist.
    ///
    /// **This asserts the clip and not the drawing**, because the drawing is what looked right: the
    /// galley is queued at the same position either way, and the only difference is the rectangle it
    /// is clipped to. A test that read the text's position saw nothing wrong, which is why nothing
    /// caught it.
#[cfg(unix)]
#[test]
    fn the_question_is_clipped_to_its_own_box() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        // The shape the ceiling bites on: narrow enough that the sentence wraps long, short enough
        // that half of it is less than the sentence needs. The diff's own measurement.
        let narrow = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(metrics.width() * 20.0, metrics.height() * 40.0),
        );

        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut discoverer = fonts::Discoverer::default();
        let mut box_of_it = None;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("the console"),
            ));
            box_of_it = painter::notices::the_question_box_with_no_button(
                &painter, &pane, &theme, &metrics, narrow,
            );
            painter::paint_a_console(
                &painter,
                &pane,
                &theme,
                &metrics,
                narrow,
                &mut discoverer,
                painter::CursorInk::Filled,
            );
        });
        let box_of_it = box_of_it.expect("that console has a question");

        let text = output
            .shapes
            .iter()
            .find(|clipped| matches!(clipped.shape, egui::Shape::Text(_)))
            .expect("the question is drawn");

        assert!(
            text.clip_rect.max.y <= box_of_it.max.y + 1.0,
            "the sentence is clipped to {:?}, whose bottom is past the box's at {:?}: the lines the \
             ceiling cut are being drawn over the console's bottom half with nothing behind them",
            text.clip_rect,
            box_of_it.max.y
        );

        // **And the clip has to COVER the way out, which is the other half of the same claim.** A
        // bound on where the clip ends says nothing about where it begins: a rectangle moved
        // sideways off the box, or cut down to the box's first row, ends no lower than the box does
        // and hides the words anyway — the whole question in the first case, and in the second
        // exactly the line saying how to get out, which is first in the galley and therefore the
        // part every other assertion here assumes survives. Both leave the shape on the queue, so
        // there is nothing missing to notice: **the sentence is queued, clipped to nowhere, and
        // never drawn**.
        //
        // Measured against the **tallest** of the answers for the same reason the box is: which one
        // is shown depends on where the keyboard is, and whichever it is has to be whole.
        let answer = every_way_to_answer()
            .map(|how| the_answer_laid_out(&theme, &metrics, narrow, how))
            .into_iter()
            .fold(0.0_f32, f32::max);
        let where_the_answer_goes = egui::Rect::from_min_size(
            box_of_it.left_top() + egui::vec2(metrics.width(), metrics.width()),
            egui::vec2(box_of_it.width() - 2.0 * metrics.width(), answer),
        );
        assert!(
            text.clip_rect.contains_rect(where_the_answer_goes),
            "the sentence is clipped to {:?} and the way out is drawn in {where_the_answer_goes:?} \
             inside a box of {box_of_it:?}: what the clip does not cover is not painted at all, and \
             a question whose answer is invisible is worse than no question",
            text.clip_rect
        );
        output.drop_without_applying_deltas();
    }

    /// **A guard was written here, found to have no scenario, and removed — and this is the test that
    /// keeps that true.**
    ///
    /// The ceiling's arithmetic goes negative on a console under one row, so it was clamped at zero.
    /// Measured: with the clamp and without it, such a console answers **no box** either way — a
    /// negative size makes a degenerate rectangle that the floor rejects for the same reason a flat
    /// one is rejected. A mechanism whose only scenario gives the same answer without it is a
    /// mechanism with no scenario, and this project removes those rather than keeping them for
    /// comfort.
    ///
    /// So what is pinned is not the clamp: it is **the relationship that made it unnecessary**. The
    /// day the floor stops rejecting a degenerate box, this goes red and the clamp's argument is
    /// reopened instead of quietly becoming false — the same answer given to the reader loop's own
    /// unreachable guard, and what the store's card on unreachable guards prescribes.
#[cfg(unix)]
#[test]
    fn a_console_under_one_row_has_no_box_either() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        // Under one row the ceiling's own arithmetic goes negative, which is the only scenario a
        // guard there would have. This is the probe that says whether the guard changes the answer.
        let hair = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(metrics.width() * 40.0, metrics.height() * 0.4),
        );

        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            found = painter::notices::the_question_box_with_no_button(
                ui.painter(),
                &pane,
                &theme,
                &metrics,
                hair,
            );
        })
        .drop_without_applying_deltas();

        assert!(found.is_none(), "a console under one row offered {found:?}");
    }

    /// **The console the ceiling makes dangerous, and it is not a sliver.** Three rows is a shape the
    /// tiling allows and the old floor accepted: the box grew to fit its text, was clamped to the
    /// console, and came out a whole row tall, so «at least one row» read as «there is something
    /// readable in it».
    ///
    /// With a ceiling of half the height that stops being true and the floor has to be measured
    /// against what the ceiling leaves. Measured on this host at the font in use: a three-row
    /// console is 48 px, half of that minus the margin is 16, and the two margins take all of it —
    /// **nothing left for a line of text**. Judged the old way it passes the floor, gets stroked,
    /// swallows the press and shows nothing, which is exactly the defect the floor exists against.
    ///
    /// So this asserts the **absence** of a box on a console that a naive ceiling would have given
    /// one, and it is the test that fails if the two are ever reordered.
#[cfg(unix)]
#[test]
    fn a_console_whose_ceiling_leaves_no_room_for_a_line_has_no_box() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        let three_rows = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(metrics.width() * 40.0, metrics.height() * 3.0),
        );

        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            found = painter::notices::the_question_box_with_no_button(
                ui.painter(),
                &pane,
                &theme,
                &metrics,
                three_rows,
            );
        })
        .drop_without_applying_deltas();

        assert!(
            found.is_none(),
            "a three-row console offered a box of {found:?}: under the ceiling it has no room for a \
             line of text, so anything stroked there takes a press and shows nothing"
        );
    }

    /// **The ceiling is a ceiling and not a size**: a console with room to spare gets the box its
    /// sentence needs, not half the console.
    ///
    /// Without this, capping to exactly half would pass every other assertion here — the box would
    /// be inside the console, under the ceiling, and tall enough for a line — while drawing a
    /// rectangle far larger than the question in it, over output somebody wants to read.
crate::windows_unix_omission!(
    a_tall_console_gets_the_box_its_question_needs_and_not_half_the_screen,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn a_tall_console_gets_the_box_its_question_needs_and_not_half_the_screen() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        // Wide enough that the sentence wraps to few lines, tall enough that half of it is far more
        // than those lines need.
        let roomy = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(metrics.width() * 60.0, metrics.height() * 60.0),
        );

        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            found = painter::notices::the_question_box_with_no_button(
                ui.painter(),
                &pane,
                &theme,
                &metrics,
                roomy,
            );
        })
        .drop_without_applying_deltas();
        let box_of_it = found.expect("a console with room to spare has a box");

        // **Against the question's own height, not against the ceiling.** Comparing with half the
        // console proves nothing, and a mutation showed it: the ceiling is half MINUS a margin, so a
        // box pinned to the ceiling is always strictly under half and the comparison stayed green
        // while every box came out exactly half a console tall.
        let needed = every_way_to_answer()
            .map(|how| the_sentence_laid_out(&pane, &theme, &metrics, roomy, how).y)
            .into_iter()
            .fold(0.0_f32, f32::max);
        assert!(
            box_of_it.height() <= needed + 1.0,
            "the box is {:?} tall and its question needs {needed:?} over a console of {:?}: the \
             ceiling is being used as a size, so the question is drawn over output nobody asked it \
             to cover",
            box_of_it.height(),
            roomy.height()
        );
    }
#[cfg(unix)]
#[test]
    fn a_capped_notice_ends_at_the_middle_of_its_console_and_not_past_it() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        // Twenty cells is this application's floor for a console and the width two panes get, which
        // is where the sentence wraps long enough to be capped at all.
        let narrow = egui::Rect::from_min_size(
            egui::pos2(41.0, metrics.height() * 1.5),
            egui::vec2(metrics.width() * 20.0, metrics.height() * 40.0),
        );
        // The question on its own: with the button under it the pair is what ends at the middle, and
        // the sweep over the button's place asserts that.
        let box_of_it = the_question_alone(&pane, &theme, &metrics, narrow)
            .expect("that console has a question with a box worth pressing");

        // **Its own guard against asserting nothing.** A box that never reaches the ceiling satisfies
        // any bound on where it ends, so the shape has to be one whose question does not fit.
        let needed = every_way_to_answer()
            .map(|how| the_sentence_laid_out(&pane, &theme, &metrics, narrow, how).y)
            .into_iter()
            .fold(0.0_f32, f32::max);
        assert!(
            needed + 2.0 * metrics.width() > narrow.height() * 0.5,
            "the question needs {needed:?} over a console of {:?}, so it already fits under the \
             ceiling and the box is not at it: this test says nothing about the bound and the shape \
             has to be measured again",
            narrow.height()
        );

        // **Half written out here rather than read from the painter's constant**, for the reason the
        // neighbouring test gives: a test that asks the same constant the code asks passes under a
        // mutation of it. The slack is the one this file uses for a bound re-derived from a sum.
        let the_middle = narrow.top() + narrow.height() * 0.5;
        assert!(
            (box_of_it.max.y - the_middle).abs() <= 1.0,
            "the box ends at {:?} over a console that runs {:?} to {:?}, whose middle is at \
             {the_middle:?} — {:?} out. Past it, the bottom half a person marks and copies in is \
             not theirs any more while the box is still under half a console TALL; short of it, the \
             notice is showing less of its question than the half it was given",
            box_of_it.max.y,
            narrow.top(),
            narrow.bottom(),
            box_of_it.max.y - the_middle
        );
    }

    /// Makes that console's reading end in a way that is **not** an ordinary ending, and waits for
    /// the reason to be recorded.
    ///
    /// **A read that failed cannot be built from outside `term-core`** — on Linux the one errno a
    /// broken pseudoterminal gives is mapped to end of file before the failing arm — so the ending
    /// used here is the other one that is not «ended well»: a bridge with nothing behind it leaves
    /// the reading with no listener while the program inside is alive and writing, which is exactly
    /// what the sentence on the glass claims.
    fn make_it_go_quiet(pane: &mut Pane) {
        let (orphan, nowhere) = std::sync::mpsc::channel();
        pane.pty_output = PtyOutput::new(nowhere, || {});
        drop(orphan);
        let deadline = Instant::now() + Duration::from_secs(5);
        while pane.pty.ending().is_none() {
            assert!(
                Instant::now() < deadline,
                "the reading never noticed its listener was gone"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_ne!(
            pane.pty.ending(),
            Some(term_core::Ending::EndedWell),
            "this console ended well, so there would be nothing to say about it"
        );
    }

    /// Writes enough bytes into that console's real pseudoterminal to trip #161's ceiling, and
    /// asserts the refusal actually happened before returning — the premise-before-conclusion
    /// shape term-core's own ceiling tests use, reached here through a real [`Pane`] instead.
    ///
    /// **The console's program has to already be raw, echo off, and not reading its input** — the
    /// same `stty raw -echo; …; exec sleep N` shape those tests use — or these bytes are either
    /// echoed back and drained by the line discipline, or land on a program that actually reads
    /// them, and the premise below never holds.
    fn make_it_refuse_a_write(pane: &mut Pane) {
        // Comfortably past term-core's own ceiling (256 KiB) without this crate hard-coding the
        // exact figure it has no business knowing — the margin is the point, not the number. The
        // ceiling is checked against what is already owed the instant a write is *handed over*,
        // not against how far the writer thread has gotten, so nothing here needs to wait for it.
        const PAST_THE_CEILING: usize = 300 * 1024;
        pane.pty
            .write(&vec![b'z'; PAST_THE_CEILING])
            .expect("a first write, alone, must still be accepted");
        assert!(
            pane.pty.write(b"one more byte").is_err(),
            "the queue did not refuse a write past its ceiling: the premise for this test does \
             not hold"
        );
    }

    /// Where that console's silence notice goes, asked of the same function the painter and the
    /// mouse pass both call.
    fn the_silence_box_on(
        pane: &Pane,
        theme: &theme::Theme,
        metrics: &CellMetrics,
        rect: egui::Rect,
    ) -> Option<egui::Rect> {
        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            found = painter::notices::the_boxes(ui.painter(), pane, theme, metrics, rect).silence;
        })
        .drop_without_applying_deltas();
        found
    }

    /// How much room this console's silence sentence needs, laid out the way the notice lays it out.
    fn the_sentence_laid_out_over(
        pane: &Pane,
        theme: &theme::Theme,
        metrics: &CellMetrics,
        area: egui::Rect,
    ) -> egui::Vec2 {
        let margin = metrics.width();
        let ctx = egui::Context::default();
        let mut size = egui::Vec2::ZERO;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            size = ui
                .painter()
                .layout(
                    painter::notices::what_it_says(false),
                    metrics.font(),
                    theme.foreground(),
                    (area.width() - 4.0 * margin).max(margin),
                )
                .size();
        })
        .drop_without_applying_deltas();
        let _ = pane;
        size
    }

    /// A console with room to spare, placed away from the window's origin so an assertion cannot
    /// read the window's top for the console's.
    fn a_roomy_console(metrics: &CellMetrics) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(41.0, metrics.height() * 1.5),
            egui::vec2(metrics.width() * 80.0, metrics.height() * 40.0),
        )
    }

    #[test]
    fn the_silence_is_stroked_filled_and_drawn_where_its_box_is() {
        // **The box existing is not the notice being painted**, and the mutation says so: deleting
        // the whole painting call from the console's pass left every other assertion about this
        // notice green, because they all ask the function that measures it. What follows reads the
        // shapes the frame actually queued.
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("yes", 0);
        make_it_go_quiet(&mut pane);
        let area = a_roomy_console(&metrics);

        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut discoverer = fonts::Discoverer::default();
        let mut box_of_it = None;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("the console"),
            ));
            box_of_it =
                painter::notices::the_boxes(&painter, &pane, &theme, &metrics, area).silence;
            painter::paint_a_console(
                &painter,
                &pane,
                &theme,
                &metrics,
                area,
                &mut discoverer,
                painter::CursorInk::Filled,
            );
        });
        let box_of_it = box_of_it.expect("that console has something to say");
        let mut output = output;
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();

        let stroked = shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Rect(shape) => {
                shape.rect == box_of_it
                    && shape.stroke.color == THE_FOCUS_COLOUR
                    && shape.stroke.width == 1.0
            }
            _ => false,
        });
        assert!(
            stroked,
            "the notice's border was never queued at {box_of_it:?}"
        );

        // **Filled as well as stroked, and nothing else asserted the fill.** Without it the
        // sentence is painted straight over the program's own text, unreadable: a border around
        // transparency is not a box.
        let filled = shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::Rect(shape) => {
                shape.rect == box_of_it && shape.fill == THE_PANEL_BACKGROUND
            }
            _ => false,
        });
        assert!(filled, "the notice's box was stroked but never filled");

        // The sentence is drawn, and clipped to the box rather than to the console: a clip that
        // ends lower paints the words over the half the ceiling exists to keep clear, and one that
        // covers nothing leaves the shape queued and invisible.
        let text = shapes
            .iter()
            .find(|clipped| matches!(clipped.shape, egui::Shape::Text(_)))
            .expect("the sentence is drawn");
        assert!(
            text.clip_rect.max.y <= box_of_it.max.y + 1.0
                && text.clip_rect.contains_rect(egui::Rect::from_min_size(
                    box_of_it.left_top() + egui::vec2(metrics.width(), metrics.width()),
                    box_of_it.size() - egui::vec2(2.0, 2.0) * metrics.width(),
                )),
            "the sentence is clipped to {:?} against a box of {box_of_it:?}: what the clip does not \
             cover is not painted at all, and what it covers past the box is painted over the \
             console's bottom half with nothing behind it",
            text.clip_rect
        );
    }

    #[test]
    fn a_click_on_the_silence_takes_it_off_the_glass_through_the_mouse_pass() {
        // AC9 through the door a person's hand comes in by, and not through the accessor: what the
        // press has to do is find the box in the same geometry the painter drew it with, in a pass
        // whose first line skips every console with nothing to say.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with("yes", "sleep 30");
        make_it_go_quiet(window.active_mut().panes_mut()[0]);
        let theme = window.theme;
        let notice = the_silence_box_on(window.active().panes()[0], &theme, &metrics, rects[0])
            .expect("the console that went quiet has nothing to say");

        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            click_at(notice.center()),
        );

        assert!(
            window.active().panes()[0]
                .pty_output
                .the_silence_was_dismissed(),
            "a click on the sentence left it on the glass"
        );
        assert!(
            the_silence_box_on(window.active().panes()[0], &theme, &metrics, rects[0]).is_none(),
            "the box is still there to swallow the next press, so the last output cannot be marked"
        );
    }
crate::windows_unix_omission!(
    the_silence_stands_under_the_held_paste_and_gives_way_to_it,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn the_silence_stands_under_the_held_paste_and_gives_way_to_it() {
        // The precedence, and the reason it is not alphabetical: the paste's notice is a question
        // with bytes waiting on an answer, and this one is a statement about something already
        // over.
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("yes", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        make_it_go_quiet(&mut pane);
        let area = a_roomy_console(&metrics);

        let paste = the_notice_box(&pane, &theme, &metrics, area);
        let silence = the_silence_box_on(&pane, &theme, &metrics, area)
            .expect("a wide console has room for both notices");

        assert!(
            silence.top() >= paste.bottom(),
            "the silence's box starts at {:?} and the paste's ends at {:?}: they overlap, so one of \
             the two sentences is drawn over the other",
            silence.top(),
            paste.bottom()
        );
    }
#[cfg(unix)]
#[test]
    fn both_notices_together_stay_out_of_the_bottom_half_of_their_console() {
        // AC6 over the pair, which is the case the ceiling was asked for: the bottom half is where a
        // console's last output is, it is the first thing somebody marks, and marking begins with a
        // bare press.
        //
        // **It walks the heights instead of picking one**, and that is not thoroughness for its own
        // sake: on a console with room to spare the pair fits under the half whatever the bound
        // says, so a single roomy shape asserts nothing about it. The walk has to see the bound
        // both refuse and allow, and it asserts that it did — otherwise a broken ceiling passes on
        // the shapes where it was never consulted.
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("yes", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        make_it_go_quiet(&mut pane);
        let mut refused = 0;
        let mut allowed = 0;

        // To seventy rows and not forty: the sentence now stands under the button's place as well as
        // the question, so the first height with room for it is taller than it was.
        for rows in 4..=70 {
            let area = egui::Rect::from_min_size(
                egui::pos2(41.0, metrics.height() * 1.5),
                egui::vec2(metrics.width() * 40.0, metrics.height() * rows as f32),
            );
            let the_middle = area.top() + area.height() * 0.5;
            match the_silence_box_on(&pane, &theme, &metrics, area) {
                None => refused += 1,
                Some(silence) => {
                    allowed += 1;
                    // **Whole, and asserted on the box that came back rather than on the clause
                    // that used to say so.** A review measured that clause dead — the ceiling
                    // refuses every box the console had to clip — so what is pinned here is the
                    // promise and not whichever line enforces it.
                    let sentence = the_sentence_laid_out_over(&pane, &theme, &metrics, area);
                    assert!(
                        silence.height() - 2.0 * metrics.width() >= sentence.y - 1.0,
                        "over a console of {rows} rows the box is {silence:?} and its sentence lays \
                         out {sentence:?}: it is shown cut, and there is no key in it to protect"
                    );
                    assert!(
                        silence.bottom() <= the_middle + 1.0,
                        "over a console of {rows} rows the silence's box ends at {:?}, past the \
                         middle at {the_middle:?}: the bottom half a person marks and copies in is \
                         not theirs any more",
                        silence.bottom()
                    );
                    let paste = the_notice_box(&pane, &theme, &metrics, area);
                    assert!(
                        paste.bottom() <= the_middle + 1.0 && silence.top() >= paste.bottom(),
                        "over a console of {rows} rows the two boxes are {:?} and {:?}: they \
                         overlap, or one of them is past the middle at {the_middle:?}",
                        paste,
                        silence
                    );
                }
            }
        }

        assert!(
            refused > 0 && allowed > 0,
            "the bound was consulted on none of the shapes walked: {refused} refused and {allowed} \
             allowed, so this test would pass with the ceiling removed"
        );
    }
crate::windows_unix_omission!(
    a_console_with_room_for_only_one_notice_keeps_the_question_and_drops_the_silence,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn a_console_with_room_for_only_one_notice_keeps_the_question_and_drops_the_silence() {
        // AC7's precedence, and the shape is measured rather than picked: the console is as tall as
        // the paste's own notice needs plus what the ceiling costs, so the second one cannot fit.
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("yes", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        make_it_go_quiet(&mut pane);
        let narrow = egui::Rect::from_min_size(
            egui::pos2(41.0, metrics.height() * 1.5),
            egui::vec2(metrics.width() * 20.0, metrics.height() * 40.0),
        );

        // What the question holds is the question and the button's place under it, so that is what
        // has to reach the ceiling for this shape to leave no room.
        let (_, send, _) = the_notices_on(&pane, &theme, &metrics, narrow);
        let paste = send.expect("a console this size has room for the button");

        assert!(
            paste.bottom() > narrow.top() + narrow.height() * 0.5 - metrics.height(),
            "the question and its button end at {:?}, well short of the ceiling, so there is room \
             for the second notice and this shape says nothing about the precedence",
            paste.bottom()
        );
        assert!(
            the_silence_box_on(&pane, &theme, &metrics, narrow).is_none(),
            "the silence was drawn where only the question fits, so one of the two is showing half \
             a sentence"
        );
    }

    #[test]
    fn a_console_too_short_for_the_whole_sentence_says_nothing_at_all() {
        // AC8 for this notice. Whole or not at all: there is no key to press here, so nothing in a
        // clipped sentence is worth protecting, and a box that shows half a statement is worse than
        // a console that says nothing.
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("yes", 0);
        make_it_go_quiet(&mut pane);
        let three_rows = egui::Rect::from_min_size(
            egui::pos2(41.0, metrics.height() * 1.5),
            egui::vec2(metrics.width() * 20.0, metrics.height() * 3.0),
        );

        assert!(
            the_silence_box_on(&pane, &theme, &metrics, three_rows).is_none(),
            "a console of three rows was given a box for a sentence that does not fit in it"
        );
    }

    #[cfg(unix)]
#[test]
    fn a_console_that_went_quiet_puts_no_sentence_of_ours_in_its_grid() {
        // AC10. The grid is what the program wrote, and a sentence of the application's put there
        // would survive in the scrollback as if the program had written it.
        //
        // **The console has to have something in its grid first, and a review measured why.** The
        // way this test makes a console go quiet takes its bridge away, so nothing can ever be
        // pumped afterwards: the first version of this asserted three absences over an **empty
        // string**, which every mutation in the world passes. So the program's own output goes in
        // before the console goes quiet, and the words looked for come from the sentence itself.
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        // **It writes and keeps writing**, and both halves matter: the ending is recorded once, so a
        // program that exits first records an ordinary ending and there would be nothing to say
        // about it — and the reading only finds out its listener is gone when it next delivers.
        let mut pane = pane_with("yes", 0);
        let deadline = Instant::now() + Duration::from_secs(5);
        while pane.screen.to_text().trim().is_empty() {
            assert!(
                Instant::now() < deadline,
                "the program never wrote anything"
            );
            pane.pump(Default::default());
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !pane.screen.to_text().trim().is_empty(),
            "the program's own output never reached the grid, so this test would be asserting \
             absences over an empty screen"
        );
        make_it_go_quiet(&mut pane);
        let area = a_roomy_console(&metrics);
        assert!(
            the_silence_box_on(&pane, &theme, &metrics, area).is_some(),
            "the console has nothing to say, so nothing could have leaked into its grid either"
        );

        // The sentence is drawn over the console — that is what this asks about — and the pump runs
        // afterwards, which is the frame a leak would show up in.
        let _ = the_sentence_over(&pane, &theme, &metrics, area);
        pane.pump(Default::default());

        let on_screen = pane.screen.to_text();
        for word in painter::notices::what_it_says(false)
            .split_whitespace()
            .take(6)
        {
            let word = word.trim_end_matches(['.', ',']);
            assert!(
                !on_screen.contains(word),
                "«{word}» of the application's own sentence reached the grid, so it is in the \
                 scrollback as if the program had written it. The screen holds: {on_screen:?}"
            );
        }
    }

    #[cfg(unix)]
#[test]
    fn dismissing_the_silence_takes_the_sentence_away_and_undoes_nothing() {
        // AC9. What the dismissal is for: the box is what swallows a press, so taking it away is
        // what gives the console's last output back to whoever wants to mark and copy it.
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("yes", 0);
        make_it_go_quiet(&mut pane);
        let area = a_roomy_console(&metrics);
        let how_it_ended = pane.pty.ending();
        assert!(
            the_silence_box_on(&pane, &theme, &metrics, area).is_some(),
            "the premise failed: the quiet console never painted its sentence"
        );

        pane.pty_output.dismiss_the_silence(pane.pty.ending());

        assert!(
            the_silence_box_on(&pane, &theme, &metrics, area).is_none(),
            "the sentence is still on the glass after being dismissed"
        );
        assert_eq!(
            pane.pty.ending(),
            how_it_ended,
            "dismissing the sentence changed how the reading is recorded as having ended"
        );
        // **What is not asserted here, and why**: «the window still will not close itself over this
        // console» cannot fail — closing asks for an ordinary ending and this console has none, so
        // the answer is the same before and after whatever the dismissal does. A review measured
        // it. The claim that can fail is the one above: the ending is untouched.
    }

    /// **A press on the panel's strip does not answer the question**, because the box is clamped to
    /// the console and the strip is not the console.
    ///
    /// **It pins the clamp, and not the frame a panel first appears on** — an earlier version of
    /// this docblock claimed the second and a mutation proved it did not: the guard it credited read
    /// the same unrefreshed state it was written for. That frame is accepted with its reason, in
    /// `answer_a_clicked_question`.
crate::windows_unix_omission!(
    a_press_on_the_panel_beside_it_does_not_answer_the_question,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn a_press_on_the_panel_beside_it_does_not_answer_the_question() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "a_press_on_the_panel_beside_it_does_not_answer_the_question",
        );
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        // The strip the panel occupies: inside the pane, outside the console. Asserted, so that a
        // pane whose panel stopped taking columns turns this test red instead of vacuous.
        let console = window.active_mut().panes_mut()[0].console_within(
            rects[0],
            &metrics,
            SystemTime::now(),
            header::Headers::PAINTED,
        );
        let on_the_panel = egui::pos2(
            (console.right() + rects[0].right()) / 2.0,
            rects[0].top() + metrics.height(),
        );
        assert!(
            rects[0].contains(on_the_panel) && !console.contains(on_the_panel),
            "the point this test presses is not on the panel's strip: pane {:?}, console \
             {console:?}, point {on_the_panel:?}",
            rects[0]
        );

        deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(on_the_panel));

        assert!(
            window.active().panes()[0].held.is_some(),
            "a press on the panel beside the console threw its held paste away"
        );
    }

    /// **Answering a question takes one press out of the queue and leaves the rest**, and the event
    /// that proves it is a **release**.
    ///
    /// **This is the adversarial case a review named and reasoning alone was closing.** Replacing the
    /// removal of that one press with clearing the queue left the whole suite green: nothing carried
    /// a second event, so there was nothing to lose. Here the batch carries the release of a press
    /// already live for a program *and* the press that answers a notice. Lose the release and the
    /// program is left believing the button is still down — for ever, per the routing's own comment,
    /// and it then steals the next release.
crate::windows_unix_omission!(
    answering_a_question_does_not_swallow_the_rest_of_the_queue,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn answering_a_question_does_not_swallow_the_rest_of_the_queue() {
        let ctx = egui::Context::default();
        let metrics = with_metrics(|metrics| metrics.clone());
        // **Wider panes than the other tests use, and that is not a convenience.** With the narrow
        // console `two_wide_panes` gives — narrower still once a panel takes its columns — the
        // notice's box fills the console top to bottom, so there is nowhere to press that is not the
        // notice. Measured while writing this: console 168.6 pt wide, box 8.4 to 300.0 tall. This
        // test needs a press that reaches the program, so it needs a console with room beside the
        // question.
        let rects = [
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0)),
            egui::Rect::from_min_size(egui::pos2(910.0, 0.0), egui::vec2(900.0, 600.0)),
        ];
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "answering_a_question_does_not_swallow_the_rest_of_the_queue",
        );
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        let theme = window.theme;
        let console = window.active_mut().panes_mut()[0].console_within(
            rects[0],
            &metrics,
            SystemTime::now(),
            header::Headers::PAINTED,
        );
        let notice = the_notice_box(window.active().panes()[0], &theme, &metrics, console);
        let on_the_notice = notice.center();
        let elsewhere =
            console.left_bottom() + egui::vec2(metrics.width() * 2.5, -metrics.height());
        // Asserted, because the whole test turns on that press reaching the program: inside the
        // notice it would answer the question instead, which is how the first version of this test
        // measured nothing at all.
        assert!(
            !notice.contains(elsewhere) && console.contains(elsewhere),
            "the press meant for the program is on the notice: console {console:?}, notice \
             {notice:?}, point {elsewhere:?}"
        );

        // **The press only, with no release**, on a frame of its own: it has to still be live when
        // the batch below arrives. Sending a whole click here would put its own release on screen and
        // the assertion at the end would find *that* one — which is how the first version of this
        // test stayed green with the queue being cleared.
        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            vec![
                egui::Event::PointerMoved(elsewhere),
                egui::Event::PointerButton {
                    pos: elsewhere,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );
        let its_cell = window.active().panes()[0]
            .cell_under(console, elsewhere, &metrics)
            .expect("the press landed on a cell of the console");
        wait_for_the_pane_to_say(
            &mut window,
            0,
            &format!("^[[<0;{};{}M", its_cell.col + 1, its_cell.row + 1),
        );

        // And now one batch with **both**: that button coming up, and a press on the notice.
        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            vec![
                egui::Event::PointerButton {
                    pos: elsewhere,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerMoved(on_the_notice),
                egui::Event::PointerButton {
                    pos: on_the_notice,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ],
        );

        assert!(
            window.active().panes()[0].held.is_none(),
            "the press on the notice did not answer the question"
        );
        // The release is reported with a lower-case ending, which is the only thing that separates
        // it from the press in this encoding.
        wait_for_the_pane_to_say(
            &mut window,
            0,
            &format!("^[[<0;{};{}m", its_cell.col + 1, its_cell.row + 1),
        );
    }

    /// A console whose program asked for the mouse and holds a paste, beside room to press that is
    /// not the notice — the staging of the test above — with where the notice is and where the
    /// program can be pressed.
    fn a_question_beside_a_program_that_asks_for_the_mouse(
        rects: &[egui::Rect],
        metrics: &CellMetrics,
        case: &str,
    ) -> Option<(Window, egui::Rect, egui::Rect, egui::Pos2)> {
        let mut window = a_pane_that_asks_for_the_mouse(rects, metrics, case);
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return None;
        }
        let theme = window.theme;
        let console = window.active_mut().panes_mut()[0].console_within(
            rects[0],
            metrics,
            SystemTime::now(),
            header::Headers::PAINTED,
        );
        let notice = the_notice_box(window.active().panes()[0], &theme, metrics, console);
        let elsewhere =
            console.left_bottom() + egui::vec2(metrics.width() * 2.5, -metrics.height());
        // **Clear of every box the mouse pass answers, not only the question**: a press on the
        // button that sends the paste would be answered too, and the program would never see it.
        let (question, send, silence) =
            the_notices_on(window.active().panes()[0], &theme, metrics, console);
        assert!(
            [question, send, silence]
                .into_iter()
                .flatten()
                .all(|answered| !answered.contains(elsewhere))
                && console.contains(elsewhere),
            "the press meant for the program is on a notice: console {console:?}, question \
             {question:?}, send {send:?}, silence {silence:?}, point {elsewhere:?}"
        );
        Some((window, console, notice, elsewhere))
    }

    /// The report a press on that point sends, as `cat -v` shows it.
    fn the_press_report_at(
        window: &Window,
        console: egui::Rect,
        at: egui::Pos2,
        metrics: &CellMetrics,
    ) -> String {
        let cell = window.active().panes()[0]
            .cell_under(console, at, metrics)
            .expect("the point is on a cell of the console");
        format!("^[[<0;{};{}M", cell.col + 1, cell.row + 1)
    }

    #[test]
    fn a_press_that_answers_the_question_is_not_handed_to_the_program_on_a_repeated_pass() {
        // **The answer takes the press out of the batch; a mouse pass that read the same batch
        // again would put it back.** egui 0.36 hands a frame's events to its first pass only (see
        // `egui_hands_a_frame_s_events_to_its_first_pass_only`), so this is the case the barrier
        // is kept against, not one it meets today. The notice pass runs on every mouse pass, before
        // the barrier: on a second one the question is gone, the press is collected again, and
        // nothing but the barrier stops it being routed to a program that asked for the mouse. The
        // barrier's sibling tests all route a press on their first pass, so a barrier armed only
        // when the answered batch still carries a press passes them; this one's first pass routes
        // nothing.
        let ctx = egui::Context::default();
        let metrics = with_metrics(|metrics| metrics.clone());
        let rects = [
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0)),
            egui::Rect::from_min_size(egui::pos2(910.0, 0.0), egui::vec2(900.0, 600.0)),
        ];
        let Some((mut window, console, notice, elsewhere)) =
            a_question_beside_a_program_that_asks_for_the_mouse(
                &rects,
                &metrics,
                "a_press_that_answers_the_question_is_not_handed_to_the_program_on_a_repeated_pass",
            )
        else {
            return;
        };
        let on_the_notice = notice.center();

        ctx.run_ui(egui::RawInput::default(), |ui| {
            window.handle_the_mouse(ui, &rects, &metrics);
        })
        .drop_without_applying_deltas();
        ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(on_the_notice),
                    egui::Event::PointerButton {
                        pos: on_the_notice,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                // Two calls inside one egui pass stand for two passes of one frame because the
                // barrier reads the frame number, which both share; egui's own repeated pass
                // would carry no events to repeat.
                window.handle_the_mouse(ui, &rects, &metrics);
                window.handle_the_mouse(ui, &rects, &metrics);
            },
        )
        .drop_without_applying_deltas();
        assert!(
            window.active().panes()[0].held.is_none(),
            "the press on the notice did not answer the question"
        );

        // **A sentinel, so the absence below means something**: once its report is on screen, any
        // report the answered press produced would have arrived before it.
        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            vec![
                egui::Event::PointerButton {
                    pos: on_the_notice,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerMoved(elsewhere),
                egui::Event::PointerButton {
                    pos: elsewhere,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        let sentinel = the_press_report_at(&window, console, elsewhere, &metrics);
        wait_for_the_pane_to_say(&mut window, 0, &sentinel);

        let answered = the_press_report_at(&window, console, on_the_notice, &metrics);
        let on_screen = window.active().panes()[0]
            .screen
            .to_text()
            .replace('\n', "");
        assert_eq!(
            on_screen.matches(answered.as_str()).count(),
            0,
            "the press that answered the question reached the program — on the second mouse pass \
             if the barrier let it through, or on the first if the answer left it in the batch; \
             on screen there is: {on_screen:?}"
        );
    }

    #[cfg(unix)]
#[test]
    fn a_drag_from_a_press_that_answered_the_question_marks_text() {
        // **The drag latch is read on a later frame than the press that set it.** A press that
        // reached the program leaves it saying «the program's»; a press that answers the question
        // has to put it back, or the drag that begins at the answered press is decided by the one
        // before and goes to the program — which never received a press to go with it.
        let ctx = egui::Context::default();
        let metrics = with_metrics(|metrics| metrics.clone());
        let rects = [
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 600.0)),
            egui::Rect::from_min_size(egui::pos2(910.0, 0.0), egui::vec2(900.0, 600.0)),
        ];
        let Some((mut window, console, notice, elsewhere)) =
            a_question_beside_a_program_that_asks_for_the_mouse(
                &rects,
                &metrics,
                "a_drag_from_a_press_that_answered_the_question_marks_text",
            )
        else {
            return;
        };

        // The premise, proved by the program: the press before this gesture was its.
        deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(elsewhere));
        let before = the_press_report_at(&window, console, elsewhere, &metrics);
        wait_for_the_pane_to_say(&mut window, 0, &before);

        let from = notice.center();
        let to = from + egui::vec2(metrics.width() * 6.0, metrics.height() * 2.0);
        drag_with(
            &mut window,
            &ctx,
            &rects,
            egui::PointerButton::Primary,
            from,
            to,
            egui::Modifiers::NONE,
            egui::Modifiers::NONE,
        );

        assert!(
            window.active().panes()[0].held.is_none(),
            "the press on the notice did not answer the question"
        );
        assert!(
            window.active().panes()[0].screen.has_selection(),
            "the drag from the press that answered the question marked nothing: it was decided by \
             the press before it and handed to the program"
        );
    }

    /// **Only the primary button answers, and nothing pinned that.** Accepting any button leaves the
    /// whole suite green, and the middle one is X11's paste — this file's own comment calls that
    /// button «not theoretical». A middle press on a notice would have destroyed the held paste.
#[cfg(unix)]
#[test]
    fn only_the_primary_button_answers_the_question() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with("sleep 30", "sleep 30");
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        let theme = window.theme;
        let at = the_notice_box(window.active().panes()[0], &theme, &metrics, rects[0]).center();

        for button in [egui::PointerButton::Secondary, egui::PointerButton::Middle] {
            deliver_to_the_mouse(
                &mut window,
                &ctx,
                &rects,
                &metrics,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button,
                        pressed: true,
                        modifiers: egui::Modifiers::default(),
                    },
                ],
            );
            assert!(
                window.active().panes()[0].held.is_some(),
                "a press of {button:?} on the notice threw the held paste away"
            );
        }
    }

    /// **One click answers one question, not every standing one.** Every other test of this fix holds
    /// a paste on a single console, so two questions never stand at once and a loop that cleared them
    /// all would pass — measured.
#[cfg(unix)]
#[test]
    fn a_click_answers_only_the_question_it_landed_on() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with("sleep 30", "sleep 30");
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        assert!(
            hold_a_paste_on(window.active_mut().panes_mut()[1]),
            "the premise failed: the second console did not hold its paste"
        );
        let theme = window.theme;
        let at = the_notice_box(window.active().panes()[0], &theme, &metrics, rects[0]).center();

        deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(at));

        assert!(
            window.active().panes()[0].held.is_none(),
            "the question the click landed on is still standing"
        );
        assert!(
            window.active().panes()[1].held.is_some(),
            "the click answered the question of a console it never touched"
        );
    }

    /// **A console too narrow for the notice has no box to press either**, and the width half of that
    /// floor was untested while the height half was.
    ///
    /// **The console had to grow very tall for that to stay true, and until it did this test was
    /// passing for the height's reason.** At three cells every word breaks, so the line saying how
    /// to get out lays out taller than most consoles are — and the floor that asks for the whole
    /// answer refuses a console of forty rows before the width is ever consulted. Measured: with the
    /// width floor **deleted altogether** the suite stayed green. So the height is now derived from
    /// the answer's own measurement instead of picked, and the width is the only thing left to
    /// refuse the box.
#[cfg(unix)]
#[test]
    fn a_console_too_narrow_for_the_notice_has_no_box_to_press() {
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = theme::Theme::default();
        let mut pane = pane_with("sleep 30", 0);
        if !hold_a_paste_on(&mut pane) {
            return;
        }
        // Only the width reaches the wrapping, so the answer can be measured before the console it
        // will be shown on exists.
        let three_cells = metrics.width() * 3.0;
        let probe =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(three_cells, metrics.height()));
        let answer = every_way_to_answer()
            .map(|how| the_answer_laid_out(&theme, &metrics, probe, how))
            .into_iter()
            .fold(0.0_f32, f32::max);
        let sliver = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(three_cells, 2.0 * (answer + 4.0 * metrics.height())),
        );

        let ctx = egui::Context::default();
        ctx.run_ui(egui::RawInput::default(), |_| {})
            .drop_without_applying_deltas();
        let mut found = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            found = painter::notices::the_question_box_with_no_button(
                ui.painter(),
                &pane,
                &theme,
                &metrics,
                sliver,
            );
        })
        .drop_without_applying_deltas();

        // **The guard mirrors the ceiling's arithmetic on purpose**: it claims nothing about the
        // geometry — the neighbouring band test is where that is asserted — it says that **height
        // is not the reason** this console has no box, which is the whole point of the assertion
        // under it.
        assert!(
            sliver.height() * 0.5 - 3.0 * metrics.width() > answer,
            "the ceiling leaves {:?} on this console and the way out needs {answer:?}, so it is the \
             HEIGHT refusing the box and the width floor is being credited for it: the console has \
             to be built taller than the answer its own width wraps to",
            sliver.height() * 0.5 - 3.0 * metrics.width()
        );
        assert!(
            found.is_none(),
            "a console three cells wide offered a box to press: {found:?}"
        );
    }

    /// **The press is judged where the button went down, not where the pointer ended up.** egui's
    /// «interact position» is the *latest* one in the batch, and a batch that carries the press and
    /// then a move — a flicked click, or any frame that batches, and this window can freeze for a
    /// tenth of a second — would test a point the person never pressed: it would let the real press
    /// through to the program and could answer a question nobody aimed at.
    ///
    /// The move that follows the press here is what no other test of this fix delivers.
#[cfg(unix)]
#[test]
    fn the_press_is_judged_where_the_button_went_down() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with("sleep 30", "sleep 30");
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }
        let theme = window.theme;
        let notice = the_notice_box(window.active().panes()[0], &theme, &metrics, rects[0]);
        let on_the_notice = notice.center();
        let far_away = rects[1].center();

        deliver_to_the_mouse(
            &mut window,
            &ctx,
            &rects,
            &metrics,
            vec![
                egui::Event::PointerMoved(on_the_notice),
                egui::Event::PointerButton {
                    pos: on_the_notice,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                // The pointer leaves the notice **in the same batch**, which is what makes the
                // frame's «latest position» the wrong thing to read.
                egui::Event::PointerMoved(far_away),
            ],
        );

        assert!(
            window.active().panes()[0].held.is_none(),
            "the press landed on the notice and was judged somewhere else, so the question stands \
             and the press went to the console instead"
        );
    }

    /// **`Ctrl+Shift` on a notice must not answer it**, because that chord moves the console
    /// (**D20**): letting it through would move a pane **and destroy the held paste in silence**,
    /// which is the precise failure this whole feature exists to stop. The bare escape hatch that
    /// takes the selection back (**D23**) must not answer either.
#[cfg(unix)]
#[test]
    fn a_chorded_press_on_the_notice_does_not_destroy_the_paste() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let theme = window_with("sleep 30", "sleep 30").theme;

        for chord in [MOVE_CHORD, egui::Modifiers::SHIFT] {
            let mut window = window_with("sleep 30", "sleep 30");
            if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
                return;
            }
            let notice = the_notice_box(window.active().panes()[0], &theme, &metrics, rects[0]);
            let at = notice.center();

            deliver_to_the_mouse(
                &mut window,
                &ctx,
                &rects,
                &metrics,
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: chord,
                    },
                ],
            );

            assert!(
                window.active().panes()[0].held.is_some(),
                "a press with {chord:?} on the notice threw the held paste away without a word"
            );
        }
    }

    /// **The half of the click that no assertion about the notice can see**: it is answered and
    /// **does not reach the program underneath**. It needs a console whose program really asked for
    /// the mouse — with a `sleep 30` inside, no report would be sent either way, so every assertion
    /// would hold with the suppression removed. Measured: that mutation left the other tests green.
    ///
    /// **The absence is clocked by an arrival, not by a timer.** An earlier version waited a fixed
    /// half second, which on a slower pseudoterminal would pass over a live defect; the control's own
    /// wait is five seconds. Now the two clicks are aimed at **different cells**, so the report that
    /// does arrive proves the round trip works *and* names its own cell — and the cell of the click
    /// that answered the question is asserted absent from a screen that already carries the other.
crate::windows_unix_omission!(
    a_click_on_the_notice_does_not_reach_the_program_that_asked_for_the_mouse,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn a_click_on_the_notice_does_not_reach_the_program_that_asked_for_the_mouse() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "a_click_on_the_notice_does_not_reach_the_program_that_asked_for_the_mouse",
        );
        if !hold_a_paste_on(window.active_mut().panes_mut()[0]) {
            return;
        }

        // The notice goes over the **console**, which is not the whole pane here: this one has a
        // panel published beside it.
        let theme = window.theme;
        let console = window.active_mut().panes_mut()[0].console_within(
            rects[0],
            &metrics,
            SystemTime::now(),
            header::Headers::PAINTED,
        );
        let notice = the_notice_box(window.active().panes()[0], &theme, &metrics, console);
        let on_the_notice = notice.center();
        // A cell of the console that is **not** the notice's, so the two reports would be told apart
        // by their coordinates rather than by counting.
        let elsewhere =
            console.left_bottom() + egui::vec2(metrics.width() * 2.5, -metrics.height());
        let its_cell = window.active().panes()[0]
            .cell_under(console, on_the_notice, &metrics)
            .expect("the notice sits over a cell of the console");
        let the_other_cell = window.active().panes()[0]
            .cell_under(console, elsewhere, &metrics)
            .expect("the control's point sits over a cell of the console");
        assert_ne!(
            (its_cell.row, its_cell.col),
            (the_other_cell.row, the_other_cell.col),
            "both clicks land on the same cell, so their reports cannot be told apart"
        );
        // What the answered click's report WOULD look like, if it had been sent.
        let would_be = format!("^[[<0;{};{}M", its_cell.col + 1, its_cell.row + 1);

        deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(on_the_notice));
        assert!(
            window.active().panes()[0].held.is_none(),
            "the click did not answer the question it was aimed at"
        );

        // The control, and it is what clocks the assertion below: a click elsewhere on the same
        // console does reach the program. When its report is on screen, the round trip has
        // demonstrably happened — so the other one's absence is an absence and not a wait.
        deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(elsewhere));
        let control = format!(
            "^[[<0;{};{}M",
            the_other_cell.col + 1,
            the_other_cell.row + 1
        );
        wait_for_the_pane_to_say(&mut window, 0, &control);

        assert!(
            !window.active().panes()[0]
                .screen
                .to_text()
                .contains(&would_be),
            "the click that answered the notice was ALSO reported to the program: looked for \
             {would_be:?} and the screen says {:?}",
            window.active().panes()[0].screen.to_text()
        );
    }
crate::windows_unix_omission!(
    a_pasted_line_that_does_not_fit_reaches_the_program_as_nothing_at_all,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn a_pasted_line_that_does_not_fit_reaches_the_program_as_nothing_at_all() {
        // **The wiring, and without this test there is none.** Every other paste test here uses
        // short text, so reverting `send_keys` to the composition it had before this fix restores the
        // bug in full and nothing goes red: all the holding coverage calls the module directly. What
        // this pins is the thing the user meets — the bytes do not reach the program — and not that
        // some field was set.
        let Some(boundary) = keyboard::the_longest_line_a_reader_keeps() else {
            return;
        };
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        let mut modifiers = egui::Modifiers::NONE;

        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(with_shift()),
                egui::Event::Paste("x".repeat(boundary + 1)),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );

        assert!(
            pane.keystrokes.is_empty(),
            "an over-long line was handed to the program anyway: {} bytes",
            pane.keystrokes.len()
        );
        assert!(
            pane.held.is_some(),
            "nothing was written and nothing is being asked about either: the paste just vanished"
        );
    }

    #[test]
    fn with_shift_the_text_is_pasted_and_without_it_the_control_byte_goes() {
        // The two shortcuts arrive as the SAME event and with no modifiers: the desktop resolves
        // them first and swallows the key. Telling them apart here is what makes Ctrl+V keep
        // reaching the program inside, which is the one that decides whether for it that means
        // paste (Claude) or visual block (vim).
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        // The window keeps this between frames; here the variable does it, which is the same thing.
        let mut modifiers = egui::Modifiers::NONE;

        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(with_shift()),
                egui::Event::Paste("hello".to_owned()),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(pane.keystrokes, b"hello", "with Shift it did not paste");

        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                egui::Event::Paste("hello".to_owned()),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(
            pane.keystrokes, b"\x16",
            "without Shift the control byte has to arrive, not the text"
        );
    }

    #[test]
    fn the_mode_the_program_asks_for_reaches_all_the_way_to_what_is_pasted() {
        // **It is the only wire in this work that was not tied down**, and the lesson is the same
        // one the design invokes for the modifiers: testing the pure function proves that it
        // composes the mark well, not that anybody hands it the real mode. Hard-coding `false` in
        // the loop left the 298 of them green and pasting several lines went back to running them
        // one by one.
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        let mut modifiers = egui::Modifiers::NONE;

        // Without the program asking for it, the text goes bare.
        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(with_shift()),
                egui::Event::Paste("ls\n".to_owned()),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(pane.keystrokes, b"ls\r", "it arrived marked without asking");

        // And the moment it asks, the mark appears — without touching anything but the screen.
        pane.screen.advance(b"\x1b[?2004h");
        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(with_shift()),
                egui::Event::Paste("ls\n".to_owned()),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(
            pane.keystrokes, b"\x1b[200~ls\n\x1b[201~",
            "the program asked for the marked paste and the loop never found out"
        );
    }

    #[test]
    fn the_modifiers_are_read_from_the_moment_of_the_paste_and_not_the_end_of_the_batch() {
        // **This is the test that separates the good mechanism from the one that looked like it.**
        // The field egui publishes —`InputState::modifiers`— is a single one per pass: it walks the
        // batch and keeps the last change notice. Reading it would describe how the keys were at
        // the END, not when the paste arrived.
        //
        // The two sequences below have the same set of events in a different order, and with that
        // field they would give the swapped result in both.
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        // The window keeps this between frames; here the variable does it, which is the same thing.
        let mut modifiers = egui::Modifiers::NONE;

        // The paste happens and the chord is released before the frame is collected: the end of the
        // batch says «none», but when the paste arrived they were down.
        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(with_shift()),
                egui::Event::Paste("good".to_owned()),
                egui::Event::ModifiersChanged(egui::Modifiers::NONE),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(
            pane.keystrokes, b"good",
            "releasing the chord after pasting stopped the paste: the end of the batch was read"
        );

        // And the other way round: the paste happens without Shift and it is pressed afterwards.
        // The end of the batch says it was down.
        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                egui::Event::Paste("bad".to_owned()),
                egui::Event::ModifiersChanged(with_shift()),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(
            pane.keystrokes, b"\x16",
            "pressing Shift after the paste turned it into a paste: the end of the batch was read"
        );
    }

    #[test]
    fn leaving_the_window_clears_the_chord_so_it_does_not_stay_stuck() {
        // **The scenario is a desktop shortcut, and it ends up dumping the clipboard.** With
        // Ctrl+Shift held down you switch windows with a global combination; the keys are released
        // outside and the notice that they were released never arrives. Without clearing the chord
        // on the way out, Shift stays down for ever and the next Ctrl+V —which the user expects to
        // enter visual block— dumps the clipboard into the program.
        //
        // It is the same safeguard egui has in its own input loop, with its reason written down
        // there; this reconstruction had forgotten it.
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        let mut modifiers = egui::Modifiers::NONE;

        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(with_shift()),
                egui::Event::WindowFocused(false),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );

        enqueue(
            &ctx,
            vec![egui::Event::Paste("you should not see me".to_owned())],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(
            pane.keystrokes, b"\x16",
            "Shift stayed stuck on the way out of the window, and the paste slipped through"
        );
    }

    #[test]
    fn a_chord_that_began_in_the_previous_frame_still_counts() {
        // Holding the keys down and releasing the V a frame later is the normal way to paste: the
        // notice that they were pressed arrived in the previous batch and is no longer in this one.
        // Without remembering how the pass closed, that paste would be read as if there were no
        // Shift.
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        // The window keeps this between frames; here the variable does it, which is the same thing.
        let mut modifiers = egui::Modifiers::NONE;

        enqueue(&ctx, vec![egui::Event::ModifiersChanged(with_shift())]);
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );

        // **Emptying the queue is what turns this into two real frames.** egui rebuilds the event
        // list in each one, so the notice that the keys were pressed is not still there on the
        // next. Without this emptying the test would pass by itself: it would read again the same
        // notice the previous pass left, and would not exercise the seed at all.
        ctx.input_mut(|input| input.events.clear());

        enqueue(&ctx, vec![egui::Event::Paste("late".to_owned())]);
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(
            pane.keystrokes, b"late",
            "the chord from the previous frame was lost"
        );
    }

    #[test]
    fn interrupting_still_interrupts_and_cutting_does_not_cut() {
        // **`Ctrl+C` is how a program has been interrupted for as long as terminals have existed**,
        // and it arrives through the same door as the request to copy: if that door swallowed both,
        // a hung program would stay hung. With Shift the request **is** to copy, and that is what
        // the `copy_at` ones prove.
        //
        // Cutting is not told apart by modifiers because **a terminal does not cut**: what is on
        // screen is not an editor to take text out of. A cut that arrives belongs to the program,
        // with Shift or without it. Since D33's second change `Ctrl+Shift+X` is not one of them — it
        // arrives as a key and closes the console — so the cut seen here with Shift held is what a
        // dedicated Cut key, or Windows' Shift+Delete, still produces.
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        // The window keeps this between frames; here the variable does it, which is the same thing.
        let mut modifiers = egui::Modifiers::NONE;

        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                egui::Event::Copy,
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert_eq!(pane.keystrokes, b"\x03", "it stopped interrupting");

        for chord in [egui::Modifiers::CTRL, with_shift()] {
            enqueue(
                &ctx,
                vec![egui::Event::ModifiersChanged(chord), egui::Event::Cut],
            );
            pane.send_keys(
                &ctx,
                Instant::now(),
                &mut modifiers,
                2,
                &keyboard::Keymap::default(),
                true,
            );
            assert_eq!(pane.keystrokes, b"\x18", "cutting changed its byte");
        }
    }

    /// **The notice a refusal hands back is what stops a publication being lost**, and this is the
    /// only place it can be asserted: the flag belongs to the pane, and a test over `PanelState`
    /// alone can only watch the value the refusal returns — a proxy that stays green with the
    /// hand-back deleted.
    #[test]
    fn a_file_published_while_the_channel_holds_the_panel_is_read_when_the_hold_ends() {
        let dir = PanelDir::create().expect("create the panel directory");
        let mut pane = pane_with("sleep 30", 0);
        pane.control = control::PaneControl::open(
            &dir,
            pane.panel_state.reader(),
            control::ReadActivity::default(),
            || {},
        );
        let mut producer =
            panel_control::Producer::of(dir.path(), dir.id()).expect("connect as a producer");

        // **Parked on a channel and not on a sleep.** A fixed sleep is the second disguise D30
        // names: on a slow machine the hold ends before the file is written, the file is then read
        // directly, and the test passes with the notice hand-back deleted — which is exactly the
        // branch it exists to cover. Here the connection cannot close until the test says so, so the
        // file is always written while the hold stands.
        let (go, parked) = std::sync::mpsc::sync_channel::<()>(0);
        let held = std::thread::spawn(move || {
            let answered = producer.publish("flow", "{\"lines\":[\"the agent\"]}");
            let _ = parked.recv();
            answered
        });
        pane.panel = Some(dir);
        let until = Instant::now() + Duration::from_secs(5);
        while pane.panel_state.held_by().is_none() && Instant::now() < until {
            pane.refresh_its_panel(&egui::Context::default());
        }
        assert!(pane.panel_state.held_by().is_some(), "nobody took the pane");

        std::fs::write(
            pane.panel
                .as_ref()
                .expect("the directory")
                .path()
                .join(panel_contract::PANEL_FILE),
            b"{\"lines\":[\"written during the hold\"]}",
        )
        .expect("publish by file");
        std::fs::File::open(
            pane.panel
                .as_ref()
                .unwrap()
                .path()
                .join(panel_contract::PANEL_FILE),
        )
        .unwrap()
        .set_modified(
            pane.panel_state
                .last_understood()
                .unwrap()
                .written_at
                .unwrap()
                + Duration::from_secs(1),
        )
        .unwrap();
        pane.to_reread.raise();
        pane.refresh_its_panel(&egui::Context::default());
        assert!(
            pane.panel_state.held_by().is_some(),
            "the hold ended before the publication was refused, so nothing was refused"
        );

        drop(go);
        let _ = held.join().expect("the producer finished");
        let until = Instant::now() + Duration::from_secs(5);
        while pane.panel_state.held_by().is_some() && Instant::now() < until {
            pane.refresh_its_panel(&egui::Context::default());
        }
        pane.refresh_its_panel(&egui::Context::default());

        assert_eq!(
            pane.panel_state
                .last_understood()
                .expect("there is a panel")
                .lines[0]
                .text,
            "written during the hold",
            "the publication made during the hold was thrown away"
        );
    }

    #[test]
    fn done_probes_the_file_after_a_watcher_notice_was_consumed() {
        let ctx = egui::Context::default();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        for by_patch in [false, true] {
            for notice_before_done in [false, true] {
                let mut pane = pane_with("sleep 30", 0);
                pane.panel = Some(PanelDir::create().unwrap());
                pane.panel_state
                    .deliver(
                        7,
                        "flow",
                        panel_contract::parse_document(r#"{"lines":["agent"]}"#).unwrap(),
                        now,
                    )
                    .unwrap();
                pane.to_reread.raise();
                pane.refresh_its_panel(&ctx);
                let path = pane
                    .panel
                    .as_ref()
                    .unwrap()
                    .path()
                    .join(panel_contract::PANEL_FILE);
                std::fs::write(&path, r#"{"lines":["later file"]}"#).unwrap();
                std::fs::File::open(&path)
                    .unwrap()
                    .set_modified(now + Duration::from_secs(2))
                    .unwrap();
                if notice_before_done {
                    pane.to_reread.raise();
                    pane.refresh_its_panel(&ctx);
                    assert_eq!(
                        pane.panel_state.last_understood().unwrap().lines[0].text,
                        "agent"
                    );
                    pane.to_reread.take();
                }
                if by_patch {
                    let ops = panel_contract::parse_ops(&serde_json::json!([
                        {"op":"set","field":"attention","value":"done"}
                    ]))
                    .unwrap();
                    pane.panel_state
                        .patch(7, &ops, now + Duration::from_secs(1))
                        .unwrap();
                } else {
                    pane.panel_state
                        .deliver(
                            7,
                            "flow",
                            panel_contract::parse_document(
                                r#"{"attention":"done","lines":["done"]}"#,
                            )
                            .unwrap(),
                            now + Duration::from_secs(1),
                        )
                        .unwrap();
                }
                pane.refresh_its_panel(&ctx);
                assert_eq!(
                    pane.panel_state.last_understood().unwrap().lines[0].text,
                    "later file"
                );
                assert_eq!(pane.panel_state.held_by(), Some("flow"));
                assert!(
                    !pane.to_reread.take(),
                    "a yielded file must not cause a per-frame probe"
                );
                assert!(!pane.panel_state.file_probe_pending());
            }
        }
    }

    /// Binds a control channel to the console called `name`, so a request can reach the window.
    fn attach_a_channel(window: &mut Window, name: usize) {
        let dir = PanelDir::create().expect("a panel directory");
        let reads = window.control.read_activity();
        let pane = window
            .all_panes_mut()
            .find(|pane| pane.name == name)
            .expect("that console exists");
        let control = control::PaneControl::open(&dir, pane.panel_state.reader(), reads, || {})
            .expect("bind the channel");
        pane.panel = Some(dir);
        pane.control = Some(control);
    }

    #[test]
    fn reads_from_two_real_panes_survive_one_closing_in_the_final_report() {
        let ctx = egui::Context::default();
        let mut window = Window::with_tabs(&ctx, 1, OpeningLayout::DEFAULT, None)
            .expect("open a production-shaped window");

        for (at, text) in ["left", "a longer right document"].into_iter().enumerate() {
            window.tabs[0].panes_mut()[at]
                .panel_state
                .deliver(
                    at as u64,
                    "test",
                    panel_contract::parse_document(&format!(r#"{{"lines":["{text}"]}}"#))
                        .expect("a panel"),
                    SystemTime::now(),
                )
                .expect("commit the panel");
        }
        let producer_for = |name| {
            let pane = window
                .all_panes()
                .find(|pane| pane.name == name)
                .expect("that pane exists");
            let dir = pane.panel.as_ref().expect("the pane has a channel");
            panel_control::Producer::of(dir.path(), dir.id()).expect("connect as a producer")
        };
        let mut left = producer_for(0);
        let mut right = producer_for(1);

        let panel_control::Answered::Document(left_document) =
            left.read_the_document().expect("read the left document")
        else {
            panic!("the left pane did not answer with its document");
        };
        let panel_control::Answered::Document(right_document) =
            right.read_the_document().expect("read the right document")
        else {
            panic!("the right pane did not answer with its document");
        };
        let largest = left_document.len().max(right_document.len());

        // **No read is sent to the left pane after it closes.** One would be answered only while that
        // connection's thread is still parked in its read — an idle one leaves within a receive
        // timeout of the close — and after that the producer gets a broken pipe, or a reset when its
        // bytes land between the timeout and the socket closing. That race is the design's, not a
        // defect: the refusal counted after a close is pinned on the counter itself, by
        // `the_window_accumulator_keeps_a_completion_and_refusal_after_pane_close`, and the one read
        // a closing connection still answers by a test beside `panel_control`'s listener.
        window
            .close_a_pane_called(0, &ctx)
            .expect("close the left pane");
        window.tabs[0].panes_mut()[0]
            .timings
            .record_outbound(std::time::Duration::from_micros(10));

        let report = report_of(&window);
        assert!(
            report.contains(&format!(
                "documents:    2 read back, 0 refused, 0 patched, 0 patches refused, {largest} bytes the largest written out"
            )),
            "the final report lost reads across a real pane close:\n{report}"
        );
    }

    #[test]
    fn a_control_requested_pane_close_retires_its_old_watcher_before_the_directory() {
        let ctx = egui::Context::default();
        let mut window = Window::with_tabs(&ctx, 1, OpeningLayout::DEFAULT, None)
            .expect("open a production-shaped window");
        let [leaving, staying]: [_; 2] = dirs_of(window.active())
            .try_into()
            .expect("two original panes");
        let retiring_watcher = window.watcher.blindness_witness();
        let retirements = window.watcher.retirement_witness();
        let retired_late = window.watcher.late_retirement_witness();

        window
            .close_a_pane_called(0, &ctx)
            .expect("close the pane requested through control");

        assert_eq!(
            retirements.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "the control close did not stop its old watcher before deleting the pane"
        );
        assert!(
            !retired_late.load(std::sync::atomic::Ordering::Relaxed),
            "the control close stopped its watcher only after deleting the pane"
        );
        let deadline = Instant::now() + Duration::from_millis(500);
        while !retiring_watcher.load(std::sync::atomic::Ordering::Relaxed)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !retiring_watcher.load(std::sync::atomic::Ordering::Relaxed),
            "the control close let its old watcher report the pane removal as lost tracking"
        );
        assert!(
            !leaving.exists(),
            "the control close left its pane directory"
        );
        assert!(
            staying.exists(),
            "the control close removed the surviving pane"
        );
    }

    /// Puts one request in the window's queue and runs the pass that answers it.
    ///
    /// **By hand and not over a socket**, because what these tests are about is what the window does
    /// with a request: that a line on a socket becomes one, and that the answer goes back, is
    /// covered beside the listener.
    fn ask_the_window(
        window: &mut Window,
        ctx: &egui::Context,
        origin: usize,
        connection: u64,
        what: panel_control::What,
    ) -> panel_control::Answer {
        // A window big enough for two consoles of this crate's minimum, which is the ordinary case.
        let whole_window =
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 800.0));
        ask_the_window_at(window, ctx, origin, connection, whole_window, what)
    }

    /// The same, with the window's rectangle said, **which is what the refusal of a split that does
    /// not fit needs**: the geometry is the whole of that answer.
    fn ask_the_window_at(
        window: &mut Window,
        ctx: &egui::Context,
        origin: usize,
        connection: u64,
        whole_window: egui::Rect,
        what: panel_control::What,
    ) -> panel_control::Answer {
        let (answering, answer) = std::sync::mpsc::sync_channel(1);
        window
            .all_panes_mut()
            .find(|pane| pane.name == origin)
            .and_then(|pane| pane.control.as_mut())
            .expect("that console keeps its channel")
            .push_for_the_tests(panel_control::Delivery {
                origin: panel_control::PublicationOrigin::Unknown,
                connection,
                what,
                answer: answering,
            });
        with_metrics(|metrics| window.answer_the_windows_requests(ctx, whole_window, metrics));
        answer
            .recv_timeout(Duration::from_secs(5))
            .expect("the window answered")
    }

    /// **AC13**: a console that cannot give both children their minimum is told so, and **nothing is
    /// spent** — not a console, and not even a name.
    #[test]
    fn a_split_that_does_not_fit_is_refused_and_spends_nothing() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);
        let panes = window.all_panes().count();
        let names = window.next_pane_name;

        // A window far narrower than two consoles' minimum: the left console cannot be halved.
        let narrow = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(20.0, 400.0));
        let panel_control::Answer::Refused(why) = ask_the_window_at(
            &mut window,
            &ctx,
            0,
            7,
            narrow,
            panel_control::What::Open {
                side: panel_control::Side::Right,
                program: Some("sleep 8".to_owned()),
                args: Vec::new(),
            },
        ) else {
            panic!("a console with no room to give both children was split anyway");
        };

        assert!(why.contains("minimum"), "not the reason: {why}");
        assert_eq!(window.all_panes().count(), panes, "a console was opened");
        assert_eq!(
            window.next_pane_name, names,
            "a name was spent on a split that did not happen"
        );
    }

    /// **AC15's doors, and the assertion is on the write happening through the real door** — not on
    /// a flag set by hand. Without this, deleting the call in `keyboard.rs` or in `mouse.rs` leaves
    /// the suite green, which is the disguise this project has paid for more than a dozen times.
    #[test]
    fn the_persons_own_input_at_the_two_doors_hands_the_console_back() {
        let ctx = egui::Context::default();

        // **The keyboard's door, which a paste also comes through.**
        let mut pane = pane_with("sleep 8", 0);
        assert!(
            pane.taken_over.is_none(),
            "the premise failed: the console was born already handed back"
        );
        enqueue(&ctx, vec![egui::Event::Paste("hello".to_owned())]);
        let mut modifiers = egui::Modifiers::NONE;
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );
        assert!(
            !pane.keystrokes.is_empty(),
            "the premise failed: the paste produced no bytes"
        );
        assert!(
            pane.taken_over.is_some(),
            "the person's own keystrokes did not hand the console back"
        );

        // **And the mouse's door**: a report, which reaches the program only once it asked for the
        // mouse — so the premise is asserted before the gesture.
        let mut pane = pane_with("sleep 8", 0);
        pane.screen.advance(b"\x1b[?1000h");
        assert!(
            pane.screen.mouse_reporting().wants_mouse(),
            "the premise failed: the program never asked for the mouse"
        );
        let sent = pane.send_mouse_to_the_program(
            mouse::MouseReport::Press(egui::PointerButton::Primary),
            Cell {
                row: 3,
                col: 4,
                half: Half::Left,
            },
            egui::Modifiers::NONE,
        );
        assert!(
            sent.is_some(),
            "the premise failed: the report went nowhere"
        );
        assert!(
            pane.taken_over.is_some(),
            "the person's mouse report did not hand the console back"
        );
    }

    /// **AC12, AC14 and AC15**: an agent's `split_open` splits the console that asked, hands back an
    /// opaque handle, and **does not move the person's keyboard**; a second connection cannot use
    /// that handle; and the moment the person's input reaches the console, a read with the handle is
    /// refused **with the reason** and not with a bare error.
    #[test]
    fn an_agent_opens_a_console_without_moving_the_focus_and_loses_it_when_somebody_types() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);
        let newborn = window.next_pane_name;
        assert_eq!(window.active().focus, 0, "the test starts on the left");

        let panel_control::Answer::Opened(Some(handle)) = ask_the_window(
            &mut window,
            &ctx,
            0,
            7,
            panel_control::What::Open {
                side: panel_control::Side::Right,
                program: Some("sleep 8".to_owned()),
                args: Vec::new(),
            },
        ) else {
            panic!("the console was not opened with a handle");
        };
        assert_eq!(window.next_pane_name, newborn + 1, "nothing was opened");
        assert_eq!(
            window.active().focus,
            0,
            "opening a console for an agent moved the person's keyboard"
        );

        // **AC14: another connection cannot name it, even holding the handle.** The scope is the
        // connection that opened it, so the second one is refused by name and not by luck.
        let panel_control::Answer::Refused(why) = ask_the_window(
            &mut window,
            &ctx,
            0,
            8,
            panel_control::What::Read {
                pane: handle.clone(),
                scrollback: false,
            },
        ) else {
            panic!("another connection read a console it did not open");
        };
        assert!(
            why.contains("another connection"),
            "the refusal does not say the handle is somebody else's: {why}"
        );

        // **The two reads, and the flag is what chooses between them.** The console's history holds
        // lines the visible screen no longer does, so swapping the two dumps cannot stay invisible:
        // making the screen scroll is the premise, and it is asserted before the read.
        {
            let opened = window
                .all_panes_mut()
                .find(|pane| pane.name == newborn)
                .expect("the console that was opened");
            for n in 0..80 {
                opened.screen.advance(format!("entry {n}\r\n").as_bytes());
            }
        }
        let read = |window: &mut Window, scrollback: bool| match ask_the_window(
            window,
            &ctx,
            0,
            7,
            panel_control::What::Read {
                pane: handle.clone(),
                scrollback,
            },
        ) {
            panel_control::Answer::Text(text) => text,
            other => panic!("the read was refused: {other:?}"),
        };
        let visible = read(&mut window, false);
        let everything = read(&mut window, true);
        assert!(
            visible.contains("entry 79"),
            "the visible read lost the screen: {visible:?}"
        );
        assert!(
            !visible.contains("entry 0"),
            "the visible read carried the history: {visible:?}"
        );
        assert!(
            everything.contains("entry 0"),
            "the scrollback read lost the history: {everything:?}"
        );

        // **The person types into it**, through the door the keyboard uses, and the handle dies.
        window
            .all_panes_mut()
            .find(|pane| pane.name == newborn)
            .expect("the console that was opened")
            .hand_it_back_to_the_person();

        let panel_control::Answer::Refused(why) = ask_the_window(
            &mut window,
            &ctx,
            0,
            7,
            panel_control::What::Read {
                pane: handle,
                scrollback: false,
            },
        ) else {
            panic!("a handle the person had taken back was allowed to read");
        };
        assert!(
            why.contains("went back"),
            "the refusal is not the reason the console was taken: {why}"
        );
    }

    /// An intentional session reset removes exactly that producer's work: its panel disappears,
    /// untouched auxiliary consoles close, a console the person took over survives, and repeating
    /// the same request reports the same transition without closing anything else. A later
    /// publication is a new generation and makes the panel visible again.
    #[test]
    fn retiring_a_session_clears_its_panel_and_only_its_untouched_consoles() {
        check_session_retirement(None);
    }

    #[test]
    fn a_done_holder_can_retire_its_auxiliaries_even_after_a_file_takes_the_document() {
        check_session_retirement(Some(false));
        check_session_retirement(Some(true));
    }

    fn check_session_retirement(file_takeover: Option<bool>) {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);
        let connection = 7;
        window
            .all_panes_mut()
            .find(|pane| pane.name == 0)
            .expect("the origin console")
            .panel_state
            .deliver(
                connection,
                "flow",
                panel_contract::parse_document(r#"{"lines":["old work"]}"#).expect("a panel"),
                SystemTime::now(),
            )
            .expect("the producer owns the panel");
        {
            let origin = window
                .all_panes_mut()
                .find(|pane| pane.name == 0)
                .expect("the origin console");
            origin.panel_state.an_agent_opened_it();
            origin
                .panel_state
                .the_person_took_it_over("the person took this console back");
        }

        let open = |window: &mut Window, side| {
            let name = window.next_pane_name;
            let answer = ask_the_window(
                window,
                &ctx,
                0,
                connection,
                panel_control::What::Open {
                    side,
                    program: Some("/bin/sleep".to_owned()),
                    args: vec!["8".to_owned()],
                },
            );
            let panel_control::Answer::Opened(Some(handle)) = answer else {
                panic!("the auxiliary console was not opened: {answer:?}");
            };
            (name, handle)
        };
        let (untouched, _) = open(&mut window, panel_control::Side::Right);
        let (also_untouched, _) = open(&mut window, panel_control::Side::Top);
        let (taken_over, taken_over_handle) = open(&mut window, panel_control::Side::Bottom);
        window
            .all_panes_mut()
            .find(|pane| pane.name == taken_over)
            .expect("the second auxiliary console")
            .hand_it_back_to_the_person();

        let panel_control::Answer::Refused(why) = ask_the_window(
            &mut window,
            &ctx,
            0,
            connection + 1,
            panel_control::What::Retire,
        ) else {
            panic!("a connection that does not own the panel retired it");
        };
        assert!(why.contains("held by"), "not the refusal: {why}");
        assert!(
            window
                .all_panes()
                .find(|pane| pane.name == 0)
                .expect("the origin console")
                .panel_state
                .is_visible(SystemTime::now()),
            "a refused retirement changed the panel"
        );

        if let Some(file_takeover) = file_takeover {
            let pane = window.all_panes_mut().find(|pane| pane.name == 0).unwrap();
            let now = SystemTime::now();
            pane.panel_state
                .deliver(
                    connection,
                    "flow",
                    panel_contract::parse_document(r#"{"attention":"done","lines":["finished"]}"#)
                        .unwrap(),
                    now,
                )
                .unwrap();
            if file_takeover {
                let path = pane
                    .panel
                    .as_ref()
                    .unwrap()
                    .path()
                    .join(panel_contract::PANEL_FILE);
                std::fs::write(&path, r#"{"lines":["later file"]}"#).unwrap();
                std::fs::File::open(&path)
                    .unwrap()
                    .set_modified(now + Duration::from_secs(1))
                    .unwrap();
                pane.refresh_its_panel(&ctx);
                assert!(pane.panel_state.document().unwrap().contains("later file"));
            }
        }

        let retiring_watcher = window.watcher.blindness_witness();
        let retirements = window.watcher.retirement_witness();
        let panel_control::Answer::Retired {
            closed: 2,
            preserved: 1,
        } = ask_the_window(
            &mut window,
            &ctx,
            0,
            connection,
            panel_control::What::Retire,
        )
        else {
            panic!("the owned session was not retired with its exact counts");
        };
        assert_eq!(
            retirements.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "remote retirement replaced its watcher without retiring the old generation"
        );
        assert!(
            window.all_panes().all(|pane| pane.name != untouched),
            "the untouched auxiliary console survived"
        );
        assert!(
            window.all_panes().all(|pane| pane.name != also_untouched),
            "the second untouched auxiliary console survived"
        );
        assert_eq!(
            window.control.retired_waiting(),
            2,
            "a detached PTY was destroyed before the retirement answer returned"
        );
        assert!(
            window.all_panes().any(|pane| pane.name == taken_over),
            "the person's auxiliary console was closed"
        );
        assert!(
            !window
                .all_panes()
                .find(|pane| pane.name == 0)
                .expect("the origin console")
                .panel_state
                .is_visible(SystemTime::now()),
            "the retired panel stayed visible"
        );

        let panes_before = window.all_panes().count();
        let panel_control::Answer::Refused(why) = ask_the_window(
            &mut window,
            &ctx,
            0,
            connection,
            panel_control::What::Open {
                side: panel_control::Side::Right,
                program: Some("/bin/sleep".to_owned()),
                args: vec!["8".to_owned()],
            },
        ) else {
            panic!("the retired connection opened work before publishing a new panel");
        };
        assert!(
            why.contains("publish a new panel"),
            "not the refusal: {why}"
        );
        assert_eq!(
            window.all_panes().count(),
            panes_before,
            "a refused post-retirement open changed the tree"
        );

        let panel_control::Answer::Retired {
            closed: 2,
            preserved: 1,
        } = ask_the_window(
            &mut window,
            &ctx,
            0,
            connection,
            panel_control::What::Retire,
        )
        else {
            panic!("a retry did not return the committed transition");
        };
        assert!(
            window.all_panes().any(|pane| pane.name == taken_over),
            "an idempotent retry closed the preserved console"
        );
        assert_eq!(
            window.control.retired_waiting(),
            0,
            "the next pass did not reap the one detached PTY left queued"
        );
        let deadline = Instant::now() + Duration::from_millis(500);
        while !retiring_watcher.load(std::sync::atomic::Ordering::Relaxed)
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !retiring_watcher.load(std::sync::atomic::Ordering::Relaxed),
            "remote retirement let its old watcher report detached directory removal"
        );
        let panel_control::Answer::Refused(why) = ask_the_window(
            &mut window,
            &ctx,
            0,
            connection,
            panel_control::What::Read {
                pane: taken_over_handle,
                scrollback: false,
            },
        ) else {
            panic!("retirement preserved ownership of the person's console");
        };
        assert!(
            why.contains("no console of yours"),
            "not the refusal: {why}"
        );

        window
            .all_panes_mut()
            .find(|pane| pane.name == 0)
            .expect("the origin console")
            .panel_state
            .deliver(
                connection,
                "flow",
                panel_contract::parse_document(r#"{"lines":["new work"]}"#).expect("a panel"),
                SystemTime::now(),
            )
            .expect("the next publication starts a new generation");
        assert!(
            window
                .all_panes()
                .find(|pane| pane.name == 0)
                .expect("the origin console")
                .panel_state
                .is_visible(SystemTime::now()),
            "a later publication did not bring the panel back"
        );
    }

    #[test]
    fn a_disconnect_after_an_admitted_retirement_does_not_release_its_authority_early() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);
        let connection = 7;
        let origin = window
            .all_panes_mut()
            .find(|pane| pane.name == 0)
            .expect("the origin console");
        origin
            .panel_state
            .deliver(
                connection,
                "flow",
                panel_contract::parse_document(r#"{"lines":["old work"]}"#).expect("a panel"),
                SystemTime::now(),
            )
            .expect("the producer owns the panel");

        let (retiring, retired) = std::sync::mpsc::sync_channel(1);
        let (closing, _closed) = std::sync::mpsc::sync_channel(1);
        let control = origin.control.as_mut().expect("the pane has its channel");
        control.enqueue_for_tests(panel_control::Delivery {
            origin: panel_control::PublicationOrigin::Unknown,
            connection,
            what: panel_control::What::Retire,
            answer: retiring,
        });
        control.enqueue_for_tests(panel_control::Delivery {
            origin: panel_control::PublicationOrigin::Unknown,
            connection,
            what: panel_control::What::Closed,
            answer: closing,
        });
        control.apply(
            &mut origin.panel_state,
            &mut crate::agent_signals::Signals::default(),
            SystemTime::now(),
        );
        assert_eq!(
            origin.panel_state.held_by(),
            Some("flow"),
            "disconnect released a retirement that the window had already admitted"
        );

        with_metrics(|metrics| {
            window.answer_the_windows_requests(
                &ctx,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0)),
                metrics,
            )
        });
        let panel_control::Answer::Retired {
            closed: 0,
            preserved: 0,
        } = retired
            .recv_timeout(Duration::from_secs(1))
            .expect("the admitted retirement was answered")
        else {
            panic!("the admitted retirement did not commit its empty result");
        };
        let origin = window
            .all_panes_mut()
            .find(|pane| pane.name == 0)
            .expect("the origin survives its own retirement");
        origin
            .control
            .as_mut()
            .expect("the pane has its channel")
            .apply(
                &mut origin.panel_state,
                &mut crate::agent_signals::Signals::default(),
                SystemTime::now(),
            );
        assert!(
            !origin.panel_state.is_visible(SystemTime::now()),
            "disconnect turned an admitted clear into an ordinary crash release"
        );
        assert!(
            origin.panel_state.held_by().is_none(),
            "the processed closure kept the retired connection's hold"
        );
    }

    #[test]
    fn a_publication_after_a_queued_retirement_cannot_overtake_it() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);
        let connection = 7;
        let origin = window
            .all_panes_mut()
            .find(|pane| pane.name == 0)
            .expect("the origin console");
        origin
            .panel_state
            .deliver(
                connection,
                "flow",
                panel_contract::parse_document(r#"{"lines":["old work"]}"#).expect("the old panel"),
                SystemTime::now(),
            )
            .expect("the producer owns the panel");
        let (retiring, retired) = std::sync::mpsc::sync_channel(1);
        let (publishing, published) = std::sync::mpsc::sync_channel(1);
        let control = origin.control.as_mut().expect("the pane has its channel");
        control.enqueue_for_tests(panel_control::Delivery {
            origin: panel_control::PublicationOrigin::Unknown,
            connection,
            what: panel_control::What::Retire,
            answer: retiring,
        });
        control.enqueue_for_tests(panel_control::Delivery {
            origin: panel_control::PublicationOrigin::Unknown,
            connection,
            what: panel_control::What::Publish {
                source: "flow".to_owned(),
                panel: panel_contract::parse_document(r#"{"lines":["new work"]}"#)
                    .expect("the new panel"),
            },
            answer: publishing,
        });
        control.apply(
            &mut origin.panel_state,
            &mut crate::agent_signals::Signals::default(),
            SystemTime::now(),
        );
        assert!(
            published.try_recv().is_err(),
            "the later publication overtook retirement"
        );

        with_metrics(|metrics| {
            window.answer_the_windows_requests(
                &ctx,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0)),
                metrics,
            )
        });
        assert!(
            matches!(
                retired.recv_timeout(Duration::from_secs(1)),
                Ok(panel_control::Answer::Retired { .. })
            ),
            "the window did not answer the queued retirement"
        );
        let origin = window
            .all_panes_mut()
            .find(|pane| pane.name == 0)
            .expect("the origin console");
        origin
            .control
            .as_mut()
            .expect("the pane has its channel")
            .apply(
                &mut origin.panel_state,
                &mut crate::agent_signals::Signals::default(),
                SystemTime::now(),
            );
        assert!(
            matches!(
                published.recv_timeout(Duration::from_secs(1)),
                Ok(panel_control::Answer::Taken)
            ),
            "the publication queued behind the retirement was not taken once it was answered"
        );
        assert_eq!(
            origin
                .panel_state
                .last_understood()
                .expect("the next generation is visible")
                .lines[0]
                .text,
            "new work",
            "the publication queued after the retirement is not the panel left on screen"
        );
    }

    #[test]
    fn an_ordinary_disconnect_preserves_the_panel_and_every_auxiliary_console() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);
        let connection = 7;
        window
            .all_panes_mut()
            .find(|pane| pane.name == 0)
            .expect("the origin console")
            .panel_state
            .deliver(
                connection,
                "flow",
                panel_contract::parse_document(r#"{"lines":["evidence"]}"#).expect("the panel"),
                SystemTime::now(),
            )
            .expect("the producer owns the panel");
        let auxiliary = window.next_pane_name;
        let panel_control::Answer::Opened(Some(handle)) = ask_the_window(
            &mut window,
            &ctx,
            0,
            connection,
            panel_control::What::Open {
                side: panel_control::Side::Right,
                program: Some("/bin/sleep".to_owned()),
                args: vec!["8".to_owned()],
            },
        ) else {
            panic!("the auxiliary console was not opened");
        };

        let (closing, _closed) = std::sync::mpsc::sync_channel(1);
        let origin = window
            .all_panes_mut()
            .find(|pane| pane.name == 0)
            .expect("the origin console");
        let control = origin.control.as_mut().expect("the pane has its channel");
        control.enqueue_for_tests(panel_control::Delivery {
            origin: panel_control::PublicationOrigin::Unknown,
            connection,
            what: panel_control::What::Closed,
            answer: closing,
        });
        control.apply(
            &mut origin.panel_state,
            &mut crate::agent_signals::Signals::default(),
            SystemTime::now(),
        );
        with_metrics(|metrics| {
            window.answer_the_windows_requests(
                &ctx,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0)),
                metrics,
            )
        });

        let origin = window
            .all_panes()
            .find(|pane| pane.name == 0)
            .expect("the origin console survived");
        assert!(
            origin.panel_state.is_visible(SystemTime::now()),
            "ordinary disconnect cleared the panel evidence"
        );
        assert!(
            window.all_panes().any(|pane| pane.name == auxiliary),
            "ordinary disconnect closed an auxiliary console"
        );
        let panel_control::Answer::Refused(why) = ask_the_window(
            &mut window,
            &ctx,
            0,
            connection,
            panel_control::What::Read {
                pane: handle,
                scrollback: false,
            },
        ) else {
            panic!("a handle survived the connection that owned it");
        };
        assert!(
            why.contains("no console of yours"),
            "not the refusal: {why}"
        );
    }

    #[test]
    fn retirement_validation_refuses_every_incomplete_tree_change_atomically() {
        let window = window_with("sleep 8", "sleep 8");
        let names = window.all_panes().map(|pane| pane.name).collect::<Vec<_>>();
        let focus = window.active().focus;

        for targets in [vec![usize::MAX], vec![names[0], names[0]], names.clone()] {
            assert!(
                window.validate_detaching_panes(&targets).is_err(),
                "an invalid retirement target set was accepted: {targets:?}"
            );
            assert_eq!(
                window.all_panes().map(|pane| pane.name).collect::<Vec<_>>(),
                names,
                "validation changed the tree before accepting the whole retirement"
            );
            assert_eq!(window.active().focus, focus, "validation changed focus");
        }
    }

    /// **The row that says the person took the console over is only published where an agent had a
    /// console to lose** (**AC15**). The sentence reports the end of an access, so on a console the
    /// person opened themselves there is no access that ended — and publishing it anyway made the
    /// panel **visible**, which is what takes [`panel::DEFAULT_WIDTH`] columns off the console the
    /// first time anybody types into it.
    ///
    /// **Both sides are asserted in the same window and through the same door**, because a gate is
    /// the one shape a single-sided test cannot cover: asserting only the agent's console passes
    /// with the gate removed, and asserting only the person's passes with the row never published at
    /// all. And the assertion is on the panel **being visible**, not on the field where the sentence
    /// is recorded: what the person meets is the panel appearing.
    #[test]
    fn only_a_console_an_agent_opened_says_on_its_panel_that_it_went_back() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);
        let newborn = window.next_pane_name;

        let panel_control::Answer::Opened(Some(_)) = ask_the_window(
            &mut window,
            &ctx,
            0,
            7,
            panel_control::What::Open {
                side: panel_control::Side::Right,
                program: Some("sleep 8".to_owned()),
                args: Vec::new(),
            },
        ) else {
            panic!("the console was not opened with a handle");
        };

        // **Nobody has published here**, so neither panel is showing anything before the typing: the
        // premise of the whole test, and without it the person's console could be passing for a
        // reason of its own.
        let now = SystemTime::now();
        let visible = |window: &Window, name: usize| {
            window
                .all_panes()
                .find(|pane| pane.name == name)
                .expect("the console")
                .panel_state
                .is_visible(now)
        };
        assert!(
            !visible(&window, 0),
            "the person's panel was already showing"
        );
        assert!(
            !visible(&window, newborn),
            "the agent's panel was already showing"
        );

        // **The person types into both, through the door a keystroke and a paste share**, and not by
        // setting the flag by hand: what is being guarded is which console grows a panel from real
        // input.
        for name in [0, newborn] {
            let pane = window
                .all_panes_mut()
                .find(|pane| pane.name == name)
                .expect("the console");
            enqueue(&ctx, vec![egui::Event::Paste("hello".to_owned())]);
            let mut modifiers = egui::Modifiers::NONE;
            pane.send_keys(
                &ctx,
                Instant::now(),
                &mut modifiers,
                2,
                &keyboard::Keymap::default(),
                true,
            );
            assert!(
                pane.taken_over.is_some(),
                "the premise failed: console {name} was not handed back"
            );
        }

        assert!(
            visible(&window, newborn),
            "the console an agent opened does not say on its panel that it went back"
        );
        assert!(
            !visible(&window, 0),
            "typing into the person's own console grew it a panel nobody published"
        );
    }

    /// **AC16, the half that is reachable**: a console the agent opened is closed by its handle, the
    /// tree loses it, and the handle is forgotten with it. Without this, a build that refused *every*
    /// close would pass the refusal tests and close nothing.
    ///
    /// The other half — a console that is its tab's only one is refused rather than closing the tab —
    /// is unreachable from here and is defended anyway: the origin pane has to stay alive for the
    /// handle to be usable at all, and opening a console always leaves at least two in that tab.
    #[test]
    fn a_console_the_agent_opened_is_closed_by_its_handle_and_the_handle_is_forgotten() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);

        let panel_control::Answer::Opened(Some(handle)) = ask_the_window(
            &mut window,
            &ctx,
            0,
            7,
            panel_control::What::Open {
                side: panel_control::Side::Bottom,
                program: Some("sleep 8".to_owned()),
                args: Vec::new(),
            },
        ) else {
            panic!("the console was not opened with a handle");
        };
        let opened = window.all_panes().count();

        let answer = ask_the_window(
            &mut window,
            &ctx,
            0,
            7,
            panel_control::What::Close {
                pane: handle.clone(),
            },
        );
        assert!(
            matches!(answer, panel_control::Answer::Taken),
            "the close was refused: {answer:?}"
        );
        assert_eq!(
            window.all_panes().count(),
            opened - 1,
            "the console was not closed"
        );

        // **And the handle went with it**: a console that is gone is not somebody else's either.
        let answer = ask_the_window(
            &mut window,
            &ctx,
            0,
            7,
            panel_control::What::Close { pane: handle },
        );
        assert!(
            matches!(&answer, panel_control::Answer::Refused(why) if why.contains("that handle")),
            "a forgotten handle was still usable: {answer:?}"
        );
    }

    /// **AC16**: a handle nobody issued is refused, **and a handle that belongs to another connection
    /// too** — the scope is the connection and not the name, which is the whole of what the handle is
    /// for.
    #[test]
    fn a_close_is_refused_for_a_handle_this_connection_does_not_own() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 8", "sleep 8");
        attach_a_channel(&mut window, 0);
        let panes = window.all_panes().count();

        let panel_control::Answer::Refused(_) = ask_the_window(
            &mut window,
            &ctx,
            0,
            7,
            panel_control::What::Close {
                pane: "not-a-handle".to_owned(),
            },
        ) else {
            panic!("a handle nobody issued was allowed to close a console");
        };
        assert_eq!(window.all_panes().count(), panes, "something was closed");

        // One the agent did open, asked for from a console that did not open it.
        let panel_control::Answer::Opened(Some(handle)) = ask_the_window(
            &mut window,
            &ctx,
            0,
            7,
            panel_control::What::Open {
                side: panel_control::Side::Right,
                program: Some("sleep 8".to_owned()),
                args: Vec::new(),
            },
        ) else {
            panic!("the console was not opened with a handle");
        };
        let newborn = window.next_pane_name - 1;
        attach_a_channel(&mut window, newborn);
        let panel_control::Answer::Refused(why) = ask_the_window(
            &mut window,
            &ctx,
            newborn,
            7,
            panel_control::What::Close { pane: handle },
        ) else {
            panic!("a handle was used from a console that did not open it");
        };
        assert!(
            why.contains("another connection"),
            "the refusal does not say the handle is somebody else's: {why}"
        );
    }

    /// Builds a pane with a program inside, with no interface framework in the way.
    ///
    /// The two halves of the frame tested here —pumping and resizing— do not touch the desktop
    /// window, so there is no need to open one: the struct is armed by hand with the same pieces as
    /// [`Pane::new_counting_reads`].
    fn pane_with(program: &str, name: usize) -> Pane {
        #[cfg(unix)]
        let (pty, output) = Pty::spawn("sh", &["-c", program], INITIAL_GRID)
            .expect("could not open the pseudoterminal");
        #[cfg(windows)]
        let (pty, output) = {
            let arguments = if let Some(seconds) = program.strip_prefix("sleep ") {
                assert!(
                    seconds.parse::<f64>().is_ok(),
                    "sleep fixture must have a numeric duration: {program:?}"
                );
                format!("Start-Sleep -Seconds {seconds}")
            } else if program == "yes" {
                "while ($true) { [Console]::WriteLine('y') }".to_owned()
            } else if program == "true" {
                "exit 0".to_owned()
            } else if program == "cat" {
                "while ($true) { $character = [Console]::In.Read(); if ($character -lt 0) { break }; [Console]::Out.Write([char]$character) }".to_owned()
            } else if let Some(text) = program.strip_prefix("printf ") {
                let text = text
                    .strip_prefix('\'')
                    .and_then(|text| text.strip_suffix('\''))
                    .unwrap_or(text)
                    .to_owned();
                assert!(
                    !text.contains(['%', '\\', '$', '`']),
                    "printf fixture needs POSIX formatting or escapes: {program:?}"
                );
                let text = text.replace("'", "''");
                format!("[Console]::Write( '{text}' )")
            } else {
                panic!("no Windows PTY fixture for Unix program {program:?}");
            };
            Pty::spawn(
                "powershell.exe",
                &[
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &arguments,
                ],
                INITIAL_GRID,
            )
            .expect("could not open the pseudoterminal")
        };

        Pane {
            name,
            label: None,
            project: None,
            program: None,
            restoration: restoration::PaneState::default(),
            signals: Default::default(),
            pty: pty.into(),
            pty_output: PtyOutput::new(output, || {}),
            _shell_integration: None,
            panel: None,
            control: None,
            panel_state: panel::PanelState::default(),
            to_reread: Flag::raised(),
            screen: Screen::new(INITIAL_GRID),
            painted_console: None,
            timings: Timings::default(),
            pending_measurement: None,
            panel_reread_unpainted: std::time::Duration::ZERO,
            control_unpainted: std::time::Duration::ZERO,
            reflowed: std::time::Duration::ZERO,
            pending_wheel: WheelInput::default(),
            panel_width: panel::DEFAULT_WIDTH,
            folded: false,
            taken_over: None,
            received: Vec::new(),
            keystrokes: Vec::new(),
            held: None,
            search: search::OnThisConsole::default(),
            measured: INITIAL_GRID,
        }
    }
crate::windows_unix_omission!(
    a_window_that_lost_the_desktops_focus_tells_its_focused_console_to_come_back,
    "unix-only: requires the Linux pseudoterminal input-buffer boundary for held-paste assertions"
);
#[cfg(unix)]
#[test]
    fn a_window_that_lost_the_desktops_focus_tells_its_focused_console_to_come_back() {
        // **The cursor and the notice answer the same question, and they used to answer it apart**:
        // with the window alt-tabbed away the focused console outlined its cursor — nobody is typing
        // here — while its notice went on saying that `Ctrl+Enter` sends the paste.
        let area = area_of(800.0 + DIVIDER_THICKNESS);
        let mut window = window_with("sleep 30", "sleep 30");
        for pane in window.active_mut().panes_mut() {
            if !hold_a_paste_on(pane) {
                return;
            }
        }
        focus_on(window.active_mut(), 1);
        let ctx = egui::Context::default();
        // The text engine does not exist until there has been a pass.
        text_of_the_two_consoles(&mut window, &ctx, area, true);

        let away = text_of_the_two_consoles(&mut window, &ctx, area, false);
        assert!(
            away[1].contains(painter::notices::ANSWERED_ONCE_BACK),
            "with the window out of the desktop's focus the console that has the keyboard inside it \
             did not say to come back: {:?}",
            away[1]
        );
        assert!(
            !away[1].contains(painter::notices::ANSWERED_BY_HAND),
            "with the window out of the desktop's focus the console that has the keyboard inside it \
             still said its keys answer now: {:?}",
            away[1]
        );
        assert!(
            away[0].contains(painter::notices::ANSWERED_BY_CLICK),
            "the console without the keyboard stopped saying to focus it: {:?}",
            away[0]
        );

        // And back, so the assertions above are not satisfied by a notice that never names the keys.
        let back = text_of_the_two_consoles(&mut window, &ctx, area, true);
        assert!(
            back[1].contains(painter::notices::ANSWERED_BY_HAND),
            "coming back to the window did not give the focused console its keys again: {:?}",
            back[1]
        );
        assert!(
            back[0].contains(painter::notices::ANSWERED_BY_CLICK),
            "coming back to the window changed what the console without the keyboard says: {:?}",
            back[0]
        );
    }

    #[test]
    fn a_click_moves_the_filled_cursor_in_the_same_frame_that_moves_the_focus() {
        // **The focus is read after the keyboard and after the mouse, and that ordering is what
        // makes the cursor land in the same frame as the click.** Nothing failed if a later delivery
        // hoisted that read above them: the cursor would follow one frame behind, which in a
        // reactive window is until something else happens to ask for a repaint.
        let area = area_of(800.0 + DIVIDER_THICKNESS);
        let mut window = window_whose_cursor_is_its_own("sleep 30", "sleep 30");
        focus_on(window.active_mut(), 0);
        let ctx = egui::Context::default();
        // A pass with no events first, for two reasons at once: the text engine does not exist until
        // there has been one, and egui resolves an interaction against the regions of the previous
        // pass — a click on a console nobody has registered yet reaches nothing.
        assert_eq!(
            cursors_of_the_two_consoles(&mut window, &ctx, area, true),
            vec![true, false],
            "the pass that sets the scene did not leave the filled cursor on the left-hand console"
        );
        let metrics =
            painter::CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let tiling = tile(
            console_area(&window, area),
            Orientation::Columns,
            &weights_of(INITIAL_FRACTION),
            metrics.width(),
        );
        let onto_the_right_one = tiling.parts[1].center();

        let ink = A_CURSOR_OF_ITS_OWN;
        // **A click is a press and a release**, and egui only calls it one once the button comes back
        // up: the frame under test is the release's, which is the frame in which the focus moves.
        let button = |pressed| egui::Event::PointerButton {
            pos: onto_the_right_one,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        let mut frame = eframe::Frame::_new_kittest();
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(area),
                events: vec![egui::Event::PointerMoved(onto_the_right_one), button(true)],
                ..Default::default()
            },
            |ui| eframe::App::ui(&mut window, ui, &mut frame),
        )
        .drop_without_applying_deltas();

        let input = egui::RawInput {
            screen_rect: Some(area),
            events: vec![button(false)],
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| eframe::App::ui(&mut window, ui, &mut frame));
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();

        assert_eq!(
            window.active().where_the_focus_is(),
            Some(1),
            "the click did not move the focus, so this test is not exercising what it claims"
        );
        // **Both consoles, not only the one that gained the focus.** Asserting that the right-hand
        // one fills says nothing about the left-hand one still filling: a painter that filled every
        // console would pass, and the scene-setting pass above is the only thing that would have
        // caught it.
        let mut filled = vec![None; tiling.parts.len()];
        for clipped in &shapes {
            let egui::Shape::Rect(rect) = &clipped.shape else {
                continue;
            };
            let it_is_the_cursor = if rect.fill == ink {
                true
            } else if rect.stroke.color == ink {
                false
            } else {
                continue;
            };
            for (at, console) in tiling.parts.iter().enumerate() {
                if console.contains_rect(clipped.clip_rect) {
                    filled[at] = Some(it_is_the_cursor);
                }
            }
        }
        assert_eq!(
            filled,
            vec![Some(false), Some(true)],
            "the click did not move the fill to the console it gave the keyboard to, in that same \
             frame and away from the one that had it"
        );
    }

    #[test]
    fn the_drag_resizes_the_consoles_in_the_same_pass_in_which_it_moves_the_divider() {
        // The tiling is done **twice** in the same pass when the drag moved the fraction, and
        // without that second time everything that comes afterwards —the painted divider and the
        // grid each console measures— is left with the previous tiling: one frame behind the mouse,
        // which is exactly what is noticed when dragging. Removing that second tiling broke no
        // test, because the drag was tested without painting and the painting without dragging.
        let area = area_of(800.0 + DIVIDER_THICKNESS);
        let mut window = window_with("sleep 30", "sleep 30");
        // The divider's center with the initial fraction: half of what is shareable after
        // whatever the sidebar reserves, where the floor per console does not get to bite.
        let consoles = console_area(&window, area);
        let shareable = consoles.width() - DIVIDER_THICKNESS;
        let grip = egui::pos2(
            consoles.left() + shareable / 2.0 + DIVIDER_THICKNESS / 2.0,
            area.center().y,
        );
        // Three quarters of the same shareable area: a place the initial fraction does not reach.
        let drop_at = egui::pos2(consoles.left() + shareable * 0.75, grip.y);
        let ctx = egui::Context::default();

        // The drag's three passes, for the same reason as in the `drag_the_divider` test: egui
        // resolves the interaction against the previous pass's regions, and the candidate to be
        // dragged comes from where the pointer was **on pressing**.
        ui_pass(&mut window, &ctx, area, move_pane(grip));
        ui_pass(&mut window, &ctx, area, press(grip));
        ui_pass(&mut window, &ctx, area, move_pane(drop_at));

        // The pointer is exactly three quarters through the shared console area, so no tolerance
        // is needed here either.
        assert_eq!(
            window.active().divider_fractions()[0],
            0.75,
            "the drag did not carry the fraction all the way to the pointer"
        );

        let metrics = CellMetrics::measure(&ctx, egui::FontId::monospace(DEFAULT_FONT_SIZE));
        let tiling = tile(
            console_area(&window, area),
            Orientation::Columns,
            &weights_of(0.75),
            metrics.width(),
        );
        assert_eq!(
            window.active().panes()[0].measured,
            grid_of(tiling.parts[0], &metrics),
            "the left console was measured with the tiling from before the drag"
        );
        assert_eq!(
            window.active().panes()[1].measured,
            grid_of(tiling.parts[1], &metrics),
            "the right console was measured with the tiling from before the drag"
        );
    }

    /// Leaves a pane with its panel published and read, ready for its border to be grabbed.
    ///
    /// **That the panel is really in place is the condition of every test of the gesture**: without
    /// it there is no border to attend to, so a gesture over nothing would pass for «the drag did
    /// not move the width» and the test would say the floor works when what happens is that there
    /// is no panel.
    fn with_the_panel_in_place(window: &mut Window, which: usize, contents: &str) {
        let mut its_panes = window.active_mut().panes_mut();
        let pane = &mut its_panes[which];
        pane.panel = Some(PanelDir::create().expect("create the pane's directory"));
        publish_at(pane, contents);
        pane.to_reread.raise();
        pane.refresh_its_panel(&egui::Context::default());
    }

    #[test]
    fn a_bare_pointer_press_routes_to_a_painted_header_after_toggle_and_republish() {
        let ctx = egui::Context::default();
        let metrics = with_metrics(|metrics| metrics.clone());
        let area = area_of_cells(200.0);
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        with_the_panel_in_place(
            &mut window,
            0,
            r#"{"header":false,"lines":[{"text":"MRs","style":"section","summary":"2/4 merged","collapsed":true,"key":"mrs"},"body assertion"]}"#,
        );

        fn app_frame(
            window: &mut Window,
            ctx: &egui::Context,
            area: egui::Rect,
            events: Vec<egui::Event>,
        ) -> Vec<(String, egui::Pos2)> {
            fn collect(shape: &egui::Shape, text: &mut Vec<(String, egui::Pos2)>) {
                match shape {
                    egui::Shape::Text(shape) => {
                        text.push((shape.galley.job.text.clone(), shape.pos));
                    }
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            collect(shape, text);
                        }
                    }
                    _ => {}
                }
            }
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::logic(window, ctx, &mut frame);
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(area),
                    events,
                    ..Default::default()
                },
                |ui| eframe::App::ui(window, ui, &mut frame),
            );
            let mut text = Vec::new();
            for shape in &output.shapes {
                collect(&shape.shape, &mut text);
            }
            output.drop_without_applying_deltas();
            text
        }

        // The first full app frame both paints the panel and yields the visible header's actual
        // screen position, so the test presses the pixels the person saw.
        let first = app_frame(&mut window, &ctx, area, Vec::new());
        assert!(
            !first
                .iter()
                .any(|(text, _)| text.contains("body assertion")),
            "the producer's collapsed header did not hide its body"
        );
        let header = first
            .iter()
            .find(|(text, _)| text.starts_with('▸'))
            .expect("the collapsed header was painted");
        let header_click = header.1 + egui::vec2(metrics.width() * 0.25, metrics.height() * 0.5);

        let click = |window: &mut Window| {
            let pressed = app_frame(
                window,
                &ctx,
                area,
                vec![
                    egui::Event::PointerMoved(header_click),
                    egui::Event::PointerButton {
                        pos: header_click,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            let released = app_frame(
                window,
                &ctx,
                area,
                vec![egui::Event::PointerButton {
                    pos: header_click,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            if released
                .iter()
                .any(|(text, _)| text.contains("body assertion"))
            {
                released
            } else {
                pressed
            }
        };

        let opened = click(&mut window);
        assert!(
            opened
                .iter()
                .any(|(text, _)| text.contains("body assertion")),
            "the queued bare press did not open the painted header at {header_click:?}; opened: {opened:?}"
        );
        let refolded = click(&mut window);
        assert!(
            !refolded
                .iter()
                .any(|(text, _)| text.contains("body assertion")),
            "the immediate repeated queued press did not fold the header"
        );

        let reopened = click(&mut window);
        assert!(
            reopened
                .iter()
                .any(|(text, _)| text.contains("body assertion"))
        );

        // Republish the producer's original default. The reader's open choice must win, and a
        // click on that just-repainted header must still be routed to the painted target.
        let pane = &window.active().panes()[0];
        publish_at(
            pane,
            r#"{"header":false,"lines":[{"text":"MRs","style":"section","summary":"2/4 merged","collapsed":true,"key":"mrs"},"body assertion"]}"#,
        );
        window.active_mut().panes_mut()[0].refresh_its_panel(&ctx);
        let after_publish = app_frame(&mut window, &ctx, area, Vec::new());
        assert!(
            after_publish
                .iter()
                .any(|(text, _)| text.contains("body assertion")),
            "republishing discarded the reader's open choice"
        );
        let folded_after_publish = click(&mut window);
        assert!(
            !folded_after_publish
                .iter()
                .any(|(text, _)| text.contains("body assertion")),
            "the click after republish did not fold the header"
        );
    }

    /// Where a pane's panel border is grabbed, in points.
    ///
    /// **Folded there is no panel and the handle is stuck to the edge**, so the point is computed
    /// the same way as in the painting. If production moved that place, the unfolding tests would
    /// grab where there is nothing left and would fall — which is what is wanted of a helper that
    /// replicates a position.
    fn console_area(window: &Window, area: egui::Rect) -> egui::Rect {
        with_metrics(|metrics| crate::tab_bar::geometry(window, area, metrics).consoles)
    }

    fn panel_cut(window: &Window, which: usize, area: egui::Rect) -> f32 {
        let cell_width = with_metrics(CellMetrics::width);
        let content =
            with_metrics(|metrics| crate::tab_bar::geometry(window, area, metrics).consoles);
        let its_area = window.active().root.place(content, cell_width).parts[which];
        match window.active().panes()[which]
            .tile_with_its_panel(
                its_area,
                &CellMetrics::of(cell_width, 16.0),
                SystemTime::now(),
            )
            .panel
        {
            Some(panel_rect) => panel_rect.left(),
            None => its_area.right() - PANEL_GRIP,
        }
    }

    /// The pointer, moving to a point.
    fn pointer_at(x: f32, y: f32) -> Vec<egui::Event> {
        vec![egui::Event::PointerMoved(egui::pos2(x, y))]
    }

    /// The primary button, going down or up at a point.
    fn button(x: f32, y: f32, pressed: bool) -> Vec<egui::Event> {
        with_button(egui::PointerButton::Primary, x, y, pressed)
    }

    /// The same with whichever button is named, for the ones that must move nothing.
    fn with_button(which: egui::PointerButton, x: f32, y: f32, pressed: bool) -> Vec<egui::Event> {
        vec![egui::Event::PointerButton {
            pos: egui::pos2(x, y),
            button: which,
            pressed,
            modifiers: egui::Modifiers::default(),
        }]
    }

    /// A whole gesture on a pane's panel border: grab where it is and drop at `drag_to`, going
    /// through the real path — the complete interface pass, not the bare method.
    fn drag_the_panel(
        window: &mut Window,
        ctx: &egui::Context,
        area: egui::Rect,
        which: usize,
        drag_to: f32,
    ) {
        let cut_along = panel_cut(window, which, area);
        let mid_y = area.center().y;
        one_frame_in(window, ctx, area, pointer_at(cut_along, mid_y));
        one_frame_in(window, ctx, area, button(cut_along, mid_y, true));
        one_frame_in(window, ctx, area, pointer_at(drag_to, mid_y));
        one_frame_in(window, ctx, area, button(drag_to, mid_y, false));
    }

    #[test]
    fn dragging_the_panel_border_narrows_it_and_gives_the_columns_to_its_console() {
        // **The two halves, and the second is the one that really matters.** That the stored width
        // goes down can be satisfied with the painting and the notice to the program still looking
        // at the old tiling, which is the double readjustment seen in any full-screen program. So
        // it is also asserted on `measured`, which is the number the pumping sends to the
        // pseudoterminal.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);

        // One frame before the gesture, so the console has measured with the panel already in
        // place.
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let before = window.active().panes()[0].measured.cols;
        let cell = with_metrics(CellMetrics::width);
        let cut_along = panel_cut(&window, 0, area);

        // Ten cells to the right: the panel narrows and the console widens by the same amount.
        drag_the_panel(&mut window, &ctx, area, 0, cut_along + 10.0 * cell);

        assert_eq!(
            window.active().panes()[0].panel_width,
            panel::DEFAULT_WIDTH - 10,
            "the panel did not keep the cells the pointer left it"
        );
        assert_eq!(
            window.active().panes()[0].measured.cols,
            before + 10,
            "the width went down but the console kept being announced the grid from before the \
             gesture"
        );
    }

    #[test]
    fn the_panel_width_belongs_to_each_pane_and_not_to_the_window() {
        // Four panes are four producers publishing things of different sizes, and the point of
        // being able to narrow the panel is to leave the one that says something wide and the one
        // that does not narrow. With one width belonging to the application, dragging in one would
        // move it in all of them.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["the first one"]}"#);
        with_the_panel_in_place(&mut window, 1, r#"{"lines":["the second one"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let cut_along = panel_cut(&window, 0, area);
        drag_the_panel(&mut window, &ctx, area, 0, cut_along + 8.0 * cell);

        assert_eq!(
            window.active().panes()[0].panel_width,
            panel::DEFAULT_WIDTH - 8,
            "dragging the border did not narrow the dragged pane's panel by eight columns"
        );
        assert_eq!(
            window.active().panes()[1].panel_width,
            panel::DEFAULT_WIDTH,
            "dragging one pane's border changed the width of its sibling"
        );
    }

    #[test]
    fn dragging_to_the_right_edge_folds_the_panel_and_the_same_gesture_brings_it_back() {
        // **Folding is dragging all the way to the end, and what this test ties down is that the
        // journey has a return.** Before folding existed, this very gesture left one cell of panel
        // for the same reason: so the border did not go away with it. Now it does fold completely,
        // and what sustains the return is the handle stuck to the edge. It is dragged **beyond**
        // the edge on purpose, which is where a negative width would turn into an absurd number.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let requested = window.active().panes()[0].panel_width;
        drag_the_panel(&mut window, &ctx, area, 0, area.right() + 500.0);

        // **Folded, the console is left with the whole rectangle** — just like one whose producer
        // has published nothing, which is what makes there not be two different ways of having no
        // panel.
        let cell = with_metrics(CellMetrics::width);
        let its_area = window.active().root.place(area, cell).parts[0];
        assert!(window.active().panes()[0].folded, "it did not fold");
        let panel_split = window.active().panes()[0].tile_with_its_panel(
            its_area,
            &CellMetrics::of(cell, 16.0),
            SystemTime::now(),
        );
        assert!(
            panel_split.panel.is_none(),
            "folded, room is still being reserved for the panel"
        );
        assert_eq!(
            panel_split.console,
            its_area.shrink(8.0),
            "folded, the console keeps its pane apart from inner padding"
        );

        // **And the return journey, pulling on the handle that stays stuck to the edge.** Without
        // it, the gesture that folded could not be undone with the mouse: a journey with no return
        // ticket.
        drag_the_panel(&mut window, &ctx, area, 0, its_area.right() - 20.0 * cell);

        assert!(
            !window.active().panes()[0].folded,
            "the panel folded and the same gesture no longer reopens it: it was left with no handle"
        );
        // **And it comes back to the width the user asked for, not the factory one.** It is the
        // reason folding is a field of its own and not a «width of zero»: a zero that also meant
        // folded would have lost this.
        assert_eq!(
            window.active().panes()[0].panel_width,
            requested,
            "unfolding gave back a width the user had not asked for"
        );
    }

    #[test]
    fn the_panel_width_survives_splitting_the_pane() {
        // The width belongs to the console, not to the place it occupies: splitting the window puts
        // it in another rectangle and with another sibling beside it, and what the user asked for
        // is still what they asked for.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let cut_along = panel_cut(&window, 0, area);
        drag_the_panel(&mut window, &ctx, area, 0, cut_along + 6.0 * cell);
        let requested = window.active().panes()[0].panel_width;
        assert_ne!(
            requested,
            panel::DEFAULT_WIDTH,
            "the gesture did not change the width, so splitting has nothing to preserve"
        );

        one_frame_in(&mut window, &ctx, area, chord_with(egui::Key::D));

        assert_eq!(
            window.active().panes()[0].panel_width,
            requested,
            "splitting the window gave the panel back the factory width"
        );
    }

    #[test]
    fn the_panel_drag_goes_by_the_name_of_its_pane_and_not_by_its_place() {
        // **The defect this identifier avoids has already happened in this project, with the
        // consoles.** egui sustains a drag between frames by its widget's id, and the chords for
        // splitting, closing and switching tabs are applied **at the start of the same pass**,
        // before the mouse is attended to. With the place inside the identifier, grabbing the
        // border and switching tabs without letting go would carry the drag on over the pane that
        // now occupies that place — the destination tab's.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["the one in front"]}"#);
        window.active = 1;
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["the one behind"]}"#);
        window.active = 0;
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let cut_along = panel_cut(&window, 0, area);
        let mid_y = area.center().y;

        // The front tab's border is grabbed…
        one_frame_in(&mut window, &ctx, area, pointer_at(cut_along, mid_y));
        one_frame_in(&mut window, &ctx, area, button(cut_along, mid_y, true));
        // …and without letting go the tab is switched and the pointer moved a long way off.
        one_frame_in(&mut window, &ctx, area, chord_with(egui::Key::ArrowRight));
        one_frame_in(
            &mut window,
            &ctx,
            area,
            pointer_at(cut_along + 15.0 * cell, mid_y),
        );

        assert_eq!(
            window.active, 1,
            "the chord did not switch tabs, so the test never got to set up the case"
        );
        assert_eq!(
            window.tabs[1].panes()[0].panel_width,
            panel::DEFAULT_WIDTH,
            "the drag in flight took down the destination tab's panel"
        );
    }

    #[test]
    fn the_tiling_reads_the_width_of_its_pane_and_not_the_factory_one() {
        // The link between the stored width and what is seen. `panel::set_aside` has its own tests,
        // but handing it the constant instead of the field would leave them all green and the
        // gesture with no effect: it is the pattern of the test that ties down the helper and not
        // the path, caught nine times in this project.
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);

        let cell = with_metrics(CellMetrics::width);
        let its_area = window.active().root.place(area, cell).parts[0];
        let factory = window.active().panes()[0]
            .tile_with_its_panel(its_area, &CellMetrics::of(cell, 16.0), SystemTime::now())
            .panel
            .expect("there is a panel")
            .width();

        window.active_mut().panes_mut()[0].panel_width = 12;
        let custom = window.active().panes()[0]
            .tile_with_its_panel(its_area, &CellMetrics::of(cell, 16.0), SystemTime::now())
            .panel
            .expect("there is a panel")
            .width();

        assert_eq!(
            factory,
            panel::DEFAULT_WIDTH as f32 * cell,
            "the premise failed: an untouched pane did not tile its panel at the factory width"
        );
        assert_eq!(
            custom,
            12.0 * cell,
            "the tiling kept using the factory width instead of this pane's"
        );
    }

    #[test]
    fn only_the_primary_button_narrows_the_panel() {
        // **The middle button is X11's paste and the right one is the selection in almost any
        // terminal**, so neither of the two can move a border. And it is not a precaution: in
        // egui's source, `PointerEvent::Pressed { .. }` matches **without looking at which button
        // it was** (`interaction.rs:162`), and a widget that only senses drags is marked as dragged
        // on the press itself (`interaction.rs:219-220`). The console drag already carries this
        // same guard.
        for which in [egui::PointerButton::Secondary, egui::PointerButton::Middle] {
            let ctx = egui::Context::default();
            let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
            let area = area_of_cells(200.0);
            with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
            one_frame_in(&mut window, &ctx, area, Vec::new());

            let cell = with_metrics(CellMetrics::width);
            let cut_along = panel_cut(&window, 0, area);
            let mid_y = area.center().y;
            one_frame_in(&mut window, &ctx, area, pointer_at(cut_along, mid_y));
            one_frame_in(
                &mut window,
                &ctx,
                area,
                with_button(which, cut_along, mid_y, true),
            );
            one_frame_in(
                &mut window,
                &ctx,
                area,
                pointer_at(cut_along + 12.0 * cell, mid_y),
            );
            one_frame_in(
                &mut window,
                &ctx,
                area,
                with_button(which, cut_along + 12.0 * cell, mid_y, false),
            );

            assert_eq!(
                window.active().panes()[0].panel_width,
                panel::DEFAULT_WIDTH,
                "the panel moved with a button that is not the primary one"
            );
        }
    }

    #[test]
    fn a_press_without_moving_does_not_trample_the_width_the_user_asked_for() {
        // **The destructive case, and it takes a pane where the tiling is clipping.** With the
        // panel asked for at thirty-six cells in a pane that can only give it a few, the handle is
        // painted at the **clipped** cut; if a bare press counted as intent, that cut would be
        // stored and the width the user asked for when the window was wide would be lost for ever —
        // on widening the clipped one would come back, not theirs.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        // Narrow on purpose: each pane gets about 27 cells, so the panel of 36 is clipped down to
        // what is left over from the console's floor.
        let area = area_of_cells(56.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let its_area = window.active().root.place(area, cell).parts[0];
        let panel_split = window.active().panes()[0].tile_with_its_panel(
            its_area,
            &CellMetrics::of(cell, 16.0),
            SystemTime::now(),
        );
        let painted = panel_split.panel.expect("there is a panel").width() / cell;
        assert!(
            painted < panel::DEFAULT_WIDTH as f32,
            "the panel is not clipped ({painted} cells), so the test does not set up the case"
        );

        let cut_along = panel_cut(&window, 0, area);
        let mid_y = area.center().y;
        one_frame_in(&mut window, &ctx, area, pointer_at(cut_along, mid_y));
        one_frame_in(&mut window, &ctx, area, button(cut_along, mid_y, true));
        one_frame_in(&mut window, &ctx, area, button(cut_along, mid_y, false));

        assert_eq!(
            window.active().panes()[0].panel_width,
            panel::DEFAULT_WIDTH,
            "a press without moving was read as «I want this width» and erased the one that had \
             been asked for"
        );
    }

    #[test]
    fn the_stored_width_never_goes_beyond_what_the_tiling_can_give() {
        // **The ceiling is what can be honoured, not what the pane measures**, and the difference
        // shows on widening the window. Dragging all the way to the left, what the user sees is a
        // panel limited by the console's floor; if the pane's whole width were stored, on widening
        // a panel they never saw would appear all at once.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        drag_the_panel(&mut window, &ctx, area, 0, area.left() - 5000.0);

        let cell = with_metrics(CellMetrics::width);
        let its_area = window
            .active()
            .root
            .place(console_area(&window, area), cell)
            .parts[0];
        let of_the_pane =
            with_metrics(|m| m.cells_in(its_area.size() - egui::vec2(16.0, 0.0)).cols) as usize;
        let fits = of_the_pane - panel::MINIMUM_CONSOLE as usize;
        assert_eq!(
            window.active().panes()[0].panel_width,
            fits,
            "{} cells were stored when the tiling can only give {fits} of the pane's {of_the_pane}",
            window.active().panes()[0].panel_width
        );
    }

    #[test]
    fn with_the_panel_at_no_width_the_divider_of_the_tree_still_moves() {
        // **What the panel's handle makes reachable, and it is this project's usual pattern: the
        // defect is not in the new code, it is in what the new code puts one gesture away.** In a
        // pane narrower than its console's floor the tiling leaves the panel with zero width, stuck
        // to the right edge — which is exactly where the divider that tiles the two consoles
        // begins. The panel's handle is registered **after** that divider, and in egui ties are won
        // by the last one, so without the guard the point would go on to resize an invisible panel
        // and the divider that does move something would be left dead.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        // Each pane stays below its console's floor, so its panel has no width.
        let area = area_of_cells(2.0 * panel::MINIMUM_CONSOLE - 4.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let its_area = window
            .active()
            .root
            .place(console_area(&window, area), cell)
            .parts[0];
        let panel_split = window.active().panes()[0].tile_with_its_panel(
            its_area,
            &CellMetrics::of(cell, 16.0),
            SystemTime::now(),
        );
        assert_eq!(
            panel_split
                .panel
                .expect("the rectangle exists, empty though it is")
                .width(),
            0.0,
            "the panel has width, so its handle does not fall on the divider and the test does not \
             set up the case"
        );

        let before = window.active().divider_fractions()[0];
        let divider = window
            .active()
            .root
            .place(console_area(&window, area), cell)
            .dividers[0];
        let grip = divider.rect.center();
        let mid_y = grip.y;
        one_frame_in(&mut window, &ctx, area, pointer_at(grip.x, mid_y));
        one_frame_in(&mut window, &ctx, area, button(grip.x, mid_y, true));
        one_frame_in(
            &mut window,
            &ctx,
            area,
            pointer_at(grip.x - 6.0 * cell, mid_y),
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            button(grip.x - 6.0 * cell, mid_y, false),
        );

        assert_ne!(
            window.active().divider_fractions()[0],
            before,
            "the tree's divider stopped moving: the panel's handle won the point from it"
        );
    }

    #[test]
    fn the_panel_width_survives_switching_tabs_and_moving_the_pane() {
        // **The other two halves of the criterion.** That the width holds up across a split is
        // already proven; these two are the ones that could really lose it, because they **move the
        // console somewhere else**: switching tabs leaves it in the background for a while, and
        // dropping it over another one takes it out of the tree and puts it back in. If the width
        // lived in a parallel list indexed by position, this is where it would come loose.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let cut_along = panel_cut(&window, 0, area);
        drag_the_panel(&mut window, &ctx, area, 0, cut_along + 7.0 * cell);
        let requested = window.active().panes()[0].panel_width;
        assert_ne!(
            requested,
            panel::DEFAULT_WIDTH,
            "the gesture did not change the width, so there is nothing to keep"
        );
        let moved = window.active().panes()[0].name;

        // There and back to the other tab: the console spends a while in the background.
        one_frame_in(&mut window, &ctx, area, chord_with(egui::Key::ArrowRight));
        one_frame_in(&mut window, &ctx, area, Vec::new());
        one_frame_in(&mut window, &ctx, area, chord_with(egui::Key::ArrowLeft));
        assert_eq!(window.active, 0, "it did not come back to the starting tab");
        // **The width its panel is TILED with is what is looked at, not just the number stored.**
        // Asserting on the field would prove that the field survives; what has to be proven is that
        // the panel is still seen with that width, which is what the user asked for. It is the
        // first disguise that the rule about mutating the tests names, and in this project it has
        // already slipped in.
        let of_that_pane = |window: &Window| {
            let cell = with_metrics(CellMetrics::width);
            let tiling = window.active().root.place(area, cell);
            let (which, pane) = window
                .active()
                .panes()
                .into_iter()
                .enumerate()
                .find(|(_, pane)| pane.name == moved)
                .expect("the console is still alive");
            let width = pane
                .tile_with_its_panel(
                    tiling.parts[which],
                    &CellMetrics::of(cell, 16.0),
                    SystemTime::now(),
                )
                .panel
                .expect("its panel still has room")
                .width();
            (pane.panel_width, (width / cell).round() as usize)
        };
        assert_eq!(
            of_that_pane(&window),
            (requested, requested),
            "switching tabs and coming back gave the panel back the factory width"
        );

        // And now it is carried elsewhere: it is dropped over its sibling, which reorders the tree.
        let other = window.active().panes()[1].name;
        let its_area = window.active().root.place(area, cell).parts[1];
        let it_moved = with_metrics(|metrics| {
            window.drop_a_pane(moved, other, Side::Bottom, its_area, metrics)
        });
        assert!(
            it_moved,
            "the console could not be moved, so the test does not pose the case"
        );

        assert_eq!(
            of_that_pane(&window),
            (requested, requested),
            "moving the console elsewhere gave the panel back the factory width"
        );
    }

    #[test]
    fn dragging_the_panel_border_does_not_move_the_pane() {
        // **The other half of keeping the two things apart.** The border's grip sits on top of the
        // console, and the console has felt drags ever since panes could be moved around: the same
        // press reaches two controls. If the console won the gesture, narrowing the panel would end
        // up rearranging the window; if both won it, the two things would happen at once.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let before: Vec<usize> = window
            .active()
            .panes()
            .iter()
            .map(|pane| pane.name)
            .collect();
        let cell = with_metrics(CellMetrics::width);
        let cut_along = panel_cut(&window, 0, area);

        // The border is dragged **into the console next door**, which is where dropping a console
        // would reorder it. Going past the right edge of its pane folds the panel, and that comes
        // in handy: the gesture runs all the way to the end and there is an effect to check.
        drag_the_panel(&mut window, &ctx, area, 0, cut_along + 60.0 * cell);

        let after: Vec<usize> = window
            .active()
            .panes()
            .iter()
            .map(|pane| pane.name)
            .collect();
        assert_eq!(
            before, after,
            "dragging the panel border reordered the consoles"
        );
        assert!(
            window.active().panes()[0].folded,
            "the gesture never took effect, so the test does not pose the case"
        );
    }

    #[test]
    fn the_strip_is_grabbed_at_the_corner_the_folded_panels_handle_would_take() {
        // **The one band that exists to be grabbed, at the one place two controls want.** The
        // panel's handle is registered **after** the console, so it wins every tie; with a folded
        // panel it is stuck inwards from the pane's right edge, which is exactly where the strip's
        // right-hand end is. Cutting the strip after tiling the panel instead of before leaves that
        // handle running the pane's whole height and swallowing that corner of the strip — the
        // delivery calls it true «by construction» and nothing was driving it.
        //
        // **Folded and not merely published**, because that is the sharp case: with a panel on show
        // the handle sits at its left edge, well inside the pane; folded there is no panel to cut
        // at and it goes to the edge itself.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        drag_the_panel(&mut window, &ctx, area, 0, area.right() + 500.0);
        assert!(
            window.active().panes()[0].folded,
            "the panel never folded, so the handle is not where this test presses"
        );

        let metrics = with_metrics(|metrics| metrics.clone());
        let rects = window
            .active()
            .root
            .place(console_area(&window, area), metrics.width())
            .parts
            .clone();
        let strip = header::set_the_header_aside(rects[0], &metrics, window.headers)
            .0
            .expect("the pane carries a strip");
        // The very abscissa the folded handle is built at, written the way the painting writes it:
        // if production moved it, this test would press where there is no contest and would stop
        // posing its case.
        let contested = egui::pos2(
            rects[0].right() - PANEL_GRIP - DIVIDER_THICKNESS,
            strip.center().y,
        );
        let before = names_in_order(&window);
        let onto = rects[1].right_center() - egui::vec2(10.0, 0.0);

        one_frame_in(
            &mut window,
            &ctx,
            area,
            pointer_at(contested.x, contested.y),
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            button(contested.x, contested.y, true),
        );
        one_frame_in(&mut window, &ctx, area, pointer_at(onto.x, onto.y));
        one_frame_in(&mut window, &ctx, area, button(onto.x, onto.y, false));

        let mut moved = before.clone();
        moved.reverse();
        assert_eq!(
            names_in_order(&window),
            moved,
            "the strip's right-hand end did not move its console: the panel's handle is taking the \
             press, and the band that says «grab me here» cannot be grabbed at its own corner"
        );
        assert!(
            window.active().panes()[1].folded,
            "the press unfolded the panel instead of moving the console"
        );
    }

    #[test]
    fn each_pane_folds_its_own_panel_and_not_its_siblings() {
        // Folding belongs to the console, like its width: the point of being able to fold is to
        // close the script's panel and leave the agent's open, in the same tab.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["the first one"]}"#);
        with_the_panel_in_place(&mut window, 1, r#"{"lines":["the second one"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        drag_the_panel(&mut window, &ctx, area, 0, area.right() + 500.0);

        assert!(
            window.active().panes()[0].folded,
            "dragging past the edge did not fold the dragged pane's panel"
        );
        assert!(
            !window.active().panes()[1].folded,
            "folding one pane's panel also folded its sibling's"
        );
    }

    #[test]
    fn folding_does_not_touch_the_saved_width() {
        // **The reason folding is a field apart and not a «zero width».** If folding wrote zero
        // into the width, the working width would be lost and unfolding would have to invent a
        // value. The width is asserted on **while it is folded**, which is where a zero would show.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let cut_along = panel_cut(&window, 0, area);
        drag_the_panel(&mut window, &ctx, area, 0, cut_along + 9.0 * cell);
        let requested = window.active().panes()[0].panel_width;
        assert_eq!(
            requested,
            panel::DEFAULT_WIDTH - 9,
            "the premise failed: the drag did not narrow the panel by nine columns"
        );

        drag_the_panel(&mut window, &ctx, area, 0, area.right() + 500.0);

        assert!(window.active().panes()[0].folded, "it did not fold");
        assert_eq!(
            window.active().panes()[0].panel_width,
            requested,
            "folding clobbered the working width, so unfolding can no longer give it back"
        );
    }

    #[test]
    fn the_folding_survives_moving_the_pane() {
        // The same thing already tied down for the width: folding belongs to the console, not to
        // the place it occupies.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        drag_the_panel(&mut window, &ctx, area, 0, area.right() + 500.0);
        assert!(window.active().panes()[0].folded, "it did not fold");
        let moved = window.active().panes()[0].name;
        let other = window.active().panes()[1].name;

        let cell = with_metrics(CellMetrics::width);
        let its_area = window.active().root.place(area, cell).parts[1];
        let it_moved = with_metrics(|metrics| {
            window.drop_a_pane(moved, other, Side::Bottom, its_area, metrics)
        });
        assert!(
            it_moved,
            "the console did not move, so the test does not pose the case"
        );

        assert!(
            window
                .active()
                .panes()
                .into_iter()
                .find(|pane| pane.name == moved)
                .expect("the console is still alive")
                .folded,
            "moving the console elsewhere unfolded its panel"
        );
    }

    #[test]
    fn a_console_with_no_panel_serves_no_border() {
        // **The application's most common scenario, and therefore the one that matters most:**
        // hardly any console has a `panel.json` published. Without the guard that skips the grip
        // when there is neither a panel **nor** a folding, an invisible control would be registered
        // on the right edge of **every** console, competing for the point with the console itself
        // and with the divider that separates it from its neighbor.
        //
        // **And the gesture goes down the real path, with its four frames, because in a single one
        // it cannot be done.** A **freshly registered** control is never «dragged» on its first
        // pass: egui resolves the interaction at the start of the frame with the previous one's
        // controls. The first version of this test called the method once and passed **by
        // construction** — with the guard and without it. It is the project's third recorded
        // disguise, and here mutation is what exposed it.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        // On purpose: **no console publishes anything**.
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let its_area = window.active().root.place(area, cell).parts[0];
        // Right where the grip would be if this console had a panel, and dragging for real.
        let edge = its_area.right() - PANEL_GRIP;
        let mid_y = its_area.center().y;
        one_frame_in(&mut window, &ctx, area, pointer_at(edge, mid_y));
        one_frame_in(&mut window, &ctx, area, button(edge, mid_y, true));
        one_frame_in(
            &mut window,
            &ctx,
            area,
            pointer_at(edge - 30.0 * cell, mid_y),
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            button(edge - 30.0 * cell, mid_y, false),
        );

        assert_eq!(
            window.active().panes()[0].panel_width,
            panel::DEFAULT_WIDTH,
            "the gesture over a console with no panel changed its width: there is a grip where there \
             should not be one"
        );
        assert!(
            !window.active().panes()[0].folded,
            "the gesture over a console with no panel folded it"
        );
    }

    #[test]
    fn with_the_panel_narrower_than_its_grip_the_tree_divider_still_moves() {
        // **The review found that the guard on the exact zero was not enough, and this is the case
        // it let through.** With a panel two points wide —more than zero, so the old guard did not
        // fire— the eight-point grab band covers the whole line of the divider that tiles the
        // consoles; and since this control is registered afterwards, in egui it wins the tie.
        // Result: the console cannot be widened again, and whoever tries resizes a two-point panel
        // they cannot see.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let cell = with_metrics(CellMetrics::width);
        // Each pane keeps the console's floor plus a hair: the panel has two points to spare.
        // Include whatever width the sidebar geometry reserves so that each console, rather than
        // the whole window, receives this carefully chosen width.
        let console_floor = panel::MINIMUM_CONSOLE * cell + 16.0 + 1.0;
        let base_width = 2.0 * console_floor + DIVIDER_THICKNESS;
        let base_area = area_of(base_width);
        let reserved = base_area.width() - console_area(&window, base_area).width();
        let area = area_of(base_width + reserved);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let its_area = window
            .active()
            .root
            .place(console_area(&window, area), cell)
            .parts[0];
        let panel_width = window.active().panes()[0]
            .tile_with_its_panel(its_area, &CellMetrics::of(cell, 16.0), SystemTime::now())
            .panel
            .expect("the panel's rectangle exists")
            .width();
        assert!(
            panel_width > 0.0 && panel_width <= PANEL_GRIP,
            "the panel measures {panel_width} points: outside the gap between zero and the grip, so \
             the test does not pose the case the review found"
        );

        let before = window.active().divider_fractions()[0];
        let divider = window
            .active()
            .root
            .place(console_area(&window, area), cell)
            .dividers[0];
        let grip = divider.rect.center();
        one_frame_in(&mut window, &ctx, area, pointer_at(grip.x, grip.y));
        one_frame_in(&mut window, &ctx, area, button(grip.x, grip.y, true));
        one_frame_in(
            &mut window,
            &ctx,
            area,
            pointer_at(grip.x + 6.0 * cell, grip.y),
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            button(grip.x + 6.0 * cell, grip.y, false),
        );

        assert_ne!(
            window.active().divider_fractions()[0],
            before,
            "the tree divider stopped moving: the panel's grab band won the point"
        );
    }

    #[test]
    fn a_one_point_tremor_on_pressing_does_not_clobber_the_width_the_user_asked_for() {
        // **The hole the review found in the protection against a dry click.** Demanding «that the
        // mouse has moved» is not enough if one point counts as movement: the grip is drawn where
        // the tiling **clips** the panel, so the hand's tremor on pressing would read that clip as
        // intent and the width asked for with the window wide would be lost forever.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        // Narrow on purpose: the panel of 36 gets clipped down to whatever is left of the console's
        // floor.
        let area = area_of_cells(56.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let cell = with_metrics(CellMetrics::width);
        let its_area = window.active().root.place(area, cell).parts[0];
        let painted = window.active().panes()[0]
            .tile_with_its_panel(its_area, &CellMetrics::of(cell, 16.0), SystemTime::now())
            .panel
            .expect("there is a panel")
            .width()
            / cell;
        assert!(
            painted < panel::DEFAULT_WIDTH as f32,
            "the panel is not clipped ({painted} cells), so the test does not pose the case"
        );

        let cut_along = panel_cut(&window, 0, area);
        let mid_y = area.center().y;
        one_frame_in(&mut window, &ctx, area, pointer_at(cut_along, mid_y));
        one_frame_in(&mut window, &ctx, area, button(cut_along, mid_y, true));
        // **The tremor goes just under half a cell, which is where the previous threshold let it
        // through.** The first version of this test used one point, and with one point it passed
        // **even** with the hole in: what discriminated was «the width asked for differs from the
        // painted one», and any movement of half a cell already makes them different. With the
        // anchor on the stored width, no tremor under half a cell writes anything, be it of one
        // point or of four.
        let tremor = with_metrics(CellMetrics::width) / 2.0 - 0.5;
        assert!(
            tremor > 1.0,
            "this test's tremor measures {tremor} points: with one alone, it would pass just the same \
             with the defect in"
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            pointer_at(cut_along + tremor, mid_y),
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            button(cut_along + tremor, mid_y, false),
        );

        assert_eq!(
            window.active().panes()[0].panel_width,
            panel::DEFAULT_WIDTH,
            "a tremor of {tremor} points read as «I want this width» and erased the one asked for"
        );
    }

    #[test]
    fn a_folded_panel_does_not_wake_the_window_for_going_stale() {
        // **The same rule that already holds for the tabs nobody is looking at**, and its own doc
        // states it: waking the window for a warning nobody is going to see is spending a frame on
        // nothing. Folded, the panel is not painted, so neither is its stale warning — and without
        // this, folding a console whose producer goes quiet leaves the window waking up every
        // minute forever.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        // **The blink is switched off for this one, and it has to be said.** What is asked here is
        // whether the PANEL wakes the window, and a focused console with a blinking cursor wakes it
        // for a reason of its own — `egui::RawInput::default()` reports the window as focused, so a
        // test window is always the blinking case. Left on, this test would answer «yes» for the
        // wrong reason and go on doing so with the panel's alarm deleted.
        window.blink.ceiling = Some(false);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(
            &mut window,
            0,
            r#"{"stale_after_minutes":10,"lines":["working"]}"#,
        );
        one_frame_in(&mut window, &ctx, area, Vec::new());

        // **With the panel in sight it IS armed**, or what is below would tell nothing apart.
        assert!(
            window.active().panes()[0]
                .panel_state
                .how_long_until_warning(SystemTime::now())
                .is_some(),
            "this panel has no stale deadline, so the test does not pose the case"
        );

        drag_the_panel(&mut window, &ctx, area, 0, area.right() + 500.0);
        assert!(window.active().panes()[0].folded, "it did not fold");
        assert!(
            window.active().panes()[0]
                .panel_state
                .how_long_until_warning(SystemTime::now())
                .is_some(),
            "the panel stopped having a deadline for another reason, so what is below does not prove \
             the folding"
        );

        // **The saving deadline is removed too, since it also wakes the window.** Folding the panel
        // changes the layout, so it arms that deadline — on purpose, and declared in D16. This test
        // is about the panel's warning: without removing it, it would measure two things and would
        // not tell which of the two did the waking.
        window.persistence.save_due = None;

        // The context is left at rest and only the half that arms the alarm is run.
        let clean = egui::Context::default();
        for _ in 0..4 {
            let _ = clean.run_logic(&egui::RawInput::default(), |_| {});
        }
        assert!(
            !clean.has_requested_repaint(),
            "the context never gets to be at rest, causes: {:?}",
            clean.repaint_causes()
        );
        window.set_the_alarm(&clean);

        assert!(
            !clean.has_requested_repaint(),
            "folded, the window still schedules a wake-up for a warning that is not painted, \
             causes: {:?}",
            clean.repaint_causes()
        );
    }

    /// Two panes **with their panel in sight**, as the panel gesture receives them.
    fn two_with_panel() -> Vec<(egui::Rect, Option<f32>)> {
        two_side_by_side()
            .into_iter()
            // The border 36 cells of 8 points from the right edge, which is the factory width.
            .map(|rect| (rect, Some(rect.right() - 288.0)))
            .collect()
    }

    #[test]
    fn the_panel_gesture_grabs_its_border_and_carries_it_from_side_to_side() {
        // **The gesture F2.4 measures, and what is tied down is that it comes in where the real
        // mouse comes in:** it grabs the panel's border, moves it, and releases. If it grabbed at
        // any other point, the grip's `interact` would not see it and the batch would be published
        // with no gesture inside.
        let mut gesture =
            InjectedDrags::new(2, InjectedGesture::ResizePanel).expect("two are more than none");
        // **Without this it sends nothing, and that is the contract**: at startup the font cannot
        // be measured, so the cell is set by the painting on the first frame. See
        // `InjectedDrags::cell`.
        gesture.already_knows_the_cell(8.0);
        let painted = two_with_panel();
        let divider = painted[0].1.expect("the first one has a panel");
        let travel = CELLS_THE_PANEL_GESTURE_CROSSES * 8.0;

        let first = one_panel_gesture(&mut gesture, &painted, 0);

        // The press falls **on the border**, not in the middle of the console.
        assert!(
            first[0].iter().any(|event| matches!(
                event,
                egui::Event::PointerButton { pos, pressed: true, .. } if pos.x == divider
            )),
            "the gesture did not grab the panel's border: {:?}",
            first[0]
        );
        // And the movement goes **into the panel**, crossing cells.
        assert!(
            first.iter().flatten().any(|event| matches!(
                event,
                egui::Event::PointerMoved(pos) if pos.x == divider + travel
            )),
            "the gesture did not carry the border the cells it claims to cross"
        );

        // **And the second gesture goes the other way**, which is what returns the width to where
        // it was and keeps the batch from measuring a panel pinned to its minimum for almost the
        // whole run.
        let second = one_panel_gesture(&mut gesture, &painted, 100);
        assert!(
            second.iter().flatten().any(|event| matches!(
                event,
                egui::Event::PointerMoved(pos) if pos.x == divider - travel
            )),
            "the second gesture did not go the other way: the batch would end with the panel at one \
             extreme"
        );
    }

    #[test]
    fn the_panel_gesture_sends_nothing_if_no_pane_has_a_panel() {
        // **The empty list is returned instead of skipping the step**, so that the batch declares
        // zero gestures and the script can invalidate it. With no panel published there is no
        // border to grab, and a batch that stayed still would publish its spans as if the gesture
        // cost nothing.
        let mut gesture =
            InjectedDrags::new(3, InjectedGesture::ResizePanel).expect("three are more than none");
        // The cell is given to it **on purpose**: without it the gesture would send nothing either,
        // and then this test would pass for the wrong reason — what it has to tie down is that
        // **there is no panel**, not that the metric is missing.
        gesture.already_knows_the_cell(8.0);
        let without_panel = two_side_by_side_painted();

        for frame_nr in 0..20 {
            assert!(
                gesture.events(&without_panel, frame_nr).is_empty(),
                "with no panel in sight some event was sent"
            );
        }
        assert_eq!(
            gesture.completed, 0,
            "some gesture was counted without having been sent"
        );
    }

    #[test]
    fn a_panel_gesture_counts_the_effects_it_really_causes() {
        // **The number the report publishes next to the gestures, tied down.** The two runs of the
        // batch gave **exactly twice** as many effects as gestures —1 344 for 671 and 1 756 for
        // 878—, and a proportion that round calls for checking where it comes from before
        // publishing it in an annex: if it were a double count of the same change, the annex would
        // be saying that the gesture causes twice the work it causes.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        window.injected_drags = InjectedDrags::new(1, InjectedGesture::ResizePanel);
        // One frame so that the painting tells it the cell and stores the borders.
        one_frame_in(&mut window, &ctx, area, Vec::new());

        // The whole gesture, down the real path: the raw input hook injects it on every frame.
        let mut frame = eframe::Frame::_new_kittest();
        for _ in 0..=FRAMES_DRAGGING + 1 {
            let mut input = egui::RawInput {
                screen_rect: Some(area),
                ..Default::default()
            };
            eframe::App::raw_input_hook(&mut window, &ctx, &mut input);
            eframe::App::logic(&mut window, &ctx, &mut frame);
            ctx.run_ui(input, |ui| eframe::App::ui(&mut window, ui, &mut frame))
                .drop_without_applying_deltas();
        }

        let injected_drags = window
            .injected_drags
            .as_ref()
            .expect("the batch asked for gestures");
        assert_eq!(
            injected_drags.completed, 1,
            "the whole gesture was not completed"
        );
        assert!(
            injected_drags.with_effect > 0,
            "the gesture was injected and changed no width: the batch would measure nothing"
        );
        // **The exact number, and probing told where it comes from.** One gesture causes **two**
        // width changes because there are two dragging frames and each pushes the border another
        // `CELLS_THE_PANEL_GESTURE_CROSSES`: the width goes 36 → 28 → 20. It is work that really
        // happened —two tilings and two layouts—, not the same thing counted twice, and that is why
        // the report can publish more effects than gestures without inflating anything.
        assert_eq!(
            injected_drags.with_effect, FRAMES_DRAGGING,
            "one gesture caused {} effects and the dragging frames are {FRAMES_DRAGGING}: if they do \
             not match, either there is a change that did not happen or there is one that is not counted",
            injected_drags.with_effect
        );
        // **And the width ended up where the arithmetic says**, which is what keeps the batch from
        // measuring another regime without warning: two pushes of eight cells from the factory
        // width.
        assert_eq!(
            window.active().panes()[0].panel_width,
            panel::DEFAULT_WIDTH - 2 * CELLS_THE_PANEL_GESTURE_CROSSES as usize,
            "the gesture did not leave the width where the count of cells says"
        );
    }

    #[test]
    fn folding_does_not_count_as_a_width_change_in_the_report() {
        // **The number the report publishes as «effects» has to be work that happened.** Folding
        // and unfolding change something —the folded state— but they **do not touch the width**,
        // and the annex reads that number as tilings and layouts. Counting a folding there would be
        // publishing invented work.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        window.injected_drags = InjectedDrags::new(1, InjectedGesture::ResizePanel);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let before = window
            .injected_drags
            .as_ref()
            .expect("there are gestures")
            .with_effect;

        // A folding by hand, down the gesture's path, with no injector: it is dragged past the
        // edge.
        drag_the_panel(&mut window, &ctx, area, 0, area.right() + 500.0);
        assert!(window.active().panes()[0].folded, "it did not fold");

        assert_eq!(
            window
                .injected_drags
                .as_ref()
                .expect("there are gestures")
                .with_effect,
            before,
            "folding was counted as a width change: the report would publish tilings and layouts that \
             did not happen"
        );
    }

    #[test]
    fn the_panel_gesture_releases_and_closes_if_it_loses_its_border_midway() {
        // **Without this the batch hangs in silence.** If the panel stops having a grabbable border
        // midway through a gesture —a smaller window, four panes—, the gesture used to stay stopped
        // forever with the button held: it never sent the release, never advanced, and the batch
        // kept publishing thousands of frames **with no gesture inside** under its label. The
        // script only invalidates when there was none at all, so a run that hung at the third
        // gesture was published as good.
        let mut gesture =
            InjectedDrags::new(3, InjectedGesture::ResizePanel).expect("three are more than none");
        gesture.already_knows_the_cell(8.0);
        let with_panel = two_with_panel();
        let without_panel = two_side_by_side_painted();

        // The gesture starts with the panel in sight…
        let first = gesture.events(&with_panel, 0);
        assert!(!first.is_empty(), "the gesture never got started");
        assert_eq!(
            gesture.completed, 0,
            "the gesture was counted before ending"
        );

        // …and midway the border disappears.
        let closing = gesture.events(&without_panel, 1);

        assert!(
            closing
                .iter()
                .any(|event| matches!(event, egui::Event::PointerButton { pressed: false, .. })),
            "the gesture did not release the button when it lost its border: the batch hangs"
        );
        assert_eq!(
            gesture.completed, 1,
            "the gesture did not close, so the count of injected ones does not advance and the batch \
             looks alive"
        );
    }

    /// How many frames a whole gesture of the tree's divider lasts: approaching, pressing, dragging
    /// and releasing. **The approach goes apart from the press**, and the why is in
    /// `move_a_divider`.
    const FRAMES_OF_A_DIVIDER_GESTURE: usize = FRAMES_DRAGGING + 3;

    /// The events of a whole gesture of the tree's divider, frame by frame.
    fn one_divider_gesture(
        gesture: &mut InjectedDrags,
        painted: &[(egui::Rect, Option<f32>)],
        from: u64,
    ) -> Vec<Vec<egui::Event>> {
        (0..FRAMES_OF_A_DIVIDER_GESTURE)
            .map(|step| gesture.events(painted, from + step as u64))
            .collect()
    }

    #[test]
    fn the_divider_gesture_grabs_between_the_two_consoles_and_alternates_sides() {
        // **What is tied down is WHERE it presses**, because the three gestures come in through the
        // same place and are told apart only by the point: the middle of a console is taken by the
        // drag that moves it, and a point inside the right-hand one is taken by the panel's grip.
        // Only the gap between the two reaches the tree's divider, which is the only thing that
        // changes the width of BOTH.
        let mut gesture =
            InjectedDrags::new(2, InjectedGesture::MoveDivider).expect("two are more than none");
        // Without this it sends nothing: the cell is set by the painting on the first frame. See
        // `InjectedDrags::cell`.
        gesture.already_knows_the_cell(8.0);
        let painted = two_side_by_side_painted();
        let divider = (painted[0].0.right() + painted[1].0.left()) / 2.0;
        let travel = CELLS_THE_DIVIDER_GESTURE_CROSSES * 8.0;

        let first = one_divider_gesture(&mut gesture, &painted, 0);

        // **The approach goes ONLY in the first frame**, without the press: it is what makes the
        // divider win the tie against the consoles. See `move_a_divider`.
        assert!(
            matches!(
                first[0].as_slice(),
                [egui::Event::PointerMoved(pos)] if pos.x == divider
            ),
            "the first frame was not just the approach to the border: {:?}",
            first[0]
        );
        assert!(
            matches!(
                first[1].as_slice(),
                [egui::Event::PointerButton { pos, pressed: true, .. }] if pos.x == divider
            ),
            "the gesture did not grab the tree's divider on the next frame: {:?}",
            first[1]
        );
        // **And not in the middle of any console**, which is where the gesture that moves them
        // grabs: if it fell there, the batch would measure F2.3's gesture under this one's label
        // and the annex would publish the cost of reordering the tree as if it were that of
        // reflowing the text.
        for (pane, _) in &painted {
            assert!(
                !first.iter().flatten().any(|event| matches!(
                    event,
                    egui::Event::PointerButton { pos, pressed: true, .. }
                        if pos.x == pane.center().x
                )),
                "the gesture grabbed by the middle of a console: that is the drag that moves it around"
            );
        }
        // **The whole travel, and counted from where the gesture grabbed.** It is the cells of each
        // dragging frame times the frames it lasts, minus the half thickness at which the border
        // settles: that is where its center ends up.
        let whole_travel = FRAMES_DRAGGING as f32 * travel - DIVIDER_THICKNESS / 2.0;
        assert!(
            first.iter().flatten().any(|event| matches!(
                event,
                egui::Event::PointerMoved(pos) if pos.x == divider + whole_travel
            )),
            "the gesture did not carry the border the cells it claims to cross: {first:?}"
        );

        // **And the second goes the other way**, which is what returns the tiling to where it was
        // and keeps the batch from measuring a console pinned to its minimum for almost the whole
        // run.
        let second = one_divider_gesture(&mut gesture, &painted, 100);
        assert!(
            second.iter().flatten().any(|event| matches!(
                event,
                egui::Event::PointerMoved(pos)
                    if pos.x == divider - FRAMES_DRAGGING as f32 * travel
                        - DIVIDER_THICKNESS / 2.0
            )),
            "the second gesture did not go the other way: the batch would end with the tiling at one \
             extreme"
        );
    }

    #[test]
    fn the_divider_gesture_sends_nothing_without_two_consoles_side_by_side() {
        // **The empty list is returned instead of grabbing anywhere**, so that the batch declares
        // zero gestures and the script can invalidate it. The two cases that do not qualify, and
        // both are reachable by asking for another layout: a single console —there is no border—
        // and two split horizontally, where the point above would fall INSIDE a console and the
        // gesture would drag the console.
        let just_one = vec![two_side_by_side_painted()[0]];
        let one_above_another = vec![
            (
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 300.0)),
                None,
            ),
            (
                egui::Rect::from_min_size(egui::pos2(0.0, 300.0), egui::vec2(1200.0, 300.0)),
                None,
            ),
        ];

        for (with_what, painted) in [
            ("just one", just_one),
            ("one above another", one_above_another),
        ] {
            let mut gesture = InjectedDrags::new(3, InjectedGesture::MoveDivider)
                .expect("three are more than none");
            // The cell is given to it **on purpose**: without it, it would send nothing either, and
            // the test would pass for the wrong reason.
            gesture.already_knows_the_cell(8.0);

            for frame_nr in 0..20 {
                assert!(
                    gesture.events(&painted, frame_nr).is_empty(),
                    "with {with_what} some event was sent"
                );
            }
            assert_eq!(
                gesture.completed, 0,
                "with {with_what} a gesture was counted without sending it"
            );
        }
    }

    #[test]
    fn the_divider_gesture_changes_both_widths_and_reflows_their_text() {
        // **The batch's whole reason for being, tied down by the effect and not by the field where
        // it is recorded.** Neither the panel's gesture nor the one that moves consoles exercises
        // this: the first changes the width of ONE, and the second reorders the tree without
        // crossing cell boundaries. Reflowing the text is paid **per console** and from the
        // scrollback it costs almost a millisecond each time, so the worst case —two at once— is
        // the only number worth the annex.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        // **Two still frames before grabbing anything, and both are needed.** The first measures
        // the grid and tells the gesture the cell; the second is the one that **applies** it to the
        // console, because the size the UI pass measures is delivered in the next one's logic half.
        // With just one, the «before» snapshot was the factory grid —80 columns— and both consoles
        // came out growing, so the check that one gains what the other loses tied nothing down.
        one_frame_in(&mut window, &ctx, area, Vec::new());
        one_frame_in(&mut window, &ctx, area, Vec::new());
        window.injected_drags = InjectedDrags::new(1, InjectedGesture::MoveDivider);
        // And one more with the gesture already set, so the painting tells it the cell and stores
        // the rectangles.
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let before: Vec<_> = window
            .active()
            .panes()
            .iter()
            .map(|pane| pane.screen.size())
            .collect();
        let tiling_before = window.active().divider_fractions();
        let reflows_before = window.reflow.frames;

        // The whole gesture, down the real path: the raw input hook injects it on every frame.
        let mut frame = eframe::Frame::_new_kittest();
        for _ in 0..FRAMES_OF_A_DIVIDER_GESTURE {
            let mut input = egui::RawInput {
                screen_rect: Some(area),
                ..Default::default()
            };
            eframe::App::raw_input_hook(&mut window, &ctx, &mut input);
            eframe::App::logic(&mut window, &ctx, &mut frame);
            ctx.run_ui(input, |ui| eframe::App::ui(&mut window, ui, &mut frame))
                .drop_without_applying_deltas();
        }
        // One more frame: the size the UI pass measures is applied to the console in the logic half
        // of the NEXT one, which is where it reflows.
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let injected_drags = window
            .injected_drags
            .as_ref()
            .expect("the batch asked for gestures");
        assert_eq!(
            injected_drags.completed, 1,
            "the whole gesture was not completed"
        );
        assert!(
            injected_drags.with_effect > 0,
            "the gesture was injected and did not move the divider: the batch would measure nothing"
        );
        assert_ne!(
            window.active().divider_fractions(),
            tiling_before,
            "the divider did not move, so the gesture came in somewhere else"
        );

        let after: Vec<_> = window
            .active()
            .panes()
            .iter()
            .map(|pane| pane.screen.size())
            .collect();
        // **One widens and the other narrows, which is what moving a divider means**: what one
        // gains it takes from the other. Checking only that «both changed» would let a window
        // resize through, which moves both in the same direction and is not this gesture.
        let [(left_before, left_after), (right_before, right_after)] =
            <[_; 2]>::try_from(before.into_iter().zip(after).collect::<Vec<_>>())
                .expect("the batch's tab opens with two consoles");
        assert!(
            (left_after.cols > left_before.cols) != (right_after.cols > right_before.cols),
            "both consoles went the same way —from {left_before:?} and {right_before:?} to \
             {left_after:?} and {right_after:?}—: that is a window resize, not a divider that moves"
        );
        assert_ne!(
            left_before.cols, left_after.cols,
            "the left console kept its columns: this gesture has to change the width of BOTH, which is \
             the only thing that tells it apart from narrowing the panel"
        );
        assert_ne!(
            right_before.cols, right_after.cols,
            "the right console kept its columns: this gesture has to change the width of BOTH, which is \
             the only thing that tells it apart from narrowing the panel"
        );
        assert!(
            window.reflow.frames > reflows_before,
            "no reflow was declared: the work the scrollback made expensive would be published as the \
             framework's cost"
        );
    }

    #[test]
    fn the_divider_gestures_round_trip_leaves_no_console_at_its_floor() {
        // **The number of cells in the travel has to leave margin, and the margin is measured.** If
        // the round trip took a console down to its twenty columns, half the gestures would be
        // no-ops against the geometric floor and the batch would average two regimes under the same
        // label — with the report declaring effects all the same, so no guard of the script would
        // catch it. The real run leaves the narrow console at 23 columns: three above the floor,
        // which means this number **is not raised without coming back here**, and not with a wider
        // factory panel either.
        //
        // It is measured with the batch's window —the 1 280 points of `WINDOW_SIZE`—, because the
        // margin depends on how many cells there are to tile.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of(1280.0);
        // **With a panel published in both, and that is where the whole margin is.** Without a
        // panel the console keeps the whole pane and the floor is nowhere in sight: the first
        // version of this test passed with the travel raised to fourteen cells —twice what is
        // published—, which means it tied nothing down. The batch seeds a panel in all its
        // consoles, and it is the panel that eats the margin.
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        with_the_panel_in_place(&mut window, 1, r#"{"lines":["working"]}"#);
        window.injected_drags = InjectedDrags::new(2, InjectedGesture::MoveDivider);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let mut frame = eframe::Frame::_new_kittest();
        let mut the_least = u16::MAX;
        for _ in 0..2 * FRAMES_OF_A_DIVIDER_GESTURE {
            let mut input = egui::RawInput {
                screen_rect: Some(area),
                ..Default::default()
            };
            eframe::App::raw_input_hook(&mut window, &ctx, &mut input);
            eframe::App::logic(&mut window, &ctx, &mut frame);
            ctx.run_ui(input, |ui| eframe::App::ui(&mut window, ui, &mut frame))
                .drop_without_applying_deltas();
            the_least = the_least.min(
                window
                    .active()
                    .panes()
                    .iter()
                    .map(|pane| pane.screen.size().cols)
                    .min()
                    .expect("the tab opens with two consoles"),
            );
        }

        assert!(
            f32::from(the_least) > panel::MINIMUM_CONSOLE,
            "the round trip left a console at {the_least} columns and its floor is {}: half the \
             gestures would hit it and the batch would measure two regimes",
            panel::MINIMUM_CONSOLE
        );
    }

    #[test]
    fn the_divider_gesture_cycle_returns_the_tiling_to_the_starting_one() {
        // **Without this the batch measures two regimes and publishes the average of both**, and it
        // is not hypothetical: it is the test that exposed that the press was losing the tie
        // against the console next door. When that happened, the gesture dragged a console, the
        // release re-tiled it and the tiling stayed half and half whatever the gesture did — a
        // whole run measuring F2.3's gesture under this one's label.
        //
        // It is the same discipline that
        // `the_cycle_of_two_gestures_returns_to_the_starting_shape_only_with_two_consoles` imposes
        // on the gesture that moves consoles.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        // Four gestures: two whole cycles, to tell «it comes back» from «it comes back the first
        // time».
        window.injected_drags = InjectedDrags::new(4, InjectedGesture::MoveDivider);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        let at_the_start = window.active().divider_fractions();

        // **With margin and not with exact equality, and the margin says how big it is.** The
        // fraction is an `f32` that comes out of a division, so there and back does not give the
        // same bit; what has to be tied down is that nothing **accumulates**. A ten-thousandth is
        // less than two tenths of a point in this test's window —the line being dragged measures
        // ten times more— and the defect this chases moved forty-seven times that in a single
        // cycle.
        let is_back = |current: &[f32], at_the_start: &[f32]| {
            current.len() == at_the_start.len()
                && std::iter::zip(current, at_the_start).all(|(a, b)| (a - b).abs() < 1e-4)
        };
        let mut frame = eframe::Frame::_new_kittest();
        let mut outbound = Vec::new();
        for gesture in 0..4 {
            for _ in 0..FRAMES_OF_A_DIVIDER_GESTURE {
                let mut input = egui::RawInput {
                    screen_rect: Some(area),
                    ..Default::default()
                };
                eframe::App::raw_input_hook(&mut window, &ctx, &mut input);
                eframe::App::logic(&mut window, &ctx, &mut frame);
                ctx.run_ui(input, |ui| eframe::App::ui(&mut window, ui, &mut frame))
                    .drop_without_applying_deltas();
            }
            let current = window.active().divider_fractions();
            if gesture % 2 == 0 {
                // The outbound one has to have moved something, or the return below would be met
                // with no gesture at all.
                assert!(
                    !is_back(&current, &at_the_start),
                    "gesture {gesture} did not move the divider, so the return proves nothing"
                );
                outbound.push(current);
            } else {
                assert!(
                    is_back(&current, &at_the_start),
                    "after cycle {} the divider ended at {current:?} and started from {at_the_start:?}: \
                     the batch would end with a console pinned to its minimum and would measure two \
                     regimes",
                    gesture / 2
                );
            }
        }
        assert!(
            is_back(&outbound[0], &outbound[1]),
            "the two cycles did not reach the same place —{:?} and {:?}—: the gesture drags the tiling \
             little by little and the batch would end in a different regime from the one it started in",
            outbound[0],
            outbound[1]
        );
    }

    #[test]
    fn with_the_panel_below_the_grip_its_border_is_not_reported() {
        // **What the injected gesture reads has to correspond to a grip that EXISTS.** With the
        // panel clipped below the grip, the gesture registers no control —so as not to steal the
        // point from the tree's divider— but the panel's rectangle is still there; reporting its
        // edge all the same made the injected gesture press right where that divider begins, and
        // the batch spent its life dragging the tiling of consoles **under the panel gesture's
        // label**. The script invalidated it for having no effects, but blaming the window instead
        // of the grip.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let cell = with_metrics(CellMetrics::width);
        // The panel has two points to spare: more than zero, less than the grip.
        let area = area_of(2.0 * (panel::MINIMUM_CONSOLE * cell + 16.0 + 1.0) + DIVIDER_THICKNESS);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        // The batch has to ask for gestures, because that is the only thing that stores what was
        // painted.
        window.injected_drags = InjectedDrags::new(1, InjectedGesture::ResizePanel);
        one_frame_in(&mut window, &ctx, area, Vec::new());

        let its_area = window.active().root.place(area, cell).parts[0];
        let width = window.active().panes()[0]
            .tile_with_its_panel(its_area, &CellMetrics::of(cell, 16.0), SystemTime::now())
            .panel
            .expect("the panel's rectangle exists")
            .width();
        assert!(
            width > 0.0 && width <= PANEL_GRIP,
            "the panel measures {width} points: outside the gap between zero and the grip, so the test \
             does not pose the case"
        );

        assert_eq!(
            window
                .last_painted
                .first()
                .and_then(|(_, divider)| *divider),
            None,
            "a border with no grip behind it was reported: the injected gesture would press on the \
             tree's divider and the batch would measure the wrong gesture"
        );
    }

    #[test]
    fn a_folded_panel_whose_producer_goes_quiet_stops_reserving_a_grip() {
        // **The two ways of having no panel, and the sibling test only covered one.** Folded, the
        // grip is reserved so it can be opened again; but if the producer stops publishing while it
        // is folded there is nothing left to show, and that eight-point band would be taking from
        // the console the edge it is grabbed by to be carried away — in exchange for unfolding a
        // panel that does not exist.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        let area = area_of_cells(200.0);
        with_the_panel_in_place(&mut window, 0, r#"{"lines":["working"]}"#);
        one_frame_in(&mut window, &ctx, area, Vec::new());
        drag_the_panel(&mut window, &ctx, area, 0, area.right() + 500.0);
        assert!(window.active().panes()[0].folded, "it did not fold");

        // The producer disappears: with no directory there is nothing to show.
        window.active_mut().panes_mut()[0].panel = None;
        window.active_mut().panes_mut()[0].panel_state = panel::PanelState::default();

        let cell = with_metrics(CellMetrics::width);
        let its_area = window.active().root.place(area, cell).parts[0];
        let edge = its_area.right() - PANEL_GRIP;
        let mid_y = its_area.center().y;
        one_frame_in(&mut window, &ctx, area, pointer_at(edge, mid_y));
        one_frame_in(&mut window, &ctx, area, button(edge, mid_y, true));
        one_frame_in(
            &mut window,
            &ctx,
            area,
            pointer_at(edge - 30.0 * cell, mid_y),
        );
        one_frame_in(
            &mut window,
            &ctx,
            area,
            button(edge - 30.0 * cell, mid_y, false),
        );

        assert!(
            window.active().panes()[0].folded,
            "a panel that no longer exists was unfolded: there was a grip reserved where none is needed"
        );
    }

    #[test]
    fn the_gesture_variable_and_the_words_it_publishes_are_the_contract_with_the_script() {
        // **Nothing tied this down, and the hole is silent: a rename leaves every batch K measuring
        // the J one under K's heading.** The script sets `MEASURE_GESTURE=panel` and then **parses
        // the word** the report publishes, to say it on screen and to invalidate the run; the annex
        // cites it. Three places that have to say the same thing and no test held them together.
        assert_eq!(GESTURE_VARIABLE, "MEASURE_GESTURE");
        assert_eq!(gesture_of(Some("panel")), InjectedGesture::ResizePanel);
        assert_eq!(gesture_of(Some("divider")), InjectedGesture::MoveDivider);
        assert_eq!(gesture_of(Some("mark")), InjectedGesture::Select);
        // With spaces around it, which is what a shell script leaves when it composes the variable.
        assert_eq!(gesture_of(Some("  panel  ")), InjectedGesture::ResizePanel);
        assert_eq!(
            gesture_of(Some("  divider  ")),
            InjectedGesture::MoveDivider
        );
        // **And what is not understood falls into the console's one**, which is what is documented:
        // it is what makes a typo measure the other gesture instead of aborting the startup.
        assert_eq!(gesture_of(Some("panelito")), InjectedGesture::MovePane);
        assert_eq!(gesture_of(Some("")), InjectedGesture::MovePane);
        assert_eq!(gesture_of(None), InjectedGesture::MovePane);

        // The report's word for each of these four gestures, exactly as the script expects them.
        assert_eq!(
            InjectedGesture::ResizePanel.as_published(),
            "narrowing the panel"
        );
        assert_eq!(InjectedGesture::MovePane.as_published(), "moving consoles");
        assert_eq!(
            InjectedGesture::MoveDivider.as_published(),
            "moving the tree divider"
        );
        assert_eq!(InjectedGesture::Select.as_published(), "marking text");
    }

    /// This window's layout, written and read back **going through the text**.
    ///
    /// The long way round is taken —describe, write, interpret, materialize— and not the short one
    /// —describe and validate— on purpose: the text is what really travels, and a round trip that
    /// skipped the JSON would tie down neither the label nor the names of the keys.
    fn wrap_around(window: &Window, ctx: &egui::Context) -> Vec<Tab> {
        let written_text =
            session::serialize(&window.describe()).expect("a live window can be described");
        let read_back =
            session::interpret(&written_text).expect("what the window describes is read back");
        // **A test program and not the user's.** With `Pane::new` these tests would open an
        // interactive shell belonging to whoever runs them for every leaf —a couple of dozen per
        // suite run—, with their startup files and their hooks. That does not test this code.
        //
        // **And a single word, not a command with arguments.** Launching a restored console does
        // not go through any shell: the whole string is looked up as an executable. With `"sleep
        // 30"` nothing was found, every leaf **fell back to the fallback shell** —the opposite of
        // what the paragraph above says— and every run spat out a warning line per console. Two
        // reviews of the milestone exposed it, and it is fixed here because this is the file this
        // change already touches.
        Window::materialize(ctx, &read_back, "cat").expect("the consoles coming back do open")
    }

    /// Drags every divider of the root to its stop, as many passes as it is told.
    ///
    /// **It is the gesture that produces the tiny weights, and there is no shorter one**: the drag
    /// clips the *fraction*, so what it leaves is a part of what the two neighbors add up to — and
    /// that sum shrinks pass by pass. With a single pass it does not get there; the number of
    /// passes **is** the mechanism.
    fn drag_every_divider_to_the_stop(window: &mut Window, passes: usize) {
        let how_many = weights_of_the_root(window).len() - 1;
        for _ in 0..passes {
            for divider in 0..how_many {
                window.active_mut().move_divider(divider, 0.0);
            }
        }
    }

    #[test]
    fn a_window_dragged_to_the_stops_can_still_be_saved_and_read_back() {
        // **AC11, and it is the test that was missing: nobody tied down that what the window
        // describes can be read back.** The other round trips start from trees written by hand with
        // sensible weights. With real gestures you reach weights the format used to reject, the
        // window still looked fine —the column floor rescues the collapsed console— and the layout
        // would have been lost on reopening, in silence.
        let ctx = egui::Context::default();
        // **Five siblings and not four, and the number matters.** Dragging every divider to the
        // stop converges to a fixed point where each weight is **the ninety-ninth part of the
        // next** —the measured ratio is exactly 99.0—, so the smallest is worth `sum · 98/(99ⁿ −
        // 1)`: with four it stays at 4.1·10⁻⁶ —above the minimum the format demanded, so the test
        // would tell nothing apart— and with five it drops to 5.2·10⁻⁸. More passes do not fix
        // four: the fixed point does not depend on them.
        let mut window = window_of_siblings(Orientation::Columns, &[1.0, 1.0, 1.0, 1.0, 1.0]);

        drag_every_divider_to_the_stop(&mut window, 6);

        // First, that the test is on the ground it claims: without a weight below the minimum the
        // format demanded, this would tell nothing apart.
        let weights = weights_of_the_root(&window);
        assert!(
            weights.iter().any(|weight| *weight < 1.0e-6),
            "the drags did not reach tiny weights, so this test proves nothing: {weights:?}"
        );

        let written_text =
            session::serialize(&window.describe()).expect("a live window can be described");
        let read_back = session::interpret(&written_text)
            .expect("what the window describes has to be readable back");
        assert_eq!(
            read_back.leaves(),
            5,
            "the layout read back lost consoles dragged to the stop"
        );
        // And back to consoles, which is the whole path.
        let rounds =
            Window::materialize(&ctx, &read_back, "cat").expect("the consoles coming back do open");
        assert_eq!(
            rounds.len(),
            1,
            "the layout read back did not open as the one tab it was"
        );
    }

    #[test]
    fn a_drag_that_grows_a_weight_does_not_leave_the_layout_unsaved() {
        // **The other side of the same defect, and an adversarial review exposed it.** What a drag
        // keeps is **the sum of the pair**, not each weight: the one that wins climbs to 99 % of
        // what the two added up to. With two large weights coming from a file, a single drag leaves
        // one **above any absolute ceiling** —two of a million give 1.98 million, measured—, and
        // while the format had a ceiling, that was another layout lost in silence on reopening.
        let mut window = window_of_siblings(Orientation::Columns, &[1.0e6, 1.0e6]);

        assert!(
            window.active_mut().move_divider(0, 0.0),
            "the premise failed: the divider did not move"
        );

        let weights = weights_of_the_root(&window);
        assert!(
            weights.iter().any(|weight| *weight > 1.0e6),
            "the drag did not grow any weight, so this test proves nothing: {weights:?}"
        );
        let written_text =
            session::serialize(&window.describe()).expect("a live window can be described");
        session::interpret(&written_text)
            .expect("what the window describes has to be readable back");
    }

    #[test]
    fn dragging_a_long_row_without_stopping_leaves_no_weight_below_the_floor() {
        // **The other half of AC11: the drag is what PUSHES the weights down, and it has no brake
        // of its own.** The fixed point leaves each weight at the ninety-ninth part of the next, so
        // the smallest is worth `sum · 98/(99ⁿ − 1)`: with twenty-two siblings that is 2.7·10⁻⁴¹,
        // that is **below the normal minimum** — numbers whose product reaches zero in a single
        // tiling, and from there to the zero weight the window does not survive.
        //
        // Twenty-two consoles is a lot and on purpose: it is the number at which the floor **does
        // something**. With five the drag stops at 10⁻⁸ and this test would pass with no floor,
        // which is what would make it useless.
        let mut window = window_of_siblings(Orientation::Columns, &[1.0; 22]);

        drag_every_divider_to_the_stop(&mut window, 30);

        let weights = weights_of_the_root(&window);
        assert!(
            weights.iter().all(|weight| weight.is_normal()),
            "the drag left weights the tiling can take to zero: {weights:?}"
        );
        assert!(
            weights.contains(&Weight::SMALLEST),
            "the drag did not reach the floor, so this test does not tell having it from not having it: \
             {weights:?}"
        );

        // **And splitting the one that ended at the floor, which is the next gesture and the other
        // place where the weight gets smaller.** Splitting divides by two: half of a weight that is
        // already at the floor falls below it, and that is where the floor has to act again.
        // Without it, the window is left with two consoles whose weight the tiling can take to
        // zero.
        window
            .active_mut()
            .root
            .split(0, Side::Right, pane_with("cat", 100));

        let after = weights_of_the_root(&window);
        assert_eq!(
            after.len(),
            23,
            "the new console did not come in as a sibling"
        );
        assert!(
            after.iter().all(|weight| weight.is_normal()),
            "splitting the console at the floor left weights the tiling can take to zero: {after:?}"
        );
    }

    #[test]
    fn closing_a_pane_of_tiny_weights_leaves_no_sibling_without_a_weight() {
        // **AC12.** Closing promotes the granddaughters to siblings with the weight scaled
        // —`group_weight · (weight / total)`—, and that product **underflows to zero** with small
        // numbers. A zero weight is what the weight's type exists to make impossible: fed into the
        // tiling, it puts a value that is not a number into a rectangle and the window stops
        // painting. This used to be an `expect`, and the comment beside it claimed that no gesture
        // reached those numbers. **The path is the real one and not a direct call**, because what
        // has to be tied down is that the rescaling happens: closing the outer leaf leaves its
        // branch with a single child, that branch is replaced by it, and then the inner one **is of
        // the same orientation as the root** and its children are promoted to siblings with the
        // weight scaled. That is the product that underflows. **The two ends of the product, and
        // both come from a file the format accepts.** At the bottom the product reaches zero; at
        // the top, `group_weight · w` overflows to infinity **even when the result is
        // representable** —10²⁰ · 10²⁰ does not fit in an `f32`, and the correct tiling is 5·10¹⁹—,
        // which is why the division comes before the multiplication. The minimum expected back
        // tells the two apart: a weight clipped to the floor is as useless as a zero, only without
        // a panic.
        for (group_weight, inner_weight, expected_minimum) in
            [(1.0e-40_f32, 1.0e-40_f32, 0.0_f32), (1.0e20, 1.0e20, 1.0)]
        {
            let mut window = window_of_siblings(Orientation::Columns, &[1.0, group_weight]);
            let Node::Branch { children, .. } = &mut window.active_mut().root else {
                panic!("the window of siblings is born with a branch");
            };
            children[1].node = Node::Branch {
                orientation: Orientation::Rows,
                children: vec![
                    Child {
                        weight: weight(1.0),
                        node: Node::Branch {
                            orientation: Orientation::Columns,
                            children: vec![
                                Child {
                                    weight: weight(inner_weight),
                                    node: Node::Leaf(pane_with("cat", 10)),
                                },
                                Child {
                                    weight: weight(inner_weight),
                                    node: Node::Leaf(pane_with("cat", 11)),
                                },
                            ],
                        },
                    },
                    Child {
                        weight: weight(1.0),
                        node: Node::Leaf(pane_with("cat", 12)),
                    },
                ],
            };

            window
                .active_mut()
                .root
                .close(12)
                .expect("the console was in the tree");

            let weights = weights_of_the_root(&window);
            assert_eq!(
                weights.len(),
                3,
                "the granddaughters were not promoted to siblings"
            );
            assert!(
                weights
                    .iter()
                    .all(|weight| weight.is_normal() && *weight > 0.0),
                "rescaling {group_weight:e} over {inner_weight:e} left a weight that cannot be one: \
                 {weights:?}"
            );
            // The two that were promoted are the last two; the first is the sibling nobody touches.
            assert!(
                weights[1] > expected_minimum && weights[2] > expected_minimum,
                "rescaling {group_weight:e} over {inner_weight:e} clipped where it was not needed: \
                 {weights:?}"
            );
        }
    }

    #[test]
    fn the_round_trip_keeps_the_trees_proportions() {
        // **The starting weights are unequal on purpose**, and with two levels: with equal weights,
        // «the weights are kept» and «it is tiled in equal parts» give exactly the same tree, and
        // the test would stay green with an implementation that threw the weights away — which is
        // precisely the defect that costs most here, because the window looks like the user's
        // without being it.
        let ctx = egui::Context::default();
        let mut window = window_of_siblings(Orientation::Columns, &[3.0, 1.0]);
        // And inside the second one, another cut with different weights: that way it is also tied
        // down that the proportion survives **nested**, and not only at the root.
        let Node::Branch { children, .. } = &mut window.active_mut().root else {
            panic!("the window of siblings is born with a branch");
        };
        children[1].node = Node::Branch {
            orientation: Orientation::Rows,
            children: vec![
                Child {
                    weight: weight(2.0),
                    node: Node::Leaf(pane_with("sleep 30", 7)),
                },
                Child {
                    weight: weight(1.0),
                    node: Node::Leaf(pane_with("sleep 30", 8)),
                },
            ],
        };

        let rounds = wrap_around(&window, &ctx);

        let Node::Branch {
            orientation,
            children,
            ..
        } = &rounds[0].root
        else {
            panic!("the tab coming back has to bring its branch");
        };
        assert_eq!(
            *orientation,
            Orientation::Columns,
            "the root's cut came back with its orientation turned"
        );
        assert_eq!(
            children
                .iter()
                .map(|h| h.weight.value())
                .collect::<Vec<_>>(),
            vec![3.0, 1.0],
            "the root's weights have to come back as they were"
        );

        let Node::Branch {
            orientation,
            children,
            ..
        } = &children[1].node
        else {
            panic!("the inner cut has to come back still being a cut");
        };
        assert_eq!(
            *orientation,
            Orientation::Rows,
            "the inner cut came back with its orientation turned"
        );
        assert_eq!(
            children
                .iter()
                .map(|h| h.weight.value())
                .collect::<Vec<_>>(),
            vec![2.0, 1.0],
            "and the inner ones too"
        );
    }

    #[test]
    fn the_round_trip_keeps_each_consoles_width_and_folding() {
        // **Four different widths, and one of them the factory one on purpose.** With all four
        // different from the factory one, an implementation that restored three well and dropped
        // one would still fall; but with none equal to the factory one, **one that restored nothing
        // and left all four at the factory width** would fall too, for the wrong reason. With one
        // equal, the test tells «it restored» from «the factory one came out».
        let ctx = egui::Context::default();
        let mut window = window_of_siblings(Orientation::Columns, &[1.0, 1.0, 1.0, 1.0]);
        let widths = [52, panel::DEFAULT_WIDTH, 24, 40];
        let folded = [false, false, true, false];
        for (which, pane) in window.active_mut().panes_mut().into_iter().enumerate() {
            pane.panel_width = widths[which];
            pane.folded = folded[which];
        }

        let rounds = wrap_around(&window, &ctx);

        let back = rounds[0].panes();
        assert_eq!(
            back.iter().map(|pane| pane.panel_width).collect::<Vec<_>>(),
            widths.to_vec(),
            "every console has to come back with ITS width, not with one for all of them"
        );
        assert_eq!(
            back.iter().map(|pane| pane.folded).collect::<Vec<_>>(),
            folded.to_vec(),
            "and with its folding, which goes apart from the width"
        );
    }

    #[test]
    fn the_tabs_all_come_back_and_in_their_order() {
        // **Every tab has a different shape, and a review asked for that.** The first version set
        // up three identical tabs and asserted that their names were `[0, 1, 2]` — but those names
        // **are handed out by the very loop that materializes**, so they held even if the tabs came
        // back the other way round. That was asserting on the field where the value is written and
        // not on its effect, which is the first disguise this project has on record. With different
        // panel widths, the order is read from the **shape** and an inversion falls.
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
            ["sleep 30", "sleep 30"],
        ]);
        let marks = [21, 22, 23];
        for (which, tab) in window.tabs.iter_mut().enumerate() {
            for pane in tab.panes_mut() {
                pane.panel_width = marks[which];
            }
        }

        let rounds = wrap_around(&window, &ctx);

        assert_eq!(
            rounds.len(),
            3,
            "the round trip did not bring back exactly the three tabs"
        );
        let as_they_came_back: Vec<usize> = rounds
            .iter()
            .map(|tab| tab.panes()[0].panel_width)
            .collect();
        assert_eq!(
            as_they_came_back,
            marks.to_vec(),
            "the tabs have to come back in their order"
        );
        // And the names are handed out by the window, not by the file: what matters is that **none
        // is repeated**, because a repeated name makes the report publish two blocks of numbers
        // under the same label.
        let mut names: Vec<usize> = rounds.iter().map(|tab| tab.name).collect();
        let tabs = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), tabs, "there are tabs with the same name");
    }

    /// **A strip that was painted is summed into the frame's tally and subtracted from its rest**,
    /// and both numbers are checked exactly.
    ///
    /// **It is driven here and not through a frame, and that is the finding it comes from.** The
    /// window test that drives a real pass asserts `timed_separately >= what the strips cost`, which
    /// is a comparison against an accumulator a dozen other paths also feed — colour glyphs, the tab
    /// bar, reflowing, the pointer. A mutation round found that inequality still holding with the
    /// subtraction deleted: that is **D30**'s second disguise, a margin where the defect and the fix
    /// give the same result. Two exact equalities over the real function do not have it.
    ///
    /// The two mutations it exists to kill, both of which survived the whole suite before it:
    /// deleting the subtraction, which publishes the strips as the interface framework's cost
    /// (**D13**), and turning the sum into a replacement, which publishes what **one** console's
    /// strip cost under a label that says the frame's.
    #[test]
    fn a_strip_that_was_painted_is_summed_into_the_tally_and_out_of_the_frames_rest() {
        let mut strips = report::Strips::default();
        let mut rest = Duration::from_millis(100);

        strips.a_strip_was_painted(&mut rest, Some(Duration::from_millis(3)));
        strips.a_strip_was_painted(&mut rest, Some(Duration::from_millis(5)));
        // **And a pane that cut no strip changes neither number**, which is the branch the walk now
        // leans on: it stopped asking the question at the call site.
        strips.a_strip_was_painted(&mut rest, None);

        assert_eq!(
            strips.pending(),
            Some(Duration::from_millis(8)),
            "the second console's strip replaced the first instead of adding to it: a frame of two \
             would publish half what it really paid, and nothing else in the report would disagree"
        );
        assert_eq!(
            rest,
            Duration::from_millis(108),
            "what the strips cost did not leave the frame's rest, so it is published as the \
             interface framework's cost — the figure D3 judges egui by (D13)"
        );
    }

    /// **A window saved with its strips on comes back with the same tree, and the strip reaches no
    /// file** (AC9).
    ///
    /// **It is true by construction and that is exactly why it is written down.** Nothing in this
    /// milestone touches persistence, so this criterion was the one its delivery plan gave to
    /// nobody — and what this project distrusts most is a promise nothing drives. Two things could
    /// break it without any other test moving: a field for the strip added to
    /// [`session::StoredLeaf`], and a panel width measured against the pane instead of against the
    /// console, which is the very defect the first PR of this milestone existed to prevent.
    ///
    /// **The control is the same layout with the strips off**, and it is what makes the second
    /// assertion mean something: a document that came out identical because nothing was painted at
    /// all would satisfy a round trip on its own.
    ///
    /// **What it does NOT cover, because AC9 does not ask it to**: `describe` reads two fields of a
    /// pane — its panel's width and whether it is folded — so this is blind to anything the strip
    /// does to the console's **geometry**. A mutation that took forty points of width off every
    /// console left it green. That belongs to AC11 and to the pane tests, and saying so here is what
    /// stops the next reader treating this as the guard for it.
    #[test]
    fn a_saved_window_comes_back_with_the_same_tree_and_the_strip_reaches_no_file() {
        let ctx = egui::Context::default();
        let area = area_of(800.0);

        // A layout with something to lose: two consoles, one of them folded and both of them at a
        // width nobody would get back by accident.
        let of_the_session = |headers| {
            let mut window = window_with("sleep 30", "sleep 30");
            window.headers = headers;
            window.active_mut().panes_mut()[0].panel_width = 52;
            window.active_mut().panes_mut()[1].panel_width = 24;
            window.active_mut().panes_mut()[1].folded = true;
            ui_pass(&mut window, &ctx, area, egui::RawInput::default());
            window
        };

        let with_strips = of_the_session(header::Headers::PAINTED);
        let described = with_strips.describe();

        let written = session::serialize(&described).expect("a live window can be described");
        let read_back = session::interpret(&written).expect("what it describes is read back");
        let reopened =
            Window::materialize(&ctx, &read_back, "cat").expect("the consoles coming back do open");

        assert_eq!(
            reopened.iter().map(Tab::describe).collect::<Vec<_>>(),
            described.tabs,
            "a window with a strip over every console did not come back as it was saved"
        );

        // **The control, and the half that says the strip is not persisted.** With the strips off
        // the panes are a row taller each; if any of that reached the file the two documents would
        // differ, and a later milestone's layout would open narrower or wider than it was left.
        assert_eq!(
            of_the_session(header::Headers::NOT_PAINTED).describe(),
            described,
            "what a console's strip takes off its pane reached the layout file"
        );
    }

    /// A valid document of two tabs: the first with two consoles of different widths and one
    /// folded, the second with a single one.
    fn saving_two_tabs() -> session::Validated {
        session::validate(session::StoredDocument {
            v: session::VERSION,
            tabs: vec![
                session::StoredTab {
                    label: None,
                    root: session::StoredNode::Branch {
                        orientation: session::StoredOrientation::Columns,
                        children: vec![
                            session::StoredChild {
                                weight: 3.0,
                                node: session::StoredNode::Leaf(session::StoredLeaf {
                                    resume: None,
                                    cwd: None,
                                    label: None,
                                    panel_width: 52,
                                    folded: false,
                                }),
                            },
                            session::StoredChild {
                                weight: 1.0,
                                node: session::StoredNode::Leaf(session::StoredLeaf {
                                    resume: None,
                                    cwd: None,
                                    label: None,
                                    panel_width: 24,
                                    folded: true,
                                }),
                            },
                        ],
                    },
                },
                session::StoredTab {
                    label: None,
                    root: session::StoredNode::Leaf(session::StoredLeaf {
                        resume: None,
                        cwd: None,
                        label: None,
                        panel_width: 36,
                        folded: false,
                    }),
                },
            ],
        })
        .expect("the test document is valid")
    }

    /// A directory of its own, and the window pointing at a file inside it.
    fn a_window_that_saves(name: &str) -> (Window, std::path::PathBuf, std::path::PathBuf) {
        let base = std::env::temp_dir().join(format!("ivy-saved-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("create the test's directory");
        let path = session::path_from(Some(base.join("config").as_os_str()), None)
            .expect("the test's path is absolute");

        let mut window = window_with("sleep 30", "sleep 30");
        window.persistence = session::Persistence::at(Some(path.clone()));
        (window, base, path)
    }

    #[test]
    fn the_deadline_counts_from_the_first_change_and_is_not_postponed() {
        // **A cap and not a postponement, and it is the difference between saving and never
        // saving.** During a drag the layout changes on **every** frame; rearming the deadline on
        // every change, a ten-second gesture would not write even once, and what the milestone
        // promises is losing half a second at most.
        let mut window = window_with("sleep 30", "sleep 30");

        window.active_mut().move_divider(0, 0.3);
        window.check_whether_the_layout_moved();
        let first = window
            .persistence
            .save_due
            .expect("the first change arms the deadline");

        window.active_mut().move_divider(0, 0.6);
        window.check_whether_the_layout_moved();

        assert_eq!(
            window.persistence.save_due,
            Some(first),
            "the second change postponed the deadline instead of leaving it"
        );
    }

    #[test]
    fn with_no_changes_no_deadline_is_armed() {
        // The other half: if looking always armed the deadline, the window would write twice a
        // second **forever**, without anybody having touched anything.
        let mut window = window_with("sleep 30", "sleep 30");

        window.check_whether_the_layout_moved();

        assert_eq!(window.persistence.save_due, None);
    }

    #[test]
    fn when_the_deadline_expires_the_layout_ends_up_on_disk() {
        let (mut window, base, path) = a_window_that_saves("vence");

        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();
        // The deadline is set in the past instead of really waiting half a second: what is being
        // tested is what happens **on expiry**, not the clock.
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
        window.save_if_due();

        let read_back = session::read(&path).expect("what was saved is read back");
        assert_eq!(
            read_back.tabs(),
            window.describe().tabs.as_slice(),
            "what is on disk is not the layout that is on screen"
        );
        assert_eq!(
            window.saving.writes(),
            1,
            "the expired deadline did not write the layout exactly once"
        );
        // And the deadline is consumed: if it stayed set, it would write on every frame.
        assert_eq!(
            window.persistence.save_due, None,
            "the deadline stayed armed after writing, so it would write on every frame"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_same_thing_is_not_written_twice() {
        // **Against the last thing written successfully, which is the second reference.** Without
        // it, every deadline would rewrite the same document, and with a drag that comes back to
        // its place it would write without anything having really changed.
        let (mut window, base, _) = a_window_that_saves("two-times");

        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
        window.save_if_due();

        // Another expired deadline with no change in between.
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
        window.save_if_due();

        assert_eq!(
            window.saving.writes(),
            1,
            "the same thing that was already on disk was rewritten"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_write_failure_is_retried_with_the_next_change() {
        // **The two references, and why they are two.** The deadline is armed against the last
        // thing **seen** —that way a non-writable directory does not keep the window awake— and the
        // write goes against the last thing **written successfully** —that way a failure is retried
        // with the next change, without storing the error or inventing any wait—.
        let base =
            std::env::temp_dir().join(format!("ivy-saved-{}-impossible", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("create the test's directory");
        // A file where the directory should go: creating the directory fails and there is no way to
        // write.
        let in_the_way = base.join("config");
        std::fs::write(&in_the_way, "I am not a directory").expect("plant the obstruction");
        let path = session::path_from(Some(in_the_way.as_os_str()), None).expect("it is absolute");

        let mut window = window_with("sleep 30", "sleep 30");
        window.persistence = session::Persistence::at(Some(path));

        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
        window.save_if_due();

        assert_eq!(
            window.saving.writes(),
            0,
            "something that never reached the disk was counted as a write"
        );
        assert!(
            window.persistence.last_written.is_none(),
            "what failed was taken as written, so the next change would not retry it"
        );

        // And the next change tries again.
        window.active_mut().move_divider(0, 0.6);
        window.check_whether_the_layout_moved();
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
        window.save_if_due();
        assert_eq!(
            window.saving.attempts(),
            2,
            "the next change did not try again: without this, the failure at ten o'clock would go \
             unretried forever"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_session_that_could_not_restore_does_not_overwrite_what_was_saved() {
        // **The data-loss case the review exposed, and it is the milestone's worst.** If the
        // application reads the file fine and **cannot open it** —an exhausted descriptor, a
        // pseudoterminal the system will not grant—, what is left on screen is a factory-fresh
        // window. With automatic saving, the user's first change would write it **over their twenty
        // consoles**, and there would be no way back: before this milestone the file survived a
        // failed startup and the next one recovered it.
        let (mut window, base, path) = a_window_that_saves("could-not-restore");
        // What had been saved, written beforehand: it is what must not be lost.
        let before = session::serialize(&window.describe()).expect("it can be described");
        std::fs::create_dir_all(path.parent().expect("the directory of its own"))
            .expect("create it");
        std::fs::write(&path, &before).expect("write what was there");

        window
            .persistence
            .forbid_overwrite("the layout that was saved could not be opened");
        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
        window.save_if_due();

        assert_eq!(
            std::fs::read_to_string(&path).expect("it is still there"),
            before,
            "a session that could not restore overwrote the layout it did not know how to open"
        );
        assert_eq!(
            window.saving.writes(),
            0,
            "a session that could not restore counted a write of its layout"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Three changes, each with the deadline it armed expired, and how many of them attempted the
    /// write. **Only a deadline the change armed is expired**: setting one by hand would walk past
    /// the very decision these tests are about.
    fn attempts_over_three_changes(window: &mut Window) -> usize {
        for ratio in [0.25, 0.6, 0.4] {
            window.active_mut().move_divider(0, ratio);
            window.check_whether_the_layout_moved();
            if window.persistence.save_due.is_some() {
                window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
            }
            window.save_if_due();
        }
        let after_the_refusal = window.persistence.save_due;
        window.active_mut().move_divider(0, 0.7);
        window.check_whether_the_layout_moved();
        assert_eq!(
            (after_the_refusal, window.persistence.save_due),
            (None, None),
            "a session that can no longer write still armed a deadline, which wakes the window for \
             an attempt that cannot succeed"
        );
        window.saving.attempts()
    }

    #[test]
    fn a_session_forbidden_to_overwrite_attempts_once_and_then_stops() {
        // **Once, and not zero**: the first attempt is the one that says the line, and every
        // measurement batch runs forbidden to overwrite, so the script's guard needs it. **And not
        // three**: nothing this session does lifts the prohibition, so every attempt after the first
        // is the whole copy, comparison and validation paid for a write that cannot happen — twice a
        // second while a divider is dragged. Three changes and not two, so that «stops after the
        // second» cannot pass either.
        let (mut window, base, _) = a_window_that_saves("forbidden-once");
        window
            .persistence
            .forbid_overwrite("the layout that was saved could not be opened");

        let attempts = attempts_over_three_changes(&mut window);
        let _ = std::fs::remove_dir_all(&base);

        assert_eq!(
            attempts, 1,
            "a refusal that cannot change during the session was attempted {attempts} times"
        );
    }

    #[test]
    fn a_session_with_nowhere_to_write_attempts_once_and_then_stops() {
        // The same rule for the other reason that lasts the session, which comes from the path and
        // not from a prohibition: with no path, no later change gives it one.
        let mut window = window_with("sleep 30", "sleep 30");
        window.persistence = session::Persistence::at(None);

        let attempts = attempts_over_three_changes(&mut window);

        assert_eq!(
            attempts, 1,
            "with nowhere to write, the save was attempted {attempts} times"
        );
    }

    #[test]
    fn a_layout_that_does_not_fit_is_warned_about() {
        // **The only signal before losing the layout**: more consoles than the format admits cannot
        // be saved, and without the line the user finds out on reopening. What is asserted is the
        // decision recorded by the warned-once set, because the error output cannot be read from
        // inside the process.
        let mut window = window_with("sleep 30", "sleep 30");
        window.persistence = session::Persistence::at(None);
        let one_too_many = session::StoredChild {
            weight: 1.0,
            node: session::StoredNode::Leaf(session::StoredLeaf {
                resume: None,
                cwd: None,
                label: None,
                panel_width: 36,
                folded: false,
            }),
        };
        window.last_seen = Some(session::StoredDocument {
            v: session::VERSION,
            tabs: vec![session::StoredTab {
                label: None,
                root: session::StoredNode::Branch {
                    orientation: session::StoredOrientation::Columns,
                    children: vec![one_too_many; session::MAX_LEAVES + 1],
                },
            }],
        });
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
        window.save_if_due();

        assert!(
            window
                .persistence
                .warned_about(&session::SaveFailure::DoesntFit),
            "a layout with more consoles than the format admits was dropped without a warning"
        );
        assert_eq!(
            window.saving.attempts(),
            0,
            "a layout that does not fit reached the write"
        );
    }

    #[test]
    fn closing_the_window_writes_what_was_still_pending() {
        // **Closing the window by hand is not dying**, and it is the only ending in which the half
        // second the delay accepts losing can be not lost. Without this, arranging something and
        // closing right away throws the change away, which is exactly the gesture of somebody who
        // has just left the window the way they want it.
        let (mut window, base, path) = a_window_that_saves("on-closing");
        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();
        assert!(!path.exists(), "it was written ahead of time");

        // The deadline has NOT expired: that is what makes this test tell flushing from waiting.
        window.save_now();

        let read_back = session::read(&path).expect("on closing, what was pending was written");
        assert_eq!(
            read_back.tabs(),
            window.describe().tabs.as_slice(),
            "on closing, the file kept a layout other than the one on screen"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn before_the_deadline_expires_nothing_is_written() {
        // **The deadline itself was tied down by no test, and a review exposed it.** The others set
        // it in the past to test what happens on expiry; deleting the clock check, the window would
        // write **sixty times a second** during a drag and the suite stayed green.
        let (mut window, base, path) = a_window_that_saves("aun-no");
        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();

        window.save_if_due();

        assert_eq!(
            window.saving.writes(),
            0,
            "it was written before the deadline expired"
        );
        assert!(!path.exists(), "the file appeared ahead of time");
        assert!(
            window.persistence.save_due.is_some(),
            "the deadline was consumed without having expired, so it will not try again"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_saving_deadline_wakes_the_window() {
        // **The angle that had no test**, and without it the window would save nothing while
        // covered: the logic half is the only one that runs then, and it starts by setting the
        // deadline to the maximum. Deleting the repaint request, the suite stayed green and saving
        // would sit waiting for somebody to move the mouse.
        let mut window = window_with("sleep 30", "sleep 30");
        let clean = egui::Context::default();
        for _ in 0..4 {
            let _ = clean.run_logic(&egui::RawInput::default(), |_| {});
        }
        assert!(
            !clean.has_requested_repaint(),
            "the context never gets to be at rest, causes: {:?}",
            clean.repaint_causes()
        );

        // With no deadline set, the window has no reason to wake up.
        window.set_the_alarm(&clean);
        assert!(
            !clean.has_requested_repaint(),
            "the window wakes up with nothing to save, causes: {:?}",
            clean.repaint_causes()
        );

        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();
        window.set_the_alarm(&clean);

        assert!(
            clean.has_requested_repaint(),
            "with an unsaved change, the window does not schedule waking up"
        );
    }

    #[test]
    fn a_freshly_opened_window_has_nothing_to_save() {
        // **On the production path and not on the helper**, which is what the review asked for: the
        // test helper duplicates the line that seeds the mirror, so deleting it from the real
        // constructor left the suite green — and **every session would write the file half a second
        // after opening**, without anybody having arranged anything.
        let ctx = egui::Context::default();
        let mut window = Window::opening(
            &ctx,
            OpenWith::Factory {
                tabs: 1,
                layout: OpeningLayout::DEFAULT,
            },
            "cat",
            None,
            session::Persistence::at(None),
        )
        .expect("factory-fresh does open");

        window.check_whether_the_layout_moved();

        assert_eq!(
            window.persistence.save_due, None,
            "a freshly opened window arms a deadline without anybody having touched anything"
        );
    }

    #[test]
    fn the_writing_span_is_subtracted_from_the_rest_of_the_frame() {
        // **The hole an audit of the measurement exposed, and it was the exact asymmetry**: the
        // mirror's span had a test proving it is subtracted and the writing's did not, so deleting
        // its line from the subtraction left the suite green **and the report's row still said it
        // was subtracted**. The milestone's most expensive path —copy, validate, serialize, write
        // and rename— would be published as the framework's cost.
        let (mut window, base, _) = a_window_that_saves("descuento");
        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);

        window.save_if_due();

        let set_aside = window.timed_separately;
        assert!(
            !set_aside.is_zero(),
            "writing set nothing aside, so there is nothing to subtract"
        );
        // The frame's synthetic cost contains the measured write plus a known residue. A fixed
        // total made this assertion depend on the filesystem completing within fifty milliseconds:
        // under load the measured span exceeded it and the expected subtraction underflowed.
        let framework_cost = std::time::Duration::from_millis(7);
        window.active_mut().panes_mut()[0]
            .timings
            .record_round(measure::Spans::of_a_round(
                Some(Instant::now()),
                Instant::now(),
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
            ));
        let mirror = window.mirror_this_frame;
        let cost = set_aside + mirror + framework_cost;

        window.close_the_frame(cost);

        assert_eq!(
            window.rest_of_frame.mean(),
            framework_cost,
            "what writing cost was not subtracted from the rest"
        );
        assert!(
            window.saving.writes() == 1,
            "the report's row does not publish the write that was just made"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn what_it_costs_to_decide_there_is_nothing_to_save_is_declared_too() {
        // **The other hole of the same audit.** When the deadline expires and it turns out there is
        // nothing new, copying the document and comparing it in full **is the whole cost** — and it
        // happens twice a second while dragging. Without declaring it, it is published as the
        // framework's cost.
        let (mut window, base, _) = a_window_that_saves("nothing-new");
        window.active_mut().move_divider(0, 0.25);
        window.check_whether_the_layout_moved();
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);
        window.save_if_due();
        // It is already written; the deadline expires again without anything having changed.
        window.mirror_this_frame = std::time::Duration::ZERO;
        window.persistence.save_due = Some(Instant::now() - SAVE_DELAY);

        window.save_if_due();

        assert!(
            !window.mirror_this_frame.is_zero(),
            "comparing to decide there is nothing to save went undeclared"
        );
        assert_eq!(
            window.saving.writes(),
            1,
            "a deadline in which nothing was written was counted as a write"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn the_mirror_span_is_subtracted_from_the_rest_of_the_frame() {
        // **The project's hard rule**: what is not declared is published as the framework's cost,
        // which is the figure egui is judged by. Comparing the mirror is paid on **every** frame,
        // so without subtracting it the residue would absorb it whole.
        let mut window = window_with("sleep 30", "sleep 30");
        let mirror = std::time::Duration::from_millis(3);
        let cost = std::time::Duration::from_millis(10);
        window.mirror_this_frame = mirror;
        // Any span of a console, or the close records no rest at all.
        window.active_mut().panes_mut()[0]
            .timings
            .record_round(measure::Spans::of_a_round(
                Some(Instant::now()),
                Instant::now(),
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
            ));

        window.close_the_frame(cost);

        assert_eq!(
            window.rest_of_frame.mean(),
            cost - mirror,
            "the mirror's span was not subtracted from the rest"
        );
        assert!(
            window.saving.has_any(),
            "the mirror's span has no row in the report"
        );
    }

    #[test]
    fn the_window_opens_with_the_saved_layout() {
        // **The whole milestone's test seen from outside**: opening with a file in front gives back
        // the tabs, the shape of the tree and **each console's panel**. The three widths are
        // different and one is folded on purpose: with all of them equal, an implementation that
        // dropped the panels along the way would pass just the same.
        let ctx = egui::Context::default();

        let window = Window::opening(
            &ctx,
            OpenWith::Restored(saving_two_tabs()),
            "cat",
            None,
            session::Persistence::at(None),
        )
        .expect("the saved layout does open");

        assert_eq!(window.tabs.len(), 2, "the two tabs did not come back");
        assert_eq!(window.active, 0, "it does not open on the first tab");
        let first_part: Vec<(usize, bool)> = window.tabs[0]
            .panes()
            .into_iter()
            .map(|pane| (pane.panel_width, pane.folded))
            .collect();
        assert_eq!(
            first_part,
            vec![(52, false), (24, true)],
            "the first tab's consoles did not come back with each one's panel"
        );
        let second: Vec<(usize, bool)> = window.tabs[1]
            .panes()
            .into_iter()
            .map(|pane| (pane.panel_width, pane.folded))
            .collect();
        assert_eq!(
            second,
            vec![(36, false)],
            "the second tab's console did not come back with its own panel"
        );
        // And the proportions of the first one's tree, which is what decides the consoles' widths.
        assert_eq!(
            weights_of_the_root(&window),
            vec![3.0, 1.0],
            "the first tab's tree did not come back with its proportions"
        );
    }

    #[test]
    fn only_the_restored_window_declares_where_its_consoles_came_from() {
        // **Without this, the report's row is covered only by whoever calls it by hand.** The
        // report publishes `restored:` out of this field, and the measurement script **invalidates
        // the run** if it does not see it: a restored window that did not fill it in would throw
        // away the whole batch, and a factory-fresh one that filled it in would publish a restored
        // startup that did not happen. Both branches, in the same test.
        let ctx = egui::Context::default();

        let restored = Window::opening(
            &ctx,
            OpenWith::Restored(saving_two_tabs()),
            "cat",
            None,
            session::Persistence::at(None),
        )
        .expect("the saved layout does open");
        let declared = restored
            .restoring
            .expect("a restored window did not say where its consoles came from");
        assert_eq!(
            declared.consoles, 3,
            "the count that is published is not the one of the document that was opened"
        );
        // **And the time, which is the half that was slipping away.** Without this, hard-wiring the
        // stopwatch to zero or starting it after opening the consoles leaves the test green while
        // the report publishes `0ns to open them` for a session that forked three processes. This
        // branch launches `cat` for real.
        assert!(
            declared.opening > std::time::Duration::ZERO,
            "it opened three consoles and claims to have taken zero doing it"
        );
        // **And what was not measured is not published.** This persistence never read the disk —the
        // test builds it empty—, so the report has to say «not measured» instead of a zero that
        // reads as an instantaneous read.
        assert!(
            declared.read.is_none(),
            "it publishes a read time for a file nobody ever read"
        );

        let factory = Window::opening(
            &ctx,
            OpenWith::Factory {
                tabs: 1,
                layout: OpeningLayout::DEFAULT,
            },
            "cat",
            None,
            session::Persistence::at(None),
        )
        .expect("the usual window does open");
        assert!(
            factory.restoring.is_none(),
            "a factory-fresh window claims to have restored consoles"
        );
    }

    #[test]
    fn the_window_that_came_from_a_real_file_publishes_what_reading_it_cost() {
        // **The other half of the number, and the one no test touched.** The one next door opens
        // with a persistence that **never read the disk**, so it can only assert that the report
        // says «not measured»: with `read` hard-wired to `None` in the constructor, that test
        // passes just the same and **every run of batch L would publish «not measured» over a read
        // that did happen**. Here the file is written, loaded and restored, which is the whole path
        // of a real startup.
        //
        // **And it is compared with what the load recorded, not with «something greater than
        // zero».** A bare greater-than-zero is also met by the stopwatch of opening the consoles:
        // without this equality, crossing the two fields publishes the cost of forking three
        // processes under the label of reading a file, which is the mislabelled number this project
        // has already paid for.
        let ctx = egui::Context::default();
        let base =
            std::env::temp_dir().join(format!("ivy-restored-from-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("create the test's directory");
        let path = session::path_from(Some(base.join("config").as_os_str()), None)
            .expect("the test's path is absolute");

        let mut persistence = session::Persistence::at(Some(path));
        assert!(
            persistence.save(&saving_two_tabs()).is_ok(),
            "what the test was going to restore was not saved"
        );
        let saved = persistence
            .load()
            .expect("what had just been saved was not read");
        let read_cost = persistence.load_time();

        let window = Window::opening(&ctx, OpenWith::Restored(saved), "cat", None, persistence)
            .expect("the saved layout does open");

        let declared = window
            .restoring
            .expect("a restored window did not say where its consoles came from");
        assert_eq!(
            declared.read, read_cost,
            "the report does not publish the time the load recorded"
        );
        assert!(
            declared
                .read
                .is_some_and(|how_much| how_much > Duration::ZERO),
            "it read a file from disk and publishes it as «not measured» or as instantaneous: {:?}",
            declared.read
        );
        // **The consoles that are declared are the ones the window opened**, and not a number
        // copied from the document: the script compares this figure with the ones it seeded to
        // decide whether the batch counts.
        let tabs_open: usize = window.tabs.iter().map(|tab| tab.panes().len()).sum();
        assert_eq!(
            declared.consoles, tabs_open,
            "it declares a number of consoles that is not the one of the window it opened"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_panel_width_no_gesture_could_leave_comes_back_grabbable() {
        // **A zero is what a file can bring and a gesture cannot**: asking for less than the drag's
        // floor means **folding**, so the gesture never stores a width below it. Restored as it
        // stands, with the folding false, it leaves a panel of zero points that cannot be grabbed
        // again —grabbing demands width— and the only thing that writes that field is the drag
        // itself: it stays that way forever.
        let ctx = egui::Context::default();
        let saving = session::validate(session::StoredDocument {
            v: session::VERSION,
            tabs: vec![session::StoredTab {
                label: None,
                root: session::StoredNode::Leaf(session::StoredLeaf {
                    resume: None,
                    cwd: None,
                    label: None,
                    panel_width: 0,
                    folded: false,
                }),
            }],
        })
        .expect("a zero is a valid document: the format does not look at the width");

        let window = Window::opening(
            &ctx,
            OpenWith::Restored(saving),
            "cat",
            None,
            session::Persistence::at(None),
        )
        .expect("the saved layout does open");

        let restored = window.tabs[0].panes();
        assert_eq!(
            restored[0].panel_width,
            panel::DRAG_FLOOR,
            "the panel came back with a width that cannot be grabbed"
        );
        assert!(!restored[0].folded, "the folding was not what was saved");
    }

    #[test]
    fn the_restored_window_arms_its_counters_from_what_it_opened() {
        // **The counters come out of the list that was built and not out of whatever the file
        // said**, and this ties it down: with three restored consoles, the next one to be opened
        // cannot be named like any of them. A counter starting at zero would give one of the
        // report's tables the name of another console.
        let ctx = egui::Context::default();

        let window = Window::opening(
            &ctx,
            OpenWith::Restored(saving_two_tabs()),
            "cat",
            None,
            session::Persistence::at(None),
        )
        .expect("the saved layout does open");

        assert_eq!(window.next_pane_name, 3);
        assert_eq!(window.next_name, 2);
    }

    #[test]
    fn with_nothing_saved_the_window_opens_factory_fresh() {
        let ctx = egui::Context::default();

        let window = Window::opening(
            &ctx,
            OpenWith::Factory {
                tabs: 1,
                layout: OpeningLayout::DEFAULT,
            },
            "cat",
            None,
            session::Persistence::at(None),
        )
        .expect("factory-fresh does open");

        assert_eq!(window.tabs.len(), 1);
        assert_eq!(window.tabs[0].panes().len(), 2);
    }

    #[test]
    fn what_a_measurement_batch_asks_for_beats_what_was_saved() {
        // **Without this precedence, a run measures a layout nobody asked it for.** And what
        // decides is that the variable IS THERE: a batch that writes it wrong has asked for
        // something, and its script already invalidates the run by comparing what was asked for
        // with what the report declares.
        let restorable = || Some(saving_two_tabs());

        assert!(matches!(
            open_with(Some("4"), None, restorable()),
            OpenWith::Factory { tabs: 4, .. }
        ));
        assert!(matches!(
            open_with(None, Some("2x2"), restorable()),
            OpenWith::Factory { .. }
        ));
        assert!(matches!(
            open_with(Some("not a number"), None, restorable()),
            OpenWith::Factory { tabs: 1, .. }
        ));
        // And with neither of the two, the file rules.
        assert!(matches!(
            open_with(None, None, restorable()),
            OpenWith::Restored(_)
        ));
        // With no file and no variables, the usual thing.
        assert!(matches!(
            open_with(None, None, None),
            OpenWith::Factory { tabs: 1, .. }
        ));
    }

    #[test]
    fn no_restored_console_is_named_like_another() {
        let ctx = egui::Context::default();
        let window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        let rounds = wrap_around(&window, &ctx);

        let mut names: Vec<usize> = rounds
            .iter()
            .flat_map(|tab| tab.panes().into_iter().map(|pane| pane.name))
            .collect();
        let panes = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            panes,
            "there are repeated names among the consoles"
        );
    }

    #[test]
    fn a_restored_tabs_focus_points_at_one_of_its_own_consoles() {
        // **The focus is not saved, so it comes back on the first one** — and that has to be tied
        // down, because a focus pointing at a name that is not in that tab sends the keys where
        // nobody expects them. It is the same defect the zero as a fallback already caused when
        // opening a tab.
        let ctx = egui::Context::default();
        let window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);

        let rounds = wrap_around(&window, &ctx);

        for tab in &rounds {
            let its_panes: Vec<usize> = tab.panes().iter().map(|pane| pane.name).collect();
            assert!(
                its_panes.contains(&tab.focus),
                "the focus {} belongs to no console of this tab ({its_panes:?})",
                tab.focus
            );
            // **And it is THE FIRST, not any one of its own.** With the membership check alone,
            // putting the focus on the last one passed — and the promise is the same as when
            // opening a new tab: typing before the first click has to reach the leftmost one.
            assert_eq!(
                tab.focus, its_panes[0],
                "the focus has to end up on the tab's first console"
            );
        }
    }

    #[test]
    fn the_orientation_written_to_disk_is_the_one_the_tree_has() {
        // **The round trip CANNOT tie this down, and that is why this separate test is needed.** If
        // **both** orientation conversions are swapped at once, the error is symmetric: describing
        // writes «rows» where there are columns, materializing reads it the other way round, and
        // the round trip gives the correct tree. All green — and the file **incompatible with any
        // other binary**, which is the worst thing that can happen to a format. The only thing that
        // catches it is asserting on the **text**.
        for (orientation, expected) in [
            (Orientation::Columns, "\"orientation\":\"columns\""),
            (Orientation::Rows, "\"orientation\":\"rows\""),
        ] {
            let window = window_of_siblings(orientation, &[1.0, 1.0]);

            let written_text =
                session::serialize(&window.describe()).expect("a live window can be described");

            assert!(
                written_text.contains(expected),
                "a tree of {orientation:?} has to write {expected} and it wrote {written_text}"
            );
        }
    }

    #[test]
    fn a_key_the_format_does_not_know_is_ignored_on_purpose() {
        // **It is ignored, and that is a decision and not an oversight**: D16 says that **adding an
        // optional key does not raise the format's version**, so an old binary has to be able to
        // read a document from a new one that brings one extra key. Rejecting them would be
        // stricter and would contradict that.
        //
        // Without this test, somebody «fixes» the format by adding the rejection of unknown keys
        // and breaks that compatibility without anything complaining.
        let with_one_more = r#"{"v":2,"tabs":[{"root":{"leaf":{"panel_width":36,"folded":false,"whatever_comes_tomorrow":7}}}]}"#;

        let read_back = session::interpret(with_one_more)
            .expect("one extra key does not invalidate the document");
        assert_eq!(read_back.leaves(), 1);
    }

    #[test]
    fn the_description_carries_no_keys_beyond_the_expected_ones() {
        // **The line that keeps launching out of sensitive ground**: the document stores geometry
        // and **never instructions** — no program, no arguments, no environment, no working
        // directory, no paths.
        //
        // **It asserts on the SET of keys and not on what the text does not contain**, and a review
        // decided that. The first version checked that no slash and no occurrence of the test
        // program's name appeared, and that **would not have caught the case it claims to catch**:
        // a `program` field with «bash» inside has no slashes and is not named like the test's
        // program. With the allow list, any new field breaks this test, which is exactly what it is
        // there for.
        let window = window_of_siblings(Orientation::Columns, &[1.0, 1.0]);

        let written_text =
            session::serialize(&window.describe()).expect("a live window can be described");

        let expected = [
            "v",
            "tabs",
            "root",
            "branch",
            "leaf",
            "orientation",
            "children",
            "weight",
            "node",
            "panel_width",
            "folded",
        ];
        // The keys of a JSON with no spaces are what goes between quotes and before a colon.
        let keys: Vec<&str> = written_text
            .match_indices("\":")
            .filter_map(|(end, _)| {
                written_text[..end]
                    .rfind('"')
                    .map(|start| &written_text[start + 1..end])
            })
            .collect();

        for key in &keys {
            assert!(
                expected.contains(key),
                "the document brings a key nobody declared: «{key}» in {written_text}"
            );
        }
        // And the allow list cannot be an empty list in disguise: the document has to bring keys.
        assert!(
            keys.len() > expected.len(),
            "no key was read: {written_text}"
        );
    }

    /// Gives a pane of a tab somewhere to publish, publishes there, and **leaves the flag raised
    /// the way the watcher leaves it**: the read then happens inside the frame, which is what these
    /// tests need to look at.
    fn publish_in_the_tab(window: &mut Window, tab: usize, which: usize, contents: &str) {
        let mut panes = window.tabs[tab].panes_mut();
        let pane = &mut panes[which];
        pane.panel = Some(PanelDir::create().expect("create the pane's directory"));
        publish_at(pane, contents);
        pane.to_reread.raise();
    }

    /// Sends the date of a pane's `panel.json` backwards, so that its panel is born **stale
    /// deterministically**.
    ///
    /// **And not a `stale_after_minutes: 0`**, which is what there used to be: with a zero
    /// deadline, whether the panel counts as old depends on the clock advancing between writing the
    /// file and painting, and that **does not always happen** — the test that used it failed once
    /// in every sixteen runs. An hour of margin against the stock deadline depends on nothing.
    fn age_the_panel(window: &mut Window, tab: usize, which: usize) {
        let panes = window.tabs[tab].panes_mut();
        let path = panes[which]
            .panel
            .as_ref()
            .expect("the pane has a directory")
            .path()
            .join(panel_contract::PANEL_FILE);
        std::fs::File::open(&path)
            .expect("open the panel")
            .set_modified(SystemTime::now() - std::time::Duration::from_secs(60 * 60))
            .expect("move the date backwards");
    }

    /// The claim symbols left queued in the bar, with the card each one fell on.
    ///
    /// They are recognized by their clip, just like the numbers: every card is painted with its own
    /// painter.
    fn attention_marks_on_the_tab_bar(
        shapes: &[egui::epaint::ClippedShape],
        tab_bar: egui::Rect,
    ) -> Vec<(egui::Rect, String, egui::Color32)> {
        shapes
            .iter()
            .filter_map(|clipped| {
                if !tab_bar.contains_rect(clipped.clip_rect) {
                    return None;
                }
                let egui::Shape::Vec(parts) = &clipped.shape else {
                    return None;
                };
                let (symbol, color) = match parts.as_slice() {
                    [egui::Shape::Path(path), egui::Shape::Circle(_)] => {
                        ("?", path.stroke.color.clone())
                    }
                    [egui::Shape::Circle(circle), egui::Shape::LineSegment { .. }] => {
                        ("⊘", egui::epaint::ColorMode::Solid(circle.stroke.color))
                    }
                    [egui::Shape::Rect(rect)] => ("∎", egui::epaint::ColorMode::Solid(rect.fill)),
                    _ => return None,
                };
                let egui::epaint::ColorMode::Solid(color) = color else {
                    return None;
                };
                Some((clipped.clip_rect, symbol.to_owned(), color))
            })
            .collect()
    }

    /// The bar of a window of so many tabs, in the area of these tests.
    fn tab_bar_of(tabs: usize, area: egui::Rect) -> egui::Rect {
        with_metrics(|metrics| {
            set_the_tab_bar_aside(area, tabs, metrics)
                .0
                .expect("with two tabs or more there has to be a bar")
        })
    }

    /// **The milestone's invariant, and it runs A SINGLE frame on purpose.**
    ///
    /// A tab's card says what its panes claim, and this frame's panels are not reread until the end
    /// of the pass: those of the tab that is in sight, when painting them; those of the rest, when
    /// measuring them. With the bar painting itself earlier —as it used to— the card goes **one
    /// frame behind**, and in a reactive window that is not «one frame»: it is forever, because the
    /// frame the watcher asked for is spent painting the old card and nobody asks for another.
    ///
    /// **With two frames this test passes with the defect in**, which is exactly what makes it
    /// necessary: the second paints what the first read.
    #[test]
    fn a_background_tabs_card_claims_in_the_same_frame_its_producer_publishes() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let area = area_of(800.0);
        publish_in_the_tab(
            &mut window,
            1,
            0,
            r#"{"attention":"wait","lines":["whatever"]}"#,
        );

        let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());

        let tab_bar = tab_bar_of(2, area);
        assert_eq!(
            attention_marks_on_the_tab_bar(&shapes, tab_bar),
            vec![(
                tab_rect(tab_bar, 1, 2),
                "?".to_owned(),
                window
                    .theme
                    .attention_color(panel_contract::Attention::Wait, true)
            )],
            "the background tab's card did not say that its console claims you, and there was no need \
             to open it"
        );
    }

    /// **The card shows the LOUDEST claim of its panes**, which is the contract's rule: what waits
    /// for you beats what is stuck, and what is stuck beats what is finished. It comes in through
    /// the painting and not through the helper that compares: what is seen is what has to be tied
    /// down.
    #[test]
    fn the_card_shows_the_loudest_claim_of_its_consoles() {
        let area = area_of(800.0);
        let tab_bar = tab_bar_of(2, area);

        for (one, other, wins, symbol) in [
            ("done", "block", panel_contract::Attention::Block, "⊘"),
            ("block", "wait", panel_contract::Attention::Wait, "?"),
            ("done", "done", panel_contract::Attention::Done, "∎"),
        ] {
            let ctx = egui::Context::default();
            let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
            for (at, which) in [one, other].into_iter().enumerate() {
                publish_in_the_tab(
                    &mut window,
                    0,
                    at,
                    &format!(r#"{{"attention":"{which}","lines":["x"]}}"#),
                );
            }

            let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());

            assert_eq!(
                attention_marks_on_the_tab_bar(&shapes, tab_bar),
                vec![(
                    tab_rect(tab_bar, 0, 2),
                    symbol.to_owned(),
                    window.theme.attention_color(wins, true)
                )],
                "with «{one}» and «{other}» in the same tab «{wins:?}» had to win"
            );
        }
    }

    /// **On a tie of class, the one somebody holds wins.** With two consoles waiting for you and
    /// one of them dead, the card has to say there is somebody waiting **for real**: painting the
    /// dimmed one would send you to the one that no longer answers, which is the worst place a bar
    /// can send you.
    #[test]
    fn between_two_equal_claims_the_card_shows_the_one_someone_holds() {
        // **Both orders, and that is the half that was missing.** Looking for the maximum of a list
        // returns the **last** of the tied ones, so with the live one always in the second pane the
        // test passed **by position** and not by the rule: removing the tie-break from the type's
        // order it stayed green. Checked.
        for dead in [0, 1] {
            let ctx = egui::Context::default();
            let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
            let area = area_of(800.0);
            publish_in_the_tab(
                &mut window,
                0,
                dead,
                r#"{"attention":"wait","lines":["x"]}"#,
            );
            age_the_panel(&mut window, 0, dead);
            publish_in_the_tab(
                &mut window,
                0,
                1 - dead,
                r#"{"attention":"wait","lines":["y"]}"#,
            );

            let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());

            let tab_bar = tab_bar_of(2, area);
            let on_the_tab_bar = attention_marks_on_the_tab_bar(&shapes, tab_bar);
            let [(_, _, color)] = on_the_tab_bar.as_slice() else {
                panic!("there had to be exactly one claim on the bar: {on_the_tab_bar:?}");
            };
            assert_eq!(
                *color,
                window
                    .theme
                    .attention_color(panel_contract::Attention::Wait, true),
                "with the dead one in pane {dead}, the claim nobody holds won and the card sends you \
                 to the console that no longer answers"
            );
        }
    }

    /// **The tab you are looking at is marked just like the rest.** In it the claim may come from
    /// the pane next door, and not marking it would be one more conditional rule — with its test
    /// and its own way of coming out lame.
    #[test]
    fn the_active_tabs_card_is_marked_like_all_the_others() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let area = area_of(800.0);
        publish_in_the_tab(&mut window, 0, 1, r#"{"attention":"block","lines":["x"]}"#);

        let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());

        let tab_bar = tab_bar_of(2, area);
        assert_eq!(
            window.active, 0,
            "the test looks at the active one, and it is zero"
        );
        assert_eq!(
            attention_marks_on_the_tab_bar(&shapes, tab_bar),
            vec![(
                tab_rect(tab_bar, 0, 2),
                "⊘".to_owned(),
                window
                    .theme
                    .attention_color(panel_contract::Attention::Block, true)
            )],
            "the active tab's card was not marked with the claim of its own pane"
        );
    }

    /// **With nobody claiming, no card is marked.** The obvious shortcut for accumulating —looking
    /// for the maximum over the `Option`s as they stand— returns «something that contains nothing»
    /// when they all keep quiet, and whoever read it asking whether there is anything would mark
    /// **every** tab of a window at rest.
    #[test]
    fn with_no_claims_no_card_is_marked() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let area = area_of(800.0);
        // With a panel published and without the field: it is the case that tells «it does not
        // claim» from «there is no panel».
        publish_in_the_tab(
            &mut window,
            0,
            0,
            r#"{"lines":[{"text":"x","mark":"block"}]}"#,
        );

        let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());

        assert_eq!(
            attention_marks_on_the_tab_bar(&shapes, tab_bar_of(2, area)),
            vec![],
            "somebody marked a card without any console claiming anything"
        );
    }

    /// A claim nobody holds reaches the card **dimmed**, just as it reaches the panel: it claims,
    /// and there is nobody behind it. Switching it off altogether would hide the tab, and this
    /// panel confesses instead of hiding.
    #[test]
    fn a_claim_nobody_holds_reaches_the_card_dimmed() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let area = area_of(800.0);
        publish_in_the_tab(&mut window, 1, 0, r#"{"attention":"wait","lines":["x"]}"#);
        age_the_panel(&mut window, 1, 0);

        let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());

        let tab_bar = tab_bar_of(2, area);
        let on_the_tab_bar = attention_marks_on_the_tab_bar(&shapes, tab_bar);
        let [(_, painted_symbol, color)] = on_the_tab_bar.as_slice() else {
            panic!("there had to be exactly one claim on the bar");
        };
        assert_eq!(
            painted_symbol, "?",
            "the card painted another claim than the waiting one its console published"
        );
        assert_eq!(
            *color,
            window
                .theme
                .attention_color(panel_contract::Attention::Wait, false),
            "the claim of a panel that is born stale reached the card as if somebody were holding it"
        );
    }

    /// **What the bar costs is declared, and the report names it.** The frame's cost is apportioned
    /// by subtraction, so a span nobody declares is published as the framework's cost — the figure
    /// D3 judges egui by. And naming it is the other half: put only into the drawer that gets
    /// subtracted, working out what the bar is worth would mean subtracting two different batches.
    ///
    /// **With a single tab there is no bar, and that is the control**: without it, this test would
    /// pass with the painting of the bar removed.
    #[test]
    fn what_the_tab_bar_costs_is_declared_and_the_report_names_it() {
        let ctx = egui::Context::default();
        let area = area_of(800.0);

        let mut one_tab = window_of_tabs([["sleep 30", "sleep 30"]]);
        ui_pass(&mut one_tab, &ctx, area, egui::RawInput::default());
        assert!(
            one_tab.tab_bar_this_frame.is_zero() && one_tab.tab_bar_cost.frames == 0,
            "with a single tab there is no bar to paint, so there is nothing to declare"
        );

        let mut two = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        ui_pass(&mut two, &ctx, area, egui::RawInput::default());

        // What was painted in this pass is **either pending recording, or already recorded**
        // depending on whether the frame got to close; what the test asserts is that it was
        // measured, not which of the two places it fell into.
        assert!(
            two.tab_bar_this_frame + two.tab_bar_cost.total > std::time::Duration::ZERO,
            "the bar was painted in zero time, which is what a span that never got measured publishes"
        );
        assert!(
            two.timed_separately >= two.tab_bar_this_frame,
            "what the bar cost is not inside what gets subtracted from the frame: it would be \
             published as the framework's cost"
        );
    }

    /// **What the strip costs is declared on a frame where its console produced nothing**, which is
    /// the whole reason it is a span of the window and not of a round.
    ///
    /// A round is closed only when bytes arrived — a frame at rest is not a lightning-fast round
    /// trip, it is a round trip that did not happen — so while the strip's cost travelled inside one
    /// it was recorded on the dumping frames alone: a console sitting at a prompt painted its band
    /// on every frame and published none of them, and a quiet control console published «the strip
    /// costs nothing». That is the shape this pass reproduces: nothing has pumped, so no console has
    /// anything pending, and the strips are painted all the same.
    ///
    /// **The `NOT_PAINTED` window is the control**, and without it this would pass with the
    /// painting torn out: what is asserted is that a band that *was* drawn is declared, not that a
    /// counter can be incremented.
    #[test]
    fn what_the_strip_costs_is_declared_on_a_frame_its_console_produced_no_output_in() {
        let ctx = egui::Context::default();
        let area = area_of(800.0);

        let mut window = window_of_tabs([["sleep 30", "sleep 30"]]);
        ui_pass(&mut window, &ctx, area, egui::RawInput::default());

        // **The premise of the test, asserted rather than assumed.** If a console had something
        // pending, the round would close and the old wiring would have declared the strip too — the
        // test would be green over the defect it exists for.
        assert!(
            window
                .active()
                .panes()
                .iter()
                .all(|pane| pane.pending_measurement.is_none()),
            "a console had output pending, so this frame is not the quiet one the defect needs"
        );

        let painted = window
            .strips
            .pending()
            .expect("the strips were painted and nothing recorded what they cost");
        assert!(
            window.timed_separately >= painted,
            "what the strips cost is not inside what gets subtracted from the frame: it would be \
             published as the framework's cost"
        );

        window.close_the_frame(std::time::Duration::from_millis(1));
        assert_eq!(
            window.strips.frames(),
            1,
            "the frame closed without the report being able to say a strip was painted in it"
        );

        // The control: the measurement gate switches the strips off, and then there is nothing to
        // declare because there is nothing on screen.
        let mut without = window_of_tabs([["sleep 30", "sleep 30"]]);
        without.headers = header::Headers::NOT_PAINTED;
        ui_pass(&mut without, &ctx, area, egui::RawInput::default());
        without.close_the_frame(std::time::Duration::from_millis(1));
        assert!(
            without.strips.pending().is_none() && without.strips.frames() == 0,
            "a run with the strips off declared a band nobody painted, and its grid is the one \
             every published batch is compared against"
        );
    }

    /// **That the report publishes what the window measured, and not a blank table.** The
    /// formatting of the line has its test in the measurement box; what this one ties down is **the
    /// wiring**: handing the heading an empty tally instead of the window's left the whole suite
    /// green, the report came out without the line, and the batch's script invalidated it without
    /// anybody knowing why.
    #[test]
    fn the_report_publishes_the_tab_bar_tally_the_window_kept() {
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        window
            .tab_bar_cost
            .record(std::time::Duration::from_millis(7));
        window.tab_bar_cost.saw_marked(2);
        // With nothing measured the report is not published, and then this would pass by not
        // finding a line in an empty text.
        window.tabs[0].panes_mut()[0]
            .timings
            .record_outbound(std::time::Duration::from_micros(10));

        let report = report_of(&window);

        assert!(
            report.contains("tab bar:      1 frames · 7ms in total"),
            "the report did not publish the tally the window was keeping:\n{report}"
        );
        assert!(
            report.contains("2 tabs marked"),
            "the report did not say whether the bar got to mark any card:\n{report}"
        );
    }

    /// **That the report publishes the strip tally the window kept, and the gate the window
    /// resolved** — the wiring, not the wording, whose test is in the measurement box.
    ///
    /// It is the mistake its sibling above already paid for once: handing the heading an empty
    /// tally instead of the window's left the whole suite green, the line came out saying nothing,
    /// and the batch's script threw the run away with nobody able to say why. Here there are two
    /// wires and not one, and they come from different places — the count from what the painting
    /// recorded, the parenthesis from the gate the session resolved — so each is driven apart.
    #[test]
    fn the_report_publishes_the_strip_tally_and_the_gate_the_window_resolved() {
        let mut window = window_with("sleep 30", "sleep 30");
        let mut rest = std::time::Duration::ZERO;
        window
            .strips
            .a_strip_was_painted(&mut rest, Some(std::time::Duration::from_millis(4)));
        window.strips.close_the_frame();
        // **A span is seeded so a report gets published at all.** `publish_the_report` returns
        // early when no console has timed anything, and then the assertions below fail on an empty
        // text — which reads like the wiring being broken and is really the fixture missing.
        window.tabs[0].panes_mut()[0]
            .timings
            .record_outbound(std::time::Duration::from_micros(10));

        let report = report_of(&window);
        assert!(
            report.contains("header:       1 frame(s) with a strip painted · 4ms in total")
                && report.contains("(the session paints them;"),
            "the report did not publish the tally the window was keeping:\n{report}"
        );

        // **The other wire, and the run every published batch is**: the gate switched the strips
        // off, so the parenthesis has to say so however the counter reads.
        window.headers = header::Headers::NOT_PAINTED;
        let vetoed = report_of(&window);
        assert!(
            vetoed.contains("(switched off"),
            "a run with the strips vetoed did not say so, and its grid is the one the published \
             series is compared against:\n{vetoed}"
        );
    }

    /// **The count the report publishes is a count of frames, and nothing drove more than one.**
    /// Every other test closes the frame once, so the whole tally rested on a single sample: a
    /// close that recorded a fixed frame, or one that never emptied what it had already recorded,
    /// publishes the same «1 frame(s)» as the real thing and is found by nobody.
    ///
    /// **The middle frame is the half that protects the other seventeen batches.** The number is
    /// read by `scripts/measure-frame.sh` in both directions, and the direction that fires on a run
    /// which asked for no strip is a count above zero: a close that counted a frame in which no
    /// pane cut a band would invalidate correct runs of every batch there is, and the failure would
    /// arrive as an unexplainable invalidation somewhere else entirely.
    ///
    /// **Asserted over the published line, count and total and mean together, and not over the
    /// counter.** The mean is what a batch is read by, and it is the one that catches a close that
    /// records an empty frame: the total stays put and gets divided by one frame more, which is the
    /// strip published as cheaper than it is.
    #[test]
    fn a_frame_nobody_cut_a_strip_in_is_not_counted_and_two_that_did_are_two() {
        let mut window = window_with("sleep 30", "sleep 30");
        let mut rest = std::time::Duration::ZERO;

        // A frame of two consoles, both of them with a band over the top.
        window
            .strips
            .a_strip_was_painted(&mut rest, Some(Duration::from_millis(3)));
        window
            .strips
            .a_strip_was_painted(&mut rest, Some(Duration::from_millis(5)));
        window.strips.close_the_frame();

        // And a frame in which no pane cut one — the shape of every run with the gate off, and of
        // any frame a pane is too short for the band.
        window.strips.close_the_frame();

        window
            .strips
            .a_strip_was_painted(&mut rest, Some(Duration::from_millis(8)));
        window.strips.close_the_frame();

        // A span is seeded so a report gets published at all: see the test above, which paid for
        // this line once already.
        window.tabs[0].panes_mut()[0]
            .timings
            .record_outbound(std::time::Duration::from_micros(10));

        let report = report_of(&window);
        assert!(
            report.contains(
                "header:       2 frame(s) with a strip painted · 16ms in total · 8ms per frame"
            ),
            "the frames a strip was painted in are not counted one by one, or a frame nobody \
             painted one in was counted: the count is read in both directions and the mean is what \
             the batch is compared by:\n{report}"
        );
    }

    /// **The symbol has to be visible on the active card too, and it very nearly was not.** That
    /// card is filled with the focus color, and against it the claim colors give **1.01 of
    /// contrast** for the red of a block and 1.02 for the dimmed yellow: the same color, as far as
    /// reading it goes. The promise that «the active one is marked too» was met in the queue of
    /// shapes and **not on the screen**.
    ///
    /// What is checked is that behind the symbol the bar's background is painted, which is what it
    /// is read against on every other card. Asserting only the text's color —as the test next door
    /// does— passes with the defect in.
    #[test]
    fn the_active_cards_symbol_is_painted_over_the_background_and_not_over_the_highlight() {
        let ctx = egui::Context::default();
        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        let area = area_of(800.0);
        publish_in_the_tab(&mut window, 0, 0, r#"{"attention":"block","lines":["x"]}"#);

        let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());

        let tab_bar = tab_bar_of(2, area);
        let its_area = tab_rect(tab_bar, 0, 2);
        let backgrounds: Vec<egui::Rect> = shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(painted)
                    if painted.fill == window.theme.background()
                        && its_area.contains_rect(painted.rect) =>
                {
                    Some(painted.rect)
                }
                _ => None,
            })
            .collect();

        let [backing] = backgrounds.as_slice() else {
            panic!(
                "behind the active card's symbol the background was not painted: {backgrounds:?}"
            );
        };
        assert_eq!(
            backing.left(),
            its_area.left(),
            "the backing does not start at its own card"
        );
        assert!(
            backing.contains(egui::pos2(
                its_area.left() + TAB_BAR_MARGIN + 1.0,
                its_area.center().y
            )),
            "the backing does not cover where the symbol falls: {backing:?}"
        );
        assert!(
            backing.width() < its_area.width() / 2.0,
            "the backing ate the whole card and covered the active one's highlight: {backing:?}"
        );
    }

    /// **With THREE consoles claiming different things, the card still shows the one that rules.**
    /// The other tests of the accumulation look at **pairs**, and a pair does not tell «the loudest
    /// claim of them all» from «the loudest claim of the first two» nor from «the claim of the last
    /// one»: with `wait` always in the same place, half a dozen ways of accumulating badly stay
    /// green.
    ///
    /// That is why **all six orderings and not one**: what is tied down is the rule, not the
    /// position — the same disguise that already slipped once into the tie-break test next door.
    #[test]
    fn with_three_consoles_the_card_shows_the_claim_that_rules() {
        for request in [
            ["done", "block", "wait"],
            ["done", "wait", "block"],
            ["block", "done", "wait"],
            ["block", "wait", "done"],
            ["wait", "done", "block"],
            ["wait", "block", "done"],
        ] {
            let ctx = egui::Context::default();
            let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
            // The tabs helper sets up two panes per tab; the third is added here, which is the only
            // thing this criterion asks for and what the pairs never get to say.
            window.tabs[0] = Tab::with_panes(
                (0..3).map(|n| pane_with("sleep 30", n)).collect(),
                &OpeningLayout::DEFAULT,
                0,
            );
            let area = area_of(800.0);
            for (at, which) in request.into_iter().enumerate() {
                publish_in_the_tab(
                    &mut window,
                    0,
                    at,
                    &format!(r#"{{"attention":"{which}","lines":["x"]}}"#),
                );
            }

            let shapes = ui_pass(&mut window, &ctx, area, egui::RawInput::default());

            let tab_bar = tab_bar_of(2, area);
            assert_eq!(
                attention_marks_on_the_tab_bar(&shapes, tab_bar),
                vec![(
                    tab_rect(tab_bar, 0, 2),
                    "?".to_owned(),
                    window
                        .theme
                        .attention_color(panel_contract::Attention::Wait, true)
                )],
                "with «{request:?}» in the same tab «wait» had to win"
            );
        }
    }

    /// **How many cards the bar got to mark is counted by the painting, and the report publishes
    /// it.** It is half of the declaration that says whether a batch measured the bar **with** the
    /// work this milestone added to it: without that number, a run whose producers did not publish
    /// the field publishes the bar's cost without the symbol and reads just like a good one.
    ///
    /// Without this test the counter can be left nailed at zero —checked, the whole suite stays
    /// green—, because the only test that looks at the number sets it by hand before asking for the
    /// report.
    ///
    /// **The control goes inside**: a window at rest walks the bar just the same and marks nothing,
    /// so a counter that counted cards instead of claims also falls here.
    #[test]
    fn the_tab_bar_counts_the_cards_it_actually_marked() {
        let area = area_of(800.0);

        let mut quiet = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        ui_pass(
            &mut quiet,
            &egui::Context::default(),
            area,
            egui::RawInput::default(),
        );
        assert_eq!(
            quiet.tab_bar_cost.marked_tabs, 0,
            "with nobody claiming, the bar said it had marked cards"
        );

        let mut window = window_of_tabs([["sleep 30", "sleep 30"], ["sleep 30", "sleep 30"]]);
        publish_in_the_tab(&mut window, 0, 0, r#"{"attention":"wait","lines":["x"]}"#);
        publish_in_the_tab(&mut window, 1, 1, r#"{"attention":"block","lines":["x"]}"#);

        ui_pass(
            &mut window,
            &egui::Context::default(),
            area,
            egui::RawInput::default(),
        );

        assert_eq!(
            window.tab_bar_cost.marked_tabs, 2,
            "the bar marked both cards and did not declare it: the report publishes its cost as if \
             there had been no claim to accumulate and no symbol to paint"
        );
    }

    /// **Looking at the scrollback belongs to the terminal: nothing reaches the program.** It is
    /// what separates this from sending it the arrows, and it is half of the criterion that gives
    /// the milestone its point — a scrollback that moves the cursor of the editor inside is no use
    /// for reading anything.
    #[test]
    fn scrolling_up_the_scrollback_sends_nothing_to_the_program() {
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        let mut modifiers = egui::Modifiers::NONE;
        for n in 0..80 {
            pane.screen.advance(format!("line {n}\r\n").as_bytes());
        }

        // **«Shift» on its own, which is every terminal's convention.** Ctrl+Shift is the window's
        // prefix here —split, close, switch tab—, so putting the scrollback there would be mixing
        // two languages.
        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::SHIFT),
                egui::Event::Key {
                    key: egui::Key::PageUp,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::SHIFT,
                },
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );

        assert!(
            pane.keystrokes.is_empty(),
            "{:?} reached the program for scrolling up the scrollback",
            String::from_utf8_lossy(&pane.keystrokes)
        );
        assert!(
            pane.screen.view().scroll() > 0,
            "and the view should have gone up"
        );
    }

    /// **Typing returns the view to where the program writes.** Without this, whoever stops to read
    /// something from the scrollback and then types **does not see what they type**: the program
    /// answers down below, off screen, and the window looks hung.
    #[test]
    fn typing_returns_the_view_to_where_the_program_writes() {
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        let mut modifiers = egui::Modifiers::NONE;
        for n in 0..80 {
            pane.screen.advance(format!("line {n}\r\n").as_bytes());
        }
        pane.screen.scroll_by(term_core::Scroll::ToTop);
        assert!(
            pane.screen.view().scroll() > 0,
            "the premise failed: the view did not move up into the scrollback"
        );

        enqueue(&ctx, vec![egui::Event::Text("a".to_owned())]);
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );

        assert_eq!(
            pane.screen.view().scroll(),
            0,
            "it was typed into and the view stayed up top: what is written would not be seen"
        );
    }

    /// The keys of one pass handed to a pane, with the window's own shortcuts.
    fn keys_to(
        pane: &mut Pane,
        ctx: &egui::Context,
        keymap: &keyboard::Keymap,
        events: Vec<egui::Event>,
    ) {
        let mut modifiers = egui::Modifiers::NONE;
        enqueue(ctx, events);
        pane.send_keys(ctx, Instant::now(), &mut modifiers, 1, keymap, true);
    }

    fn search_chord() -> Vec<egui::Event> {
        key_with(egui::Key::F, egui::Modifiers::CTRL | egui::Modifiers::SHIFT)
    }

    /// **AC7, the half that keeps a console from being typed into by accident.** Every key the
    /// search uses is in one batch, so a single one leaking would leave bytes behind — and what is
    /// asserted is the bytes this pass hands the program, which is what `send_keys` writes.
    #[test]
    fn an_open_search_keeps_its_own_keys_from_the_program() {
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        let keymap = keyboard::Keymap::default();
        pane.screen.advance(b"the needle\r\n");

        let mut batch = search_chord();
        batch.push(egui::Event::Text("needle".to_owned()));
        batch.extend(key_with(egui::Key::Backspace, egui::Modifiers::NONE));
        batch.push(egui::Event::Text("e x".to_owned()));
        batch.extend(key_with(egui::Key::W, egui::Modifiers::CTRL));
        batch.extend(key_with(egui::Key::Backspace, egui::Modifiers::CTRL));
        batch.extend(key_with(egui::Key::U, egui::Modifiers::CTRL));
        // The desktop turns the paste chord into a paste before it is a key, so only the change of
        // modifiers and the paste itself arrive.
        batch.push(egui::Event::ModifiersChanged(
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
        ));
        batch.push(egui::Event::Paste("needle".to_owned()));
        batch.extend(key_with(egui::Key::Enter, egui::Modifiers::NONE));
        batch.extend(key_with(egui::Key::Enter, egui::Modifiers::SHIFT));
        keys_to(&mut pane, &ctx, &keymap, batch);

        assert!(
            pane.keystrokes.is_empty(),
            "the search's own keys reached the program: {:?}",
            String::from_utf8_lossy(&pane.keystrokes)
        );
        assert_eq!(
            pane.screen.selected_text().as_deref(),
            Some("needle"),
            "the paste did not land in the query, or Enter did not search with it"
        );

        keys_to(
            &mut pane,
            &ctx,
            &keymap,
            key_with(egui::Key::Escape, egui::Modifiers::NONE),
        );
        assert!(
            pane.keystrokes.is_empty(),
            "the Escape that closes the search reached the program"
        );
        keys_to(
            &mut pane,
            &ctx,
            &keymap,
            vec![egui::Event::Text("x".to_owned())],
        );
        assert_eq!(
            pane.keystrokes, b"x",
            "after closing the search, typing did not reach the program"
        );
    }

    /// **AC7, the half that keeps the search from deafening a console**: a key that is not the
    /// search's is the program's, and getting there closes the search.
    #[test]
    fn a_key_meant_for_the_program_reaches_it_and_closes_the_search() {
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        let keymap = keyboard::Keymap::default();

        let mut batch = search_chord();
        batch.extend(key_with(egui::Key::C, egui::Modifiers::CTRL));
        keys_to(&mut pane, &ctx, &keymap, batch);
        assert_eq!(
            pane.keystrokes,
            [0x03],
            "the interrupt did not reach the program"
        );

        keys_to(
            &mut pane,
            &ctx,
            &keymap,
            vec![egui::Event::Text("x".to_owned())],
        );
        assert_eq!(
            pane.keystrokes, b"x",
            "the search stayed open after a key went to the program"
        );
    }

    /// **AC8**: with the shortcut handed back, its chord is the program's `Ctrl+F`, and no search
    /// opens to swallow what follows.
    #[test]
    fn with_the_search_shortcut_handed_back_its_chord_reaches_the_program() {
        let ctx = egui::Context::default();
        let mut pane = pane_with("sleep 30", 0);
        let mut keymap = keyboard::Keymap::default();
        keymap.bind(keyboard::Action::Search, None);

        let mut batch = search_chord();
        batch.push(egui::Event::Text("x".to_owned()));
        keys_to(&mut pane, &ctx, &keymap, batch);

        assert_eq!(pane.keystrokes, b"\x06x");
    }

    /// **The bench's search gesture, run through real frames, never types into the console.** It is
    /// what batch E's own guard checks after a run, brought into the suite: the first run of the
    /// batch was thrown away because the gesture never released its keys, egui turned every later
    /// chord into a repeat, and the query and the presses went to the program. Frames and not
    /// `send_keys` alone, because the repeat is decided where egui begins a pass.
    #[test]
    fn the_bench_s_search_gesture_never_types_into_the_console() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let mut gesture = injected::InjectedDrags::new(3, injected::InjectedGesture::Search)
            .expect("three gestures were asked for");
        let typed_into = window
            .active()
            .where_the_focus_is()
            .expect("a focused console");
        // A history to scroll into, so the gesture's last step has something to bring back from.
        for n in 0..200 {
            window.active_mut().panes_mut()[typed_into]
                .screen
                .advance(format!("module {n}\r\n").as_bytes());
        }

        for frame in 0..40 {
            let events = gesture.events(&[], frame);
            one_frame_with(&mut window, &ctx, events);
            assert!(
                window.active().panes()[typed_into].keystrokes.is_empty(),
                "frame {frame} of the gesture sent the console {:?}",
                String::from_utf8_lossy(&window.active().panes()[typed_into].keystrokes)
            );
        }

        assert_eq!(gesture.completed, 3, "the gestures did not run to the end");
        assert_eq!(
            window.active().panes()[typed_into].screen.view().scroll(),
            0,
            "the gesture left the console scrolled into the history, painting rows its control never paints"
        );
        assert_eq!(
            window.searching.presses, 24,
            "not every gesture opened the search: its presses did not reach it"
        );
    }

    /// **The keyboard's pass is declared on a frame somebody typed in, and only then (D61, D13).**
    /// Both halves, because each mutation is the other's: declared on every frame inflates every
    /// other declaring test with a non-zero it did not earn, and declared on none publishes a
    /// press's walk over the history as the framework's cost.
    #[test]
    fn the_keyboard_pass_is_declared_when_somebody_typed_and_only_then() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");

        enqueue(&ctx, Vec::new());
        window.send_keys_to_the_focused(&ctx);
        assert!(
            window.timed_separately.is_zero(),
            "a pass nobody typed in declared {:?}",
            window.timed_separately
        );

        let at = window
            .active()
            .where_the_focus_is()
            .expect("a focused console");
        let pane = &mut window.active_mut().panes_mut()[at];
        for n in 0..3_000 {
            pane.screen.advance(format!("line {n}\r\n").as_bytes());
        }
        let mut steps = search::Steps::new(false);
        steps.toggle();
        assert!(steps.took(&egui::Event::Text("absent".into()), egui::Modifiers::NONE));
        search::apply(&mut pane.search, &mut pane.screen, steps);

        enqueue(&ctx, key_with(egui::Key::Enter, egui::Modifiers::NONE));
        window.send_keys_to_the_focused(&ctx);

        let pressing = window.active_mut().panes_mut()[at].search.cost().presses;
        assert_eq!(pressing.count, 1, "the premise: Enter walked the history");
        assert!(
            window.timed_separately >= pressing.total,
            "the pass declared {:?} and its press alone took {:?}: the walk is published as the \
             framework's cost",
            window.timed_separately,
            pressing.total
        );
    }

    /// What one painting of this pane hands the window about searching it.
    fn search_cost_of_painting(pane: &mut Pane) -> search::Cost {
        let ctx = egui::Context::default();
        let mut cost = None;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let metrics =
                CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE));
            let outcome = painter::paint_pane(
                ui,
                pane,
                area_of(800.0),
                &Theme::default(),
                &metrics,
                &mut fonts::Discoverer::default(),
                painter::HowToPaint {
                    integrated: false,
                    maximized: None,
                    blink: painter::Blink {
                        ceiling: Some(false),
                        since_a_key: std::time::Duration::ZERO,
                        keys: painter::Keys::Here,
                    },
                    headers: header::Headers::PAINTED,
                    button: header::Button::in_a_tab_of(&[egui::Rect::NOTHING]),
                    measure_links: false,
                    ligatures: false,
                },
            );
            cost = Some(outcome.search);
        })
        .drop_without_applying_deltas();
        cost.expect("the pane was painted")
    }

    /// **AC11, and the half of D13 a run cannot show**: a console with no search open hands the
    /// window no marking at all, and one with a search open hands it what marking cost and how many
    /// cells it marked — which is what the window subtracts and what the bench's guard reads.
    #[test]
    fn only_a_console_with_a_search_open_pays_for_marking() {
        let mut pane = pane_with("sleep 30", 0);
        pane.screen.advance(b"the needle\r\n");

        assert!(
            search_cost_of_painting(&mut pane).marking.is_none(),
            "a console with no search open declared a marking"
        );

        let mut steps = search::Steps::new(false);
        steps.toggle();
        assert!(steps.took(&egui::Event::Text("needle".into()), egui::Modifiers::NONE));
        search::apply(&mut pane.search, &mut pane.screen, steps);
        let marking = search_cost_of_painting(&mut pane)
            .marking
            .expect("a console with a search open declared no marking");
        assert_eq!(marking.cells, 6, "the visible match was not the one marked");
    }
    crate::windows_unix_omission!(
        opening_the_search_does_not_resize_the_console,
        "unix-only: requires POSIX stty to set terminal modes in the PTY fixture"
    );

    /// **AC9**: opening the search changes nothing the program or the pointer measure from. The
    /// row is painted over the console, and a row cut out of it would resize the program.
    #[cfg(unix)]
    #[test]
    fn opening_the_search_does_not_resize_the_console() {
        let mut pane = pane_with("sleep 30", 0);
        one_frame_over(&mut pane);
        let closed = pane.measured;

        pane.search.open = Some(search::Mode::default());
        one_frame_over(&mut pane);

        assert_eq!(pane.measured, closed);
    }

    /// A window of two consoles where the left one **asks for the mouse** and shows what it
    /// receives.
    ///
    /// **The program inside is real and echoes back what reaches it**, with the control characters
    /// in visible notation. That is what allows asserting on what **the program received** instead
    /// of on the counter where it is recorded that something was sent to it: checked by mutating, a
    /// test that looked at the measurement's series stays green with the bytes going to the wrong
    /// console.
    fn a_pane_that_asks_for_the_mouse(
        rects: &[egui::Rect],
        metrics: &CellMetrics,
        case: &str,
    ) -> Window {
        let mut window = window_with(
            "stty -echo -icanon && printf '\\033[?1000h\\033[?1002h\\033[?1006h' && echo ready && cat -v",
            "sleep 30",
        );
        for (pane, rect) in window.active_mut().panes_mut().into_iter().zip(rects) {
            // **The panel is made visible by PUBLISHING a real one, and the grid is DERIVED from
            // the tiling.** The two go together: a panel nobody published takes up no columns, so
            // without publishing it the console keeps the whole pane and **the panel's strip does
            // not exist** for the test — every press would fall «inside the grid» and the scenario
            // would not happen. Fixing the grid by hand, which is what the helpers of the selection
            // tests do, has the same problem the other way round: it invents a strip the tiling did
            // not put there. It is the hole the previous delivery left declared, and this is what
            // closes it. **The directory carries the test's name, and not just the process's.** The
            // suite runs in parallel inside the same process: without that discriminant, one test's
            // `remove_dir_all` falls between another's `write` and `refresh`, the latter's panel
            // never gets to be seen and its strip —what the test exists to exercise— disappears.
            let dir = std::env::temp_dir().join(format!(
                "ivy-mouse-{}-{case}-{}",
                std::process::id(),
                pane.name
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("create the test panel's directory");
            std::fs::write(
                dir.join("panel.json"),
                br#"{"v":1,"lines":[{"text":"the test panel"}]}"#,
            )
            .expect("publish the test's panel");
            pane.panel_state.refresh(&dir);
            assert!(
                pane.panel_state.is_visible(SystemTime::now()),
                "the test's panel never got to be seen, so its strip does not exist"
            );

            let chrome =
                pane.chrome_within(*rect, metrics, SystemTime::now(), header::Headers::PAINTED);
            pane.measured = metrics.cells_in(chrome.console().size());
            pane.screen.resize(pane.measured);
        }
        let left_pane = &mut window.active_mut().panes_mut()[0];
        wait_for_the_screen_to_say(left_pane, "ready");
        assert!(
            left_pane.screen.mouse_reporting().wants_mouse(),
            "the test's program never got to ask for the mouse"
        );
        confirm_the_console_is_in_the_mode_it_asked_for(left_pane);
        window
    }

    /// Pumps the console until it shows that, or gives up saying what there is.
    fn wait_for_the_pane_to_say(window: &mut Window, which: usize, text: &str) {
        wait_for_this_console_to_say(window.active_mut().panes_mut()[which], text);
    }

    /// The same wait over a loose console, **without clearing the grid**.
    ///
    /// The difference with [`wait_for_the_screen_to_say`] is the one that matters here: that one
    /// starts by replacing the `Screen`, which throws away everything already parsed into it —the
    /// mouse modes among it—, so waiting with it after the program has asked for the mouse would
    /// take the answer away along with the question.
    fn wait_for_this_console_to_say(pane: &mut Pane, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !pane.screen.to_text().contains(text) {
            assert!(
                Instant::now() < deadline,
                "the program did not receive «{text}»; on screen there is: {:?}",
                pane.screen.to_text()
            );
            pane.pump(Default::default());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Confirms the program's terminal really is in the mode its script asked for, by writing a
    /// byte whose fate **depends on that mode**.
    ///
    /// **The ready marker does not prove it, and neither does the exit status.** What these tests
    /// read as «the program got the gesture» is a text the **kernel's own echo** can put there,
    /// character for character the same one — measured: with `stty` missing, fourteen of the
    /// fifteen tests hanging off these two scripts stayed green.
    ///
    /// **So the probe has to be a byte the echo cannot forge, and no control character is one.**
    /// `ECHOCTL` and `cat -v` both write a control character in caret notation, so they are
    /// indistinguishable: `0x1b` comes back `^[` from either. `0x7f` looks like an exception —
    /// canonical mode eats it as the erase key — and it is not: **move the erase key and the echo
    /// renders it `^?` like anything else**, so the probe would pass over the very terminal it
    /// exists to reject. Measured over three pseudoterminals, one per mode.
    ///
    /// A byte **above** `0x7f` has no such twin. `cat -v` writes it in meta notation, `M-^@`, and
    /// the line discipline echoes the raw byte instead — so those four characters on screen can
    /// only have been printed by the `cat -v` that received it, whatever the erase key is. That
    /// matters for F2.7 before it matters here: the erase key is configuration, and the milestone
    /// heading for macOS and Windows is where somebody else's default arrives.
    /// **And the grid is wiped afterwards**, so the probe leaves the console exactly as clean as
    /// the tests below expect. Two columns of residue are not cosmetic here: the screen wraps, and
    /// a report split across two rows stops matching what the test looks for — measured, it broke
    /// two of them.
    fn confirm_the_console_is_in_the_mode_it_asked_for(pane: &mut Pane) {
        pane.pty
            .write(&[0x80])
            .expect("write the byte that probes the mode");
        wait_for_this_console_to_say(pane, "M-^@");
        // Through the parser and not by replacing the `Screen`: what has been parsed into it —the
        // mouse modes, the alternate screen— is precisely what the tests below are about.
        pane.screen.advance(b"\x1b[H\x1b[2J");
    }
    crate::windows_unix_omission!(
        the_navigation_keys_and_the_arrows_reach_a_real_program_in_the_form_it_asked_for,
        "unix-only: requires POSIX stty to set terminal modes in the PTY fixture"
    );

    #[cfg(unix)]
    #[test]
    fn the_navigation_keys_and_the_arrows_reach_a_real_program_in_the_form_it_asked_for() {
        // The program itself turns application cursor keys on, so the mode Home and End are
        // translated against is the one the screen parsed from it — not one the test set by hand.
        // What is read back is what `cat -v` printed, in a mode the probe proved: with the echo on,
        // the kernel would print the same carets and the test would pass without the program.
        let mut pane = pane_with(
            "stty -echo -icanon && printf '\\033[?1h' && echo ready && cat -v",
            0,
        );
        wait_for_this_console_to_say(&mut pane, "ready");
        assert!(
            pane.screen.application_cursor_keys(),
            "premise: the program's CSI ?1h never reached the screen"
        );
        confirm_the_console_is_in_the_mode_it_asked_for(&mut pane);

        let ctx = egui::Context::default();
        let mut modifiers = egui::Modifiers::NONE;
        let plain = |key| key_press(key, egui::Modifiers::NONE);
        enqueue(
            &ctx,
            vec![
                plain(egui::Key::Insert),
                plain(egui::Key::Delete),
                plain(egui::Key::PageUp),
                plain(egui::Key::PageDown),
                plain(egui::Key::Home),
                plain(egui::Key::End),
                plain(egui::Key::ArrowUp),
                egui::Event::ModifiersChanged(egui::Modifiers::CTRL),
                key_press(egui::Key::Delete, egui::Modifiers::CTRL),
                key_press(egui::Key::ArrowLeft, egui::Modifiers::CTRL),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            1,
            &keyboard::Keymap::default(),
            true,
        );

        wait_for_this_console_to_say(&mut pane, "^[[2~^[[3~^[[5~^[[6~^[OH^[OF^[OA^[[3;5~^[[1;5D");
    }

    /// Turns the wheel over that console, with whatever chord it is told.
    ///
    /// **The pointer moves first**: the wheel goes to the console under the pointer, and without
    /// that movement egui has no position to give.
    fn turn_the_wheel(
        window: &mut Window,
        ctx: &egui::Context,
        rects: &[egui::Rect],
        at: egui::Pos2,
        lines: f32,
        chord: egui::Modifiers,
    ) {
        let mut pass = |input: egui::RawInput| {
            ctx.run_ui(input, |ui| {
                window.handle_the_mouse(
                    ui,
                    rects,
                    &CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE)),
                );
            })
            .drop_without_applying_deltas();
        };

        pass(egui::RawInput {
            events: vec![egui::Event::PointerMoved(at)],
            ..Default::default()
        });
        pass(egui::RawInput {
            events: vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, lines),
                modifiers: chord,
                phase: egui::TouchPhase::Move,
            }],
            ..Default::default()
        });
    }

    #[test]
    fn the_wheel_reaches_the_program_that_asked_for_it_and_does_not_move_the_scrollback() {
        // **D23's first branch after «Shift».** It is what makes `htop`'s list go up and down with
        // the wheel; and the scrollback does **not** move, because both things at once would leave
        // the screen scrolling while the program believes its list is what is on view.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "the_wheel_reaches_the_program_that_asked_for_it_and_does_not_move_the_scrollback",
        );
        // **With scrollback behind, and it is not decoration.** With nothing above, scrolling moves
        // nothing, so the assertion on the zero holds just the same with the defect in: checked by
        // mutating —sending the notch to the program AND scrolling— the test stayed green.
        for n in 0..80 {
            window.active_mut().panes_mut()[0]
                .screen
                .advance(format!("line {n}\r\n").as_bytes());
        }

        // **It is turned over a specific cell and THAT cell is asserted.** A program with two areas
        // that scroll —`tmux` with two panes— decides which one it moves by the report's cell: with
        // the corner hard-wired, the wheel would always move the top left one wherever the pointer
        // was looking.
        turn_the_wheel(
            &mut window,
            &ctx,
            &rects,
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * 3.5, metrics.height() * 2.5),
            ),
            3.0,
            egui::Modifiers::NONE,
        );

        wait_for_the_pane_to_say(&mut window, 0, "^[[<64;4;3M");
        assert_eq!(
            window.active().panes()[0].screen.view().scroll(),
            0,
            "the notch was counted to the program and ALSO moved the scrollback"
        );
    }

    #[test]
    fn a_page_of_wheel_reaches_the_program_as_one_notch_per_row() {
        // **A program does not understand «one page»**: what it expects are notches, and the page
        // is measured by the grid it has in front of it. Without this count, a touchpad that sends
        // pages —or any mouse whose desktop emits them— would leave `htop` still while the gesture
        // happens. Checked by mutating: ignoring the pages, no test fell.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "a_page_of_wheel_reaches_the_program_as_one_notch_per_row",
        );
        let rows = usize::from(window.active().panes()[0].screen.size().rows);

        let pass = |window: &mut Window, input: egui::RawInput| {
            ctx.run_ui(input, |ui| {
                window.handle_the_mouse(
                    ui,
                    &rects,
                    &CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE)),
                );
            })
            .drop_without_applying_deltas();
        };
        let over = inside_the_console(
            rects[0],
            &metrics,
            egui::vec2(metrics.width() * 0.5, metrics.height() * 0.5),
        );
        pass(
            &mut window,
            egui::RawInput {
                events: vec![egui::Event::PointerMoved(over)],
                ..Default::default()
            },
        );
        pass(
            &mut window,
            egui::RawInput {
                events: vec![egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Page,
                    delta: egui::vec2(0.0, 1.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                }],
                ..Default::default()
            },
        );

        // **The line breaks are stripped before counting**: the screen wraps, so a report split
        // across two rows would not match and the tally would come out short without anything being
        // wrong.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let pane = &mut window.active_mut().panes_mut()[0];
            let joined = pane.screen.to_text().replace('\n', "");
            let how_many = joined.matches("^[[<64;1;1M").count();
            if how_many >= rows {
                assert_eq!(
                    how_many, rows,
                    "one page sent more notches than the console has rows"
                );
                return;
            }
            assert!(
                Instant::now() < deadline,
                "one page had to be worth {rows} notches and {how_many} arrived"
            );
            pane.pump(Default::default());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn with_shift_the_wheel_belongs_to_the_scrollback_even_if_the_program_asks_for_it() {
        // **The escape hatch that guarantees the console's scrollback is always reachable.** What a
        // pager shows is not what went off the top of the console, so without this branch there are
        // whole sessions —an agent inside a full-screen interface— where there would be no way to
        // see what came before.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "with_shift_the_wheel_belongs_to_the_scrollback_even_if_the_program_asks_for_it",
        );
        for n in 0..80 {
            window.active_mut().panes_mut()[0]
                .screen
                .advance(format!("line {n}\r\n").as_bytes());
        }

        turn_the_wheel(
            &mut window,
            &ctx,
            &rects,
            rects[0].center(),
            3.0,
            egui::Modifiers::SHIFT,
        );

        assert_eq!(
            window.active().panes()[0].screen.view().scroll(),
            3,
            "with Shift the notch went to the program instead of scrolling up the scrollback"
        );
    }
    crate::windows_unix_omission!(
        on_the_alternate_screen_without_mouse_the_wheel_arrives_as_arrows,
        "unix-only: requires POSIX stty to set terminal modes in the PTY fixture"
    );

    #[cfg(unix)]
    #[test]
    fn on_the_alternate_screen_without_mouse_the_wheel_arrives_as_arrows() {
        // **The branch that makes `less` and `man` scroll with the wheel without asking for the
        // mouse.** It is the only one of the four where what comes out is not a report but keys,
        // and that is why it has to be seen: sending the report to a program that did not ask for
        // it fills its screen with rubbish.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with(
            "stty -echo -icanon && printf '\\033[?1049h' && echo ready && cat -v",
            "sleep 30",
        );
        // With no gesture asked for there is no counter to look at, and the assertion below would
        // pass by being empty.
        window.injected_drags = InjectedDrags::new(1, InjectedGesture::MouseToProgram);
        for (pane, rect) in window.active_mut().panes_mut().into_iter().zip(&rects) {
            pane.measured = grid_of(*rect, &metrics);
            pane.screen.resize(pane.measured);
        }
        wait_for_the_pane_to_say(&mut window, 0, "ready");
        assert!(
            window.active().panes()[0].screen.wheel_acts_as_arrows(),
            "the test's program never reached the alternate screen"
        );
        confirm_the_console_is_in_the_mode_it_asked_for(window.active_mut().panes_mut()[0]);

        turn_the_wheel(
            &mut window,
            &ctx,
            &rects,
            rects[0].center(),
            -2.0,
            egui::Modifiers::NONE,
        );

        wait_for_the_pane_to_say(&mut window, 0, "^[[B^[[B");
        // **And they do not count as mouse reports**, which is what the batch reads. What comes out
        // along this path are the same keys the keyboard sends; publishing them under «N mouse
        // reports» put foreign samples into that series **and made the guard fire** that demands
        // zero reports in every batch that is not the mouse's: a correct run with a `less`
        // scrolling was marked invalid.
        assert_eq!(
            window
                .injected_drags
                .as_ref()
                .map(|injected_drags| injected_drags.reported),
            Some(0),
            "the wheel's arrows were counted as mouse reports"
        );
    }
    crate::windows_unix_omission!(
        under_application_cursor_keys_the_wheel_sends_the_arrows_the_program_asked_for,
        "unix-only: requires POSIX stty to set terminal modes in the PTY fixture"
    );

    #[cfg(unix)]
    #[test]
    fn under_application_cursor_keys_the_wheel_sends_the_arrows_the_program_asked_for() {
        // `less` and `vim` send `smkx` on start, so on the alternate screen the wheel almost always
        // meets DECCKM set: the arrows it sends have to be the keyboard's, in that mode.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = window_with(
            "stty -echo -icanon && printf '\\033[?1049h\\033[?1h' && echo ready && cat -v",
            "sleep 30",
        );
        for (pane, rect) in window.active_mut().panes_mut().into_iter().zip(&rects) {
            pane.measured = grid_of(*rect, &metrics);
            pane.screen.resize(pane.measured);
        }
        wait_for_the_pane_to_say(&mut window, 0, "ready");
        assert!(
            window.active().panes()[0].screen.wheel_acts_as_arrows()
                && window.active().panes()[0].screen.application_cursor_keys(),
            "the test's program never reached the alternate screen in application cursor mode"
        );
        confirm_the_console_is_in_the_mode_it_asked_for(window.active_mut().panes_mut()[0]);

        turn_the_wheel(
            &mut window,
            &ctx,
            &rects,
            rects[0].center(),
            -2.0,
            egui::Modifiers::NONE,
        );

        wait_for_the_pane_to_say(&mut window, 0, "^[OB^[OB");
    }

    #[test]
    fn a_click_reaches_the_program_that_asked_for_it_and_with_its_cell() {
        // **AC7.** It is what makes clicking inside `vim` put the cursor where it was clicked, and
        // the cell matters as much as the click: one column too many opens the menu entry next
        // door. The click is on the **fourth column of the third row**, which in the report are 4
        // and 3 because there they are counted from one.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "a_click_reaches_the_program_that_asked_for_it_and_with_its_cell",
        );

        let at = inside_the_console(
            rects[0],
            &metrics,
            egui::vec2(metrics.width() * 3.5, metrics.height() * 2.5),
        );
        drag_to_select(&mut window, &ctx, &rects, at, at);

        wait_for_the_pane_to_say(&mut window, 0, "^[[<0;4;3M");
    }

    #[test]
    fn dragging_inside_a_program_that_asks_for_the_mouse_does_not_mark_text() {
        // **Both gestures come in through the same button and only one can win.** Marking as well
        // as reporting would leave the program painting its own selection with ours highlighted on
        // top, two layers saying different things about the same cells.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "dragging_inside_a_program_that_asks_for_the_mouse_does_not_mark_text",
        );

        drag_to_select(
            &mut window,
            &ctx,
            &rects,
            inside_the_console(rects[0], &metrics, egui::vec2(1.0, 1.0)),
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * 3.5, metrics.height() * 0.5),
            ),
        );

        assert_eq!(
            selected_at(&window, 0),
            None,
            "the drag marked text in a console whose program had asked for the mouse"
        );
    }

    #[test]
    fn a_press_on_the_strip_is_not_reported_to_the_program_that_asked_for_the_mouse() {
        // **AC12: the strip belongs to the window, whatever is running underneath.** The console's
        // grid starts below it, so a press there has no cell to report — and reporting it as row 0
        // would put a click inside somebody's `vim` where they had aimed at the band that moves the
        // console. The delivery that cut the strip left this pinned as an assumption; here it is
        // driven.
        //
        // **With a sentinel afterwards, because «nothing arrived» is satisfied by a race.** A test
        // that only asserted the absence would pass just as well against a program that had not
        // read anything yet: the second press is one that must arrive, and it is what makes the
        // first one's absence mean something.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "a_press_on_the_strip_is_not_reported_to_the_program_that_asked_for_the_mouse",
        );
        let strip = header::set_the_header_aside(rects[0], &metrics, window.headers)
            .0
            .expect("the pane carries a strip");
        // The centre remains the drag handle; the left now carries pane controls.
        let on_the_strip = strip.center();
        assert!(
            !header::button_within(strip, &metrics, header::Button::in_a_tab_of(&rects))
                .is_some_and(|target| target.holds(on_the_strip)),
            "the press this test drives lands on the close button, so it measures something else"
        );

        let press = |window: &mut Window, at: egui::Pos2| {
            ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(at),
                        egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    window.handle_the_mouse(
                        ui,
                        &rects,
                        &CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE)),
                    );
                },
            )
            .drop_without_applying_deltas();
        };

        press(&mut window, on_the_strip);
        press(
            &mut window,
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * 3.5, metrics.height() * 2.5),
            ),
        );

        wait_for_the_pane_to_say(&mut window, 0, "^[[<0;4;3M");
        assert_eq!(
            window.active().panes()[0]
                .screen
                .to_text()
                .matches("^[[<")
                .count(),
            1,
            "the program received a second report: the press on the strip reached it as a click on \
             its own first row"
        );
    }

    #[test]
    fn a_repeated_pass_does_not_send_the_click_twice_to_the_program() {
        // **A defence, pinned by hand**: egui 0.36 hands a repeated pass no events, but these
        // events cannot be consumed — the press is what egui needs to start the drag — so if an
        // egui replayed them, without keeping track of the frame the program inside would receive
        // **two presses where the user gave one** — a double click nobody made, which inside an
        // editor selects a word or opens whatever is under the pointer.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "a_repeated_pass_does_not_send_the_click_twice_to_the_program",
        );
        let cell = |column: f32| {
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * column, metrics.height() * 0.5),
            )
        };

        // **Two mouse passes over one frame's events, inside ONE egui pass**: they stand for two
        // passes of one frame because the barrier reads the frame number, which both share. And
        // afterwards a click on another cell acting as a sentinel, so that counting happens once
        // the first one's has already arrived.
        let input = |at: egui::Pos2| egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        for at in [cell(0.5), cell(0.5), cell(2.5)] {
            ctx.run_ui(input(at), |ui| {
                window.handle_the_mouse(
                    ui,
                    &rects,
                    &CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE)),
                );
                // The hand-made second pass of the same frame, which is what this test is after.
                window.handle_the_mouse(
                    ui,
                    &rects,
                    &CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE)),
                );
            })
            .drop_without_applying_deltas();
        }

        wait_for_the_pane_to_say(&mut window, 0, "^[[<0;3;1M");
        let on_screen = window.active().panes()[0]
            .screen
            .to_text()
            .replace('\n', "");
        assert_eq!(
            on_screen.matches("^[[<0;1;1M").count(),
            2,
            "the press was counted to the program more times than there were presses; on screen there \
             is: {on_screen:?}"
        );
    }

    #[test]
    fn pressing_on_the_panel_does_not_hand_the_drag_to_the_program() {
        // **The status panel's columns are inside the pane's rectangle but they do not belong to
        // the console.** Recording that press as «the program's», dragging from the panel inwards
        // sent `vim` or `htop` a gesture that **started outside it**. It is the mouse version of
        // the same defect the selection already paid for one delivery earlier.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "pressing_on_the_panel_does_not_hand_the_drag_to_the_program",
        );
        // **Well inside the panel and far from its grip.** Just past the console's last column is
        // the draggable border, and a press there is taken by it: the gesture would never reach the
        // path this test is after, and the test would pass by not exercising it.
        let on_the_panel = egui::pos2(
            rects[0].right() - metrics.width() * 5.0,
            rects[0].min.y + metrics.height() * 0.5,
        );
        drag_to_select(
            &mut window,
            &ctx,
            &rects,
            on_the_panel,
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * 2.5, metrics.height() * 0.5),
            ),
        );

        // The sentinel: a clean click INSIDE the console, which does have to arrive. With it on
        // screen, whatever the previous gesture had sent would have arrived too.
        let inside_the_console = inside_the_console(
            rects[0],
            &metrics,
            egui::vec2(metrics.width() * 5.5, metrics.height() * 0.5),
        );
        drag_to_select(
            &mut window,
            &ctx,
            &rects,
            inside_the_console,
            inside_the_console,
        );

        // **It waits for the sentinel's RELEASE and not for its press**, which is what makes this
        // test deterministic: the assertion below counts both ends, so waiting only for the first
        // leaves the second racing with the reading of the screen — green with the machine free and
        // red as soon as anything else competes. Measured: with two tests launching processes in
        // the same batch, it fell one time in two.
        //
        // **And waiting for the release does not make it «less likely», it makes it impossible**,
        // which is the difference between a fix and a patch. The bytes arrive through **a single
        // pseudoterminal and in order**, and after the sentinel's gesture **no further pass runs**:
        // nothing can write behind that release. Seeing it on screen proves that everything before
        // it is already interpreted. What that reasoning does NOT cover, said for whoever comes
        // next: adding a pass after the sentinel, or a defect that deferred a report to a later
        // frame, reopens the race in silence.
        wait_for_the_pane_to_say(&mut window, 0, "^[[<0;6;1m");
        let on_screen = window.active().panes()[0]
            .screen
            .to_text()
            .replace('\n', "");
        assert_eq!(
            on_screen.matches("^[[<32;").count(),
            0,
            "the drag that started on the panel reached the program; on screen there is: \
             {on_screen:?}"
        );
        // **And neither its press nor its release.** Without the first assertion, a press on the
        // panel reported with an invented cell would pass; without the second, the program would
        // receive **a release with no press of its own**, which for anybody keeping mouse state is
        // worse than receiving nothing. The two that remain are the sentinel's: its press and its
        // release.
        assert_eq!(
            on_screen.matches("^[[<0;").count(),
            2,
            "the gesture that started on the panel sent the program one of its ends; on screen there \
             is: {on_screen:?}"
        );
    }

    #[test]
    fn a_button_that_is_not_the_primary_gets_its_press_and_its_release_too() {
        // **The drag's latch belongs to the primary alone, because only the primary drags**; the
        // release's has to belong to each button. With a single one for all of them, the right
        // click inside a program that asks for the mouse —the one that opens a context menu in
        // `htop` or in an editor— received its press and **never its release**: the program is left
        // with the menu open believing the button is down.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(&rects, &metrics, "non_primary_button");
        let at = inside_the_console(
            rects[0],
            &metrics,
            egui::vec2(metrics.width() * 0.5, metrics.height() * 0.5),
        );

        let pass = |window: &mut Window, input: egui::RawInput| {
            ctx.run_ui(input, |ui| {
                window.handle_the_mouse(
                    ui,
                    &rects,
                    &CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE)),
                );
            })
            .drop_without_applying_deltas();
        };
        pass(&mut window, egui::RawInput::default());
        for pressed in [true, false] {
            pass(
                &mut window,
                egui::RawInput {
                    events: vec![egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Secondary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
            );
        }

        wait_for_the_pane_to_say(&mut window, 0, "^[[<2;1;1M^[[<2;1;1m");
    }

    #[test]
    fn the_chord_does_not_vanish_midway_through_the_drag() {
        // **A key held down during the gesture has to travel in EVERY report.** With the chord
        // fixed to «none» in the movement ones, the key went in the press and in the release but in
        // none of those in between: for a program that distinguishes a drag with a chord —block
        // selection in an editor— it released itself and pressed itself again midway through the
        // gesture.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window =
            a_pane_that_asks_for_the_mouse(&rects, &metrics, "the_chord_does_not_vanish");

        drag_with_chord(
            &mut window,
            &ctx,
            &rects,
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * 0.5, metrics.height() * 0.5),
            ),
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * 2.5, metrics.height() * 0.5),
            ),
            egui::Modifiers::CTRL,
        );

        // 32 from the movement plus 16 from `Ctrl`. Without the chord a bare 32 would come out.
        wait_for_the_pane_to_say(&mut window, 0, "^[[<48;");
    }

    #[test]
    fn releasing_over_another_console_reports_it_to_the_one_that_got_the_press() {
        // **The defect this is after leaves the program believing the button is pressed FOREVER.**
        // Ending a drag outside the console where it started is the normal thing; resolving the
        // release by where the pointer falls, the console of the press never finds out —`htop`
        // keeps stretching its column, `vim` extending its selection— and the neighbor receives a
        // release for a press it never saw.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(&rects, &metrics, "releasing_over_another");

        drag_to_select(
            &mut window,
            &ctx,
            &rects,
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * 0.5, metrics.height() * 0.5),
            ),
            rects[1].center(),
        );

        // The trailing `m` is the release, and it has to reach the console on the LEFT even though
        // the pointer was over the right one.
        wait_for_the_pane_to_say(&mut window, 0, "m");
        let on_screen = window.active().panes()[0]
            .screen
            .to_text()
            .replace('\n', "");
        // **The release's cell goes clamped to ITS grid.** The pointer was over the console next
        // door, which starts much further right: without clamping, the program was told about a
        // release in a column its screen does not have.
        let is_last = window.active().panes()[0].measured;
        assert_eq!(
            on_screen
                .matches(&format!("^[[<0;{};", is_last.cols))
                .count(),
            1,
            "the release did not arrive clamped to its console's last column; on screen there is: \
             {on_screen:?}"
        );
        // And the same for the movement reports, which come out along another path and had the same
        // hole.
        assert_eq!(
            on_screen.matches("^[[<32;").count(),
            on_screen
                .matches(&format!("^[[<32;{};", is_last.cols))
                .count(),
            "some drag report came out with a column the console does not have; on screen there is: \
             {on_screen:?}"
        );
    }

    #[test]
    fn a_mouse_gesture_is_three_reports_and_a_single_effect() {
        // **Three things that are counted wrong separately and this test ties down together**,
        // because all three live in the same gesture:
        //
        // 1. **The press seeds the cell as already counted**: without that, the drag's first frame
        //    repeats the cell the press has just counted — one extra write to the pseudoterminal
        //    per gesture.
        // 2. **The release clears it**: without that, a new gesture starting where the previous one
        //    ended swallows its first report.
        // 3. **Effects are counted per GESTURE, not per report.** The counter answers «how many of
        //    the injected gestures changed something» and in the other batches it never exceeds the
        //    gestures; counting each report, the mouse batch published three times as many and its
        //    column stopped comparing.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window =
            a_pane_that_asks_for_the_mouse(&rects, &metrics, "one_gesture_three_reports");
        window.injected_drags = InjectedDrags::new(2, InjectedGesture::MouseToProgram);
        let cell = |column: f32| {
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * column, metrics.height() * 0.5),
            )
        };

        let pass = |window: &mut Window, input: egui::RawInput| {
            ctx.run_ui(input, |ui| {
                window.handle_the_mouse(
                    ui,
                    &rects,
                    &CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE)),
                );
            })
            .drop_without_applying_deltas();
        };
        let gesture = |window: &mut Window, from: egui::Pos2, to: egui::Pos2| {
            pass(window, egui::RawInput::default());
            pass(
                window,
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(from),
                        egui::Event::PointerButton {
                            pos: from,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
            );
            // One frame moving **inside the cell of the press**, which is the one that is redundant
            // without the seeding, and another that does change cell. **The displacement has to
            // exceed the threshold at which egui decides a gesture is a drag** —about six points—
            // or that frame does not reach the path this wants to exercise and the test passes
            // without testing it: checked, with one point of movement the mutation of the seeding
            // did not knock it down.
            let inside_the_same_one =
                from + egui::vec2(metrics.width() * 0.3, metrics.height() * 0.4);
            for at in [inside_the_same_one, to] {
                pass(
                    window,
                    egui::RawInput {
                        events: vec![egui::Event::PointerMoved(at)],
                        ..Default::default()
                    },
                );
            }
            pass(
                window,
                egui::RawInput {
                    events: vec![egui::Event::PointerButton {
                        pos: to,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
            );
        };

        gesture(&mut window, cell(0.5), cell(2.5));
        // The second one starts **where the first ended**, which is what exposes a release that did
        // not clear.
        gesture(&mut window, cell(2.5), cell(4.5));

        let injected_drags = window
            .injected_drags
            .as_ref()
            .expect("I have just set them");
        assert_eq!(
            (injected_drags.reported, injected_drags.with_effect),
            (6, 2),
            "one gesture is three reports —press, drag, release— and ONE effect; {} reports and {} \
             effects came out",
            injected_drags.reported,
            injected_drags.with_effect
        );
    }

    #[test]
    fn the_drag_does_not_repeat_the_report_of_the_cell_it_is_already_on() {
        // **A slow drag spends dozens of frames over the same cell**, and counting them all sends
        // the program sixty messages a second saying what it already knew — with its own trip to
        // the pseudoterminal each one. What the program expects is to find out that it moved on to
        // **another** one.
        //
        // **It is counted after a sentinel**, not there and then: asserting «a second one did not
        // arrive» right after the first would pass just the same with the defect in, because the
        // second would still be on its way. With the third cell's report on screen, the previous
        // two have arrived.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "the_drag_does_not_repeat_the_report_of_the_cell_it_is_already_on",
        );
        let cell = |column: f32| {
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * column, metrics.height() * 0.5),
            )
        };

        let pass = |window: &mut Window, input: egui::RawInput| {
            ctx.run_ui(input, |ui| {
                window.handle_the_mouse(
                    ui,
                    &rects,
                    &CellMetrics::measure(ui.ctx(), egui::FontId::monospace(DEFAULT_FONT_SIZE)),
                );
            })
            .drop_without_applying_deltas();
        };
        pass(&mut window, egui::RawInput::default());
        pass(
            &mut window,
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(cell(0.5)),
                    egui::Event::PointerButton {
                        pos: cell(0.5),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
        );
        // Two frames inside the SAME cell, and one more in the next one acting as a sentinel.
        for at in [cell(1.3), cell(1.7), cell(2.5)] {
            pass(
                &mut window,
                egui::RawInput {
                    events: vec![egui::Event::PointerMoved(at)],
                    ..Default::default()
                },
            );
        }

        wait_for_the_pane_to_say(&mut window, 0, "^[[<32;3;1M");
        // **The line breaks are stripped before counting**: with the panel on the right the console
        // is narrow and the screen wraps, so a report split across two rows would not match and the
        // tally would come out short without anything being wrong.
        let on_screen = window.active().panes()[0]
            .screen
            .to_text()
            .replace('\n', "");
        assert_eq!(
            on_screen.matches("^[[<32;2;1M").count(),
            1,
            "the drag repeated the report of a cell it was already on; on screen there is: \
             {on_screen:?}"
        );
    }

    #[test]
    fn with_shift_text_is_marked_even_if_the_program_asks_for_the_mouse() {
        // **AC7, the other half, and it is the one that guarantees a line can always be got off the
        // screen.** Without this escape hatch, inside `htop` or `vim` there would be NO way of
        // copying what is being looked at — and it is what any terminal does with that same key.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "with_shift_text_is_marked_even_if_the_program_asks_for_the_mouse",
        );
        window.active_mut().panes_mut()[0]
            .screen
            .advance(b"\x1b[Hgood night");

        drag_with_chord(
            &mut window,
            &ctx,
            &rects,
            inside_the_console(rects[0], &metrics, egui::vec2(1.0, 1.0)),
            inside_the_console(
                rects[0],
                &metrics,
                egui::vec2(metrics.width() * 3.5, metrics.height() * 0.5),
            ),
            egui::Modifiers::SHIFT,
        );

        assert_eq!(
            selected_at(&window, 0).as_deref(),
            Some("good"),
            "with Shift held the gesture went to the program instead of marking"
        );
    }

    #[test]
    fn moving_the_pane_beats_the_program_that_asks_for_the_mouse() {
        // **The window's chord cannot be kept by a program.** If it could, opening `vim` in a
        // console would leave that console nailed where it is: there would be no way of rearranging
        // it without closing it.
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let mut window = a_pane_that_asks_for_the_mouse(
            &rects,
            &metrics,
            "moving_the_pane_beats_the_program_that_asks_for_the_mouse",
        );
        let first_part = window.active().panes()[0].name;

        drag_a_pane(
            &mut window,
            &ctx,
            &rects,
            rects[0].center(),
            rects[1].right_center() - egui::vec2(10.0, 0.0),
        );

        assert_eq!(
            window.active().panes()[1].name,
            first_part,
            "the program kept the move chord and the console did not move anywhere"
        );
    }

    /// The top left cell: the one the pointer gives when the test does not bother with it.
    fn top_left() -> Cell {
        Cell {
            row: 0,
            col: 0,
            half: Half::Left,
        }
    }

    /// A console with eighty lines of scrollback behind it.
    fn pane_with_scrollback() -> Pane {
        let mut pane = pane_with("sleep 30", 0);
        for n in 0..80 {
            pane.screen.advance(format!("line {n}\r\n").as_bytes());
        }
        pane
    }

    /// A wheel event of whatever unit and displacement is asked for.
    fn wheel_event(unit: egui::MouseWheelUnit, y: f32) -> egui::Event {
        egui::Event::MouseWheel {
            unit,
            delta: egui::vec2(0.0, y),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        }
    }

    /// A key press with the given modifiers.
    fn key_press(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn a_touchpad_scrolls_the_scrollback_even_if_each_stroke_is_worth_less_than_a_line() {
        // **The defect this case is after left the scrollback STILL FOREVER with a touchpad.** A
        // touchpad sends **points**, not notches: every frame of a slow drag is worth a fraction of
        // a line. Truncating without keeping the remainder, each of those frames scrolled zero and
        // the event was consumed all the same, so two fingers dragging slowly moved nothing — and
        // there is no way of noticing except by trying to turn very hard, which is when the
        // fraction does reach one. **Through the whole UI pass**, which is where the conversion
        // from points to lines lives: a test that called the scroll directly would let through that
        // the points are not divided by a cell's height.
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(1200.0);
        for pane in window.active_mut().panes_mut() {
            for n in 0..80 {
                pane.screen.advance(format!("line {n}\r\n").as_bytes());
            }
        }
        let height = with_metrics(CellMetrics::height);
        let over_the_left_one = egui::pos2(area.left() + area.width() / 4.0, area.center().y);
        // A third of a line per frame, **one frame each**: it is what a finger dragging slowly
        // does, and it is what the defect threw away whole.
        let a_third = height / 3.0;

        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(area),
                events: vec![
                    egui::Event::PointerMoved(over_the_left_one),
                    wheel_event(egui::MouseWheelUnit::Point, a_third),
                ],
                ..Default::default()
            };
            ui_pass(&mut window, &ctx, area, input);
        }

        assert_eq!(
            window.active().panes()[0].screen.view().scroll(),
            1,
            "three strokes of a third of a line had to go up exactly one: the remainder is being \
             thrown away"
        );
    }

    #[test]
    fn the_wheels_remainder_belongs_to_each_console_and_is_not_given_to_its_neighbor() {
        // **The accumulated remainder is state, and storing it in the window would move it from
        // console to console with the mouse.** Whoever moves the pointer over another console does
        // not expect that one to jump half a line because of what they did in the first.
        let mut left_pane = pane_with_scrollback();
        let mut right_pane = pane_with_scrollback();
        let half = WheelInput {
            lines: 0.5,
            pages: 0.0,
        };

        left_pane.handle_wheel(half, egui::Modifiers::NONE, top_left());
        right_pane.handle_wheel(half, egui::Modifiers::NONE, top_left());

        assert_eq!(
            (
                left_pane.screen.view().scroll(),
                right_pane.screen.view().scroll()
            ),
            (0, 0),
            "half a line moved something: it is being rounded instead of keeping the remainder"
        );

        left_pane.handle_wheel(half, egui::Modifiers::NONE, top_left());

        assert_eq!(
            (
                left_pane.screen.view().scroll(),
                right_pane.screen.view().scroll()
            ),
            (1, 0),
            "one console's remainder was counted to the other"
        );
    }

    #[test]
    fn the_page_wheel_moves_a_screen_and_the_line_wheel_a_line() {
        // **egui's three units, and two of them were exercised by nobody.** Checked by mutating:
        // with `Page` ignored altogether and with `Point` unconverted, the suite stayed green
        // because the only wheel test sent `Line`. A page jump that does not jump —or that jumps
        // one line— is one of those defects that do not fail, they just behave badly. **Through the
        // whole UI pass**, which is where each unit separates from the others: calling the scroll
        // directly, ignoring the page unit broke nothing.
        let ctx = egui::Context::default();
        let area = area_of(1200.0);
        let over_the_left_one = egui::pos2(area.left() + area.width() / 4.0, area.center().y);
        let turn = |unit, how_much| {
            let mut window = window_with("sleep 30", "sleep 30");
            for pane in window.active_mut().panes_mut() {
                for n in 0..80 {
                    pane.screen.advance(format!("line {n}\r\n").as_bytes());
                }
            }
            let input = egui::RawInput {
                screen_rect: Some(area),
                events: vec![
                    egui::Event::PointerMoved(over_the_left_one),
                    wheel_event(unit, how_much),
                ],
                ..Default::default()
            };
            ui_pass(&mut window, &ctx, area, input);
            let pane = &window.active().panes()[0];
            (pane.screen.view().scroll(), pane.screen.size().rows)
        };

        let (one_line, _) = turn(egui::MouseWheelUnit::Line, 1.0);
        let (one_page, rows) = turn(egui::MouseWheelUnit::Page, 1.0);
        assert_eq!(one_line, 1, "one wheel notch did not go up one line");
        assert!(
            one_page > one_line,
            "one page went up {one_page} lines and one line went up {one_line}: the page unit is \
             falling into the same path as the line one"
        );
        // And one page is the screen minus one line of context, which is what any pager does.
        assert_eq!(
            one_page,
            usize::from(rows - 1),
            "one page was not the screen minus one line of context"
        );
    }

    #[test]
    fn the_four_scrollback_keys_each_do_their_own_thing() {
        // **Only `PageUp` was exercised, and the other three could be crossed without anything
        // falling.** Checked by mutating: with `Home` going to the end and `End` to the beginning,
        // the suite stayed green.
        let ctx = egui::Context::default();
        let to_the_top = {
            let mut pane = pane_with_scrollback();
            pane.screen.scroll_by(term_core::Scroll::ToTop);
            pane.screen.view().scroll()
        };
        assert!(
            to_the_top > 0,
            "the test console has no scrollback behind it"
        );

        // Each key, from the state in which it means something, and what it has to leave behind.
        let cases: [(egui::Key, usize, usize); 4] = [
            // From the bottom, `PageUp` goes up one page; `PageDown` from the bottom cannot go
            // lower.
            (egui::Key::PageUp, 0, usize::MAX),
            (egui::Key::PageDown, to_the_top, usize::MAX),
            (egui::Key::Home, 0, to_the_top),
            (egui::Key::End, to_the_top, 0),
        ];

        for (key, from, expected) in cases {
            let mut pane = pane_with_scrollback();
            let mut modifiers = egui::Modifiers::NONE;
            pane.screen.scroll_by(term_core::Scroll::Lines(from as i32));
            let before = pane.screen.view().scroll();

            enqueue(
                &ctx,
                vec![
                    egui::Event::ModifiersChanged(egui::Modifiers::SHIFT),
                    key_press(key, egui::Modifiers::SHIFT),
                ],
            );
            pane.send_keys(
                &ctx,
                Instant::now(),
                &mut modifiers,
                2,
                &keyboard::Keymap::default(),
                true,
            );
            let after = pane.screen.view().scroll();

            if expected == usize::MAX {
                // The two page ones do not go to a fixed place: what is tied down is the DIRECTION.
                // **And that it MOVED**, which is the half that was missing: asserting only «it did
                // not go up» let `PageDown` stay still —with no chord, doing nothing— with the same
                // result as going down. Checked by taking its chord away: it was the only one of
                // the eleven that could be untied entirely with the suite green.
                let goes_up = key == egui::Key::PageUp;
                assert_ne!(
                    after, before,
                    "{key:?} did not move the view: it stayed at {before}"
                );
                assert_eq!(
                    after > before,
                    goes_up,
                    "{key:?} went the other way: from {before} to {after}"
                );
            } else {
                assert_eq!(
                    after, expected,
                    "{key:?} did not leave the view where it belongs"
                );
            }
        }
    }

    #[test]
    fn without_shift_the_scrollback_keys_still_belong_to_the_program() {
        // **`PageUp` on its own belongs to whoever runs inside** —a pager uses it—, and taking it
        // away would be breaking `less` inside our window. Checked by mutating: deleting the
        // «Shift» requirement the suite stayed green, because nothing covered this side.
        //
        // Since D52 these keys have bytes, so which side kept the key is told by the bytes: the
        // scrollback action writes none. And reaching the program is typing, so the view comes
        // back to where the program writes — which D52 names as the price of giving these keys to
        // the program. Insert and Delete ride along: they pay the same price and no shortcut
        // claims them, so they are the half of the six this test is not about Shift for.
        let ctx = egui::Context::default();

        for (key, sequence) in [
            (egui::Key::PageUp, "\x1b[5~"),
            (egui::Key::PageDown, "\x1b[6~"),
            (egui::Key::Home, "\x1b[H"),
            (egui::Key::End, "\x1b[F"),
            (egui::Key::Insert, "\x1b[2~"),
            (egui::Key::Delete, "\x1b[3~"),
        ] {
            let mut pane = pane_with_scrollback();
            let mut modifiers = egui::Modifiers::NONE;
            pane.screen.scroll_by(term_core::Scroll::Lines(3));
            assert_ne!(
                pane.screen.view().scroll(),
                0,
                "premise: the console was not scrolled back"
            );

            enqueue(&ctx, vec![key_press(key, egui::Modifiers::NONE)]);
            pane.send_keys(
                &ctx,
                Instant::now(),
                &mut modifiers,
                2,
                &keyboard::Keymap::default(),
                true,
            );

            assert_eq!(
                String::from_utf8_lossy(&pane.keystrokes),
                sequence,
                "{key:?} without Shift did not reach the program inside"
            );
            assert_eq!(
                pane.screen.view().scroll(),
                0,
                "{key:?} reached the program and the view stayed in the scrollback"
            );
        }
    }

    #[test]
    fn the_scrollback_keys_do_not_go_down_to_be_translated_into_bytes() {
        // **They are consumed, and nothing tied that down.** Checked by mutating: deleting the
        // consumption, the test asserting «nothing reached the program» stayed green — because
        // those four keys **had no bytes** in the keyboard milestone's table, so the defect and
        // the fix gave the same result. What has to be tied down is that the event **disappears
        // from the queue**: a key left there would be translated by whatever read the queue next —
        // a second keyboard pass over the same queue, which would turn the key into bytes after all
        // (see `send_keys`).
        let ctx = egui::Context::default();
        let mut pane = pane_with_scrollback();
        let mut modifiers = egui::Modifiers::NONE;

        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::SHIFT),
                key_press(egui::Key::PageUp, egui::Modifiers::SHIFT),
            ],
        );
        pane.send_keys(
            &ctx,
            Instant::now(),
            &mut modifiers,
            2,
            &keyboard::Keymap::default(),
            true,
        );

        let remaining = ctx.input(|input| input.events.clone());
        assert!(
            !remaining.iter().any(
                |event| matches!(event, egui::Event::Key { key, .. } if *key == egui::Key::PageUp)
            ),
            "the scrollback key stayed in the queue: whoever looks at it later sends it to the program"
        );
        // Since D52 the key does have bytes, so this half now binds as well.
        assert!(
            pane.keystrokes.is_empty(),
            "the scrollback key reached the program as {:?}",
            String::from_utf8_lossy(&pane.keystrokes)
        );
    }

    #[test]
    fn an_unbound_scrollback_key_reaches_the_program_with_its_modifier() {
        // `key.scroll-page-up = none` gives the chord back, and what comes back is Shift+PageUp —
        // not a bare PageUp, which a pager would read as a different key.
        let ctx = egui::Context::default();
        let mut pane = pane_with_scrollback();
        let mut modifiers = egui::Modifiers::NONE;
        let mut keymap = keyboard::Keymap::default();
        keymap.bind(keyboard::Action::ScrollPageUp, None);

        enqueue(
            &ctx,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::SHIFT),
                key_press(egui::Key::PageUp, egui::Modifiers::SHIFT),
            ],
        );
        pane.send_keys(&ctx, Instant::now(), &mut modifiers, 2, &keymap, true);

        assert_eq!(
            String::from_utf8_lossy(&pane.keystrokes),
            "\x1b[5;2~",
            "the unbound scrollback chord did not reach the program in xterm's form"
        );
    }

    #[test]
    fn home_takes_the_form_the_program_asked_for_through_the_door_that_types() {
        // Driven through `send_keys` and not the translation helper: the mode is read there, and
        // dropping that read would leave every helper test green.
        let ctx = egui::Context::default();
        let mut pane = pane_with_scrollback();
        let mut modifiers = egui::Modifiers::NONE;
        let keymap = keyboard::Keymap::default();

        pane.screen.advance(b"\x1b[?1h");
        assert!(
            pane.screen.application_cursor_keys(),
            "premise: the screen did not take the mode the program asked for"
        );
        enqueue(
            &ctx,
            vec![key_press(egui::Key::Home, egui::Modifiers::NONE)],
        );
        pane.send_keys(&ctx, Instant::now(), &mut modifiers, 1, &keymap, true);
        assert_eq!(
            String::from_utf8_lossy(&pane.keystrokes),
            "\x1bOH",
            "the program asked for application keys and Home kept the normal form"
        );

        pane.keystrokes.clear();
        pane.screen.advance(b"\x1b[?1l");
        enqueue(
            &ctx,
            vec![key_press(egui::Key::Home, egui::Modifiers::NONE)],
        );
        pane.send_keys(&ctx, Instant::now(), &mut modifiers, 1, &keymap, true);
        assert_eq!(
            String::from_utf8_lossy(&pane.keystrokes),
            "\x1b[H",
            "the mode was withdrawn and Home kept the application form"
        );
    }

    /// **The wheel moves the console UNDER THE POINTER, not the one that has the keyboard.** It is
    /// what any terminal does; the opposite surprises you daily, because looking at one console and
    /// using another is the normal case when there are several.
    #[test]
    fn the_wheel_scrolls_the_scrollback_of_the_console_under_the_pointer() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");
        let area = area_of(1200.0);
        // The keyboard stays on the left one on purpose: if the wheel went to the focus, the test
        // would pass by moving the wrong console.
        assert_eq!(
            window.active().focus,
            0,
            "the premise failed: the keyboard did not start on the left console"
        );
        for pane in window.active_mut().panes_mut() {
            for n in 0..80 {
                pane.screen.advance(format!("line {n}\r\n").as_bytes());
            }
        }

        // The pointer, over the right half.
        let over_the_right_one = egui::pos2(area.right() - area.width() / 4.0, area.center().y);
        let input = egui::RawInput {
            screen_rect: Some(area),
            events: vec![
                egui::Event::PointerMoved(over_the_right_one),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: egui::vec2(0.0, 3.0),
                    modifiers: egui::Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                },
            ],
            ..Default::default()
        };
        ui_pass(&mut window, &ctx, area, input);

        let panes = window.active().panes();
        assert_eq!(
            panes[0].screen.view().scroll(),
            0,
            "the wheel moved the focused console instead of the one under the pointer"
        );
        assert!(
            panes[1].screen.view().scroll() > 0,
            "the console under the pointer did not scroll up its scrollback"
        );
    }

    /// **Reflowing the text is declared, and until this milestone nobody did it.** It runs in the
    /// frame's logic half, where there is no painting to wrap it, so the subtraction took it — that
    /// is, the framework, which is the figure **D3** judges egui by. With the scrollback switched
    /// off it cost 7 µs and got lost in the noise; with the lines that are now kept, almost a
    /// millisecond each time.
    ///
    /// **And it is only recorded when it happens**: a zero recorded on the thousands of still
    /// frames would sink the mean and would make this span say the opposite of what it is worth.
    #[test]
    fn reflowing_the_text_is_declared_and_only_when_it_happens() {
        let ctx = egui::Context::default();
        let mut window = window_with("sleep 30", "sleep 30");

        // One still frame: nobody changes size, so there is nothing to record.
        one_frame_with(&mut window, &ctx, Vec::new());
        assert_eq!(
            window.reflow.frames, 0,
            "without a width change nothing is reflowed, and recording it would sink the mean"
        );

        // And now one that does: the window changes size, so both consoles reflow.
        let before = window.timed_separately;
        one_frame_in(&mut window, &ctx, area_of(700.0), Vec::new());

        assert_eq!(
            window.reflow.frames, 1,
            "the consoles' width changed and it was not recorded"
        );
        assert!(
            window.reflow.total > std::time::Duration::ZERO,
            "it was recorded in zero time, which is what a span that never got measured publishes"
        );
        // **What is checked is that THAT went in, not that the accumulator went up.** Checked by
        // mutating: deleting the line that adds what was reflowed, the check on «it went up» stayed
        // green, because in the same frame the layout's mirror and the mouse add to it too. A span
        // that is declared and not subtracted is exactly the mislabelled number this project
        // chases.
        assert!(
            window.timed_separately >= before + window.reflow.total,
            "what is subtracted from the frame went up {:?} and reflowing cost {:?}: it did not go in, \
             so it would be published as the framework's cost",
            window.timed_separately - before,
            window.reflow.total
        );
    }

    /// **The reflow stopwatch has to cover the screen's resize.** That is where the scrollback is
    /// walked, and a positive duration alone cannot prove it was timed: other work in the span
    /// could make the total nonzero. Comparing a populated screen with an empty one was also
    /// fragile because scheduler noise could outweigh the resize cost. The injected operation
    /// clock advances exactly when the screen resize runs, so moving the stopwatch's end before
    /// it makes this assertion fail deterministically. The real wrapped scrollback is still
    /// reflowed, and the resized grid is checked alongside the clock witness.
    /// Only this thread uses logical time; concurrent resize tests retain the real clock.
    #[test]
    fn the_reflow_stopwatch_covers_reflowing_the_text_and_not_just_telling_the_program() {
        let _clock = OperationClockGuard::enter();
        // Fill scrollback with lines that wrap when the grid narrows.
        let mut with_text = pane_with("sleep 30", 0);
        let wide = with_text.screen.size();
        // Lines longer than the grid, which are the ones that really reflow when narrowing: one
        // that fits whole is pushed in one go and goes out through the cheap door.
        let long = "x".repeat(usize::from(wide.cols) * 3);
        for _ in 0..term_core::SCROLLBACK {
            with_text.screen.advance(long.as_bytes());
            with_text.screen.advance(b"\r\n");
        }

        let narrow = term_core::Size {
            rows: wide.rows,
            cols: wide.cols / 2,
        };
        with_text.measured = narrow;
        with_text.resize();

        assert_eq!(
            with_text.screen.size(),
            narrow,
            "the screen was not resized"
        );
        assert_eq!(
            with_text.reflowed,
            std::time::Duration::from_secs(10),
            "the screen reflow did not advance the injected clock inside the stopwatch"
        );
    }

    /// What the console **shows** right now, not what the program has written.
    ///
    /// It goes through the snapshot and not through `to_text`, which always returns the active
    /// area: with the scrollback scrolled up the two are different, and the one to look at is the
    /// one that is seen.
    fn visible_text(pane: &Pane) -> String {
        let view = pane.screen.view();
        (0..view.size().rows)
            .map(|row| {
                view.row(row)
                    .expect("the row is inside the snapshot")
                    .iter()
                    .map(|cell| cell.c)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// **Whatever the program goes on writing does not take you back down**, and that is the half
    /// of the criterion that makes the scrollback useful: stopping to read something **while** an
    /// agent dumps is exactly the case where it is needed, and a scrollback that falls to the end
    /// as soon as a line comes in is no use for that.
    ///
    /// **It is tested through the console's pumping and not through the screen.** The screen
    /// already has its test in `term-core`; what is tied down here is that the path by which the
    /// program's output enters the application does not drag the view to the end — one
    /// `scroll_by(ToBottom)` too many in `pump`, which is the most natural way of writing «let the
    /// latest always be visible», leaves the scrollback useless and breaks no test of `term-core`.
    ///
    /// **And it asserts on the text that is seen**, not just on the number of lines scrolled up: it
    /// is what tells «the view did not move» from «the counter ended up at something other than
    /// zero».
    #[test]
    fn pumping_the_console_does_not_return_the_scrollback_to_where_the_program_writes() {
        let mut pane = pane_with("printf 'JUST WRITTEN\\n'; sleep 30", 0);
        for n in 0..80 {
            pane.screen.advance(format!("line {n}\r\n").as_bytes());
        }
        pane.screen.scroll_by(term_core::Scroll::ToTop);
        let looking_at = visible_text(&pane);
        assert!(
            looking_at.contains("line 0"),
            "the test never got up to the old stuff, so it compares nothing: {looking_at:?}"
        );

        // Pump until what the program wrote is **inside** the console: it is the moment at which a
        // pumping that dragged the view would already have done so. `to_text` gives the active
        // area, so it sees the line arrive without depending on where the view is.
        let deadline = Instant::now() + Duration::from_secs(30);
        while !pane.screen.to_text().contains("JUST WRITTEN") {
            assert!(
                Instant::now() < deadline,
                "the console's program never wrote anything, so the pumping brought nothing in"
            );
            pane.pump(Default::default());
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(
            visible_text(&pane),
            looking_at,
            "output came in through the pumping and what is seen changed: whoever stops to read \
             something while a program writes ends up unable to read it"
        );
        assert!(
            pane.screen.view().scroll() > 0,
            "the pumping returned the scroll to zero"
        );
    }
    crate::windows_unix_omission!(
        a_press_on_a_button_makes_the_program_receive_its_command,
        "unix-only: uses POSIX shell commands and terminal-mode setup in a PTY fixture"
    );

    /// **A press on a button makes the program receive its command**, and the evidence is a file.
    ///
    /// **The program's output goes to a file and not to the screen, and that is the whole design of
    /// this test.** With `ICANON` and `ECHO` on — which is how a console starts — the line discipline
    /// echoes the same bytes back out of the master and renders a control exactly as `cat -v` would,
    /// so a test that asserted on the grid would be satisfied by the kernel instead of by the
    /// program. This repository has measured that: fourteen of fifteen tests of the mouse path stayed
    /// green against an answer the kernel had written. A file is written by the program alone.
    ///
    /// **It asserts a line terminator and not a carriage return**, and the difference is the kernel's
    /// rather than ours. What is written is `\r`, because that is what the Enter key sends; `ICRNL` is
    /// on by default, so the line discipline turns it into `\n` before the program sees it. Asserting
    /// the byte we wrote would be asserting against the wrong side of the boundary.
    #[cfg(unix)]
    #[test]
    fn a_press_on_a_button_makes_the_program_receive_its_command() {
        let ctx = egui::Context::default();
        let rects = two_wide_panes();
        let metrics = with_metrics(|metrics| metrics.clone());
        let received = term_core::received::Received::new("button");
        let mut window = window_with(&received.script(""), "sleep 30");

        // The panel is made visible by publishing a real one, and the grid is derived from the same
        // tiling the window uses: a panel nobody published takes up no columns, so without this the
        // console keeps the whole pane and the strip this test presses in does not exist.
        let dir = std::env::temp_dir().join(format!("ivy-button-{}-panel", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test panel's directory");
        std::fs::write(
            dir.join("panel.json"),
            br#"{"v":1,"actions":[{"label":"Run tests","send":"npm test"}],
                 "lines":[{"text":"the test panel"}]}"#,
        )
        .expect("publish the test's panel");
        let panel_rect = {
            let pane = &mut window.active_mut().panes_mut()[0];
            pane.panel_state.refresh(&dir);
            assert!(
                pane.panel_state.is_visible(SystemTime::now()),
                "the test's panel never got to be seen, so its strip does not exist"
            );
            let split = pane.chrome_within(
                rects[0],
                &metrics,
                SystemTime::now(),
                header::Headers::PAINTED,
            );
            pane.measured = metrics.cells_in(split.console().size());
            pane.screen.resize(pane.measured);
            split
                .split()
                .panel
                .expect("the panel takes columns of its own")
        };

        // **Painted twice, and that is the subject rather than setup**: the first pass builds the
        // layout, so the panel is not settled and a press would be refused; the second reuses it,
        // which is the state of a panel nobody is writing to.
        for _ in 0..2 {
            paint_the_panel_of(&mut window, panel_rect, &metrics);
        }
        let at = window.active().panes()[0]
            .panel_state
            .buttons_drawn()
            .next()
            .expect("the painting recorded a button")
            .center();

        deliver_to_the_mouse(&mut window, &ctx, &rects, &metrics, click_at(at));

        // The write goes to a thread of its own (**D29**), and `cat` has to be scheduled on top of
        // that, so what is waited for is the file's content.
        let got = received.wait_for(b"npm test\n".len(), Duration::from_secs(5));
        let got = String::from_utf8_lossy(&got);

        assert_eq!(
            got, "npm test\n",
            "the program did not receive the command the button painted; the file it writes to \
             holds {got:?}"
        );
        // **A press on a button does not hand the console back to the person** (**AC15**): that door
        // is the application writing on their behalf — a remote control they were shown and pressed —
        // and revoking on it would take a console away from the agent the moment somebody used the
        // button beside it. The doors that do revoke carry the person's own input: keys, wheel,
        // mouse reports, and a held paste sent from its notice (**D44**).
        assert!(
            window.active().panes()[0].taken_over.is_none(),
            "a press on a panel's button revoked the console"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Paints that pane's panel, which is what records what a press may reach.
    fn paint_the_panel_of(window: &mut Window, panel_rect: egui::Rect, metrics: &CellMetrics) {
        let ctx = egui::Context::default();
        let theme = window.theme;
        let mut discoverer = fonts::Discoverer::default();
        let pane = &mut window.active_mut().panes_mut()[0];
        let id = pane
            .panel
            .as_ref()
            .map_or_else(|| "a1b2".to_owned(), |dir| dir.id().to_owned());
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            panel::paint(
                &ui.painter_at(panel_rect),
                &mut pane.panel_state,
                &id,
                &theme,
                metrics,
                SystemTime::now(),
                &mut discoverer,
                panel::Chrome::Historical,
            );
        });
        output.drop_without_applying_deltas();
    }
}

#[cfg(test)]
mod ligature_tests;
