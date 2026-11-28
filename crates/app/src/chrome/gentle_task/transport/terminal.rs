//! Terminal failures cannot wait for a spare control-queue slot.
use super::{Arc, Mutex, Wake};
#[derive(Clone)]
pub(super) struct Terminal {
    slot: Arc<Mutex<Option<String>>>,
    failed: Arc<std::sync::atomic::AtomicBool>,
    wake: Wake,
}
impl Terminal {
    pub(super) fn new(wake: Wake) -> Self {
        Self {
            slot: Arc::default(),
            failed: Arc::default(),
            wake,
        }
    }
    pub(super) fn fail(&self, reason: String) {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if !self.failed.swap(true, std::sync::atomic::Ordering::AcqRel) {
            *slot = Some(reason);
            (self.wake)();
        }
    }
    pub(super) fn failed(&self) -> bool {
        self.failed.load(std::sync::atomic::Ordering::Acquire)
    }
    pub(super) fn take(&self) -> Option<String> {
        self.slot.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}
