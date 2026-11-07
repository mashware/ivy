//! The per-session half of the measurement: what the **window** pays, once or a few times a run.
//!
//! Its sibling [`super`] keeps the per-console spans — the series, the round trip and the rest of the
//! frame — and the seam between the two is the vehicle: a span in there belongs to a console and
//! carries its name, a figure in here belongs to the run.
//!
//! It came out under **D28**, which gives a module a ceiling of 600 production lines: `measure.rs`
//! had reached exactly that, and the milestone that follows adds two series to it. The shape is the
//! one [`crate::painter`] already has with its own `notices` and `blocks`.

use super::Series;
use std::time::Duration;

/// The two spans **the window** pays to save its layout: checking whether it changed, and writing it.
///
/// **It lives here and not in the pane for the same reason as the rest of the frame**: they belong to
/// the whole window, not to a console, so they do not have the vehicle a pane's spans carry.
///
/// **And BOTH things are needed, which is the finding that cost a whole review.** A series **only keeps
/// samples to print them**: it subtracts nothing. The rest of the frame is computed by subtracting from
/// the cost everything that was timed, and that pool has to be entered separately. With a series and no
/// subtraction, these two spans would be published with their row **and at the same time** inside the
/// figure D3 uses to judge the framework.
#[derive(Default)]
pub struct Saving {
    mirror: Series,
    write: Series,
    /// How many times the layout file has been written.
    ///
    /// **The report publishes it and the script cannot know it**, which is the same partition the
    /// project already uses with the injected gestures: this number lives inside the process. **The
    /// script does not guard on it**: every batch has saving vetoed, so it is zero by construction, and
    /// the guard reads the attempts instead.
    writes: usize,
}

impl Saving {
    /// Records what it cost to compare the mirror in **a whole frame**.
    ///
    /// The caller accumulates per pass and records **once on close**: egui repeats the interface pass,
    /// and recording per pass would make the mean describe "per pass" instead of "per frame" — the exact
    /// inverse of the bias this instrument already swallowed once.
    pub fn record_mirror(&mut self, how_long: Duration) {
        self.mirror.record(how_long);
    }

    /// Records what **one attempt** at writing cost: from resolving the path until the rename returns,
    /// or until the temporary is deleted if it failed.
    ///
    /// **The time is always recorded and the count only when it reached the disk**, and the distinction
    /// is not fussy: the work happened inside the frame in both cases and has to be subtracted, but
    /// publishing the failed attempts as writes would say the layout is saved when it is not. A full disk
    /// would publish "a thousand writes" without having written any.
    pub fn record_write(&mut self, how_long: Duration, reached_disk: bool) {
        self.write.record(how_long);
        if reached_disk {
            self.writes += 1;
        }
    }

    /// How many times it was **really** written, so the report declares it and the script can invalidate
    /// a run that says it measures the saving and saved nothing.
    pub fn writes(&self) -> usize {
        self.writes
    }

    /// How many times a write to disk was **attempted**, for tests.
    ///
    /// It exists because the counter above only counts the ones that reached, so without this a test
    /// cannot distinguish "it retried and failed again" from "it did not retry" — which is exactly the
    /// gap a review found in the retry test.
    #[cfg(test)]
    pub fn attempts(&self) -> usize {
        self.write.total
    }

    /// What the two spans have recorded so far, **for tests only**: it is what makes it possible to
    /// assert that the real path declares them, and not just that the recording works when called by
    /// hand.
    #[cfg(test)]
    pub fn has_any(&self) -> bool {
        self.mirror.has_samples() || self.write.has_samples()
    }

    pub fn publish(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if self.mirror.has_samples() {
            writeln!(
                a,
                "\nchecking whether the layout changed — the window's, shared by every console   ({} frames with a pass)",
                self.mirror.total
            )?;
            writeln!(a, "  {}", self.mirror)?;
            writeln!(
                a,
                "  walks the tree of every tab and builds its mirror to compare it against: paid in\n                   every frame that paints, which is why it is subtracted from the rest. Also\n                   includes comparing against what was last written when the deadline expires\n                   with nothing new to save: it is the same work"
            )?;
        }

        if self.write.has_samples() {
            writeln!(
                a,
                "\nwriting the layout to disk   ({} writes of {} attempts)",
                self.writes, self.write.total
            )?;
            writeln!(a, "  {}", self.write)?;
            writeln!(
                a,
                "  covers copying the layout, VALIDATING it, serializing it, resolving the path, creating\n                   the directory the first time, writing the temporary and renaming it over; and\n                   if something fails, deleting the temporary and stating the failing line. It only\n                   happens when the deadline expires, not every frame: an attempt that never\n                   reaches the disk is almost all validation, and a session that can never write makes\n                   only one"
            )?;
        }

        Ok(())
    }
}

/// What **the window** pays to search the machine for the fonts it is missing.
///
/// **It lives here and not in the pane for the same reason as the saving**: the faces are registered
/// with the text engine, which belongs to the whole window, not to a console.
///
/// **And the subtraction is needed, not just the series.** The rest of the frame is computed by
/// subtracting from the cost everything that was timed: a series alone would print its row **and at the
/// same time** leave the number inside the figure **D3** judges the framework with. It is the finding
/// that cost a whole review in the saving span, and here it bites the same — this work is
/// **milliseconds**, not microseconds.
#[derive(Default)]
pub struct Discovery {
    search: Series,
    /// How many characters have been searched for on the machine, in total.
    ///
    /// **The report publishes it and the script cannot know it**, just like the layout's writes: this
    /// number lives inside the process. A batch that says it measures the discovery and searches for
    /// nothing would publish the cost of a path that was not walked.
    searched: usize,
    /// How many system faces have been given to the text engine.
    ///
    /// **It goes apart from the searched ones and is not redundant**: a character no font covers is
    /// searched for —and paid in full— **without registering anything**. With a single number, a machine
    /// with no fonts and one with everything installed would publish indistinguishable reports.
    registered: usize,
}

impl Discovery {
    /// Records what **one round** of discovery cost: searching for the faces of what was noted and
    /// registering them.
    pub fn record(&mut self, how_long: Duration, searched: usize, registered: usize) {
        self.search.record(how_long);
        self.searched += searched;
        self.registered += registered;
    }

    /// Whether anything got searched for, **so a test can assert that the real path records it** and not
    /// just that the recording works when called by hand.
    ///
    /// The two counters are read by the report directly; this is the only thing that was needed from
    /// outside.
    #[cfg(test)]
    pub fn has_any(&self) -> bool {
        self.search.has_samples()
    }

