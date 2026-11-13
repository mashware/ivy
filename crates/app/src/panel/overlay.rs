//! A hook may repaint only the live client invocation that owns the current hold.

use panel_contract::{Attention, Panel};
use panel_control::{Connection, Delivery, Reply, What, session::Peer};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use term_core::ForegroundScope;

#[derive(Clone)]
struct Witness {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    root: crate::client_attestation::Root,
}

impl Witness {
    fn current(&self, scope: &ForegroundScope) -> bool {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        return self.root.current(scope);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = scope;
            false
        }
    }

    fn same(&self, other: &Self) -> bool {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        return self.root.same_invocation(&other.root);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = other;
            false
        }
    }
}

#[derive(Clone)]
struct Hold {
    connection: Connection,
    incarnation: u64,
    source: String,
    root: Option<Witness>,
}

#[derive(Default)]
struct Snapshot {
    incarnation: u64,
    hold: Option<Hold>,
}

pub(super) struct HoldCell(Arc<Mutex<Snapshot>>, bool);

impl Default for HoldCell {
    fn default() -> Self {
        Self(Arc::default(), true)
    }
}

impl Clone for HoldCell {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0), false)
    }
}

impl Drop for HoldCell {
    fn drop(&mut self) {
        if self.1 {
            self.clear();
        }
    }
}

impl HoldCell {
    pub(super) fn replace(&self, connection: Connection, source: &str) {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        state.incarnation = state
            .incarnation
            .checked_add(1)
            .expect("hold incarnation exhausted");
        state.hold = Some(Hold {
            connection,
            incarnation: state.incarnation,
            source: source.to_owned(),
            root: None,
        });
    }

    pub(super) fn clear(&self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).hold = None;
    }

    fn snapshot(&self) -> Option<Hold> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .hold
            .clone()
    }
}

pub(crate) struct Authority {
    scope: ForegroundScope,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    programs: crate::client_attestation::Programs,
}

impl Authority {
    pub(crate) fn new(scope: ForegroundScope) -> Arc<Self> {
        Arc::new(Self {
            scope,
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            programs: crate::client_attestation::Programs::discover(),
        })
    }

    fn attest(&self, peer: &Peer) -> Option<Witness> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        return self
            .programs
            .root(&self.scope, peer)
            .ok()
            .map(|root| Witness { root });
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = peer;
            None
        }
    }

    pub(crate) fn capture(&self, state: &super::PanelState, connection: Connection, peer: &Peer) {
        let root = self.attest(peer);
        let mut snapshot = state.overlay.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(hold) = &mut snapshot.hold
            && hold.connection == connection
        {
            hold.root = root;
        }
    }
}

struct Proof {
    root: Witness,
    authority: Arc<Authority>,
}

impl std::fmt::Debug for Proof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("holding client invocation")
    }
}

impl panel_control::listener::OverlayAuthority for Proof {
    fn current(&self) -> bool {
        self.root.current(&self.authority.scope)
    }
}

