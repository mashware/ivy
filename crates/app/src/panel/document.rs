//! The immutable producer document shared with off-frame readers.

use std::sync::{Arc, Mutex, Weak};

use panel_contract::Panel;

use super::{PanelState, driving::NOTHING_PUBLISHED};

pub(super) struct Committed {
    shared: Arc<Mutex<Shared>>,
}

struct Shared {
    alive: bool,
    panel: Option<Arc<Panel>>,
}

impl Default for Committed {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(Shared {
                alive: true,
                panel: None,
            })),
        }
    }
}

impl Committed {
    pub(super) fn replace(&self, panel: Arc<Panel>) {
        self.shared
            .lock()
            .expect("the committed panel pointer is only cloned or replaced")
            .panel = Some(panel);
    }

    /// Removes the committed document from every reader without ending the pane's lifetime.
    ///
    /// **Retirement is not pane closure.** Existing readers keep the weak capability and may read a
    /// later publication, but from this swap until that publication they get the contract's honest
    /// «nothing has been published» answer. Keeping this beside [`Self::replace`] makes the
    /// interface-owned document and the off-frame document one transition rather than two callers'
    /// responsibility.
    pub(super) fn clear(&self) {
        self.shared
            .lock()
            .expect("the committed panel pointer is only cloned or replaced")
            .panel = None;
    }

    pub(super) fn reader(&self) -> Reader {
        Reader {
            shared: Arc::downgrade(&self.shared),
        }
    }
}

impl Drop for Committed {
    fn drop(&mut self) {
        self.shared
            .lock()
            .expect("the committed panel pointer is only cloned or replaced")
            .alive = false;
    }
}

/// A weak capability so a connection cannot keep its pane alive.
#[derive(Clone)]
pub(crate) struct Reader {
    shared: Weak<Mutex<Shared>>,
}

impl Reader {
    /// Acquires this read before the pane lifetime ends.
    pub(crate) fn acquire(&self) -> Option<Acquired> {
        let shared = self.shared.upgrade()?;
        let alive = shared
            .lock()
            .expect("the committed panel pointer is only cloned or replaced")
            .alive;
        alive.then_some(Acquired(shared))
    }

    /// `None` means the pane lifetime ended before this request acquired it.
    pub(crate) fn read(&self) -> Option<Result<String, String>> {
        Some(self.acquire()?.read())
    }
}

/// One read admitted while its pane still existed.
pub(crate) struct Acquired(Arc<Mutex<Shared>>);

impl Acquired {
    /// Selects and serializes one complete committed version.
    pub(crate) fn read(self) -> Result<String, String> {
        let panel = self
            .0
            .lock()
            .expect("the committed panel pointer is only cloned or replaced")
            .panel
            .clone();
        match panel {
            Some(panel) => {
                panel_contract::serialize(panel.as_ref()).map_err(|problem| problem.to_string())
            }
            None => Err(NOTHING_PUBLISHED.to_owned()),
        }
    }
}

/// Retirement clears the producer document, its observation, and the geometry painted for it.
pub(super) fn clear(state: &mut PanelState) {
    state.published_by = panel_control::PublicationOrigin::Unknown;
    state.last_good = None;
    state.committed.clear();
    #[cfg(windows)]
    state.publish_windows_document(None);
    state.layout = None;
    state.drawn = None;
}

impl PanelState {
    /// A capability that reads the last complete producer document without keeping this pane alive.
    pub(crate) fn reader(&self) -> Reader {
        self.committed.reader()
    }

    /// Installs one complete producer document in interface-owned and connection-visible state.
    pub(super) fn commit(&mut self, panel: Panel) {
        #[cfg(windows)]
        self.publish_windows_document(Some(&panel));
        let panel = Arc::new(panel);
        self.last_good = Some(Arc::clone(&panel));
        self.committed.replace(panel);
        // A successful publication starts a new generation. From this point a retry of an earlier
        // retirement is not idempotent: it is an attempt to retire this new publication, and must
        // pass the holder check again.
        self.retirement = super::RetirementState::Active;
    }

    /// Writes the producer's last committed document without taking a hold.
    pub fn document(&self) -> Result<String, String> {
        self.reader()
            .read()
            .expect("the state owns the strong document capability")
    }

    #[cfg(windows)]
    pub(crate) fn install_windows_document_publisher(
        &mut self,
        publisher: panel_control::windows_services::DocumentPublisher,
    ) {
        self.windows_document = Some(publisher);
        self.publish_windows_document(self.last_good.as_deref());
    }

    #[cfg(windows)]
    fn publish_windows_document(&self, panel: Option<&Panel>) {
        let Some(publisher) = &self.windows_document else {
            return;
        };
        let result = match panel {
            Some(panel) => match panel_contract::serialize(panel) {
                Ok(document) => publisher.publish(document),
                Err(problem) => publisher.publish_failed(problem.to_string()),
            },
            None => publisher.clear(),
        };
        if let Err(problem) = result {
            eprintln!("Windows panel snapshot was not updated: {problem}");
        }
    }
}

#[cfg(all(test, windows))]
mod windows_control_tests {
    use super::*;

    #[test]
    fn windows_control_document_snapshot_tracks_commit_clear_and_failed_serialize() {
        let (reader, publisher) = panel_control::windows_services::DocumentSnapshot::new();
        let mut state = PanelState::default();
        state.install_windows_document_publisher(publisher);
        let panel = panel_contract::parse_document(r#"{"lines":["committed"]}"#).unwrap();
        let canonical = panel_contract::serialize(&panel).unwrap();
        state.commit(panel);
        assert_eq!(reader.read().unwrap(), canonical);

        clear(&mut state);
        assert_eq!(
            reader.read().unwrap_err(),
            NOTHING_PUBLISHED,
            "retirement clears the snapshot instead of leaving a prior commit"
        );

        let mut too_large = panel_contract::parse_document(r#"{"lines":["valid"]}"#).unwrap();
        too_large.phase = Some("x".repeat(panel_contract::MAX_BYTES as usize));
        assert!(panel_contract::serialize(&too_large).is_err());
        state.commit(too_large);
        assert!(
            reader.read().unwrap_err() != NOTHING_PUBLISHED,
            "an unrepresentable commit reports its serialization failure, not a stale document"
        );
    }
}