    pub fn publish(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self.search.has_samples() {
            return Ok(());
        }
        writeln!(
            a,
            "\nsearching the machine for a missing font — the window's, ({} rounds · {} characters searched · {} faces registered)",
            self.search.total, self.searched, self.registered
        )?;
        writeln!(a, "  {}", self.search)?;
        writeln!(
            a,
            "  does NOT happen every frame: only when a character appears that neither the registered\n                   faces nor the factory ones cover, so this mean does not describe an ordinary\n                   frame. It covers asking the engine whether the character already shows —laying it\n                   out and comparing its drawing with the frame's—, listing the system fonts the\n                   first time, opening the candidates until one carries it, and telling the engine\n                   to keep it. A character NOBODY covers is paid all the same and registers\n                   nothing: that is why the two counters are separate"
        )?;
        writeln!(
            a,
            "  WHAT THIS SPAN DOES NOT MEASURE, stated here because hiding it would be worse: rebuilding\n                   the glyph ATLAS. Handing the configuration to the engine only stores it; the atlas\n                   is rebuilt when the NEXT pass starts, before any line of this project takes\n                   control, so there is nowhere to start the clock. Measured separately: about\n                   4.5 ms per registration with a 20 MB CJK face, and it lands in \"the rest of the\n                   frame\". It happens as many times as the registered faces column says, not once\n                   per frame"
        )?;
        Ok(())
    }
}

/// What it costs to **rasterize a color glyph**, which is the only thing in this project that does
/// expensive work inside the painting.
///
/// **It is named for what it measures and not for its subject**: in this project `Color` is a color
/// —so are `Color32`, `TermColor` and the theme table—, and a type with that name that actually
/// measures microseconds reads wrong everywhere it appears.
///
/// **It goes apart from the font search because it happens elsewhere in the frame, and that changes who
/// pays for it.** Registering a face rebuilds the engine's atlas, so it is done **outside** the
/// painting and its span is subtracted from the rest; rasterizing an emoji invalidates nothing, so it
/// is done where it is needed — inside "enqueuing the paint" for the console, and inside painting the
/// panel for a status panel's emoji. It is subtracted from whichever of the two it happened in:
/// without that, the span would stop meaning what its name says, and this project already has it
/// written that a mislabelled number is worse than no number.
#[derive(Default)]
pub struct Rasterizing {
    rasterizing: Series,
    /// How many color glyphs have really been rasterized.
    ///
    /// **The report publishes it and the script cannot know it**, just like the searched characters: a
    /// batch that says it measures the color and rasterizes none would be publishing the cost of a path
    /// that was not walked.
    rasterized: usize,
    /// How many times a color glyph was requested that the machine does not paint that way.
    ///
    /// **It goes apart and is not redundant**: a character whose presentation asks for color but that no
    /// installed font carries in color **is paid in full** and returns nothing. With a single number, a
    /// machine with an emoji font and one without would publish indistinguishable reports.
    failed: usize,
}

impl Rasterizing {
    /// Records what the color path cost **in a pane's painting**.
    pub fn record(&mut self, how_long: Duration, rasterized: usize, failed: usize) {
        self.rasterizing.record(how_long);
        self.rasterized += rasterized;
        self.failed += failed;
    }

    pub fn publish(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self.rasterizing.has_samples() {
            return Ok(());
        }
        writeln!(
            a,
            "\nrasterizing a color glyph — the window's   ({} frames with work · {} glyphs rasterized | {} without a color face)",
            self.rasterizing.total, self.rasterized, self.failed
        )?;
        writeln!(a, "  {}", self.rasterizing)?;
        writeln!(
            a,
            "  does NOT happen every frame: only the first time an emoji is asked at a given\n                   height and width, so this mean does not describe an ordinary frame. It covers searching the\n                   system's emoji families for the one that carries it in color, decoding its bitmap\n                   —in Noto Color Emoji an indexed 136x128 PNG—, scaling it down to the cell and\n                   uploading the texture. A character NO font paints in color is paid all the same\n                   and returns nothing: that is why the two counters are separate"
        )?;
        writeln!(
            a,
            "  IT IS SUBTRACTED from the span it ran in, not added on top: the paying console's\n                   \"enqueueing the paint\", or that pane's panel for an emoji in its status panel. It\n                   runs WITHIN those spans, the opposite of searching a font. Without the subtraction,\n                   the span that claims to measure enqueueing or the panel would also measure this"
        )?;
        Ok(())
    }
}

/// How big the window is, in the unit it is asked to open with.
///
/// **Two named fields and not a pair of numbers**, for the same reason the table's spans carry theirs:
/// two `f32` in a row can be swapped without the compiler saying anything, and the report would publish
/// a window with the width and height swapped without whoever reads it being able to notice.
pub struct SizeInPoints {
    pub width: f32,
    pub height: f32,
}

/// What it has cost to **reflow the text** of the consoles over the course of the session.
///
/// **It is the leak the history opens, and that is why it is declared from the same milestone that
/// turns it on.** Reflowing runs in the logical half of the frame —when applying the size the previous
/// pass measured— and until now **nobody timed it**: with the history off it cost 7 µs and was lost in
/// the noise, so the leak was invisible. With history it walks **all** the saved lines: 984 µs per
/// console measured with wrapping content (**D21**), and one border moves **two**.
///
/// Without this, that work would be published as the framework's cost — the figure **D3** judges egui
/// with.
#[derive(Clone, Copy, Default)]
pub struct Reflow {
    /// In how many frames **a reflow actually happened**.
    ///
    /// **Almost every frame reflows nothing**: only the ones that cross a cell boundary while a border
    /// is dragged, or the ones that follow a resize of the window. Without this number, a mean spread
    /// over thousands of still frames publishes "reflowing costs nothing", which is exactly the disguise
    /// the rest of the report avoids.
    pub frames: usize,
    /// What all of them cost together.
    pub total: Duration,
}

impl Reflow {
    /// Records what reflowing cost in a frame. **It is only called when there was a reflow**: a zero
    /// noted as a sample sinks the mean and makes this span say the opposite of what it is worth.
    pub fn record(&mut self, how_long: Duration) {
        self.frames += 1;
        self.total += how_long;
    }

    /// What each reflow cost on average, or nothing if there was none.
    fn per_reflow(&self) -> Option<Duration> {
        self.total.checked_div(self.frames as u32)
    }
}

/// What it has cost to paint the tab bar over the course of the session.
///
/// **It is declared because the frame is split by subtraction**: what nobody names is published as the
/// framework's cost, which is the figure D3 judges egui with. The bar belongs to no console —it has
/// nowhere to fall except here— and ever since its tab says what each tab's panes claim, it also
/// **walks**: not declaring it would be handing that walk to the framework.
///
/// **The span is the whole bar —accumulating and painting—, and saying it in full matters.** Declaring
/// only the walk would leave the tabs' painting with no owner, which is the same half-drawn boundary
/// this project already paid once.
#[derive(Clone, Copy, Default)]
pub struct TabBar {
    /// In how many frames it got painted.
    ///
    /// **Zero is not "it cost nothing", it is "there was no bar"** —with a single tab it is not
    /// painted—, and those are two things a zero total does not distinguish. It is also what makes the
    /// batch comparable: without this number, a run of a single tab would publish "the bar costs
    /// nothing" with the code taken out.
    pub frames: usize,
    /// What all of them cost together.
    pub total: Duration,
    /// The most tabs that got marked at once.
    ///
    /// **Zero means the bar walked and marked nothing.** The walk happens whether there is a claim or
    /// not, so without this number a batch whose producers did not publish the field would publish the
    /// bar's cost **without the half that has to be measured**, and it would read just like a good one.
    pub marked_tabs: usize,
}

