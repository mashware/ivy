//! What the application says **over** a console — two notices, and the button under the first —
//! and where each of them goes.
//!
//! **They are here and not in the painter because a module holds six hundred production lines**
//! (**D28**) and the painter reached them the moment the second notice arrived. The piece that left
//! is the one this delivery was already editing, which is the way through that decision records —
//! and it is a piece rather than a slice: what a notice says, how much of a console it may take,
//! which of the two gives way, and where a press on one lands, all in one place.
//!
//! **Nothing here is stored.** Every box is worked out from the console and its rectangle every
//! time they are asked for, by the painting and by the mouse alike, so there is no rectangle kept
//! anywhere to go stale — an earlier shape of the held paste's notice kept one and it was wrong in
//! four separate ways.

use super::CellMetrics;
use crate::theme::Theme;
use eframe::egui::{Painter, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

/// How much of a console a notice may take.
///
/// **A half, and the half is a product decision rather than a taste.** What it protects is the
/// bottom of the grid: that is where a console's last output is, it is the first thing somebody
/// wants to mark and copy, and marking begins with a bare press — the same press that answers a
/// notice. Without a ceiling the box runs a console's whole height at the width two panes get, so
/// **the entire console becomes a button that throws the paste away**, which is the defect this
/// closes.
///
/// It is also what leaves room for a second thing to say about the same console, which is why that
/// feature's own delivery is where this ceiling was asked for.
const NOTICE_SHARE: f32 = 0.5;

/// What a notice says about how to answer it, **when the keyboard reaches that console**.
///
/// **A click on this box discards and never sends**, and the asymmetry is deliberate: sending a line
/// the program will cut in half is the irreversible half of this question, so it never shares a
/// target with the dismissal — it costs a deliberate chord, or a press on a box of its own labelled
/// [`SEND_IT_ANYWAY`]. Dismissing a question you did not ask for is the safe half, and it is the one
/// somebody with a hand on the mouse most often wants.
pub(crate) const ANSWERED_BY_HAND: &str =
    "Escape or a click discards it · Ctrl+Enter sends it anyway";

/// What it says **when the keys would go to another console**, which is every pane of the tab but
/// the focused one.
///
/// **It names no key, and that is the defect this replaced.** Every pane paints its own question and
/// the keys are read for one of them, so «Escape discards it» was an instruction that went to a
/// different program. It does not go quiet either: a paste held in silence is the failure this whole
/// feature exists to stop.
pub(crate) const ANSWERED_BY_CLICK: &str =
    "A click discards it · focus this console to send it anyway";

/// What it says on **the console that has the keyboard inside a window that lost the desktop's
/// focus**, which is what the cursor says too: an outlined one there means nobody is typing.
///
/// **Neither of the other two is true there.** The keys it names would not arrive until the window
/// comes back, and «focus this console» asks for what that console already has.
///
/// **It may never lay out taller than the other two**, and that is why it is this short. The box is
/// measured against every wording, so a longer one raised the floor of every console — including the
/// ones in a focused window, where it is never shown — and a console that used to hold the question
/// held its paste in silence instead. The first wording did exactly that, and so did «come back,
/// then Ctrl+Enter sends it» at twenty cells; a test holds the line, with the built-in font. The box
/// is still measured against all three, because a font somebody chose is what that test cannot walk.
pub(crate) const ANSWERED_ONCE_BACK: &str =
    "A click discards it · come back and Ctrl+Enter sends it";

/// Every way a notice can say how it is answered, which is what its box is measured against.
pub(crate) const EVERY_ANSWER: [&str; 3] =
    [ANSWERED_BY_HAND, ANSWERED_BY_CLICK, ANSWERED_ONCE_BACK];

/// What the box under the question says, and a press on it sends the held paste (**D31**).
///
/// **A box of its own, under the question and never inside it**, so «a click discards it» stays
/// literally true of the box it is written in, and the irreversible answer never shares a target
/// with the safe one.
pub(crate) const SEND_IT_ANYWAY: &str = "Send it anyway";

/// Where a held paste's notice goes **when no button is placed under it**, or nothing if the console
/// holds no paste.
///
/// **Only the tests ask for it**, and on purpose: the painter and the mouse pass both ask
/// [`the_boxes`], which decides between this box and a shorter one with the button's slot under it,
/// from the same measurement. What this one keeps checkable is the question's own ceiling and floor,
/// apart from the button's.
#[cfg(test)]
pub(crate) fn the_question_box_with_no_button(
    painter: &Painter,
    console: &crate::Pane,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
) -> Option<Rect> {
    let measured = measure_the_question(painter, console, theme, metrics, area)?;
    question_box_under(area, metrics, &measured, 0.0)
}

/// What the question's text needs, laid out once and asked of by every ceiling.
struct Measured {
    /// The whole question, the tallest of its wordings.
    tallest: Vec2,
    /// The way out on its own, which is what the floor asks for.
    answer: f32,
}

/// Lays the question out, or nothing if the console holds no paste.
///
/// **Once per pass, whatever ceilings are tried after it**: laying it out again for each one is work
/// the frame would not be declaring (**D13**).
///
/// **It is laid out against EVERY answer sentence and takes the tallest box, whichever one will be
/// shown.** Otherwise the box would change size on the very frame the focus moves — the console's or
/// the window's — and the mouse pass would be hit-testing a rectangle different from the one the
/// painter is about to draw.
///
/// **Taking the longest string was the first attempt and a review took it apart**: what sets the
/// height is **wrapping**, not counting. At a narrow console one of two sentences of the same length
/// took two lines and the other three, so measuring by length made an unfocused pane's box one line
/// short — text painted outside its own border, and a hit target smaller than the drawing.
fn measure_the_question(
    painter: &Painter,
    console: &crate::Pane,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
) -> Option<Measured> {
    let held = console.held.as_ref()?;
    let wrap = (area.width() - 4.0 * metrics.width()).max(metrics.width());
    let tallest = EVERY_ANSWER
        .into_iter()
        .map(|how| {
            painter
                .layout(
                    format!("{how}\n\n{}", held.sentence()),
                    metrics.font(),
                    theme.foreground(),
                    wrap,
                )
                .size()
        })
        .reduce(|one, other| one.max(other))?;
    // How much room the way out needs on its own, which is what the floor asks for. Laid out at the
    // same width, and the tallest of them for the same reason the box is: whichever is shown, it has
    // to be whole.
    let answer = EVERY_ANSWER
        .into_iter()
        .map(|how| {
            painter
                .layout(how.to_owned(), metrics.font(), theme.foreground(), wrap)
                .size()
                .y
        })
        .reduce(f32::max)?;
    Some(Measured { tallest, answer })
}

/// The question's box with `reserved` points of the top half left free under it.
///
/// **Only the ceiling moves.** A question that does not reach it comes out exactly as it would have
/// anyway, and one that does is shortened — which clips the tail of the explanation, never the way
/// out, because the keys are first in the galley — and still has to pass the same floor.
///
/// **It takes at most half the console's height**, and the bottom half is what that buys: a dead
/// console's last output is at the bottom of its grid, it is the first thing somebody wants to mark
/// and copy, and marking starts with a bare press. Without the ceiling the box runs the console's
/// whole height at the width two panes get — measured at 168 pt — so the entire console becomes a
/// button that throws the paste away.
///
/// `None` unless **the whole answer fits inside the capped box**, and a few cells wide. Two things
/// about that floor, and both were got wrong first:
///
/// - **It is measured against what the ceiling leaves**, not against the box the text asked for.
///   The other way round, a three-row console gets a box of about 26 px over a text area of about
///   10 — under one line — which is stroked, swallows the press and shows nothing. That is the
///   defect this floor has always existed against, recreated by the ceiling.
/// - **The answer, not «a line».** A review pointed out that one line's worth of room is not the
///   promise: the line saying how to get out wraps to about three at this application's narrowest
///   console, so «a line fits» let through boxes showing half an instruction. A console that cannot
///   show the whole way out gets no box, which is a silence — recorded as such rather than traded
///   for an unreadable rectangle.
fn question_box_under(
    area: Rect,
    metrics: &CellMetrics,
    measured: &Measured,
    reserved: f32,
) -> Option<Rect> {
    let margin = metrics.width();
    // **Clamped to the console, and what the clamp does and does not do is worth being exact about.**
    // A pane can be three rows tall and twenty columns wide, and an unclamped box runs off it. The
    // clamp keeps the **box** inside the console so its border is not stroked where the console is
    // not; it does **not** shorten the text, which the painter's own clip cuts wherever it ends.
    // What protects the answer is not this line: it is that **the keys are the first thing in the
    // galley**, so the line that survives a clip is the one saying how to get out. A question whose
    // answer is off-screen is worse than no question.
    // The box starts a margin below the console's top, so what the ceiling leaves it is half the
    // height minus that offset — which is what makes «nothing is drawn in the bottom half» true of
    // the box's own bottom edge and not merely of its origin.
    let ceiling = area.height() * NOTICE_SHARE - margin - reserved;
    let tall_enough = (measured.tallest.y + 2.0 * margin).min(ceiling);
    let box_of_it = Rect::from_min_size(
        area.left_top() + vec2(margin, margin),
        vec2(measured.tallest.x + 2.0 * margin, tall_enough),
    )
    .intersect(area);
    // **The floor asks for the ANSWER, not a line, and not a box a line tall.** With the ceiling in
    // place those stopped being the same question twice over: a box exactly one row high has a text
    // area of one row minus two margins, which is nothing — and one row of room still cuts an
    // instruction that wraps to three at a narrow console.
    let worth_pressing =
        box_of_it.height() - 2.0 * margin >= measured.answer && box_of_it.width() >= 4.0 * margin;
    worth_pressing.then_some(box_of_it)
}

/// The button's label, laid out on one line: it is two words and it is never wrapped.
fn send_label(
    painter: &Painter,
    theme: &Theme,
    metrics: &CellMetrics,
) -> std::sync::Arc<eframe::egui::Galley> {
    painter.layout_no_wrap(
        SEND_IT_ANYWAY.to_owned(),
        metrics.font(),
        theme.foreground(),
    )
}

/// What the console says when it has stopped receiving its program's output.
///
/// **What it says and why it says only that is **D41**.** The constraint whoever edits this line
/// needs is the one it is easiest to break: it promises what the reading's ending tells us and no
/// more. That the output stopped is a fact; that the program is over is not (**D38**), and whether
/// what you type still arrives is the input side's business and travels in the sentence below.
pub(crate) const STOPPED_RECEIVING: &str =
    "This console stopped receiving its program's output. The program may still be running.";

/// What it adds when the console can still be typed into, and **only** then.
///
/// **The reading's ending says nothing about the input side, and a review caught this promise being
/// made on its behalf.** The two directions are two threads with two endings: a console can be
/// quiet **and** deaf — the writer gives up, and **D29**'s own line then says «this console can no
/// longer be typed into» — so the glass would have been promising delivery in the same window where
/// standard error denied it. The clause is added only where it is true.
pub(crate) const AND_STILL_TAKES_WHAT_YOU_TYPE: &str = " What you type still reaches it.";

/// What the console says when the input queue refused what was just typed — **#161, D60**.
///
/// **A sentence of its own, not a clause appended to [`STOPPED_RECEIVING`].** That sentence opens
/// by claiming the *output* stopped, which is false here: the queue's ceiling is an input-side
/// limit, and it is routinely hit while the program is reading its output perfectly normally.
/// Promising the wrong direction is exactly the mistake **D41** recorded once already, over the
/// clause this sentence's sibling adds.
pub(crate) const INPUT_QUEUE_FULL: &str =
    "This console's input queue is full. What you just typed did not reach the program.";

/// What a console that went quiet says, whole.
///
/// **It takes the fact and not the console**, so both sentences can be asked for: a write failure
/// is what makes a console deaf, and one cannot be produced from outside `term-core` any more than
/// a failed read can. What a test can hold is the decision.
pub(crate) fn what_it_says(deaf: bool) -> String {
    let mut sentence = STOPPED_RECEIVING.to_owned();

    if !deaf {
        sentence.push_str(AND_STILL_TAKES_WHAT_YOU_TYPE);
    }

    sentence
}

/// Whether this console has stopped taking what is typed into it as well as writing.
fn is_deaf(console: &crate::Pane) -> bool {
    console.pty.backlog().is_some_and(|owed| owed.gave_up)
}

/// Whether this console **still** has something to say about having gone quiet.
///
/// **Any ending that is not an ordinary one**, which is the same partition of endings the window
/// closes itself by: a read that failed, a reader that died, and an output nobody was left to take.
/// Nothing recorded is a console still reading, and says nothing.
///
/// **«Still», because a dismissal makes it false** while the ending it was about stays exactly as
/// the pseudoterminal recorded it. The name says what the answer is for and not what happened to
/// the console, which is the other question and is asked of [`went_quiet`].
///
/// **It is a free function and not a method on the pane** because the pane lives in the file whose
/// ratchet (**D28**) may not grow, and both readers of this question — the painting and the mouse —
/// are out here.
pub(crate) fn still_says_it_went_quiet(console: &crate::Pane) -> bool {
    went_quiet(console.pty.ending()) && !console.pty_output.the_silence_was_dismissed()
}

/// Whether an ending is one there is something to say about.
///
/// **It takes the value and not the console** because the one other caller has the value and no
/// console: what a press is allowed to dismiss is decided where the press is answered.
pub(crate) fn went_quiet(ending: Option<term_core::Ending>) -> bool {
    ending.is_some_and(|how| how != term_core::Ending::EndedWell)
}

/// Whether this console still has something to say about a write the input queue refused —
/// **#161, D60** — independent of whether its output also stopped: the two are different threads
/// with different endings, the same reason [`still_says_it_went_quiet`] is a question of its own
/// rather than a clause on this one.
///
/// **«Still» means «since the last dismissal», not «ever»**: each refusal is owed its own sentence,
/// and [`crate::pty_output::PtyOutput::still_owes_the_refusal`] is what tells them apart.
pub(crate) fn still_says_a_write_was_refused(console: &crate::Pane) -> bool {
    console
        .pty_output
        .still_owes_the_refusal(console.pty.backlog())
}

/// Which of its two reasons the box under the held paste's question is about.
///
/// **One answer, asked by the painting and by the press alike**, because a press dismisses the
/// reason on screen and nothing else. The delivery that brought the second reason first wrote the
/// precedence out twice — once to choose the sentence, once to choose what a press dismisses — and
/// two copies of one rule are how a press ends up dismissing a sentence nobody was shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Silence {
    /// The console stopped receiving its program's output (**D40**, **D41**).
    WentQuiet,
    /// The input queue refused what was just typed (**D60**).
    Refused,
    RestoreDirectory,
}

