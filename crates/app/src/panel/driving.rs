//! Who is publishing to a panel, when it is not its file.
//!
//! **This is D17's answer to «who rules», and the whole of it.** The contract has two transports and
//! the file cannot identify its producer, so authority cannot be given to it: while a connection
//! holds a pane it is the only writer, the file is not read at all, and a second source is refused
//! by name. There is no sequence number anywhere and none is needed — one connection's
//! newline-framed requests cannot reorder themselves, and with the file unread there is nobody else
//! to order against.
//!
//! It sits beside the panel rather than inside it for the reason the size rule gives (**D28**): what
//! the panel is and how it is painted is one subject, and who is allowed to write it is another.

use std::io::{self, Read as _};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};

use panel_contract::{Panel, Patch};

use super::PanelState;

/// What a connection that is not the holder is told, **for both verbs that write**.
///
/// **One function and not the sentence written twice**, because a producer that reads one wording
/// from a publication and another from a patch has to work out whether they mean the same refusal.
/// It is the connection that is compared and never the source: two connections calling themselves
/// the same thing are two producers, which is the collision the hold exists to stop.
fn held_by_somebody_else(source: &str) -> String {
    format!("this panel is held by \"{source}\" until that connection closes")
}

/// What a patch is told when a document exists and no connection is driving it.
///
/// **It says the move**, because publishing is the ground state a patch is measured against and a
/// refusal that did not say so would leave an agent guessing which verb it is missing.
///
/// **It is not the sentence for a pane with nothing on it, and the merge that used to make it one
/// was wrong.** The design argued the two states «cannot be told apart from outside» — and the read
/// verb tells them apart, to the same producer over the same channel, answering
/// [`NOTHING_PUBLISHED`]. An agent that had just read its own document back and was then told that
/// document belonged to the file would be reading two contradictory sentences about one pane,
/// twelve lines apart in the contract it works from.
///
/// **And the two states are both reachable**: a producer that never published, and one whose holder
/// released while what it published stayed on screen, are not the same pane.
const ITS_FILES: &str =
    "this console's panel is its file's: publish with panel_set before patching";

/// What a patch is told when this pane has no document at all.
///
/// **The same sentence a read of the same pane answers**, from one place so the two verbs cannot
/// drift: a producer that asks what is on a pane and is told nothing has been published must not be
/// told, one request later, that the panel belongs to a file.
pub(crate) const NOTHING_PUBLISHED: &str = "nothing has been published on this console yet";

/// The date the contract file carries right now, or nothing when it is not there or will not say.
///
/// **A stat and not a read**, which is what lets [`PanelState::refresh`] tell a real publication from a
/// raised flag while a connection holds the pane without becoming a second writer. It lives beside
/// the hold because that is the only place the distinction is needed.
pub(super) fn file_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir.join(panel_contract::PANEL_FILE))
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// What a connection closing did to the panel it was on.
///
/// **It exists so the report's `released` counter can mean what it says.** Counting every closure
/// would count the connections that never held anything — a producer that only listed the tools, or
/// one whose hold a later connection had already taken over — and **D13** rates a number that says
/// one thing and counts another worse than no number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Released {
    /// This connection was holding the panel, and its file rules it again.
    Given,
    /// It was holding nothing.
    Nothing,
}

/// What the window's one retirement transition did with this connection's auxiliary consoles.
///
/// **These are effects, not requested numbers.** The window computes them from the live tree after
/// rechecking takeover, then gives the value to [`PanelState::retire`] as the result to commit. A
/// retry receives the value first committed even if it proposes the empty result after the panes
/// have already gone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Retirement {
    pub closed: usize,
    pub preserved: usize,
}