impl TabBar {
    /// Records what it cost to paint it **in a frame**, with its passes already summed.
    ///
    /// The caller is the frame close, not the painting: egui repeats the pass after a discard and
    /// both really paint, so recording per pass would give a mean per pass under a label that says "per
    /// frame".
    pub fn record(&mut self, how_long: Duration) {
        self.frames += 1;
        self.total += how_long;
    }

    /// Keeps the most tabs that got marked at once.
    ///
    /// **The maximum and not the last**: in the last frame they may all have expired, and what has to
    /// be sayable is whether any got marked in the whole run.
    pub fn saw_marked(&mut self, how_many: usize) {
        self.marked_tabs = self.marked_tabs.max(how_many);
    }

    /// What it cost on average, or nothing if it never got painted.
    fn per_frame(&self) -> Option<Duration> {
        self.total.checked_div(self.frames as u32)
    }
}

/// What it cost to open the window with the layout that had been saved.
///
/// **It is published outside the span table and is subtracted from no frame**, and that is what has to
/// be understood before reading its numbers: restoring happens **entirely before the first frame** —the
/// file is read in `run` and the consoles are opened in the constructor—, so it does not contaminate the
/// subtraction split this project judges its framework with. It is a **startup** cost, which is another
/// path and until this milestone nobody declared it.
#[derive(Clone, Copy)]
pub struct Restoring {
    /// How many consoles came out of the file. **It is the number that makes the batch comparable**:
    /// without it, a run in which the file never got to command publishes the factory window under a
    /// restored label, which is the same disguise the report already avoids with the panels and with the
    /// gestures.
    pub consoles: usize,
    /// Reading the file from disk, interpreting it and validating it.
    ///
    /// **It is optional, and that keeps a number nobody took from being published.** With a bare
    /// `Duration`, a window built with a persistence that never read the disk would publish `0ns to read
    /// and validate` — indistinguishable from a truly instant read, and exactly the kind of figure this
    /// project forbids. With no measurement, the report says so in those words.
    pub read: Option<Duration>,
    /// Opening the window with what the file asked for: a `fork` and an `exec` per leaf, **and
    /// everything that comes behind it per console** — building its directory's watcher and describing
    /// the whole tree for the first mirror.
    ///
    /// **It goes until the window is built, not until the processes exist**, and a review moved that
    /// line: stopping it earlier, two jobs that **grow with N** were left out of a number that says it
    /// measures precisely that, and they would have been published as the framework's cost.
    ///
    /// **It is the one that grows with N** and that is why it goes apart from the one above: reading a
    /// four-leaf document and a two-hundred-leaf one costs almost the same, and opening them does not.
    pub opening: Duration,
}

/// What it cost to check the configuration of whoever uses the terminal, **and whether it got
/// checked**.
///
/// **It is named for the act and not for the thing**, like its siblings: `Restoring`, `Discovery`,
/// `Rasterizing` name the work. A type called `Configuracion` inside the measurement module reads as
/// "the measurement's configuration", which is exactly what it is not.
///
/// **A one-off span, like the restoring one**: it happens entirely before the first frame, so it has no
/// series and no mean. What is needed is that it **be declared**, or its microseconds fall inside the
/// nameless startup.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReadingSettings {
    /// It was not checked: it is a measurement batch and the file is vetoed.
    #[default]
    Vetoed,
    /// It was checked and there was nothing to apply — there is no file, or it could not be used.
    Missing,
    /// What the file asked for was applied.
    Applied {
        /// What it cost to read and interpret it.
        how_long: Duration,
        /// How many lines could not be applied.
        warnings: usize,
    },
}

/// What choosing the font cost, **and whether it was chosen at all**.
///
/// **It is named for the act, like its siblings** —`Restoring`, `Discovery`, `Rasterizing`— and it is
/// declared here rather than in the module that does the work, for the same reason as
/// [`ReadingSettings`]: what a span is worth to the report is the report's word. `fonts.rs` says what
/// happened to the face; this says what the run has to publish about it.
///
/// **Every state that looked carries its time, including the ones that found nothing.** Looking for a
/// family the machine does not have pays the same enumeration as one it does —the expensive half is
/// building the collection— so a search that failed is **not** a search that cost nothing. Publishing
/// only the successful one would leave those milliseconds inside the nameless start-up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ChoosingTheFont {
    /// The file named no family, so the machine was never enumerated.
    #[default]
    NotAsked,
    /// The family was found and is what the terminal paints with, under the machine's own name for it.
    Found { family: String, how_long: Duration },
    /// The machine does not have that family.
    Missing {
        asked_for: String,
        how_long: Duration,
    },
    /// The machine **has** it and none of its files can be opened.
    ///
    /// **It is its own state and not a flavour of the one above**, because the two send whoever reads
    /// them to different places: one to fix what they typed, the other to fix what is installed.
    Unusable {
        asked_for: String,
        how_long: Duration,
    },
}

