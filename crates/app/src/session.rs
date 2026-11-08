//! The format in which the window layout goes to and comes back from disk, and what is required of it.
//!
//! **Nothing here opens a console**, and that sentence used to read «there is not a single console
//! here» — it narrowed on purpose with the delivery that brought the window's saving in. This module
//! describes shapes, validates them, and now also decides *when* what is on screen is written down;
//! materializing a shape —which ends in forking one process per leaf— still happens elsewhere:
//! [`crate::tree::Node::materialize`] walks the shape, and the `fork` itself is one call in
//! [`crate::Pane`]'s launching. **This sentence named only `crate::tree` and that sent the reader to
//! the wrong file**; the version it replaced said «at the root» and was right. The boundary is not a
//! matter of taste: it keeps this half **testable without opening a pseudoterminal**, and leaves the
//! `fork` in a single place.
//!
//! **The watcher did not come with the saving, and that is a decision rather than an omission.**
//! `watcher.rs` declares that it knows nothing of windows, and arming or re-arming one is **twenty
//! production lines** —forty-two with their docs, which is where «thirty-odd» came from— that would
//! earn no module and would cost that boundary. They stay in `lib.rs`, said here so
//! that nobody looks for them in the obvious place and concludes they were forgotten.
//!
//! **And the types here are a mirror, not those of the live layout.** Changing a field on the
//! [`crate::tree::Node`] tree must not change what is written on anyone's disk; the price is one
//! conversion each way, and what it buys is that the two shapes evolve separately. The only exception,
//! and for its reason, is the weight: see [`StoredChild::weight`]. **The conversions themselves live in
//! `crate::tree` and not here**, because the shape written down is the tree's own: this module owns
//! what the file looks like, and the tree owns how it becomes a tree.
//!
//! **The second half of the module does touch the disk**, and also without opening any console: where
//! the file lives ([`path_from`]), how it is read ([`read`]) and how it is replaced without being able
//! to leave it half done ([`Persistence::save`]). It is here and not in whoever starts up because the
//! file name, its ceiling and its version **are the format**: whoever looked at them on their own would
//! be writing them a second time.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How many leaves are accepted in a whole document.
///
/// **It exists even though D14 rejected putting a ceiling on the number of panes, and it does not
/// contradict it.** Splitting requires a human gesture and the geometry **refuses before creating
/// anything**; restoring forks N shells **before any geometry exists** —when opening, the window size is
/// unknown—, so the floor that protects splitting is **downstream of the `fork`**. It is a ceiling **on
/// the document**, not on panes.
///
/// **And the number is chosen against the resource it guards, not against what fits on screen.** Each
/// restored leaf is a process, a pseudoterminal, its descriptors and ~1.06 MiB of queues: a thousand
/// leaves —what the byte ceiling lets through— would be more than a gigabyte of queues before painting
/// anything, that is, the ceiling turned into the very disaster it came to avoid. 128 are about 136 MiB,
/// and are still above the order of a hundred consoles that fit in the window it is measured with.
pub const MAX_LEAVES: usize = 128;

/// The format version.
///
/// **It is mandatory, unlike the panel contract's, and for the opposite reason.** There it may be
/// missing because there are files older than the field, written by third parties; here none exists,
/// because this file is always written by the application.
///
/// It goes up **only with what an old reader would misinterpret** —a renamed key, a removed one, or one
/// with a different meaning—; **adding an optional key does not raise it**, and staying at 2 for a long
/// time is the sign that it works. It goes **once per document and never per node**: nested versions
/// would allow mixing shapes within the same file.
pub const VERSION: u64 = 2;

/// The largest thing that is accepted for interpretation.
///
/// **It is here and not in whoever reads the file, and that is the decision.** D16 commits to this
/// ceiling, and if it lived on the path that opens the file, whoever writes **another** path that
/// interprets a document would have to remember it on their own. In the module that owns the format
/// there is nothing to remember.
///
/// **64 KiB and not the panel contract's 256**: the largest legitimate document this format produces is
/// about 27 KiB —the leaf ceiling times what each one takes up—, so this is 2.4 times that. And the
/// reason there is a ceiling is not the interpretation peak: it is that **the file has an arbitrary
/// size, and reading it without a cap is allocating without a cap at startup**.
pub const MAX_BYTES: usize = 64 * 1024;

/// What is saved of a console, and it is everything that is saved of it.
///
/// Of the fifteen fields of a live pane, two. The other thirteen no, and each one for its reason: the
/// name because the counter that guarantees they do not repeat belongs to the window and an old name
/// would clash with it; the panel directory and its state because a restored console **gets a fresh
/// identity**; the pseudoterminal and its screen because they are a live process and a history; and the
/// timers because they belong to the run.
///
/// The optional label is display-only data; it never supplies a program, argument, environment,
/// working directory or path to the process launcher.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct StoredLeaf {
    /// In cells, as in the live pane: what the panel has to preserve when the desktop zoom changes is
    /// **how many characters fit**, not how many points it takes up.
    pub panel_width: usize,
    /// Separate from the width on purpose: folding does not lose the working width, and unfolding goes
    /// back **to the one it had** instead of inventing one.
    pub folded: bool,
    /// Optional user-provided label, never used to launch a program.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "read_label"
    )]
    pub label: Option<String>,
}

pub(crate) fn valid_label(label: &str) -> bool {
    !label.chars().any(char::is_control) && label.chars().count() <= 128
}

fn read_label<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let label = Option::<String>::deserialize(deserializer)?;
    if label.as_deref().is_some_and(|s| !valid_label(s)) {
        return Err(serde::de::Error::custom("invalid display label"));
    }
    Ok(label)
}

/// In which direction a branch divides.
///
/// **It is a type of its own and not [`crate::tiling::Orientation`], on purpose.** The names of this enum are
/// **the format**: they appear literally in the file of everyone who uses the application. Reusing the
/// live type would tie the file to a name that the code may want to change tomorrow for reasons that
/// have nothing to do with the disk. The price is four lines of conversion.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoredOrientation {
    /// Side by side; the width is divided.
    Columns,
    /// One above the other; the height is divided.
    Rows,
}

/// A console, or a cut with its parts.
///
/// **Explicit tag —`leaf` or `branch`— and not «deduced from which key is present».** The untagged
/// shape reads just as well and **loses the reason for the failure**: whoever tries to resolve the
/// either-or by trying shapes returns «fits none of them», and what needs to be said is **which** key is
/// extra or missing and **where**. The tag is written by `serde` out of the box, so the reason comes for
/// free.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoredNode {
    Leaf(StoredLeaf),
    Branch {
        orientation: StoredOrientation,
        children: Vec<StoredChild>,
    },
}

/// A child of a branch, with the share of the tiling that falls to it.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StoredChild {
    /// **A bare number, and not the live weight type, and this one is deliberate.** Deriving the reading
    /// over [`crate::tiling::Weight`] **would skip its constructor**, which is the only thing that makes a sum of
    /// weights equal to zero impossible — the `NaN` in a rectangle that that type exists to avoid. So the
    /// wire carries `f32` and the validation passes it through `crate::tiling::Weight::new`: **a single
    /// implementation of the invariant**, the one that was already tested.
    pub weight: f32,
    pub node: StoredNode,
}

/// A tab.
///
/// **An object with a single key instead of the bare node**, so that something optional can be added to
/// it tomorrow without raising the version. It carries neither the focus nor the name: the window opens
/// on the first tab, and names are not reused.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StoredTab {
    pub root: StoredNode,
    /// Optional display-only tab name.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "read_label"
    )]
    pub label: Option<String>,
}

/// The whole document, just as it is read from disk: **not validated yet**.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct StoredDocument {
    /// The format version. The key is short because it appears in every file and says nothing more than
    /// what it says.
    pub v: u64,
    pub tabs: Vec<StoredTab>,
}

/// A document that **has already gone through validation**, and can therefore be materialized.
///
/// **The field is private and there is no way to construct it other than [`validate`]**, so «this tree
/// is well formed» is a property of the type and not of someone remembering to check it beforehand. It
/// is the same doctrine as [`crate::tiling::Weight`] one level up, and for the same reason: this project has
/// counted with numbers what happens to invariants that depend on the memory of whoever writes.
#[derive(Clone, PartialEq, Debug)]
pub struct Validated {
    document: StoredDocument,
    /// **The count that validation made, stored instead of recomputed.** Walking the tree again to
    /// recount the same thing has two costs and only one is the work: the other is that **the number
    /// that was checked against the ceiling and the one the caller reads could stop being the same**.
    leaves: usize,
}

impl Validated {
    /// The tabs of the document, in order.
    pub fn tabs(&self) -> &[StoredTab] {
        &self.document.tabs
    }

    /// How many consoles will have to be opened. It is known **before** opening any.
    pub fn leaves(&self) -> usize {
        self.leaves
    }
}

/// Why a document is no good.
///
/// **Tolerance is per document and not per field, unlike in the panel contract.** That one is written
/// by an agent by hand and losing the whole panel over a field no one looks at would be the worst
/// possible thing; this one is always written by the application, so if it is not understood, **opening
/// factory-fresh is a correct and cheap answer** and guessing half a layout is worse than not restoring
/// it.
///
/// Each variant carries **what** and **where**, because the only channel to say it is one line.
///
/// **A single type for the file failures and the shape failures, and that is decided by the mold.** The
/// panel contract gathers its own the same way —«it's not there», «it's not a file», «it's not
/// understood»— in an enum (`crates/panel-contract/src/panel.rs:121`), and for the reason that here
/// counts double: whoever reads cannot do anything different with an absent file and with a corrupt one
/// other than **open factory-fresh**, so splitting them into two types would give two paths that end in
/// the same warning line. The byte ceiling was already in this enum before there was a file, which is
/// the same boundary.
#[derive(Clone, PartialEq, Debug)]
pub enum LayoutProblem {
    /// There is no file yet. **It is everyone's very first startup, so it is not warned about**: the
    /// distinction with «there is something and it is not understood» is exactly what makes the normal
    /// case quiet.
    Absent,
    /// There is something with that name and it is not a file.
    ///
    /// **It is checked before opening**, with the data that had to be requested anyway —it is [`read`]'s
    /// reserve hint—, so the branch is free. And what it prevents matters: a named pipe there **would
    /// block the opening**, and this happens before the window exists, so it would never open.
    ///
    /// **It covers the oversight, not a race**, just as in the mold: requesting the data and opening are
    /// two distinct resolutions of the same path, and between the two a substitution fits. Whoever can
    /// make it is the user themselves in their own configuration directory.
    NotAFile,
    /// It could not be opened or read.
    ///
    /// **It carries the reason as text and not as the system error**, so that this enum stays comparable
    /// with itself: the input/output error cannot be cloned or compared, and the failure-table tests
    /// assert over the whole problem.
    Unreadable(String),
    /// It was read, and the bytes are not text.
    NotText,
    /// It is not JSON, or it does not fit the shape. It carries the reason just as the interpreter gives
    /// it, which already names the key and the position.
    Unintelligible(String),
    /// The version is not the one this binary knows. **What to do with that is not decided here**: a
    /// newer one cannot be overwritten and an older one would have to be read with its reader, and both
    /// things belong to the path that touches the file.
    Version { found: u64 },
    /// A document with no tabs describes no window.
    NoTabs,
    /// A branch with fewer than two children. **The invariant that upholds the tree's reason for
    /// being**: if a branch can have a single child, back comes the nesting that makes a boundary drag a
    /// block of two consoles, which is what `tmux` is most criticized for.
    SingleChildBranch { tab: usize },
    /// Two branches of the same direction, one inside the other. The other half of the same invariant:
    /// splitting inserts as a sibling precisely so as not to build this.
    NestedBranches { tab: usize },
    /// A weight that the live weight type rejects —zero, negative, or not a number— or that falls
    /// outside the range the tiling can multiply.
    ///
    /// **It does not carry the value, and a review decided that.** Carrying it, this type compared badly
    /// with itself: a `NaN` weight stored here makes `InvalidWeight != InvalidWeight`, because `NaN` is
    /// not equal to anything, not even itself. The symptom was already in the tests —the weights one was
    /// **the only** one forced to check just the variant while its neighbors compared the whole
    /// problem—, so it stopped pinning down **which** weight was rejected. The value goes in the message,
    /// which is where whoever has to fix it reads it.
    InvalidWeight { tab: usize },
    /// The weights of a branch are each a number **and their sum is not**.
    ///
    /// **It is not «the weights have to sum to something specific», which was rejected on purpose**: they
    /// are proportions, and `{3, 1}` and `{0.5, 1.5}` describe the same tiling. It is that the
    /// division **divides each weight by the sum of its branch**, so with an infinite sum **all** of them
    /// get zero — two weights of 3·10³⁸ are each finite and positive, and summed they overflow. The other
    /// direction **does not need checking**: two positive numbers cannot sum to zero.
    ///
    /// **And it is the gate that upholds the whole format now that there is no dedicated weight range.**
    /// The sum of a branch is the only thing the gestures do **not** grow —dragging preserves it,
    /// splitting divides into two halves that sum to the same, closing shares the group's weight among
    /// its grandchildren—, so requiring it finite here is requiring exactly the invariant the live tree
    /// maintains. Not one condition more nor one less: that is what makes the round trip unable to lose a
    /// layout.
    NonFiniteSum { tab: usize },
    /// More consoles than are accepted to open at once.
    TooManyLeaves { count: usize },
    /// More bytes than are accepted for interpretation.
    TooLarge { bytes: u64 },
}

