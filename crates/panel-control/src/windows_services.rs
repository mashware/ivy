//! Callback-free capabilities used by the Windows channel workers.
//!
//! The worker side reads bounded snapshots, posts a native message and may inspect a concrete
//! native process scope. App state stays on the UI thread; there is no generic callback surface.

use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, TryLockError};

use crate::listener::Connection;

const CAPACITY: usize = 8;

#[derive(Debug, Clone)]
struct Cell<T> {
    value: Arc<Mutex<Option<T>>>,
    alive: Arc<AtomicBool>,
    max_bytes: usize,
}

impl<T> Cell<T> {
    fn publish(&self, value: T, size: usize) -> io::Result<()> {
        if size > self.max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "snapshot exceeds the panel contract limit",
            ));
        }
        if !self.alive.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "this pane is gone",
            ));
        }
        let mut cell = self
            .value
            .lock()
            .map_err(|_| io::Error::other("snapshot state is unavailable"))?;
        *cell = Some(value);
        Ok(())
    }

    fn clear(&self) -> io::Result<()> {
        let mut cell = self
            .value
            .lock()
            .map_err(|_| io::Error::other("snapshot state is unavailable"))?;
        *cell = None;
        Ok(())
    }

    fn revoke(&self) {
        self.alive.store(false, Ordering::Release);
    }

    fn read(&self) -> Result<Option<T>, &'static str>
    where
        T: Clone,
    {
        if !self.alive.load(Ordering::Acquire) {
            return Err("this pane is gone");
        }
        match self.value.try_lock() {
            Ok(value) => Ok(value.clone()),
            Err(TryLockError::WouldBlock) => Err("this pane's snapshot is busy; try again"),
            Err(TryLockError::Poisoned(_)) => Err("this pane's snapshot is unavailable"),
        }
    }
}

#[derive(Debug, Default)]
struct ReadMetricAtoms {
    read_back: AtomicU64,
    read_refused: AtomicU64,
    largest_serialized: AtomicUsize,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct SnapshotReadMetrics {
    pub read_back: u64,
    pub read_refused: u64,
    pub largest_serialized: usize,
}
#[derive(Debug, Clone)]
pub struct DocumentReader(Cell<DocumentValue>, Arc<ReadMetricAtoms>);
#[derive(Debug, Clone)]
pub struct DocumentPublisher(Cell<DocumentValue>, Arc<ReadMetricAtoms>);
#[derive(Debug, Clone)]
pub struct PlanReader(Cell<PlanValue>);
#[derive(Debug, Clone)]
pub struct PlanPublisher(Cell<PlanValue>);

#[derive(Debug, Clone)]
enum PlanValue {
    Document(String),
    Text(String),
    Failed(String),
}
#[derive(Debug, Clone)]
pub(crate) enum DocumentValue {
    Document(String),
    Failed(String),
}

/// Bounded committed document snapshot shared between UI publisher and channel worker.
pub struct DocumentSnapshot;
impl DocumentSnapshot {
    // The type names the pair; it is never a value of its own.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> (DocumentReader, DocumentPublisher) {
        let cell = Cell {
            value: Arc::new(Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
            max_bytes: panel_contract::MAX_BYTES as usize,
        };
        let metrics = Arc::new(ReadMetricAtoms::default());
        (
            DocumentReader(cell.clone(), Arc::clone(&metrics)),
            DocumentPublisher(cell, metrics),
        )
    }
}
impl DocumentPublisher {
    pub fn is_alive(&self) -> bool {
        self.0.alive.load(Ordering::Acquire)
    }
    pub fn read_metrics(&self) -> SnapshotReadMetrics {
        SnapshotReadMetrics {
            read_back: self.1.read_back.load(Ordering::Relaxed),
            read_refused: self.1.read_refused.load(Ordering::Relaxed),
            largest_serialized: self.1.largest_serialized.load(Ordering::Relaxed),
        }
    }
    pub fn publish(&self, document: String) -> io::Result<()> {
        let n = document.len();
        self.0.publish(DocumentValue::Document(document), n)
    }
    pub fn publish_failed(&self, message: String) -> io::Result<()> {
        let n = message.len();
        self.0.publish(DocumentValue::Failed(message), n)
    }
    pub fn clear(&self) -> io::Result<()> {
        self.0.clear()
    }
    pub fn revoke(&self) {
        self.0.revoke();
    }
}
impl DocumentReader {
    pub fn read(&self) -> Result<String, String> {
        let result = match self.0.read() {
            Ok(Some(DocumentValue::Document(document))) => {
                self.1.read_back.fetch_add(1, Ordering::Relaxed);
                self.1
                    .largest_serialized
                    .fetch_max(document.len(), Ordering::Relaxed);
                Ok(document)
            }
            Ok(Some(DocumentValue::Failed(message))) => Err(message),
            Ok(None) => Err("nothing has been published on this console yet".to_owned()),
            Err(message) => Err(message.to_owned()),
        };
        if result.is_err() {
            self.1.read_refused.fetch_add(1, Ordering::Relaxed);
        }
        result
    }
    #[cfg(test)]
    pub(crate) fn lock_for_test(&self) -> std::sync::MutexGuard<'_, Option<DocumentValue>> {
        self.0.value.lock().unwrap()
    }
}