/// What the box under the held paste's question is about, or nothing if it has nothing to say.
///
/// **The output ending wins when both apply.** A write that gave up entirely (permanent) can leave
/// `refused` true from before it did, alongside an ending that has since gone quiet too; composing
/// both into one sentence is out of this ticket's scope (`03-design.md`'s `## Identified risks`,
/// F2) — the existing, more severe claim about the program being gone is the one kept.
pub(crate) fn what_the_silence_is(console: &crate::Pane) -> Option<Silence> {
    if still_says_it_went_quiet(console) {
        Some(Silence::WentQuiet)
    } else if still_says_a_write_was_refused(console) {
        Some(Silence::Refused)
    } else if console.restoration.failed {
        Some(Silence::RestoreDirectory)
    } else {
        None
    }
}

/// The sentence for a reason, as the box lays it out and paints it.
fn what_the_silence_says(console: &crate::Pane, silence: Silence) -> String {
    match silence {
        Silence::WentQuiet => what_it_says(is_deaf(console)),
        Silence::Refused => INPUT_QUEUE_FULL.to_owned(),
        Silence::RestoreDirectory => {
            if let Some(identity) = &console.restoration.resume {
                return format!(
                    "The saved {} session could not be restored. It will be retried next time. Typing or clicking this notice starts fresh.",
                    identity.client.name()
                );
            }
            let path = console
                .restoration
                .directory
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_string_lossy();
            let short: String = path.chars().take(80).collect();
            let suffix = if path.chars().count() > 80 { "..." } else { "" };
            format!(
                "The saved directory could not be restored: {short}{suffix}. It will be retried next time. Typing or clicking this notice starts fresh."
            )
        }
    }
}