impl std::fmt::Display for LayoutProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absent => write!(f, "there is no saved layout"),
            Self::NotAFile => write!(f, "there is something there that is not a file"),
            Self::Unreadable(reason) => write!(f, "could not be read ({reason})"),
            Self::NotText => write!(f, "it is not UTF-8 text"),
            Self::Unintelligible(reason) => write!(f, "not understood: {reason}"),
            Self::Version { found } => {
                write!(
                    f,
                    "it was written by version {found} of the format and this one knows {VERSION}"
                )
            }
            Self::NoTabs => write!(f, "it carries no tab"),
            Self::SingleChildBranch { tab } => {
                write!(f, "tab {tab} has a cut with a single child")
            }
            Self::NestedBranches { tab } => write!(
                f,
                "tab {tab} nests two cuts of the same direction, which is the shape that makes \
                 a divider drag two consoles"
            ),
            Self::InvalidWeight { tab } => write!(
                f,
                "tab {tab} carries a weight that cannot be one: neither zero, nor negative, nor infinite"
            ),
            Self::NonFiniteSum { tab } => write!(
                f,
                "tab {tab} has a cut whose weights are numbers and their sum is not: dividing \
                 would leave all its consoles with zero width"
            ),
            Self::TooManyLeaves { count } => write!(
                f,
                "it asks for {count} consoles and no more than {MAX_LEAVES} are opened at once"
            ),
            Self::TooLarge { bytes } => write!(
                f,
                "it takes up {bytes} bytes and no more than {MAX_BYTES} are interpreted"
            ),
        }
    }
}

/// The only thing required of a document **before** requiring its shape.
///
/// **It exists because looking at the version after fitting the shape is having no version.** The
/// version goes up exactly with what an old reader would misinterpret —a renamed key, a removed one, or
/// one with a different meaning—, that is, **precisely with what makes the shape interpreter fail**.
/// Reading it afterwards, a version 2 file with a new key is indistinguishable from one of ours that is
/// broken, and the saving path overwrites it: an old binary running over the new one's layout, which is
/// the exact damage the version exists to avoid.
#[derive(Deserialize)]
struct StoredProbe {
    v: u64,
}

/// What a document says, if it says something that can be constructed.
pub fn interpret(text: &str) -> Result<Validated, LayoutProblem> {
    // **The ceiling is applied BEFORE interpreting, not after.** Interpreting allocates the whole tree,
    // so checking the size at the end is having no ceiling: the cost already happened.
    if text.len() > MAX_BYTES {
        return Err(LayoutProblem::TooLarge {
            bytes: text.len() as u64,
        });
    }

    // Two interpreter passes on purpose, and the first is over a one-field mold: it costs what walking
    // the text once costs —once at startup— and it is what makes the version rule over the shape instead
    // of the other way around. That `v` is missing or not a number comes through here as «not
    // understood», which is what corresponds: a file with no version is not of a version we do not know,
    // it is a file of ours that is broken, and it gets overwritten.
    let sonda: StoredProbe = serde_json::from_str(text)
        .map_err(|error| LayoutProblem::Unintelligible(error.to_string()))?;
    if sonda.v != VERSION {
        return Err(LayoutProblem::Version { found: sonda.v });
    }

    let document: StoredDocument = serde_json::from_str(text)
        .map_err(|error| LayoutProblem::Unintelligible(error.to_string()))?;
    validate(document)
}

/// What has to be written to read this back, or **nothing** if it could not be done.
///
/// **It returns `Option` and not a string, and a review decided that.** The first version returned an
/// empty string on failure, and the path that writes the file **would not have been able to distinguish
/// it from a document**: it writes a temporary and renames it over the good one, so a failure here would
/// have **destroyed the saved layout** and left zero bytes in its place. Opening factory-fresh is the
/// answer to a file that is not understood; **deleting the one that was understood is no answer to
/// anything.** With `None`, whoever writes skips the rename and the previous file survives.
pub fn serialize(document: &StoredDocument) -> Option<String> {
    // Unindented: no one edits it by hand —it is always written by the application— and one line per
    // console makes the file several times larger without anyone being grateful for it.
    match serde_json::to_string(document) {
        Ok(text) => Some(text),
        Err(error) => {
            // Unreachable with the types here —there are no keys that are not text nor numbers that
            // cannot be ones—, so it is said and not kept quiet: a silent failure on the saving path is
            // the one that is discovered when reopening and finding nothing.
            eprintln!("could not write the layout ({error})");
            None
        }
    }
}

/// The only path that produces a [`Validated`].
///
/// **The whole tree is walked before accepting anything**, and in particular the leaves are counted
/// **before** the first one is opened: checking the ceiling after the `fork` is having no ceiling.
pub fn validate(document: StoredDocument) -> Result<Validated, LayoutProblem> {
    if document.v != VERSION {
        return Err(LayoutProblem::Version { found: document.v });
    }
    if document.tabs.is_empty() {
        return Err(LayoutProblem::NoTabs);
    }

    // **A single walk, that validates and counts.** Two —one to review and another to count— visit each
    // node twice on the startup path, and moreover open the door to the number that is compared against
    // the ceiling not being the one that is later read.
    let mut leaves = 0;
    for (which, tab) in document.tabs.iter().enumerate() {
        leaves += check(&tab.root, None, which)?;
    }

    if leaves > MAX_LEAVES {
        return Err(LayoutProblem::TooManyLeaves { count: leaves });
    }

    Ok(Validated { document, leaves })
}

/// The shape invariants, branch by branch.
///
/// `from_outside` is the direction of the branch that contains this one, or nothing at the root: it is
/// the only thing needed to catch the nesting, because splitting can only create it between a branch and
/// its direct parent.
fn check(
    node: &StoredNode,
    from_outside: Option<StoredOrientation>,
    tab: usize,
) -> Result<usize, LayoutProblem> {
    let StoredNode::Branch {
        orientation,
        children,
    } = node
    else {
        return Ok(1);
    };

    if children.len() < 2 {
        return Err(LayoutProblem::SingleChildBranch { tab });
    }
    if from_outside == Some(*orientation) {
        return Err(LayoutProblem::NestedBranches { tab });
    }

    let mut leaves = 0;
    for child in children {
        // **A single gate per weight, the live type's, and none of our own.** It is the only way for
        // there not to be two implementations of the same invariant —the one over there is already
        // tested— and, above all, for **what the tree can hold to be readable back**: there was a
        // dedicated range, `[1e-6, 1e6]`, and both ends rejected states that are reached by **dragging**.
        // At the bottom, because dragging trims the *fraction* and not the weight; at the top, because
        // what it preserves is the **sum of the pair**, so a weight rises up to 99 % of what two
        // neighbors summed. What the tiling really needs is not a bounded weight: it is **a finite
        // branch sum**, and that is checked below.
        if crate::tiling::Weight::new(child.weight).is_none() {
            return Err(LayoutProblem::InvalidWeight { tab });
        }
        leaves += check(&child.node, Some(*orientation), tab)?;
    }

    // **And the sum, which is an invariant of the branch and not of each weight.** With the range above
    // this can no longer overflow with two children, but it is kept: the range may be widened tomorrow,
    // and it costs one sum.
    let sum: f32 = children.iter().map(|child| child.weight).sum();
    if !sum.is_finite() {
        return Err(LayoutProblem::NonFiniteSum { tab });
    }

    Ok(leaves)
}

// ── The file: where it lives, how it is read and how it is replaced ──────────────────────────────

/// The dedicated directory inside the user's configuration one.
pub const DIRECTORY: &str = "agent-terminal";

/// The file name, **fixed**.
///
/// That it depends on nothing is what makes the layout one, global, as the ticket decided. And it is
/// also where the accepted limit comes from: two open instances write the same destination and **the
/// last one to save takes it**.
pub const FILE: &str = "layout.json";