/// Bounded committed plan snapshot shared between UI publisher and channel worker.
pub struct PlanSnapshot;
impl PlanSnapshot {
    // The type names the pair; it is never a value of its own.
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> (PlanReader, PlanPublisher) {
        let cell = Cell {
            value: Arc::new(Mutex::new(None)),
            alive: Arc::new(AtomicBool::new(true)),
            max_bytes: panel_contract::MAX_BYTES as usize,
        };
        (PlanReader(cell.clone()), PlanPublisher(cell))
    }
}
impl PlanPublisher {
    pub fn is_alive(&self) -> bool {
        self.0.alive.load(Ordering::Acquire)
    }
    pub fn publish_document(&self, document: String) -> io::Result<()> {
        let n = document.len();
        self.0.publish(PlanValue::Document(document), n)
    }
    pub fn publish_text(&self, text: String) -> io::Result<()> {
        let n = text.len();
        self.0.publish(PlanValue::Text(text), n)
    }
    pub fn publish_failed(&self, message: String) -> io::Result<()> {
        let n = message.len();
        self.0.publish(PlanValue::Failed(message), n)
    }
    pub fn clear(&self) -> io::Result<()> {
        self.0.clear()
    }
    pub fn revoke(&self) {
        self.0.revoke();
    }
}
impl PlanReader {
    pub fn read(&self) -> Result<crate::wire::Reply, String> {
        match self.0.read().map_err(str::to_owned)? {
            Some(PlanValue::Document(document)) => Ok(crate::wire::Reply::Document(document)),
            Some(PlanValue::Text(text)) => Ok(crate::wire::Reply::Text(text)),
            Some(PlanValue::Failed(message)) => Ok(crate::wire::Reply::Failed(message)),
            None => Ok(crate::wire::Reply::Text(
                "no plan has been published".into(),
            )),
        }
    }
}

/// A revocable native window capability. Posting is asynchronous and never invokes app code.
#[derive(Clone)]
pub struct WindowsWake {
    hwnd: isize,
    alive: Arc<AtomicBool>,
}
impl WindowsWake {
    /// The caller must pass a live HWND owned by the UI thread.
    pub fn from_hwnd(hwnd: isize) -> io::Result<Self> {
        if hwnd == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid window handle",
            ));
        }
        Ok(Self {
            hwnd,
            alive: Arc::new(AtomicBool::new(true)),
        })
    }
    pub fn revoke(&self) {
        self.alive.store(false, Ordering::Release);
    }
    pub fn post(&self) -> io::Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "window is closed",
            ));
        }
        // SAFETY: PostMessageW copies scalar arguments and does not wait for dispatch. The HWND is
        // supplied and revoked by the UI owner; no pane or Rust pointer crosses this boundary.
        let ok = unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                self.hwnd as _,
                windows_sys::Win32::UI::WindowsAndMessaging::WM_PAINT,
                0,
                0,
            )
        };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Default)]
struct Slot {
    id: AtomicU64,
    state: AtomicU64,
}
const CLOSED: u64 = 1 << 63;
const PENDING_MASK: u64 = !CLOSED;
const CLAIMED: u64 = u64::MAX;

