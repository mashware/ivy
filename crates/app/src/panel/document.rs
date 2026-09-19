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

impl PanelState {
    /// A capability that reads the last complete producer document without keeping this pane alive.
    pub(crate) fn reader(&self) -> Reader {
        self.committed.reader()
    }

    /// Installs one complete producer document in interface-owned and connection-visible state.
    pub(super) fn commit(&mut self, panel: Panel) {
        let panel = Arc::new(panel);
        self.last_good = Some(Arc::clone(&panel));
        self.committed.replace(panel);
    }

    /// Writes the producer's last committed document without taking a hold.
    pub fn document(&self) -> Result<String, String> {
        self.reader()
            .read()
            .expect("the state owns the strong document capability")
    }
}
