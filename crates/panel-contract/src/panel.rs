//! What `panel.json` says, and how it is read without anything it carries taking down the window.
//!
//! The names of the types and of their fields **are the contract's**, in English like it: whoever
//! compares this file with the specification does not have to translate anything along the way.

use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// The most that is read from a file.
///
/// It is not prudence: it bounds **what a file from outside can make get reserved when it is
/// interpreted**, and that is why it is applied **inside** the read —see [`read`]—, because checking
/// it afterwards would already have reserved.
///
/// **The multiplier depends on the shape of the file, not on its size**, and that is why there are
/// three numbers and not one. With exactly 256 KiB, measured in `--release` with
/// `cargo run --release --example read -p panel-contract`:
///
/// | Shape | Peak | Factor | Interpret |
/// |---|---|---|---|
/// | Lines of a real panel | 1.8 MiB | 7× | 2.8 ms |
/// | A single giant line | 769 KiB | 3× | 0.1 ms |
/// | Objects with one key, 32,766 of them — the worst case | **24 MiB** | **95×** | **22 ms** |
///
/// **What the ceiling does NOT bound is the time.** The normal case does not come close: the nine
/// `panel.json` in the repo cost around **10 µs each**, so re-reading the four of four splits would be
/// a quarter of 1 % of a frame. The one that bites is the pathological one —22 ms, a frame and a
/// half—, and it is enough that it is possible: whoever reads this from a painting loop **cannot
/// re-read on every frame**, it has to re-read only when the file has really changed.
///
/// And along the way it bounds the lines and their length, which is what makes it unnecessary to put a
/// separate ceiling on them.
pub const MAX_BYTES: u64 = 256 * 1024;

/// The most a line is indented.
///
/// It is the width of the widest panel in the mockup, in cells: more than that would not fit anyway,
/// and without a ceiling whoever paints would repeat as many spaces as a file from outside says.
pub const MAX_INDENT: u64 = 64;

/// How much silence is legitimate before warning, when the producer does not say.
pub const STALE_AFTER: Duration = Duration::from_secs(30 * 60);

/// What the producer publishes.
///
/// All fields may be absent except the lines: a panel without lines is not a panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel {
    /// Contract version. **Absent in seven of the nine files in the repo**, because it was added
    /// later: its absence is not an error.
    pub version: Option<u64>,
    /// What the producer says about when it published, as is, **as text and uninterpreted**. It is
    /// neither painted yet nor used to compute the age — the age comes from the file's own date, see
    /// [`Panel::written_at`], and `docs/04-contrato-panel.md` records that this field is read and not
    /// painted.
    pub updated_at: Option<String>,
    /// What the producer is working on. Free text of its own, and painted literally: translating it
    /// to a color or an icon would be interpreting its vocabulary, and the vocabulary belongs to
    /// whoever does the work.
    pub phase: Option<String>,
    /// Whether the application adds its identity line at the top. By default, yes.
    pub header: bool,
    /// How much silence this producer tolerates before its panel is marked as old.
    pub stale_after: Duration,
    /// Whether this split **is calling for something**, and of what kind. Nothing = it is not calling.
    ///
    /// **It is the only field of the contract that the application interprets**, and that is
    /// deliberate: it answers the only question it needs in order to navigate —do I have to go there?—
    /// and from it come the panel's indicator and the one on its tab's label. The rest of the
    /// vocabulary —the marks, the styles, the phase— **is stored and painted**, because translating it
    /// would be the application deciding what someone else's work means.
    ///
    /// **It is not deduced from the body**: a line with [`Mark::Block`] is content of the summary, not
    /// a signal. Deducing it from there would be fragile in the silliest way — a "I waited for approval
    /// on Tuesday" would mark the tab forever.
    pub attention: Option<Attention>,
    /// The body.
    pub lines: Vec<Line>,
    /// When the file was written, **which is where the age comes from**.
    ///
    /// It does not come from `updated_at`, and the reason is the contract's own: the producer
    /// **overwrites the whole file** on every publication, so the file's date *is* that of its last
    /// publication. This way there is a stale notice even when the producer does not write its mark
    /// —which is when it is most needed— and this crate does not need to interpret dates nor drag along
    /// a calendar.
    pub written_at: Option<SystemTime>,
}

/// A line of the panel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub text: String,
    /// What this line **is**. The application chooses what symbol it is painted with and how it is
    /// aligned.
    pub mark: Option<Mark>,
    /// Short label that is aligned in a column. It does not have to be a number.
    pub reference: Option<String>,
    /// How it is painted. **It can come alongside a mark**, and then the color is set by the style and
    /// the symbol by the mark: two of the nine files in the repo do it, and on purpose — it is how you
    /// say "this line is information, and its color *is* the data".
    pub style: Option<Style>,
    /// Manual indentation, in cells.
    pub indent: usize,
    /// The address the producer associates with this line. **It is read and not used yet**: painting
    /// it would show something different from what would be copied, because painting discards
    /// characters the string does have.
    pub link: Option<String>,
}

/// What a line is. The six of the contract, not one more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Done,
    Current,
    Pending,
    Wait,
    Block,
    Info,
}

/// What a split is calling for, when it is calling for something.
///
/// **It is ordered by the priority the contract declares**, not by the place where the variants are
/// written: when a tab has several splits, "what calls the most" is the maximum of their calls.
/// [`Attention::Wait`] is the greatest because **it is the only one that gets unblocked with you in
/// front of it right now**; a split blocked by a third party can wait five more minutes, and one that
/// finished and nobody has looked at, all day.
///
/// **The order is written, not derived, and the difference matters the day there is a fourth value.** A
/// derived `Ord` orders by the position of the variants in the file: whoever added one in the middle
/// would change the priority **silently**, without the compilation or any test failing. With the
/// `match` below, that fourth variant **does not compile** until someone decides what priority it has —
/// which is exactly the conversation that has to happen. And the variants can then be declared in
/// **the contract's order**, like those of [`Mark`], instead of the other way around just so the
/// derive works out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    /// **It is waiting for you**: a question, a permission, a decision.
    Wait,
    /// Stuck on something else, not you.
    Block,
    /// It finished and nobody looked.
    Done,
}

