//! Immutable projection of already-refreshed authority; never a second transport.
use super::{AttentionState, PanelState};
use panel_contract::Panel;
use std::{sync::Arc, time::SystemTime};

pub(crate) struct Snapshot {
    pub(crate) document: Option<Arc<Panel>>,
    pub(crate) warning: Option<String>,
    pub(crate) attention: Option<AttentionState>,
}

pub(crate) fn snapshot(state: &PanelState, now: SystemTime) -> Snapshot {
    if state.is_retired() {
        return Snapshot {
            document: None,
            warning: None,
            attention: None,
        };
    }
    let warning = state.warning(now);
    let attention = state.attention_with(now, warning.is_none());
    Snapshot {
        document: state.last_good.clone(),
        warning,
        attention,
    }
}

pub(crate) fn body_symbol(mark: panel_contract::Mark) -> char {
    super::symbol(mark)
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_contract::Attention;
    use std::time::{Duration, SystemTime};

    #[test]
    fn snapshot_shares_the_document_and_uses_existing_done_backing_exception() {
        let mut document =
            panel_contract::parse_document(r#"{"attention":"wait","lines":[{"text":"literal"}]}"#)
                .unwrap();
        document.written_at = Some(SystemTime::UNIX_EPOCH);
        document.stale_after = Duration::from_secs(1);
        let document = Arc::new(document);
        let mut state = PanelState {
            last_good: Some(document.clone()),
            ..PanelState::default()
        };
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(60);
        let projected = snapshot(&state, now);
        assert!(Arc::ptr_eq(projected.document.as_ref().unwrap(), &document));
        assert!(projected.warning.is_some());
        assert_eq!(
            projected.attention,
            Some(AttentionState {
                class: Attention::Wait,
                backed: false
            })
        );
        Arc::make_mut(state.last_good.as_mut().unwrap()).attention = Some(Attention::Done);
        let projected = snapshot(&state, now);
        assert!(projected.warning.is_some());
        assert_eq!(
            projected.attention,
            Some(AttentionState {
                class: Attention::Done,
                backed: true
            })
        );
    }
}