/// Where the notice about a console going quiet goes, or nothing if there is no room for it whole.
///
/// **It stacks under the held paste's box, and gives way to it.** Both notices share one ceiling —
/// half the console, **D39** — so when only one fits, the paste keeps the top and this one is not
/// drawn at all. That order is not alphabetical: the paste's notice is a **question**, with bytes
/// waiting on an answer and a key to press, and this one is a statement about something that has
/// already happened and cannot be undone.
///
/// **Whole or not at all**, unlike the paste's box, which the ceiling is allowed to shorten. There
/// the ceiling wins over showing the whole sentence because a clipped question still has its answer
/// on the first line; here there is no answer to protect and a half-shown statement is worse than
/// none — a console that cannot show it says nothing, and what a person sees then is a console that
/// stopped and a window that stayed open.
///
/// **It is told where the boxes above it end** — the question, or the button's slot under it — so
/// they are worked out in one pass and a dismissal of this one moves nothing above it.
fn the_silence_box_under(
    painter: &Painter,
    console: &crate::Pane,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
    above: Option<Rect>,
) -> Option<Rect> {
    let silence = what_the_silence_is(console)?;
    let margin = metrics.width();
    let wrap = (area.width() - 4.0 * margin).max(metrics.width());
    let sentence = painter
        .layout(
            what_the_silence_says(console, silence),
            metrics.font(),
            theme.foreground(),
            wrap,
        )
        .size();
    // Under the paste's boxes when there are any, and a margin below the console's top when there
    // are not. Asked of the one place that decides them, so the two cannot disagree about where they
    // end.
    let top = above.map_or(area.top() + margin, |paste| paste.bottom() + margin);
    let box_of_it = Rect::from_min_size(
        pos2(area.left() + margin, top),
        vec2(sentence.x + 2.0 * margin, sentence.y + 2.0 * margin),
    )
    .intersect(area);
    // **The half is measured on the box's own bottom edge**, which is what makes «the bottom half
    // carries no notice» a statement about what is drawn and not about where it starts.
    let inside_the_ceiling = box_of_it.bottom() <= area.top() + area.height() * NOTICE_SHARE;
    // **The ceiling is also what makes it whole, and a review measured the other clause dead.** A
    // box the console cannot hold is clipped to the console, whose bottom is past the half by
    // definition, so «the sentence fits» can only fail where «it is inside the ceiling» already
    // has. What is left is the width, which a three-cell console does refuse on its own. The
    // promise is asserted over the box that comes back, not over this line.
    let wide_enough = box_of_it.width() >= 4.0 * margin;
    (inside_the_ceiling && wide_enough).then_some(box_of_it)
}