/// The drag gestures a measurement batch injected into itself, as they are declared.
#[derive(Clone, Copy)]
pub struct Drags {
    /// Whole gestures that were injected: the ones that got to send their release event.
    pub injected: usize,
    /// How many times the gesture **really changed something**: releases the tree accepted, or frames in
    /// which a drag moved a panel's width.
    ///
    /// **It can be GREATER than the injected ones, and it is not a counting error.** With the panel
    /// gesture, a single drag crosses several cell boundaries and each crossing changes the width, so one
    /// gesture produces several effects —and with two panes in view, both panes'—. That is why the two
    /// figures are published **side by side and not as a fraction**: the first says how many gestures
    /// happened, the second how much work they caused. The first version printed them as "N of M" and
    /// published "423 of 211", which is a number impossible to read right.
    pub effective: usize,
    /// In how many **frames** the gesture left text marked in some console.
    ///
    /// **Frames and not gestures, and saying it matters**: a gesture extends the selection once per drag
    /// frame, so this number comes out **greater** than the injected gestures'. Publishing it under the
    /// word "gestures" gave a row with the third number above the first, which is impossible to read
    /// right — the same as already happened once publishing "423 of 211".
    ///
    /// **The batches read it in BOTH directions.** The one that measures marking requires there to be
    /// **more than one per gesture**: if the gesture entered by another route —the panel handle, the
    /// divider— the run would inject itself whole and publish the cost of something else under its label.
    /// And the ones that measure **another** gesture require it to be **zero**: since **D20** the bare
    /// primary button inside a console marks text, so a gesture that lost its handle would turn into a
    /// selection without anything else giving it away.
    ///
    /// **In the batch that marks it coincides with the effects, and that is correct**: the effect of
    /// marking **is** leaving something marked. The two are published the same because in the other
    /// batches they do not coincide — there the effects are tilings and layouts, and this one has to come
    /// out zero.
    pub marked: usize,
    /// How many **mouse reports** went out to the program inside during the run.
    ///
    /// **It is read in both directions, just like the marked ones.** The batch that measures the mouse
    /// requires it to be **greater than zero**: without that number, a run in which the program never got
    /// to ask for the mouse would publish the spans of a gesture that resolved as a selection, under the
    /// mouse's label. And the others require it to be **zero**: a report that slipped out would put a way
    /// out to the pseudoterminal inside another batch's mean.
    pub reported: usize,
    /// Which gesture was injected, as it is published. **It goes in the report** because the three
    /// gestures do not cost the same and one batch is not compared with another of a different class.
    pub gesture: &'static str,
}

/// The report header: where and how it was measured — how many tabs there were, which one was visible
/// and with what grid each console.
///
/// **This comment carried the title twice and the two paragraphs stuck together with no separation**,
/// from before this milestone; it was fixed while passing through here, because a doc that presents
/// itself twice reads as if it described two things.
///
/// The grids go one per console **and with its name**, because the width tiling leaves them different
/// and no span is understood without knowing over how many cells it was taken. It is the datum that is
/// needed to compare with an old measurement of a single full-screen console: without it, a lower
/// "enqueueing the paint" reads as the framework going faster, when what is happening is that there are
/// half the columns.
///
/// **Each tab arrives with its name, not with its position in the list.** When one is closed, the ones
/// to its right move up a place, so numbering here by the order of the walk would put another tab's name
/// on a table of numbers — and whoever reads the report has no way to notice. `visible` is also a name,
/// for the same reason.
///
/// **And each pane arrives with its own, for the same reason and as soon as they can be closed.** Today
/// there are two and fixed, so the name coincides with the position; publishing the name already is what
/// makes the batches taken before and after the pane tree read the same.
///
/// **A list per tab and not an array of two**: the number of panes stops being fixed by the type as
/// soon as the tree arrives, and a signature that kept promising two would force trimming or padding
/// right here, which is where the report turns into the datum someone is going to believe.
/// What the whole session has to tell about itself, on top of its per-console spans.
///
/// **It is a struct with names and not four parameters in a row**, for the same reason as [`super::Spans`]:
/// when these were positional arguments, swapping two compiled without a complaint and published a lying
/// report. And they are a list that grows —every milestone that declares a new span adds its own; it
/// came in at four and this one makes six—, so the signature's days were numbered.
// **`Copy` falls away here, and the docblock above already saw it coming**: the chosen font travels
// with its name, and a name is a `String`. It is built once per report and moved into it.
#[derive(Clone, Default)]
pub struct SessionHeader {
    /// The gestures a batch injected, if it injected any.
    pub drags: Option<Drags>,
    /// Where the consoles came from, if they came from a file.
    pub restoring: Option<Restoring>,
    /// What it cost to read the configuration, and whether it was read.
    pub settings: ReadingSettings,
    /// What choosing the font cost, and whether one was asked for.
    pub chosen_font: ChoosingTheFont,
    /// What it cost to paint the tab bar.
    pub tab_bar: TabBar,
    /// What it cost to reflow the text when a console's width changed.
    pub reflow: Reflow,
    /// What a blinking cursor woke the window for.
    pub blink: Blinking,
    /// What the panel's buttons drew and what was pressed.
    pub buttons: Buttons,
    /// What links were painted and activated.
    pub links: Links,
    /// What the control channel did.
    pub control: Control,
    /// What the strips over the consoles cost, and whether this run painted any.
    pub strips: Strips,
    /// What searching a console's scrollback cost (D61).
    pub search: Searching,
}

/// What searching a console's scrollback cost this run.
///
/// **Two costs and two places, and neither may land in the framework's figure (D13).** A press of
/// `Enter` walks the history in the keyboard's pass, which the window times whole; marking the
/// matches on screen runs inside a console's painting and is taken out of that span. Both are
/// published here so the subtraction can be audited, and the script reads the counts in both
/// directions: the search batch must have pressed, found and marked, and every other batch must have
/// done none of the three.
#[derive(Clone, Copy, Debug, Default)]
pub struct Searching {
    /// Presses of `Enter` or `Shift+Enter` that walked the history.
    pub presses: usize,
    /// How many of them found a match.
    pub found: usize,
    /// What all the presses cost together.
    pub pressing: Duration,
    /// The slowest of them, which is the stall a person can feel.
    pub slowest: Duration,
    /// Paintings of a console with a search open, **passes and not frames**: egui can paint a frame
    /// twice, and both passes mark.
    pub paintings: usize,
    /// How many of those paintings marked at least one match, which is what the search batch has to
    /// show it did. A painting with a search open and nothing to mark is not one of them.
    pub markings: usize,
    /// What marking cost, every painting with a search open together.
    pub marking: Duration,
}

