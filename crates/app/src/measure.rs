//! The two numbers this milestone has to publish, and their labels.
//!
//! **A mislabelled number is worse than no number**, and here that is not a figure of speech: the
//! milestone exists to decide whether egui holds up, and that decision is made by reading this. That
//! is why each number says exactly which span it measures and which it does not.
//!
//! **What is measured on the way out:** from when the interface pass that reads the key begins until
//! its bytes are written to the pseudoterminal. **And what is not**: the stretch between pressing the
//! key and that pass. It is not an oversight — it is not measurable from inside the process: egui's
//! keyboard events carry no timestamp of their own, and the clock of its input is documented as
//! "monotonic, relative to something unspecified". A number called "key → pixel" would be an invented
//! number.
//!
//! **What is measured on the way back:** from when the byte was available to be picked up until the
//! frame that shows it is paid for, in **five spans that add up to the total**:
//!
//! 1. **Waiting** — in the queue, until the loop picks it up. In a dump it is the span that
//!    dominates, and by far: the per-frame budget lets 128 KiB through each time, so what does not
//!    fit waits its turn.
//! 2. **Interpreting** — putting it into the terminal grid.
//! 3. **Enqueuing** — the call to the painter. It is called that because that is what it does: it
//!    stacks shapes and returns.
//! 4. **The status panel** — checking whether its file changed, reading and interpreting it when it
//!    does, splitting how much width it takes, **serving the drag of its border** and enqueuing its
//!    paint. It goes apart from the previous span even though both are work of the same frame:
//!    **grouping them would give the same subtraction with a false label**, and here that already
//!    cost a whole measurement.
//! 5. **The rest of the frame** — tessellating those shapes —**the consoles' and the panels'**—,
//!    drawing them, and all the scaffolding egui builds around it: gathering the desktop's input,
//!    opening and closing the pass, measuring the font, uploading textures. It is what is left of the
//!    total cost once the previous spans are taken away, and **it is called "the rest" on purpose**:
//!    from inside there is no way to isolate the tessellation, so naming it that would be publishing
//!    as measured something that is not. The panel span covers **enqueuing** its paint, not
//!    tessellating it.
//!
//! The total is published by `eframe`, which is the one that can: by the time tessellation happens,
//! the pass is already over.
//!
//! **The first four spans belong to a pane; the fifth belongs to the window.** That boundary is not
//! organizational, the instrument imposes it: the four are timed around *one* console, and so they
//! stay comparable with the D3 tables whether there is one pane or four. The CPU consumption, by
//! contrast, is published by the framework **only once per pass**: it does not come split by pane and
//! splitting it would be inventing an allocation nobody has measured. That is why the rest is computed
//! once, subtracting from the total the sum of what **all** the panes timed.
//!
//! The five spans are saved **from the same frame**. The total cost arrives one frame late —`eframe`
//! publishes it from the previous one—, so the spans are held until their total appears; subtracting
//! one frame's spans from another's total would give a number that looks good and is not.

use std::collections::VecDeque;
use std::fmt;
use std::time::{Duration, Instant};

/// How many samples are kept of each number.
///
/// A cap, and not all of them: a long session would accumulate millions of samples and the report
/// would measure mostly the stretch when nothing was happening. With the latest ones, what is
/// published describes how things were going **while it was being used**, which is what matters.
const SAMPLES: usize = 4096;

/// A series of times, with what is needed to summarize it.
#[derive(Default)]
struct Series {
    samples: VecDeque<Duration>,
    /// How many have been taken in total, including the ones no longer kept. Without this, a report
    /// of 4,096 samples over a session of a million would look like the whole session.
    total: usize,
}

impl Series {
    fn record(&mut self, how_long: Duration) {
        self.total += 1;
        // A double-ended queue, and not a vector: pulling the oldest sample from the front of a
        // vector shifts the other four thousand, **inside the frame the instrument is measuring**.
        // The instrument's cost would end up inside the span the instrument publishes as "the rest",
        // which is precisely the one that has to be explainable.
        if self.samples.len() == SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(how_long);
    }

    fn mean(&self) -> Duration {
        let sum: Duration = self.samples.iter().sum();
        sum / self.samples.len().max(1) as u32
    }

    /// The value below which 95% of the samples fall.
    ///
    /// It is published alongside the mean because **the worst of all is not a useful figure here**: a
    /// single pause of the desktop's collector or a context switch spikes it and hides the shape of
    /// the distribution. The 95th is the one you notice while using it.
    fn p95(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        let mut sorted: Vec<Duration> = self.samples.iter().copied().collect();
        sorted.sort_unstable();
        let index = (sorted.len() * 95 / 100).min(sorted.len() - 1);
        sorted[index]
    }

    fn has_samples(&self) -> bool {
        !self.samples.is_empty()
    }
}

impl fmt::Display for Series {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3?} mean, {:.3?} p95", self.mean(), self.p95())
    }
}

/// The spans of a frame, waiting for their total cost to arrive.
///
/// **It is public and built with named fields on purpose.** When this was passed as three positional
/// durations in a row, swapping two compiled without a complaint and published a lying report — exactly
/// the failure that separating the spans exists to prevent. A span that can be confused with another in
/// the call is not separated from anything.
#[derive(Clone, Copy)]
pub struct Spans {
    /// How long the oldest byte waited in the queue, or `None` if nothing reached this console in this
    /// frame.
    ///
    /// **Being optional is what separates two things that are not the same:** if no output arrived,
    /// there is no *way back* to measure —a frame at rest is not a lightning-fast round trip—, but the
    /// console **did paint**, and that work has to be returned so the window subtracts it from the
    /// total. When this was a bare `Duration`, the frame with no output was discarded whole and its
    /// painting ended up published as "the rest of the frame".
    waited: Option<Duration>,
    interpreted: Duration,
    enqueued: Duration,
    /// What it cost to check whether this console's `panel.json` changed, read and interpret it if it
    /// did, split the width, **serve the panel's border** and **paint it**.
    ///
    /// **All six things, and saying it in full matters.** The number is made up of **two disjoint
    /// spans** of `crate::Window::paint_pane`, summed: one before painting the console —checking the
    /// timestamp, reading whether it changed, splitting the width, serving the drag of its border and
    /// splitting again if the width changed— and another after —painting the panel—. What is left **in
    /// between**, which is enqueuing the console's paint, has its own field and does not enter here.
    /// Describing the whole thing as "reading" would leave the panel's painting with no declared owner,
    /// and a span with no owner is taken by the one computed by subtraction — which here is the
    /// interface framework, that is, precisely the figure D3 has to decide about.
    ///
    /// **It goes apart and not summed into `enqueued`, and that separation is the reason the field
    /// exists.** Both happen inside the frame and both have to be returned, so for the subtraction it
    /// would make no difference. But a span called "enqueuing the paint" that also included reading a
    /// file from disk would be a number with the label swapped, and here that already cost a whole
    /// measurement: the cost that is not named well ends up attributed to whoever did not do it.
    panel: Duration,
}

impl Spans {
    /// The spans of a round trip, with each duration set by its name.
    ///
    /// `waiting_since` is when the oldest of the collected bytes arrived; with no bytes in this frame
    /// there is nothing to measure of the round trip — a frame at rest is not a lightning-fast round
    /// trip, it is a round trip that did not happen.
    pub fn of_a_round(
        waiting_since: Option<Instant>,
        collected_at: Instant,
        interpreted: Duration,
        enqueued: Duration,
        panel: Duration,
    ) -> Self {
        Self {
            waited: waiting_since.map(|since| collected_at.saturating_duration_since(since)),
            interpreted,
            enqueued,
            panel,
        }
    }

    /// Merges the spans of **another pass of the same frame**.
    ///
    /// egui may repeat the pass over the same input, and `eframe` calls the two halves of the
    /// application inside the closure that repeats (`epi_integration.rs:288-303`), so in a repeated
    /// frame it pumps and paints again. **Both jobs happened**, and both have to be returned to the
    /// frame's subtraction: summing them is what tells the truth.
    ///
    /// **The wait keeps the first one there was, and they are not summed.** The wait is not the
    /// application's work —it is how long the oldest byte had been in the queue—, so summing two waits
    /// would give a number nobody waited. The first pass's is the one from the batch that really
    /// waited; the second picks up an already-empty queue.
    /// What has to be returned to the frame's subtraction: everything the application really did.
    ///
    /// The wait does not enter, and it is not an oversight: **it is not work**, it is how long the byte
    /// had been in the queue.
    fn declarable(&self) -> Duration {
        self.interpreted + self.enqueued + self.panel
    }

    fn and_also(self, other: Self) -> Self {
        Self {
            waited: self.waited.or(other.waited),
            interpreted: self.interpreted + other.interpreted,
            enqueued: self.enqueued + other.enqueued,
            panel: self.panel + other.panel,
        }
    }
}

/// The stopwatches **of one pane** and what they have measured so far.
///
/// Everything here is timed around a specific console. The fourth span does not fit here: see
/// [`RestOfFrame`].
///
/// **Comparing these numbers with those of an earlier measurement requires looking at the grid, not
/// just the span.** `enqueue` is proportional to the number of cells and `interpret` depends on the
/// width over which the text wraps, so a console that takes up half the window gives them lower than
/// one at full screen **without anything having improved**. That is why the report header publishes
/// each console's grid: reading "enqueueing the paint went down" as "the framework holds up better" is
/// the error this warning exists to avoid. The only span whose definition does not change with the
/// pane is `wait`.
#[derive(Default)]
pub struct Timings {
    outbound: Series,
    /// **Encoding** a mouse gesture and writing it to the program.
    ///
    /// **The label says exactly what the clock spans, and that cost a correction.** The first version
    /// published it as "from the frame that sees the gesture to the byte", which is what the key series
    /// measures; this clock starts **much later** —walking the tree, splitting the pointer and
    /// resolving the cell stay outside, inside the clock that already times serving the whole mouse— so
    /// under that label it published a span strictly shorter than the one it named. **It is not
    /// comparable with the key's way out**, and that is why they do not share a name.
    ///
    /// **Its own series and not the key's** also for what it measures: a drag drops **dozens of samples
    /// per gesture** —one per cell crossed— against the single one a key drops, so averaging them
    /// together would leave the figure D3 publishes describing mostly the mouse.
    mouse: Series,
    wait: Series,
    interpret: Series,
    enqueue: Series,
    /// Checking the `panel.json` timestamp, reading and interpreting it when it has changed, splitting
    /// the width, **serving the drag of its border** and **painting it**.
    panel: Series,
    round: Series,
    /// The previous frame's spans, waiting for `eframe` to publish what it cost.
    pending: Option<Spans>,
}

impl Timings {
    /// Records how long the way out took: from starting the interface pass to writing to the program.
    pub fn record_outbound(&mut self, how_long: Duration) {
        self.outbound.record(how_long);
    }

    /// Records how long it took to encode a mouse gesture and write it to the program.
    pub fn record_mouse_outbound(&mut self, how_long: Duration) {
        self.mouse.record(how_long);
    }