pub(crate) fn service(
    state: &super::PanelState,
    authority: Arc<Authority>,
    sender: SyncSender<Delivery>,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> panel_control::listener::PublishHandler {
    let hold = state.overlay.clone();
    Arc::new(
        move |peer: Peer, _connection: Connection, document: String| {
            admit(&hold, &authority, &sender, &peer, &document, &*wake)
        },
    )
}

fn refused(code: &str, message: impl Into<String>) -> Reply {
    Reply::PublishRefused {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn admit(
    cell: &HoldCell,
    authority: &Arc<Authority>,
    sender: &SyncSender<Delivery>,
    peer: &Peer,
    document: &str,
    wake: &dyn Fn(),
) -> Reply {
    let Some(hold) = cell.snapshot() else {
        return Reply::PublishUnheld;
    };
    let Some(root) = &hold.root else {
        return refused(
            "holder_unattested",
            format!(
                "this panel is held by \"{}\", which is not an attested agent client",
                hold.source
            ),
        );
    };
    let Some(publisher) = authority.attest(peer) else {
        return refused(
            "unauthorized",
            "publisher does not descend from this panel's holding client",
        );
    };
    if !root.same(&publisher) || !root.current(&authority.scope) {
        return refused(
            "unauthorized",
            "publisher does not descend from this panel's holding client",
        );
    }
    let panel = match panel_contract::parse_document(document) {
        Ok(panel) => panel,
        Err(why) => return refused("invalid_document", why.to_string()),
    };
    let proof = Arc::new(Proof {
        root: root.clone(),
        authority: Arc::clone(authority),
    });
    let (answer, _) = sync_channel(1);
    match sender.try_send(Delivery {
        connection: hold.connection,
        what: What::Overlay {
            panel,
            holder: hold.connection,
            incarnation: hold.incarnation,
            authority: proof,
        },
        answer,
    }) {
        Ok(()) => {
            wake();
            Reply::PublishAccepted
        }
        Err(_) => refused("queue_full", "this panel's publication queue is full"),
    }
}

impl super::PanelState {
    pub(crate) fn accepts_overlay(&self, connection: Connection, incarnation: u64) -> bool {
        self.overlay
            .snapshot()
            .is_some_and(|hold| hold.connection == connection && hold.incarnation == incarnation)
    }

    pub(crate) fn apply_overlay(&mut self, hook: Panel, now: SystemTime) -> Result<(), String> {
        let mut merged = self.last_good.as_deref().ok_or("no held document")?.clone();
        merged.lines = hook.lines;
        merged.attention = hook.attention;
        merged.phase = hook.phase;
        merged.updated_at = hook.updated_at;
        merged.stale_after = hook.stale_after;
        merged.written_at = Some(now);
        if merged.attention != Some(Attention::Wait) {
            merged.actions.clear();
        }
        panel_contract::serialize(&merged).map_err(|e| e.to_string())?;
        self.hook_source = self.held_by().map(|source| format!("{source} · hook"));
        self.commit(merged);
        self.problem = None;
        self.layout = None;
        self.drawn = None;
        Ok(())
    }

    pub(crate) fn overlay_refused(&mut self) {
        // The admission reply cannot describe a later refusal, so the pane must confess it.
        self.hook_source = self
            .held_by()
            .map(|source| format!("{source} · hook update refused"));
        self.layout = None;
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "overlay/tests.rs"]
mod native_tests;

#[cfg(test)]
mod merge_tests {
    use super::*;
    use panel_contract::parse_document;

    fn held() -> super::super::PanelState {
        let mut state = super::super::PanelState::default();
        state.deliver(7, "flow", parse_document(r#"{"header":false,"milestone":"m","deliverable":"d","attention":"wait","actions":[{"label":"Approve","send":"yes"}],"lines":[{"key":"old","text":"Before"}]}"#).unwrap(), SystemTime::now()).unwrap();
        state
    }

    #[test]
    fn overlay_clears_wait_and_buttons_but_keeps_holder_metadata() {
        let mut state = held();
        let hook = parse_document(r#"{"header":true,"milestone":"attack","phase":"build","actions":[{"label":"Other","send":"evil"}],"lines":["After"]}"#).unwrap();
        state.apply_overlay(hook, SystemTime::now()).unwrap();
        let panel = state.last_good.as_deref().unwrap();
        assert!(!panel.header);
        assert_eq!(panel.milestone.as_deref(), Some("m"));
        assert_eq!(panel.deliverable.as_deref(), Some("d"));
        assert!(panel.attention.is_none());
        assert!(panel.actions.is_empty());
        assert_eq!(state.held_by(), Some("flow"));
        assert_eq!(state.displayed_source(), Some("flow · hook"));
        assert!(state.attention(SystemTime::now()).is_none());
        assert!(state.document().unwrap().contains("After"));
    }

    #[test]
    fn waiting_overlay_keeps_only_current_holder_actions_and_patch_uses_overlay() {
        let mut state = held();
        state.apply_overlay(parse_document(r#"{"attention":"wait","lines":[{"key":"new","text":"After"}],"actions":[{"label":"Inject","send":"evil"}]}"#).unwrap(), SystemTime::now()).unwrap();
        assert_eq!(state.last_good.as_deref().unwrap().actions[0].send(), "yes");
        let ops = panel_contract::parse_ops(
            &serde_json::from_str(
                r#"[{"op":"replace","key":"new","line":{"key":"new","text":"Patched"}}]"#,
            )
            .unwrap(),
        );
        let ops = ops.unwrap();
        state.patch(7, &ops, SystemTime::now()).unwrap();
        assert!(state.document().unwrap().contains("Patched"));
        assert!(!state.document().unwrap().contains("Before"));
    }

    #[test]
    fn new_set_release_and_retire_invalidate_the_queued_hold_generation() {
        let mut state = held();
        let initial = state.overlay.snapshot().unwrap();
        assert!(state.accepts_overlay(7, initial.incarnation));
        state
            .deliver(
                7,
                "flow",
                parse_document(r#"{"lines":["Again"]}"#).unwrap(),
                SystemTime::now(),
            )
            .unwrap();
        assert!(!state.accepts_overlay(7, initial.incarnation));
        assert_eq!(state.displayed_source(), Some("flow"));
        let current = state.overlay.snapshot().unwrap();
        state.release(7);
        assert!(!state.accepts_overlay(7, current.incarnation));
        assert!(state.document().unwrap().contains("Again"));
        state
            .deliver(
                7,
                "flow",
                parse_document(r#"{"lines":["Last"]}"#).unwrap(),
                SystemTime::now(),
            )
            .unwrap();
        let last = state.overlay.snapshot().unwrap();
        let dir = panel_contract::PanelDir::create().unwrap();
        let baseline = super::super::PanelState::retirement_baseline(dir.path()).unwrap();
        state
            .retire(7, baseline, super::super::Retirement::default())
            .unwrap();
        assert!(!state.accepts_overlay(7, last.incarnation));
        assert!(state.document().is_err());
    }

    #[test]
    fn merged_result_ceiling_refuses_atomically() {
        let mut state = held();
        let before = state.document().unwrap();
        let mut hook = parse_document(r#"{"attention":"wait","lines":["x"]}"#).unwrap();
        // A legal hook can exceed the ceiling once holder metadata and buttons are retained.
        let filler = "x".repeat(panel_contract::MAX_BYTES as usize - 55);
        let text = serde_json::json!({"attention":"wait","lines":[filler]}).to_string();
        assert!(text.len() <= panel_contract::MAX_BYTES as usize);
        hook.lines = parse_document(&text).unwrap().lines;
        assert!(state.apply_overlay(hook, SystemTime::now()).is_err());
        assert_eq!(state.document().unwrap(), before);
    }
}