/// What the panel's control channel did this run.
///
/// **Three numbers, and a run is invalidated in both directions by the first of them.** A batch that
/// declares a publisher and applied nothing never reached a pane — a socket that did not bind, a
/// client that could not connect — and would publish «the channel costs nothing»; any batch that
/// does **not** declare one and applied something has work in its spans that its load did not ask
/// for, which is exactly what **D13** forbids.
///
/// **The other two are published rather than checked**, because neither has a value a batch can
/// promise: a refusal needs a second source and a release needs a connection to end, and the batch
/// that measures the ordinary path does neither.
///
/// **Reading a document back is counted apart from applying one**, and that is not tidiness: the
/// script reads `applied` in both directions, and a read puts nothing on a panel — folding it in
/// would leave the number meaning something other than its own label, which is the mislabelled figure
/// **D13** is about.
#[derive(Default, Clone, Copy)]
pub struct Control {
    /// Documents taken and put on a panel.
    pub applied: usize,
    /// Publications refused because another source was holding the pane.
    pub refused: usize,
    /// Connections that ended and gave their pane back.
    pub released: usize,
    /// Documents written back out and handed to whoever asked for one.
    pub read_back: usize,
    /// Reads refused, because nothing had been published on that console yet.
    ///
    /// **Counted apart, and not folded into `refused`.** That one means *a publication refused
    /// because another source held the pane*, and a read is refused for a different reason entirely;
    /// one number for both would be two populations under a label that names one. It also tells the
    /// bench's two failures apart: a batch whose reads were all refused and one whose publisher never
    /// reached the pane both leave `read_back` at zero, and only this number says which happened.
    pub read_refused: usize,
    /// Documents changed in place, a line or a field at a time.
    ///
    /// **Counted apart from `applied` for the reason that one is counted apart from a read**: a
    /// patch and a publication both put a new document on a panel, and they are not the same work —
    /// one reads a body it was handed and the other walks the one that is there, indexes its keys
    /// and writes the result out to weigh it. Folding them together would leave the number that
    /// invalidates a run in both directions counting two populations under a label that names one.
    pub patched: usize,
    /// Patches the pane refused, **which is not every patch that was refused**.
    ///
    /// Everything a document is not needed to decide — an op nobody serves, a field that is not
    /// patchable, a value of the wrong kind, more ops than the ceiling allows — is refused on the
    /// connection's own thread and never reaches a pane, so it reaches neither of these numbers. What
    /// lands here are the four the panel itself decides: no hold, a key that names nothing, a key
    /// that names two lines, and a result over the ceiling. That is the right population for a
    /// measurement — the others cost no frame — and it is said out loud because a reader would
    /// otherwise take a zero here for «every patch was accepted».
    ///
    /// **A batch that patches declares this must be zero, and no other counter here is asserted that
    /// way.** The others are published because no batch can promise them — a refusal needs a second
    /// source, a release needs a connection to end. This one a batch *can* promise: its publisher
    /// holds the pane it patches, and every op it sends names a key it published itself. A run whose
    /// patches were all refused leaves the panel exactly as the publication left it, paints the same
    /// pixels and costs the parse and the refusal instead of the walk — and it would be published as
    /// the cost of patching.
    pub patches_refused: usize,
    /// The largest document this run wrote out, in bytes.
    ///
    /// **Without it a run publishes a cost with no size attached to it**, and a reader extrapolates
    /// what a six-line panel cost to a document at the ceiling. This is the first path of this module
    /// whose cost follows the size of what a producer published rather than the number of requests
    /// that arrived, so the size has to travel with the number — the same reason the font discovery
    /// publishes how many characters it searched.
    pub largest_serialized: usize,
    /// Intentional session retirements committed by the window.
    pub retired: usize,
    /// Auxiliary consoles detached by those retirements.
    pub auxiliary_closed: usize,
    /// User-taken auxiliary consoles preserved by those retirements.
    pub auxiliary_preserved: usize,
    /// Detached consoles whose resources have subsequently been destroyed.
    pub auxiliary_reaped: usize,
    /// Time spent committing panel and tree retirement transitions.
    pub retirement_time: std::time::Duration,
    /// Time spent destroying already-detached consoles.
    pub reaping_time: std::time::Duration,
}

impl Control {
    /// Two panes' numbers as one, **and which of the two rules each field is decided here**.
    ///
    /// **It is a method and not a closure at the fold**, and that is the whole reason it exists: every
    /// field but the last is summed and that one is a maximum, nothing in their types says which, and
    /// a fold written inline is a choice made where no test can reach it. Turning the maximum into a sum
    /// compiled, kept every test green, and could only be caught by reading a two-pane report by hand.
    ///
    /// **The maximum is not tidiness**: the number answers *how big was the document behind that
    /// cost*, and two panes' peaks added together name a document nobody ever wrote.
    pub fn and(self, other: Self) -> Self {
        Self {
            applied: self.applied + other.applied,
            refused: self.refused + other.refused,
            released: self.released + other.released,
            read_back: self.read_back + other.read_back,
            read_refused: self.read_refused + other.read_refused,
            patched: self.patched + other.patched,
            patches_refused: self.patches_refused + other.patches_refused,
            largest_serialized: self.largest_serialized.max(other.largest_serialized),
            retired: self.retired + other.retired,
            auxiliary_closed: self.auxiliary_closed + other.auxiliary_closed,
            auxiliary_preserved: self.auxiliary_preserved + other.auxiliary_preserved,
            auxiliary_reaped: self.auxiliary_reaped + other.auxiliary_reaped,
            retirement_time: self.retirement_time + other.retirement_time,
            reaping_time: self.reaping_time + other.reaping_time,
        }
    }
}

/// What the panel's buttons did this run.
///
/// **Both numbers, and a run is invalidated by either of them being zero where it should not be.**
/// Painted with nothing pressed says the injector never hit a rectangle — a narrow window, a
/// derivation that produced none — and the batch would publish «pressing a button costs nothing».
/// Pressed with nothing painted cannot happen and says the walk stopped matching. And either of them
/// above zero in a batch that declares no buttons says something other than the load is declaring
/// them, which mixes foreign work into the spans.
#[derive(Default, Clone, Copy)]
pub struct Buttons {
    /// Buttons drawn **whole**, summed over the run's frames. A button the clip cut in half is not
    /// counted, for the same reason it is not pressable.
    pub painted: usize,
    /// Presses that reached a button and wrote to a program.
    pub pressed: usize,
}

/// Link sources the bench's injector found, frame by frame, and accepted activations per source,
/// split so a batch must exercise every source.
#[derive(Default, Clone, Copy)]
pub struct Links {
    pub found: usize,
    pub plain: usize,
    pub osc8: usize,
    pub panel: usize,
}

/// What a blinking cursor cost this run, **and why it cost that**.
///
/// **It is declared even though every published batch reads zero**, because **D13**'s rule is about
/// the paths that *could* go undeclared, not the ones that did: a frame's cost is split by
/// subtraction, so a repaint nobody names is published as the interface framework's cost — the
/// figure **D3** judges egui by. A zero somebody can read is what makes the claim checkable, and the
/// second half of the line is what stops that zero being read as «nothing here can blink».
#[derive(Clone, Copy, Debug, Default)]
pub struct Blinking {
    /// Frames in which a blinking cursor **was painted**.
    ///
    /// **Painted and not «woken», because that is what is counted.** The two part company when the
    /// desktop takes the focus away between the painting half and the alarm. **D13** rates a
    /// mislabelled number worse than none, and this line is read by a script that throws runs away.
    pub frames: usize,
    /// What the settings this run read say about the blink, which is what decides the number above.
    pub ceiling: Option<bool>,
}