    /// Records the spans of this frame. They stay pending until what it cost as a whole is known.
    ///
    /// **It is always saved, even with no output.** What decides whether there was a round trip is
    /// that the wait is `Some`; the work of painting happened in both cases and the window has to be
    /// able to subtract it.
    ///
    /// **It is ADDED to whatever was already pending, and this fixes a bias that was alive.** It used
    /// to replace, and the frame with the repeated pass —egui repeats it when something asks to
    /// recompose— reached here twice: the second recording, with the interpreted at almost zero because
    /// the queue was already empty, **clobbered the good one**. What the console had really interpreted
    /// was published as the interface framework's cost, which is the figure D3 decides with. Measured
    /// before the fix: **50 ns declared where interpreting had cost 20.158 µs**. See
    /// [`Spans::and_also`] for why the wait is not summed.
    ///
    /// **Summing is only correct because the frame is closed once, and the two things go together.** If
    /// the close happened per pass, this would accumulate the first pass's spans inside the second's
    /// total; and if the close were per frame but this replaced, the first pass's would be lost. What
    /// makes it fit: in the logical half of frame **N** it is closed with the spans of **N-1** and the
    /// cost of **N-1** —`eframe` publishes the total one frame late—, so what is summed here is always
    /// the passes **of a single frame**. Whoever touches one of the two pieces has to look at the
    /// other.
    pub fn record_round(&mut self, spans: Spans) {
        self.pending = Some(match self.pending.take() {
            Some(already_there) => already_there.and_also(spans),
            None => spans,
        });
    }

    /// Closes this pane's previous frame with what `eframe` says it cost in CPU.
    ///
    /// Returns **what this pane had timed** —interpreting plus enqueuing—, so the window can subtract
    /// everyone's at once and get the rest. It returns it **whether there was output or not**: painting
    /// happens in every frame, and if this console did not declare it, its painting would appear
    /// published as "the rest of the frame", which is the number that decides whether the interface
    /// framework holds up. `None` only when there was not even an interface pass to measure.
    ///
    /// **The four series, by contrast, only count when output arrived**: a frame at rest is not a
    /// lightning-fast round trip, it is a round trip that did not happen, and putting it into the mean
    /// would say the application responds instantly precisely for having done nothing.
    pub fn close_frame(&mut self, cpu_cost: Duration) -> Option<Duration> {
        let spans = self.pending.take()?;

        if let Some(waited) = spans.waited {
            self.wait.record(waited);
            self.interpret.record(spans.interpreted);
            self.enqueue.record(spans.enqueued);
            self.panel.record(spans.panel);
            self.round.record(waited + cpu_cost);
        }

        Some(spans.declarable())
    }

    /// Takes what was still pending **without recording any mean**, and returns what has to be
    /// subtracted.
    ///
    /// **It is for the pane that leaves with its tab.** Its spans happened and have to be declared, or
    /// the rest of the frame keeps its work. But nobody is going to publish its means —the report walks
    /// the live tabs—, and closing it by the normal path would require inventing a frame cost for the
    /// round-trip series: with a zero, an artificially short round trip; with the real one, a sample of
    /// a frame this console is no longer going to finish. The honest thing is to record nothing and keep
    /// only what the subtraction needs.
    pub fn take_pending(&mut self) -> Duration {
        self.pending
            .take()
            .map_or(Duration::ZERO, |spans| spans.declarable())
    }

    /// What this pane has recorded so far of the panel span.
    ///
    /// It exists so a test can assert that **the real path declares that span**, and not just that the
    /// recording works when called by hand. Without this, leaving the panel cost at zero inside the
    /// interface pass broke nothing.
    #[cfg(test)]
    pub fn recorded_panel(&self) -> Option<Duration> {
        self.panel.has_samples().then(|| self.panel.mean())
    }

    /// Whether any of the four means has samples.
    ///
    /// It exists for the same reason as [`Timings::recorded_panel`]: a test can assert over the real
    /// path that a console **that did not paint** does not dirty any mean. Checking it by calling the
    /// recording by hand proves that the recording works, not that the caller passes it the right
    /// thing.
    #[cfg(test)]
    pub fn has_means(&self) -> bool {
        self.wait.has_samples()
            || self.interpret.has_samples()
            || self.enqueue.has_samples()
            || self.panel.has_samples()
    }

    /// Whether this pane got to measure anything, so as not to publish a blank console's report.
    ///
    /// **The mouse counts.** A batch that only injects gestures and neither types nor receives output
    /// has its samples here and nowhere else; without this condition its report would come out blank and
    /// the run would read as if the gesture cost nothing.
    pub fn has_any(&self) -> bool {
        self.outbound.has_samples() || self.mouse.has_samples() || self.round.has_samples()
    }

    /// Publishes **this pane's** numbers to whatever destination it is given.
    ///
    /// It is published on close and not during: the window is the terminal, so there is no place inside
    /// to show this without covering the program's screen.
    ///
    /// **The destination is received, and it is not a convenience: it is what makes the test honest.**
    /// The property that has to be tied down is that the published text **mentions that the panel span
    /// includes painting it**, because whether whoever copies the number into a document attributes it
    /// to someone else depends on that. With a `println!` inside there is no way to read what comes out,
    /// and the alternative —asserting over the constant— would prove that the constant is well worded,
    /// not that anyone prints it.
    ///
    /// **And that is why this has no wrapper.** The first version left a `publish_spans()` with no
    /// arguments that called this function with standard output, and the test entered through here:
    /// emptying the wrapper left the suite **green**, which is the ninth time this project ties down the
    /// helper instead of the real path. Now the production door and the one that is tested are the same,
    /// and the only thing the caller contributes is where to write.
    pub fn publish_spans(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if self.outbound.has_samples() {
            writeln!(
                a,
                "\nkey in the frame → byte in the program   ({} keystrokes)",
                self.outbound.total
            )?;
            writeln!(a, "  {}", self.outbound)?;
            writeln!(
                a,
                "  does not include the stretch from the keystroke to the frame: not measurable from here"
            )?;
        }

        if self.mouse.has_samples() {
            writeln!(
                a,
                "\nencoding the mouse gesture → byte in the program   ({} mouse reports)",
                self.mouse.total
            )?;
            writeln!(a, "  {}", self.mouse)?;
            writeln!(
                a,
                "  a drag drops one sample per cell crossed, not one per gesture"
            )?;
            writeln!(
                a,
                "  does NOT cover the whole pass the way the key span does: placing the pointer and"
            )?;
            writeln!(
                a,
                "  resolving the cell stay outside, inside the clock that serves the mouse; the two"
            )?;
            writeln!(a, "  numbers are not comparable")?;
        }

        if self.round.has_samples() {
            writeln!(
                a,
                "\nbyte available → frame paid for   ({} frames with output)",
                self.round.total
            )?;
            writeln!(a, "  {}", self.round)?;
            writeln!(a, "  waiting in the queue:   {}", self.wait)?;
            writeln!(a, "  interpreting:           {}", self.interpret)?;
            writeln!(a, "  enqueueing the paint:   {}", self.enqueue)?;
            writeln!(a, "  the status panel:       {}", self.panel)?;
            // **This span's mean mixes two populations and it has to be said.** Almost every frame
            // only checks the file timestamp and paints the already-laid-out text; the few in which
            // the producer has published cost, on top of that, reading, interpreting and redoing the
            // layout, which is three orders of magnitude more. The mean comes out stuck to the cheap
            // case, so the number that describes the expensive case is the 95th, not it.
            //
            // **And the span includes painting**, not only reading: the clock runs from before
            // checking the timestamp to after painting the last row. Saying it here is what keeps
            // whoever copies this number into a document from attributing it to someone else.
            writeln!(a, "{PANEL_EXPLANATION}")?;
            writeln!(
                a,
                "  it includes the whole frame, the other consoles' painting too: a frame\n  \
                 presents itself as indivisible. That is why two of these numbers do NOT add up"
            )?;
        }

        Ok(())
    }
}

/// What the report says under the panel span's line.
///
/// **The two sentences are there for different reasons and neither is decoration.** The first warns
/// that the mean and the 95th describe different populations, so whoever cites "the mean" will be
/// citing the case in which there was no publishing. The second says **what the span spans**, and it
/// exists because throughout the whole F1.6 milestone this number was described as "reading the panel"
/// when the clock includes painting it: a mislabelled span ends up attributed to whoever did not do
/// it, and here the one that takes what nobody declares is the interface framework, which is precisely
/// the figure D3 decides.
const PANEL_EXPLANATION: &str = "\
    \x20   mixes checking the timestamp and painting the already laid-out text (almost\n\
    \x20   every frame) with reading, interpreting and laying the text out again (the\n\
    \x20   publishing ones): the mean describes the former, the 95th the latter\n\
    \x20   it is two clocks, one before and one after painting the console —which stays\n\
    \x20   outside—: this span INCLUDES painting the panel, not only reading it\n\
    \x20   and also serving the drag of its border and splitting the width again if it\n\
    \x20   changed: the gesture that narrows the panel is paid here, not in the rest of\n\
    \x20   the frame";

/// The rest of the frame: what the pass cost and nobody timed.
///
/// **It lives in the window and not in the pane, and it is not a style choice.** The framework
/// publishes a single CPU consumption per pass, without splitting by pane; splitting it among the
/// panes would be inventing an allocation nobody has measured, and recording it once per pane would
/// count the same cost as many times as there are consoles open.
#[derive(Default)]
pub struct RestOfFrame {
    series: Series,
}

impl RestOfFrame {
    /// Records the rest of this frame: what it cost as a whole minus what **all** the panes timed.
    pub fn record(&mut self, cpu_cost: Duration, timed: Duration) {
        // The total may come out below the measured spans: they are from contiguous but not identical
        // frames, and `eframe`'s clock is not the same as the one here. When that happens, the rest is
        // zero instead of a negative number that looks like data.
        self.series.record(cpu_cost.saturating_sub(timed));
    }

    /// What has been published as the rest, on average.
    ///
    /// **Only the tests use it, and that is why it is behind its flag.** It is what makes it possible
    /// to assert that a declared job **is really subtracted**: without this, a test can only check that
    /// someone wrote the number into a field, which is tying down the helper and not the path — and
    /// that gap already let a mutation through in this very work.
    #[cfg(test)]
    pub fn mean(&self) -> Duration {
        self.series.mean()
    }

    /// How many times a rest has been recorded, that is, **how many frames have been closed**.
    ///
    /// **Only the tests use it, and that is why it is behind its flag.** It is the only observable that
    /// distinguishes a frame closed once from one closed twice: the means do not give it away —two
    /// closes of the same frame record a similar value— and neither does the report. It is what is
    /// needed to tie down the guard that prevents the double close.
    #[cfg(test)]
    pub fn closed_frames(&self) -> usize {
        self.series.total
    }