/// Fixed-capacity storage for connection close notifications and admitted UI work.
#[derive(Clone)]
pub struct ReleaseRegistry(Arc<RegistryInner>);
#[derive(Default)]
struct RegistryInner {
    slots: [Slot; CAPACITY],
    revoked: AtomicBool,
}
impl Default for ReleaseRegistry {
    fn default() -> Self {
        Self(Arc::new(RegistryInner::default()))
    }
}
impl ReleaseRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn reserve(&self, connection: Connection) -> bool {
        if self.0.revoked.load(Ordering::Acquire) {
            return false;
        }
        let Some(encoded) = connection.checked_add(1) else {
            return false;
        };
        if encoded == CLAIMED {
            return false;
        }
        for slot in &self.0.slots {
            if slot
                .id
                .compare_exchange(0, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                slot.state.store(0, Ordering::Relaxed);
                slot.id.store(encoded, Ordering::Release);
                if self.0.revoked.load(Ordering::Acquire) {
                    slot.state.fetch_or(CLOSED, Ordering::AcqRel);
                    return false;
                }
                return true;
            }
        }
        false
    }
    pub fn admit_delivery(&self, connection: Connection) -> bool {
        if self.0.revoked.load(Ordering::Acquire) {
            return false;
        }
        let Some(slot) = self.slot(connection) else {
            return false;
        };
        let mut old = slot.state.load(Ordering::Acquire);
        loop {
            if old != 0 {
                return false;
            }
            match slot.state.compare_exchange_weak(
                old,
                old + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    if self.0.revoked.load(Ordering::Acquire) {
                        self.delivery_completed(connection);
                        return false;
                    }
                    return true;
                }
                Err(now) => old = now,
            }
        }
    }
    pub fn admit(&self, connection: Connection) -> Option<ReleaseToken> {
        self.admit_delivery(connection).then(|| ReleaseToken {
            registry: self.clone(),
            connection,
        })
    }
    pub fn delivery_completed(&self, connection: Connection) {
        if let Some(slot) = self.slot(connection) {
            let _ = slot
                .state
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |s| {
                    if s & PENDING_MASK != 0 {
                        Some(s - 1)
                    } else {
                        None
                    }
                });
        }
    }
    pub fn mark_closed(&self, connection: Connection) {
        if let Some(slot) = self.slot(connection) {
            slot.state.fetch_or(CLOSED, Ordering::Release);
        }
    }
    /// Returns closed connection IDs ready for UI-side holder cleanup; each is emitted once.
    pub fn take_closed_ready(&self) -> Vec<Connection> {
        let mut ready = Vec::new();
        for slot in &self.0.slots {
            let encoded = slot.id.load(Ordering::Acquire);
            if encoded == 0 || encoded == CLAIMED || slot.state.load(Ordering::Acquire) != CLOSED {
                continue;
            }
            if slot
                .id
                .compare_exchange(encoded, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                slot.state.store(0, Ordering::Relaxed);
                slot.id.store(0, Ordering::Release);
                ready.push(encoded - 1);
            }
        }
        ready
    }
    pub fn revoke(&self) {
        self.0.revoked.store(true, Ordering::Release);
        // Race each slot CAS against closure: admitted work retains its pending count, while
        // a worker that passed the stop flag cannot acquire an idle slot after this cutoff.
        for slot in &self.0.slots {
            slot.state.fetch_or(CLOSED, Ordering::AcqRel);
        }
    }
    #[cfg(test)]
    pub(crate) fn is_closed_pending(&self, connection: Connection) -> bool {
        self.slot(connection)
            .is_some_and(|slot| slot.state.load(Ordering::Acquire) & CLOSED != 0)
    }
    fn slot(&self, connection: Connection) -> Option<&Slot> {
        self.0
            .slots
            .iter()
            .find(|slot| slot.id.load(Ordering::Acquire) == connection.saturating_add(1))
    }
}

/// Ownership token that keeps a connection's close slot pending through deferred UI work.
pub struct ReleaseToken {
    registry: ReleaseRegistry,
    connection: Connection,
}
impl std::fmt::Debug for ReleaseToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReleaseToken")
            .field("connection", &self.connection)
            .finish_non_exhaustive()
    }
}
impl Drop for ReleaseToken {
    fn drop(&mut self) {
        self.registry.delivery_completed(self.connection);
    }
}

/// Immutable Windows-only worker capabilities.
#[derive(Clone)]
pub struct WindowsServices {
    pub(crate) document: DocumentReader,
    pub(crate) plan: PlanReader,
    pub(crate) wake: WindowsWake,
    pub(crate) releases: ReleaseRegistry,
    pub(crate) publication_scope: Option<Arc<term_core::ForegroundScope>>,
    pub(crate) hooks: crate::WindowsHooks,
}
impl WindowsServices {
    pub fn new(hwnd: isize) -> io::Result<(Self, DocumentPublisher, PlanPublisher)> {
        let (document, document_publisher) = DocumentSnapshot::new();
        let (plan, plan_publisher) = PlanSnapshot::new();
        let services = Self {
            document,
            plan,
            wake: WindowsWake::from_hwnd(hwnd)?,
            releases: ReleaseRegistry::new(),
            publication_scope: None,
            hooks: crate::WindowsHooks::new(None),
        };
        Ok((services, document_publisher, plan_publisher))
    }
    /// Diagnostic ancestry uses only native handles; no app callback can hold a joined worker.
    pub fn with_publication_scope(
        mut self,
        scope: Option<Arc<term_core::ForegroundScope>>,
    ) -> Self {
        self.publication_scope = scope;
        self
    }
    pub fn with_hooks(mut self, hooks: crate::WindowsHooks) -> Self {
        self.hooks = hooks;
        self
    }
    pub fn release_registry(&self) -> ReleaseRegistry {
        self.releases.clone()
    }
    pub fn revoke(&self) {
        self.hooks.revoke();
        self.document.0.revoke();
        self.plan.0.revoke();
        self.wake.revoke();
        self.releases.revoke();
    }
}