impl Attention {
    /// The three, for whoever has to iterate over them — a color table, a symbol one, a test.
    ///
    /// **It lives here, right next to `priority`, and not in every place that iterates over them.** A
    /// literal list repeated around admits a fourth variant **silently**, which is exactly what the
    /// `match` below prevents for the production tables: whoever adds one will not compile until
    /// touching `priority`, and then has this list three lines above.
    pub const ALL: [Self; 3] = [Self::Wait, Self::Block, Self::Done];

    /// How much it is calling, so two can be compared. Numbers with no more meaning than their order.
    fn priority(self) -> u8 {
        match self {
            Self::Done => 0,
            Self::Block => 1,
            Self::Wait => 2,
        }
    }
}

impl Ord for Attention {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority().cmp(&other.priority())
    }
}

impl PartialOrd for Attention {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// How a line is painted. The seven of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Normal,
    Dim,
    Title,
    Accent,
    Ok,
    Warn,
    Error,
}

/// Why a panel could not be read.
///
/// Each variant is a different thing to tell the user: that its producer has not published yet looks
/// nothing like it having published something that cannot be understood.
#[derive(Debug)]
pub enum Problem {
    /// There is no file yet. The normal case before the first publication.
    Missing,
    /// There is something with that name, but it is not a file.
    ///
    /// **It matters because a named pipe would block the open**, and since reading happens inside the
    /// frame, that would be the window frozen. It is checked **before** opening, so it covers the
    /// oversight —a producer that creates there what it should not— and not a deliberate race: between
    /// the check and the open there is a gap, and whoever can write in that directory is the user
    /// themselves.
    NotRegular,
    /// It goes over the [`MAX_BYTES`] ceiling.
    TooBig,
    /// It could not be opened or read.
    Unreadable(std::io::Error),
    /// It was read, but it is not a panel: it is not JSON, or it carries no lines.
    Invalid(String),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "not published yet"),
            Self::NotRegular => write!(f, "there is something there that is not a file"),
            Self::TooBig => write!(f, "over {} KiB", MAX_BYTES / 1024),
            Self::Unreadable(failure) => write!(f, "could not be read ({failure})"),
            Self::Invalid(reason) => write!(f, "not understood ({reason})"),
        }
    }
}

impl Panel {
    /// Whether what is shown is no longer trustworthy for being old.
    ///
    /// **The subtraction is signed on purpose.** A date in the future is not the rare case: a fast
    /// clock or a foreign filesystem is enough. With the naive subtraction of times that is a panic,
    /// and this path cannot panic over anything that comes from outside.
    pub fn is_stale_at(&self, now: SystemTime) -> bool {
        let Some(written) = self.written_at else {
            return false;
        };
        match now.duration_since(written) {
            Ok(silence) => silence > self.stale_after,
            // The file says it was written in the future: there is no silence to measure.
            Err(_) => false,
        }
    }
}

/// Reads a directory's panel **only if its file has changed** since the date it is given.
///
/// `Ok(None)` means "what you have is still current", and it is the answer that makes re-reading from
/// a painting loop cheap: asking for the date costs less than a microsecond, and interpreting can cost
/// a whole frame — see [`MAX_BYTES`].
///
/// **It lives here and not in the caller, and that boundary is the reason it exists.** Deciding whether
/// the file changed requires knowing **what the file is called**, and that name is the contract's:
/// whoever looked at it on their own would be writing it a second time, and would ask for the file's
/// data twice per read. The date to compare against comes from [`Panel::written_at`] of the previous
/// read.
///
/// Without a file the problem is returned, not `None`: "it disappeared" and "it is unchanged" are
/// different things for whoever paints.
pub fn read_if_newer(dir: &Path, seen: Option<SystemTime>) -> Result<Option<Panel>, Problem> {
    match std::fs::metadata(dir.join(crate::PANEL_FILE)) {
        Ok(metadata)
            if metadata
                .modified()
                .is_ok_and(|modified| Some(modified) == seen) =>
        {
            Ok(None)
        }
        // **Absence is resolved here and not by going back to [`read`]**, which is the most frequent
        // case of all: a console whose producer publishes nothing passes through here on every frame,
        // and letting it continue would ask for the file's data **twice** to reach the same
        // conclusion.
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => Err(Problem::Missing),
        _ => read(dir).map(Some),
    }
}

/// Reads the panel in a split's directory.
///
/// The file name is set by this crate, not the caller: it is part of the contract and lives in a
/// single place.
pub fn read(dir: &Path) -> Result<Panel, Problem> {
    let path = dir.join(crate::PANEL_FILE);

    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(failure) if failure.kind() == std::io::ErrorKind::NotFound => {
            return Err(Problem::Missing);
        }
        Err(failure) => return Err(Problem::Unreadable(failure)),
    };
    if !metadata.is_file() {
        return Err(Problem::NotRegular);
    }

    let file = std::fs::File::open(&path).map_err(Problem::Unreadable)?;
    let bytes = read_capped(file, metadata.len()).map_err(Problem::Unreadable)?;
    // **The cap and the encoding are decided separately, and in this order.** Reading directly to
    // text tied the two things together: the cap's cut splits a multi-byte character —and here there
    // are accents and em dashes in every real file—, so a huge file came out as "could not be read"
    // and the "goes over the ceiling" path was almost never reached.
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Problem::TooBig);
    }
    let text =
        String::from_utf8(bytes).map_err(|_| Problem::Invalid("not UTF-8 text".to_owned()))?;

    let mut panel = parse(&text)?;
    panel.written_at = metadata.modified().ok();
    Ok(panel)
}