/// The file transport generation observed immediately before retirement commits.
///
/// The value is opaque so a caller cannot synthesize a date and accidentally suppress somebody's
/// later publication. It is live-pane state only: pane closure removes its private directory, and a
/// restored pane receives a fresh identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetirementBaseline {
    file: FileGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FileGeneration {
    Missing,
    Present {
        modified: SystemTime,
        length: u64,
        bytes: Vec<u8>,
        #[cfg(unix)]
        device: u64,
        #[cfg(unix)]
        inode: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FileStamp {
    Missing,
    Present {
        modified: SystemTime,
        length: u64,
        #[cfg(unix)]
        device: u64,
        #[cfg(unix)]
        inode: u64,
    },
}

const RETIRED_CONTENT_PROBE: Duration = Duration::from_millis(500);

pub(super) enum RetirementRefresh {
    NotRetired,
    Read,
    Deferred(Duration),
}

enum RetiredFile {
    Checked(Option<FileGeneration>),
    Deferred(Duration),
}

impl FileGeneration {
    fn stamp(&self) -> FileStamp {
        match self {
            Self::Missing => FileStamp::Missing,
            Self::Present {
                modified,
                length,
                #[cfg(unix)]
                device,
                #[cfg(unix)]
                inode,
                ..
            } => FileStamp::Present {
                modified: *modified,
                length: *length,
                #[cfg(unix)]
                device: *device,
                #[cfg(unix)]
                inode: *inode,
            },
        }
    }

    fn into_panel(self) -> Option<Panel> {
        let Self::Present {
            modified,
            length,
            bytes,
            ..
        } = self
        else {
            return None;
        };
        if length > panel_contract::MAX_BYTES || bytes.len() as u64 != length {
            return None;
        }
        let document = std::str::from_utf8(&bytes).ok()?;
        let mut panel = panel_contract::parse_document(document).ok()?;
        panel.written_at = Some(modified);
        Some(panel)
    }
}

/// One retired producer generation, kept only until the next successful publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RetiredGeneration {
    connection: panel_control::Connection,
    result: Retirement,
}

#[derive(Debug, Default)]
pub(super) enum RetirementState {
    #[default]
    Active,
    Retired {
        generation: FileGeneration,
        producer: RetiredGeneration,
        next_content_probe: Instant,
    },
}

/// Who is driving a panel, and under what name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Holder {
    /// **The connection and not the source**, because the source is a label its own holder chooses:
    /// two connections calling themselves the same thing are two producers, and letting the second
    /// one publish because the name matched is exactly the collision the hold exists to stop.
    connection: panel_control::Connection,
    pub(super) source: String,
    /// Whether the file tried to publish while this connection held.
    ///
    /// **It lives on the hold and not on the panel**, which is the whole of what round 3 fixed: a
    /// bool on `PanelState` was set by any raised look-again flag — and a split, an open tab or the
    /// blind watcher raise one on every pane — so a held panel confessed «the file is not being read»
    /// for the rest of its life over a file nobody wrote. Keyed on the hold and set only when the
    /// file really changed, the confession is as long-lived as the refusal it describes. Not read by
    /// [`PanelState::refresh`]'s own module: the panel asks for it to build the warning.
    pub(super) file_refused: bool,
}

impl PanelState {
    /// Snapshots the current file generation for an intentional retirement.
    ///
    /// Missing is a real generation: there was no file to resurrect. Other metadata failures are
    /// returned so the window can refuse before changing either the panel or its tree.
    pub fn retirement_baseline(dir: &Path) -> io::Result<RetirementBaseline> {
        snapshot(dir).map(|file| RetirementBaseline { file })
    }

    /// Verifies retirement authority without changing the panel or tree.
    ///
    /// `Some` is an idempotent retry whose original result must be returned without touching either.
    pub(crate) fn retirement_status(
        &self,
        connection: panel_control::Connection,
    ) -> Result<Option<Retirement>, String> {
        if let RetirementState::Retired { producer, .. } = self.retirement
            && producer.connection == connection
        {
            return Ok(Some(producer.result));
        }
        let Some(holder) = &self.holder else {
            return Err("this connection does not hold this console's panel".to_owned());
        };
        if holder.connection != connection {
            return Err(held_by_somebody_else(&holder.source));
        }
        Ok(None)
    }

    pub(crate) fn was_retired_by(&self, connection: panel_control::Connection) -> bool {
        matches!(
            self.retirement,
            RetirementState::Retired { producer, .. } if producer.connection == connection
        )
    }

    pub(super) fn is_retired(&self) -> bool {
        matches!(self.retirement, RetirementState::Retired { .. })
    }

    pub(super) fn retired_probe_is_due(&self) -> bool {
        matches!(
            self.retirement,
            RetirementState::Retired {
                next_content_probe,
                ..
            } if Instant::now() >= next_content_probe
        )
    }

    fn changed_retired_file(&mut self, dir: &Path) -> io::Result<RetiredFile> {
        let RetirementState::Retired {
            generation,
            next_content_probe,
            ..
        } = &mut self.retirement
        else {
            return Ok(RetiredFile::Checked(None));
        };
        let now = Instant::now();
        if file_stamp(dir)? == generation.stamp() && now < *next_content_probe {
            return Ok(RetiredFile::Deferred(
                next_content_probe.saturating_duration_since(now),
            ));
        }
        *next_content_probe = now + RETIRED_CONTENT_PROBE;
        snapshot(dir)
            .map(|current| RetiredFile::Checked((current != *generation).then_some(current)))
    }

