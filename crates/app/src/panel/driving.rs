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

use std::path::Path;
use std::time::SystemTime;

use panel_contract::Panel;

use super::PanelState;

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

/// Who is driving a panel, and under what name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Holder {
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
            return Err(format!(
                "this panel is held by \"{}\" until that connection closes",
                holder.source
            ));
        }

        self.holder = Some(Holder {
            connection,
            source: source.to_owned(),
            file_refused: false,
        });
        self.last_good = Some(Panel {
            written_at: Some(now),
            ..panel
        });
        self.problem = None;
        self.layout = None;
        Ok(())
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
        self.layout = None;
        Released::Given
    }

    /// The source driving this panel, when one is.
    pub fn held_by(&self) -> Option<&str> {
        self.holder.as_ref().map(|held| held.source.as_str())
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