/// Reads at most one byte more than the ceiling.
///
/// **The ceiling is applied while reading, and only here.** Looking at the size before opening seems
/// cheaper, but it would be a second path that no test can exercise —the first would always reject
/// earlier— and on top of that it would leave out the race that matters: the producer overwrites the
/// whole file, so between looking and reading it may have written a bigger one.
///
/// The extra byte is what lets "it takes up exactly the ceiling" be told apart from "it goes over". And
/// it is separate from the rest **so that how much is read can be asserted**: looking only at the
/// result of [`read`], a cap that bounded nothing would give the same error, because the check
/// afterwards would cover it up.
///
/// **`expected` is a size hint, not a check**, and the distinction is exactly the one the decision
/// above defends: nobody decides anything with that number, it only reserves room in one go. Without
/// it, `read_to_end` starts at 32 bytes and doubles: measured, **nine system calls for an 861-byte
/// panel and seventeen for one at the ceiling**, which on top of that leaves reserved twice what it
/// takes up. With the hint it is four and five, and no reallocation. It matters little on a local disk
/// and much on a network one, where each round trip is milliseconds inside the frame. If the producer
/// overwrites the file with a bigger one between the `metadata` and the read, the hint falls short and
/// `read_to_end` grows on its own: the ceiling is still set by the `take`.
fn read_capped(file: std::fs::File, expected: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(expected.min(MAX_BYTES) as usize + 1);
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Turns the text into a panel, tolerating per field.
///
/// **Tolerance is per field and not per document**, and that decides how it is read: a single field
/// that is not understood —a number where text went, a mark name that does not exist— is ignored and
/// its default value is used, instead of throwing away the whole file. Whoever publishes is an agent
/// writing JSON by hand: it will happen, and losing the complete panel over a field nobody looks at
/// would be the worst thing it could do.
///
/// There are only two ways for a file not to be valid: that it is not JSON, or that it carries no
/// lines.
fn parse(text: &str) -> Result<Panel, Problem> {
    let raw: serde_json::Value =
        serde_json::from_str(text).map_err(|failure| Problem::Invalid(failure.to_string()))?;

    let lines = raw
        .get("lines")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Problem::Invalid("carries no \"lines\"".to_owned()))?;

    Ok(Panel {
        version: raw.get("v").and_then(serde_json::Value::as_u64),
        updated_at: string(&raw, "updated_at"),
        phase: string(&raw, "phase"),
        header: raw
            .get("header")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        stale_after: raw
            .get("stale_after_minutes")
            .and_then(serde_json::Value::as_u64)
            .map_or(STALE_AFTER, |minutes| {
                Duration::from_secs(minutes.saturating_mul(60))
            }),
        attention: string_ref(&raw, "attention").and_then(attention),
        lines: lines.iter().filter_map(line).collect(),
        written_at: None,
    })
}

/// Turns an element of the list into a line, or **discards it**.
///
/// An element that is neither a string nor an object —a number, a null— is not a line of anything:
/// painting it as a blank line would be inventing content out of garbage.
fn line(raw: &serde_json::Value) -> Option<Line> {
    // A loose string is a normal line, and the empty string is a gap: the two cases are the same
    // here, and it is whoever paints that knows an empty text leaves a space.
    if let Some(text) = raw.as_str() {
        return Some(Line {
            text: text.to_owned(),
            ..Line::default()
        });
    }
    if !raw.is_object() {
        return None;
    }

    Some(Line {
        text: string(raw, "text").unwrap_or_default(),
        mark: string_ref(raw, "mark").and_then(mark),
        reference: string(raw, "ref"),
        style: string_ref(raw, "style").and_then(style),
        // Bounded **here**, which is the boundary with what comes from outside: whoever paints is
        // going to repeat that number of spaces, and an absurd value would be a panic from reserving
        // memory. The ceiling also prevents the conversion from silently truncating where the
        // machine's integer is smaller.
        indent: raw
            .get("indent")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            .min(MAX_INDENT) as usize,
        link: string(raw, "link"),
    })
}

fn string(raw: &serde_json::Value, field: &str) -> Option<String> {
    string_ref(raw, field).map(str::to_owned)
}

/// The same, **without copying**: for the fields whose value is translated to an own type and thrown
/// away.
///
/// **It exists because this crate measures what it reserves.** The three closed vocabularies —the
/// mark, the style and the call— turn the string into an `enum` that fits in a byte and drop the text
/// one call later; asking for memory for that is three reservations per line that nobody ever uses, in
/// a crate with a whole module dedicated to counting reservations and a table of peaks in its
/// documentation.
fn string_ref<'a>(raw: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    raw.get(field).and_then(serde_json::Value::as_str)
}

fn attention(name: &str) -> Option<Attention> {
    match name {
        "wait" => Some(Attention::Wait),
        "block" => Some(Attention::Block),
        "done" => Some(Attention::Done),
        _ => None,
    }
}

fn mark(name: &str) -> Option<Mark> {
    match name {
        "done" => Some(Mark::Done),
        "current" => Some(Mark::Current),
        "pending" => Some(Mark::Pending),
        "wait" => Some(Mark::Wait),
        "block" => Some(Mark::Block),
        "info" => Some(Mark::Info),
        _ => None,
    }
}