    /// Keeps the retired file invisible, or commits the first later valid file snapshot.
    pub(super) fn refresh_after_retirement(&mut self, dir: &Path) -> RetirementRefresh {
        if !self.is_retired() {
            return RetirementRefresh::NotRetired;
        }
        let changed = match self.changed_retired_file(dir) {
            Ok(RetiredFile::Deferred(delay)) => return RetirementRefresh::Deferred(delay),
            Ok(RetiredFile::Checked(None)) => {
                self.problem = None;
                return RetirementRefresh::Read;
            }
            // Retirement ends only after a valid later publication commits. A broken attempt stays
            // invisible and will be tried again on the next notice or blind rescan.
            Ok(RetiredFile::Checked(Some(changed))) => changed,
            Err(_) => {
                self.problem = None;
                return RetirementRefresh::Read;
            }
        };
        if let Some(panel) = changed.into_panel() {
            self.file_seen = panel.written_at;
            self.commit(panel);
            self.layout = None;
        }
        self.problem = None;
        RetirementRefresh::Read
    }

    /// Takes a document that arrived over the channel, if the connection handing it over may.
    ///
    /// **The date is stamped here, on arrival.** The contract's parser leaves it empty because a
    /// delivered document has none of its own, and leaving it so would make a held panel never go
    /// stale — an agent that hung with its connection open would show the same summary for ever,
    /// which is precisely the failure the panel exists to expose.
    ///
    /// **Refusing names the holder rather than saying «busy»**, because the caller's next move
    /// depends on which of the two it is: its own earlier connection, or somebody else's.
    pub fn deliver(
        &mut self,
        connection: panel_control::Connection,
        source: &str,
        panel: Panel,
        now: SystemTime,
    ) -> Result<(), String> {
        if let Some(holder) = &self.holder
            && holder.connection != connection
        {
            return Err(held_by_somebody_else(&holder.source));
        }

        self.overlay.replace(connection, source);
        self.hook_source = None;
        self.holder = Some(Holder {
            connection,
            source: source.to_owned(),
            file_refused: false,
        });
        self.commit(Panel {
            written_at: Some(now),
            ..panel
        });
        self.problem = None;
        self.layout = None;
        Ok(())
    }

    /// Applies a patch to what this connection published, **or changes nothing at all**.
    ///
    /// **It needs the hold, and one check answers two of the cases the ticket names.** `deliver`
    /// sets the holder and the document together, so there is no state where a connection holds a
    /// pane and there is nothing to patch: *nothing has been published yet* and *the file rules this
    /// panel* are the same refusal. The sentence names the second, because that is the one a
    /// producer can act on, and it says the move — which is what keeps a publication the ground
    /// state a patch is measured against.
    ///
    /// **Either every op applies or none does**, and the ceiling is part of that: the patched
    /// document is written out before it is installed, so a patch whose own request is small and
    /// whose result is over [`panel_contract::MAX_BYTES`] is refused with the contract's own word
    /// and the panel on screen does not change.
    ///
    /// **The file's confession is left exactly where it is, unlike a publication.** `deliver` rebuilds
    /// the hold and clears `file_refused` with it; a patch does not, and that is the right way round
    /// rather than an oversight. The notice says *somebody wrote this pane's file while you held it,
    /// and it was not read* — republishing makes that stale, because the producer has just replaced
    /// everything the file might have said; changing one line does not, so the notice is still true
    /// and stays as long as what it describes.
    ///
    /// **A patch that succeeds stamps the date, and a patch that is refused stamps nothing.** A
    /// producer that fails to patch for an hour still goes stale, which is the failure the panel
    /// exists to expose. And a refusal never touches `problem`: that field says a file could not be
    /// read, and here nothing was read and nothing was lost — the producer is the one told, over the
    /// channel, which is the thing this transport gives that the file does not.
    pub fn patch(
        &mut self,
        connection: panel_control::Connection,
        ops: &Patch,
        now: SystemTime,
    ) -> Result<usize, String> {
        // **Nothing published is answered first, and in the words a read of the same pane uses.**
        let Some(published) = self.last_good.as_ref() else {
            return Err(NOTHING_PUBLISHED.to_owned());
        };
        let Some(holder) = &self.holder else {
            return Err(ITS_FILES.to_owned());
        };
        if holder.connection != connection {
            return Err(held_by_somebody_else(&holder.source));
        }

        let patched = published
            .patched_with(ops)
            .map_err(|problem| problem.to_string())?;
        // **Weighed by writing it out**, because the ceiling a patched document has to meet is the
        // one `serialize` itself enforces: asking any other question here would be a second ceiling
        // for one contract. **The size comes back to the caller instead of the bytes**, so the
        // measurement can say how large the document behind a patch's cost was without serializing
        // it a second time inside the same frame — which would publish this path as costing about
        // twice what it does.
        let written = panel_contract::serialize(&patched).map_err(|problem| problem.to_string())?;

        self.commit(Panel {
            written_at: Some(now),
            ..patched
        });
        self.problem = None;
        self.layout = None;
        Ok(written.len())
    }

