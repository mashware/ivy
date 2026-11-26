//! Concrete holder snapshots for callback-free Windows control workers (D90).
use crate::session::Peer;
use crate::windows_services::{ReleaseRegistry, WindowsWake};
use crate::{Connection, Delivery, PublicationOrigin, Reply, What};
use std::sync::{
    Arc, Mutex, TryLockError,
    atomic::{AtomicBool, Ordering},
    mpsc::{SyncSender, sync_channel},
};
use term_core::{WindowsInvocation, WindowsInvocationWitness};

#[derive(Clone)]
struct Hold {
    connection: Connection,
    incarnation: u64,
    source: String,
    witness: Option<WindowsInvocationWitness>,
}

#[derive(Clone, Default)]
pub struct WindowsHooks {
    hold: Arc<Mutex<Option<Hold>>>,
    invocation: Option<WindowsInvocation>,
    revoked: Arc<AtomicBool>,
}

impl WindowsHooks {
    pub fn new(invocation: Option<WindowsInvocation>) -> Self {
        Self {
            hold: Arc::default(),
            invocation,
            revoked: Arc::default(),
        }
    }
    pub fn commit(
        &self,
        connection: Connection,
        incarnation: u64,
        source: &str,
        witness: Option<WindowsInvocationWitness>,
    ) {
        if self.revoked.load(Ordering::Acquire) {
            return;
        }
        *self.hold.lock().unwrap_or_else(|e| e.into_inner()) = Some(Hold {
            connection,
            incarnation,
            source: source.to_owned(),
            witness,
        });
    }
    pub fn clear(&self) {
        *self.hold.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
        self.clear();
    }
    pub(crate) fn attest(&self, peer: &Peer) -> Option<WindowsInvocationWitness> {
        self.invocation
            .as_ref()?
            .attest_with_stop(peer.handle(), &self.revoked)
            .ok()
    }
    pub(crate) fn admit(
        &self,
        peer: Option<&Peer>,
        document: &str,
        sender: &SyncSender<Delivery>,
        releases: &ReleaseRegistry,
        wake: &WindowsWake,
    ) -> Reply {
        if self.revoked.load(Ordering::Acquire) {
            return refused("unauthorized", "this pane's hook authority is revoked");
        }
        let hold = match self.hold.try_lock() {
            Ok(hold) => hold.clone(),
            Err(TryLockError::WouldBlock) => {
                return refused(
                    "snapshot_busy",
                    "this panel's hook snapshot is busy; try again",
                );
            }
            Err(TryLockError::Poisoned(_)) => {
                return refused(
                    "snapshot_busy",
                    "this panel's hook snapshot is busy; try again",
                );
            }
        };
        let Some(hold) = hold else {
            return Reply::PublishUnheld;
        };
        let Some(root) = hold.witness else {
            return refused(
                "holder_unattested",
                &format!(
                    "this panel is held by \"{}\", which is not an attested agent client",
                    hold.source
                ),
            );
        };
        let Some(publisher) = peer.and_then(|peer| self.attest(peer)) else {
            return refused(
                "unauthorized",
                "publisher does not descend from this panel's holding client",
            );
        };
        if !root.same_invocation(&publisher) || !root.current() {
            return refused(
                "unauthorized",
                "publisher does not descend from this panel's holding client",
            );
        }
        let panel = match panel_contract::parse_document(document) {
            Ok(panel) => panel,
            Err(error) => return refused("invalid_document", &error.to_string()),
        };
        let Some(release) = releases.admit(hold.connection) else {
            return refused(
                "queue_full",
                "this panel's holding connection cannot admit another update",
            );
        };
        let (answer, _) = sync_channel(1);
        let delivery = Delivery {
            connection: hold.connection,
            origin: PublicationOrigin::Unknown,
            what: What::Overlay {
                panel,
                holder: hold.connection,
                incarnation: hold.incarnation,
                authority: Arc::new(Proof {
                    root,
                    revoked: Arc::clone(&self.revoked),
                }),
            },
            answer,
            release: Some(release),
        };
        if sender.try_send(delivery).is_err() {
            return refused("queue_full", "this panel's publication queue is full");
        }
        let _ = wake.post();
        Reply::PublishAccepted
    }
}

struct Proof {
    root: WindowsInvocationWitness,
    revoked: Arc<AtomicBool>,
}
impl std::fmt::Debug for Proof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("holding client invocation")
    }
}
impl crate::OverlayAuthority for Proof {
    fn current(&self) -> bool {
        !self.revoked.load(Ordering::Acquire) && self.root.current()
    }
}
fn refused(code: &str, message: &str) -> Reply {
    Reply::PublishRefused {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

#[derive(Clone)]
pub struct WindowsHolder(pub WindowsInvocationWitness);
impl std::fmt::Debug for WindowsHolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("verified Windows holder")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_unattested_holders_are_distinct_and_clear_is_immediate() {
        let hooks = WindowsHooks::new(None);
        let (send, _) = sync_channel(1);
        let releases = ReleaseRegistry::new();
        let wake = WindowsWake::from_hwnd(1).unwrap();
        assert!(matches!(
            hooks.admit(None, "{}", &send, &releases, &wake),
            Reply::PublishUnheld
        ));
        hooks.commit(7, 1, "script", None);
        assert!(
            matches!(hooks.admit(None, "{}", &send, &releases, &wake), Reply::PublishRefused { code, message } if code == "holder_unattested" && message.contains("script"))
        );
        hooks.clear();
        assert!(matches!(
            hooks.admit(None, "{}", &send, &releases, &wake),
            Reply::PublishUnheld
        ));
    }

    #[test]
    fn snapshot_contention_refuses_without_waiting() {
        let hooks = WindowsHooks::new(None);
        let guard = hooks.hold.lock().unwrap();
        let (send, _) = sync_channel(1);
        let releases = ReleaseRegistry::new();
        let wake = WindowsWake::from_hwnd(1).unwrap();
        assert!(
            matches!(hooks.admit(None, "{}", &send, &releases, &wake), Reply::PublishRefused { code, .. } if code == "snapshot_busy")
        );
        drop(guard);
    }

    #[test]
    fn holder_release_waits_for_its_one_admitted_delivery() {
        let registry = ReleaseRegistry::new();
        assert!(registry.reserve(7));
        let token = registry.admit(7).unwrap();
        assert!(registry.admit(7).is_none());
        registry.mark_closed(7);
        assert!(registry.take_closed_ready().is_empty());
        drop(token);
        assert_eq!(registry.take_closed_ready(), vec![7]);
    }
}