/// Says, over the console, that it stopped receiving.
///
/// It is painted **over the grid and not into it**, for the reason the sibling notice gives: the
/// grid is what the program wrote, and a sentence of the application's put there would survive in
/// the scrollback as if the program had said it.
fn paint_the_silence_over_it(
    painter: &Painter,
    console: &crate::Pane,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
    (box_of_it, silence): (Rect, Silence),
) {
    let margin = metrics.width();
    let sentence = painter.layout(
        what_the_silence_says(console, silence),
        metrics.font(),
        theme.foreground(),
        (area.width() - 4.0 * margin).max(metrics.width()),
    );
    // **The box was measured from a layout of its own, and nothing tied the two together** — the
    // sibling notice carries the same guard for the same reason. Diverge and the clip shows half a
    // sentence with no assertion anywhere failing.
    debug_assert!(
        sentence.size().y <= box_of_it.height() - 2.0 * margin + 1.0,
        "the box is {:?} and its sentence lays out {:?}: it was measured against something other \
         than what is being drawn",
        box_of_it,
        sentence.size()
    );
    painter.rect_filled(box_of_it, margin, theme.panel_background());
    painter.rect_stroke(
        box_of_it,
        margin,
        Stroke::new(1.0, theme.focus()),
        StrokeKind::Inside,
    );
    // Clipped to its own box for the reason the sibling records: the only clip otherwise in force is
    // the console's, and the intersection is kept rather than replaced so this does not depend on
    // every caller handing in an area inside its own clip.
    painter
        .with_clip_rect(box_of_it.intersect(painter.clip_rect()))
        .galley(
            box_of_it.left_top() + vec2(margin, margin),
            sentence,
            theme.foreground(),
        );
}

