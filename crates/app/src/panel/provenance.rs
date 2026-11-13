//! Application-owned provenance is adjacent to the document, never part of producer JSON.
use std::time::SystemTime;

use panel_control::{Connection, PublicationOrigin};

use super::{Panel, PanelState, Row, Style, Theme};

const OUTSIDE: &str = "published from a process outside this console";

/// A failed publication cannot replace the current document's observation.
pub(crate) fn deliver_observed(
    state: &mut PanelState,
    connection: Connection,
    source: &str,
    panel: Panel,
    now: SystemTime,
    origin: PublicationOrigin,
) -> Result<(), String> {
    state.deliver(connection, source, panel, now)?;
    state.published_by = origin;
    Ok(())
}

pub(super) fn append_row(rows: &mut Vec<Row>, state: &PanelState, width: usize, theme: &Theme) {
    if state.published_by == PublicationOrigin::Outside {
        super::wrap(
            rows,
            0,
            OUTSIDE,
            theme.panel_color(None, Some(Style::Warn)),
            width,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use panel_contract::{PANEL_FILE, PanelDir, parse_document, parse_ops};
    use std::time::Duration;

    fn panel(done: bool) -> Panel {
        parse_document(if done {
            r#"{"header":false,"attention":"done","lines":[{"text":"Body","style":"section","collapsed":true},"content"]}"#
        } else {
            r#"{"header":false,"lines":["content"]}"#
        }).unwrap()
    }

    fn write(dir: &PanelDir, text: &str, at: SystemTime) {
        let path = dir.path().join(PANEL_FILE);
        std::fs::write(&path, text).unwrap();
        std::fs::File::open(path).unwrap().set_modified(at).unwrap();
    }

    #[test]
    fn observation_survives_disconnect_and_only_a_valid_newer_file_replaces_it() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let dir = PanelDir::create().unwrap();
        let mut state = PanelState::default();
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(true),
            at,
            PublicationOrigin::Outside,
        )
        .unwrap();
        assert!(!state.document().unwrap().contains(OUTSIDE));
        state.release(1);
        for (text, stamp) in [
            ("{", at + Duration::from_secs(1)),
            (r#"{"lines":["old"]}"#, at),
            (r#"{"lines":["older"]}"#, at - Duration::from_secs(1)),
        ] {
            write(&dir, text, stamp);
            state.refresh(dir.path());
            assert_eq!(state.published_by, PublicationOrigin::Outside);
        }
        write(&dir, r#"{"lines":["new"]}"#, at + Duration::from_secs(2));
        state.refresh(dir.path());
        assert_eq!(state.published_by, PublicationOrigin::Unknown);
    }

    #[test]
    fn patches_keep_observation_refusals_do_not_change_it_and_full_publications_replace_it() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let mut state = PanelState::default();
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(false),
            at,
            PublicationOrigin::Outside,
        )
        .unwrap();
        let ops = parse_ops(&serde_json::json!([{"op":"set","field":"attention","value":"done"}]))
            .unwrap();
        state.patch(1, &ops, at).unwrap();
        assert_eq!(state.published_by, PublicationOrigin::Outside);
        assert!(
            deliver_observed(
                &mut state,
                2,
                "other",
                panel(false),
                at,
                PublicationOrigin::Inside
            )
            .is_err()
        );
        assert_eq!(state.published_by, PublicationOrigin::Outside);
        for origin in [
            PublicationOrigin::Inside,
            PublicationOrigin::Unknown,
            PublicationOrigin::Outside,
        ] {
            deliver_observed(&mut state, 1, "agent", panel(false), at, origin).unwrap();
            assert_eq!(state.published_by, origin);
        }
    }

    #[test]
    fn app_row_survives_hidden_header_and_block_folds_and_invalidates_cached_layout() {
        let at = SystemTime::now();
        let theme = Theme::default();
        let mut state = PanelState::default();
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(true),
            at,
            PublicationOrigin::Outside,
        )
        .unwrap();
        let (rows, cached) = state
            .lay_out_if_needed("pane", 100, &theme, None, None)
            .unwrap();
        assert!(!cached);
        assert!(rows.iter().any(|row| row.text == OUTSIDE));
        assert!(!rows.iter().any(|row| row.text == "content"));
        assert!(
            state
                .lay_out_if_needed("pane", 100, &theme, None, None)
                .unwrap()
                .1
        );
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(true),
            at,
            PublicationOrigin::Unknown,
        )
        .unwrap();
        let (rows, cached) = state
            .lay_out_if_needed("pane", 100, &theme, None, None)
            .unwrap();
        assert!(!cached);
        assert!(!rows.iter().any(|row| row.text == OUTSIDE));
    }

    #[test]
    fn retirement_clears_observation() {
        let dir = PanelDir::create().unwrap();
        let mut state = PanelState::default();
        deliver_observed(
            &mut state,
            1,
            "agent",
            panel(true),
            SystemTime::now(),
            PublicationOrigin::Outside,
        )
        .unwrap();
        state
            .retire(
                1,
                PanelState::retirement_baseline(dir.path()).unwrap(),
                super::super::Retirement::default(),
            )
            .unwrap();
        assert_eq!(state.published_by, PublicationOrigin::Unknown);
    }
}