/// A scope classifies only a retained kernel peer, with revocation checked on both sides.
pub(crate) fn publication_origin(
    scope: &term_core::ForegroundScope,
    peer: &crate::DiagnosticPeer,
) -> crate::PublicationOrigin {
    use crate::PublicationOrigin;
    if !peer.is_alive() || !scope.is_open() {
        return PublicationOrigin::Unknown;
    }
    let origin = match scope.classify_ancestry(peer.pid()) {
        term_core::Ancestry::Inside => PublicationOrigin::Inside,
        term_core::Ancestry::Outside => PublicationOrigin::Outside,
        term_core::Ancestry::Unknown => PublicationOrigin::Unknown,
    };
    if peer.is_alive() && scope.is_open() {
        origin
    } else {
        PublicationOrigin::Unknown
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) struct TestWindow(windows_sys::Win32::Foundation::HWND);
    impl TestWindow {
        pub(crate) fn hwnd(&self) -> isize {
            self.0 as isize
        }
    }
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(self.0);
            }
        }
    }
    pub(crate) fn test_message_window() -> TestWindow {
        use std::ptr::{null, null_mut};
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                null_mut(),
                null(),
            )
        };
        assert!(!hwnd.is_null(), "create a message-only static test window");
        TestWindow(hwnd)
    }

    #[test]
    fn release_slots_bound_capacity_and_require_ui_completion_before_reuse() {
        let registry = ReleaseRegistry::new();
        for id in 0..CAPACITY as u64 {
            assert!(registry.reserve(id));
        }
        assert!(!registry.reserve(99));
        assert!(registry.admit_delivery(3));
        assert!(
            !registry.admit_delivery(3),
            "a timed-out request must still occupy its slot"
        );
        registry.mark_closed(3);
        assert!(
            registry.take_closed_ready().is_empty(),
            "close must wait for admitted UI work"
        );
        registry.delivery_completed(3);
        assert_eq!(registry.take_closed_ready(), vec![3]);
        assert!(registry.reserve(99));
    }

    #[test]
    fn revocation_refuses_new_admissions_and_keeps_admitted_work_pending() {
        let registry = ReleaseRegistry::new();
        assert!(registry.reserve(1));
        assert!(registry.reserve(2));
        let admitted = registry.admit(1).unwrap();
        registry.revoke();
        assert!(!registry.reserve(3));
        assert!(registry.admit(2).is_none());
        assert_eq!(registry.take_closed_ready(), vec![2]);
        drop(admitted);
        assert_eq!(registry.take_closed_ready(), vec![1]);
        assert!(registry.take_closed_ready().is_empty());
    }

    #[test]
    fn registry_churn_does_not_accumulate_slots() {
        let registry = ReleaseRegistry::new();
        for id in 0..10_000 {
            assert!(registry.reserve(id));
            let token = registry.admit(id).expect("one request may be admitted");
            assert!(
                registry.admit(id).is_none(),
                "a second outstanding request was admitted"
            );
            registry.mark_closed(id);
            assert!(registry.take_closed_ready().is_empty());
            drop(token);
            assert_eq!(registry.take_closed_ready(), vec![id]);
        }
        for id in 20_000..20_000 + CAPACITY as u64 {
            assert!(registry.reserve(id));
        }
        assert!(!registry.reserve(30_000));
    }

    #[test]
    fn snapshot_reader_is_bounded_and_revocable() {
        let (reader, publisher) = DocumentSnapshot::new();
        publisher.publish("doc".into()).unwrap();
        assert_eq!(reader.read().unwrap(), "doc");
        assert!(
            publisher
                .publish("x".repeat(panel_contract::MAX_BYTES as usize + 1))
                .is_err()
        );
        publisher.revoke();
        assert_eq!(reader.read().unwrap_err(), "this pane is gone");
        let metrics = publisher.read_metrics();
        assert_eq!(metrics.read_back, 1);
        assert_eq!(metrics.read_refused, 1);
        assert_eq!(metrics.largest_serialized, 3);
    }

    #[test]
    fn snapshot_reads_fail_fast_while_the_ui_publishes() {
        let (reader, publisher) = DocumentSnapshot::new();
        publisher.publish("committed".into()).unwrap();
        let held = reader.0.value.lock().unwrap();
        assert_eq!(
            reader.read().unwrap_err(),
            "this pane's snapshot is busy; try again"
        );
        drop(held);
        assert_eq!(reader.read().unwrap(), "committed");
    }

    #[test]
    fn native_wake_post_does_not_wait_for_window_dispatch() {
        let window = test_message_window();
        let wake = WindowsWake::from_hwnd(window.hwnd()).unwrap();
        let (sent, received) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || sent.send(wake.post()).unwrap());
        received
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("PostMessageW must not wait for dispatch")
            .unwrap();
        worker.join().unwrap();
    }
}