/// Says, over the console, that a paste is being held and what answers it.
///
/// It is painted **over the grid and not into it**. Writing the sentence into the screen would put
/// words in the program's mouth — the grid is what the program wrote — and it would survive in the
/// scrollback as if it had. This is the application speaking, and it goes where the application can
/// take it back.
fn paint_the_question_over_it(
    painter: &Painter,
    question: String,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
    box_of_it: Rect,
    reserved: f32,
) {
    let margin = metrics.width();
    let sentence = painter.layout(
        question,
        metrics.font(),
        theme.foreground(),
        (area.width() - 4.0 * margin).max(metrics.width()),
    );
    // **The escape clause moved with the ceiling, and it had to.** It used to be «the box already
    // fills the console, so a clip is expected» — a condition the ceiling makes **unreachable**,
    // since the box can no longer reach the console's height. Left as it was, every console too
    // short for the sentence would trip this assert in a debug build. What is expected now is a clip
    // when the box is **at its ceiling**, which is the same thought about the new bound.
    // **What this catches changed twice, and the message says what it catches now.** It was written
    // for text painted outside its own border; with the clip below that cannot happen any more, so
    // what is left — and it is still worth having — is a box that is **not** at its ceiling and yet
    // too small for its text, which means the box was measured with something other than the
    // sentence it is about to draw.
    //
    // **Both clauses carry the same slack, and the second one had none.** The height is a difference
    // of two sums, so it lands a few units of least precision under a bound re-derived from the
    // console's own height — never at the window's top, where every test builds its rectangle, and
    // in a good share of the geometries a split or a tab bar produces. A debug build would have
    // panicked while painting a held paste, in exactly the case this clause exempts.
    debug_assert!(
        sentence.size().y <= box_of_it.height() - 2.0 * margin + 1.0
            || box_of_it.height() + 1.0 >= area.height() * NOTICE_SHARE - margin - reserved,
        "the box is {:?} and its sentence lays out {:?}, and the box is not at its ceiling either: \
         it was measured against something other than what is being drawn",
        box_of_it,
        sentence.size()
    );
    painter.rect_filled(box_of_it, margin, theme.panel_background());
    painter.rect_stroke(
        box_of_it,
        margin,
        Stroke::new(1.0, theme.focus()),
        StrokeKind::Inside,
    );
    // **Clipped to its own box, and until a review said so it was not.** The ceiling shortens the
    // box and nothing shortened the text: the only clip in play is the console's, set by whoever
    // paints the pane, so every line past the ceiling was drawn **outside** the filled rectangle,
    // unbacked, over the grid — in the half the ceiling exists to keep clear — and outside the
    // rectangle a press is judged by, so a click on the visible words started a selection instead of
    // answering. The reasoning that made the ceiling the right choice («a clip takes the tail, never
    // the answer, because the keys are first in the galley») was true of a clip that did not exist.
    //
    // **Intersected with the clip already in force** rather than replacing it, and it is worth being
    // exact about why, because a validation round proposed removing it as the identity — which it is,
    // on every path that exists today.
    //
    // It is kept anyway, and not as insurance against a hypothetical: `with_clip_rect` **sets** the
    // clip, so the bare box would draw this notice wherever the box happens to be, and the box's
    // position comes from an `area` this function does not own. Removing the intersection makes
    // correctness depend on every caller handing in an area inside its own clip — a contract nobody
    // states and nothing checks. That is a present constraint, not a future one, which is what tells
    // this apart from the clamp this delivery did remove: that one gave the same answer in its only
    // scenario, and this one changes what is drawn in a scenario no caller announces it cannot
    // create.
    painter
        .with_clip_rect(box_of_it.intersect(painter.clip_rect()))
        .galley(
            box_of_it.left_top() + vec2(margin, margin),
            sentence,
            theme.foreground(),
        );
}