/// Where the layout lives, reading the environment.
///
/// **The one-line wrapper, and the work in [`path_from`]**, which is the pattern that startup already
/// follows five times: `std::env::set_var` has a contract over the whole process and this suite runs in
/// parallel launching processes, so a test that wanted to set these variables would be undefined
/// behavior.
pub fn path() -> Option<PathBuf> {
    path_from(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

/// The decision, given the two values: **the file, or nothing if there is no trustworthy place**.
///
/// **Nothing means it is not persisted: it is neither read nor written, and it does not fall back to a
/// temporary directory.** The fallback exists in the sibling crate
/// (`crates/panel-contract/src/lib.rs:186`) and there it is correct because that directory **must** be
/// emptied when the session closes; here it would be restoring from a file the system deletes, that is,
/// a machine of silent failures.
///
/// **And a variable that is set and empty is not a path.** It is the trap already paid for in the
/// sibling crate: letting it through gives a relative path and the directory is created inside the
/// working directory **without failing**, which with a fixed file name ends up inside any repository
/// someone clones. Here it is caught by the same check that rejects the relative case, because the empty
/// string is not absolute either.
pub fn path_from(config: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    Some(directory_from(config, home)?.join(FILE))
}

/// The dedicated directory inside the configuration one, given the two values.
///
/// **It comes out separately because this project saves more than one file there.** The layout is
/// written by the application; the settings are written by whoever uses the terminal. They share the
/// place and the decision of what place it is, which is exactly what this function encloses; they share
/// nothing else.
pub fn directory_from(config: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    // The order is the desktop standard's: the dedicated variable rules, and where there is none the
    // home's `.config` is used. That one of the two is no good **does not rule out the other**: what
    // rules out is that neither is any good.
    let base = absolute(config).or_else(|| absolute(home).map(|home| home.join(".config")))?;
    Some(base.join(DIRECTORY))
}

/// The path a variable carries, if it is trustworthy.
fn absolute(variable: Option<&OsStr>) -> Option<PathBuf> {
    let path = PathBuf::from(variable?);
    path.is_absolute().then_some(path)
}

/// Reads the layout saved at that path.
///
/// **The order is the mold's and it is no accident** (`crates/panel-contract/src/panel.rs:199`):
/// request the file's data, check that it is a file, open, read capped, apply the ceiling **on bytes**
/// and only then look at whether it is text.
pub fn read(path: &Path) -> Result<Validated, LayoutProblem> {
    let data = match std::fs::metadata(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(LayoutProblem::Absent);
        }
        Err(error) => return Err(LayoutProblem::Unreadable(error.to_string())),
    };
    if !data.is_file() {
        return Err(LayoutProblem::NotAFile);
    }

    let file =
        std::fs::File::open(path).map_err(|error| LayoutProblem::Unreadable(error.to_string()))?;
    let bytes = read_capped(file, data.len())
        .map_err(|error| LayoutProblem::Unreadable(error.to_string()))?;

    // **The ceiling is applied here and not only inside [`interpret`], and that apparent duplication is
    // the fix of a named bug in the mold.** The read cuts one byte above the ceiling, and that cut
    // **splits a multi-byte character**: without this check, a huge file with an accent in just the
    // right spot would come out as «not text» instead of «over the ceiling», and the ceiling path would
    // almost never be reached. The number is the same as the format's; what is decided twice is *when*.
    //
    // **What DECIDES is what was read, and the number that is said is the larger of the two.** Deciding
    // with the disk's datum would be trusting a figure that may already be stale —the file is replaced by
    // rename, and between requesting the data and reading it may have changed— and moreover it truncates
    // on 32-bit targets; what was read is bounded by definition and can neither lie nor overflow. For the
    // message, in contrast, what was read falls short by construction: it would say «takes up 65537» of a
    // one-megabyte file.
    if bytes.len() > MAX_BYTES {
        return Err(LayoutProblem::TooLarge {
            bytes: data.len().max(bytes.len() as u64),
        });
    }

    let text = String::from_utf8(bytes).map_err(|_| LayoutProblem::NotText)?;
    interpret(&text)
}

/// Reads at most one byte more than the ceiling.
///
/// The extra byte is what distinguishes «takes up exactly the ceiling» from «goes over». And `expected`
/// is a **reserve hint and not a check**: no one decides anything with that number, it just keeps the
/// read from growing by doubling from thirty-two bytes. The ceiling is still set by the cut.
pub(crate) fn read_capped(file: std::fs::File, expected: u64) -> std::io::Result<Vec<u8>> {
    let ceiling = MAX_BYTES as u64;
    let mut bytes = Vec::with_capacity(expected.min(ceiling) as usize + 1);
    file.take(ceiling + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Whether a problem on loading deserves a line.
///
/// **The only exception is the normal case**: that there is no file is everyone's very first startup,
/// and warning about that would put an error line on every user's first time. It is separate from
/// whoever prints it **because the decision is the only thing that can be asserted**: writing to the
/// error output cannot be observed from inside the process, so a test about the warning can only look at
/// this.
fn warrants_warning(problem: &LayoutProblem) -> bool {
    !matches!(problem, LayoutProblem::Absent)
}

/// Why the layout was not saved.
///
/// **Each variant is a distinct reason, and that has a visible consequence**: the warning is suppressed
/// **per reason** ([`Persistence::save`]), so two failures the user cannot distinguish must not be two
/// variants, and two that call for a different fix —a directory that cannot be created and a rename that
/// cannot be done— must not be one.
#[derive(Debug)]
pub(crate) enum SaveFailure {
    /// There is no trustworthy place to put it: see [`path_from`]. **The loading path also uses it**,
    /// with its own verb, because the cause is the same and the user does not have two things to fix.
    NoPlace,
    /// On disk there is a layout of a **newer** version, and it is not overwritten.
    Vetoed { found: u64 },
    /// The document could not be described. **Unreachable with these types**, and even so a variant and
    /// not an `unwrap`: what is on disk is what the user placed by hand.
    NotSerializable,
    /// The directory could not be created.
    Directory(std::io::Error),
    /// The temporary could not be created **exclusively**. That it already exists comes through here,
    /// and it is what prevents writing through whatever someone else has planted with that name.
    TempFile(std::io::Error),
    /// The temporary could not be written.
    Writing(std::io::Error),
    /// The temporary could not be renamed over the destination. **The destination stays as it was.**
    Renaming(std::io::Error),
    /// This session **must not overwrite** what is on disk, and it carries why.
    ///
    /// **It exists because not all reasons for not writing belong to the file.** Two, today: that the
    /// application **could not open** the saved layout —then what is on screen is a factory-fresh window,
    /// and writing it would run over the user's **forever**— and that a measurement batch requested it,
    /// which opens what the batch says and not what the user left.
    WontOverwrite(&'static str),
    /// What is on screen **does not fit the format**: today, more consoles than are accepted to open at
    /// once.
    ///
    /// **A variant of its own and not shared with another reason**, which is what this enum's contract
    /// demands: the warning is suppressed **per reason**, so two things fixed in a different way —closing
    /// consoles, and the disk coming back— cannot spend the same slot.
    DoesntFit,
}

impl std::fmt::Display for SaveFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPlace => write!(
                f,
                "there is nowhere to put it: neither $XDG_CONFIG_HOME nor $HOME give an absolute path"
            ),
            Self::Vetoed { found } => write!(
                f,
                "on disk there is a layout of version {found} of the format and this one knows \
                 {VERSION}, so it is not overwritten"
            ),
            Self::NotSerializable => write!(f, "the layout could not be described"),
            Self::Directory(error) => write!(f, "the directory could not be created ({error})"),
            Self::TempFile(error) => write!(f, "the temporary file could not be created ({error})"),
            Self::Writing(error) => {
                write!(f, "the temporary file could not be written ({error})")
            }
            Self::Renaming(error) => {
                write!(f, "the file could not be put in its place ({error})")
            }
            Self::WontOverwrite(reason) => write!(f, "what is saved is not overwritten: {reason}"),
            Self::DoesntFit => write!(f, "the current layout does not fit the format"),
        }
    }
}

impl SaveFailure {
    /// Whether nothing this session does can make the next attempt succeed.
    ///
    /// **Three reasons, and each is fixed only by restarting**: the veto and the prohibition are set
    /// once and never lifted, and the path is resolved when the session starts. The disk's four are
    /// the opposite — a full disk empties, a planted temporary is dodged by the next attempt's name —
    /// and so is `NotSerializable`, which belongs to the document and not to the session. Classifying a transient
    /// reason here would stop saving for good over a disk that was busy for a second.
    fn lasts_the_session(&self) -> bool {
        matches!(
            self,
            Self::Vetoed { .. } | Self::WontOverwrite(_) | Self::NoPlace
        )
    }
}

/// How long is waited from the first change without writing until the layout is written.
///
/// **Half a second, and the number has two reasons.** It is the same order as the breather of the
/// watcher's degraded mode, which is already in the code and for the same motive: it is what is
/// barely noticed and does not keep the window awake. And with it, **a continuous drag writes twice
/// per second instead of sixty**, while what is lost on dying —half a second of placing— is not
/// missed.
///
/// **It is a cap counted from the first change, not a postponement.** The difference decides
/// whether the milestone works: rearming the deadline on every change, a ten-second drag would not
/// write even once.
///
/// **It lives beside the deadline it arms, and it used to live in the crate root.** The delivery that
/// brought the deciding of *when* to write into this module left the number that says *when* on the
/// other side and imported it back for its single use; a seam that makes a module reach into the
/// crate root for its own constant is the seam being one item short. Its reasoning travelled with it
/// rather than being rewritten — the two paragraphs above are the ones that were always here.
///
/// **`pub(crate)` and never `pub`**: nine tests in `lib.rs`'s test module assert against this exact
/// number, and that is the reach it already had as a crate-root `const` — nothing outside the crate
/// can see it either way.
pub(crate) const SAVE_DELAY: Duration = Duration::from_millis(500);

/// The saving policy **of a session**: where it goes, whether it can be written, and what has already
/// been warned about.
///
/// **It is not a file and that is why it is not named one.** It has no descriptor, opens nothing and
/// survives every read and write; what it holds is what lasts **until the application closes**: the veto
/// of a newer version, the reasons already warned about, the count of attempts, the deadline of the
/// change still pending and the last document that reached the disk. With that data in
/// the parameters of a free function, the caller would have to carry it —and that is exactly the kind of
/// invariant this module refuses to leave in the memory of whoever writes.
///
/// **And hence the rule for whoever uses it: the one that loads at startup and the one that saves
/// afterwards have to be THE SAME.** Creating a second one mid-session **loses the veto**, and losing
/// the veto means overwriting the layout of a newer version, which is the exact damage the format
/// version exists to avoid. It also drops the change still waiting for its deadline.
pub struct Persistence {
    path: Option<PathBuf>,
    /// The version found on disk that prevents overwriting it.
    ///
    /// **It is set on reading and never removed.** An old binary opened over a new one's layout cannot
    /// restore it —it does not know its shape— nor should it clobber it: having two versions installed
    /// on the same machine is not rare, and the damage would be silent and in the most noticeable place.
    veto: Option<u64>,
    /// Why this session must not write, if there is a reason. See [`SaveFailure::WontOverwrite`].
    ///
    /// **Separate from the version veto on purpose**: that one carries a number and comes from what was
    /// on disk; this one comes from what happened to this session. Joining them into one field would
    /// force inventing a number for the second case.
    no_overwrite: Option<&'static str>,
    /// Which reasons have already been warned about. A full disk would print one line per missed
    /// deadline, that is, two per second while the window is in use.
    warned: HashSet<std::mem::Discriminant<SaveFailure>>,
    /// How many times a save has been attempted. **It goes in the temporary's name, and a review asked
    /// for that.**
    ///
    /// With the name depending only on the process number, a temporary that survived a violent death
    /// —between creating it and renaming it— would leave **every future session unable to save** that
    /// the system gave that same number, because exclusive creation would always fail and adopting what
    /// is there is rightly forbidden. Counting the attempts, a planted temporary hinders **one attempt**
    /// and not a session.
    attempts: usize,
    /// How long [`Self::load`] took the last time it was called, if it was called.
    ///
    /// **It lives here and is not returned by `load`, and it is what avoids touching six signatures to
    /// publish one number.** Whoever publishes it is the measurement report, which already receives this
    /// same `Persistence` for another reason —the version veto stays with the window—, so the datum
    /// travels with whoever already traveled. Returning it in the `Option` would have forced changing the
    /// type the window opens with and the six tests that build it by hand.
    load_time: Option<Duration>,
    /// Whether an attempt already failed for a reason that [lasts the
    /// session](SaveFailure::lasts_the_session).
    ///
    /// **Learned from an attempt and not from the fields above, on purpose**: the veto, the
    /// prohibition and the missing path are all known before anything is tried, but deciding from
    /// them would skip the first attempt too — and that attempt is the one that says the line, and
    /// the one the measurement script demands to see in a batch, where writing is always forbidden.
    refused_for_good: bool,
    /// The last thing that reached the disk **for real**. It is what decides whether there is
    /// anything to write, so a failure **is retried with the next change** without saving the error
    /// or inventing any wait.
    pub(crate) last_written: Option<StoredDocument>,
    /// When the deadline falls due, counted from the **first** unwritten change.
    ///
    /// **A cap and not a postponement**, and the difference is what makes the milestone work:
    /// rearming on every change, a ten-second drag would not write **even once** —every frame would
    /// push the deadline along—, and what the ticket promises is to lose half a second at most.
    ///
    /// **`pub(crate)` for the alarm that wakes the window** (`alarms.rs`) **and for the saving
    /// tests**, which expire it by hand instead of waiting half a second.
    pub(crate) save_due: Option<Instant>,
}

impl Persistence {
    /// The file the environment says.
    pub fn from_env() -> Self {
        Self::at(path())
    }

    /// The one at a given path, or the one at none. **Nothing here touches the disk yet.**
    pub fn at(path: Option<PathBuf>) -> Self {
        Self {
            path,
            veto: None,
            no_overwrite: None,
            warned: HashSet::new(),
            attempts: 0,
            load_time: None,
            refused_for_good: false,
            last_written: None,
            save_due: None,
        }
    }

    /// Leaves it stated that this session **cannot overwrite** what is saved, and why.
    ///
    /// **It is what prevents the data loss a review uncovered.** If the application read the file well
    /// and **could not open it** —an exhausted descriptor, a pseudoterminal that is not granted—, what is
    /// left on screen is a factory-fresh window; without this, the user's first change would write over
    /// it and their twenty-console layout would disappear **forever**, whereas before saving
    /// automatically the file survived and the next startup recovered it.
    pub fn forbid_overwrite(&mut self, reason: &'static str) {
        self.no_overwrite = Some(reason);
    }

    /// The saved layout, if there is one that is understood.
    ///
    /// **Everything that fails returns nothing and lets startup proceed**, which is this format's
    /// per-document tolerance: guessing half a layout is worse than not restoring it. Everything is
    /// warned about except that there is no file.
    pub fn load(&mut self) -> Option<Validated> {
        let start = std::time::Instant::now();
        let loaded = self.loading();
        self.load_time = Some(start.elapsed());
        loaded
    }

    /// How long the last load took, for the measurement report to declare. **Nothing if no one called
    /// it.**
    ///
    /// **And that distinction is what prevents publishing an invented number**, which was the gap the
    /// first version left: returning a bare `Duration`, a window built with a persistence that never
    /// touched the disk —what the tests do, and what a new path could do— published `0ns reading and
    /// validating`, indistinguishable from a genuinely instantaneous read. With `None` the report says
    /// «not measured», which is what happened.
    pub fn load_time(&self) -> Option<Duration> {
        self.load_time
    }

    fn loading(&mut self) -> Option<Validated> {
        let Some(path) = self.path.clone() else {
            // **With its own verb, and without touching the list of reasons already warned about, and a
            // review asked for both things.** Saying «it was not saved» before having attempted to save
            // anything is a message that does not correspond to its path; and spending the reason here
            // would leave **mute** the first genuine save failure, which is the one the user needs to
            // see.
            eprintln!("the layout was not restored: {}", SaveFailure::NoPlace);
            return None;
        };

        match read(&path) {
            Ok(validated) => Some(validated),
            Err(problem) if !warrants_warning(&problem) => None,
            Err(problem) => {
                // **A version that is not ours is not a broken file, and the rule depends on which side
                // it is not by.** Newer: it was written by a version that knows something this one does
                // not, so writing is vetoed for the whole session. Older **and known**: it would be read
                // with that version's reader and rewritten in the new one on the first save —today only
                // 1 exists, so there is no case, but withdrawing support for an old version is a separate
                // decision and it is written down—. And what is neither of the two, like a zero, is a
                // file of ours that is broken: factory-fresh, and it is overwritten.
                if let LayoutProblem::Version { found } = problem
                    && found > VERSION
                {
                    self.veto = Some(found);
                }
                eprintln!("the layout was not restored: {problem}");
                None
            }
        }
    }

    /// Leaves that layout on disk, or says why it did not.
    ///
    /// **It receives the already-validated one and not just any document, and a review asked for that.**
    /// Without it, the writing path accepted exactly what the reading path would reject: a file could be
    /// left on disk that on reopening is discarded, and the user would lose their layout **without
    /// anything saying so** —the failure would appear one startup later and with no visible cause—.
    /// Passing through the same type the read demands, what is written can be read back **by
    /// construction**.
    ///
    /// **Each failure is warned about once per reason, and the retry is suppressed only when it cannot
    /// succeed**: the caller tries again with the next change, so a disk that empties recovers on its
    /// own without having filled the error output while it was full; a reason that lasts the session
    /// is remembered, and [`Self::will_never_write`] tells the caller to stop arming deadlines.
    pub(crate) fn save(&mut self, layout: &Validated) -> Result<(), SaveFailure> {
        self.attempts = self.attempts.wrapping_add(1);
        let attempt = if let Some(found) = self.veto {
            Err(SaveFailure::Vetoed { found })
        } else if let Some(reason) = self.no_overwrite {
            Err(SaveFailure::WontOverwrite(reason))
        } else if let Some(path) = &self.path {
            save_to(path, self.attempts, &layout.document)
        } else {
            Err(SaveFailure::NoPlace)
        };

        if let Err(error) = &attempt {
            self.refused_for_good |= error.lasts_the_session();
            self.warn(error);
        }
        attempt
    }

    /// Whether a save already failed for a reason nothing this session does can change.
    pub(crate) fn will_never_write(&self) -> bool {
        self.refused_for_good
    }

    /// Warns that what is on screen **cannot be saved**, once per session.
    ///
    /// **It exists because the format has a ceiling that the window does not.** No more than
    /// [`MAX_LEAVES`] consoles are accepted in a document —the ceiling bounds the startup `fork`, so it
    /// cannot be removed—, and yet nothing prevents opening more tabs than fit there. Without this
    /// warning, whoever reaches that number would lose their layout **with no signal at all**: saving
    /// would stop and they would notice it on reopening, a day later, with no visible cause.
    fn warn_cannot_save(&mut self, problem: &LayoutProblem) {
        if self.should_warn(&SaveFailure::DoesntFit) {
            eprintln!("the current layout cannot be saved: {problem}");
        }
    }

    /// Says the line, if that reason has not been said already.
    fn warn(&mut self, error: &SaveFailure) {
        if self.should_warn(error) {
            eprintln!("the layout was not saved: {error}");
        }
    }

    /// The decision, apart from saying the line: **it is the only thing with content to assert**,
    /// because writing to the error output cannot be observed from inside the process.
    fn should_warn(&mut self, error: &SaveFailure) -> bool {
        self.warned.insert(std::mem::discriminant(error))
    }

    /// Whether that reason has already been said, for tests.
    #[cfg(test)]
    pub(crate) fn warned_about(&self, error: &SaveFailure) -> bool {
        self.warned.contains(&std::mem::discriminant(error))
    }
}

/// Writes the document at that path **replacing it in one go**.
fn save_to(path: &Path, attempt: usize, document: &StoredDocument) -> Result<(), SaveFailure> {
    save_described(path, attempt, serialize(document))
}

/// The write, already receiving the text **or the lack of it**.
///
/// **That it receives `Option` is what makes this path's central promise checkable**: with no text
/// nothing is renamed, so what was already saved survives. With the call inside, the only way to reach
/// that case would be a failure these types cannot produce, and the promise would be left written in a
/// comment.
///
/// **No `fsync`, and it is a decision with its limit.** With the system running, «half a file is never
/// seen» is given by the rename alone. After a power cut a truncated destination **can** be left,
/// depending on the file system: that comes out on reopening as a file that is not understood, that is,
/// opening factory-fresh, which is what is already accepted for the last missed deadline. What the
/// rename guarantees is that **no one sees it half done while the machine works**; it is not a journal.
fn save_described(
    path: &Path,
    attempt: usize,
    described: Option<String>,
) -> Result<(), SaveFailure> {
    let text = described.ok_or(SaveFailure::NotSerializable)?;
    // With no directory, or no file name, there is nowhere to put anything. It is only reached with a
    // path that did not come out of `path_from` —the root, for example—, and the reason is the same as
    // not having a configuration directory: there is nowhere. Hence the variant is one and its line
    // covers them both.
    let directory = path.parent().ok_or(SaveFailure::NoPlace)?;
    create_directory(directory).map_err(SaveFailure::Directory)?;

    let temp_path = temp_path_for(path, attempt).ok_or(SaveFailure::NoPlace)?;
    let mut file = create_exclusive(&temp_path).map_err(SaveFailure::TempFile)?;

    // From here on there is an extra file in the directory, and **every exit has to take it away**.
    // Leaving it does not break the next save —the name carries the attempt count—, but one would
    // accumulate per failure in the user's configuration directory, which is unbounded garbage.
    if let Err(error) = file.write_all(text.as_bytes()) {
        remove(&temp_path);
        return Err(SaveFailure::Writing(error));
    }
    // Closed before renaming. There is nothing to flush —this writes directly, without intermediate
    // buffering—, so closing is just returning the descriptor.
    drop(file);

    if let Err(error) = std::fs::rename(&temp_path, path) {
        remove(&temp_path);
        return Err(SaveFailure::Renaming(error));
    }
    Ok(())
}

/// The temporary next to the destination, with this process's number and the attempt's.
///
/// **In the same directory and not in the system's temporary one**, because that would in all
/// likelihood be another file system, the rename would fail for crossing them, and the fallback —copy
/// and truncate— would no longer be atomic. The temptation is real and it is in the repo: the sibling
/// crate falls there on purpose.
///
/// **The process number because nothing prevents two open instances**: with a fixed name both would
/// write the same temporary, the bytes would interleave and a corrupt document would be renamed over the
/// good one. **And the attempt's because the process's is recycled**: a temporary that survives a
/// violent death is cleaned by no one, and with the name depending only on the process it would leave,
/// forever, every session the system gave that number unable to save.
fn temp_path_for(path: &Path, attempt: usize) -> Option<PathBuf> {
    let mut name = path.file_name()?.to_os_string();
    name.push(format!(".{}.{attempt}", std::process::id()));
    Some(path.with_file_name(name))
}

/// Creates the directory if it is not there, **only for the user**.
///
/// Without the mode, the permission is decided by the session's mask: measured on this machine at
/// `0002`, a directory would come out `0775` —readable and traversable by everyone— and this file tells
/// how the user works: what they had open and how they had it laid out.
///
/// **With intermediate segments and without complaining if it already exists**, and both things are
/// needed: the configuration directory may not exist on a freshly made account, and from the second save
/// on ours is already there. The mode is only applied to what **this** call creates; a directory that
/// was already there keeps its permissions, and that is why the only thing protecting the temporary is
/// creating it exclusively.
fn create_directory(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

/// Creates the temporary **and fails if it already exists**.
///
/// The name is predictable, so without this a link planted there would be opened and **written
/// through**: the destination is protected by never being opened for writing, and the temporary only by
/// this.
fn create_exclusive(path: &Path) -> std::io::Result<std::fs::File> {
    let mut builder = std::fs::OpenOptions::new();
    builder.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        builder.mode(0o600);
    }
    builder.open(path)
}

/// Takes the temporary away, and keeps quiet if it cannot.
///
/// What is being reported is the earlier failure; a second warning for not having been able to clean up
/// would say two things of which only one can be fixed.
fn remove(temp_path: &Path) {
    let _ = std::fs::remove_file(temp_path);
}

/// The window's own saving, which is the other half of this format.
///
/// **It came here rather than to a module of its own** because these three methods say nothing that
/// is not already this file's vocabulary: when the layout has moved, when the deadline is due, and
/// how the file is replaced without ever being left half written. A `saving.rs` beside a `session.rs`
/// would be two names for one subject, and **D28** asks for a ceiling, not for a module per method.
///
/// **They are `Window`'s and they stay `Window`'s.** What crosses the boundary is only that this file
/// now holds them; the tree they walk is [`crate::tree`]'s and the shapes they write are the ones
/// declared above.
impl crate::Window {
    /// Builds the mirror of the layout and **arms the deadline if anything changed**.
    ///
    /// **What it costs is timed and declared in both places**: it goes into the accumulator the
    /// window subtracts —or its work would be published as framework cost, the figure D3 decides
    /// with— and it also has a series of its own in the report, because that accumulator
    /// **subtracts but publishes no row at all**. They are two mechanisms and both are needed.
    pub(crate) fn check_whether_the_layout_moved(&mut self) {
        let started = Instant::now();

        let current = self.describe();
        if self.last_seen.as_ref() != Some(&current) {
            self.last_seen = Some(current);
            // **Only if it was empty**, which is the difference between a cap and a postponement:
            // during a drag the layout changes on every frame, and by rearming here it would never
            // be written. **And never for a session that can no longer write**: a deadline it arms
            // wakes the window for an attempt that cannot succeed, and the copy, comparison and
            // validation that attempt pays.
            if self.persistence.save_due.is_none() && !self.persistence.will_never_write() {
                self.persistence.save_due = Some(Instant::now() + SAVE_DELAY);
            }
        }

        self.mirror_this_frame += started.elapsed();
    }

    /// Writes the layout if the deadline fell due and there is something different from the last
    /// thing that reached the disk.
    ///
    /// **It is called in the logic half and AFTER closing the frame**, and the order matters:
    /// closing empties what the window timed separately, so writing before would charge its cost to
    /// the previous frame — which is exactly the kind of error this instrument is after.
    pub(crate) fn save_if_due(&mut self) {
        let Some(due) = self.persistence.save_due else {
            return;
        };
        if Instant::now() < due {
            return;
        }
        self.save_now();
    }

    /// Writes what is pending **without looking at the clock**.
    ///
    /// **It exists for the clean closing of the window, and a review asked for it.** The delay
    /// accepts losing half a second «on dying», and closing the window by hand **is not dying**: it
    /// is the only ending in which the loss can be avoided, and avoiding it costs nothing because
    /// by then there is no frame left to measure.
    pub(crate) fn save_now(&mut self) {
        if self.persistence.save_due.is_none() {
            return;
        }
        self.persistence.save_due = None;

        // **The clock starts here, before copying and comparing.** Copying the whole document and
        // comparing it thoroughly costs something, happens on every deadline that falls due —twice
        // a second while it is being dragged— and on the path where there is nothing to write **it
        // is the whole cost**: leaving it outside the clock would publish it as framework cost,
        // which is the leak the project's hard rule names.
        let started = Instant::now();

        // What is going to be written is the last thing seen: it has just been built in the
        // interface pass, so walking the tree again would be paying twice for the same datum.
        let Some(document) = self.last_seen.clone() else {
            self.mirror_this_frame += started.elapsed();
            return;
        };
        if self.persistence.last_written.as_ref() == Some(&document) {
            // **It goes to the mirror's span and not to the write's, because it is what it is**:
            // comparing two mirrors. Counting it as a write would put into that average some
            // samples that wrote nothing.
            self.mirror_this_frame += started.elapsed();
            return;
        }
        // **Through the validated type, which is what makes it impossible to write something that
        // cannot be read back.** And what does not pass validation **is warned about once**: the
        // real case is having more consoles open than the format admits —the ceiling exists to
        // bound the start-up `fork`, so it cannot be removed—, and without this warning the user
        // would lose their layout with no signal at all.
        let validated = match validate(document.clone()) {
            Ok(validated) => validated,
            Err(problem) => {
                self.persistence.warn_cannot_save(&problem);
                // **This is not a write and is not counted as one, not even as an attempt.** No
                // path was resolved and the disk was not touched: what ran was validating, which is
                // the same kind of work as comparing the mirror. Counting it in the write's span
                // would put into that average some samples that are **all** validation, and the
                // number would stop describing what writing costs.
                self.mirror_this_frame += started.elapsed();
                return;
            }
        };

        let written = self.persistence.save(&validated).is_ok();
        let how_much = started.elapsed();
        self.timed_separately += how_much;
        self.saving.record_write(how_much, written);

        if written {
            self.persistence.last_written = Some(document);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(width: usize) -> StoredNode {
        StoredNode::Leaf(StoredLeaf {
            label: None,
            panel_width: width,
            folded: false,
        })
    }

    /// A branch with those weights and one leaf for each.
    fn branch(orientation: StoredOrientation, weights: &[f32]) -> StoredNode {
        StoredNode::Branch {
            orientation,
            children: weights
                .iter()
                .map(|weight| StoredChild {
                    weight: *weight,
                    node: leaf(36),
                })
                .collect(),
        }
    }

    fn document(raices: Vec<StoredNode>) -> StoredDocument {
        StoredDocument {
            v: VERSION,
            tabs: raices
                .into_iter()
                .map(|root| StoredTab { root, label: None })
                .collect(),
        }
    }

    #[test]
    fn a_leaf_and_a_branch_are_told_apart_by_an_explicit_tag() {
        // **This is the contract, so it is asserted over the literal text and not over the type.** If one
        // day someone changes the enum's representation, the round trip would still work and **the file
        // of everyone who already uses the application would stop being read**: it is exactly the failure
        // the format version exists to avoid having to fix by surprise.
        let written = serialize(&document(vec![branch(
            StoredOrientation::Columns,
            &[3.0, 1.0],
        )]))
        .expect("a two-leaf document is written");

        assert_eq!(
            written,
            r#"{"v":2,"tabs":[{"root":{"branch":{"orientation":"columns","children":[{"weight":3.0,"node":{"leaf":{"panel_width":36,"folded":false}}},{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}}]}}}]}"#
        );
    }

    #[test]
    fn what_is_written_is_read_back_the_same() {
        let start = document(vec![
            StoredNode::Branch {
                orientation: StoredOrientation::Columns,
                children: vec![
                    StoredChild {
                        weight: 3.0,
                        node: leaf(52),
                    },
                    StoredChild {
                        weight: 1.0,
                        node: StoredNode::Branch {
                            orientation: StoredOrientation::Rows,
                            children: vec![
                                StoredChild {
                                    weight: 2.0,
                                    node: StoredNode::Leaf(StoredLeaf {
                                        label: None,
                                        panel_width: 36,
                                        folded: true,
                                    }),
                                },
                                StoredChild {
                                    weight: 1.0,
                                    node: leaf(24),
                                },
                            ],
                        },
                    },
                ],
            },
            leaf(36),
        ]);

        let written = serialize(&start).expect("a valid document is written");
        let after = interpret(&written).expect("what was just written is read");

        assert_eq!(after.tabs(), start.tabs.as_slice());
    }

    #[test]
    fn the_weights_do_not_have_to_sum_to_anything_specific() {
        // The weights are **proportions**, and the tiling normalizes by the sum of its branch: `{3, 1}`
        // and `{0.5, 1.5}` describe exactly the same tiling. Requiring them to sum to one or a hundred
        // would be a guard with no scenario **and would break the round trip** of any tree built by
        // dragging, which is where the real weights come from.
        let middle = document(vec![branch(StoredOrientation::Rows, &[0.5, 1.5])]);
        let whole = document(vec![branch(StoredOrientation::Rows, &[3.0, 9.0])]);

        assert!(validate(middle).is_ok());
        assert!(validate(whole).is_ok());
    }

    #[test]
    fn a_branch_with_a_single_child_is_rejected() {
        // **The invariant that upholds the tree's reason for being.** If a branch can have a single
        // child, back comes the nesting that makes a boundary drag a block of two consoles — what `tmux`
        // is most criticized for and what D14 exists to avoid.
        let one = document(vec![branch(StoredOrientation::Columns, &[1.0])]);

        assert_eq!(
            validate(one),
            Err(LayoutProblem::SingleChildBranch { tab: 0 })
        );
    }

    #[test]
    fn two_nested_branches_of_the_same_direction_are_rejected() {
        let nested = document(vec![StoredNode::Branch {
            orientation: StoredOrientation::Columns,
            children: vec![
                StoredChild {
                    weight: 1.0,
                    node: leaf(36),
                },
                StoredChild {
                    weight: 1.0,
                    node: branch(StoredOrientation::Columns, &[1.0, 1.0]),
                },
            ],
        }]);

        assert_eq!(
            validate(nested),
            Err(LayoutProblem::NestedBranches { tab: 0 })
        );
    }

    #[test]
    fn two_nested_branches_of_different_directions_are_accepted() {
        // **The sibling of the previous test, and it is needed.** Without it, «reject the nesting» and
        // «reject any branch inside another» pass the same tests, and the second would make impossible
        // the grid layout that F2.2 delivers.
        let grid = document(vec![StoredNode::Branch {
            orientation: StoredOrientation::Columns,
            children: vec![
                StoredChild {
                    weight: 1.0,
                    node: branch(StoredOrientation::Rows, &[1.0, 1.0]),
                },
                StoredChild {
                    weight: 1.0,
                    node: branch(StoredOrientation::Rows, &[1.0, 1.0]),
                },
            ],
        }]);

        assert!(validate(grid).is_ok());
    }

    #[test]
    fn a_weight_the_live_type_rejects_is_rejected_here() {
        // Zero and negative arrive via JSON; `NaN` and infinity have no literal, so the only way for them
        // to get in is a document built by hand — and even then the constructor stops them.
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let document = document(vec![branch(StoredOrientation::Columns, &[1.0, bad])]);

            assert!(
                matches!(validate(document), Err(LayoutProblem::InvalidWeight { .. })),
                "a weight of {bad} had to fall"
            );
        }
    }

    #[test]
    fn the_ceiling_accepts_exactly_the_leaves_and_rejects_one_more() {
        // **The two halves, and the bottom one is the one that matters.** A ceiling tested only from
        // above does not distinguish «reject more than 128» from «reject more than 1»: with the rejection
        // test alone, setting the ceiling at any smaller number would still be green.
        let weights: Vec<f32> = vec![1.0; MAX_LEAVES];
        let justas = document(vec![branch(StoredOrientation::Columns, &weights)]);
        assert!(validate(justas).is_ok(), "{MAX_LEAVES} leaves have to fit");

        let weights: Vec<f32> = vec![1.0; MAX_LEAVES + 1];
        let one_more = document(vec![branch(StoredOrientation::Columns, &weights)]);
        assert_eq!(
            validate(one_more),
            Err(LayoutProblem::TooManyLeaves {
                count: MAX_LEAVES + 1
            })
        );
    }

    #[test]
    fn the_ceiling_counts_the_leaves_of_the_whole_document_and_not_of_one_tab() {
        // If it counted per tab, two tabs of 128 would open 256 consoles and the ceiling would have been
        // of no use: what is bounded is how many processes are forked **on opening**, and that belongs to
        // the window.
        let half: Vec<f32> = vec![1.0; MAX_LEAVES / 2 + 1];
        let dos = document(vec![
            branch(StoredOrientation::Columns, &half),
            branch(StoredOrientation::Columns, &half),
        ]);

        assert!(matches!(
            validate(dos),
            Err(LayoutProblem::TooManyLeaves { .. })
        ));
    }

    #[test]
    fn a_version_that_is_not_the_known_one_is_rejected_saying_which_it_is() {
        let of_the_future = StoredDocument {
            v: VERSION + 1,
            tabs: vec![StoredTab {
                root: leaf(36),
                label: None,
            }],
        };

        assert_eq!(
            validate(of_the_future),
            Err(LayoutProblem::Version { found: VERSION + 1 })
        );
    }

    #[test]
    fn a_document_with_no_tabs_is_rejected() {
        let empty = StoredDocument {
            v: VERSION,
            tabs: Vec::new(),
        };

        assert_eq!(validate(empty), Err(LayoutProblem::NoTabs));
    }

    #[test]
    fn what_is_not_json_is_rejected_and_the_reason_is_said() {
        // **It asserts that the reason arrives, not only that it fails.** The only channel to tell it is
        // one line, so a `LayoutProblem` with no text would leave the user with «could not» and nothing
        // more.
        let Err(LayoutProblem::Unintelligible(reason)) = interpret("{this is not json") else {
            panic!("a broken document has to fall for not being understood");
        };

        assert!(!reason.is_empty(), "the reason has to say something");
    }

    #[test]
    fn a_node_with_no_tag_is_not_read() {
        // The shape that was rejected: leaf or branch deduced from which key is present. If this were
        // read, the file would have two valid shapes and one of them would not be the contract's.
        let unlabeled = r#"{"v":2,"tabs":[{"root":{"panel_width":36,"folded":false}}]}"#;

        assert!(matches!(
            interpret(unlabeled),
            Err(LayoutProblem::Unintelligible(_))
        ));
    }
}

#[cfg(test)]
mod ceiling_tests {
    use super::*;

    fn with_weights(weights: &[f32]) -> StoredDocument {
        StoredDocument {
            v: VERSION,
            tabs: vec![StoredTab {
                label: None,
                root: StoredNode::Branch {
                    orientation: StoredOrientation::Columns,
                    children: weights
                        .iter()
                        .map(|weight| StoredChild {
                            weight: *weight,
                            node: StoredNode::Leaf(StoredLeaf {
                                label: None,
                                panel_width: 36,
                                folded: false,
                            }),
                        })
                        .collect(),
                },
            }],
        }
    }

    #[test]
    fn two_weights_whose_product_would_be_zero_are_accepted_because_the_rescaler_prevents_the_zero()
    {
        // **This test asserted the opposite, and it was wrong.** The hole is real —when a pane is closed
        // the grandchildren rise with the scaled weight, `w · theirs / sum`, and that product
        // **underflows**: checked by running it, `1e-40 · 1e-40 / 2e-40` gives **zero**— but closing it
        // here, rejecting the document, was closing it in the wrong place: **these numbers are reached by
        // dragging**, so what this rejection threw away was the layout of whoever had used the
        // application. The answer now lives where the product happens, in `crate::tiling::Weight::scaled`, and
        // that is why they are accepted here.
        let tiny = 1.0e-40_f32;
        assert!(
            crate::tiling::Weight::new(tiny).is_some(),
            "each weight on its own has to be valid, or this test proves nothing"
        );
        assert_eq!(
            tiny * tiny / (tiny + tiny),
            0.0,
            "and its scaling has to give zero, or it does not either"
        );

        assert!(validate(with_weights(&[tiny, tiny])).is_ok());
    }

    #[test]
    fn two_finite_weights_whose_sum_is_not_a_number_are_rejected() {
        let at_limit = 3.0e38_f32;
        assert!(
            crate::tiling::Weight::new(at_limit).is_some(),
            "each weight on its own has to be valid"
        );
        assert!(
            !(at_limit + at_limit).is_finite(),
            "and their sum has to overflow"
        );

        // **And it falls for the sum, not for the weight.** Now that there is no dedicated range, this is
        // the gate that upholds the format: the exact variant is asserted because confusing them would be
        // not knowing which of the two checks is working.
        assert_eq!(
            validate(with_weights(&[at_limit, at_limit])),
            Err(LayoutProblem::NonFiniteSum { tab: 0 })
        );
    }

    #[test]
    fn any_weight_the_live_tree_can_have_is_accepted() {
        // **And this fixes a defect instead of avoiding it.** There was a dedicated range, `[1e-6, 1e6]`,
        // and both ends rejected layouts that are reached by **dragging**: at the bottom dragging trims
        // the fraction and not the weight, and at the top what it preserves is the sum of the pair, so a
        // weight rises up to 99 % of what two neighbors summed. Either of the two rejections lost the
        // user's placement on reopening, without anything having been seen on screen.
        for weights in [
            [1.0, 1e-30],
            [1.0, f32::MIN_POSITIVE],
            [1.0, 1e30],
            [0.5, 1.5],
            [100.0, 0.01],
        ] {
            assert!(
                validate(with_weights(&weights)).is_ok(),
                "{weights:?} had to be valid"
            );
        }
    }

    #[test]
    fn what_is_not_a_weight_is_still_rejected() {
        // The gate that remains, and it is the live type's: neither zero, nor negative, nor infinite.
        for bad in [0.0, -1.0, f32::INFINITY] {
            assert_eq!(
                validate(with_weights(&[1.0, bad])),
                Err(LayoutProblem::InvalidWeight { tab: 0 }),
                "a weight of {bad:e} had to fall"
            );
        }
    }

    #[test]
    fn a_document_larger_than_the_ceiling_is_rejected_without_interpreting_it() {
        // **And it is rejected for the size, not for the syntax.** The text is garbage: if the ceiling
        // were checked after interpreting, the reason would be «not understood» and the cost would
        // already have happened. That the reason is the size **is** the proof that the order is the
        // correct one.
        let huge = "x".repeat(MAX_BYTES + 1);

        assert_eq!(
            interpret(&huge),
            Err(LayoutProblem::TooLarge {
                bytes: MAX_BYTES as u64 + 1
            })
        );
    }

    #[test]
    fn a_document_exactly_the_size_of_the_ceiling_is_interpreted() {
        // The other half: the ceiling cannot be a disguised zero. It is padded with spaces, which JSON
        // ignores, until it takes up exactly the ceiling.
        let minimum = r#"{"v":2,"tabs":[{"root":{"leaf":{"panel_width":36,"folded":false}}}]}"#;
        let padding = " ".repeat(MAX_BYTES - minimum.len());
        let exactly = format!("{minimum}{padding}");
        assert_eq!(exactly.len(), MAX_BYTES);

        assert!(interpret(&exactly).is_ok());
    }

    /// **What the measurement batch seeds has to keep being understood, and no one asserted it.**
    ///
    /// `scripts/measure-frame.sh` writes this document **from `sh`**, with the format tags put in by
    /// hand in a `printf`. So the day this enum renames `leaf`, `branch`, `weight` or `orientation`, the
    /// script would keep writing the old name and **the application would open factory-fresh**: the batch
    /// that claims to measure restored startup would measure the usual one. The run would catch it —the
    /// report would not declare restored consoles and the script invalidates it— but that is fifteen
    /// minutes of measurement thrown away, and this test catches it in the suite.
    #[test]
    fn the_document_the_measurement_batch_seeds_keeps_being_understood() {
        let seeded = r#"{"v":2,"tabs":[{"root":{"branch":{"orientation":"columns","children":[{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}},{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}},{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}},{"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}}]}}}]}"#;

        let validated =
            interpret(seeded).expect("the batch seeds a document that is no longer understood");

        assert_eq!(validated.leaves(), 4, "the four consoles of batch L");

        // **And where it leaves it, which is the other half and was slipping away.** The script writes
        // to `<config>/agent-terminal/layout.json` with those two names put in by hand; renaming either
        // of the two constants would leave the suite green while **every run of the batch would open
        // factory-fresh**, because the application would look for its file elsewhere and would find
        // nothing.
        assert_eq!(
            DIRECTORY, "agent-terminal",
            "the script seeds in this directory"
        );
        assert_eq!(FILE, "layout.json", "and with this file name");
    }

    #[test]
    fn the_leaf_count_that_is_published_is_the_one_compared_against_the_ceiling() {
        // If `leaves()` recomputed instead of reading what was stored, they could stop being the same
        // number.
        let two_and_three = StoredDocument {
            v: VERSION,
            tabs: vec![
                StoredTab {
                    label: None,
                    root: StoredNode::Branch {
                        orientation: StoredOrientation::Columns,
                        children: vec![
                            StoredChild {
                                weight: 1.0,
                                node: StoredNode::Leaf(StoredLeaf {
                                    label: None,
                                    panel_width: 36,
                                    folded: false,
                                }),
                            },
                            StoredChild {
                                weight: 1.0,
                                node: StoredNode::Leaf(StoredLeaf {
                                    label: None,
                                    panel_width: 36,
                                    folded: false,
                                }),
                            },
                        ],
                    },
                },
                StoredTab {
                    label: None,
                    root: StoredNode::Leaf(StoredLeaf {
                        label: None,
                        panel_width: 36,
                        folded: false,
                    }),
                },
            ],
        };

        let validated = validate(two_and_three).expect("the document is valid");
        assert_eq!(validated.leaves(), 3);
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    /// A test's directory, which is taken away when what the test wrote is done.
    ///
    /// **One per test and with the process number**, because the suite runs in parallel and the file
    /// name is fixed: two tests that shared a directory would read each other's layout and would fail
    /// only sometimes.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("agent-terminal-test-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create the test directory");
            Self(path)
        }

        /// The file where the application would put it.
        ///
        /// **With the configuration directory not created, and not just the application's own**: on a
        /// freshly made account it does not exist either, so every test that saves exercises the creation
        /// of both segments. With the upper one already made, a creation that did not admit intermediate
        /// segments would pass these tests and would fail on the machine of whoever first runs the
        /// application.
        fn file(&self) -> PathBuf {
            path_from(Some(self.0.join("config").as_os_str()), None)
                .expect("the test path is absolute")
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A document with two consoles of different widths, one folded and with unequal weights.
    ///
    /// **Nothing like a minimal document**: with a single console of factory width, a round trip that
    /// lost the widths, the weights or the orientation would still pass.
    /// The same, already validated: it is what the saving path demands.
    fn test_layout() -> Validated {
        validate(test_document()).expect("the test document is valid")
    }

    fn test_document() -> StoredDocument {
        StoredDocument {
            v: VERSION,
            tabs: vec![StoredTab {
                label: None,
                root: StoredNode::Branch {
                    orientation: StoredOrientation::Rows,
                    children: vec![
                        StoredChild {
                            weight: 3.0,
                            node: StoredNode::Leaf(StoredLeaf {
                                label: None,
                                panel_width: 52,
                                folded: false,
                            }),
                        },
                        StoredChild {
                            weight: 1.0,
                            node: StoredNode::Leaf(StoredLeaf {
                                label: None,
                                panel_width: 24,
                                folded: true,
                            }),
                        },
                    ],
                },
            }],
        }
    }

    /// **What is published as read time has to come out of a read**, and nothing asserted it.
    ///
    /// The measurement report prints this number. Without a test, changing the stopwatch for a zero left
    /// the whole suite green while the report published a time no one took — the disguise this project
    /// names: choosing a zero with which the defect and the fix give the same thing.
    #[test]
    fn the_load_time_only_exists_when_there_was_a_load() {
        let home = TestDir::new("load-time");
        let file = home.file();
        std::fs::create_dir_all(file.parent().expect("the file has a directory"))
            .expect("create the configuration directory");
        std::fs::write(
            &file,
            r#"{"v":2,"tabs":[{"root":{"leaf":{"panel_width":36,"folded":false}}}]}"#,
        )
        .expect("seed the layout");

        let mut persistence = Persistence::at(Some(file));
        assert!(
            persistence.load_time().is_none(),
            "it claims to have measured a load that has not happened yet"
        );

        persistence.load().expect("the document is understood");

        // **Greater than zero, and not just «there is a number».** Asserting a bare `is_some()`, changing
        // the stopwatch for a zero leaves the test green: `Some(0)` is also something. And a zero is
        // exactly what this project has on file as the number with which the defect and the fix give the
        // same result. Reading a file from disk does not take zero.
        let took = persistence
            .load_time()
            .expect("it read the file and did not state how long it took: the report would say «not measured»");
        assert!(
            took > Duration::ZERO,
            "it publishes a zero as read time, which is a number no one took"
        );
    }

    #[test]
    fn the_path_comes_out_of_the_configuration_directory() {
        assert_eq!(
            path_from(Some(OsStr::new("/home/config")), Some(OsStr::new("/home"))),
            Some(PathBuf::from("/home/config/agent-terminal/layout.json"))
        );
    }

    #[test]
    fn with_no_configuration_directory_it_falls_back_to_home() {
        assert_eq!(
            path_from(None, Some(OsStr::new("/home"))),
            Some(PathBuf::from("/home/.config/agent-terminal/layout.json"))
        );
    }

    #[test]
    fn an_empty_or_relative_variable_is_not_a_path_and_does_not_rule_out_the_other() {
        // Home is different from the two variables above on purpose: if the empty or the relative one
        // slipped through, the path would not be this one.
        for variable in ["", "config", "./config", "~/config"] {
            assert_eq!(
                path_from(Some(OsStr::new(variable)), Some(OsStr::new("/home"))),
                Some(PathBuf::from("/home/.config/agent-terminal/layout.json")),
                "with {variable:?} it did not fall back to home"
            );
        }
    }

    #[test]
    fn with_no_absolute_path_at_all_nothing_is_persisted() {
        assert_eq!(path_from(None, None), None);
        assert_eq!(path_from(Some(OsStr::new("")), Some(OsStr::new(""))), None);
        assert_eq!(
            path_from(Some(OsStr::new("config")), Some(OsStr::new("home"))),
            None
        );
    }

    #[test]
    fn what_is_saved_is_found_again() {
        let directory = TestDir::new("round-trip");
        let path = directory.file();

        assert!(
            Persistence::at(Some(path.clone()))
                .save(&test_layout())
                .is_ok(),
            "it was not saved"
        );

        let read_back = read(&path).expect("what was just saved is read");
        assert_eq!(read_back.tabs(), test_document().tabs.as_slice());
    }

    #[cfg(unix)]
    #[test]
    fn the_destination_is_replaced_whole_and_never_written_over() {
        use std::os::unix::fs::MetadataExt;

        // **The milestone's central promise, and only this pins it down.** Copying the text over the
        // destination would leave the same three final-result tests green —the file ends up with the
        // correct content and no temporaries next to it— and yet there would be an instant in which what
        // is on disk is half a document. What distinguishes replacing from writing over is that **the
        // destination is another file**: with a rename it changes node, copying it does not.
        let directory = TestDir::new("substitution");
        let path = directory.file();
        let mut persistence = Persistence::at(Some(path.clone()));

        assert!(persistence.save(&test_layout()).is_ok());
        let first = std::fs::metadata(&path).expect("it exists").ino();
        assert!(persistence.save(&test_layout()).is_ok());
        let second = std::fs::metadata(&path).expect("it exists").ino();

        assert_ne!(
            first, second,
            "the destination was written over instead of replaced"
        );
    }

    #[test]
    fn saving_leaves_no_temporary_behind() {
        let directory = TestDir::new("no-leftovers");
        let path = directory.file();
        let mut file = Persistence::at(Some(path.clone()));

        // Twice, because the second save is the one that finds the directory and the destination already
        // made: it is the path that is really walked, twice per second while dragging.
        assert!(file.save(&test_layout()).is_ok());
        assert!(file.save(&test_layout()).is_ok());

        let inside: Vec<_> = std::fs::read_dir(path.parent().expect("the dedicated directory"))
            .expect("read the directory")
            .map(|entry| entry.expect("an entry").file_name())
            .collect();
        assert_eq!(inside, vec![std::ffi::OsString::from(FILE)]);
    }

    #[cfg(unix)]
    #[test]
    fn the_directory_and_the_file_are_born_only_for_the_user() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new("permissions");
        let path = directory.file();
        assert!(
            Persistence::at(Some(path.clone()))
                .save(&test_layout())
                .is_ok()
        );

        let modo = |path: &Path| {
            std::fs::metadata(path)
                .expect("it exists")
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(
            modo(path.parent().expect("the dedicated directory")),
            0o700,
            "the directory is left open to the session's mask"
        );
        assert_eq!(modo(&path), 0o600, "the file is left readable by others");
    }

    #[test]
    fn an_absent_file_restores_nothing_and_lets_startup_proceed() {
        let directory = TestDir::new("absent");
        let path = directory.file();

        assert_eq!(read(&path), Err(LayoutProblem::Absent));
        assert!(Persistence::at(Some(path)).load().is_none());
    }

    #[test]
    fn what_is_not_a_file_is_rejected_before_opening_it() {
        let directory = TestDir::new("not-a-file");
        let path = directory.file();
        std::fs::create_dir_all(&path).expect("plant a directory with the file's name");

        assert_eq!(read(&path), Err(LayoutProblem::NotAFile));
    }

    #[test]
    fn a_file_that_goes_over_the_ceiling_is_rejected_for_its_size_and_not_for_its_encoding() {
        let directory = TestDir::new("too-large");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        // **Accents, and not single letters, and there is the defect this pins down.** The read cuts one
        // byte above the ceiling; with two-byte characters that cut splits one in half, so without the
        // size check this would come out as «not text» and the ceiling would never be reached.
        let huge = "á".repeat(MAX_BYTES);
        std::fs::write(&path, &huge).expect("write the fat file");

        assert_eq!(
            read(&path),
            Err(LayoutProblem::TooLarge {
                bytes: huge.len() as u64
            })
        );
    }

    #[test]
    fn exactly_at_the_ceiling_it_is_read_and_one_byte_more_is_not() {
        let directory = TestDir::new("at-the-ceiling");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        let document = serialize(&test_document()).expect("it is described");

        // The padding is spaces, which the interpreter accepts at the end: this way the document stays
        // valid and the only thing that changes between the two halves of this test is **one byte of
        // size**. Without the bottom half, a ceiling that bounded nothing would pass just the same.
        let at_ceiling = format!("{document}{}", " ".repeat(MAX_BYTES - document.len()));
        std::fs::write(&path, &at_ceiling).expect("write");
        assert!(
            read(&path).is_ok(),
            "what takes up exactly the ceiling was not read"
        );

        std::fs::write(&path, format!("{at_ceiling} ")).expect("write one more");
        assert_eq!(
            read(&path),
            Err(LayoutProblem::TooLarge {
                bytes: MAX_BYTES as u64 + 1
            })
        );
    }

    #[test]
    fn what_is_not_text_is_rejected() {
        let directory = TestDir::new("not-text");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        std::fs::write(&path, [0xff, 0xfe, b'{', b'}']).expect("write bytes that are not text");

        assert_eq!(read(&path), Err(LayoutProblem::NotText));
    }

    #[test]
    fn with_no_text_to_write_nothing_is_renamed() {
        // **This is the reason the write receives the already-described text.** If describing failed and
        // the path renamed anyway, the destination would be left with zero bytes: the layout the user
        // placed by hand, deleted by a failure on saving.
        let directory = TestDir::new("no-text");
        let path = directory.file();
        let mut file = Persistence::at(Some(path.clone()));
        assert!(file.save(&test_layout()).is_ok());
        let start = std::fs::read_to_string(&path).expect("there is something saved");

        let error = save_described(&path, 1, None);

        assert!(
            matches!(error, Err(SaveFailure::NotSerializable)),
            "{error:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("it is still there"),
            start,
            "the saved layout was lost"
        );
        let inside = std::fs::read_dir(path.parent().expect("the dedicated directory"))
            .expect("read the directory")
            .count();
        assert_eq!(inside, 1, "something else was left in the directory");
    }

    #[test]
    fn a_temporary_that_already_exists_is_not_adopted() {
        let directory = TestDir::new("planted-temporary");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        let temp_path = temp_path_for(&path, 1).expect("the temporary has a name");
        std::fs::write(&temp_path, "someone else's").expect("plant the temporary");

        let error = save_to(&path, 1, &test_document());

        assert!(matches!(error, Err(SaveFailure::TempFile(_))), "{error:?}");
        assert!(!path.exists(), "the destination was written anyway");
        assert_eq!(
            std::fs::read_to_string(&temp_path).expect("it is still there"),
            "someone else's",
            "it was written through what had been planted"
        );
    }

    #[test]
    fn if_it_cannot_be_put_in_its_place_the_temporary_is_not_left() {
        // The destination is a directory with something inside, which is a way to make the rename fail
        // without touching anyone's permissions. What is pinned down is the cleanup: a temporary that
        // survives a failure **leaves this very session unable to create its own exclusively**, so the
        // first failure would be the last attempt.
        let directory = TestDir::new("impossible-rename");
        let path = directory.file();
        std::fs::create_dir_all(path.join("inside"))
            .expect("plant a directory with something inside");

        let error = save_to(&path, 1, &test_document());

        assert!(matches!(error, Err(SaveFailure::Renaming(_))), "{error:?}");
        assert!(
            !temp_path_for(&path, 1)
                .expect("the temporary has a name")
                .exists(),
            "the temporary was left, and with it the session is left unable to save"
        );
    }

    #[test]
    fn a_newer_version_is_not_touched_for_the_whole_session() {
        let directory = TestDir::new("newer-version");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        // **The shape is the one a version 3 would really have: a renamed key.** And that is the heart of
        // this test, not a detail. The version goes up **exactly** with what an old reader would
        // misinterpret —a renamed key, a removed one, or one with a different meaning—, that is, with
        // what makes the shape interpreter fail. With a version 3 file that kept version 2's shape, this
        // test passes even looking at the version **after** requiring the shape, which is exactly the
        // defect that was slipping through: the file from the future was read as «not understood» and
        // overwritten.
        let of_the_future = r#"{"v":3,"solapas":[{"raiz":{"hoja":{"ancho":36,"plegado":false}}}]}"#;
        std::fs::write(&path, of_the_future).expect("write the file from the future");
        let mut file = Persistence::at(Some(path.clone()));

        assert!(
            file.load().is_none(),
            "a shape that is not known was restored"
        );
        // Two attempts, because the veto belongs to the session and not to the attempt: with one, a veto
        // that only held for the first would pass just the same.
        assert!(file.save(&test_layout()).is_err());
        assert!(file.save(&test_layout()).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("it is still there"),
            of_the_future,
            "an old binary ran over the new one's layout"
        );
    }

    #[test]
    fn only_the_three_reasons_restarting_fixes_end_the_attempts() {
        // **Every variant, both ways.** Calling a transient reason permanent stops saving for the
        // rest of the session over a disk that was busy for a second, and the window-level retry
        // test only walks `Directory`: without this table, a `TempFile` or a `Writing` added to the
        // match would pass everything.
        let io = || std::io::Error::other("test");
        let cases = [
            (SaveFailure::NoPlace, true),
            (SaveFailure::Vetoed { found: VERSION + 1 }, true),
            (SaveFailure::WontOverwrite("test"), true),
            (SaveFailure::NotSerializable, false),
            (SaveFailure::Directory(io()), false),
            (SaveFailure::TempFile(io()), false),
            (SaveFailure::Writing(io()), false),
            (SaveFailure::Renaming(io()), false),
            (SaveFailure::DoesntFit, false),
        ];
        for (failure, lasts) in cases {
            assert_eq!(
                failure.lasts_the_session(),
                lasts,
                "«{failure}» was classified the wrong way round"
            );
        }
    }

    #[test]
    fn a_newer_version_on_disk_ends_the_attempts_after_the_first() {
        // **The veto is one of the three reasons that last the session, and the only one no other
        // test binds**: the prohibition and the missing path are exercised through the window in
        // `lib.rs`, the veto only here. And the permanence is **learned from an attempt**: deciding
        // it from the veto on loading would skip the attempt that says the line, so before saving
        // it has to be false even though the veto is already known.
        let directory = TestDir::new("newer-version-ends-attempts");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        let of_the_future = format!(
            r#"{{"v":{},"solapas":[{{"raiz":{{"hoja":{{"ancho":36,"plegado":false}}}}}}]}}"#,
            VERSION + 1
        );
        std::fs::write(&path, &of_the_future).expect("write the file from the future");
        let mut persistence = Persistence::at(Some(path.clone()));

        assert!(persistence.load().is_none(), "a newer version was restored");
        assert!(
            !persistence.will_never_write(),
            "the session gave up before attempting a single save: the line would never be said"
        );
        assert!(
            matches!(
                persistence.save(&test_layout()),
                Err(SaveFailure::Vetoed { found }) if found == VERSION + 1
            ),
            "a newer version on disk did not veto the save"
        );
        assert!(
            persistence.will_never_write(),
            "a veto that lasts the session left the window retrying on every change"
        );
    }

    #[test]
    fn a_file_with_no_version_is_one_of_ours_that_is_broken_and_it_is_overwritten() {
        // The other half of the veto, and the one that distinguishes «I don't know what version this is»
        // from «this was written by something that knows more than I do». A file **with no** version
        // cannot be of a future version —they all carry it—, so it is ours and it is broken: it opens
        // factory-fresh and it is replaced.
        let directory = TestDir::new("no-version");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        std::fs::write(
            &path,
            r#"{"tabs":[{"root":{"leaf":{"panel_width":36,"folded":false}}}]}"#,
        )
        .expect("write");
        let mut persistence = Persistence::at(Some(path.clone()));

        assert!(persistence.load().is_none());
        assert!(
            persistence.save(&test_layout()).is_ok(),
            "a file with no version was treated as one from the future"
        );
        assert_eq!(
            read(&path).expect("it is read").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn a_version_that_is_not_known_and_is_not_newer_is_overwritten() {
        let directory = TestDir::new("version-zero");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        std::fs::write(&path, r#"{"v":0,"tabs":[]}"#).expect("write");
        let mut file = Persistence::at(Some(path.clone()));

        assert!(file.load().is_none());
        assert!(
            file.save(&test_layout()).is_ok(),
            "a file of ours that is broken is not replaced"
        );
        assert_eq!(
            read(&path).expect("it is read").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn with_nowhere_to_put_it_it_is_neither_read_nor_written() {
        let mut file = Persistence::at(None);

        assert!(file.load().is_none());
        assert!(file.save(&test_layout()).is_err());
    }

    #[test]
    fn each_reason_is_warned_about_once_and_the_others_likewise() {
        let mut file = Persistence::at(None);
        let other = || std::io::Error::other("other");

        assert!(file.should_warn(&SaveFailure::Renaming(other())));
        assert!(
            !file.should_warn(&SaveFailure::Renaming(std::io::Error::other("different"))),
            "the same reason with another system error was warned about twice"
        );
        assert!(
            file.should_warn(&SaveFailure::Directory(other())),
            "a new reason was kept quiet because another had already been warned about"
        );
    }

    #[test]
    fn the_only_failure_that_is_normal_is_not_warned_about_and_the_others_are() {
        // The warning is a line to the error output, which cannot be observed from inside the process:
        // what can be asserted is **the decision**, and that is why it lives apart from whoever prints
        // it.
        assert!(
            !warrants_warning(&LayoutProblem::Absent),
            "every user's first startup would come out with an error line"
        );
        for problem in [
            LayoutProblem::NotAFile,
            LayoutProblem::NotText,
            LayoutProblem::Unreadable("any".to_owned()),
            LayoutProblem::Version { found: 3 },
            LayoutProblem::TooLarge { bytes: 1 },
            LayoutProblem::NoTabs,
        ] {
            assert!(warrants_warning(&problem), "it stays quiet: {problem}");
        }
    }

    #[test]
    fn the_temporary_carries_the_process_and_the_attempt() {
        // **An assertion about the name, and not about the effect, because the effect does not fit in one
        // process**: what the name avoids is that two instances share a temporary and that a temporary
        // orphaned by a violent death leaves the session that receives that same process number unable to
        // save.
        let path = PathBuf::from("/home/.config/agent-terminal/layout.json");
        let pid = std::process::id();

        let first = temp_path_for(&path, 1).expect("it has a name");
        let second = temp_path_for(&path, 2).expect("it has a name");

        assert_eq!(
            first,
            PathBuf::from(format!("/home/.config/agent-terminal/layout.json.{pid}.1"))
        );
        assert_ne!(first, second, "two attempts share a temporary");
        assert_eq!(
            first.parent(),
            path.parent(),
            "the temporary is not next to the destination, so the rename can cross volumes"
        );
    }

    #[test]
    fn an_orphaned_temporary_hinders_one_attempt_and_not_the_session() {
        // The scenario: the application died between creating the temporary and renaming it, so one was
        // left on disk that no one cleans; and this session received the same process number. Adopting it
        // is forbidden —it would be written through what someone else planted—, so what has to happen is
        // that the next attempt uses another name. Without that, this session **would never save
        // anything**.
        let directory = TestDir::new("orphan");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        std::fs::write(
            temp_path_for(&path, 1).expect("it has a name"),
            "the one the dead left",
        )
        .expect("plant the orphan");
        let mut persistence = Persistence::at(Some(path.clone()));

        assert!(
            persistence.save(&test_layout()).is_err(),
            "the temporary that was there was adopted"
        );
        assert!(
            persistence.save(&test_layout()).is_ok(),
            "an orphan left the whole session unable to save"
        );
        assert_eq!(
            read(&path).expect("it is read").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn reading_does_not_reserve_what_the_file_claims_to_take_up() {
        // **The ceiling exists to bound the reservation, not only the interpreter**, and that is the only
        // thing this test looks at: the capacity. With the file size as the reservation and without
        // bounding it, a file that claims to take up ten gigs asks for them at startup, and the format's
        // ceiling never gets looked at.
        let directory = TestDir::new("reservation");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        let file = std::fs::File::create(&path).expect("create");
        // A hole, not four megs of written zeros: what matters is what the file's metadata says, which is
        // where the reservation comes from.
        file.set_len(64 * MAX_BYTES as u64)
            .expect("enlarge it without writing");
        drop(file);

        let bytes = read_capped(
            std::fs::File::open(&path).expect("open"),
            std::fs::metadata(&path).expect("metadata").len(),
        )
        .expect("read");

        assert!(
            bytes.capacity() <= MAX_BYTES + 1,
            "it reserved {} bytes for a ceiling of {MAX_BYTES}",
            bytes.capacity()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_named_pipe_is_rejected_without_waiting() {
        // **It is the case the «is it a file» check exists to avoid, and the only one a planted directory
        // does not show**: opening a pipe with no one at the other end does not fail, it **waits**, and
        // this happens before the window exists. The deadline is the witness: without the check the read
        // never returns, and what this test can tell is that it did not return.
        let directory = TestDir::new("pipe");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        let planted = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("launch mkfifo");
        assert!(planted.success(), "the pipe could not be planted");

        // On another thread on purpose: if the opening keeps waiting, the one that waits is it. It hangs
        // and is taken away by the end of the process.
        let (dice, listening) = std::sync::mpsc::channel();
        let its_own = path.clone();
        std::thread::spawn(move || dice.send(read(&its_own)));

        match listening.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(result) => assert_eq!(result, Err(LayoutProblem::NotAFile)),
            Err(_) => panic!("opening the pipe kept waiting: the window would never open"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_cannot_be_read_neither_prevents_startup_nor_vetoes_writing() {
        use std::os::unix::fs::PermissionsExt;

        // The only system failure the read can give and that no one exercises. And what matters is not
        // the variant: it is that **it is not confused with a version from the future**, so the session
        // can still save. A misset permission must not leave anyone unable to write for the rest of the
        // session.
        //
        // **This test assumes the suite does not run as administrator**, which skips permissions: there
        // the file would be read and the test would fail on not finding the failure. It is let to fail
        // instead of skipping itself, because a test that disables itself depending on who runs it is
        // indistinguishable from one that passes.
        let directory = TestDir::new("unreadable");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        std::fs::write(&path, r#"{"v":2,"tabs":[]}"#).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000))
            .expect("strip it of all permissions");

        let problem = read(&path);
        assert!(
            matches!(problem, Err(LayoutProblem::Unreadable(_))),
            "{problem:?}"
        );

        let mut persistence = Persistence::at(Some(path.clone()));
        assert!(persistence.load().is_none());
        assert!(
            persistence.save(&test_layout()).is_ok(),
            "a file that could not be read was treated as one from the future"
        );
        assert_eq!(
            read(&path).expect("it is read").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn an_invalid_shape_of_the_known_version_does_not_prevent_startup_and_is_replaced() {
        // **The version is ours and the shape is no good**, which is what distinguishes this path from
        // the other two broken files: there the interpreter fails, and here everything fits and what does
        // not hold up is the tree. Without this, a reader that forgot to validate what it interprets
        // would pass just the same: the shape tests call `validate` by hand, so none pins down that the
        // disk path goes through there.
        let directory = TestDir::new("invalid-shape");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        // A cut with a single child: the shape that makes a boundary drag a block of two.
        std::fs::write(
            &path,
            r#"{"v":2,"tabs":[{"root":{"branch":{"orientation":"columns","children":[
                 {"weight":1.0,"node":{"leaf":{"panel_width":36,"folded":false}}}]}}}]}"#,
        )
        .expect("write");
        let mut persistence = Persistence::at(Some(path.clone()));

        assert_eq!(
            read(&path),
            Err(LayoutProblem::SingleChildBranch { tab: 0 })
        );
        assert!(
            persistence.load().is_none(),
            "a tree that validation rejects was restored"
        );
        assert!(
            persistence.save(&test_layout()).is_ok(),
            "a file of ours with a shape that is no good was treated as one from the future"
        );
        assert_eq!(
            read(&path).expect("it is read").tabs(),
            test_document().tabs.as_slice()
        );
    }

    #[test]
    fn if_the_directory_cannot_be_created_it_is_said_for_that_reason_and_not_another() {
        // **The variant matters because the warning is suppressed per reason**: a directory that cannot
        // be created and a temporary that cannot be created are fixed in different places, so arriving
        // here as the second would leave the user looking in the wrong place.
        let directory = TestDir::new("impossible-directory");
        std::fs::write(directory.0.join("config"), "I am not a directory")
            .expect("plant a file where the configuration directory would go");
        let path = directory.file();

        let error = save_to(&path, 1, &test_document());

        assert!(matches!(error, Err(SaveFailure::Directory(_))), "{error:?}");
        assert!(
            Persistence::at(Some(path.clone()))
                .save(&test_layout())
                .is_err(),
            "it was taken as saved without anywhere to put it"
        );
        assert!(!path.exists(), "something ended up in the file's place");
    }

    #[test]
    fn a_path_that_does_not_name_a_file_does_not_bring_down_the_save() {
        // Neither of the two comes out of `path_from`, so it is only reached by placing them by hand.
        // What is pinned down is that this path has no unwrap that brings down the window instead of
        // saying it could not: saving runs twice per second while a boundary is dragged.
        let without_directory = save_to(Path::new("/"), 1, &test_document());
        assert!(
            matches!(without_directory, Err(SaveFailure::NoPlace)),
            "{without_directory:?}"
        );
        assert!(
            Persistence::at(Some(PathBuf::from("/")))
                .save(&test_layout())
                .is_err()
        );

        let directory = TestDir::new("no-name");
        let subiendo = directory
            .file()
            .parent()
            .expect("the dedicated directory")
            .join("..");
        let without_name = save_to(&subiendo, 1, &test_document());
        assert!(
            matches!(without_name, Err(SaveFailure::NoPlace)),
            "{without_name:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_planted_at_the_destination_is_not_written_through() {
        // The destination is protected by **never opening it for writing**, and this is what that buys: a
        // link with its name does not take the write to where it points. That the destination changes
        // node is pinned down by another test; this one pins down that the outside file is not touched.
        let directory = TestDir::new("link-at-the-destination");
        let path = directory.file();
        std::fs::create_dir_all(path.parent().expect("the dedicated directory"))
            .expect("create it");
        let victim = directory.0.join("victim");
        std::fs::write(&victim, "what was outside").expect("write the victim");
        std::os::unix::fs::symlink(&victim, &path).expect("plant the link");

        assert!(
            Persistence::at(Some(path.clone()))
                .save(&test_layout())
                .is_ok()
        );

        assert_eq!(
            std::fs::read_to_string(&victim).expect("it is still there"),
            "what was outside",
            "it was written through the link, that is, outside the configuration directory"
        );
        assert!(
            !std::fs::symlink_metadata(&path)
                .expect("it exists")
                .is_symlink(),
            "the link is still in the file's place"
        );
    }

    #[test]
    fn a_save_that_fails_spends_the_warning_of_its_reason() {
        // **It pins down that the suppression is applied by the saving path**, and not only that the list
        // of reasons works. With the warning printed without consulting it, a full disk would say two
        // lines per second while the window is in use, which is what the list exists to avoid.
        let mut file = Persistence::at(None);

        assert!(file.save(&test_layout()).is_err());

        assert!(
            !file.should_warn(&SaveFailure::NoPlace),
            "the save failure did not go through the suppression"
        );
        assert!(
            file.should_warn(&SaveFailure::Directory(std::io::Error::other("other"))),
            "it also spent the warning of reasons that have not failed"
        );
    }

    #[test]
    fn loading_with_nowhere_does_not_spend_the_save_warning() {
        // The two paths share the reason and not the message. If loading spent the warning, the first
        // genuine save failure —the one the user needs to see— would come out mute.
        let mut file = Persistence::at(None);

        assert!(file.load().is_none());

        assert!(
            file.should_warn(&SaveFailure::NoPlace),
            "loading spent the warning and the first failure on saving would go unsaid"
        );
    }
}