fn style(name: &str) -> Option<Style> {
    match name {
        "normal" => Some(Style::Normal),
        "dim" => Some(Style::Dim),
        "title" => Some(Style::Title),
        "accent" => Some(Style::Accent),
        "ok" => Some(Style::Ok),
        "warn" => Some(Style::Warn),
        "error" => Some(Style::Error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PanelDir;

    /// Composed from the quirks of several real `panel.json` in the repo —it is not a copy of one—:
    /// **style and mark on the same line**, the version absent, and references of very different widths
    /// with an em dash.
    const ACTUAL: &str = r##"{
      "updated_at": "2026-08-11T22:00:00+02:00",
      "phase": "done",
      "header": true,
      "lines": [
        {"text": "F1.4: two splits side by side", "style": "title"},
        "",
        {"ref": "#1", "text": "extract Division from the window", "mark": "done",
         "link": "https://github.com/mashware/agent-terminal/pull/31"},
        {"ref": "#1–#11", "text": "the knowledge", "mark": "wait"},
        {"ref": "Now", "text": "milestone closed", "mark": "info", "style": "ok"},
        "a loose line"
      ]
    }"##;

    fn panel_in(dir: &PanelDir, contents: &str) -> Result<Panel, Problem> {
        std::fs::write(dir.path().join(crate::PANEL_FILE), contents).expect("publish the panel");
        read(dir.path())
    }

    #[test]
    fn a_real_panel_is_read_whole_and_in_order() {
        let dir = PanelDir::create().expect("create the directory");
        let panel = panel_in(&dir, ACTUAL).expect("read the panel");

        assert_eq!(panel.lines.len(), 6);
        assert_eq!(panel.lines[0].style, Some(Style::Title));
        assert_eq!(panel.lines[1].text, "", "the empty string is a gap");
        assert_eq!(panel.lines[2].mark, Some(Mark::Done));
        assert_eq!(panel.lines[2].reference.as_deref(), Some("#1"));
        assert_eq!(panel.lines[3].reference.as_deref(), Some("#1–#11"));
        assert_eq!(panel.lines[5].text, "a loose line");
        assert_eq!(panel.phase.as_deref(), Some("done"));
    }

    /// The contract describes the style as something "for lines without a mark", but **two of the nine
    /// files in the repo combine them**, and on purpose: the mark says what the line is and the style
    /// says its color is the data. If this were read as mutually exclusive, one of the two would be
    /// lost.
    #[test]
    fn a_line_can_carry_a_mark_and_a_style_at_the_same_time() {
        let dir = PanelDir::create().expect("create the directory");
        let panel = panel_in(&dir, ACTUAL).expect("read the panel");

        let now_line = &panel.lines[4];
        assert_eq!(now_line.mark, Some(Mark::Info));
        assert_eq!(now_line.style, Some(Style::Ok));
    }

    #[test]
    fn the_version_can_be_absent() {
        let dir = PanelDir::create().expect("create the directory");
        let panel = panel_in(&dir, ACTUAL).expect("read the panel");

        assert_eq!(
            panel.version, None,
            "seven of the nine in the repo do not carry it"
        );
    }

    #[test]
    fn the_header_is_assumed_and_can_be_removed() {
        let dir = PanelDir::create().expect("create the directory");

        let unstated = panel_in(&dir, r#"{"lines":[]}"#).expect("read");
        assert!(unstated.header, "by default the application adds it");

        let removed = panel_in(&dir, r#"{"header":false,"lines":[]}"#).expect("read");
        assert!(!removed.header);
    }

    /// **Tolerance is per field, not per document.** Whoever publishes is an agent writing JSON by
    /// hand: a badly placed field will happen, and losing the whole panel over one that nobody looks
    /// at would be the worst thing that could be done with it.
    #[test]
    fn a_field_that_is_not_understood_does_not_throw_away_the_file() {
        let dir = PanelDir::create().expect("create the directory");
        let panel = panel_in(
            &dir,
            r#"{"v":"one","header":"yes","stale_after_minutes":-5,"attention":3,
                "lines":[{"text":"ok","mark":"made-up","style":42,"indent":"a lot"}]}"#,
        )
        .expect("a bad field does not invalidate the panel");

        assert_eq!(panel.version, None);
        assert_eq!(
            panel.attention, None,
            "a call that is not even a string has to fall into \"not calling\", not blow up"
        );
        assert!(
            panel.header,
            "a value that is not a boolean falls into the default"
        );
        assert_eq!(panel.stale_after, STALE_AFTER);
        assert_eq!(panel.lines.len(), 1);
        assert_eq!(panel.lines[0].text, "ok", "the line is kept");
        assert_eq!(
            panel.lines[0].mark, None,
            "a mark that does not exist is ignored"
        );
        assert_eq!(panel.lines[0].style, None);
        assert_eq!(panel.lines[0].indent, 0);
    }

    #[test]
    fn the_tolerated_silence_is_said_by_the_producer() {
        let dir = PanelDir::create().expect("create the directory");

        let its_own = panel_in(&dir, r#"{"stale_after_minutes":5,"lines":[]}"#).expect("read");
        assert_eq!(its_own.stale_after, Duration::from_secs(300));

        let by_default = panel_in(&dir, r#"{"lines":[]}"#).expect("read");
        assert_eq!(by_default.stale_after, STALE_AFTER);
    }

    #[test]
    fn without_lines_it_is_not_a_panel() {
        let dir = PanelDir::create().expect("create the directory");

        let failure =
            panel_in(&dir, r#"{"phase":"something"}"#).expect_err("without lines it is not valid");

        assert!(matches!(failure, Problem::Invalid(_)), "was {failure:?}");
    }

    /// A `lines` that is not a list **is not an empty panel**: if it were read as such, whoever paints
    /// would lose the last good panel and in its place would leave a gap, which is exactly what the
    /// distinction between "not understood" and "there is nothing" exists to avoid.
    #[test]
    fn lines_that_are_not_a_list_are_not_a_panel() {
        let dir = PanelDir::create().expect("create the directory");

        for text in [
            r#"{"lines":"just one"}"#,
            r#"{"lines":{"1":"one"}}"#,
            r#"{"lines":7}"#,
        ] {
            let failure = panel_in(&dir, text).expect_err("\"lines\" has to be a list");
            assert!(
                matches!(failure, Problem::Invalid(_)),
                "with {text} was {failure:?}"
            );
        }
    }

    /// Valid JSON but of another shape: the root has to be an object. Without this, looking for the
    /// lines in something that does not have them could end up taking the root itself for the panel's
    /// body.
    #[test]
    fn a_json_that_is_not_an_object_at_the_root_is_not_a_panel() {
        let dir = PanelDir::create().expect("create the directory");

        for text in [r#"["one","another"]"#, r#""just a string""#, "42", "null"] {
            let failure = panel_in(&dir, text).expect_err("the root has to be an object");
            assert!(
                matches!(failure, Problem::Invalid(_)),
                "with {text} was {failure:?}"
            );
        }
    }

    #[test]
    fn what_is_not_json_is_reported_without_breaking_anything() {
        let dir = PanelDir::create().expect("create the directory");

        // A half-written file is exactly this: JSON cut in half.
        let failure = panel_in(&dir, r#"{"lines":[{"text":"a hal"#).expect_err("not JSON");

        assert!(matches!(failure, Problem::Invalid(_)), "was {failure:?}");
    }

    /// **The zero-byte file is the intermediate instant of each publication**: the producer that
    /// overwrites opens to truncate and has not written anything yet. It has to come out "not
    /// understood" and not an empty panel, because an empty panel would erase the last good one right
    /// in the middle of each write.
    #[test]
    fn an_empty_file_is_a_half_write_and_not_a_panel_without_lines() {
        let dir = PanelDir::create().expect("create the directory");

        let failure = panel_in(&dir, "").expect_err("an empty file is not a panel");

        assert!(matches!(failure, Problem::Invalid(_)), "was {failure:?}");
    }

    #[test]
    fn not_published_yet_is_not_a_strange_error() {
        let dir = PanelDir::create().expect("create the directory");

        let failure = read(dir.path()).expect_err("there is no file");

        assert!(matches!(failure, Problem::Missing), "was {failure:?}");
    }

    /// "It has not published yet" and "there is something there I cannot read" are two different things
    /// to tell whoever looks, and only the first is normal. No permissions is the only thing that
    /// separates the two failure branches, and there are **two** branches: the one that looks at what
    /// is there and the one that opens it.
    #[cfg(unix)]
    #[test]
    fn without_permission_to_read_it_it_is_not_confused_with_not_having_published() {
        use std::os::unix::fs::PermissionsExt;

        let dir = PanelDir::create().expect("create the directory");
        let path = dir.path().join(crate::PANEL_FILE);
        std::fs::write(&path, r#"{"lines":[]}"#).expect("publish the panel");
        let mode = |path: &Path, mode| {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
                .expect("change the permissions")
        };

        // The permissions are restored right away, **before** asserting anything: with the restoration
        // behind the `assert`, a test failure would leave a directory its destructor can no longer
        // empty.
        mode(&path, 0o000);
        // Whoever runs the suite as administrator can read anyway, and then there is nothing to check
        // here. It is asked instead of assumed, which is cheaper than bringing in a library just to
        // look at the effective user.
        let unreadable = std::fs::File::open(&path).is_err();
        let from_file = read(dir.path());
        mode(&path, 0o600);

        mode(dir.path(), 0o000);
        let from_dir = read(dir.path());
        mode(dir.path(), 0o700);

        if unreadable {
            assert!(
                matches!(from_file, Err(Problem::Unreadable(_))),
                "the file without permissions came out as {from_file:?}"
            );
            assert!(
                matches!(from_dir, Err(Problem::Unreadable(_))),
                "the directory without permissions came out as {from_dir:?}"
            );
        }
    }

    /// **The same property as the one below, but over the real path.**
    ///
    /// The one below calls [`read_capped`] directly, so it only ties down the helper: replacing that
    /// call inside [`read`] with a `std::fs::read` —reserving the whole file, which is exactly what the
    /// ceiling exists to prevent— **the suite stayed green**, because the check afterwards returns
    /// [`Problem::TooBig`] all the same. Measured: the error does not tell the two paths apart, only
    /// the memory does.
    ///
    /// The file is eight times the ceiling, so reading it whole would reserve 2 MiB and the margin
    /// asserted against is 1 MiB: between what the honest path costs —around 512 KiB, the slack that
    /// `read_to_end`'s doubling leaves over 256 KiB— and what the dishonest one would cost there is a
    /// factor of four, which is plenty of room for this not to depend on the machine.
    #[test]
    fn reading_a_huge_file_does_not_reserve_what_it_takes_up() {
        let dir = PanelDir::create().expect("create the directory");
        let padding = "x".repeat(MAX_BYTES as usize * 8);
        let contents = format!(r#"{{"lines":["{padding}"]}}"#);
        std::fs::write(dir.path().join(crate::PANEL_FILE), &contents).expect("publish a huge one");
        drop(contents);

        let (failure, peak) = while_measuring(|| read(dir.path()));

        assert!(matches!(failure, Err(Problem::TooBig)), "was {failure:?}");
        assert!(
            peak < MAX_BYTES as usize * 4,
            "it reserved {peak} bytes reading a file that is rejected for being big"
        );
    }

    /// The property is **how much is read**, not what error comes out: looking only at the error of
    /// `read`, a cap that bounded nothing would give the same result because the check afterwards would
    /// cover it up.
    ///
    /// **And the size hint cannot turn into the cap.** It is passed the biggest lie that fits —that the
    /// file takes up nothing—: if some day someone reserves by the hint instead of reading up to the
    /// `take`, this case returns one byte and falls here.
    #[test]
    fn of_a_huge_file_only_the_first_bytes_are_read() {
        let dir = PanelDir::create().expect("create the directory");
        let path = dir.path().join(crate::PANEL_FILE);
        std::fs::write(&path, "x".repeat(MAX_BYTES as usize * 4)).expect("write a huge one");

        let read_back = read_capped(std::fs::File::open(&path).expect("open"), 0).expect("read");

        assert_eq!(
            read_back.len() as u64,
            MAX_BYTES + 1,
            "more than the ceiling and one byte was read"
        );
    }

    #[test]
    fn a_huge_file_is_not_read_whole() {
        let dir = PanelDir::create().expect("create the directory");
        let padding = "x".repeat(MAX_BYTES as usize + 1024);
        let too_big = format!(r#"{{"lines":["{padding}"]}}"#);

        let failure = panel_in(&dir, &too_big).expect_err("it goes over the ceiling");

        assert!(matches!(failure, Problem::TooBig), "was {failure:?}");
    }

    /// **With accents, which is the normal thing here**: every real file in the repo carries accents
    /// and em dashes. Reading directly to text tied the cap to the encoding, and the cut split a
    /// two-byte character: the huge file came out as "could not be read" and this path was never
    /// reached.
    #[test]
    fn a_huge_file_with_accents_is_still_too_big() {
        let dir = PanelDir::create().expect("create the directory");
        let padding = "ñ".repeat(MAX_BYTES as usize);

        let failure = panel_in(&dir, &format!(r#"{{"lines":["{padding}"]}}"#))
            .expect_err("it goes over the ceiling");

        assert!(matches!(failure, Problem::TooBig), "was {failure:?}");
    }

    /// A file that is not text is not "could not be read" —that sounds like permissions— but "not
    /// understood". And it is a real case: a write cut right between the two bytes of an accented
    /// letter.
    #[test]
    fn what_is_not_text_is_reported_as_what_it_is() {
        let dir = PanelDir::create().expect("create the directory");
        let path = dir.path().join(crate::PANEL_FILE);
        std::fs::write(&path, b"{\"lines\":[\"caf\xe9\"]}").expect("write raw bytes");

        let failure = read(dir.path()).expect_err("not UTF-8");

        assert!(matches!(failure, Problem::Invalid(_)), "was {failure:?}");
    }

    /// What is neither a string nor an object is not a line: painting it blank would be inventing
    /// content out of garbage.
    #[test]
    fn what_is_not_a_line_is_discarded_instead_of_painted_blank() {
        let dir = PanelDir::create().expect("create the directory");

        let panel = panel_in(&dir, r#"{"lines":[null,42,"good",{"text":"too"}]}"#)
            .expect("the good ones are read");

        assert_eq!(
            panel.lines.len(),
            2,
            "the null and the number are not lines"
        );
        assert_eq!(panel.lines[0].text, "good");
        assert_eq!(panel.lines[1].text, "too");
    }

    /// The time the producer tolerates is multiplied by sixty: an absurd number cannot overflow.
    #[test]
    fn an_absurd_tolerance_time_does_not_overflow() {
        let dir = PanelDir::create().expect("create the directory");

        let panel = panel_in(
            &dir,
            r#"{"stale_after_minutes":18446744073709551615,"lines":[]}"#,
        )
        .expect("read");

        assert_eq!(panel.stale_after, Duration::from_secs(u64::MAX));
    }

    /// The three words with which a producer says its split is calling for something.
    #[test]
    fn the_three_kinds_of_call_are_read() {
        let dir = PanelDir::create().expect("create the directory");

        for (written, expected) in [
            ("wait", Attention::Wait),
            ("block", Attention::Block),
            ("done", Attention::Done),
        ] {
            let panel = panel_in(&dir, &format!(r#"{{"attention":"{written}","lines":[]}}"#))
                .expect("read");

            assert_eq!(panel.attention, Some(expected), "with \"{written}\"");
        }
    }

    /// **The priority is the type's order, and what a tab's label shows depends on it.**
    ///
    /// It is asserted on the **maximum of a list**, which is how whoever accumulates is going to use
    /// it, and not on loose comparisons: with `a < b` in pairs, a badly declared order can keep passing
    /// if the pairs that are tested are not the ones that rule.
    ///
    /// **What this test CANNOT cover, and that is why the priority is written by hand**: a fourth kind
    /// of call that someone added tomorrow would not appear in any of these lists, so the test would
    /// stay green with the new priority undecided. What prevents it is that the `match` of `priority`
    /// stops compiling.
    #[test]
    fn what_calls_the_most_is_the_maximum() {
        use Attention::{Block, Done, Wait};

        assert_eq!([Done, Block, Wait].into_iter().max(), Some(Wait));
        assert_eq!([Block, Wait].into_iter().max(), Some(Wait));
        assert_eq!([Done, Block].into_iter().max(), Some(Block));
        assert_eq!([Done].into_iter().max(), Some(Done));
    }

    /// It is written by a third party by hand, so a word that does not exist is a matter of time — and
    /// the rest of its panel is not to blame.
    #[test]
    fn a_call_that_does_not_exist_is_read_as_if_nothing_had_been_said() {
        let dir = PanelDir::create().expect("create the directory");

        let panel = panel_in(
            &dir,
            r#"{"attention":"waiting","phase":"validate","lines":[{"text":"something"}]}"#,
        )
        .expect("read");

        assert_eq!(panel.attention, None);
        assert_eq!(
            panel.phase.as_deref(),
            Some("validate"),
            "a badly written field wiped out the rest of the panel"
        );
        assert_eq!(panel.lines.len(), 1, "and the body too");
    }

    /// **The body is not a signal, and this is the half of the milestone that lives in this crate.** A
    /// "I waited for approval on Tuesday" would be enough to leave a tab marked forever.
    ///
    /// **It is a regression test, not a coverage one, and it is worth knowing what makes it fail**:
    /// today there is no path that derives the call from the body, so the assertion holds by absence.
    /// It fails **when someone writes that path** — verified by adding to the read an `or_else` that
    /// looked for the first line with a wait or block mark. That is exactly the shortcut this milestone
    /// exists so that nobody takes.
    #[test]
    fn a_body_mark_is_not_a_call_from_the_split() {
        let dir = PanelDir::create().expect("create the directory");

        let panel = panel_in(
            &dir,
            r#"{"lines":[{"text":"I waited for approval on Tuesday","mark":"wait"},{"text":"and this","mark":"block"}]}"#,
        )
        .expect("read");

        assert_eq!(
            panel.lines[0].mark,
            Some(Mark::Wait),
            "the body does carry it"
        );
        assert_eq!(
            panel.attention, None,
            "the body produced a call for the whole split"
        );
    }

    /// The indentation comes from outside and whoever paints will repeat those spaces.
    #[test]
    fn an_absurd_indentation_is_bounded() {
        let dir = PanelDir::create().expect("create the directory");

        let panel = panel_in(
            &dir,
            r#"{"lines":[{"text":"t","indent":18446744073709551615}]}"#,
        )
        .expect("read");

        assert_eq!(panel.lines[0].indent, MAX_INDENT as usize);
    }

    /// A negative indentation is not "a lot of indentation": it is a badly written field and is worth
    /// its default value. The case exists because the number comes in unsigned, and reading it signed
    /// to then convert it would flip the value around and would indent up to the ceiling exactly when
    /// the producer was asking for the opposite.
    #[test]
    fn a_negative_indentation_does_not_flip_around() {
        let dir = PanelDir::create().expect("create the directory");

        let panel = panel_in(&dir, r#"{"lines":[{"text":"t","indent":-3}]}"#).expect("read");

        assert_eq!(panel.lines[0].indent, 0);
    }

    /// What is there might not be a file, and a named pipe would block the open: since reading happens
    /// inside the frame, that would be the window frozen.
    #[test]
    fn what_is_not_a_file_is_rejected_before_opening_it() {
        let dir = PanelDir::create().expect("create the directory");
        std::fs::create_dir(dir.path().join(crate::PANEL_FILE))
            .expect("put a directory where the file went");

        let failure = read(dir.path());
        // Cleanup happens **before** asserting: with the cleanup behind, a test failure would leave a
        // directory the destructor also cannot delete, because it is not empty.
        std::fs::remove_dir(dir.path().join(crate::PANEL_FILE)).expect("clean up");

        assert!(
            matches!(failure, Err(Problem::NotRegular)),
            "was {failure:?}"
        );
    }

    /// **The case the mechanism exists for, and until now was not tested.** The test above uses a
    /// directory, which is precisely the one that does **not** block: opening it fails and that is
    /// that. A named pipe does stay waiting for someone to write on the other side, and since reading
    /// happens inside the frame, that is the window frozen — forever, not for a while.
    ///
    /// If this test were to hang instead of fail, **that is the failure**: it means the regular-file
    /// check disappeared.
    #[cfg(unix)]
    #[test]
    fn a_named_pipe_is_rejected_instead_of_hanging_the_window() {
        let dir = PanelDir::create().expect("create the directory");
        let path = dir.path().join(crate::PANEL_FILE);
        let mkfifo = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .is_ok_and(|status| status.success());
        assert!(mkfifo, "the named pipe could not be created");

        let failure = read(dir.path());
        std::fs::remove_file(&path).expect("clean up the pipe");

        assert!(
            matches!(failure, Err(Problem::NotRegular)),
            "was {failure:?}"
        );
    }

    /// **The flip side of requiring a file: a link to a normal file does work.** What is rejected is
    /// what would block the open, and a link blocks nothing. And it is not a contrived case: the
    /// producer that exists today publishes in its work directory, so linking the contract file there
    /// is the obvious way to appear in the panel without moving.
    #[cfg(unix)]
    #[test]
    fn a_link_to_a_normal_file_is_read_as_the_file_it_points_to() {
        let producer_dir = PanelDir::create().expect("create the producer's directory");
        let dir = PanelDir::create().expect("create the split's directory");
        let published = producer_dir.path().join(crate::PANEL_FILE);
        std::fs::write(&published, r#"{"lines":["from the other side"]}"#).expect("publish");
        std::os::unix::fs::symlink(&published, dir.path().join(crate::PANEL_FILE))
            .expect("link the contract file");

        let panel = read(dir.path()).expect("a link to a normal file is read");

        assert_eq!(panel.lines[0].text, "from the other side");
    }

    /// **The whole chain of the age, end to end.** The two tests below build the panel by hand, so
    /// none of them ties down the decision that matters: that the age comes from the file's date.
    /// Without this one, forgetting to store it would leave the suite green and **no panel would ever
    /// be marked old**.
    #[test]
    fn the_age_comes_from_the_date_of_the_read_file() {
        let dir = PanelDir::create().expect("create the directory");
        let panel = panel_in(&dir, r#"{"stale_after_minutes":1,"lines":[]}"#).expect("read");

        let written = panel.written_at.expect("the file's date has to come set");

        assert!(!panel.is_stale_at(written), "just written cannot be stale");
        assert!(
            panel.is_stale_at(written + Duration::from_secs(120)),
            "two minutes of silence with a cap of one is stale"
        );
    }

    /// The six marks and the seven styles, **all of them**. With only the ones an example file carries,
    /// deleting the arm of three marks and five styles left the suite green.
    #[test]
    fn the_six_marks_and_the_seven_styles_are_read() {
        let dir = PanelDir::create().expect("create the directory");
        let panel = panel_in(
            &dir,
            r#"{"lines":[
                {"text":"a","mark":"done"},{"text":"b","mark":"current"},
                {"text":"c","mark":"pending"},{"text":"d","mark":"wait"},
                {"text":"e","mark":"block"},{"text":"f","mark":"info"},
                {"text":"g","style":"normal"},{"text":"h","style":"dim"},
                {"text":"i","style":"title"},{"text":"j","style":"accent"},
                {"text":"k","style":"ok"},{"text":"l","style":"warn"},
                {"text":"m","style":"error"}
            ]}"#,
        )
        .expect("read");

        let marks: Vec<Option<Mark>> = panel.lines[..6].iter().map(|l| l.mark).collect();
        assert_eq!(
            marks,
            [
                Some(Mark::Done),
                Some(Mark::Current),
                Some(Mark::Pending),
                Some(Mark::Wait),
                Some(Mark::Block),
                Some(Mark::Info)
            ]
        );

        let styles: Vec<Option<Style>> = panel.lines[6..].iter().map(|l| l.style).collect();
        assert_eq!(
            styles,
            [
                Some(Style::Normal),
                Some(Style::Dim),
                Some(Style::Title),
                Some(Style::Accent),
                Some(Style::Ok),
                Some(Style::Warn),
                Some(Style::Error)
            ]
        );
    }

    /// The three fields that are only stored for someone to use later. Without this, "it is not read"
    /// and "it comes empty" are indistinguishable.
    #[test]
    fn the_version_the_producers_timestamp_and_the_link_are_read() {
        let dir = PanelDir::create().expect("create the directory");
        let panel = panel_in(
            &dir,
            r#"{"v":1,"updated_at":"2026-08-06T18:12:00+02:00",
                "lines":[{"text":"t","link":"https://example.test/1","indent":3}]}"#,
        )
        .expect("read");

        assert_eq!(panel.version, Some(1));
        assert_eq!(
            panel.updated_at.as_deref(),
            Some("2026-08-06T18:12:00+02:00")
        );
        assert_eq!(
            panel.lines[0].link.as_deref(),
            Some("https://example.test/1")
        );
        assert_eq!(panel.lines[0].indent, 3, "a valid indentation is kept");
    }

    /// The two exact boundaries, which is where a `>` too many or too few does not show.
    #[test]
    fn right_at_the_ceiling_it_is_accepted_and_one_byte_more_is_not() {
        let dir = PanelDir::create().expect("create the directory");
        let overhead = r#"{"lines":["WIDTH"]}"#.len() - "WIDTH".len();
        let exact_fit = "x".repeat(MAX_BYTES as usize - overhead);

        let fits =
            panel_in(&dir, &format!(r#"{{"lines":["{exact_fit}"]}}"#)).expect("it just fits");
        assert_eq!(fits.lines.len(), 1);

        let failure =
            panel_in(&dir, &format!(r#"{{"lines":["{exact_fit}x"]}}"#)).expect_err("one more, no");
        assert!(matches!(failure, Problem::TooBig), "was {failure:?}");
    }

    #[test]
    fn silence_exactly_equal_to_the_cap_is_not_stale_yet() {
        let panel = Panel {
            stale_after: Duration::from_secs(60),
            written_at: Some(SystemTime::UNIX_EPOCH),
            ..empty_panel()
        };

        assert!(!panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(60)));
        assert!(panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(61)));
    }

    #[test]
    fn a_panel_silent_for_a_while_is_stale() {
        let panel = Panel {
            stale_after: Duration::from_secs(60),
            written_at: Some(SystemTime::UNIX_EPOCH),
            ..empty_panel()
        };

        assert!(panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(120)));
        assert!(!panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(30)));
    }

    /// A date in the future is not the rare case: a fast clock is enough. With the naive subtraction of
    /// times this would be a panic, and nothing that comes from outside can cause one.
    #[test]
    fn a_date_in_the_future_does_not_panic_nor_mark_stale() {
        let panel = Panel {
            stale_after: Duration::from_secs(60),
            written_at: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(7200)),
            ..empty_panel()
        };

        assert!(!panel.is_stale_at(SystemTime::UNIX_EPOCH + Duration::from_secs(60)));
    }

    #[test]
    fn without_knowing_when_it_was_written_there_is_no_staleness_to_measure() {
        assert!(!empty_panel().is_stale_at(SystemTime::now()));
    }

    fn empty_panel() -> Panel {
        Panel {
            version: None,
            updated_at: None,
            phase: None,
            header: true,
            stale_after: STALE_AFTER,
            attention: None,
            lines: Vec::new(),
            written_at: None,
        }
    }

    // ─── Counting what is reserved ──────────────────────────────────────────────────────────────
    //
    // It exists for a single test, and there was no cheaper way: the property "the whole file is not
    // read" **is not visible in the result** —the error is the same whether a quarter of a meg or ten
    // is reserved—, so either the memory is measured or it is not tied down. From outside the process
    // it is no good either: the resident maximum the system gives is that of the whole run, and here
    // dozens of tests run at once.
    //
    // **The count is per thread, on purpose.** The tests run in parallel, and a shared counter would
    // make what one reserves get blamed on another: the failure would be intermittent, which is the
    // worst kind of test. With one thread per test, each measures its own.

    /// What is live and the maximum it ever was, **in this thread**.
    ///
    /// They are initialized without reserving —hence the `const`—: a lazy value would reserve memory
    /// inside the allocator the first time it is looked at, which is a call to itself.
    mod counting {
        use std::cell::Cell;

        thread_local! {
            pub static LIVE: Cell<usize> = const { Cell::new(0) };
            pub static PEAK: Cell<usize> = const { Cell::new(0) };
        }
    }

    struct Counter;

    /// Adds to or subtracts from what is live and keeps the maximum.
    ///
    /// `try_with` and not `with`: when a thread is torn down, its thread-locals are gone, and there
    /// memory is still freed. With `with` that would be a panic inside the allocator.
    fn record(delta: isize) {
        let _ = counting::LIVE.try_with(|live| {
            let current = live.get().saturating_add_signed(delta);
            live.set(current);
            let _ = counting::PEAK.try_with(|peak| peak.set(peak.get().max(current)));
        });
    }

    unsafe impl std::alloc::GlobalAlloc for Counter {
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            let pointer = unsafe { std::alloc::System.alloc(layout) };
            if !pointer.is_null() {
                record(layout.size() as isize);
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: std::alloc::Layout) {
            record(-(layout.size() as isize));
            unsafe { std::alloc::System.dealloc(pointer, layout) }
        }

        unsafe fn realloc(
            &self,
            pointer: *mut u8,
            layout: std::alloc::Layout,
            new_size: usize,
        ) -> *mut u8 {
            let moved = unsafe { std::alloc::System.realloc(pointer, layout, new_size) };
            if !moved.is_null() {
                record(new_size as isize - layout.size() as isize);
            }
            moved
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counter = Counter;

    /// Runs the work and returns, besides what it produced, **how much extra it ever had live**.
    fn while_measuring<T>(work: impl FnOnce() -> T) -> (T, usize) {
        let baseline = counting::LIVE.get();
        counting::PEAK.set(baseline);

        let produced = work();

        (produced, counting::PEAK.get().saturating_sub(baseline))
    }
}