/// Where the notices and the button go on one console, worked out in one pass.
///
/// **One pass because the second box is measured against where the first one ends**, so asking for
/// them separately lays the question's text out twice — four layouts on every quiet pane, in the
/// mouse pass as well as the painting one, and **undeclared work is published as the framework's
/// cost** (**D13**). A review found it.
pub(crate) struct Notices {
    /// The held paste's question, which keeps the top.
    pub question: Option<Rect>,
    /// Where the button goes, whether or not it is offered: the question left this room under it.
    pub slot: Option<Rect>,
    /// The button under it that sends the paste, where it has room and the console takes input.
    pub send: Option<Rect>,
    /// The sentence about this console having gone quiet, under whatever is above it.
    pub silence: Option<Rect>,
}

/// Where every box goes, asked once by whoever needs any of them.
///
/// **That is not the same as the press being judged against this frame's geometry, and a review was
/// right to say so.** The mouse pass runs before anything is painted, so the console rectangle it
/// hands in is the one the previous frame drew with; on the frame a panel first appears they differ.
/// The press is answered where the person aimed it, which is what they could see. What is gone is the
/// *stored* rectangle, not the one-frame-old geometry — those are different claims and only the first
/// one is made here.
pub(crate) fn the_boxes(
    painter: &Painter,
    console: &crate::Pane,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
) -> Notices {
    let Some(measured) = measure_the_question(painter, console, theme, metrics, area) else {
        return Notices {
            question: None,
            slot: None,
            send: None,
            silence: the_silence_box_under(painter, console, theme, metrics, area, None),
        };
    };
    let margin = metrics.width();
    let button = send_label(painter, theme, metrics).size() + vec2(2.0 * margin, 2.0 * margin);
    // **The button is the convenience and it is the one that gives way.** The question is laid out
    // leaving the button's height free under the ceiling; if that shorter question fails its own
    // floor, or the button placed a margin in would run past the console's right edge, there is no
    // slot and the question is the one it always was — so no console loses the notice to make room.
    // The slot's bottom edge is inside the top half by construction: the question is capped at the
    // ceiling minus exactly what goes under it.
    let with_room =
        question_box_under(area, metrics, &measured, button.y + margin).and_then(|question| {
            let slot =
                Rect::from_min_size(pos2(question.left(), question.bottom() + margin), button);
            (slot.right() <= area.right()).then_some((question, slot))
        });
    let (question, slot) = match with_room {
        Some((question, slot)) => (Some(question), Some(slot)),
        None => (question_box_under(area, metrics, &measured, 0.0), None),
    };
    // **The geometry above depends neither on the reading's ending nor on whether «stopped
    // receiving» was dismissed** — two things that can change between a paint and a press — so a
    // press aimed at the button cannot find the question grown under it. Whether the button is **offered** is the one thing
    // left to the console's state: not on a console whose writer gave up, where the write would go
    // nowhere. Its slot stays empty there, and a press on it answers whatever is drawn there — the
    // sentence about that console when it stands, the grid otherwise.
    let send = slot.filter(|_| !is_deaf(console));

    Notices {
        // Under the button when it is offered. On a console whose writer gave up its empty place
        // protects nothing, and the sentence about that console is worth more than the room; the
        // writer never comes back, so the sentence only ever moves up, where a press dismisses it.
        silence: the_silence_box_under(painter, console, theme, metrics, area, send.or(question)),
        question,
        slot,
        send,
    }
}