/// What the strips over the consoles cost this run, **and whether this run paints any**.
///
/// **It is a span of the window and not of a round, and that is the whole point of the type.** Every
/// other cost a console pays travels inside [`crate::measure::Spans`], which is closed only on a
/// frame where that console produced output — a frame at rest is not a lightning-fast round trip, it
/// is a round trip that did not happen. The strip is the opposite case: it is painted on **every**
/// frame the pane is painted, output or none, so a per-round series would publish a mean taken over
/// the dumping frames alone and a quiet console would publish no strip at all. Its sibling here is
/// [`TabBar`], which is painted per frame for the same reason.
///
/// **Where the two part company is the zero**, and it is what the `Option` one file away exists
/// for: the bar's frame is counted only when it cost something, and a strip's frame is counted
/// because it was **drawn**, cost or none. A band drawn faster than the clock can see is a frame
/// that had one.
///
/// **The frames are counted where the painting happened**, like [`Blinking::frames`] and never from
/// the gate: a batch that asks for strips and paints none, and a batch that asks for none and paints
/// some, are the two failures the measurement script throws a run away for, and neither is visible
/// from what was configured.
#[derive(Clone, Copy, Debug, Default)]
pub struct Strips {
    /// Frames in which at least one strip **was painted**.
    ///
    /// **Frames and not strips**, so the mean below is what one frame of this window pays for all of
    /// its consoles' strips — which is what a batch is compared against its control by. How many
    /// consoles there were is already in the grid block at the top of this same header.
    pub frames: usize,
    /// What all of them cost together, every pane of every frame summed.
    pub total: Duration,
    /// Whether this run paints them at all — the gate's answer, not the count's.
    ///
    /// **Named for what the value says and not for what a batch does with it.** It is fed from
    /// [`crate::header::Headers`], which asserts «this session paints strips»; the report is
    /// published on **every** run and not only on a measured one, so a field called «asked for»
    /// would have an ordinary terminal printing that somebody asked for something nobody asked for.
    /// What a batch asked is the knob at [`crate::header::HEADER_VARIABLE`], and the only reader
    /// that needs it is the script, which has it in its own environment.
    ///
    /// **Published beside the count and not instead of it.** The two together are what the script
    /// reads in both directions; on its own, either one is a run that validates clean while
    /// measuring something other than what it claims.
    pub painted_this_run: bool,
}

impl Strips {
    /// The same tally, saying what the session's gate resolved.
    ///
    /// **It exists so the gate arrives as [`crate::header::Headers`] and not as a bare `bool`**,
    /// which is the whole reason that newtype exists one file away: at the call site there are two
    /// booleans within reach and swapping them compiles. Chained rather than a field of the tally
    /// because the window accumulates the count frame by frame and learns nothing new about its own
    /// gate while doing it.
    pub(crate) fn under(self, gate: crate::header::Headers) -> Self {
        Self {
            painted_this_run: gate.are_painted(),
            ..self
        }
    }

    /// Records what the strips cost **in a frame**, with its passes already summed.
    ///
    /// The caller is the frame close and not the painting, for the reason [`TabBar::record`] gives:
    /// egui repeats the pass after a discard and both passes really paint, so recording there
    /// would give a mean per pass under a label that says «per frame».
    ///
    /// **It takes no zero check, unlike the bar's caller**: what decides whether a frame is counted
    /// is whether a strip was **drawn**, and that question is answered one file away by an `Option`
    /// rather than here by a duration.
    pub fn record(&mut self, how_long: Duration) {
        self.frames += 1;
        self.total += how_long;
    }

    /// What they cost on average, or nothing if none was ever painted.
    fn per_frame(&self) -> Option<Duration> {
        self.total.checked_div(self.frames as u32)
    }
}