    /// Intentionally removes the document published by this connection.
    ///
    /// **This differs from [`Self::release`] in the only way that matters:** a disconnect keeps the
    /// last committed document as evidence, while an explicit retirement clears the interface copy,
    /// the off-frame reader and every route by which an old action could still be pressed. A foreign
    /// connection changes nothing, even when it uses the same source name.
    ///
    /// The tree is the window's, so it computes `result` before this infallible state swap. Repeating
    /// the request on the same connection returns the original result. Any later successful channel
    /// or file publication goes through [`PanelState::commit`] and ends that idempotent generation.
    pub fn retire(
        &mut self,
        connection: panel_control::Connection,
        baseline: RetirementBaseline,
        result: Retirement,
    ) -> Result<Retirement, String> {
        if let Some(result) = self.retirement_status(connection)? {
            return Ok(result);
        }

        self.last_good = None;
        self.committed.clear();
        self.layout = None;
        self.problem = None;
        self.drawn = None;
        self.holder = None;
        self.overlay.clear();
        self.file_seen = match &baseline.file {
            FileGeneration::Missing => None,
            FileGeneration::Present { modified, .. } => Some(*modified),
        };
        self.retirement = RetirementState::Retired {
            generation: baseline.file,
            producer: RetiredGeneration { connection, result },
            next_content_probe: Instant::now() + RETIRED_CONTENT_PROBE,
        };
        Ok(result)
    }

    /// Gives the panel back to its file, if this is the connection that was holding it.
    ///
    /// **What was published stays on screen.** The agent is gone and what it last said is still the
    /// truest thing anybody knows about that pane; blanking it would throw away information in order
    /// to be tidy. The file takes over from the next publication, not from this instant.
    pub fn release(&mut self, connection: panel_control::Connection) -> Released {
        if self
            .holder
            .as_ref()
            .is_none_or(|held| held.connection != connection)
        {
            return Released::Nothing;
        }
        self.holder = None;
        self.overlay.clear();
        self.layout = None;
        Released::Given
    }

    /// The source driving this panel, when one is.
    pub fn held_by(&self) -> Option<&str> {
        self.holder.as_ref().map(|held| held.source.as_str())
    }

    pub(super) fn displayed_source(&self) -> Option<&str> {
        self.hook_source.as_deref().or_else(|| self.held_by())
    }

    /// Whether the connection holding this panel has had a file publication refused.
    ///
    /// The caller is `Pane::refresh_its_panel`, which hands the look-again notice back only when this
    /// is true: a flag raised by a rearm or the blind watcher must not keep this panel looking every
    /// frame for the rest of the hold, and it must not turn into a confession either.
    pub(crate) fn refused_the_file(&self) -> bool {
        self.holder.as_ref().is_some_and(|held| held.file_refused)
    }
}

fn snapshot(dir: &Path) -> io::Result<FileGeneration> {
    let path = dir.join(panel_contract::PANEL_FILE);
    let file = match open_snapshot(&path) {
        Ok(file) => file,
        Err(problem) if problem.kind() == io::ErrorKind::NotFound => {
            return Ok(FileGeneration::Missing);
        }
        Err(problem) => return Err(problem),
    };
    // Metadata and bytes come from the same open file description. Reading the path first and
    // opening it afterwards would let an atomic rename splice the old metadata to the new bytes.
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the panel path is not a regular file",
        ));
    }
    let mut bytes = Vec::with_capacity(
        metadata
            .len()
            .min(panel_contract::MAX_BYTES + 1)
            .try_into()
            .expect("the panel ceiling fits in memory"),
    );
    file.take(panel_contract::MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    Ok(FileGeneration::Present {
        modified: metadata.modified()?,
        length: metadata.len(),
        bytes,
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(unix)]
        inode: metadata.ino(),
    })
}

fn file_stamp(dir: &Path) -> io::Result<FileStamp> {
    match std::fs::metadata(dir.join(panel_contract::PANEL_FILE)) {
        Ok(metadata) => Ok(FileStamp::Present {
            modified: metadata.modified()?,
            length: metadata.len(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        }),
        Err(problem) if problem.kind() == io::ErrorKind::NotFound => Ok(FileStamp::Missing),
        Err(problem) => Err(problem),
    }
}

#[cfg(unix)]
fn open_snapshot(path: &Path) -> io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_snapshot(path: &Path) -> io::Result<std::fs::File> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the panel path is not a regular file",
        ));
    }
    std::fs::File::open(path)
}