/// Says the notices over a console, and the button under the question, in the order they stack.
///
/// `keys` is what the caller paints this console's cursor from. See [`crate::painter::Keys`].
pub(crate) fn paint_them_over_it(
    painter: &Painter,
    console: &crate::Pane,
    theme: &Theme,
    metrics: &CellMetrics,
    area: Rect,
    keys: crate::painter::Keys,
) {
    let boxes = the_boxes(painter, console, theme, metrics, area);

    if let (Some(box_of_it), Some(held)) = (boxes.question, console.held.as_ref()) {
        // What the question was laid out leaving free under it, which is what its assert's escape
        // clause has to be measured against — read off the slot, not worked out again.
        let reserved = boxes
            .slot
            .map_or(0.0, |slot| slot.bottom() - box_of_it.bottom());
        // **Every console but the one the window hands its keys to is told «focus this console»**,
        // with the window away or not: a click there focuses both.
        let how_to_answer = match keys {
            crate::painter::Keys::Here => ANSWERED_BY_HAND,
            crate::painter::Keys::OnceTheWindowIsBack => ANSWERED_ONCE_BACK,
            crate::painter::Keys::Elsewhere => ANSWERED_BY_CLICK,
        };
        let question = format!("{how_to_answer}\n\n{}", held.sentence());
        paint_the_question_over_it(painter, question, theme, metrics, area, box_of_it, reserved);
    }
    if let Some(box_of_it) = boxes.send {
        paint_the_button_over_it(painter, theme, metrics, box_of_it);
    }
    if let Some(silence) = boxes.silence.zip(what_the_silence_is(console)) {
        paint_the_silence_over_it(painter, console, theme, metrics, area, silence);
    }
}

/// Paints the button that sends a held paste.
///
/// The notice's own look — filled, stroked in the focus colour, fill before stroke so the border is
/// not covered — and its label clipped to its box for the reason the question gives.
fn paint_the_button_over_it(
    painter: &Painter,
    theme: &Theme,
    metrics: &CellMetrics,
    box_of_it: Rect,
) {
    let margin = metrics.width();
    painter.rect_filled(box_of_it, margin, theme.panel_background());
    painter.rect_stroke(
        box_of_it,
        margin,
        Stroke::new(1.0, theme.focus()),
        StrokeKind::Inside,
    );
    painter
        .with_clip_rect(box_of_it.intersect(painter.clip_rect()))
        .galley(
            box_of_it.left_top() + vec2(margin, margin),
            send_label(painter, theme, metrics),
            theme.foreground(),
        );
}