pub fn header(
    a: &mut dyn std::io::Write,
    window: SizeInPoints,
    grids: &[(usize, Vec<(usize, term_core::Size)>)],
    visible: usize,
    session: SessionHeader,
) -> std::io::Result<()> {
    let SessionHeader {
        drags,
        restoring,
        settings,
        chosen_font,
        tab_bar,
        reflow,
        blink,
        buttons,
        links,
        control,
        strips,
        search,
    } = session;
    let profile = if cfg!(debug_assertions) {
        "debug — the number that counts is the one from --release"
    } else {
        "release"
    };
    writeln!(a, "\n── latency ──")?;
    // **How big the window was, and it is the only thing in the header that can be asked for again.**
    // Each console's grid in cells comes out of this size and the font metric, so reading it does not
    // say with what window the batch was taken: the annexes before this line were measured with windows
    // whose size nobody noted, and that is why they cannot be reproduced. It is published in points
    // because that is the unit it is asked for in.
    writeln!(a, "window:       {}×{} points", window.width, window.height)?;
    // **How many tabs there were, and not only which ones were painted.** The spans below were taken
    // with all of them pumping, so a batch of two tabs and one of one cannot be compared without this
    // datum — and without it, the report of the two reads as if it were of a single one and with the
    // frame inexplicably more expensive.
    writeln!(a, "tabs:         {}", grids.len())?;
    for (which_tab, grids) in grids {
        let seen_as = if *which_tab == visible {
            "was visible"
        } else {
            "in the background, pumped without being painted"
        };
        writeln!(a, "tab {which_tab}:        {seen_as}")?;
        for (which, (pane, grid)) in grids.iter().enumerate() {
            writeln!(
                a,
                "{:<15} {}×{}{}",
                format!("  pane {pane}:"),
                grid.rows,
                grid.cols,
                where_it_falls(grids.len(), which)
            )?;
        }
    }
    // **The gestures the batch injected, and only if it asked for any.** Two numbers, and both carry a
    // signal: the **injected** ones say the gesture happened, and the ones that **took effect** say it
    // got to change something. Both are needed because they are not the same: the geometric floor rejects
    // the release when the destination console cannot give both their minimum, and in a narrow window
    // that happens in all of them — the batch injects a thousand gestures and reorders not one. With a
    // single number, that run would read as "the gesture costs nothing" instead of as "nothing
    // happened", which is the disguise of declaring zero this project has already swallowed once.
    //
    // **The requested ones are NOT published, and the review fixed that.** The first version printed
    // "requested" and said the difference with the done ones was the warning; but the script asks for a
    // deliberately sky-high number so the gesture lasts the whole batch, so that difference is worth the
    // same in a good run as in a broken one. A number a reader is going to try to interpret and cannot is
    // worse than not publishing it.
    //
    // **The line does not come out when nobody asked for drags**, and that is deliberate: the annex
    // batches before this milestone would be compared with reports of a different shape, and here what is
    // compared is batches against each other.
    if let Some(drags) = drags {
        writeln!(
            a,
            "drags:        {} injected · {} effective · {} frames with marked text · {} reports to the program ({})",
            drags.injected, drags.effective, drags.marked, drags.reported, drags.gesture
        )?;
    }
    // **Where the consoles came from, and only when they came from the file.** Without this line, a
    // batch that says it measures the restored startup and one that opened from factory produce
    // **identical** reports: the header publishes how many consoles there are, not who asked for them.
    // And with the same console count none of the script's guards fire — which is exactly the disguise
    // this project already swallowed once with the panels and once with the gestures.
    //
    // **The two times go separate on purpose**: reading does not grow with the number of consoles and
    // opening them does, so summing them would give a number that cannot be extrapolated to a larger
    // layout.
    if let Some(restoring) = restoring {
        let read = match restoring.read {
            Some(how_long) => format!("{how_long:?}"),
            None => "not measured".to_string(),
        };
        writeln!(
            a,
            "restored:     {} consoles · {read} to read and validate · {:?} to open them (before the \
             first frame, subtracted from none)",
            restoring.consoles, restoring.opening
        )?;
    }
    // **It is ALWAYS declared, whether it was read or not**, and that is the half that matters. A run
    // that read the configuration of whoever measures and one that ran from factory publish grids that
    // may have nothing to do with each other, and without this line their reports are indistinguishable.
    // Every measurement batch has the file vetoed, so here it must always read "factory": if it ever says
    // otherwise, that run is compared with none.
    match settings {
        // **The three states are distinguished BY THE FIRST NOUN, not by the parenthesis**, and that is
        // what makes the measurement script able to read them. With "factory" for the vetoed one and for
        // the one that checked and found nothing, a broken veto on a machine with no file —which is **the
        // bench's real case**, because the script empties the configuration directory— would read as a
        // good run. A review caught it.
        ReadingSettings::Vetoed => writeln!(
            a,
            "settings:     vetoed (not read under the measurement gate: the font size of whoever \
             measures would change the grid, and every published figure depends on the grid)"
        )?,
        // **"Checked and there was nothing" is NOT "the user's", and confusing them publishes a number
        // with the label swapped.** A machine with no file and one that applied a different font size
        // have grids that may have nothing to do with each other, and with a single state for the two
        // their reports came out identical. A review caught it.
        ReadingSettings::Missing => writeln!(
            a,
            "settings:     factory (looked and there was nothing to apply)"
        )?,
        ReadingSettings::Applied { how_long, warnings } => writeln!(
            a,
            "settings:     the user's · {how_long:?} to read it · {warnings} line(s) that could not be \
             applied (before the first frame, subtracted from none)"
        )?,
    }
    // **It is ALWAYS declared, like the line above and for its same reason.** Asking for a family by
    // name is the only thing that makes the terminal enumerate the machine's fonts at start-up —
    // between 17.3 and 23.2 ms (**D24**)— and the script invalidates any run whose batch searched for
    // fonts without dumping Japanese. Every measurement batch has the file vetoed, so here it must
    // always read "none": if it ever says otherwise, the veto broke and that run is compared with
    // none.
    //
    // **The three states are told apart by the first word**, so the script can read them, and the one
    // that found nothing carries its time too: looking for a family that is not installed pays the
    // same enumeration as one that is.
    match &chosen_font {
        ChoosingTheFont::NotAsked => writeln!(
            a,
            "chosen font:  none (no family was asked for at startup; no startup family enumeration)"
        )?,
        ChoosingTheFont::Found { family, how_long } => writeln!(
            a,
            "chosen font:  {family} · {how_long:?} to find it in the machine (before the first frame, \
             subtracted from none)"
        )?,
        ChoosingTheFont::Missing {
            asked_for,
            how_long,
        } => writeln!(
            a,
            "chosen font:  missing '{asked_for}' · {how_long:?} spent looking for it (not installed; \
             painting with the built-in ones)"
        )?,
        ChoosingTheFont::Unusable {
            asked_for,
            how_long,
        } => writeln!(
            a,
            "chosen font:  unusable '{asked_for}' · {how_long:?} spent looking for it (installed and \
             its files cannot be opened; painting with the built-in ones)"
        )?,
    }
    // **Only when there was one.** With a single tab there is no bar to paint, and publishing "0 frames"
    // invites reading that it costs nothing.
    if let Some(mean) = tab_bar.per_frame() {
        writeln!(
            a,
            "tab bar:      {} frames · {:?} in total · {mean:?} per frame · {} tabs marked at \
             once at most (accumulating the claims and painting the tabs; subtracted from its \
             frame)",
            tab_bar.frames, tab_bar.total, tab_bar.marked_tabs
        )?;
    }
    // **Only when there was one.** In a batch that resizes nothing there is not a single reflow, and
    // publishing "0 times" invites reading that it costs nothing.
    if let Some(each) = reflow.per_reflow() {
        writeln!(
            a,
            "reflow:       {} times · {:?} in total · {each:?} each (the text, when a console's width \
             changes; runs in the logical half and is subtracted from its frame)",
            reflow.frames, reflow.total
        )?;
    }
    // **Always, and with the reason next to the number.** Since the cursor blinks by default, a
    // console nobody has spoken to would wake the window 1.7 times a second: the measurement gate
    // switches it off — the same gate that vetoes the settings file and forbids saving — and this
    // line is where that is said instead of assumed.
    writeln!(
        a,
        "blink:        {} frame(s) painted with a blinking cursor ({})",
        blink.frames,
        match blink.ceiling {
            Some(false) => "switched off, as every measured run has it",
            Some(true) => "forced on by the settings",
            // **Not «left to the program»**: `CONSOLE_NAME` publishes a pane as «left» or
            // «right», and a guard here refuses a header that says either with more than two
            // panes. It caught this line, which is what that guard is for.
            None => "the program decides, and it blinks unless it says otherwise",
        }
    )?;
    // **Always, and both numbers on one line.** A batch that declares buttons and reports zero
    // painted seeded nothing; one that reports buttons painted and none pressed never hit a
    // rectangle, and would publish «pressing a button costs nothing». The script reads the line in
    // both directions, the way it already reads the drags, the fonts and the emoji.
    writeln!(
        a,
        "buttons:      {} drawn whole, {} pressed",
        buttons.painted, buttons.pressed
    )?;
    writeln!(
        a,
        "links:        {} sources found · {} plain · {} OSC 8 · {} panel activations",
        links.found, links.plain, links.osc8, links.panel
    )?;
    // **Always, and every count on one line**, for the buttons' reason: the search's batch must have
    // pressed, found and marked, and every other batch must have pressed nothing and opened no
    // search — so the script reads the presses both ways, and each painting count one way each.
    writeln!(
        a,
        "search:       {} presses, {} found · {:?} each, {:?} the slowest (in the keyboard's pass, \
         subtracted from its frame) · {} paintings with a search open, {} with matches marked · {:?} \
         each painting with a search open (subtracted from the console's painting)",
        search.presses,
        search.found,
        search
            .pressing
            .checked_div(search.presses as u32)
            .unwrap_or_default(),
        search.slowest,
        search.paintings,
        search.markings,
        search
            .marking
            .checked_div(search.paintings as u32)
            .unwrap_or_default(),
    )?;
    // **Always, and all three on one line.** The first is read in both directions by the script; the
    // other two are published because a batch cannot promise them — a refusal needs a second source
    // and a release needs a connection to end — and a number nobody can check is still worth reading
    // when the one beside it is the one that decides.
    writeln!(
        a,
        "control:      {} applied, {} refused, {} released",
        control.applied, control.refused, control.released
    )?;
    // **A line of its own and not three more numbers on the one above**, because the script's readers
    // are anchored to the shape of that line and every number appended to it is a chance to break one
    // in silence.
    //
    // **The first is read in both directions; the second is what stops an extrapolation.** Writing a
    // document out is the first thing this channel does whose cost follows the *size* of what a
    // producer published rather than the number of requests that arrived, so a run that answers a
    // read of a six-line panel and publishes only «the control channel costs N µs» invites a reader
    // to carry that figure to the ceiling. The size travels with the number, the way the font
    // discovery publishes how many characters it searched.
    writeln!(
        a,
        "documents:    {} read back, {} refused, {} patched, {} patches refused, {} bytes the largest written out",
        control.read_back,
        control.read_refused,
        control.patched,
        control.patches_refused,
        control.largest_serialized
    )?;
    writeln!(
        a,
        "retirement:   {} committed, {} auxiliary closed, {} preserved, {} reaped · {:?} transition · {:?} reaping",
        control.retired,
        control.auxiliary_closed,
        control.auxiliary_preserved,
        control.auxiliary_reaped,
        control.retirement_time,
        control.reaping_time
    )?;
    // **Always, and both halves on one line.** A batch that asks for strips and painted none has
    // the gate broken or a window too short to cut one, and would publish «the strip costs
    // nothing»; one that asks for none and painted some measures a console a row shorter per pane
    // than every batch it is compared against, and nothing else in the run gives that away — the
    // grid it publishes is simply smaller. The script reads the line in both directions, the way it
    // already reads the buttons, the fonts and the emoji.
    //
    // **The count and the parenthesis are independent, and that is the point.** The number says what
    // was **painted** and the parenthesis says what was **asked for**; a line that inferred either
    // from the other could not report the one case this guard exists for, which is the two of them
    // disagreeing.
    //
    // **The mean only comes out when there was something to paint.** A «0 per frame» invites reading
    // that the strip is free, which is the opposite of what a run with the gate on is saying.
    let gate = if strips.painted_this_run {
        "the session paints them"
    } else {
        "switched off: the measurement gate vetoes the strip, which is what keeps this run \
         comparable with the published series"
    };
    match strips.per_frame() {
        Some(mean) => writeln!(
            a,
            "header:       {} frame(s) with a strip painted · {:?} in total · {mean:?} per frame \
             ({gate}; every console's strip of that frame, subtracted from it)",
            strips.frames, strips.total
        )?,
        // **This branch IS the zero case, and saying so is the point**: `per_frame` divides by the
        // frames, so it answers nothing exactly when none was painted. The count is still read off
        // the tally rather than written as a literal `0`, so the line and the tally cannot drift
        // apart if what decides the mean ever changes — but it is not a guard and it does not
        // pretend to be one. A tally with frames at zero and a total above it is a shape the three
        // public fields allow and `record` cannot produce; nothing here reports it, and nothing
        // needs to.
        None => writeln!(
            a,
            "header:       {} frame(s) with a strip painted ({gate})",
            strips.frames
        )?,
    }
    writeln!(a, "renderer:     glow")?;
    writeln!(a, "profile:      {profile}")?;
    writeln!(a, "machine:      {}", machine())
}