    pub fn publish(&self, a: &mut dyn std::io::Write) -> std::io::Result<()> {
        if !self.series.has_samples() {
            return Ok(());
        }
        writeln!(
            a,
            "\nthe rest of the frame — the window's, shared by every console   ({} frames)",
            self.series.total
        )?;
        writeln!(a, "  {}", self.series)?;
        writeln!(
            a,
            "  it is tessellating, painting and egui's scaffolding: from inside they cannot be\n  \
             separated, and calling it \"tessellation\" would publish what is not measured as measured"
        )?;
        writeln!(
            a,
            "  every console of every tab already has its timing subtracted from it:\n  \
             also the painting of one that received no output, and the interpreting of one\n  \
             that never got painted for being in a background tab"
        )
    }
}

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
    /// project already uses with the injected gestures: this number lives inside the process. The script
    /// invalidates the run if it is zero, because a batch that says it measures the saving and saves
    /// nothing would publish the cost of a path that was not walked.
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
                "  covers copying the layout, VALIDATING it, serializing it, resolving the path, creating\n                   the directory the first time, writing the temporary and renaming it over; and\n                   if something fails, deleting the temporary and stating the failing line. It only\n                   happens when the deadline expires, not every frame: an attempt that never\n                   reaches the disk is almost all validation"
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
/// is done where it is needed — inside "enqueuing the paint". Without subtracting it from there, that
/// span would stop meaning what its name says, and this project already has it written that a
/// mislabelled number is worse than no number.
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
            "  does NOT happen every frame: only the first time an emoji appears at a given cell\n                   height, so this mean does not describe an ordinary frame. It covers searching the\n                   system's emoji families for the one that carries it in color, decoding its bitmap\n                   —in Noto Color Emoji an indexed 136x128 PNG—, scaling it down to the cell and\n                   uploading the texture. A character NO font paints in color is paid all the same\n                   and returns nothing: that is why the two counters are separate"
        )?;
        writeln!(
            a,
            "  IT IS SUBTRACTED from the paying console's \"enqueueing the paint\", not added on top: it\n                   runs WITHIN that span, the opposite of searching a font. Without the subtraction,\n                   the span that claims to measure the cost of enqueueing would also measure this"
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
    /// The caller is the frame close, not the painting: egui repeats the pass over the same input and
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
/// **It is a struct with names and not four parameters in a row**, for the same reason as [`Spans`]:
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
            "chosen font:  none (no family was asked for, so the machine was not enumerated)"
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

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// What the report really publishes, read from the same path it writes to the screen with.
    fn report_of(timings: &Timings) -> String {
        let mut salida = Vec::new();
        timings
            .publish_spans(&mut salida)
            .expect("writing to memory does not fail");
        String::from_utf8(salida).expect("the report is text")
    }

    /// The header that is really published, by the same path it writes on exit.
    ///
    /// The window size is fixed because none of the tests here talk about it; the one that does passes
    /// it on its own.
    fn header_of(grids: &[(usize, Vec<(usize, term_core::Size)>)], visible: usize) -> String {
        header_with(
            SizeInPoints {
                width: 800.0,
                height: 600.0,
            },
            grids,
            visible,
        )
    }

    fn header_with(
        window: SizeInPoints,
        grids: &[(usize, Vec<(usize, term_core::Size)>)],
        visible: usize,
    ) -> String {
        header_of_a_run(window, grids, visible, None)
    }

    fn header_of_a_run(
        window: SizeInPoints,
        grids: &[(usize, Vec<(usize, term_core::Size)>)],
        visible: usize,
        drags: Option<Drags>,
    ) -> String {
        full_header(window, grids, visible, drags, None)
    }

    fn full_header(
        window: SizeInPoints,
        grids: &[(usize, Vec<(usize, term_core::Size)>)],
        visible: usize,
        drags: Option<Drags>,
        restoring: Option<Restoring>,
    ) -> String {
        with_tab_bar(window, grids, visible, drags, restoring, TabBar::default())
    }

    /// The header with a concrete configuration state, which is what the measurement script reads.
    fn with_settings(settings: ReadingSettings) -> String {
        let mut salida = Vec::new();
        header(
            &mut salida,
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39))])],
            0,
            SessionHeader {
                drags: None,
                restoring: None,
                settings,
                chosen_font: ChoosingTheFont::NotAsked,
                tab_bar: TabBar::default(),
                reflow: Reflow::default(),
            },
        )
        .expect("writing to memory does not fail");
        String::from_utf8(salida).expect("the report is text")
    }

    #[test]
    fn the_three_configuration_states_are_distinguished_where_the_script_looks() {
        // **The script invalidates a run by reading the FIRST NOUN of this line**, so its shape is as
        // much a contract as a file name. Changing the text left the whole suite green and **every
        // measurement batch would come out invalid** — or, worse, one that broke the veto would pass as
        // good.
        //
        // **And the three have to start differently.** The script empties the configuration directory,
        // so a broken veto on the bench produces "checked and there was nothing": if that started the
        // same as the vetoed one, the broken veto would read as a good run. A review caught it, after
        // another had asked to separate the states without seeing this consequence.
        let vetada = with_settings(ReadingSettings::Vetoed);
        let line = vetada
            .lines()
            .find(|l| l.starts_with("settings:"))
            .expect("there is no configuration line the script can read");
        assert!(
            line.starts_with("settings:     vetoed"),
            "the vetoed configuration line no longer has the shape the script knows how to read: '{line}'"
        );

        let sin = with_settings(ReadingSettings::Missing);
        let line = sin
            .lines()
            .find(|l| l.starts_with("settings:"))
            .expect("there is no configuration line");
        assert!(
            !line.starts_with("settings:     vetoed"),
            "'checked and there was nothing' starts the same as 'vetoed', so a broken veto on the bench \
             —where there is no file by construction— would pass as a good run: '{line}'"
        );

        let con = with_settings(ReadingSettings::Applied {
            how_long: Duration::from_micros(25),
            warnings: 0,
        });
        let line = con
            .lines()
            .find(|l| l.starts_with("settings:"))
            .expect("there is no configuration line");
        assert!(
            line.starts_with("settings:     the user's"),
            "the applied configuration line changed shape: '{line}'"
        );
    }

    /// The header with a concrete chosen-font state, which is the other line the script reads.
    fn with_chosen_font(chosen_font: ChoosingTheFont) -> String {
        let mut salida = Vec::new();
        header(
            &mut salida,
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39))])],
            0,
            SessionHeader {
                drags: None,
                restoring: None,
                settings: ReadingSettings::default(),
                chosen_font,
                tab_bar: TabBar::default(),
                reflow: Reflow::default(),
            },
        )
        .expect("writing to memory does not fail");
        String::from_utf8(salida).expect("the report is text")
    }

    #[test]
    fn the_four_chosen_font_states_are_distinguished_where_the_script_looks() {
        // **The script invalidates a run by reading the FIRST WORD of this line**, exactly like its
        // sibling above, and it is the only guard that can see this path: asking for a family by name
        // enumerates the machine —between 17.3 and 23.2 ms— and **does not go through the discovery
        // counters**, so the guard that catches a batch searching for fonts without dumping Japanese
        // does not notice it.
        //
        // **Each state is asserted by the word only IT carries, and that is the whole point of this
        // test.** Written the obvious way —«it does not start with none» plus the family name— the
        // four lines are interchangeable: every one of them carries the name, so swapping two bodies
        // left the suite green and the report could say a font was found when it was not. A reviewer
        // caught it by swapping them.
        let word_of = |line: &str| {
            line.split_whitespace()
                .nth(2)
                .unwrap_or_default()
                .to_owned()
        };
        let line_of = |report: &str| {
            report
                .lines()
                .find(|l| l.starts_with("chosen font:"))
                .expect("there is no chosen-font line the script can read")
                .to_owned()
        };

        let asked_for = "JetBrains Mono".to_owned();
        let none = line_of(&with_chosen_font(ChoosingTheFont::NotAsked));
        let found = line_of(&with_chosen_font(ChoosingTheFont::Found {
            family: asked_for.clone(),
            how_long: Duration::from_millis(21),
        }));
        let missing = line_of(&with_chosen_font(ChoosingTheFont::Missing {
            asked_for: asked_for.clone(),
            how_long: Duration::from_millis(19),
        }));
        let unusable = line_of(&with_chosen_font(ChoosingTheFont::Unusable {
            asked_for,
            how_long: Duration::from_millis(17),
        }));

        assert_eq!(
            word_of(&none),
            "none",
            "the line for «no font was asked for» no longer starts the way the script reads it: '{none}'"
        );
        // **The four first words have to differ**, which is what makes the line readable at all: the
        // script decides with the first, and a person decides what to go and fix with it.
        let words = [
            word_of(&none),
            word_of(&found),
            word_of(&missing),
            word_of(&unusable),
        ];
        let distinct: std::collections::BTreeSet<_> = words.iter().collect();
        assert_eq!(
            distinct.len(),
            4,
            "two of the four states are published under the same word, so they cannot be told apart: {words:?}"
        );
        // And each still says what it is worth: which family, and what looking for it cost.
        assert!(
            found.contains("JetBrains Mono") && found.contains("21"),
            "the found line says neither which family is being painted with nor what it cost: '{found}'"
        );
        assert!(
            missing.contains("JetBrains Mono") && missing.contains("19"),
            "the search that found nothing publishes no time, so its milliseconds fall into the \
             nameless start-up: '{missing}'"
        );
        assert!(
            unusable.contains("JetBrains Mono") && unusable.contains("17"),
            "the family that is installed and cannot be opened publishes no time either: '{unusable}'"
        );
    }

    fn with_tab_bar(
        window: SizeInPoints,
        grids: &[(usize, Vec<(usize, term_core::Size)>)],
        visible: usize,
        drags: Option<Drags>,
        restoring: Option<Restoring>,
        tab_bar: TabBar,
    ) -> String {
        let mut salida = Vec::new();
        header(
            &mut salida,
            window,
            grids,
            visible,
            SessionHeader {
                drags,
                restoring,
                settings: ReadingSettings::default(),
                chosen_font: ChoosingTheFont::NotAsked,
                tab_bar,
                reflow: Reflow::default(),
            },
        )
        .expect("writing to memory does not fail");
        String::from_utf8(salida).expect("the header is text")
    }

    #[test]
    fn the_header_declares_where_the_consoles_came_from_only_when_they_came_from_the_file() {
        // **It is the line that distinguishes the batch that restores from the one that asks.** Without
        // it, a run that opened from factory with four consoles and one that took them out of the file
        // publish **the same report**: the header says how many there are, not who asked for them. And
        // the script invalidates the run by reading precisely this line, so if it stops coming out, batch
        // L goes on to measure the usual startup without anything saying so.
        let restaurada = full_header(
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39)), (1, grid(49, 39))])],
            0,
            None,
            Some(Restoring {
                consoles: 4,
                read: Some(Duration::from_micros(310)),
                opening: Duration::from_millis(12),
            }),
        );
        assert!(
            restaurada.contains("restored:     4 consoles"),
            "the header did not declare the consoles that came out of the file:\n{restaurada}"
        );
        // **The two times, and separately.** Publishing only the sum would give a number that cannot be
        // extrapolated: reading does not grow with the number of consoles and opening them does.
        assert!(
            restaurada.contains("310µs") && restaurada.contains("12ms"),
            "the header did not declare what it cost to read and what it cost to open them:\n{restaurada}"
        );

        let from_factory = full_header(
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39)), (1, grid(49, 39))])],
            0,
            None,
            None,
        );
        assert!(
            !from_factory.contains("restored"),
            "a window opened from factory says it restored, which is the disguise this line exists to \
             remove:\n{from_factory}"
        );
    }

    #[test]
    fn a_read_nobody_took_is_published_with_those_words_and_not_as_a_zero() {
        // **The branch that makes the `Option` honest, and no test exercised it.** The field became
        // optional precisely so that a window raised without touching the disk would not publish `0ns to
        // read and validate` —indistinguishable from a truly instant read—, and yet the only case that
        // passed through here brought a number: changing the empty branch's text to a zero, or to the sum
        // of the two times, left the whole file green.
        //
        // **What is asserted is what is read, not that there is an `Option` inside.** The header is the
        // only thing that reaches whoever interprets the batch, and there the difference between "not
        // measured" and "0ns" is the difference between knowing nobody measured and believing a number.
        let sin_medir = full_header(
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39)), (1, grid(49, 39))])],
            0,
            None,
            Some(Restoring {
                consoles: 2,
                read: None,
                opening: Duration::from_millis(7),
            }),
        );

        assert!(
            sin_medir.contains("not measured to read and validate"),
            "a read nobody took is not declared as such:\n{sin_medir}"
        );
        assert!(
            sin_medir.contains("7ms to open them"),
            "the line lost what was measured:\n{sin_medir}"
        );
        // **And in the shape the script knows how to read, even without a read number.** Batch L's guard
        // pulls the console count with `s/^restored: *\([0-9]*\) consoles .*$/\1/p`, and that `.*` after
        // "consoles" **requires there to still be something**: a line that stopped at `restored: 2
        // consoles` would not match, the script would see no restored console and would invalidate a good
        // run by saying the window opened from factory.
        let line = sin_medir
            .lines()
            .find(|line| line.starts_with("restored:"))
            .expect("there is no restored line the script can read");
        assert!(
            line.starts_with("restored:     2 consoles ·"),
            "the restored line no longer has the shape the measurement script knows how to read: '{line}'"
        );
    }

    #[test]
    fn the_header_declares_the_drags_only_when_the_batch_had_them() {
        // **This test exists because the helper here never got to pass `Some`.** The drags line was only
        // covered by a test in `lib.rs` that enters through the whole report, so emptying this block left
        // **all** the tests in this file green. The review caught it.
        //
        // The two numbers are different on purpose: equal ones would not distinguish publishing one from
        // publishing the other.
        let with_gesture = header_of_a_run(
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39)), (1, grid(49, 39))])],
            0,
            Some(Drags {
                injected: 1720,
                effective: 1719,
                marked: 3,
                reported: 7,
                gesture: "moving consoles",
            }),
        );
        assert!(
            with_gesture.contains(
                "drags:        1720 injected · 1719 effective · 3 frames with marked text · \
                 7 reports to the program (moving consoles)"
            ),
            "the header did not declare the batch's drags:\n{with_gesture}"
        );

        let without_gesture = header_of_a_run(
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39)), (1, grid(49, 39))])],
            0,
            None,
        );
        assert!(
            !without_gesture.contains("drags"),
            "the header talks about drags in a batch that did not have them, so the batches of the earlier \
             annexes stop having the same shape and cannot be compared:\n{without_gesture}"
        );
    }

    fn grid(rows: u16, cols: u16) -> term_core::Size {
        term_core::Size { rows, cols }
    }

    #[test]
    fn the_window_size_comes_out_in_a_whole_line_and_unrounded() {
        // **This line is a contract with `scripts/measure-frame.sh`**, which reads it with an expression
        // anchored at both ends —`^window: *\(.*\) points$`— to check that the window opened at the size
        // it was asked for and to decide whether two runs can be averaged. Anything added **after**
        // "points" breaks the end anchor without breaking the `contains` of the `lib.rs` test, and what
        // the script then finds is not an error: it is an **empty** size, that is, the kind of plausible
        // number its guards exist for. The whole line is asserted for that reason.
        //
        // **And with a fraction, which is what a compositor really delivers** (1279.8125 points,
        // measured): publishing it rounded would give a number that is not the one there was, and the
        // script compares with the same margin as the application precisely because the fraction arrives.
        // The two axes are different so that swapping them does not publish the same.
        let texto = header_with(
            SizeInPoints {
                width: 1279.8125,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39))])],
            0,
        );

        assert!(
            texto
                .lines()
                .any(|line| line == "window:       1279.8125×800 points"),
            "the size line is not the one the measurement script knows how to read:\n{texto}"
        );
    }

    #[test]
    fn with_two_panes_the_header_says_which_side_each_falls_on() {
        let texto = header_of(&[(0, vec![(0, grid(59, 57)), (1, grid(59, 21))])], 0);

        assert!(
            texto.contains("  pane 0:       59×57  (left)"),
            "it does not publish the first with its side:\n{texto}"
        );
        assert!(
            texto.contains("  pane 1:       59×21  (right)"),
            "it does not publish the second with its side:\n{texto}"
        );
    }

    #[test]
    fn with_three_panes_the_header_neither_invents_a_side_nor_falls_over() {
        // **The type no longer prevents three from arriving.** Until this change the header received an
        // array of two and the number was fixed by it; passing it to a list —what the pane tree needs—
        // indexing the array of sides by position became a possible panic **inside the report that is
        // published on exit**, that is, losing at a stroke everything measured in the session.
        //
        // And it is not enough that it does not blow up: "left" and "right" only describe something when
        // there are two, so with three the hint has to **stay silent**, not be handed to the first two.
        // The assertion looks at both things because a `get` that returns `None` only for the third would
        // pass the first half and go on publishing a false geometry for the other two.
        let texto = header_of(
            &[(
                0,
                vec![(0, grid(59, 30)), (1, grid(59, 30)), (2, grid(59, 30))],
            )],
            0,
        );

        for pane in [0, 1, 2] {
            assert!(
                texto.contains(&format!("  pane {pane}:")),
                "it does not publish pane {pane}:\n{texto}"
            );
        }
        for side in CONSOLE_NAME {
            assert!(
                !texto.contains(side),
                "with three panes it still says '{side}', which no longer describes anything:\n{texto}"
            );
        }
    }

    /// A measurement with one recorded frame, which is the minimum for the report to speak.
    fn with_one_frame() -> Timings {
        let mut timings = Timings::default();
        timings.record_round(Spans::of_a_round(
            Some(Instant::now()),
            Instant::now(),
            ms(2),
            ms(1),
            ms(3),
        ));
        timings.close_frame(ms(7));
        timings
    }

    #[test]
    fn two_passes_of_the_same_frame_sum_their_work_instead_of_clobbering_each_other() {
        // **The bias this fixes was alive and measured:** with `record_round` replacing, the second pass
        // of a frame —egui repeats it when something asks to recompose— clobbered the first's good round
        // trip. And the second arrives with the interpreted at almost zero, because the queue was already
        // emptied in the first, so what the console really interpreted disappeared from the declared and
        // was published as the interface framework's cost.
        let mut timings = Timings::default();
        let recogido = Instant::now();

        // The first pass: the one that brings the wait and the real work.
        timings.record_round(Spans::of_a_round(
            Some(recogido - ms(5)),
            recogido,
            ms(20),
            ms(2),
            ms(1),
        ));
        // The second: with no wait, because it collected nothing, and with what it cost to repeat the
        // work.
        //
        // **The second's three spans are made different from zero on purpose.** In production it barely
        // interprets —the queue was emptied in the first—, but a zero here stops distinguishing summing
        // from not summing: checked by mutating it, with a zero in the interpreted this test passed **with
        // the sum of that span removed**. The three have to be able to fail separately.
        timings.record_round(Spans::of_a_round(None, recogido, ms(3), ms(2), ms(1)));

        assert_eq!(
            timings.close_frame(ms(60)),
            Some(ms(29)),
            "the declared is not the sum of what the two passes did: 20+2+1 from the first and 3+2+1 from \
             the second"
        );
    }

    #[test]
    fn of_two_passes_the_wait_is_the_first_ones_and_not_the_sum() {
        // The wait **is not the application's work**: it is how long the oldest byte had been in the
        // queue. Summing the two would give a number nobody waited, and that number goes into the "key →
        // screen" mean the report publishes. It keeps the one from the pass that really collected.
        let mut timings = Timings::default();
        let recogido = Instant::now();

        timings.record_round(Spans::of_a_round(
            Some(recogido - ms(5)),
            recogido,
            ms(1),
            ms(0),
            ms(0),
        ));
        timings.record_round(Spans::of_a_round(
            Some(recogido - ms(9)),
            recogido,
            ms(1),
            ms(0),
            ms(0),
        ));
        timings.close_frame(ms(10));

        // It is read from the report, which is what a human ends up seeing: the published wait has to be
        // the first pass's —5 ms— and not the 14 from summing them.
        let informe = report_of(&timings);
        assert!(
            informe.contains("waiting in the queue:   5.000ms mean"),
            "the declared wait is not the first pass's; the report says: {informe}"
        );
        assert!(
            !informe.contains("14.000ms"),
            "the wait of the two passes was summed, and that is a number nobody waited: {informe}"
        );
    }

    #[test]
    fn the_report_says_the_panel_span_includes_painting_it() {
        // **Throughout the whole F1.6 milestone this span was described as "reading the panel"** —in its
        // two fields and in the text the report prints— when the clock that produces it spans from before
        // checking the timestamp to after painting the last row. The "Measured" annex of D3 leans on this
        // number, so a reader who copies it believing it only measures reading would attribute the
        // painting to the one computed by subtraction: the interface framework, that is, the figure D3
        // decides about.
        //
        // **It is asserted over what the report writes, not over the constant.** Checking the constant
        // would prove that the constant is well worded, not that anyone prints it — which is the failure
        // that in this project has let eight things through with the suite green.
        let informe = report_of(&with_one_frame());

        assert!(
            informe.contains("the status panel:"),
            "the report does not publish the panel span:\n{informe}"
        );
        assert!(
            informe.contains("serving the drag of its border"),
            "the report publishes the panel span without saying that the gesture of narrowing it is paid \
             inside it: whoever reads the batch will believe that cost is in the rest of the frame, that \
             is, attributed to the interface framework:\n{informe}"
        );
        assert!(
            informe.contains("INCLUDES painting the panel"),
            "the report publishes the panel span without saying it includes painting it, which is what \
             makes the number copyable wrong:\n{informe}"
        );
    }

    #[test]
    fn the_five_spans_add_up_to_the_frame_total() {
        // It is the property that makes the breakdown useful for attribution: if the spans do not add
        // up, the verdict would have to explain a gap, and whoever reads it would not know what to blame.
        //
        // **The fifth span is the panel**, and it entered here as soon as it started being painted: work
        // that happens inside the frame and is not declared is taken by the rest, which is precisely the
        // number that decides whether the interface framework holds up.
        let mut timings = Timings::default();
        let mut resto = RestOfFrame::default();
        let ahora = Instant::now();

        timings.record_round(Spans::of_a_round(
            Some(ahora - ms(100)),
            ahora,
            ms(2),
            ms(3),
            ms(1),
        ));
        let timed = timings
            .close_frame(ms(10))
            .expect("there were pending spans to close");
        resto.record(ms(10), timed);

        assert_eq!(timings.wait.mean(), ms(100));
        assert_eq!(timings.interpret.mean(), ms(2));
        assert_eq!(timings.enqueue.mean(), ms(3));
        assert_eq!(timings.panel.mean(), ms(1));
        assert_eq!(resto.series.mean(), ms(4), "10 minus 2, 3 and 1");
        assert_eq!(timings.round.mean(), ms(110), "the wait plus the frame");
    }

    /// **What the panel takes from the rest, and why this test exists apart.** The one above would pass
    /// just the same summing the panel span into `enqueue`: both are subtracted and the rest comes out
    /// the same. What would break is the *label* —"enqueuing the paint" including reading a file from
    /// disk—, and a swapped label is the error that already invalidated a whole measurement in this
    /// project.
    #[test]
    fn the_panel_span_is_not_summed_into_the_painting_one() {
        let mut timings = Timings::default();
        let ahora = Instant::now();

        timings.record_round(Spans::of_a_round(Some(ahora), ahora, ms(2), ms(3), ms(4)));
        timings.close_frame(ms(10));

        assert_eq!(
            timings.enqueue.mean(),
            ms(3),
            "the panel cost slipped into the painting span"
        );
        assert_eq!(timings.panel.mean(), ms(4));
    }

    #[test]
    fn the_rest_subtracts_what_all_the_panes_timed() {
        // The reason the rest lives in the window: the CPU consumption arrives only once per pass.
        // Subtracting only one pane's spans, the other would be counted as rest —and the rest is exactly
        // the number that says whether the painting holds up.
        let mut izquierda = Timings::default();
        let mut derecha = Timings::default();
        let mut resto = RestOfFrame::default();
        let ahora = Instant::now();

        izquierda.record_round(Spans::of_a_round(
            Some(ahora),
            ahora,
            ms(2),
            ms(1),
            Duration::ZERO,
        ));
        derecha.record_round(Spans::of_a_round(
            Some(ahora),
            ahora,
            ms(3),
            ms(1),
            Duration::ZERO,
        ));

        let timed: Duration = [izquierda.close_frame(ms(10)), derecha.close_frame(ms(10))]
            .into_iter()
            .flatten()
            .sum();

        resto.record(ms(10), timed);

        assert_eq!(timed, ms(7), "2+1 from one and 3+1 from the other");
        assert_eq!(resto.series.mean(), ms(3), "10 minus the two panes");
    }

    #[test]
    fn a_frame_with_no_output_does_not_count_as_a_lightning_fast_round_trip() {
        // Without this discard, every frame at rest would put a zero into the series and the mean would
        // say the application responds instantly precisely for having done nothing.
        let mut timings = Timings::default();

        timings.record_round(Spans::of_a_round(None, Instant::now(), ms(2), ms(3), ms(1)));
        timings.close_frame(ms(10));

        assert!(
            !timings.round.has_samples(),
            "a round trip that did not happen was recorded"
        );
        assert!(
            !timings.wait.has_samples(),
            "a wait that did not happen was recorded"
        );
        assert!(!timings.panel.has_samples(), "nor the panel span");
    }

    #[test]
    fn a_console_with_no_output_declares_all_the_same_what_it_cost_to_paint_it() {
        // **The failure this test exists to catch is one of attribution, and it was invisible.** With two
        // consoles, the normal case is one receiving output and the other still at its prompt. The still
        // one paints all the same every frame, so its enqueuing is real work; if it did not declare it,
        // the window would not subtract it from the total and that time would come out published as "the
        // rest of the frame" — the number that decides whether the interface framework holds up several
        // consoles.
        let mut timings = Timings::default();
        let mut resto = RestOfFrame::default();

        // And its panel is also checked every frame, even if its producer is silent: the `ms(1)` is what
        // it costs to ask the file timestamp, which is paid just like the painting.
        timings.record_round(Spans::of_a_round(None, Instant::now(), ms(0), ms(4), ms(1)));
        let timed = timings
            .close_frame(ms(10))
            .expect("a painted console has to declare what it cost to paint it");
        resto.record(ms(10), timed);

        assert_eq!(
            timed,
            ms(5),
            "the still console's enqueuing plus the glance at its panel"
        );
        assert_eq!(
            resto.series.mean(),
            ms(5),
            "the rest kept the painting of the still console"
        );
    }

    #[test]
    fn a_console_that_interprets_and_is_not_painted_declares_the_interpreted_without_touching_the_means()
     {
        // **The new case, and the most expensive of the three**: a console that interprets and is not
        // painted. Here what makes no series necessary for it is tied down — **the four means do not
        // move**, because putting a zero enqueuing into its mean would say that painting comes out cheaper
        // precisely in the frames in which nothing was painted— and that the interpreted **is** returned
        // to be subtracted.
        let mut timings = Timings::default();
        let mut resto = RestOfFrame::default();

        timings.record_round(Spans::of_a_round(
            None,
            Instant::now(),
            ms(2),
            Duration::ZERO,
            Duration::ZERO,
        ));
        let timed = timings
            .close_frame(ms(10))
            .expect("the interpreted has to be declared even if it was not painted");
        resto.record(ms(10), timed);

        assert_eq!(timed, ms(2), "what it interpreted, and nothing more");
        assert_eq!(
            resto.series.mean(),
            ms(8),
            "the rest kept what a console that was not painted interpreted"
        );
        assert!(
            !timings.enqueue.has_samples(),
            "the enqueuing mean was dirtied"
        );
        assert!(
            !timings.interpret.has_samples(),
            "the interpreting mean was dirtied"
        );
        assert!(
            !timings.round.has_samples(),
            "a round trip that did not happen was recorded"
        );
        assert!(!timings.panel.has_samples(), "the panel span was dirtied");
    }

    #[test]
    fn a_total_smaller_than_its_spans_does_not_produce_a_negative() {
        // The spans and the total come from contiguous frames and from different clocks, so the total may
        // fall short. Before subtracting without more, a negative rest would come out published as a huge
        // number with every look of being real.
        let mut resto = RestOfFrame::default();

        resto.record(ms(1), ms(16));

        assert_eq!(resto.series.mean(), Duration::ZERO);
    }

    #[test]
    fn the_95th_percentile_is_not_set_by_the_worst_of_all() {
        // A single pause of the desktop cannot decide the number that is published.
        let mut series = Series::default();
        for _ in 0..99 {
            series.record(ms(1));
        }
        series.record(ms(500));

        assert_eq!(series.p95(), ms(1), "a single value took the percentile");
    }

    #[test]
    fn the_total_count_survives_the_sample_cap() {
        // The report says how many measurements there were, not how many were kept: with the cap, the two
        // figures stop coinciding and publishing the second would make a long session look short.
        let mut series = Series::default();
        for _ in 0..(SAMPLES + 10) {
            series.record(ms(1));
        }

        assert_eq!(series.samples.len(), SAMPLES);
        assert_eq!(series.total, SAMPLES + 10);
    }

    /// **The line that says what the bar costs, and that only comes out when there was one.** Without it,
    /// what it costs to paint the tabs —and since this milestone, to accumulate what their panes claim—
    /// could only be figured out by subtracting two different batches, which is how the numbers that
    /// cannot be defended are published. And the script invalidates the run by reading precisely this
    /// line.
    #[test]
    fn the_header_says_what_the_bar_cost_only_when_there_was_a_bar() {
        let window = SizeInPoints {
            width: 1280.0,
            height: 800.0,
        };
        let grids = &[
            (0, vec![(0, grid(49, 39)), (1, grid(49, 39))]),
            (1, vec![(2, grid(49, 39)), (3, grid(49, 39))]),
        ];

        let con = with_tab_bar(
            window,
            grids,
            0,
            None,
            None,
            TabBar {
                frames: 100,
                total: Duration::from_micros(3_600),
                marked_tabs: 2,
            },
        );
        assert!(
            con.contains("tab bar:      100 frames") && con.contains("36µs per frame"),
            "the header did not declare what the bar cost nor its mean:\n{con}"
        );

        let sin = with_tab_bar(
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            grids,
            0,
            None,
            None,
            TabBar::default(),
        );
        assert!(
            !sin.contains("tab bar:"),
            "with no bar to paint, the header published a span nobody measured:\n{sin}"
        );
    }

    fn with_reflow(reflow: Reflow) -> String {
        let mut salida = Vec::new();
        header(
            &mut salida,
            SizeInPoints {
                width: 1280.0,
                height: 800.0,
            },
            &[(0, vec![(0, grid(49, 39)), (1, grid(49, 39))])],
            0,
            SessionHeader {
                reflow,
                ..Default::default()
            },
        )
        .expect("writing to memory does not fail");
        String::from_utf8(salida).expect("the header is text")
    }

    /// **The line that says what reflowing the text costs, and that only comes out when there was a
    /// reflow.** Reflowing runs in the logical half of the frame, where there is no painting to wrap it:
    /// what is not named here is published as the framework's cost, which is the figure **D3** judges
    /// egui with. With the history on it is almost a thousand microseconds **per console** and one border
    /// moves two.
    ///
    /// **And the shape of the line is a contract with `scripts/measure-frame.sh`**, which reads it to
    /// invalidate a run of batch M. Its guard pulls the number with
    /// `s/^reflow: *\([0-9]*\) times .*$/\1/p` and compares it against the injected gestures: if the line
    /// changes shape, the script sees **no** reflow and invalidates good runs — and if the line
    /// disappears entirely, the guard that claims to catch "the border moved without crossing cell
    /// boundaries" stops catching anything.
    #[test]
    fn the_header_says_what_reflowing_cost_only_when_there_was_a_reflow() {
        // The numbers of a published run: 320 reflows at the ~984 µs it costs to walk one console's
        // history.
        let con = with_reflow(Reflow {
            frames: 320,
            total: Duration::from_micros(320 * 984),
        });

        assert!(
            con.contains("314.88ms in total") && con.contains("984µs each"),
            "the header did not declare what reflowing cost nor what it costs each time:\n{con}"
        );

        // **The exact shape the script knows how to read.** The `.*` that follows "times" requires there
        // to be something after it, just like in the restored line: a trimmed line would not match and the
        // script would invalidate a good run.
        let line = con
            .lines()
            .find(|line| line.starts_with("reflow:"))
            .expect("there is no reflow line the script can read");
        let (etiqueta, resto) = line
            .split_once(" times ")
            .expect("the reflow line lost the words the script looks for");
        assert!(
            etiqueta
                .strip_prefix("reflow:")
                .expect("the line starts with its label")
                .trim_start()
                .parse::<usize>()
                == Ok(320),
            "the script would not pull 320 reflows out of this line: '{line}'"
        );
        assert!(
            !resto.is_empty(),
            "after 'times' there is nothing left and the script requires something to be left: '{line}'"
        );

        // **And in a batch that resizes nothing, the line does not come out.** Publishing "0 times" reads
        // as "reflowing costs nothing" instead of as "there was no reflow", and it would also give the
        // script's guard a number that would pass both its checks without anything having been reflowed.
        let sin = with_reflow(Reflow::default());
        assert!(
            !sin.contains("reflow:"),
            "with nothing reflowed, the header published a span nobody measured:\n{sin}"
        );
    }

    // ── The coupling with the measurement script, tied down ──────────────────────────────────────
    //
    // **The script invalidates runs by what this report SAYS**, and until here that coupling lived
    // in a comment. Four of its patterns were covered by no test and one was tied with a negation,
    // which is satisfied vacuously as soon as the literal is renamed.
    //
    // **What makes what follows strong is that it does not copy the patterns: it pulls them out of
    // the script and runs them.** A test that repeated the literal by hand fixes itself when someone
    // translates the script —the `sed` changes, the test changes, green— and would have tied nothing
    // down.

    /// The measurement script, read at compile time so it cannot desynchronize.
    const SCRIPT: &str = include_str!("../../../scripts/measure-frame.sh");

    /// A pattern the script applies to the report, with the line where it is so it can be pointed at.
    struct ScriptPattern {
        line: usize,
        order: &'static str,
        patron: String,
    }

    /// The patterns the script looks for **in the report**, pulled from its `sed` and `grep`.
    ///
    /// It keeps only the ones that carry a report literal: the anchored ones (`^`…`$`) and the two
    /// counts the script looks for unanchored. A `grep` over something else —the exit status, a
    /// witness file— does not describe the report and has nothing to tie down here.
    ///
    /// **And one is left out that does describe the report**, said so as not to misdescribe what is
    /// excluded: the `grep -m1 '  pane 0:'` with which the script composes its grid line on finishing.
    /// It carries neither anchor nor keyword, so this criterion does not see it. **It invalidates no
    /// run** —it only feeds a message— and that is why it is left out instead of widening the criterion
    /// until it starts picking up noise; if some day it comes to decide something, it goes in.
    fn script_patterns() -> Vec<ScriptPattern> {
        let mut encontrados = Vec::new();
        // **The command is carried across lines on purpose.** The script splits its long `sed`s with a
        // trailing backslash and puts each `-e` on its own line, so the line that carries the pattern
        // many times does **not** contain the word `sed`. The first version of this extractor did not
        // take that into account and found 2 of 20 — the count guard caught it, which is right below for
        // this.
        let mut order_in_progress = "";
        let mut continuacion = false;
        for (numero, line) in SCRIPT.lines().enumerate() {
            if !continuacion {
                order_in_progress = "";
            }
            // **A comment is not a command, and counting it inflated the count from 21 to 24.** The
            // script explains its own guards by citing the patterns it applies, so its prose is full of
            // `grep` and quoted literals. With the comments inside, the guard that exists to keep this
            // test from passing vacuously **was satisfied with prose**.
            if line.trim_start().starts_with('#') {
                continue;
            }
            if line.contains("sed ") {
                order_in_progress = "sed";
            } else if line.contains("grep ") {
                order_in_progress = "grep";
            }
            continuacion = line.trim_end().ends_with('\\');
            if order_in_progress.is_empty() {
                continue;
            }
            // The script's patterns go inside quotes, and it uses **both**: single ones for the `sed`s
            // and double ones for the two counts it looks for unanchored.
            for comilla in ['\'', '"'] {
                let mut resto = line;
                while let Some(abre) = resto.find(comilla) {
                    let tras = &resto[abre + 1..];
                    let Some(cierra) = tras.find(comilla) else {
                        break;
                    };
                    let dentro = &tras[..cierra];
                    resto = &tras[cierra + 1..];
                    // **The anchor is looked for anywhere, not at the start.** The `sed` patterns come
                    // wrapped in `s/…/…/p`, so the `^` goes inside and not in front: asking for it at
                    // the start found 8 of 20.
                    //
                    // **And it has to carry some letter.** A pattern made only of anchors and character
                    // classes —`^[0-9]+`, the second `grep` of a pipe— does not read the report but the
                    // previous one's output, and above all **it has no text that could change
                    // language**: there is no coupling to tie down there. It is a criterion and not a
                    // list of exceptions, so it keeps holding when the script grows.
                    let describes_the_report = (dentro.contains('^')
                        || dentro.contains("characters searched")
                        || dentro.contains("glyphs rasterized"))
                        && dentro.chars().any(|c| c.is_alphabetic());
                    if describes_the_report {
                        encontrados.push(ScriptPattern {
                            line: numero + 1,
                            order: order_in_progress,
                            patron: dentro.to_string(),
                        });
                    }
                }
            }
        }
        encontrados
    }

    /// What comes out of applying a script pattern to a text.
    ///
    /// **Empty and broken are not the same, and that is why they are two variants.** Both give empty
    /// output, and confusing them makes the test blame the wrong place: "the report changed text" when
    /// what is happening is that the pattern extractor cut wrong the one it was going to apply. One
    /// points to a defect in the project, the other to a defect in this test.
    ///
    /// **And the blind spot this does NOT cover, said instead of hidden.** The distinction is made with
    /// the exit status, so it only separates the patterns the command **rejects**. A cut that leaves a
    /// pattern still valid but that does not print —removing the trailing `p` from a `sed`, for example—
    /// exits with 0 and is **indistinguishable** from "the report changed". Against that the only thing
    /// there is is the minimum-count guard down below: if the extractor really breaks, it finds fewer
    /// patterns and the test says so. Checked with the two mutations.
    enum Salida {
        /// The pattern was applied and extracted something.
        Extrajo,
        /// The pattern was applied well and matched nothing.
        NoCaso,
        /// The command could not handle the pattern: it is not a failure of the report, it is of this
        /// test.
        PatronInvalido(i32),
    }

    /// Applies a script pattern to a text **with the real command**, not with an imitation.
    ///
    /// Reimplementing `sed` in Rust would be introducing a second source of divergence right where the
    /// first is meant to be removed.
    ///
    /// **The price, declared: this makes `cargo test` depend on `sed` and `grep`.** It is accepted here
    /// because what this test protects is a shell script, so where there is no `sed` there is nothing to
    /// protect either. If they are not there, it blows up with its message instead of skipping itself — a
    /// `#[cfg(unix)]` would make it disappear silently, which is worse. **F2.7 brings Windows**, and
    /// there a decision will have to be made: gate it to the system or accept that the bench does not run
    /// on that platform.
    fn extract(p: &ScriptPattern, informe: &str) -> Salida {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let mut order = Command::new(p.order);
        match p.order {
            "sed" => {
                order.arg("-n").arg(&p.patron);
            }
            _ => {
                order.arg("-E").arg(&p.patron);
            }
        }
        let mut child = order
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| {
                panic!(
                    "could not launch '{}': {e}. The bench would not work without it either",
                    p.order
                )
            });
        child
            .stdin
            .as_mut()
            .expect("the input is open")
            .write_all(informe.as_bytes())
            .expect("writing to a memory pipe does not fail");
        let salida = child.wait_with_output().expect("collect the output");
        let codigo = salida.status.code().unwrap_or(-1);
        // **Each command says "I found nothing" in a different way.** `grep` exits with 1 when it does
        // not match and with 2 or more when the pattern does not work for it; `sed` exits with 0 even if
        // it substitutes nothing, and only complains if the pattern is badly written. Without this
        // distinction, a pattern badly extracted by this test would be published as a broken coupling of
        // the project.
        let patron_invalido = match p.order {
            "grep" => codigo >= 2,
            _ => codigo != 0,
        };
        if patron_invalido {
            return Salida::PatronInvalido(codigo);
        }
        if String::from_utf8_lossy(&salida.stdout).trim().is_empty() {
            Salida::NoCaso
        } else {
            Salida::Extrajo
        }
    }

    /// The reports the producer knows how to write, one for each variant the script distinguishes.
    ///
    /// **They are several and not one because the three configuration states are mutually exclusive**: a
    /// report cannot say at the same time that the configuration is vetoed and that the user's was read.
    /// Each pattern has to extract from **some**; none has to extract from all.
    fn reference_reports() -> Vec<(&'static str, String)> {
        let grids = vec![
            (0, vec![(0, grid(49, 39)), (1, grid(49, 21))]),
            (1, vec![(0, grid(49, 39))]),
        ];
        let window = SizeInPoints {
            width: 1280.0,
            height: 800.0,
        };

        let mut tab_bar = TabBar::default();
        tab_bar.record(ms(7));
        tab_bar.saw_marked(3);

        let mut reflow = Reflow::default();
        reflow.record(ms(2));

        let drags = Drags {
            injected: 12,
            effective: 6,
            marked: 4,
            reported: 2,
            gesture: "moving consoles",
        };

        let restoring = Restoring {
            consoles: 4,
            read: Some(ms(1)),
            opening: ms(5),
        };

        let header_with_everything = |settings| {
            let mut salida = Vec::new();
            header(
                &mut salida,
                SizeInPoints {
                    width: window.width,
                    height: window.height,
                },
                &grids,
                0,
                SessionHeader {
                    drags: Some(drags),
                    restoring: Some(restoring),
                    settings,
                    // **Always `None` here, and it is what the bench really publishes**: every batch
                    // has the file vetoed, so no batch can ask for a family. A reference report with
                    // a font would be teaching the script to accept a run that broke the veto.
                    chosen_font: ChoosingTheFont::NotAsked,
                    tab_bar,
                    reflow,
                },
            )
            .expect("writing to memory does not fail");
            String::from_utf8(salida).expect("the report is text")
        };

        // The window's spans the script also reads, each one populated.
        let mut saving = Saving::default();
        saving.record_mirror(ms(1));
        saving.record_write(ms(3), true);
        saving.record_write(ms(3), false);

        let mut discovery = Discovery::default();
        discovery.record(ms(4), 9, 2);

        let mut rasterizing = Rasterizing::default();
        rasterizing.record(ms(1), 15, 1);

        let mut window_spans = Vec::new();
        saving
            .publish(&mut window_spans)
            .expect("writing to memory does not fail");
        discovery
            .publish(&mut window_spans)
            .expect("writing to memory does not fail");
        rasterizing
            .publish(&mut window_spans)
            .expect("writing to memory does not fail");

        vec![
            (
                "header with the configuration vetoed",
                header_with_everything(ReadingSettings::Vetoed),
            ),
            (
                "header with no configuration to apply",
                header_with_everything(ReadingSettings::Missing),
            ),
            (
                "header with the user's configuration",
                header_with_everything(ReadingSettings::Applied {
                    how_long: ms(1),
                    warnings: 2,
                }),
            ),
            (
                "the window's spans",
                String::from_utf8(window_spans).expect("the report is text"),
            ),
        ]
    }

    /// **The script runs, and until this test nothing checked it.**
    ///
    /// Everything the script ties down —the derived patterns, the literals, the seeding— it reads **as
    /// text**. That catches an expression ceasing to extract, and catches nothing of what makes an `sh`
    /// program work: a variable that kept the old name inside a `$(( ))`, a function called by a name
    /// that no longer exists. `sh -n` does not either: both things are valid syntax.
    ///
    /// **What is exercised is the turn allocation**, which is the script's only concurrency mechanism and
    /// the one that decides which console dumps and which stays still — the whole F batch hangs off it.
    /// Two consoles against the same directory: the first keeps turn 1, the second turn 2.
    ///
    /// **The failure this catches is not hypothetical**: the delivery that translated the script's
    /// identifiers left a `$((turno + 1))` with the old name. `sh` treats the undefined variable as zero,
    /// so the counter stayed stuck at 1, the second console never got its turn and **the loop did not
    /// end**. The whole suite stayed green.
    // **Only on Linux, like the bench it exercises.** The script asks for `/run/user/<uid>`, `pgrep -x`
    // and `timeout`, which is coreutils' and does not come on macOS or the BSDs. Without this condition,
    // the crate F2.7 is going to compile there would fail on a test that cannot pass, instead of skipping
    // it.
    #[cfg(target_os = "linux")]
    #[test]
    fn two_consoles_of_the_script_take_different_turns_without_clobbering_each_other() {
        let root =
            std::env::temp_dir().join(format!("agent-terminal-turns-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the turns directory");

        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/measure-frame.sh")
            .canonicalize()
            .expect("the script lives two levels above this crate");

        // `MEASURE_TRIES=0` leaves the console with no load to dump: it takes its turn and finishes,
        // which is exactly the path that has to be exercised and nothing more.
        // **The call is time-bounded on purpose.** The failure this guard catches —the turn counter that
        // does not advance— does not make the script fail: it makes it **not finish**, and a test that
        // hangs is worse than one that does not exist. With the clock set, the same failure comes out as
        // code 124 and with its message.
        let console = || {
            std::process::Command::new("timeout")
                .arg("20")
                .arg("sh")
                .arg(&script)
                .env("MEASURE_AS_CONSOLE", "1")
                .env("MEASURE_RUNS", &root)
                .env("MEASURE_TRIES", "0")
                .output()
                .expect("launch the script in console mode")
        };

        assert!(
            console().status.success(),
            "the first console did not finish"
        );
        assert!(
            console().status.success(),
            "the second console did not finish"
        );

        // **It is asserted over the turns left on disk, not over the exit code.** A console that did not
        // get a turn and exited fine would leave this directory with a single name, and the exit code
        // would not say so.
        let mut turns: Vec<String> = std::fs::read_dir(&root)
            .expect("read the turns")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        turns.sort();
        assert_eq!(
            turns,
            vec!["1".to_owned(), "2".to_owned()],
            "the two consoles did not take one turn each: {turns:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// **The other half of the script that runs: the mode that decides whether two runs are averaged.**
    ///
    /// It is a third of the file and no test touched it, even though it is the one that **grants
    /// permission** to publish a mean in the decisions record. If it broke, it would not break red: it
    /// would say "they can be averaged" of two runs that cannot, or the other way around, and the annex
    /// would publish a number that describes neither of the two.
    ///
    /// It is given two pairs through the two doors: one that only differs in the **fraction** with which
    /// the compositor delivers the size —which is the one the margin exists to admit— and another with
    /// genuinely different windows.
    // **Only on Linux, like the bench it exercises.** The script asks for `/run/user/<uid>`, `pgrep -x`
    // and `timeout`, which is coreutils' and does not come on macOS or the BSDs. Without this condition,
    // the crate F2.7 is going to compile there would fail on a test that cannot pass, instead of skipping
    // it.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_scripts_compare_mode_admits_the_fraction_and_rejects_another_window() {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/measure-frame.sh")
            .canonicalize()
            .expect("the script lives two levels above this crate");
        let dir =
            std::env::temp_dir().join(format!("agent-terminal-compare-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");

        let write = |name: &str, body: &str| {
            let path = dir.join(name);
            std::fs::write(&path, body).expect("write the test report");
            path
        };
        // The minimum the comparator reads: the window, the tabs and each pane's grid.
        let exact = write(
            "exact.txt",
            "window: 1280x800 points\ntabs: 1\n  pane 0: 49x39\n",
        );
        // **The same window, delivered with a fraction.** It is not an invented case: it is what the
        // compositor returns, and comparing character by character threw out perfectly averageable runs.
        let fraction = write(
            "fraction.txt",
            "window: 1279.8125x799.8125 points\ntabs: 1\n  pane 0: 49x39\n",
        );
        let other = write(
            "other.txt",
            "window: 800x600 points\ntabs: 1\n  pane 0: 30x20\n",
        );
        // **The same window and ANOTHER grid, which is the case that really ties the comparator down.**
        // Without it, the pair that is rejected differs in both things at once and **the size comparison
        // alone sustains the rejection**: measured, taking away from the comparator the line that looks
        // at each pane's grid left this test green. It is the second disguise this project has on file
        // —choosing a case with which the defect and the fix give the same— and a review caught it.
        let same_window_other_grid = write(
            "regrid.txt",
            "window: 1280x800 points\ntabs: 1\n  pane 0: 30x20\n",
        );
        // And the same window and the same grid, with the consoles come from the file instead of asked
        // for: it is the only thing that distinguishes the batch that restores from its control, and by
        // construction it is not seen in the geometry.
        // **And the same grid with ANOTHER window, which is the only case that gets to ask about the
        // size.** The comparator short-circuits —geometry first, and only if it coincides does it look at
        // the size—, so a pair that differs in both things leaves the size comparison unexercised:
        // measured, raising the margin to a hundred thousand left the other assertions green while the
        // comparator treated a 1280×800 window and an 800×600 one as averageable.
        let other_window_same_grid = write(
            "rewindow.txt",
            "window: 800x600 points\ntabs: 1\n  pane 0: 49x39\n",
        );
        let restored = write(
            "restored.txt",
            "window: 1280x800 points\ntabs: 1\n  pane 0: 49x39\nrestored: 1 consoles in 5 ms\n",
        );

        let compare = |a: &std::path::Path, b: &std::path::Path| {
            std::process::Command::new("timeout")
                .arg("20")
                .arg("sh")
                .arg(&script)
                .arg("compare")
                .arg(a)
                .arg(b)
                .output()
                .expect("launch the script in compare mode")
                .status
                .success()
        };

        assert!(
            compare(&exact, &fraction),
            "the comparator rejects two runs that only differ in the fraction of the size, which is \
             exactly what the margin exists to admit"
        );
        assert!(
            !compare(&exact, &other),
            "the comparator treats two runs of different windows as averageable: their mean would \
             describe neither of the two"
        );
        assert!(
            !compare(&exact, &same_window_other_grid),
            "the comparator treats two runs with the SAME window and a different grid as averageable: \
             it is the case the size margin cannot catch, and without it the half that looks at the \
             grid is tied by nothing"
        );
        assert!(
            !compare(&exact, &other_window_same_grid),
            "the comparator treats two runs with the same grid and a different window as averageable: \
             it is the only case that gets to ask about the size, so without it the margin is tied by \
             nothing"
        );
        assert!(
            !compare(&exact, &restored),
            "the comparator treats a requested run and a restored one as averageable: they are identical \
             in window, tabs and grid by construction, so the line that declares where the consoles came \
             from is the only thing that distinguishes them"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **The third of the script that decides which batch each letter is, executed.**
    ///
    /// The two guards above run console mode and `compare` mode. The **batch mode** —the allocation that
    /// translates a letter into tabs, panes, how many dump, how many drags and which gesture— was walked
    /// by no one, and it is the one that carries the arithmetic that already bit once: an old name inside
    /// a `$(( ))` or a `${VAR%x*}`.
    ///
    /// **The part that does not touch the machine of whoever measures is exercised**, which reaches up to
    /// the binaries check: from there on the script deletes the panel directories in `/run/user/<uid>`
    /// and asks `pgrep` whether there is a window open, and a test can do neither —it would wreck the
    /// session of whoever is working— nor the other without becoming dependent on what is running. Before
    /// that line the header, the two mode doors, the whole `case` and the two multiplications that work
    /// out how many consoles there are have all already passed.
    ///
    /// **The script is copied to a tree of its own**, and it is not a convenience: `ROOT` comes from
    /// `dirname $0`, so running the repository's the script would write into `target/measurements` and
    /// **delete the turns directory of a batch in progress**.
    ///
    /// **And the letters are not written here: they are asked of the script.** A copied list would be a
    /// second place to maintain, and the day it disagreed the wrong one would be this. It is given a
    /// letter that does not exist, the ones its own message offers are read and it is checked that **each
    /// one of them** reaches the end of the allocation — which is what a letter dropped from the `case`
    /// stops doing, publishing "unknown batch" where the annex expects a measurement.
    // **Only on Linux, like the bench it exercises.** The script asks for `/run/user/<uid>`, `pgrep -x`
    // and `timeout`, which is coreutils' and does not come on macOS or the BSDs. Without this condition,
    // the crate F2.7 is going to compile there would fail on a test that cannot pass, instead of skipping
    // it.
    #[cfg(target_os = "linux")]
    #[test]
    fn every_letter_the_script_offers_goes_through_its_batch_allocation() {
        let root =
            std::env::temp_dir().join(format!("agent-terminal-batches-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("scripts")).expect("create the test tree");
        let script = root.join("scripts/measure-frame.sh");
        std::fs::write(&script, SCRIPT).expect("copy the script to the test tree");

        // **With no binaries on purpose.** It is what cuts the run right after the allocation, without
        // getting to open any window or touch anything on the machine.
        let batch = |letter: &str| {
            let out = std::process::Command::new("timeout")
                .arg("20")
                .arg("sh")
                .arg(&script)
                .arg(letter)
                .output()
                .expect("launch the script in batch mode");
            (
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
            )
        };

        // **The letter that does not exist, first**: it is the one that gives the list and the one that
        // proves the `case` still rejects. Without it, an allocation that accepted anything would leave
        // everything below green.
        let (code, offered) = batch("Ñ");
        assert_eq!(
            code,
            Some(2),
            "the script has accepted a letter that is no batch: {offered}"
        );
        assert!(
            offered.contains("unknown batch"),
            "the script does not reject an unknown letter by its name, so this test cannot read it the \
             list: {offered}"
        );

        let letters: Vec<String> = offered
            .split("use ")
            .nth(1)
            .unwrap_or_default()
            .split(|c: char| !c.is_ascii_uppercase())
            .filter(|piece| piece.len() == 1)
            .map(str::to_owned)
            .collect();
        // **Counting is not identifying**, which is the disguise this project has on file: an extractor
        // that returned any sixteen letters would satisfy a bare floor. So three of the ones the annex
        // cites are asked for by name, one of each family: the reference one, the one that restores from
        // the file and the one that rasterizes color.
        assert!(
            letters.len() >= 16,
            "the script offers {} letters and the annex batches are sixteen: either one has dropped out \
             of the allocation, or this test does not know how to read its message — and then it is \
             looking at nothing: {:?}",
            letters.len(),
            letters
        );
        for known in ["A", "L", "Q"] {
            assert!(
                letters.iter().any(|letter| letter == known),
                "the script no longer offers batch {known}, which the annex cites: its earlier runs are \
                 left with no way to be repeated"
            );
        }

        // **And every offered letter has to go through the whole allocation.** That the message names it
        // does not say that the `case` recognizes it: they are two different literals, and the day they
        // disagree the batch is rejected with "unknown" fifteen minutes after launching it.
        for letter in &letters {
            let (code, said) = batch(letter);
            assert!(
                said.contains("is missing — build with"),
                "batch {letter} does not reach the end of its allocation: the script said '{}' instead of \
                 stopping when it did not find the binaries. Either the letter is not in the `case`, or \
                 something in its row —the tabs, the panes, the gesture— was left with a name that no \
                 longer exists",
                said.trim()
            );
            assert_eq!(
                code,
                Some(1),
                "batch {letter} stopped when it did not find the binaries and did not exit with its code"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// **The measurement variables the script sends to itself, and the only one that breaks silently.**
    ///
    /// The guard below checks that the script **mentions** the five the application reads. The other
    /// seven are not read by the application: batch mode sends them to console mode, which is the same
    /// file invoked as if it were the user's command interpreter. Renaming one of those **on one side
    /// only** is caught by neither the compiler, nor `sh -n`, nor `set -u` —the script reads them with
    /// `${… :-}`—, nor any of the twenty guards at the end.
    ///
    /// **And what is lost is not a run: it is its meaning.** `MEASURE_TRIES` is how many consoles dump,
    /// and it is read with two by default. Exported under another name, batches F, G, H, I and L —the
    /// four-console ones— dump **two**, and the report comes out with its four panes, its witness, its
    /// tabs and its grid: identical to the good one. The annex would publish under the letter H the cost
    /// of half the load.
    ///
    /// It is checked in a single direction, which is the one there is: **everything the script reads it
    /// has to export**. Not the other way around, and on purpose — `MEASURE_SIZE`, `MEASURE_TABS`,
    /// `MEASURE_PANES` and `MEASURE_DRAGS` it exports for the application and never reads.
    #[test]
    fn the_script_exports_every_measurement_variable_it_reads_itself() {
        let mut read = std::collections::BTreeSet::new();
        let mut exported = std::collections::BTreeSet::new();
        for line in SCRIPT.lines() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            for (at, _) in line.match_indices("MEASURE_") {
                let name: String = line[at..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                let before = line[..at].chars().next_back();
                let after = line[at + name.len()..].chars().next();
                // `$MEASURE_X` and `${MEASURE_X:-…}` are reads; `MEASURE_X=` is what the script puts into
                // the child's environment. An `unset` is neither one nor the other.
                if before == Some('$') || before == Some('{') {
                    read.insert(name);
                } else if after == Some('=') {
                    exported.insert(name);
                }
            }
        }

        // **The two floors, because an extractor that recognized nothing would pass vacuously.** And by
        // name, not only by number: counting is not identifying.
        assert!(
            read.len() >= 6 && exported.len() >= 10,
            "the extractor has seen {} variables read and {} exported, and there are at least 6 and 10: \
             it has stopped recognizing their shape, so this test would be passing without looking at \
             anything",
            read.len(),
            exported.len()
        );
        for named in ["MEASURE_TRIES", "MEASURE_RUNS"] {
            assert!(
                read.contains(named),
                "the extractor does not see the script read {named}, which is one of the ones it sends to \
                 itself: it is finding the number of variables and not the variables"
            );
        }

        let orphans: Vec<&String> = read.difference(&exported).collect();
        assert!(
            orphans.is_empty(),
            "the script reads {:?} and does not export them, so console mode receives them empty: the \
             ones that carry a default value —`MEASURE_TRIES` is how many consoles dump— keep measuring, \
             but something else, and the report comes out indistinguishable from the good one",
            orphans
        );
    }

    #[test]
    fn every_pattern_the_script_looks_for_in_the_report_still_extracts_something() {
        let patrones = script_patterns();

        // **The guard counts BY SHAPE and not a total, and a review found the difference by mutating
        // it.** With a total —"at least 20", there being 21— the guard tolerated losing exactly one, and
        // the two recognized by keyword are precisely the ones that can drop out one at a time:
        // translating **only** the fonts one, the set went down to 20, the guard passed, and its batch
        // was invalidated forever without anything saying so. It was the second disguise this project has
        // on file: a number with which the defect and the fix give the same result.
        //
        // Counting the two families separately, losing **a single one** of the two by keyword already
        // fails. And it is still a criterion and not a list, so it holds up as the script grows.
        let anclados = patrones.iter().filter(|p| p.patron.contains('^')).count();
        let by_keyword = patrones.len() - anclados;
        assert!(
            anclados >= 19 && by_keyword >= 2,
            "the extractor has found {anclados} anchored patterns and {by_keyword} by keyword, and there \
             are at least 19 and 2: it has stopped recognizing the shape of some, so this test would be \
             passing vacuously right where it is least noticed"
        );

        // **And the guard that really survives the script growing: coverage BY USE.** The two above are
        // by shape —how many patterns and of what type— and both can be satisfied while a real guard is
        // lost: it is enough for the script to gain a line that reads the report with a shape this
        // extractor does not recognize. It was measured, and a whole guard was lost with the 23 tests
        // green.
        //
        // So it is counted the other way around: **every line of the script that applies `grep` or `sed`
        // to the report has to have contributed at least one pattern**. Whatever is exempt is named here
        // one by one, with its reason — a short, explicit list ages better than a criterion that widens
        // until it stops failing.
        let mut sin_aportar = Vec::new();
        for (numero, line) in SCRIPT.lines().enumerate() {
            let numero = numero + 1;
            if line.trim_start().starts_with('#') || !line.contains("$REPORT") {
                continue;
            }
            let aplica_un_patron = line.contains("grep") || line.contains("sed");
            if !aplica_un_patron {
                continue;
            }
            // **And its pattern has to carry text, for the same reason as above.** The script has a line
            // that counts the ones in the error file with an empty pattern: it applies a `grep` to the
            // report and **reads no word from it**, so there is nothing that could be left behind on
            // translating. It is the same criterion that decides which pattern is extracted, applied to
            // which line has to contribute one — and by being the same, the two halves cannot
            // desynchronize.
            let some_pattern_with_text =
                line.split(['\'', '"'])
                    .skip(1)
                    .step_by(2)
                    .any(|entre_comillas| {
                        !entre_comillas.starts_with('$')
                            && entre_comillas.chars().any(|c| c.is_alphabetic())
                    });
            if !some_pattern_with_text {
                continue;
            }
            // The only exempt one: it composes the grid message on finishing and **invalidates no run**.
            // Its literal is covered incidentally by the pane count. If some day it decides something, it
            // stops being an `echo` and has to contribute.
            //
            // **It is exempted by what the line IS and not by where it is**, and a review asked for that:
            // it was anchored to a line number, and the delivery that translated the script moved it from
            // 933 to 975 **without anything saying so** — the exemption stopped exempting and went on
            // seeming to protect something. Today there is exactly one line of this shape among the
            // thirty-two that carry a pattern.
            if line.trim_start().starts_with("echo ") {
                continue;
            }
            if !patrones.iter().any(|p| p.line == numero) {
                sin_aportar.push(format!("  measure-frame.sh:{numero}: {}", line.trim()));
            }
        }
        assert!(
            sin_aportar.is_empty(),
            "{} line(s) of the script read the report with `grep` or `sed` and this test has pulled no \
             pattern out of them, so it **is not tying them down**. Either the extractor does not \
             recognize their shape, or they are new guards that have to be covered — and meanwhile they \
             can break silently:\n{}",
            sin_aportar.len(),
            sin_aportar.join("\n")
        );

        let informes = reference_reports();
        let mut huerfanos = Vec::new();
        let mut invalidos = Vec::new();
        for p in &patrones {
            let mut extracted_from_any = false;
            for (_, informe) in &informes {
                match extract(p, informe) {
                    Salida::Extrajo => extracted_from_any = true,
                    Salida::NoCaso => {}
                    // One entry per pattern and not per report: the same badly-made cut was seen four
                    // times, one per reference report.
                    Salida::PatronInvalido(codigo) => {
                        let entrada = format!(
                            "  measure-frame.sh:{} ({}) {:?} → exited with {codigo}",
                            p.line, p.order, p.patron
                        );
                        if !invalidos.contains(&entrada) {
                            invalidos.push(entrada);
                        }
                    }
                }
            }
            if !extracted_from_any {
                huerfanos.push(format!(
                    "  measure-frame.sh:{} ({}) {:?}",
                    p.line, p.order, p.patron
                ));
            }
        }
        // **This is checked first, and blaming this test and not the project.** A pattern the command
        // rejects says nothing about the report: it says the extractor above cut it wrong.
        assert!(
            invalidos.is_empty(),
            "this test's extractor has pulled {} patterns that its own command rejects, so it has cut \
             them wrong. **It is not a failure of the report nor of the script, it is of this test** — \
             fix it before believing what it says about the rest:\n{}",
            invalidos.len(),
            invalidos.join("\n")
        );
        assert!(
            huerfanos.is_empty(),
            "{} of {} script patterns extract nothing from any report the producer knows how to write. \
             Either the report changed text without the script, or the other way around — and as soon as \
             that happens the script stops invalidating bad runs instead of failing:\n{}",
            huerfanos.len(),
            patrones.len(),
            huerfanos.join("\n")
        );

        // **And the five environment variables, which is the half of the contract this test does not tie
        // down.** The window's `assert_eq!`s compare each constant with itself; what is really asked for
        // is that the script **export** that name, because the app reads these variables to know that the
        // session asked for the measurement and to decide its vetoes. Without this pin, renaming on one
        // side only blows up only at measurement time, with the batch already running.
        use crate::{
            DRAGS_VARIABLE, GESTURE_VARIABLE, PANES_VARIABLE, SIZE_VARIABLE, TABS_VARIABLE,
        };
        for variable in [
            TABS_VARIABLE,
            SIZE_VARIABLE,
            PANES_VARIABLE,
            DRAGS_VARIABLE,
            GESTURE_VARIABLE,
        ] {
            assert!(
                SCRIPT.contains(variable),
                "the script does not mention {variable}: the app would stop recognizing the session as a \
                 measurement batch and its startup vetoes would stop applying"
            );
        }
    }

    #[test]
    fn the_script_seeds_the_same_layout_file_the_application_writes() {
        // **The other half of the coupling, and the one that is not seen.** The script writes a layout by
        // hand for the batch that restores, with the path and the keys put in its own text. The test
        // there was asserted that the name constant equalled a literal written **right beside it**: the
        // two halves are renamed at once, it stays green, and the script goes on seeding the old name.
        assert!(
            SCRIPT.contains(crate::session::FILE),
            "the measurement script does not seed '{}', which is the file the application reads: the \
             batch that restores would open from factory and its run would be published as good",
            crate::session::FILE
        );
    }
}