/// What each console is called in the report, by its place in the window.
///
/// A bare number forces guessing which side it is, and the width tiling makes the side matter.
pub const CONSOLE_NAME: [&str; 2] = ["left", "right"];

/// Which side a pane falls on, **if the side means anything at all**.
///
/// **"Left" and "right" only describe something when there are exactly two**, so with any other number
/// this returns nothing instead of inventing a side. It is not a just-in-case guard: until the header
/// stopped receiving an array of two, the number was fixed by the type, and passing it to a list —which
/// is what the pane tree needs— the guarantee ceased to exist. Indexing the array by position turned
/// into a panic **inside the report that is published on exit**, that is, losing at a stroke everything
/// measured in the session.
///
/// The three places in the report that name a pane use it —the header, the title of its span table and
/// the focus line—, each with its own decoration around it. That the rule lives in a single place is
/// what keeps one of the three from indexing on its own the day splitting a pane stops being a rarity:
/// since it can be split, three panes are one gesture.
pub fn side(how_many: usize, which: usize) -> Option<&'static str> {
    match CONSOLE_NAME.get(which) {
        Some(side_name) if how_many == CONSOLE_NAME.len() => Some(side_name),
        _ => None,
    }
}

/// The side hint as the header carries it: in parentheses and with its indentation, or nothing.
fn where_it_falls(how_many: usize, which: usize) -> String {
    side(how_many, which).map_or_else(String::new, |side_name| format!("  ({side_name})"))
}

/// What this machine is called, or "unknown" if there is no way to know.
///
/// **It never stays silent.** The criterion asks for the machine alongside the number, and a report
/// missing that line silently is a number whose origin tomorrow is unknown. `HOSTNAME` is a shell
/// variable and is not usually exported, so relying on it alone was exactly that silent failure: in the
/// first measured dump the line did not come out and nobody noticed.
fn machine() -> String {
    if let Ok(name) = std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")) {
        return name;
    }
    // The file is Linux's and the BSDs'; on other systems it falls back to the literal, which is the
    // honest answer and not an invention.
    std::fs::read_to_string("/etc/hostname")
        .map(|name| name.trim().to_owned())
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every one of these numbers but the last is summed, and the last is a maximum — and nothing
    /// in their types says which.** The choice used to live in a closure at the fold, where no test could reach it:
    /// turning the maximum into a sum compiled and left the whole suite green, and the only way to
    /// catch it was reading a two-pane report by hand. This is what binds it.
    #[test]
    fn two_panes_sum_their_counts_and_keep_the_larger_document() {
        let one = Control {
            applied: 3,
            refused: 1,
            released: 2,
            read_back: 4,
            read_refused: 2,
            patched: 7,
            patches_refused: 1,
            largest_serialized: 900,
            ..Control::default()
        };
        let other = Control {
            applied: 5,
            refused: 0,
            released: 1,
            read_back: 6,
            read_refused: 3,
            patched: 2,
            patches_refused: 4,
            largest_serialized: 120,
            ..Control::default()
        };

        let both = one.and(other);

        assert_eq!(both.applied, 8);
        assert_eq!(both.refused, 1);
        assert_eq!(both.released, 3);
        assert_eq!(both.read_back, 10);
        assert_eq!(both.read_refused, 5);
        assert_eq!(both.patched, 9);
        assert_eq!(both.patches_refused, 5);
        assert_eq!(
            both.largest_serialized, 900,
            "the size published named a document nobody wrote"
        );
    }
}
